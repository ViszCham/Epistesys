//! 日本語: 純粋なbounded plan。実compiler/authority/source観測とは分離する。
//! English: A bounded plan is separate from actual compiler, authority and source observations.
use super::{
    admission, grammar, AuthorityStamp, BuildError, BuildPlanInput, BuildStage, CompilerSpec,
};
use lc631_receipt_kernel::ReceiptClass;
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    Digest32,
};
use sha2::{Digest, Sha256};
use std::{cell::Cell, path::PathBuf};

pub(super) const DRIVER: &str = "fn main(){use std::io::Read;let mut s=String::new();std::io::stdin().read_to_string(&mut s).unwrap();for line in s.lines(){let v:Vec<u32>=line.split_whitespace().map(|x|x.parse().unwrap()).collect();match checked_add(v[0],v[1]){Some(n)=>println!(\"some:{n}\"),None=>println!(\"none\")}}}\n";

#[derive(Clone, Copy, Default)]
pub(super) struct PlanWork {
    pub(super) builds: u32,
    pub(super) reads: u32,
    pub(super) authority: Option<AuthorityStamp>,
}
pub struct BuildPlan<'s, 'v> {
    pub(super) input: BuildPlanInput<'s>,
    pub(super) subject_digest: Digest32,
    pub(super) driver_digest: Digest32,
    pub(super) frame_digest: Digest32,
    pub(super) cwd_digest: Digest32,
    pub(super) environment_digest: Digest32,
    pub(super) subject_path: PathBuf,
    pub(super) driver_path: PathBuf,
    pub(super) frame_path: PathBuf,
    pub(super) output_path: PathBuf,
    pub(super) frame_source: String,
    pub(super) work: Cell<PlanWork>,
    pub(super) retained: ResourceClaim<'v>,
}
impl<'s, 'v> BuildPlan<'s, 'v> {
    /// Planned compiler input, not evidence that a compiler executed it.
    pub fn frame_source(&self) -> &str {
        &self.frame_source
    }

