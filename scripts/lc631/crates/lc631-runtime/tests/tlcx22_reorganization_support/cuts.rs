//! Fixed-universe, lifecycle and resource cuts against the future actual owner.
use super::support::*;
use std::{thread, time::Instant};

#[test]
fn tlcx22_reorganization_fixed_denominator_and_permutation() {
    for (ids, symbol, reverse) in [([0, 1], 0, false), ([4, 9], 17, true)] {
        fixture(ids, symbol, RAM, Duration::from_secs(60), |fx, view, _| {
            let mut base = fx.bindings([1, 1], false);
            let mut cand = fx.bindings([1, 2], true);
            let mut uses = fx.root_uses();
            let mut probes = fx.probes([1, 3]);
            let mut maps = fx.maps([1, 2]);
            if reverse {
                base.reverse();
                cand.reverse();
                uses.reverse();
                probes.reverse();
                maps.reverse();
            }
            let baseline = fx.model(&base, &uses);
            let candidate = fx.model(&cand, &uses);
            let plan = r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), limits(), view)
                .unwrap();
            let p =
                r::PreparedRebindProgram::prepare(&plan, &candidate, &maps, CandidateId(80), view)
                    .unwrap();
            let q = r::RebindQueries::capture(&p, view).unwrap();
            let owner = r::OwnedRebind::evaluate(&q, view).unwrap();
            assert_eq!(plan.denominator(), 4);
            owner
                .with_current(view, |c| assert_point(c.report(), 3, 4))
                .unwrap();
            let duplicate = [probes[0], probes[0]];
            assert!(matches!(
                r::RebindPlan::capture_fixed(&baseline, &duplicate, fx.pins(), limits(), view),
                Err(r::Error::DuplicateProbe)
            ));
            assert!(matches!(
                r::RebindPlan::capture_fixed(&baseline, &probes[..1], fx.pins(), limits(), view),
                Err(r::Error::ProbeSetMismatch)
            ));
            assert!(matches!(
                r::RebindPlan::capture_fixed(&baseline, &[], fx.pins(), limits(), view),
                Err(r::Error::EmptyProbeSet)
            ));
            let mut zero = probes;
            zero[0].weight = 0;
            assert!(matches!(
                r::RebindPlan::capture_fixed(&baseline, &zero, fx.pins(), limits(), view),
                Err(r::Error::InvalidWeight)
            ));
            let mut overweight = probes;
            overweight[0].weight = 1_000_001;
            assert!(matches!(
                r::RebindPlan::capture_fixed(&baseline, &overweight, fx.pins(), limits(), view),
                Err(r::Error::InvalidWeight)
            ));
        });
    }
}

#[test]
fn tlcx22_reorganization_source_theory_type_and_goal_cuts() {
    normal(|fx, view, _| {
        let base = fx.bindings([1, 1], false);
        let cand = fx.bindings([1, 2], false);
        let uses = fx.root_uses();
        let baseline = fx.model(&base, &uses);
        let maps = fx.maps([1, 2]);
        let probes = fx.probes([1, 3]);
        let plan =
            r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), limits(), view).unwrap();
        let foreign = SourceStore::new(Arc::from(fx.source.text()), 1024).unwrap();
        let model = r::RoleModel::checked(&foreign, fx.context, &fx.theory, &cand, &uses).unwrap();
        assert!(matches!(
            r::PreparedRebindProgram::prepare(&plan, &model, &maps, CandidateId(2), view),
            Err(r::Error::ForeignSourceOwner)
        ));
        let narrowed = SmtStatusQuery::checked(
            vec![fx.parameter],
            vec![],
            vec![fx.functions[1].clone()],
            SmtEmitLimits::default(),
        )
        .unwrap();
        let model = r::RoleModel::checked(&fx.source, fx.context, &narrowed, &cand, &uses).unwrap();
        assert!(matches!(
            r::PreparedRebindProgram::prepare(&plan, &model, &maps, CandidateId(2), view),
            Err(r::Error::TheoryMismatch)
        ));
        let mut wrong_goal = probes;
        wrong_goal[1].root = &fx.functions[2];
        assert!(matches!(
            r::RebindPlan::capture_fixed(&baseline, &wrong_goal, fx.pins(), limits(), view),
            Err(r::Error::RootBaselineMismatch)
        ));
        let integer = BoolExpr::Equal(
            lc631_solvers::typed_query::IntExpr::Constant(0),
            lc631_solvers::typed_query::IntExpr::Constant(0),
        );
        let mut wrong_type = cand;
        wrong_type[1].body = &integer;
        assert!(matches!(
            r::RoleModel::checked(&fx.source, fx.context, &fx.theory, &wrong_type, &uses),
            Err(r::Error::UnsupportedInteger)
        ));
    });
}

