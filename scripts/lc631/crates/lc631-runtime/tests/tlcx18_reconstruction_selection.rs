//! Actual finite K6 producer -> owned current projection -> existing selector.
//! The common goal1 is a declared hypothesis, not observed NLP truth or proof.
use lc631_analysis::creativity_measurement::Rational;
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::facet_ledger::{ComparisonScope, Revision, ScopeId};
use lc631_runtime::joint_selection::{Disposition, RecommendationState};
use lc631_runtime::reconstruction_measurement::*;
use lc631_runtime::resource::*;
use std::time::Duration;

const SOURCE: &[u8] = &[4, 3, 1, b'f', 2, 3, 1, b'a', 3, 1, b'b'];
const DROP: &[u8] = &[4, 3, 1, b'f', 1, 3, 1, b'a'];
fn fixture(body: impl FnOnce(ContextRevision, &ResourceReservationView<'_>, &Cancellation)) {
    let source = SourceRevision::of("Given structural source interpretation");
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
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(48 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(24 * 1024 * 1024).unwrap(),
            ),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        body(ContextRevision::original(source), &view, &cancel);
    }
    reservation.release_confirmed().unwrap();
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
fn probes() -> [ProbeSpec; 2] {
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

#[test]
fn tlcx18_actual_reconstruction_changes_frontier_and_recommendation_without_forged_loss() {
    fixture(|context, view, _| {
        let plan = ProtectedFeatureSet::capture(SOURCE, &probes(), context, scope(), view).unwrap();
        let intact = produce(&plan, SOURCE, view).unwrap();
        let lost = produce(&plan, DROP, view).unwrap();
        let chosen = select_current(
            &plan,
            [&intact, &lost],
            [SOURCE, DROP],
            context,
            SelectionGoal::DeclaredHypothesisSatisfied,
            view,
        )
        .unwrap();
        let current = chosen.current(view).unwrap();
        assert_eq!(current.recommendation(), Some(0));
        assert_eq!(
            current.dispositions(),
            &[Disposition::Frontier, Disposition::Dominated { by: 0 }]
        );
        assert_eq!(current.frontier(), [true, false]);
        let changed = select_current(
            &plan,
            [&lost, &intact],
            [DROP, SOURCE],
            context,
            SelectionGoal::DeclaredHypothesisSatisfied,
            view,
        )
        .unwrap();
        assert_eq!(changed.current(view).unwrap().recommendation(), Some(1));
        assert_eq!(changed.current(view).unwrap().frontier(), [false, true]);
        assert_eq!(current.goal(), SelectionGoal::DeclaredHypothesisSatisfied);
        assert_eq!(intact.semantic_status(), SemanticStatus::Unverified);
        assert_eq!(lost.independence(), Independence::SharedDependency);
        assert!(select_current(
            &plan,
            [&intact, &lost],
            [DROP, DROP],
            context,
            SelectionGoal::DeclaredHypothesisSatisfied,
            view
        )
        .is_err());
        assert!(select_current(
            &plan,
            [&intact, &lost],
            [SOURCE, DROP],
            context.reconstructed([9; 32]),
            SelectionGoal::DeclaredHypothesisSatisfied,
            view
        )
        .is_err());
        let foreign_source = ContextRevision::original(SourceRevision::of("different source"));
        assert!(select_current(
            &plan,
            [&intact, &lost],
            [SOURCE, DROP],
            foreign_source,
            SelectionGoal::DeclaredHypothesisSatisfied,
            view
        )
        .is_err());
        fixture(|_, foreign_view, _| {
            assert!(select_current(
                &plan,
                [&intact, &lost],
                [SOURCE, DROP],
                context,
                SelectionGoal::DeclaredHypothesisSatisfied,
                foreign_view
            )
            .is_err());
            assert!(chosen.current(foreign_view).is_err());
        });
    });
}

#[test]
fn tlcx18_cut_and_cancel_cannot_reuse_a_prior_recommendation_or_unknown_zero() {
    fixture(|context, view, cancel| {
        let plan = ProtectedFeatureSet::capture(SOURCE, &probes(), context, scope(), view).unwrap();
        let intact = produce(&plan, SOURCE, view).unwrap();
        let lost = produce(&plan, DROP, view).unwrap();
        let before = select_current(
            &plan,
            [&intact, &lost],
            [SOURCE, DROP],
            context,
            SelectionGoal::DeclaredHypothesisSatisfied,
            view,
        )
        .unwrap();
        assert_eq!(before.current(view).unwrap().recommendation(), Some(0));
        let unknown_a = produce(&plan, &[255], view).unwrap();
        let unknown_b = produce(&plan, &[254], view).unwrap();
        assert!(unknown_a.historical_bounds().is_none());
        let cut = select_current(
            &plan,
            [&unknown_a, &unknown_b],
            [&[255], &[254]],
            context,
            SelectionGoal::DeclaredHypothesisSatisfied,
            view,
        )
        .unwrap();
        assert_eq!(cut.current(view).unwrap().recommendation(), None);
        assert!(matches!(
            cut.current(view).unwrap().recommendation_state(),
            RecommendationState::NoEligibleCandidate | RecommendationState::NeedsEvidence
        ));
        assert_eq!(
            cut.current(view).unwrap().dispositions(),
            &[Disposition::NeedsEvidence; 2]
        );
        cancel.cancel();
        assert!(before.current(view).is_err());
        assert!(cut.current(view).is_err());
        assert!(select_current(
            &plan,
            [&intact, &lost],
            [SOURCE, DROP],
            context,
            SelectionGoal::DeclaredHypothesisSatisfied,
            view
        )
        .is_err());
        assert_eq!(before.historical().recommendation(), Some(0)); // past, NOT current
    });
}
