//! Actual public producer fixture; only caller epochs are controlled here.
//! Local test issuers do not represent observed host users or fresh revocations.
#![allow(dead_code)]
#[path = "../tlcx31_coding_support/mod.rs"]
pub mod native;
#[path = "../../../lc631-analysis/tests/tlcx31_file_repair_fixture/mod.rs"]
mod package;

use lc631_analysis::{
    build_dgcl_pipeline, capture_dgcl_cargo_snapshot_with_runner,
    declared_build_capture::CurrentSnapshotAuthority, BoundedToolRequest, DgclBackendSet,
    ResourceToolResult, ToolProcessRunner,
};
use lc631_core::{context::SourceRevision, AuthorityRevision};
use lc631_receipt_kernel::{ReceiptIssuer, ReceiptVerifier};
use lc631_runtime::{
    contextual_repair::{execution::observe_current, measurement::*},
    resource::*,
};
use lc631_substrate::Digest32;
use std::{cell::Cell, time::Duration};

thread_local! {
    // Independent per test-thread/role inputs, not production global clocks.
    static BASE_EPOCH: Cell<u64> = const { Cell::new(100) };
    static CANDIDATE_EPOCH: Cell<u64> = const { Cell::new(100) };
}
fn baseline_clock() -> u64 {
    BASE_EPOCH.with(Cell::get)
}
fn candidate_clock() -> u64 {
    CANDIDATE_EPOCH.with(Cell::get)
}
pub fn epochs(baseline: u64, candidate: u64) {
    BASE_EPOCH.with(|cell| cell.set(baseline));
    CANDIDATE_EPOCH.with(|cell| cell.set(candidate));
}
struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        epochs(100, 100);
    }
}

// The inventory is read by the real registered snapshot implementation. A tool
// process request at this read-only staging boundary is an error, not fake output.
struct Reader<'a>(&'a RunResources);
impl ToolProcessRunner for Reader<'_> {
    fn run(
        &mut self,
        _: &str,
        _: BoundedToolRequest<'_>,
        _: &mut dyn FnMut(&ResourceToolResult<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        Err(ResourceError::InvalidConfig)
    }
    fn with_input(
        &mut self,
        source: &str,
        cap: usize,
        consume: &mut dyn FnMut(&ResourceReservationView<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        let amount = u64::try_from(cap).map_err(|_| ResourceError::ArithmeticOverflow)?;
        let mut reservation = self.0.reserve_for_source(
            SourceRevision::of(source).digest(),
            &[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(amount)?,
            )],
        )?;
        let result = consume(&reservation.lease());
        reservation.release_confirmed()?;
        result
    }
}
fn q(kind: ResourceKind, n: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap())
}

pub fn fixture(
    body: impl for<'v, 'lease> FnOnce(
        &CodingProbePlan,
        &OwnedCodingProbeObservation<'v>,
        &OwnedCodingProbeObservation<'v>,
        &'v ResourceReservationView<'lease>,
    ),
) {
    epochs(100, 100);
    let _reset = Reset;
    let root = package::fixture_package();
    eprintln!("retained clock-only native fixture: {}", root.display());
    // The existing package calls checked_add. Preserve its actual entrypoint.
    let code = native::AFTER
        .replace("fn add(", "fn checked_add(")
        .replace("match add(", "match checked_add(");
    let target = root.join("src/lib.rs");
    std::fs::write(&target, code).unwrap();
    let program = root.join(format!("clock-native{}", std::env::consts::EXE_SUFFIX));
    // Only central's explicit test execution runs rustc and native processes.
    native::compile_native(&target, &program);
    let pipeline = build_dgcl_pipeline(
        native::SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let plan = CodingProbePlan::checked_add(
        Digest32::from_bytes([21; 32]),
        Digest32::from_bytes([22; 32]),
    )
    .unwrap();
    let issuer = ReceiptIssuer::from_key_bytes("clock-native-fixture", &[123; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("clock-native-fixture", &[123; 32]).unwrap();
    let limits = ResourceLimits::uniform(256 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            3131,
            SourceRevision::of(native::SOURCE).digest(),
            limits,
            Deadline::after(Duration::from_secs(120)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            q(ResourceKind::Ram, 96 * 1024 * 1024),
            q(ResourceKind::Output, 2 * 1024 * 1024),
            q(ResourceKind::Child, 2),
            q(ResourceKind::Disk, 64 * 1024 * 1024),
            q(ResourceKind::Cleanup, 2),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        let mut reader = Reader(&run);
        let mut authority =
            native::snapshot_authority(&root, &issuer, &verifier, 100, AuthorityRevision(1));
        let snapshot = {
            let actual = authority.current().unwrap();
            capture_dgcl_cargo_snapshot_with_runner(
                &root,
                native::SOURCE,
                &pipeline,
                &actual.context,
                &mut reader,
            )
            .unwrap()
        };
        let baseline_fixture = native::NativeProbeFixture::registered_at(
            &plan,
            snapshot.clone(),
            target.clone(),
            program.clone(),
            &issuer,
            &verifier,
            100,
            baseline_clock,
        );
        let candidate_fixture = native::NativeProbeFixture::registered_at(
            &plan,
            snapshot,
            target,
            program,
            &issuer,
            &verifier,
            100,
            candidate_clock,
        );
        let baseline = baseline_fixture.with_invocation(&verifier, |input| {
            observe_current(&plan, input, &view).unwrap()
        });
        let candidate = candidate_fixture.with_invocation(&verifier, |input| {
            observe_current(&plan, input, &view).unwrap()
        });
        // TWO actual executions, no raw report/owner literal/deserialization.
        assert!(!std::ptr::eq(&baseline, &candidate));
        assert_eq!(
            baseline.actual_outputs(),
            &[Some(0), Some(3), None, Some(4294967295)]
        );
        assert_eq!(
            candidate.actual_outputs(),
            &[Some(0), Some(3), None, Some(4294967295)]
        );
        assert_eq!(baseline.checked_epoch(), 100);
        assert_eq!(candidate.checked_epoch(), 100);
        body(&plan, &baseline, &candidate, &view);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}
