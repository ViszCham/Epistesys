//! 日本語: 実owned producerのraw weight admission。新leafの完成証拠ではない。
//! English: Real owned-producer raw-weight admission; not completion of a new leaf.
use lc631_analysis::creativity_measurement::Rational;
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::{
    facet_ledger::{ComparisonScope, Revision, ScopeId},
    reconstruction_measurement::{
        self as reconstruction, Error, FeatureKind, ProbeSpec, ProtectedFeatureSet,
    },
    resource::*,
};
use lc631_substrate::measurement_numeric::BigInt;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    time::Duration,
};
const SOURCE: &[u8] = &[1, 1]; // Actual canonical K6 Bool(true), not an injected result.
fn fixture(body: impl FnOnce(ContextRevision, &ResourceReservationView<'_>)) {
    let source = SourceRevision::of("Given finite Boolean reconstruction admission audit");
    let cap = ResourceLimits::uniform(64 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker
        .begin_run(
            22,
            source.digest(),
            cap,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(48 * 1024 * 1024).unwrap(),
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
fn scope() -> ComparisonScope {
    ComparisonScope {
        id: ScopeId([1; 32]),
        baseline: Revision([2; 32]),
        root_goal: Revision([3; 32]),
        probes: Revision([4; 32]),
        policy: Revision([5; 32]),
    }
}
fn probe(weight: Rational) -> ProbeSpec {
    ProbeSpec {
        path: vec![],
        kind: FeatureKind::Boolean,
        weight,
    }
}
#[test]
fn tlcx22_weight_admission_valid_owned_capture_and_real_decoder_control() {
    fixture(|context, view| {
        let specs = [probe(Rational::from_integer(1.into()))];
        let plan = ProtectedFeatureSet::capture(SOURCE, &specs, context, scope(), view).unwrap();
        assert_eq!(plan.feature_count(), 1);
        let measured = reconstruction::produce(&plan, SOURCE, view).unwrap();
        assert_eq!(
            measured.historical_bounds(),
            Some(
                &lc631_analysis::creativity_measurement::RationalInterval::exact_ratio(0, 1)
                    .unwrap()
            )
        );
        assert!(measured.bundle_bits().is_some());
        assert_eq!(measured.historical_gap(), None);
        // These are actual finite structural checks, not source/world truth.
        assert_eq!(measured.input_basis(), reconstruction::InputBasis::Given);
    });
}
#[test]
fn tlcx22_weight_admission_zero_denominator_rejected_before_comparison_without_panic() {
    fixture(|context, view| {
        let specs = [probe(Rational::new_raw(1.into(), 0.into()))];
        let attempted = catch_unwind(AssertUnwindSafe(|| {
            ProtectedFeatureSet::capture(SOURCE, &specs, context, scope(), view)
        }));
        assert!(
            attempted.is_ok(),
            "raw zero denominator must produce InvalidPlan, never panic during comparison"
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
fn tlcx22_weight_admission_other_noncanonical_or_over_profile_weights_typed() {
    fixture(|context, view| {
        for weight in [
            Rational::new_raw(1.into(), (-2).into()),
            Rational::new_raw(2.into(), 4.into()),
            Rational::new_raw(BigInt::from(1) << 1025usize, 1.into()),
            Rational::from_integer(0.into()),
            Rational::from_integer((-1).into()),
        ] {
            let specs = [probe(weight)];
            let attempted = catch_unwind(AssertUnwindSafe(|| {
                ProtectedFeatureSet::capture(SOURCE, &specs, context, scope(), view)
            }));
            assert!(
                attempted.is_ok(),
                "malformed/out-of-profile weights must not panic"
            );
            let returned = attempted.unwrap();
            assert!(
                matches!(&returned, Err(Error::InvalidPlan)),
                "actual error {:?}",
                returned.as_ref().err()
            );
        }
    });
}