    pub fn given(
        input: BuildPlanInput<'s>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, BuildError> {
        view.check()?;
        admission::validate(&input)?;
        if view.source_revision() != input.binding.source() {
            return Err(ResourceError::SourceMismatch.into());
        }
        let retained = claim(view, admission::PLAN_RAM)?;
        let scratch = claim(view, admission::PARSE_RAM)?;
        input.binding.check_goal_profile_inputs(view)?;
        grammar::admit(input.subject.text())?;
        let expected_payload = compiler_registration_payload(input.compiler);
        let expected_scope = compiler_registration_scope(input.compiler);
        let expected_subject = digest_text(input.compiler.expected_image);
        let claims = input.compiler_registration.claims();
        // These checks reject mismatched descriptors only. Actual authentication
        // against the fresh authority provider's verifier remains in the producer.
        if claims.class() != ReceiptClass::ToolExecution
            || claims.subject_revision().as_str() != expected_subject
            || claims.scope().as_str() != expected_scope
            || claims.payload_digest() != expected_payload
        {
            return Err(BuildError::CompilerRegistrationMismatch);
        }
        let frame_len = input.subject.text().len() + 1 + DRIVER.len();
        let mut frame_source = String::new();
        frame_source
            .try_reserve_exact(frame_len)
            .map_err(|_| ResourceError::AllocationFailed)?;
        frame_source.push_str(input.subject.text());
        frame_source.push('\n');
        frame_source.push_str(DRIVER);
        let subject_digest = input.subject.id().digest();
        let driver_digest = digest(DRIVER.as_bytes());
        let frame_digest = digest(frame_source.as_bytes());
        let cwd_digest = digest(
            input
                .frame
                .cwd
                .to_str()
                .ok_or(BuildError::FrameMismatch)?
                .as_bytes(),
        );
        let mut env_hash = Sha256::new();
        env_hash.update(b"Epistesys/SourceBuild/Environment/v1\0");
        field(
            &mut env_hash,
            &(input.frame.environment.len() as u64).to_le_bytes(),
        );
        for entry in input.frame.environment {
            field(&mut env_hash, entry.name.as_bytes());
            field(&mut env_hash, entry.value.as_bytes());
        }
        let environment_digest = finish(env_hash);
        let plan = Self {
            subject_path: input.root.join(input.subject_relative),
            driver_path: input.root.join(input.driver_relative),
            frame_path: input.root.join(input.frame_relative),
            output_path: input.root.join(input.output_relative),
            input,
            subject_digest,
            driver_digest,
            frame_digest,
            cwd_digest,
            environment_digest,
            frame_source,
            work: Cell::new(PlanWork::default()),
            retained,
        };
        scratch.check()?;
        plan.check(view)?;
        Ok(plan)
    }
    pub fn frame_digest(&self) -> Digest32 {
        self.frame_digest
    }
    pub fn root_digest(&self) -> Digest32 {
        self.input.binding.root()
    }
    pub fn program_digest(&self) -> Digest32 {
        self.input.binding.program()
    }
    pub fn plan_digest(&self) -> Digest32 {
        self.input.binding.plan()
    }
    pub fn cwd_digest(&self) -> Digest32 {
        self.cwd_digest
    }
    pub fn environment_digest(&self) -> Digest32 {
        self.environment_digest
    }

    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), BuildError> {
        super::check_claim(&self.retained, view)?;
        if view.source_revision() != self.input.binding.source() {
            return Err(ResourceError::SourceMismatch.into());
        }
        Ok(())
    }
    pub(super) fn begin_build(&self, view: &ResourceReservationView<'_>) -> Result<(), BuildError> {
        self.check(view)?;
        let mut work = self.work.get();
        if work.builds >= self.input.limits.max_builds {
            return Err(BuildError::BuildAlreadyAttempted);
        }
        work.builds += 1;
        self.work.set(work);
        Ok(())
    }
    pub(super) fn debit_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), BuildError> {
        self.check(view)?;
        let mut work = self.work.get();
        if work.reads >= self.input.limits.max_current_reads {
            return Err(BuildError::ReadBudget);
        }
        work.reads += 1;
        self.work.set(work);
        Ok(())
    }
    pub(super) fn observe_authority(&self, stamp: AuthorityStamp) -> Result<(), BuildError> {
        let mut work = self.work.get();
        if let Some(previous) = work.authority {
            if stamp.epoch < previous.epoch {
                return Err(BuildError::ClockRollback);
            }
            if stamp.revocation < previous.revocation {
                return Err(BuildError::RevocationRollback);
            }
            if stamp.principal != previous.principal || stamp.host != previous.host {
                return Err(BuildError::Unauthorized);
            }
        }
        // A checked observation is not erased when its interval then refuses use.
        work.authority = Some(stamp);
        self.work.set(work);
        if stamp.epoch >= stamp.expires {
            return Err(BuildError::Expired);
        }
        Ok(())
    }
    pub(super) fn args(&self, stage: BuildStage) -> Vec<String> {
        match stage {
            BuildStage::CompilerVersion => vec!["-vV".to_owned()],
            BuildStage::CompilerSysroot => vec!["--print".to_owned(), "sysroot".to_owned()],
            BuildStage::Link => vec![
                "--edition=2021".to_owned(),
                "--crate-type=bin".to_owned(),
                "--crate-name".to_owned(),
                "epistesys_sbo".to_owned(),
                "--emit=link".to_owned(),
                "-C".to_owned(),
                "opt-level=0".to_owned(),
                "--sysroot".to_owned(),
                path_text(self.input.compiler.declared_sysroot),
                "-o".to_owned(),
                path_text(&self.output_path),
                path_text(&self.frame_path),
            ],
            BuildStage::CurrentOrigin => Vec::new(),
        }
    }
    pub fn options_digest(&self, stage: BuildStage) -> Digest32 {
        let mut h = Sha256::new();
        h.update(b"Epistesys/SourceBuild/Options/v1\0");
        field(&mut h, stage.label().as_bytes());
        let args = self.args(stage);
        field(&mut h, &(args.len() as u64).to_le_bytes());
        for argument in args {
            field(&mut h, argument.as_bytes());
        }
        finish(h)
    }
    pub fn invocation_digest(&self, stage: BuildStage) -> Digest32 {
        let mut h = Sha256::new();
        h.update(b"Epistesys/SourceBuild/Invocation/v1\0");
        field(&mut h, stage.label().as_bytes());
        field(
            &mut h,
            self.input
                .compiler
                .executable
                .to_str()
                .expect("admitted UTF-8 executable")
                .as_bytes(),
        );
        for pin in [
            self.root_digest(),
            self.program_digest(),
            self.plan_digest(),
            self.subject_digest,
            self.driver_digest,
            self.frame_digest,
            self.cwd_digest,
            self.environment_digest,
            self.input.compiler.expected_image,
            self.input.compiler.expected_verbose,
            self.input.compiler.toolchain_profile,
            self.options_digest(stage),
        ] {
            field(&mut h, pin.as_bytes());
        }
        field(&mut h, self.input.entrypoint.as_bytes());
        field(&mut h, &self.input.limits.child_timeout_ms.to_le_bytes());
        field(
            &mut h,
            &(self.input.limits.max_metadata_bytes as u64).to_le_bytes(),
        );
        field(
            &mut h,
            &(self.input.limits.max_metadata_bytes as u64).to_le_bytes(),
        );
        field(&mut h, &[0]); // fixed compiler stages have no stdin
        finish(h)
    }
    pub fn execution_scope(&self, stage: BuildStage) -> String {
        format!(
            "tlcx/source-build/v1/{}/{}/{}/{}",
            hex(self.root_digest()),
            hex(self.plan_digest()),
            stage.label(),
            hex(self.invocation_digest(stage))
        )
    }
}

