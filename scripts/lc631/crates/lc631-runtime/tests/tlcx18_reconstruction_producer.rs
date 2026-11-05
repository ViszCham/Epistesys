//! Hand feature/weight oracle: [a,b], weights [1,3], drop b => 3/4.
use lc631_analysis::creativity_measurement::{Rational, RationalInterval};
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::facet_ledger::*;
use lc631_runtime::joint_selection::AxisValue;
use lc631_runtime::reconstruction_measurement::*;
use lc631_runtime::resource::*;
use std::time::Duration;

// Independent literal wire vectors, not golds obtained from production encode.
const SOURCE: &[u8] = &[4, 3, 1, b'f', 2, 3, 1, b'a', 3, 1, b'b'];
const DROPPED: &[u8] = &[4, 3, 1, b'f', 1, 3, 1, b'a'];
fn scope() -> ComparisonScope {
    ComparisonScope {
        id: ScopeId([1; 32]),
        baseline: Revision([2; 32]),
        root_goal: Revision([3; 32]),
        probes: Revision([4; 32]),
        policy: Revision([5; 32]),
    }
}
fn specs() -> [ProbeSpec; 2] {
    [
        ProbeSpec {
            path: vec![0],
            kind: FeatureKind::Symbol,
            weight: Rational::from_integer(1.into()),
        },
        ProbeSpec {
            path: vec![1],
            kind: FeatureKind::Symbol,
            weight: Rational::from_integer(3.into()),
        },
    ]
}
fn fixture(
    ram: u64,
    body: impl FnOnce(ContextRevision, &ResourceReservationView<'_>, &Cancellation),
) {
    let source = SourceRevision::of("given finite structural interpretation");
    let limits = ResourceLimits::uniform(64 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let cancel = Cancellation::default();
    let run = broker
        .begin_run(
            18,
            source.digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(ram).unwrap()),
            ResourceRequest::new(
                ResourceKind::Output,
                // Source-cut case intentionally holds two plans and three
                // reports simultaneously: 5 * 2MiB, without early payload drop.
                ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
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
fn actual_target_roundtrip_drop_and_owned_axis_and_ledger() {
    fixture(32 * 1024 * 1024, |context, resources, _| {
        let plan =
            ProtectedFeatureSet::capture(SOURCE, &specs(), context, scope(), resources).unwrap();
        let intact = produce(&plan, SOURCE, resources).unwrap();
        let drop = produce(&plan, DROPPED, resources).unwrap();
        assert_eq!(
            intact.historical_bounds(),
            Some(&RationalInterval::exact_ratio(0, 1).unwrap())
        );
        assert_eq!(
            drop.historical_bounds(),
            Some(&RationalInterval::exact_ratio(3, 4).unwrap())
        );
        assert_eq!(plan.feature_count(), 2); // Candidate cannot drop/reweight it.
        assert!(drop.bundle_bits().unwrap() > (DROPPED.len() * 8) as u64);
        assert_eq!(drop.input_basis(), InputBasis::Given);
        assert_eq!(drop.semantic_status(), SemanticStatus::Unverified);
        assert_eq!(drop.independence(), Independence::SharedDependency);
        assert!(matches!(
            drop.axis_current(&plan, DROPPED, context, resources)
                .unwrap(),
            AxisValue::Exact(_)
        ));
        let claim = resources
            .claim_resources(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(16_384).unwrap(),
            )])
            .unwrap();
        let mut ledger = FacetLedger::new(1, 3).unwrap();
        let id = TransformId([18; 32]);
        ledger.admit_transform(drop.binding(id)).unwrap();
        drop.retain_current(&plan, DROPPED, context, resources, &mut ledger, id, None)
            .unwrap();
        drop.retain_current(&plan, DROPPED, context, resources, &mut ledger, id, Some(1))
            .unwrap();
        assert!(drop
            .retain_current(&plan, DROPPED, context, resources, &mut ledger, id, Some(0))
            .is_err());
        assert!(matches!(ledger.entries()[0].facet, Facet::Uncertainty(_)));
        assert!(matches!(
            ledger.entries()[1].facet,
            Facet::Defect(Defect {
                axis: LossAxis::Reconstruction,
                ..
            })
        ));
        assert!(!ledger
            .entries()
            .iter()
            .any(|e| matches!(e.facet, Facet::Gain(_))));
        std::mem::drop(ledger);
        std::mem::drop(claim);
    });
}
#[test]
fn source_and_sidecar_cuts_cannot_restore_missing_target_information() {
    fixture(48 * 1024 * 1024, |context, resources, _| {
        let plan =
            ProtectedFeatureSet::capture(SOURCE, &specs(), context, scope(), resources).unwrap();
        let other_source = &[4, 3, 1, b'f', 2, 3, 1, b'a', 3, 1, b'c'];
        let other =
            ProtectedFeatureSet::capture(other_source, &specs(), context, scope(), resources)
                .unwrap();
        let a = produce(&plan, SOURCE, resources).unwrap();
        let b = produce(&other, SOURCE, resources).unwrap();
        assert_eq!(
            b.historical_bounds(),
            Some(&RationalInterval::exact_ratio(3, 4).unwrap())
        );
        assert_ne!(plan.source_term_digest(), other.source_term_digest());
        assert!(a.axis_current(&other, SOURCE, context, resources).is_err());
        assert!(a.axis_current(&plan, DROPPED, context, resources).is_err());
        assert!(a
            .axis_current(&plan, SOURCE, context.reconstructed([99; 32]), resources)
            .is_err());
        // A plausible source reference is only a symbol, never a sidecar lookup.
        let sidecar = &[3, 6, b's', b'o', b'u', b'r', b'c', b'e'];
        let ref_only = produce(&plan, sidecar, resources).unwrap();
        assert_eq!(
            ref_only.historical_bounds(),
            Some(&RationalInterval::exact_ratio(1, 1).unwrap())
        );
    });
}
#[test]
fn unsupported_invalid_and_cancelled_are_not_zero_or_current() {
    fixture(32 * 1024 * 1024, |context, resources, cancel| {
        let plan =
            ProtectedFeatureSet::capture(SOURCE, &specs(), context, scope(), resources).unwrap();
        let unknown = produce(&plan, &[255], resources).unwrap();
        assert_eq!(
            unknown.historical_gap(),
            Some(lc631_runtime::reconstruction_measurement::Gap::Unsupported)
        );
        assert!(unknown.historical_bounds().is_none());
        assert!(unknown.bundle_bits().is_none());
        assert!(matches!(
            unknown
                .axis_current(&plan, &[255], context, resources)
                .unwrap(),
            AxisValue::Unavailable(_)
        ));
        let bad = produce(&plan, &[1, 2], resources).unwrap();
        assert_eq!(
            bad.historical_gap(),
            Some(lc631_runtime::reconstruction_measurement::Gap::InvalidTarget)
        );
        let good = produce(&plan, SOURCE, resources).unwrap();
        cancel.cancel();
        assert!(good
            .axis_current(&plan, SOURCE, context, resources)
            .is_err());
        assert!(good.historical_bounds().is_some());
        assert!(produce(&plan, SOURCE, resources).is_err());
    });
}
#[test]
fn fixed_plan_rejects_empty_duplicate_zero_weight_and_underfunded_capture() {
    fixture(32 * 1024 * 1024, |context, resources, _| {
        assert!(ProtectedFeatureSet::capture(SOURCE, &[], context, scope(), resources).is_err());
        let s = specs();
        assert!(ProtectedFeatureSet::capture(
            SOURCE,
            &[s[0].clone(), s[0].clone()],
            context,
            scope(),
            resources
        )
        .is_err());
        let mut zero = specs();
        zero[0].weight = Rational::from_integer(0.into());
        assert!(ProtectedFeatureSet::capture(SOURCE, &zero, context, scope(), resources).is_err());
    });
    fixture(1024, |context, resources, _| {
        assert!(
            ProtectedFeatureSet::capture(SOURCE, &specs(), context, scope(), resources).is_err()
        );
    });
}
