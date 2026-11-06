//! Actual test artifacts/captures only. Local configured issuers are not host observations.
//! This fixture does not validate source-to-binary correspondence or coding completion.
#![allow(dead_code)]
#[path = "../tlcx31_coding_support/mod.rs"]
#[allow(
    clippy::duplicate_mod,
    reason = "separate integration fixture namespaces intentionally reuse native setup"
)]
pub mod native;
#[path = "../../../lc631-analysis/tests/tlcx31_file_repair_fixture/mod.rs"]
mod package;
use lc631_analysis::{
    build_dgcl_pipeline, capture_dgcl_cargo_snapshot_with_runner, declared_build_capture::*,
    BoundedToolRequest, DgclBackendSet, ResourceToolResult, ToolProcessRunner,
};
use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    AuthorityRevision,
};
use lc631_receipt_kernel::{ReceiptIssuer, ReceiptVerifier};
use lc631_runtime::{
    contextual_repair::{measurement::*, snapshot_gate::CodingBuildPort},
    goal_suite_measurement as g,
    resource::*,
};
use lc631_substrate::Digest32;
use std::{path::Path, time::Duration};

pub const HAND: [Option<u32>; 4] = [Some(0), Some(3), None, Some(4294967295)];
pub const BASE: [Option<u32>; 4] = [Some(0), Some(3), Some(0), Some(4294967295)];
pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn scope() -> ContextScope {
    ContextScope::declared(d(21), d(22), d(23), d(24))
}
pub fn pins() -> g::ArtifactPins {
    g::ArtifactPins {
        context: ContextRevision::original(SourceRevision::of(native::SOURCE)),
        scope: scope(),
        definition: d(0xc1),
        oracle: d(0xc2),
        hard: d(25),
        evidence: d(26),
    }
}
pub fn limits() -> g::Limits {
    g::Limits {
        max_queries: 4,
        max_pairs: 1,
        max_current_checks: 8,
        max_added_ram_bytes: 64 * 1024,
        max_summary_bytes: 64 * 1024,
    }
}
pub fn value(
    report: &lc631_analysis::creativity_measurement::ComponentReport,
) -> &lc631_analysis::creativity_measurement::RationalInterval {
    match &report.state {
        lc631_analysis::creativity_measurement::ComponentState::Measured(q) => q,
        state => panic!("expected finite artifact measurement, got {state:?}"),
    }
}
pub fn ratio(n: i64, d: i64) -> lc631_analysis::creativity_measurement::RationalInterval {
    lc631_analysis::creativity_measurement::RationalInterval::exact_ratio(n, d).unwrap()
}
pub fn invocation<'a>(
    old: &'a lc631_runtime::contextual_repair::execution::ProbeInvocation<'_>,
    authority: &'a lc631_core::ExecutionPermitContext<'a>,
) -> lc631_runtime::contextual_repair::execution::ProbeInvocation<'a> {
    use lc631_runtime::contextual_repair::execution::{ProbeInvocation, ProbeToolRegistration};
    ProbeInvocation {
        source: old.source,
        registration: ProbeToolRegistration {
            program: old.registration.program,
            executable_digest: old.registration.executable_digest,
            target_path: old.registration.target_path,
            target_ref: old.registration.target_ref,
            snapshot: old.registration.snapshot,
        },
        lease: old.lease,
        tool_receipt: old.tool_receipt,
        tool_verifier: old.tool_verifier,
        permit: old.permit,
        authority,
        clock: old.clock,
        timeout_ms: old.timeout_ms,
    }
}
pub struct Harness<'a, 'v> {
    pub pipeline: &'a lc631_analysis::DgclPipelineReport,
    pub plan: &'a CodingProbePlan,
    pub baseline: &'a OwnedCodingProbeObservation<'v>,
    pub candidate: &'a OwnedCodingProbeObservation<'v>,
    pub baseline_origin: &'a OwnedDeclaredBuildCapture<'v>,
    pub candidate_origin: &'a OwnedDeclaredBuildCapture<'v>,
    pub baseline_root: &'a Path,
    pub candidate_root: &'a Path,
    pub baseline_binary: &'a Path,
    pub candidate_binary: &'a Path,
    pub cancel: &'a Cancellation,
}
impl<'a, 'v> Harness<'a, 'v> {
    pub fn root_input(&self) -> g::ArtifactRootInput<'a, 'v> {
        g::ArtifactRootInput {
            plan: self.plan,
            pins: pins(),
            limits: limits(),
            baseline_before: Some(self.baseline_origin),
            candidate_expected: self.candidate_origin,
        }
    }
    pub fn pair(&self) -> g::CodingPair<'a, 'v> {
        g::CodingPair {
            baseline: self.baseline,
            candidate: self.candidate,
        }
    }
}