/// Pure registration metadata; no receipt, execution or authority is issued.
pub fn compiler_registration_scope(spec: CompilerSpec<'_>) -> String {
    if !admission::compiler_shape(spec) {
        return String::new();
    }
    format!(
        "tlcx/source-build/compiler/v1/{}",
        hex(compiler_identity(spec))
    )
}
pub fn compiler_registration_payload(spec: CompilerSpec<'_>) -> String {
    if !admission::compiler_shape(spec) {
        return String::new();
    }
    digest_text(compiler_identity(spec))
}
fn compiler_identity(spec: CompilerSpec<'_>) -> Digest32 {
    let mut h = Sha256::new();
    h.update(b"Epistesys/SourceBuild/CompilerRegistration/v1\0");
    field(
        &mut h,
        spec.executable
            .to_str()
            .expect("admitted UTF-8 compiler")
            .as_bytes(),
    );
    field(&mut h, spec.expected_image.as_bytes());
    field(&mut h, spec.expected_verbose.as_bytes());
    field(
        &mut h,
        spec.declared_sysroot
            .to_str()
            .expect("admitted UTF-8 sysroot")
            .as_bytes(),
    );
    field(&mut h, spec.toolchain_profile.as_bytes());
    finish(h)
}
pub(super) fn field(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
pub(super) fn digest(bytes: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(bytes).into())
}
fn finish(hash: Sha256) -> Digest32 {
    Digest32::from_bytes(hash.finalize().into())
}
pub(super) fn hex(pin: Digest32) -> String {
    let mut text = String::with_capacity(64);
    for byte in pin.as_bytes() {
        use std::fmt::Write as _;
        let _ = write!(text, "{byte:02x}");
    }
    text
}
fn digest_text(pin: Digest32) -> String {
    format!("sha256:{}", hex(pin))
}
fn path_text(path: &std::path::Path) -> String {
    path.to_str().expect("admitted UTF-8 path").to_owned()
}
fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    bytes: u64,
) -> Result<ResourceClaim<'v>, BuildError> {
    Ok(view.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(bytes)?,
    )])?)
}
