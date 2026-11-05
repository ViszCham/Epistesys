//! Actual lease identity, cancellation and admission/refund boundaries.
#[path = "tlcx23_formal_support/mod.rs"]
mod support;
use lc631_core::context::ContextScope;
use lc631_core::measurement_numeric::RationalInterval as Interval;
use lc631_runtime::formal_measurement as m;
use lc631_runtime::formal_measurement::owned;
use lc631_runtime::formal_measurement::owned::OwnedError;
use lc631_runtime::joint_selection::formal::BothDirectionReduction as Reduction;
use lc631_runtime::joint_selection::formal::*;
use lc631_substrate::resource::{Cancellation, ResourceError, ResourceKind};
use support::*;

#[test]
fn tlcx23_formal_actual_input_and_policy_identity() {
    cases(|c| {
        let (_b, _run, reservation) = setup(&c, Cancellation::default());
        let view = reservation.lease();
        let (s, k) = missing_pair(&c, &view);
        let one = Interval::exact_ratio(1, 1).unwrap();
        let policy = policy(pins(c.plan), one.lower(), one.lower(), &view);
        let p = policy.pins();
        assert!(matches!(
            reduce_formal_owned(&policy, &k, &s, c.plan, c.good, &view),
            Err(FormalAxisError::Owned(OwnedError::BindingMismatch))
        ));
        let axis = reduce_formal_owned(&policy, &s, &k, c.plan, c.good, &view).unwrap();
        let same_policy =
            Reduction::declared_registered(p, p.policy, d(2), one.lower(), one.lower());
        assert!(matches!(
            axis.project_current(&same_policy, p, c.plan, c.good, &view),
            Err(FormalAxisError::PolicyMismatch)
        ));
        let template = c.plan.fixed_probes()[0].template;
        let other_plan = m::ProbePlan::checked(
            c.plan.scope(),
            template.source,
            c.plan.fixed_probes(),
            work(),
        )
        .unwrap();
        assert!(matches!(
            axis.project_current(&policy, p, &other_plan, c.good, &view),
            Err(FormalAxisError::Owned(OwnedError::BindingMismatch))
        ));
        let other_candidate = m::Candidate::given(
            c.good.id(),
            p.scope.comparison(),
            template.target,
            c.good.revision(),
            c.good.context(),
        )
        .unwrap();
        assert!(matches!(
            axis.project_current(&policy, p, c.plan, &other_candidate, &view),
            Err(FormalAxisError::Owned(OwnedError::BindingMismatch))
        ));
        let mut foreign = p;
        foreign.scope = ContextScope::declared(d(99), d(20), d(3), d(21));
        assert!(matches!(
            axis.project_current(&policy, foreign, c.plan, c.good, &view),
            Err(FormalAxisError::PolicyMismatch)
        ));
    });
}

#[test]
fn tlcx23_formal_same_ids_foreign_lease() {
    cases(|c| {
        let (_b, _run, r) = setup(&c, Cancellation::default());
        let view = r.lease();
        let (s, k) = missing_pair(&c, &view);
        let one = Interval::exact_ratio(1, 1).unwrap();
        let policy = policy(pins(c.plan), one.lower(), one.lower(), &view);
        let p = policy.pins();
        let axis = reduce_formal_owned(&policy, &s, &k, c.plan, c.good, &view).unwrap();
        let b2 = broker();
        let run2 = run(&b2, c.plan.scope().source.digest(), Cancellation::default());
        let r2 = reserve(&run2, 32 * 1024 * 1024, 1024 * 1024);
        let other = r2.lease();
        assert_eq!(view.run_id(), other.run_id());
        assert_eq!(view.reservation_id(), other.reservation_id());
        assert!(matches!(
            axis.project_current(&policy, p, c.plan, c.good, &other),
            Err(FormalAxisError::ForeignReservation)
        ));
        assert!(matches!(
            reduce_formal_owned(&policy, &s, &k, c.plan, c.good, &other),
            Err(FormalAxisError::Owned(OwnedError::ForeignReservation))
        ));
    });
}

#[test]
fn tlcx23_formal_cancelled_history_only() {
    cases(|c| {
        let cancel = Cancellation::default();
        let (_b, _run, r) = setup(&c, cancel.clone());
        let view = r.lease();
        let (s, k) = missing_pair(&c, &view);
        let one = Interval::exact_ratio(1, 1).unwrap();
        let policy = policy(pins(c.plan), one.lower(), one.lower(), &view);
        let p = policy.pins();
        let axis = reduce_formal_owned(&policy, &s, &k, c.plan, c.good, &view).unwrap();
        cancel.cancel();
        assert_eq!(
            axis.historical_bounds(),
            &Interval::from_ratio(0, 1, 1, 1).unwrap()
        );
        assert!(matches!(
            axis.project_current(&policy, p, c.plan, c.good, &view),
            Err(FormalAxisError::Resource(ResourceError::Cancelled))
        ));
        assert!(matches!(
            reduce_formal_owned(&policy, &s, &k, c.plan, c.good, &view),
            Err(FormalAxisError::Resource(ResourceError::Cancelled))
        ));
    });
}

#[test]
fn tlcx23_formal_claim_quotas_and_refunds() {
    cases(|c| {
        let bounds = owned::requirements(c.plan).unwrap();
        for (ram_short, output_short) in [(true, false), (false, true)] {
            let b = broker();
            let run = run(&b, c.plan.scope().source.digest(), Cancellation::default());
            let ram = bounds.scratch_ram + 2 * bounds.retained_ram;
            let output =
                2 * bounds.retained_output + REDUCTION_RETAINED_OUTPUT - u64::from(output_short);
            let r = reserve(&run, ram, output);
            let view = r.lease();
            let (s, k) = missing_pair(&c, &view);
            let one = Interval::exact_ratio(1, 1).unwrap();
            let policy = policy(pins(c.plan), one.lower(), one.lower(), &view);
            let hog = if ram_short {
                Some(
                    view.claim_resources(&[req(
                        ResourceKind::Ram,
                        bounds.scratch_ram - REDUCTION_SCRATCH_RAM + 1,
                    )])
                    .unwrap(),
                )
            } else {
                None
            };
            let result = reduce_formal_owned(&policy, &s, &k, c.plan, c.good, &view);
            let expected = if ram_short {
                ResourceKind::Ram
            } else {
                ResourceKind::Output
            };
            assert!(
                matches!(result, Err(FormalAxisError::Resource(ResourceError::Exhausted(kind))) if kind == expected)
            );
            drop(hog);
            let _refunded = view
                .claim_resources(&[req(ResourceKind::Ram, bounds.scratch_ram)])
                .unwrap();
        }
    });
}
