//! Real isolated Cargo package fixtures. No checked observations are fabricated.
pub mod authority;
pub mod validation;
use lc631_analysis::*;
use lc631_core::{stable_sha256, ExecutionPermit};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
pub const SOURCE: &str = "Please edit the source and test the package.";

pub fn fixture_package() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("tlcx31-real-total-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../fixtures/dgcl/producer-package");
    for name in [
        "Cargo.toml",
        "Cargo.lock",
        "src/lib.rs",
        "tests/acceptance.rs",
        "tests/connection.rs",
    ] {
        std::fs::copy(fixture.join(name), root.join(name)).unwrap();
    }
    root
}

pub fn cargo_path() -> PathBuf {
    let output = std::process::Command::new("rustup")
        .args(["which", "cargo"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fixture requires installed Cargo, no installation"
    );
    PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
}

pub fn request(
    pipeline: &DgclPipelineReport,
    permit: &ExecutionPermit,
    root: &Path,
    attempt: u8,
    before: &str,
    replacement: &str,
) -> DgclRepairRequest {
    let mut request = DgclRepairRequest {
        attempt,
        gap_id: pipeline
            .implementation_closure
            .gaps
            .iter()
            .find(|gap| gap.evidence_kind == Some(CodingEvidenceKind::RuntimeValidation))
            .unwrap()
            .gap_id
            .clone(),
        source_revision: pipeline.source_revision.clone(),
        plan_digest: pipeline.completion_plan.plan_digest.clone(),
        target_ref: "src/lib.rs".into(),
        target_digest_before: stable_sha256(before),
        candidate_digest: stable_sha256(replacement),
        authorization_scope: String::new(),
        authorization_span: permit.source_span(),
        repository_root_digest: Some(dgcl_repository_root_digest(root).unwrap()),
    };
    request.authorization_scope = repair_request_authorization_scope(&request);
    request
}

pub struct CountingFileDriver<'a> {
    pub inner: DgclFileRepairDriver<'a>,
    pub applies: usize,
    pub validations: usize,
    pub fail_after_actual_apply: bool,
}
impl DgclRepairDriver for CountingFileDriver<'_> {
    fn observation_epoch(&self) -> Option<u64> {
        self.inner.observation_epoch()
    }
    fn apply(
        &mut self,
        request: &DgclRepairRequest,
        permit: &ExecutionPermit,
        time: Duration,
        delta: u64,
    ) -> Result<DgclRepairApplication, DgclRepairDriverError> {
        self.applies += 1;
        let application = self.inner.apply(request, permit, time, delta)?;
        if self.fail_after_actual_apply {
            // Actual file write/backup happened, but its receipt is withheld
            // from the coordinator by this explicit return-boundary cut.
            return Err(DgclRepairDriverError::TimedOut);
        }
        Ok(application)
    }
    fn revalidate(
        &mut self,
        request: &DgclRepairRequest,
        application: &DgclRepairApplication,
        time: Duration,
    ) -> Result<DgclRepairValidation, DgclRepairDriverError> {
        self.validations += 1;
        self.inner.revalidate(request, application, time)
    }
}
