//! Actual artifact current-gate cuts. No fallback scores or claimed host authority.
mod tlcx22_goal_artifact_support;
use tlcx22_goal_artifact_support::*;
use lc631_analysis::declared_build_capture::{CurrentSnapshotAuthority, SnapshotAuthorityView, SnapshotCaptureError};
use lc631_runtime::goal_suite_measurement as g;
use lc631_substrate::resource::{ResourceAmount, ResourceError, ResourceKind, ResourceRequest};
use std::time::Duration;
use std::cell::Cell;
fn expired() -> u64 { 401 }
fn rollback() -> u64 { 99 }
struct LateReject<'a> { inner: &'a mut dyn CurrentSnapshotAuthority, calls: usize }
impl CurrentSnapshotAuthority for LateReject<'_> {
    fn current(&mut self) -> Result<SnapshotAuthorityView<'_>, SnapshotCaptureError> {
        self.calls += 1;
        if self.calls > 1 { return Err(SnapshotCaptureError::AuthorityUnavailable); }
        self.inner.current()
    }
}
struct NeverAuthority<'a>(&'a Cell<usize>);
impl CurrentSnapshotAuthority for NeverAuthority<'_> {
    fn current(&mut self) -> Result<SnapshotAuthorityView<'_>, SnapshotCaptureError> {
        self.0.set(self.0.get() + 1); Err(SnapshotCaptureError::AuthorityUnavailable)
    }
}

#[test]
fn tlcx22_artifact_current_native_test_expiry_and_clock_rollback_are_not_goal_zero() {
    for (clock, is_expiry) in [(expired as fn() -> u64, true), (rollback as fn() -> u64, false)] {
        fixture(|h, current, view| {
            let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
            let owner = g::measure_artifact_pair(&root, h.pair(), current, view).unwrap();
            let baseline = invocation(&current.baseline_invocation, current.baseline_invocation.authority);
            let mut candidate = invocation(&current.candidate_invocation, current.candidate_invocation.authority);
            candidate.clock = clock;
            let mut cut = g::ArtifactCurrent { baseline_invocation: baseline, candidate_invocation: candidate,
                baseline_build: &mut *current.baseline_build, candidate_build: &mut *current.candidate_build,
                baseline_authority: &mut *current.baseline_authority,
                candidate_authority: &mut *current.candidate_authority };
            let error = owner.with_current(&root, h.pair(), &mut cut, view, |_| ()).err().unwrap();
            if is_expiry { assert!(matches!(error, g::Error::Expired)); }
            else { assert!(matches!(error, g::Error::ClockRollback)); }
        });
    }
}

#[test]
fn tlcx22_artifact_actual_resource_deadline_cuts_already_owned_metric_and_noescape_entry() {
    fixture_with_deadline(native::AFTER, Duration::from_secs(3), |h, current, view| {
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let owner = g::measure_artifact_pair(&root, h.pair(), current, view).unwrap();
        owner.with_current(&root, h.pair(), current, view, |v| {
            assert_eq!(value(v.solve()), &ratio(1, 4));
        }).unwrap();
        loop { match view.remaining() {
            Ok(left) => std::thread::sleep(left),
            Err(ResourceError::DeadlineExceeded) => break,
            Err(e) => panic!("unexpected deadline cause {e:?}"),
        } }
        assert!(matches!(owner.with_current(&root, h.pair(), current, view, |_| ()),
            Err(g::Error::Resource(ResourceError::DeadlineExceeded))));
    });
}

