//! One registered native producer-positive pilot, selected centrally after the
//! common startup diagnosis. This does not assess the requested source Goal.
//! Existing fixture setup compiles two binaries; this case observes one native
//! child in an explicitly labelled supplemental 30s positive profile. The shared
//! run stays at120s; existing 5s negative fixtures and expiry assertions stay intact.
mod tlcx31_registered_clock_support;

use tlcx31_registered_clock_support::at_native_birth_supplemental_30s;

const POSITIVE_TIMEOUT_MS: u64 = 30_000;

#[test]
fn registered_first_native_positive_preserves_exact_capture_supplemental_30s() {
    at_native_birth_supplemental_30s(|case| {
        let remaining = case.view.remaining().expect("positive fixture run must still be live");
        assert!(remaining >= std::time::Duration::from_millis(POSITIVE_TIMEOUT_MS),
            "positive fixture needs its full30s operation budget; remaining={remaining:?}");
        eprintln!("registered-clock profile=supplemental_positive_30s timeout_ms={} shared_remaining={remaining:?} original_negative_timeout_ms=5000 logical_epoch=100 expiry=400",
            POSITIVE_TIMEOUT_MS);
        assert!(!case.marker.exists());

        let diagnostic = case.native_diagnostic();
        let attempt = case.session.observe_native(
            case.plan, case.input, case.origin, case.view,
        );
        if let Err(error) = &attempt {
            case.report_native_diagnostic("positive_pilot_native", diagnostic, error);
        }
        let observed = attempt.expect("the registered native producer must complete");
        assert_eq!(observed.checked_epoch(), 100);
        assert_eq!(
            observed.actual_outputs(),
            &[Some(0), Some(3), Some(0), Some(u32::MAX)],
        );
        let process = observed.process();
        assert_eq!(process.reported().exit_code, Some(0));
        assert!(!process.reported().timed_out);
        assert_eq!(
            process.stdout(),
            b"some:0\nsome:3\nsome:0\nsome:4294967295\n",
        );
        assert_eq!(std::fs::read(case.marker).unwrap(), b"X");
        case.session
            .with_native_failure(|history| assert!(history.is_none()))
            .unwrap();
    });
}

