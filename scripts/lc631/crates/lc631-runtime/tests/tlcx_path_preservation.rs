//! Literal K6/hand fractions, never production-generated expected answers.
use lc631_analysis::creativity_measurement::{Rational, RationalInterval};
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::facet_ledger::*;
use lc631_runtime::path_preservation::*;
use lc631_runtime::reconstruction_measurement::{self as recon, FeatureKind, ProbeSpec};
use lc631_runtime::resource::*;
use lc631_substrate::Digest32;
use std::time::Duration;

const ROOT: &[u8] = &[4, 3, 1, b'f', 2, 3, 1, b'a', 3, 1, b'b'];
const DROP: &[u8] = &[4, 3, 1, b'f', 1, 3, 1, b'a'];
fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn scope() -> ComparisonScope {
    ComparisonScope {
        id: ScopeId([1; 32]),
        baseline: Revision([2; 32]),
        root_goal: Revision([3; 32]),
        probes: Revision([4; 32]),
        policy: Revision([5; 32]),
    }
}
fn mapping() -> RegisteredMap {
    RegisteredMap::identity_namespace(d(6), d(7)).unwrap()
}
fn probes() -> Vec<RootProbeSpec> {
    [1, 3]
        .into_iter()
        .enumerate()
        .map(|(i, w)| RootProbeSpec {
            obligation: ObligationId([10; 32]),
            property: PropertyId([i as u8; 32]),
            hard: true,
            feature: ProbeSpec {
                path: vec![i],
                kind: FeatureKind::Symbol,
                weight: Rational::from_integer(w.into()),
            },
        })
        .collect()
}
fn fixture(body: impl FnOnce(ContextRevision, &ResourceReservationView<'_>, &Cancellation)) {
    let source = SourceRevision::of("Given K6 interpretation; UTF8 correspondence unverified");
    let limits = ResourceLimits::uniform(128 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let cancel = Cancellation::default();
    let run = broker
        .begin_run(
            105,
            source.digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(96 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(48 * 1024 * 1024).unwrap(),
            ),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        body(ContextRevision::original(source), &view, &cancel);
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tl_path_drop_restore_remeasures_final_and_keeps_historical_defect() {
    fixture(|context, resources, _| {
        let plan =
            RootPathPlan::capture(ROOT, &probes(), context, scope(), mapping(), resources).unwrap();
        let edges = [
            PathEdge::new(ROOT, DROP, mapping()),
            PathEdge::new(DROP, ROOT, mapping()),
        ];
        let result = evaluate_path(&plan, &edges, ROOT, resources).unwrap();
        assert_eq!(
            result.history()[0].loss(),
            Some(&RationalInterval::exact_ratio(3, 4).unwrap())
        );
        assert_eq!(
            result.history()[1].loss(),
            Some(&RationalInterval::exact_ratio(0, 1).unwrap())
        );
        assert_eq!(
            result.final_observation().loss(),
            Some(&RationalInterval::exact_ratio(0, 1).unwrap())
        );
        assert_eq!(result.actual_evaluation_count(), 6); //2 probes*(2 history+fresh final)
        assert_eq!(
            consume_root_gate(Some(&result), &plan, ROOT, resources).unwrap(),
            RootGate::PassInGivenFiniteProfile
        );
        assert_eq!(result.ledger().unique_defects(scope()).count(), 1);
        assert!(result
            .ledger()
            .entries()
            .iter()
            .any(|e| matches!(e.facet, Facet::Uncertainty(_))));
        assert_eq!(
            consume_root_gate(None, &plan, ROOT, resources).unwrap(),
            RootGate::NeedsEvidence
        );
    });
}

#[test]
fn tl_path_local_zero_and_duplicate_edges_cannot_free_root_final_obligation() {
    fixture(|context, resources, _| {
        let plan =
            RootPathPlan::capture(ROOT, &probes(), context, scope(), mapping(), resources).unwrap();
        let local_spec = [ProbeSpec {
            path: vec![0],
            kind: FeatureKind::Symbol,
            weight: Rational::from_integer(1.into()),
        }];
        let local =
            recon::ProtectedFeatureSet::capture(DROP, &local_spec, context, scope(), resources)
                .unwrap();
        let local_pass = recon::produce(&local, DROP, resources).unwrap();
        assert_eq!(
            local_pass.historical_bounds(),
            Some(&RationalInterval::exact_ratio(0, 1).unwrap())
        );
        let plain = [PathEdge::new(ROOT, DROP, mapping())];
        let duplicate = [
            PathEdge::new(ROOT, DROP, mapping()),
            PathEdge::new(DROP, DROP, mapping()),
        ];
        let a = evaluate_path(&plan, &plain, DROP, resources).unwrap();
        let b = evaluate_path(&plan, &duplicate, DROP, resources).unwrap();
        assert_eq!(
            a.final_observation().loss(),
            Some(&RationalInterval::exact_ratio(3, 4).unwrap())
        );
        assert_eq!(a.final_observation().loss(), b.final_observation().loss());
        assert_eq!(a.ledger().unique_defects(scope()).count(), 1);
        assert_eq!(b.ledger().unique_defects(scope()).count(), 1);
        assert_eq!(
            consume_root_gate(Some(&b), &plan, DROP, resources).unwrap(),
            RootGate::FailInGivenFiniteProfile
        );
    });
}

#[test]
fn tl_path_unsupported_final_and_broken_namespace_chain_are_not_zero() {
    fixture(|context, resources, _| {
        let plan =
            RootPathPlan::capture(ROOT, &probes(), context, scope(), mapping(), resources).unwrap();
        let unknown = [255];
        let edge = [PathEdge::new(ROOT, &unknown, mapping())];
        let result = evaluate_path(&plan, &edge, &unknown, resources).unwrap();
        assert!(result.final_observation().loss().is_none());
        assert_eq!(
            consume_root_gate(Some(&result), &plan, &unknown, resources).unwrap(),
            RootGate::NeedsEvidence
        );
        let wrong = [PathEdge::new(DROP, ROOT, mapping())];
        assert!(matches!(
            evaluate_path(&plan, &wrong, ROOT, resources),
            Err(Error::BrokenPath)
        ));
        let other = RegisteredMap::identity_namespace(d(99), d(7)).unwrap();
        let wrong_map = [PathEdge::new(ROOT, ROOT, other)];
        assert!(matches!(
            evaluate_path(&plan, &wrong_map, ROOT, resources),
            Err(Error::UnsupportedMap)
        ));
        let unsupported = [PathEdge::new(ROOT, ROOT, RegisteredMap::unsupported(d(9)))];
        assert!(matches!(
            evaluate_path(&plan, &unsupported, ROOT, resources),
            Err(Error::UnsupportedMap)
        ));
        assert!(matches!(
            evaluate_path(&plan, &[], DROP, resources),
            Err(Error::BrokenPath)
        ));
    });
}

#[test]
fn tl_path_current_gate_rejects_substitution_and_cancelled_historical_pass() {
    fixture(|context, resources, cancel| {
        let plan =
            RootPathPlan::capture(ROOT, &probes(), context, scope(), mapping(), resources).unwrap();
        let result = evaluate_path(&plan, &[], ROOT, resources).unwrap();
        let foreign =
            RootPathPlan::capture(ROOT, &probes(), context, scope(), mapping(), resources).unwrap();
        assert!(consume_root_gate(Some(&result), &foreign, ROOT, resources).is_err());
        assert!(consume_root_gate(Some(&result), &plan, DROP, resources).is_err());
        cancel.cancel();
        assert!(result.final_observation().loss().is_some());
        assert!(consume_root_gate(Some(&result), &plan, ROOT, resources).is_err());
        assert!(consume_root_gate(None, &plan, ROOT, resources).is_err());
        assert!(evaluate_path(&plan, &[], ROOT, resources).is_err());
    });
}

#[test]
fn tl_path_soft_defect_does_not_hide_hard_defect_or_change_denominator() {
    fixture(|context, resources, _| {
        let mut fixed = probes();
        fixed[1].hard = false;
        let plan =
            RootPathPlan::capture(ROOT, &fixed, context, scope(), mapping(), resources).unwrap();
        let edge = [PathEdge::new(ROOT, DROP, mapping())];
        let result = evaluate_path(&plan, &edge, DROP, resources).unwrap();
        assert_eq!(
            result.final_observation().loss(),
            Some(&RationalInterval::exact_ratio(3, 4).unwrap())
        );
        assert_eq!(
            consume_root_gate(Some(&result), &plan, DROP, resources).unwrap(),
            RootGate::PassInGivenFiniteProfile
        );
        assert_eq!(result.edges().len(), 1);
        assert_ne!(result.edges()[0].from, result.edges()[0].to);
        let mut duplicate = fixed.clone();
        duplicate[1].property = duplicate[0].property;
        assert!(matches!(
            RootPathPlan::capture(ROOT, &duplicate, context, scope(), mapping(), resources),
            Err(Error::InvalidPlan)
        ));
    });
}

#[test]
fn tl_path_preclaims_reject_insufficient_output_before_evaluation() {
    let source = SourceRevision::of("Given K6 interpretation; UTF8 correspondence unverified");
    let limits = ResourceLimits::uniform(128 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            106,
            source.digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(96 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(7 * 1024 * 1024).unwrap(),
            ),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        let plan = RootPathPlan::capture(
            ROOT,
            &probes(),
            ContextRevision::original(source),
            scope(),
            mapping(),
            &view,
        )
        .unwrap();
        assert!(matches!(
            evaluate_path(&plan, &[], ROOT, &view),
            Err(Error::Resource(_))
        ));
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tl_path_foreign_lease_with_same_numeric_run_id_is_rejected() {
    let source = SourceRevision::of("Given K6 interpretation; UTF8 correspondence unverified");
    let limits = ResourceLimits::uniform(128 * 1024 * 1024).unwrap();
    let broker_a = ResourceBroker::new(limits).unwrap();
    let broker_b = ResourceBroker::new(limits).unwrap();
    let deadline = || Deadline::after(Duration::from_secs(30)).unwrap();
    let run_a = broker_a
        .begin_run(
            107,
            source.digest(),
            limits,
            deadline(),
            Cancellation::default(),
        )
        .unwrap();
    let run_b = broker_b
        .begin_run(
            107,
            source.digest(),
            limits,
            deadline(),
            Cancellation::default(),
        )
        .unwrap();
    let request = [
        ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(96 * 1024 * 1024).unwrap(),
        ),
        ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(48 * 1024 * 1024).unwrap(),
        ),
    ];
    let mut reservation_a = run_a.reserve(&request).unwrap();
    let mut reservation_b = run_b.reserve(&request).unwrap();
    {
        let view_a = reservation_a.lease();
        let view_b = reservation_b.lease();
        assert_eq!(view_a.run_id(), view_b.run_id());
        let plan = RootPathPlan::capture(
            ROOT,
            &probes(),
            ContextRevision::original(source),
            scope(),
            mapping(),
            &view_a,
        )
        .unwrap();
        let result = evaluate_path(&plan, &[], ROOT, &view_a).unwrap();
        assert!(consume_root_gate(Some(&result), &plan, ROOT, &view_b).is_err());
        assert!(consume_root_gate(None, &plan, ROOT, &view_b).is_err());
        assert!(evaluate_path(&plan, &[], ROOT, &view_b).is_err());
    }
    reservation_a.release_confirmed().unwrap();
    reservation_b.release_confirmed().unwrap();
}

#[test]
fn tl_path_direct_enum_cannot_bypass_namespace_pin_validation() {
    fixture(|context, resources, _| {
        let bad = RegisteredMap::IdentityNamespace {
            namespace: d(0),
            revision: d(7),
        };
        assert!(matches!(
            RootPathPlan::capture(ROOT, &probes(), context, scope(), bad, resources),
            Err(Error::UnsupportedMap)
        ));
    });
}