#[test]
fn tlcx22_artifact_late_build_authority_rejection_cannot_refresh_frozen_expected_origin() {
    fixture(|h, current, view| {
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let owner = g::measure_artifact_pair(&root, h.pair(), current, view).unwrap();
        let baseline = invocation(&current.baseline_invocation, current.baseline_invocation.authority);
        let candidate = invocation(&current.candidate_invocation, current.candidate_invocation.authority);
        let mut reject = LateReject { inner: &mut *current.candidate_authority, calls: 0 };
        let mut cut = g::ArtifactCurrent { baseline_invocation: baseline, candidate_invocation: candidate,
            baseline_build: &mut *current.baseline_build, candidate_build: &mut *current.candidate_build,
            baseline_authority: &mut *current.baseline_authority, candidate_authority: &mut reject };
        assert!(matches!(owner.with_current(&root, h.pair(), &mut cut, view, |_| ()),
            Err(g::Error::BuildCapture(SnapshotCaptureError::AuthorityUnavailable))));
        assert_eq!(cut.candidate_build.binding().root(), h.candidate_origin.root());
        assert_eq!(h.baseline_origin.content_pins().build, h.baseline.build_input_digest());
    });
}

#[test]
fn tlcx22_artifact_ram_refusal_preserves_actual_observations_without_fabricating_scalar() {
    fixture(|h, current, view| {
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let mut fills = Vec::new(); let mut exhausted = false;
        for _ in 0..24576 { // finite96MiB reservation /4KiB; no production-layout oracle
            let request = ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(4096).unwrap());
            match view.claim_resources(&[request]) {
                Ok(fill) => fills.push(fill),
                Err(ResourceError::Exhausted(ResourceKind::Ram)) => { exhausted = true; break; }
                Err(e) => panic!("unexpected RAM cause {e:?}"),
            }
        }
        assert!(exhausted);
        assert!(matches!(g::measure_artifact_pair(&root, h.pair(), current, view),
            Err(g::Error::Resource(ResourceError::Exhausted(ResourceKind::Ram)))
            | Err(g::Error::BuildCapture(SnapshotCaptureError::Resource(ResourceError::Exhausted(ResourceKind::Ram))))));
        assert_eq!(h.baseline.actual_outputs(), &BASE); assert_eq!(h.candidate.actual_outputs(), &HAND);
        drop(fills);
        let owner = g::measure_artifact_pair(&root, h.pair(), current, view).unwrap();
        owner.with_current(&root, h.pair(), current, view, |v| {
            assert_eq!(value(v.solve()), &ratio(1, 4));
        }).unwrap();
    });
}