// An actual filesystem reader in the same accounting run. No tool report or
// inventory success is injected; an unexpected compiler call is an error.
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
    body: impl for<'a, 'v, 'lease, 'g, 'bi, 'br, 'ci, 'cr> FnOnce(
        Harness<'a, 'v>,
        &mut g::ArtifactCurrent<'g, 'bi, 'br, 'ci, 'cr>,
        &'v ResourceReservationView<'lease>,
    ),
) {
    fixture_with_output(native::AFTER, body)
}
pub fn fixture_with_output(
    candidate_source: &str,
    body: impl for<'a, 'v, 'lease, 'g, 'bi, 'br, 'ci, 'cr> FnOnce(
        Harness<'a, 'v>,
        &mut g::ArtifactCurrent<'g, 'bi, 'br, 'ci, 'cr>,
        &'v ResourceReservationView<'lease>,
    ),
) {
    fixture_with_deadline(candidate_source, Duration::from_secs(120), body)
}
pub fn fixture_with_deadline(
    candidate_source: &str,
    deadline: Duration,
    body: impl for<'a, 'v, 'lease, 'g, 'bi, 'br, 'ci, 'cr> FnOnce(
        Harness<'a, 'v>,
        &mut g::ArtifactCurrent<'g, 'bi, 'br, 'ci, 'cr>,
        &'v ResourceReservationView<'lease>,
    ),
) {
    let baseline_root = package::fixture_package();
    let candidate_root = package::fixture_package();
    assert_ne!(baseline_root, candidate_root);
    eprintln!(
        "retained owned artifact-pair roots: {} / {}",
        baseline_root.display(),
        candidate_root.display()
    );
    let wrapping = native::BEFORE.replace("{None}", "{Some(a.wrapping_add(b))}");
    assert_ne!(wrapping, native::BEFORE);
    let package_entrypoint = |text: &str| {
        text.replace("fn add(", "fn checked_add(")
            .replace("match add(", "match checked_add(")
    };
    std::fs::write(
        baseline_root.join("src/lib.rs"),
        package_entrypoint(&wrapping),
    )
    .unwrap();
    std::fs::write(
        candidate_root.join("src/lib.rs"),
        package_entrypoint(candidate_source),
    )
    .unwrap();
    let baseline_binary = baseline_root.join(format!("baseline{}", std::env::consts::EXE_SUFFIX));
    let candidate_binary =
        candidate_root.join(format!("candidate{}", std::env::consts::EXE_SUFFIX));
    // Only central's selected test run executes these compilations/native effects.
    native::compile_native(&baseline_root.join("src/lib.rs"), &baseline_binary);
    native::compile_native(&candidate_root.join("src/lib.rs"), &candidate_binary);
    let pipeline = build_dgcl_pipeline(
        native::SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let plan = CodingProbePlan::checked_add(scope().goal(), scope().semantics()).unwrap();
    let issuer = ReceiptIssuer::from_key_bytes("goal-artifact-fixture", &[121; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("goal-artifact-fixture", &[121; 32]).unwrap();
    let limits = ResourceLimits::uniform(256 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let cancel = Cancellation::default();
    let mut run = broker
        .begin_run(
            2211,
            SourceRevision::of(native::SOURCE).digest(),
            limits,
            Deadline::after(deadline).unwrap(),
            cancel.clone(),
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
        let mut baseline_reader = Reader(&run);
        let mut candidate_reader = Reader(&run);
        let mut ba = native::snapshot_authority(
            &baseline_root,
            &issuer,
            &verifier,
            100,
            AuthorityRevision(1),
        );
        let mut ca = native::snapshot_authority(
            &candidate_root,
            &issuer,
            &verifier,
            100,
            AuthorityRevision(1),
        );
        let bs = {
            let a = ba.current().unwrap();
            capture_dgcl_cargo_snapshot_with_runner(
                &baseline_root,
                native::SOURCE,
                &pipeline,
                &a.context,
                &mut baseline_reader,
            )
            .unwrap()
        };
        let cs = {
            let a = ca.current().unwrap();
            capture_dgcl_cargo_snapshot_with_runner(
                &candidate_root,
                native::SOURCE,
                &pipeline,
                &a.context,
                &mut candidate_reader,
            )
            .unwrap()
        };
        let parse = |s: &str| declared_capture_digest(s).unwrap();
        let program = parse(&pipeline.program_ir.program_digest);
        let completion = parse(&pipeline.completion_plan.plan_digest);
        let mut bp = CodingBuildPort::checked(
            &baseline_root,
            native::SOURCE,
            &pipeline,
            parse(&lc631_analysis::dgcl_repository_root_digest(&baseline_root).unwrap()),
            program,
            completion,
            &mut baseline_reader,
        )
        .unwrap();
        let mut cp = CodingBuildPort::checked(
            &candidate_root,
            native::SOURCE,
            &pipeline,
            parse(&lc631_analysis::dgcl_repository_root_digest(&candidate_root).unwrap()),
            program,
            completion,
            &mut candidate_reader,
        )
        .unwrap();
        // Freeze TWO actual origins before observations/comparison. They remain
        // Given conditional-artifact origins, never ExpectedCurrentBuild/verified31.
        let before = bp.capture_current(&mut ba, &view);
        let expected = cp.capture_current(&mut ca, &view);
        let bnf = native::NativeProbeFixture::registered(
            &plan,
            bs,
            baseline_root.join("src/lib.rs"),
            baseline_binary.clone(),
            &issuer,
            &verifier,
        );
        let cnf = native::NativeProbeFixture::registered(
            &plan,
            cs,
            candidate_root.join("src/lib.rs"),
            candidate_binary.clone(),
            &issuer,
            &verifier,
        );
        bnf.with_invocation(&verifier, |bi| {
            cnf.with_invocation(&verifier, |ci| {
                let baseline =
                    lc631_runtime::contextual_repair::execution::observe_current(&plan, bi, &view)
                        .unwrap();
                let candidate =
                    lc631_runtime::contextual_repair::execution::observe_current(&plan, ci, &view)
                        .unwrap();
                bnf.with_invocation(&verifier, |bi| {
                    cnf.with_invocation(&verifier, |ci| {
                        let mut current = g::ArtifactCurrent {
                            baseline_invocation: bi,
                            candidate_invocation: ci,
                            baseline_build: &mut bp,
                            candidate_build: &mut cp,
                            baseline_authority: &mut ba,
                            candidate_authority: &mut ca,
                        };
                        body(
                            Harness {
                                pipeline: &pipeline,
                                plan: &plan,
                                baseline: &baseline,
                                candidate: &candidate,
                                baseline_origin: before.current().unwrap(),
                                candidate_origin: expected.current().unwrap(),
                                baseline_root: &baseline_root,
                                candidate_root: &candidate_root,
                                baseline_binary: &baseline_binary,
                                candidate_binary: &candidate_binary,
                                cancel: &cancel,
                            },
                            &mut current,
                            &view,
                        );
                    })
                });
            })
        });
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}
