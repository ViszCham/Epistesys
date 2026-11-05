//! Hand gold and negative cuts for the fixed Given tuple reduction, not eligibility.
mod tlcx19_checked_tl_support;
use lc631_runtime::checked_tl_reduction::Error as ReductionError;
use lc631_runtime::{checked_tl_reduction::*, spatiotemporal_selection::*};
use lc631_solvers::temporal::{EvaluationBudget, Truth};
use tlcx19_checked_tl_support::*;

#[test]
fn tlcx19_checked_tl_actual_tuple_handgold_and_bounded_unknown() {
    for unknown in [false, true] {
        let q = query();
        let base = trace(Truth::True);
        let changed = trace(if unknown {
            Truth::Unknown
        } else {
            Truth::False
        });
        let a = temporal(&q, &base);
        let b = temporal(&q, &changed);
        let sa = spatial(false);
        let sb = spatial(true);
        let tp = time_policy(a, a);
        let sp = space_policy(sa, sa, false);
        let cancel = Cancellation::default();
        let (_broker, _run, reservation) = run(cancel.clone());
        let view = reservation.lease();
        let root = RootPlan::capture(a, sa, &tp, &sp, &view).unwrap();
        let candidate = TupleCandidate::capture(&root, b, sb).unwrap();
        let actual_tp = time_policy(a, b);
        let actual_sp = space_policy(sa, sb, false);
        let t = TemporalOwner::produce(&actual_tp, a, b, &view, &view, EvaluationBudget::default())
            .unwrap();
        let s = SpatialOwner::produce(&actual_sp, sa, sb, &view, &view).unwrap();
        let reduction = reduce_current(
            &root,
            &candidate,
            Owners {
                temporal: Some(&t),
                spatial: Some(&s),
            },
            &view,
        )
        .unwrap();
        // Temporal truth loss1 or [0,1], spatial left relation lost1:
        // (1*T+3*S)/4 =1 or[3/4,1]. No production routine creates this gold.
        let expected = if unknown {
            bounds(3, 4, 1, 1)
        } else {
            bounds(1, 1, 1, 1)
        };
        assert_eq!(
            reduction.current(&root, &candidate, &view).unwrap().total(),
            Some(&expected)
        );
        assert_eq!(
            reduction
                .current(&root, &candidate, &view)
                .unwrap()
                .hard_gates(),
            [
                if unknown {
                    Gate::NeedsEvidence
                } else {
                    Gate::Fail
                },
                Gate::Fail
            ]
        );
        cancel.cancel();
        assert!(reduction.current(&root, &candidate, &view).is_err());
        assert_eq!(reduction.historical().total(), Some(&expected));
    }
}

#[test]
fn tlcx19_checked_tl_missing_unsupported_and_candidate_swap_are_not_zero() {
    let q = query();
    let tr = trace(Truth::True);
    let a = temporal(&q, &tr);
    let sa = spatial(false);
    let tp = time_policy(a, a);
    let sp = space_policy(sa, sa, false);
    let (_b, _r, res) = run(Cancellation::default());
    let view = res.lease();
    let root = RootPlan::capture(a, sa, &tp, &sp, &view).unwrap();
    let c = TupleCandidate::capture(&root, a, sa).unwrap();
    let other = TupleCandidate::capture(&root, a, sa).unwrap();
    let t = TemporalOwner::produce(&tp, a, a, &view, &view, EvaluationBudget::default()).unwrap();
    let missing = reduce_current(
        &root,
        &c,
        Owners {
            temporal: Some(&t),
            spatial: None,
        },
        &view,
    )
    .unwrap();
    assert!(missing.current(&root, &c, &view).unwrap().total().is_none());
    assert_eq!(missing.historical().gaps()[1], Some(NoScalar::MissingOwner));
    assert!(matches!(
        missing.current(&root, &other, &view),
        Err(ReductionError::BindingMismatch)
    ));
    let unsupported = space_policy(sa, sa, true);
    let unsupported_root = RootPlan::capture(a, sa, &tp, &unsupported, &view).unwrap();
    let uc = TupleCandidate::capture(&unsupported_root, a, sa).unwrap();
    let s = SpatialOwner::produce(&unsupported, sa, sa, &view, &view).unwrap();
    let u = reduce_current(
        &unsupported_root,
        &uc,
        Owners {
            temporal: Some(&t),
            spatial: Some(&s),
        },
        &view,
    )
    .unwrap();
    assert!(u.historical().total().is_none());
    assert_eq!(
        u.historical().gaps()[1],
        Some(NoScalar::Unavailable(
            lc631_runtime::joint_selection::MissingAxis::Unsupported
        ))
    );
    assert!(matches!(
        u.current(&root, &uc, &view),
        Err(ReductionError::BindingMismatch)
    ));
}