#[test]
fn tlcx22_artifact_partial_native_output_never_constructs_a_metric_observation_owner() {
    fixture(|h, current, view| {
        let path = h.candidate_root.join("partial-output.rs");
        let program = h.candidate_root.join(format!("partial-output{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&path, "fn main(){println!(\"some:0\");println!(\"some:3\");}\n").unwrap();
        native::compile_native(&path, &program);
        let issuer = lc631_receipt_kernel::ReceiptIssuer::from_key_bytes("partial-fixture", &[122; 32]).unwrap();
        let verifier = lc631_receipt_kernel::ReceiptVerifier::from_key_bytes("partial-fixture", &[122; 32]).unwrap();
        let fixture = native::NativeProbeFixture::registered(h.plan,
            current.candidate_invocation.registration.snapshot.clone(),
            h.candidate_root.join("src/lib.rs"), program, &issuer, &verifier);
        fixture.with_invocation(&verifier, |input| {
            assert!(matches!(lc631_runtime::contextual_repair::execution::observe_current(h.plan, input, view),
                Err(lc631_runtime::contextual_repair::measurement::ProbeError::InvalidOutput)));
        });
        // This is a parser/actual-effect cut, not source correspondence proof.
    });
}

#[test]
fn tlcx22_artifact_source_snapshot_and_revocations_are_admitted_before_hash_or_provider() {
    for kind in 0..3 { fixture(|h, current, view| {
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let source = "x".repeat(256 * 1024 + 1);
        let mut snapshot = current.candidate_invocation.registration.snapshot.clone();
        let target = snapshot.targets[0].clone(); snapshot.targets.resize(65, target);
        let revocations = vec![format!("sha256:{}", "00".repeat(32)); 257];
        let a = current.candidate_invocation.authority;
        let authority = lc631_core::ExecutionPermitContext { principal: a.principal, action: a.action,
            scope: a.scope, source_revision: a.source_revision, authority_revision: a.authority_revision,
            now_epoch: a.now_epoch, revocation_revision: a.revocation_revision,
            revoked_permit_digests: if kind == 2 { &revocations } else { a.revoked_permit_digests },
            caller_origin: a.caller_origin, trusted_host_fingerprint: a.trusted_host_fingerprint };
        let baseline = invocation(&current.baseline_invocation, current.baseline_invocation.authority);
        let mut candidate = invocation(&current.candidate_invocation, &authority);
        if kind == 0 { candidate.source = &source; }
        if kind == 1 { candidate.registration.snapshot = &snapshot; }
        let calls = Cell::new(0); let mut ba = NeverAuthority(&calls); let mut ca = NeverAuthority(&calls);
        let mut cut = g::ArtifactCurrent { baseline_invocation: baseline, candidate_invocation: candidate,
            baseline_build: &mut *current.baseline_build, candidate_build: &mut *current.candidate_build,
            baseline_authority: &mut ba, candidate_authority: &mut ca };
        assert!(matches!(g::measure_artifact_pair(&root, h.pair(), &mut cut, view), Err(g::Error::InputLimit)));
        assert_eq!(calls.get(), 0);
        assert_eq!(h.baseline.actual_outputs(), &BASE); assert_eq!(h.candidate.actual_outputs(), &HAND);
    }); }
}

struct NeverReader<'a>(&'a Cell<usize>);
impl lc631_analysis::ToolProcessRunner for NeverReader<'_> {
    fn run(&mut self, _: &str, _: lc631_analysis::BoundedToolRequest<'_>,
        _: &mut dyn FnMut(&lc631_analysis::ResourceToolResult<'_>) -> Result<(), ResourceError>)
        -> Result<(), ResourceError> { self.0.set(self.0.get() + 1); Err(ResourceError::InvalidConfig) }
    fn with_input(&mut self, _: &str, _: usize,
        _: &mut dyn FnMut(&lc631_substrate::resource::ResourceReservationView<'_>) -> Result<(), ResourceError>)
        -> Result<(), ResourceError> { self.0.set(self.0.get() + 1); Err(ResourceError::InvalidConfig) }
}
#[test]
fn tlcx22_artifact_huge_private_port_pipeline_is_rejected_before_provider_or_inventory() {
    for wide_vector in [true, false] { fixture(|h, current, view| {
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let mut poisoned = h.pipeline.clone();
        if wide_vector { poisoned.unsupported_operative_region_ids = vec![0; 65537]; }
        else { poisoned.program_ir.source_revision = "x".repeat(256 * 1024 + 1); }
        let calls = Cell::new(0); let mut reader = NeverReader(&calls);
        let mut bad = lc631_runtime::contextual_repair::snapshot_gate::CodingBuildPort::checked(
            h.candidate_root, native::SOURCE, &poisoned, h.candidate_origin.root(),
            h.candidate_origin.program(), h.candidate_origin.plan(), &mut reader).unwrap();
        let baseline = invocation(&current.baseline_invocation, current.baseline_invocation.authority);
        let candidate = invocation(&current.candidate_invocation, current.candidate_invocation.authority);
        let mut ba = NeverAuthority(&calls); let mut ca = NeverAuthority(&calls);
        let mut cut = g::ArtifactCurrent { baseline_invocation: baseline, candidate_invocation: candidate,
            baseline_build: &mut *current.baseline_build, candidate_build: &mut bad,
            baseline_authority: &mut ba, candidate_authority: &mut ca };
        assert!(matches!(g::measure_artifact_pair(&root, h.pair(), &mut cut, view),
            Err(g::Error::SnapshotGate(lc631_runtime::contextual_repair::snapshot_gate::SnapshotGateError::Capture(
                SnapshotCaptureError::InventoryLimit)))));
        assert_eq!(calls.get(), 0);
    }); }
}
