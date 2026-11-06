//! Both kernels of one preregistration draw on its declared call allowance.
#[path = "tlcx22_framework_p2_support/mod.rs"]
mod support;
use support::*;
#[path = "beta7_framework_fixture_support/mod.rs"]
mod native_fixture;
use native_fixture::capture_native;

#[test]
fn framework_capability_native_prepayment_cannot_start_a_second_boolean_allowance() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let declaration = h.declaration();
        // The fixed profile evaluates 40 chain models × 24 recipes. Its 960
        // source digests cover nine bytes each, in addition to 303,072 work.
        assert_eq!(h.source.text().len(), 9);
        const NATIVE_PREPAYMENT: u64 = 303_072 + 960 * 9;
        let capture = |call_limit| {
            let mut limits = context_limits();
            limits.max_call_work = call_limit;
            assert_eq!(limits.max_total_work, 8_000_000);
            let native = capture_native(&h, limits, view).expect("the valid native root admits these limits");
            f::FrameworkRoot::preregister(native, &declaration, view)
        };

        let complete = capture(1_000_000).expect("high-cap control admits both capability families");
        let ce = complete
            .expected_inputs_current(f::Subject::Original, f::Kernel::ClassicalPointV1, view)
            .unwrap();
        complete.execute_case_current(ce, view).unwrap().with_current(ce, view, |case| {
            assert_eq!(case.capability_model_counts(), &[1, 2, 4, 8]);
            assert_eq!(case.counts().native_dispatches, 28);
        }).unwrap();
        drop(complete);

        assert!(matches!(capture(NATIVE_PREPAYMENT - 1), Err(f::Error::WorkBudget)));
        assert!(
            matches!(capture(NATIVE_PREPAYMENT), Err(f::Error::WorkBudget)),
            "the native prepayment alone consumes the call cap; positive Boolean work must not receive another allowance"
        );
    });
}
