//! 実file作用用の有限fixture。compiler/model/host証拠は生成しない。
//! Bounded actual-file fixtures; no compiler/model/host evidence is fabricated.
#![allow(dead_code)]
use lc631_analysis::*;
use lc631_core::*;
use lc631_receipt_kernel::*;
use lc631_runtime::resource::*;
use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const SOURCE: &str = "Please edit the source and test the package.";
pub const BEFORE: &str = "pub fn value() -> u8 { 0 }\n";
pub const AFTER: &str = "pub fn value() -> u8 { 1 }\n";
pub fn root() -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let p = std::env::temp_dir().join(format!("tlcx32-verified-{}-{now}", std::process::id()));
    std::fs::create_dir_all(p.join("src")).unwrap();
    std::fs::write(
        p.join("Cargo.toml"),
        "[package]\nname='tlcx32_verified'\nversion='0.0.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(p.join("src/lib.rs"), BEFORE).unwrap();
    p
}
pub fn permit(scope: &str, issuer: &ReceiptIssuer, verifier: &ReceiptVerifier) -> ExecutionPermit {
    let parent = stable_sha256("TLCX32 fixture-only host authority, not actual host ingress");
    let event = AuthorityEvent {
        action: Action::Edit,
        source: AuthoritySourceKind::VerifiedHostUserSpan,
        polarity: DeonticPolarity::Grant,
        span: SourceSpan {
            start: 0,
            end: SOURCE.len(),
        },
        scope: scope.into(),
    };
    let subject = SubjectRevision::checked(stable_sha256(SOURCE)).unwrap();
    let receipt_scope = ReceiptScope::checked(format!("authority/edit/{scope}")).unwrap();
    let payload = authority_event_payload_digest(&event);
    let raw = issuer
        .issue(
            ReceiptClass::Authority,
            subject.clone(),
            receipt_scope.clone(),
            payload.clone(),
            100,
            Some(400),
            Some(parent.clone()),
        )
        .unwrap();
    let verified = verifier
        .verify(
            raw,
            &ReceiptPolicy::exact(
                ReceiptClass::Authority,
                subject,
                receipt_scope,
                payload,
                100,
            )
            .with_parent(&parent),
            &mut ReplayGuard::default(),
        )
        .unwrap();
    ExecutionPermit::from_verified_user_event(
        VerifiedAuthorityEvent::checked(event, verified).unwrap(),
        SOURCE,
        AuthorityRevision(1),
        100,
        400,
        0,
    )
    .unwrap()
}
pub struct Inputs<'a> {
    pub run: &'a RunResources,
}
impl ToolProcessRunner for Inputs<'_> {
    fn run(
        &mut self,
        _source: &str,
        _request: BoundedToolRequest<'_>,
        _consume: &mut dyn FnMut(&ResourceToolResult<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        panic!("application observation fixture starts no compiler process")
    }
    fn with_input(
        &mut self,
        source: &str,
        cap: usize,
        consume: &mut dyn FnMut(&ResourceReservationView<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        let mut reservation = self.run.reserve_for_source(
            context::SourceRevision::of(source).digest(),
            &[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(cap as u64)?,
            )],
        )?;
        let result = consume(&reservation.lease());
        reservation.release_confirmed()?;
        result
    }
}
pub struct Effects<'a> {
    pub run: &'a RunResources,
    pub reject: bool,
    pub calls: usize,
}
impl DgclFileEffectOwner for Effects<'_> {
    fn apply(
        &mut self,
        plan: DgclFileEffectPlan<'_>,
        operation: &mut dyn FnMut(
            &ResourceReservationView<'_>,
        ) -> Result<(), DgclRepairDriverError>,
    ) -> Result<(), DgclRepairDriverError> {
        self.calls += 1;
        if self.reject {
            return Err(DgclRepairDriverError::Failed);
        }
        let mut reservation = self
            .run
            .reserve_for_source(
                context::SourceRevision::of(plan.source).digest(),
                &[ResourceRequest::new(
                    ResourceKind::Disk,
                    ResourceAmount::checked(2 * 1024 * 1024).unwrap(),
                )],
            )
            .map_err(|_| DgclRepairDriverError::Failed)?;
        let result = operation(&reservation.lease());
        reservation
            .release_confirmed()
            .map_err(|_| DgclRepairDriverError::Failed)?;
        result
    }
}
pub struct EmptyValidation;
impl DgclFileEvidenceValidator for EmptyValidation {
    fn observe(
        &mut self,
        _snapshot: &DgclExecutionSnapshot,
        _budget: Duration,
    ) -> Result<Vec<CodingEvidenceClaim>, DgclRepairDriverError> {
        Ok(Vec::new())
    }
}
pub fn parse(s: &str) -> lc631_substrate::Digest32 {
    let hex = s.strip_prefix("sha256:").unwrap();
    let mut out = [0; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
    }
    lc631_substrate::Digest32::from_bytes(out)
}
