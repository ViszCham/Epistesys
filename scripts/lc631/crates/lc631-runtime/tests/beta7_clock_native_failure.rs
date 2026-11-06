//! A genuine registered native child completes before its after-sample expires.
//! The observed stdout and marker remain historical; the failed role cannot retry.
mod tlcx31_registered_clock_support;

use lc631_runtime::goal_clock_domain::{ClockFault, Error};
use std::sync::atomic::Ordering;
use tlcx31_registered_clock_support::at_native_birth;

#[test]
fn registered_post_dispatch_expiry_retains_actual_capture_and_refuses_second_child() {
    at_native_birth(|case| {
        let clock = &case.clocks.be;
        let before_reads = clock.calls();
        clock.sequence(&[100, 100, 400], 400);
        assert!(!case.marker.exists());

        let native_diagnostic = case.native_diagnostic();
        let error = case.session.observe_native(case.plan, case.input, case.origin, case.view)
            .err().expect("the real after-child sample is exactly the exclusive expiry");
        case.report_native_diagnostic("first_native", native_diagnostic, &error);
        assert_eq!(error, Error::Clock(ClockFault::ObservedExpiry));
        assert_eq!(clock.calls(), before_reads + 3);
        assert_eq!(std::fs::read(case.marker).unwrap(), b"X");
        case.session.with_native_failure(|failed| {
            let history = failed.expect("the actual lower owner survives the refusal");
            assert_eq!(history.cause(), &Error::Clock(ClockFault::ObservedExpiry));
            assert!(history.lower_dispatch_entered());
            let process = history.process().expect("the child completed before expiry");
            assert_eq!(process.reported().exit_code, Some(0));
            assert_eq!(process.stdout(), b"some:0\nsome:3\nsome:0\nsome:4294967295\n");
            assert_eq!(history.parsed_outputs(), Some(&[Some(0), Some(3), Some(0), Some(u32::MAX)]));
        }).unwrap();

        // An otherwise valid clock cannot erase that failure or start a second
        // child on this same role, including by calling the real factory again.
        clock.set(200);
        let registry_calls = case.registry.calls.load(Ordering::SeqCst);
        let retry_diagnostic = case.native_diagnostic();
        let retry = case.session.observe_native(case.plan, case.input, case.origin, case.view)
            .err().expect("the occupied failure slot rejects before native dispatch");
        case.report_native_diagnostic("retry_native", retry_diagnostic, &retry);
        assert_eq!(retry, Error::Clock(ClockFault::BudgetExhausted));
        assert_eq!(
            case.registry.calls.load(Ordering::SeqCst), registry_calls,
            "retry must stop before querying any provider for this irrecoverable role"
        );
        assert_eq!(clock.calls(), before_reads + 3);
        assert_eq!(std::fs::read(case.marker).unwrap(), b"X");
    });
}
