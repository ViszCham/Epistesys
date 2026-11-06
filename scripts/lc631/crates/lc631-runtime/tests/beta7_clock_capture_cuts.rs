//! Exercise the public registered capture path with a signed Test grant and the
//! real declared-file inventory. The existing fixture directories are read-only.
mod tlcx31_registered_clock_support;

use lc631_analysis::{
    build_dgcl_pipeline,
    declared_build_capture::{CurrentSnapshotAuthority, SnapshotCaptureError, SourceReadBinding},
    BoundedToolRequest, DgclBackendSet, ResourceToolResult, ToolProcessRunner,
};
use lc631_core::{context::SourceRevision, AuthorityRevision, CallerOrigin, PermitValidation};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, SubjectRevision,
};
use lc631_runtime::{
    contextual_repair::{measurement::CodingProbePlan, snapshot_gate::CodingBuildPort},
    goal_clock_domain as clock,
    resource::{
        Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
        ResourceLimits, ResourceRequest, ResourceReservationView, RunResources,
    },
};
use std::{
    cell::Cell,
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tlcx31_registered_clock_support::{bind, d, profile, providers::native, Clock, Registry};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Delivery {
    WrongScope,
    StandaloneOrigin,
    Twice,
    Once,
}

struct GrantActor {
    material: Mutex<native::FixtureSnapshotAuthority>,
    mode: Delivery,
    calls: AtomicUsize,
    delivered: AtomicUsize,
    refused: AtomicUsize,
}

impl clock::SnapshotGrantSource for GrantActor {
    fn with_current(
        &self,
        consume: &mut dyn FnMut(clock::SnapshotGrantView<'_>) -> Result<(), clock::Error>,
    ) -> Result<(), clock::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut material = self.material.lock().unwrap();
        let count = if self.mode == Delivery::Twice { 2 } else { 1 };
        for _ in 0..count {
            let mut grant = material.current().map_err(clock::Error::Snapshot)?;
            // This is a genuine permit, valid in its own originally issued context.
            assert_eq!(
                grant.permit.validate(&grant.context),
                PermitValidation::ValidGrant
            );
            if self.mode == Delivery::StandaloneOrigin {
                grant.context.caller_origin = CallerOrigin::StandaloneCli;
            }
            self.delivered.fetch_add(1, Ordering::SeqCst);
            let result = consume(clock::SnapshotGrantView {
                permit: grant.permit,
                expected_principal: grant.expected_principal,
                context: grant.context,
            });
            if result.is_err() {
                self.refused.fetch_add(1, Ordering::SeqCst);
            }
            if self.mode == Delivery::Once {
                result?;
            }
        }
        // Deliberately suppress a callback rejection in the three hostile cases.
        Ok(())
    }
}

struct InputReader<'a> {
    run: &'a RunResources,
    reads: &'a Cell<usize>,
}

impl ToolProcessRunner for InputReader<'_> {
    fn run(
        &mut self,
        _: &str,
        _: BoundedToolRequest<'_>,
        _: &mut dyn FnMut(&ResourceToolResult<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        panic!("declared source capture must not start a tool process")
    }

    fn with_input(
        &mut self,
        source: &str,
        bytes: usize,
        read: &mut dyn FnMut(&ResourceReservationView<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        self.reads.set(self.reads.get() + 1);
        let bytes = u64::try_from(bytes).map_err(|_| ResourceError::ArithmeticOverflow)?;
        let mut reservation = self.run.reserve_for_source(
            SourceRevision::of(source).digest(),
            &[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(bytes)?,
            )],
        )?;
        let result = read(&reservation.lease());
        reservation.release_confirmed()?;
        result
    }
}

fn signed_registration(
    candidate: &clock::EndpointCandidate<'_, '_>,
) -> lc631_receipt_kernel::UntrustedReceipt {
    // The clock registration remains valid past the independent Test permit's
    // expiry at 400. The expiry test isolates that permit, not registry expiry.
    Registry::issuer()
        .issue(
            ReceiptClass::ToolExecution,
            SubjectRevision::checked(candidate.registration_subject()).unwrap(),
            ReceiptScope::checked(candidate.registration_scope()).unwrap(),
            candidate.registration_payload(),
            100,
            Some(800),
            None,
        )
        .unwrap()
}