#[test]
fn tlcx19_checked_tl_target_source_policy_and_foreign_lease_cuts() {
    let q = query();
    let tr = trace(Truth::True);
    let bad = trace(Truth::False);
    let a = temporal(&q, &tr);
    let b = temporal(&q, &bad);
    let sa = spatial(false);
    let sb = spatial(true);
    let tp = time_policy(a, a);
    let sp = space_policy(sa, sa, false);
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let root = RootPlan::capture(a, sa, &tp, &sp, &view).unwrap();
    let c = TupleCandidate::capture(&root, b, sb).unwrap();
    let wrong_t =
        TemporalOwner::produce(&tp, a, a, &view, &view, EvaluationBudget::default()).unwrap();
    let asp = space_policy(sa, sb, false);
    let s = SpatialOwner::produce(&asp, sa, sb, &view, &view).unwrap();
    assert!(matches!(
        reduce_current(
            &root,
            &c,
            Owners {
                temporal: Some(&wrong_t),
                spatial: Some(&s)
            },
            &view
        ),
        Err(ReductionError::BindingMismatch)
    ));
    let atp = time_policy(a, b);
    let t = TemporalOwner::produce(&atp, a, b, &view, &view, EvaluationBudget::default()).unwrap();
    let reduction = reduce_current(
        &root,
        &c,
        Owners {
            temporal: Some(&t),
            spatial: Some(&s),
        },
        &view,
    )
    .unwrap();
    let (_br2, _run2, res2) = run(Cancellation::default());
    let foreign = res2.lease();
    assert!(reduction.current(&root, &c, &foreign).is_err());
    let changed_source = time_policy(b, b);
    assert!(RootPlan::capture(a, sa, &changed_source, &sp, &view).is_err());
}

#[test]
fn tlcx19_checked_tl_actual_reduction_is_consumed_by_joint_representative() {
    use lc631_core::CandidateId;
    use lc631_runtime::joint_selection as j;
    let q = query();
    let good = trace(Truth::True);
    let bad = trace(Truth::False);
    let a = temporal(&q, &good);
    let b = temporal(&q, &bad);
    let sa = spatial(false);
    let sb = spatial(true);
    let tp = time_policy(a, a);
    let sp = space_policy(sa, sa, false);
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let root = RootPlan::capture(a, sa, &tp, &sp, &view).unwrap();
    let ca = TupleCandidate::capture(&root, b, sa).unwrap();
    let cb = TupleCandidate::capture(&root, a, sb).unwrap();
    let bad_tp = time_policy(a, b);
    let bad_sp = space_policy(sa, sb, false);
    let ta =
        TemporalOwner::produce(&bad_tp, a, b, &view, &view, EvaluationBudget::default()).unwrap();
    let tb = TemporalOwner::produce(&tp, a, a, &view, &view, EvaluationBudget::default()).unwrap();
    let sx = SpatialOwner::produce(&sp, sa, sa, &view, &view).unwrap();
    let sy = SpatialOwner::produce(&bad_sp, sa, sb, &view, &view).unwrap();
    let ra = reduce_current(
        &root,
        &ca,
        Owners {
            temporal: Some(&ta),
            spatial: Some(&sx),
        },
        &view,
    )
    .unwrap();
    let rb = reduce_current(
        &root,
        &cb,
        Owners {
            temporal: Some(&tb),
            spatial: Some(&sy),
        },
        &view,
    )
    .unwrap();
    let la = ra.current(&root, &ca, &view).unwrap().total().unwrap();
    let lb = rb.current(&root, &cb, &view).unwrap().total().unwrap();
    assert_eq!(la, &bounds(1, 4, 1, 4));
    assert_eq!(lb, &bounds(3, 4, 3, 4));
    let pins = j::SelectionPins {
        source: context().source(),
        scope: scope(),
        baseline: root.digest(),
        hard_invariants: d(91),
        policy: root.digest(),
        goal_profile: Some(d(92)),
        evidence_revision: d(93),
    };
    let goal = bounds(1, 1, 1, 1);
    let aa = [
        j::AxisValue::Exact(&goal),
        ta.axis_current(&view).unwrap(),
        sx.axis_current(&view).unwrap(),
    ];
    let ab = [
        j::AxisValue::Exact(&goal),
        tb.axis_current(&view).unwrap(),
        sy.axis_current(&view).unwrap(),
    ];
    // Given goals/gates stay descriptive. Only total TL is real-owner-derived.
    let rows = [
        j::JointCandidate {
            id: CandidateId(1),
            context: context(),
            pins,
            hard_gate: j::ReportedGate::Pass,
            map_back: j::ReportedGate::Pass,
            axes: &aa,
            total_loss: j::AxisValue::Exact(la),
            canonical_key: b"z",
            measurement_binding: d(94),
        },
        j::JointCandidate {
            id: CandidateId(2),
            context: context(),
            pins,
            hard_gate: j::ReportedGate::Pass,
            map_back: j::ReportedGate::Pass,
            axes: &ab,
            total_loss: j::AxisValue::Exact(lb),
            canonical_key: b"a",
            measurement_binding: d(95),
        },
    ];
    let kinds = [
        j::AxisKind::Goal(d(92)),
        j::AxisKind::TemporalLoss,
        j::AxisKind::SpatialLoss,
    ];
    let policy = j::SelectionPolicy {
        pins,
        axes: &kinds,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 2,
        max_canonical_bytes: 64,
    };
    let selected =
        j::select_joint_reported(&policy, &rows, j::SearchCoverage::EvaluatedSet).unwrap();
    assert_eq!(selected.frontier.len(), 2);
    assert_eq!(selected.recommendation, Some(0));
    // A total-TL cut must not use the cached recommendation.
    let mut cut = rows;
    cut[0].total_loss = j::AxisValue::Unavailable(j::MissingAxis::MissingEvaluator);
    assert_eq!(
        j::select_joint_reported(&policy, &cut, j::SearchCoverage::EvaluatedSet)
            .unwrap()
            .recommendation,
        Some(1)
    );
}