#[test]
fn tlcx22_reorganization_cancel_expiry_callback_and_budget() {
    normal(|fx, view, cancel| {
        let b = fx.bindings([1, 1], false);
        let c = fx.bindings([1, 2], false);
        let uses = fx.root_uses();
        let baseline = fx.model(&b, &uses);
        let candidate = fx.model(&c, &uses);
        let probes = fx.probes([1, 3]);
        let maps = fx.maps([1, 2]);
        let plan =
            r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), limits(), view).unwrap();
        let p = r::PreparedRebindProgram::prepare(&plan, &candidate, &maps, CandidateId(2), view)
            .unwrap();
        let q = r::RebindQueries::capture(&p, view).unwrap();
        let owner = r::OwnedRebind::evaluate(&q, view).unwrap();
        // Even a computed callback result is discarded by the AFTER-current check.
        assert!(matches!(
            owner.with_current(view, |current| {
                assert_point(current.report(), 3, 4);
                cancel.cancel();
                777u16
            }),
            Err(r::Error::Resource(ResourceError::Cancelled))
        ));
        assert!(matches!(
            owner.with_current(view, |_| ()),
            Err(r::Error::Resource(ResourceError::Cancelled))
        ));
    });
    fixture([0, 1], 0, RAM, Duration::from_secs(2), |fx, view, _| {
        let b = fx.bindings([1, 1], false);
        let uses = fx.root_uses();
        let baseline = fx.model(&b, &uses);
        let probes = fx.probes([1, 3]);
        let maps = fx.maps([1, 1]);
        let plan =
            r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), limits(), view).unwrap();
        let p = r::PreparedRebindProgram::prepare(&plan, &baseline, &maps, CandidateId(2), view)
            .unwrap();
        let q = r::RebindQueries::capture(&p, view).unwrap();
        let owner = r::OwnedRebind::evaluate(&q, view).unwrap();
        let started = Instant::now();
        while view.check().is_ok() {
            assert!(
                started.elapsed() < Duration::from_secs(3),
                "deadline did not expire"
            );
            thread::sleep(Duration::from_millis(2));
        }
        assert!(matches!(
            owner.with_current(view, |_| ()),
            Err(r::Error::Resource(ResourceError::DeadlineExceeded))
        ));
    });
    fixture([0, 1], 0, 65_535, Duration::from_secs(60), |fx, view, _| {
        let b = fx.bindings([1, 1], false);
        let uses = fx.root_uses();
        let baseline = fx.model(&b, &uses);
        let probes = fx.probes([1, 3]);
        assert!(matches!(
            r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), limits(), view),
            Err(r::Error::Resource(ResourceError::Exhausted(
                ResourceKind::Ram
            )))
        ));
    });
}

#[test]
fn tlcx22_reorganization_work_is_not_refunded_by_drop_or_unknown() {
    normal(|fx, view, _| {
        let b = fx.bindings([1, 1], false);
        let uses = fx.root_uses();
        let baseline = fx.model(&b, &uses);
        let probes = fx.probes([1, 3]);
        let maps = fx.maps([1, 1]);
        let mut cap = limits();
        cap.max_check_work = 1;
        cap.max_total_work = 4096;
        let plan = r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), cap, view).unwrap();
        let before = plan.work_spent();
        {
            let p =
                r::PreparedRebindProgram::prepare(&plan, &baseline, &maps, CandidateId(2), view)
                    .unwrap();
            let q = r::RebindQueries::capture(&p, view).unwrap();
            let owner = r::OwnedRebind::evaluate(&q, view).unwrap();
            owner.with_current(view,|current| {
                assert!(!matches!(current.report().state,ComponentState::Measured(ref v) if *v==ratio(0,1)));
                assert!(current.has_gaps());
            }).unwrap();
        }
        let after = plan.work_spent();
        assert!(after > before && after <= 4096);
        assert_eq!(
            plan.work_spent(),
            after,
            "dropping query/evaluation refunded work"
        );
        let mut exhausted = false;
        for _ in 0..4097 {
            let attempt = (|| {
                let p = r::PreparedRebindProgram::prepare(
                    &plan,
                    &baseline,
                    &maps,
                    CandidateId(2),
                    view,
                )?;
                let q = r::RebindQueries::capture(&p, view)?;
                let owner = r::OwnedRebind::evaluate(&q, view)?;
                drop(owner);
                Ok::<(), r::Error>(())
            })();
            match attempt {
                Ok(()) => {
                    assert!(plan.work_spent() <= 4096);
                }
                Err(r::Error::WorkBudget) => {
                    exhausted = true;
                    break;
                }
                Err(other) => panic!("unexpected budget diagnostic {other:?}"),
            }
        }
        assert!(exhausted, "same plan reset its work account on retries");
        assert!(plan.work_spent() >= after && plan.work_spent() <= 4096);
    });
}

#[test]
fn tlcx22_reorganization_scratch_and_foreign_reservation_cuts() {
    normal(|fx, view, _| {
        let b = fx.bindings([1, 1], false);
        let uses = fx.root_uses();
        let baseline = fx.model(&b, &uses);
        let probes = fx.probes([1, 3]);
        let maps = fx.maps([1, 1]);
        let plan =
            r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), limits(), view).unwrap();
        let p = r::PreparedRebindProgram::prepare(&plan, &baseline, &maps, CandidateId(2), view)
            .unwrap();
        let q = r::RebindQueries::capture(&p, view).unwrap();
        // Four retained stages reserve exactly 4*64KiB in this profile.
        let pressure = view
            .claim_resources(&[resource(ResourceKind::Ram, RAM - 4 * 65_536 - 262_144 + 1)])
            .unwrap();
        assert!(matches!(
            r::OwnedRebind::evaluate(&q, view),
            Err(r::Error::Resource(ResourceError::Exhausted(
                ResourceKind::Ram
            )))
        ));
        drop(pressure);
        let owner = r::OwnedRebind::evaluate(&q, view).unwrap();
        normal(|_, foreign, _| {
            assert!(matches!(
                owner.with_current(foreign, |_| ()),
                Err(r::Error::ForeignReservation)
            ));
        });
        owner
            .with_current(view, |c| assert_point(c.report(), 0, 1))
            .unwrap();
    });
}