fn exercise(mode: Delivery) {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let root = fixture_dir.join("fixtures");
    let grant_root = if mode == Delivery::WrongScope {
        fixture_dir.join("tlcx31_coding_support")
    } else {
        root.clone()
    };
    assert!(root.join("tlcx10_solver_child.rs").is_file());
    let pipeline = build_dgcl_pipeline(
        native::SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let plan = CodingProbePlan::checked_add(d(22), d(24)).unwrap();
    let registration = bind(
        &root,
        &pipeline,
        &plan,
        clock::Role::Baseline,
        clock::Purpose::Capture,
    );
    let issuer = ReceiptIssuer::from_key_bytes("capture-cut-grant", &[137; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("capture-cut-grant", &[137; 32]).unwrap();
    let grant =
        native::snapshot_authority(&grant_root, &issuer, &verifier, 100, AuthorityRevision(1));
    let source = SourceReadBinding::checked(
        &root,
        native::SOURCE,
        &pipeline,
        registration.root,
        registration.program,
        registration.completion_plan,
    )
    .unwrap();
    if mode == Delivery::WrongScope {
        assert_ne!(grant.scope, source.execution_scope());
    } else {
        assert_eq!(grant.scope, source.execution_scope());
    }
    let actor = Arc::new(GrantActor {
        material: Mutex::new(grant),
        mode,
        calls: AtomicUsize::new(0),
        delivered: AtomicUsize::new(0),
        refused: AtomicUsize::new(0),
    });
    let mut caps = [0; lc631_substrate::resource::RESOURCE_KINDS];
    caps[ResourceKind::Ram.index()] = 128 * 1024 * 1024;
    let limits = ResourceLimits::checked(caps).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            731,
            source.source(),
            limits,
            Deadline::after(Duration::from_secs(120)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(96 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    {
        let view = reservation.lease();
        let registry = Registry::new();
        let profile = profile(&registry, &view);
        let exec_clock = Clock::new();
        let capture_clock = Clock::new();
        let execution = clock::EndpointCandidate::execution(
            &profile,
            clock::EndpointBinding {
                purpose: clock::Purpose::Execution,
                ..registration
            },
            exec_clock.clone(),
            &view,
        )
        .unwrap();
        let capture = clock::EndpointCandidate::capture(
            &profile,
            registration,
            capture_clock.clone(),
            actor.clone(),
            &view,
        )
        .unwrap();
        let er = signed_registration(&execution);
        let cr = signed_registration(&capture);
        let execution = clock::register(&profile, execution, er, registry.clone(), &view).unwrap();
        let capture = clock::register(&profile, capture, cr, registry, &view).unwrap();
        let session =
            clock::RoleSession::bind(&profile, &execution, &capture, &source, &plan, &view)
                .unwrap();
        let reads = Cell::new(0);
        let mut reader = InputReader {
            run: &run,
            reads: &reads,
        };
        let mut port = CodingBuildPort::checked(
            &root,
            native::SOURCE,
            &pipeline,
            registration.root,
            registration.program,
            registration.completion_plan,
            &mut reader,
        )
        .unwrap();
        let before = session.capture_before(&mut port, &view);
        if mode == Delivery::Once {
            before.current().unwrap();
            assert!(
                reads.get() > 0,
                "the origin must own an actual file inventory"
            );
            assert_eq!(capture_clock.calls(), 2);
            assert_expiry(&session, &mut port, &capture_clock, &reads, &view);
            assert_eq!(actor.calls.load(Ordering::SeqCst), 5);
            assert_eq!(profile.work_current(&view).unwrap().endpoint_reads, 5);
        } else {
            let wrong = clock::Error::Snapshot(SnapshotCaptureError::Unauthorized);
            let duplicate = clock::Error::Clock(clock::ClockFault::BindingMismatch);
            let expected = if mode == Delivery::Twice {
                duplicate
            } else {
                wrong
            };
            assert_eq!(before.current().err(), Some(expected));
            assert_eq!(actor.calls.load(Ordering::SeqCst), 1);
            assert_eq!(
                actor.delivered.load(Ordering::SeqCst),
                if mode == Delivery::Twice { 2 } else { 1 }
            );
            assert_eq!(actor.refused.load(Ordering::SeqCst), 1);
            assert!(before.actual_capture().is_none());
            assert_eq!(reads.get(), 0, "invalid grant must precede inventory");
            assert_eq!(
                capture_clock.calls(),
                0,
                "invalid delivery must precede sampling"
            );
            assert_eq!(profile.work_current(&view).unwrap().endpoint_reads, 0);
        }
        assert_eq!(exec_clock.calls(), 0);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}

fn assert_expiry<'v>(
    session: &clock::RoleSession<'_, '_, 'v>,
    port: &mut CodingBuildPort<'_, '_>,
    capture_clock: &Clock,
    reads: &Cell<usize>,
    view: &'v ResourceReservationView<'_>,
) {
    let origin_reads = reads.get();
    capture_clock.sequence(&[200, 400], 150);
    let failed = session.capture_current(port, view);
    assert!(matches!(
        failed.current(),
        Err(clock::Error::Snapshot(SnapshotCaptureError::Unauthorized))
    ));
    assert_eq!(
        failed.clock_fault(),
        Some(&clock::ClockFault::ObservedExpiry)
    );
    let inventory = failed
        .actual_capture()
        .expect("post-inventory expiry retains the actual owner");
    assert_eq!(inventory.authority_before().checked_epoch, 200);
    assert_eq!(inventory.authority_after(), None);
    assert_eq!(
        inventory.check_current(view),
        Err(SnapshotCaptureError::Unauthorized)
    );
    assert!(reads.get() > origin_reads);
    let partial_reads = reads.get();
    let rewound = session.capture_current(port, view);
    assert!(matches!(
        rewound.current(),
        Err(clock::Error::Clock(clock::ClockFault::ClockRollback))
    ));
    assert_eq!(
        rewound.clock_fault(),
        Some(&clock::ClockFault::ClockRollback)
    );
    assert!(rewound.actual_capture().is_none());
    assert_eq!(
        reads.get(),
        partial_reads,
        "rewind must reject before another inventory"
    );
    assert_eq!(capture_clock.calls(), 5);
}

#[test]
fn beta7_capture_rejects_signed_foreign_scope_even_if_provider_suppresses_error() {
    exercise(Delivery::WrongScope);
}

#[test]
fn beta7_capture_rejects_standalone_origin_even_if_provider_suppresses_error() {
    exercise(Delivery::StandaloneOrigin);
}

#[test]
fn beta7_capture_duplicate_grant_delivery_fails_before_any_clock_read() {
    exercise(Delivery::Twice);
}

#[test]
fn beta7_capture_expiry_retains_partial_inventory_and_rejects_later_rewind() {
    exercise(Delivery::Once);
}
