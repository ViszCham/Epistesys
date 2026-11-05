//! 日本語: 実capability Plan constructorのraw weight admission。backendは実行しない。
//! English: Actual capability-plan raw-weight admission; no backend execution.
//! Registry capture only observes the current test executable's bounded path hash,
//! never image attestation, capability success, permission or semantic truth.
use lc631_analysis::creativity_measurement::{Rational, RationalInterval};
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::{
    capability_measurement::{Capability, Error, Plan, ProbeSpec, Registry},
    resource::*,
};
use lc631_substrate::measurement_numeric::BigInt;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    time::Duration,
};
fn fixture(body: impl FnOnce(ContextRevision, &ResourceReservationView<'_>)) {
    let source = SourceRevision::of("Given capability weight constructor audit");
    let cap = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker
        .begin_run(
            22,
            source.digest(),
            cap,
            Deadline::after(Duration::from_secs(120)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(24 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(8 * 1024 * 1024).unwrap(),
            ),
        ])
        .unwrap();
    let view = reservation.lease();
    body(ContextRevision::original(source), &view);
}
fn probe(weight: Rational) -> ProbeSpec {
    ProbeSpec {
        capability: Capability::Both,
        weight,
    }
}
#[test]
fn tlcx22_capability_weight_valid_constructor_stays_unobserved() {
    fixture(|context, view| {
        let registry = Registry::capture(context, [5; 32], &[Capability::Both], view).unwrap();
        let probes = [probe(Rational::from_integer(1.into()))];
        let plan = Plan::capture(&registry, &probes, 100, view).unwrap();
        assert_eq!(
            plan.unobserved_bounds(),
            Some(&RationalInterval::from_ratio(0, 1, 1, 1).unwrap())
        );
        // No evaluate(), subprocess or actual capability-label acquisition occurs.
    });
}
#[test]
fn tlcx22_capability_weight_zero_denominator_is_typed_rejection_not_panic() {
    fixture(|context, view| {
        let registry = Registry::capture(context, [5; 32], &[Capability::Both], view).unwrap();
        let probes = [probe(Rational::new_raw(1.into(), 0.into()))];
        let attempted = catch_unwind(AssertUnwindSafe(|| {
            Plan::capture(&registry, &probes, 100, view)
        }));
        assert!(
            attempted.is_ok(),
            "raw zero denominator must be rejected before comparison, never panic"
        );
        let returned = attempted.unwrap();
        assert!(
            matches!(&returned, Err(Error::InvalidPlan)),
            "actual error {:?}",
            returned.as_ref().err()
        );
    });
}
#[test]
fn tlcx22_capability_weight_other_invalid_canonical_and_budget_controls() {
    fixture(|context, view| {
        let registry = Registry::capture(context, [5; 32], &[Capability::Both], view).unwrap();
        for weight in [
            Rational::new_raw(1.into(), (-2).into()),
            Rational::new_raw(2.into(), 4.into()),
            Rational::new_raw(BigInt::from(1) << 1025usize, 1.into()),
            Rational::from_integer(0.into()),
            Rational::from_integer((-1).into()),
        ] {
            let probes = [probe(weight)];
            let attempted = catch_unwind(AssertUnwindSafe(|| {
                Plan::capture(&registry, &probes, 100, view)
            }));
            assert!(attempted.is_ok(), "invalid weights must not panic");
            let returned = attempted.unwrap();
            assert!(
                matches!(&returned, Err(Error::InvalidPlan)),
                "actual error {:?}",
                returned.as_ref().err()
            );
        }
    });
}
