//! 日本語: TEST-FIRST planned API。module未提供ならconstruction REDでありbehavior証明でない。
//! English: Test-first planned API; absent module is construction RED, not behavior proof.
#[allow(dead_code)]
#[path = "tlcx22_cf_capture_support/mod.rs"]
mod support;
use lc631_runtime::cf_bounded_probe as probe;
use std::time::Duration;
use support::*;
#[test]
fn cf_capture_fixture_entry() {
    support::fixture_entry();
}
fn started<'v>(
    request: sup::RunRequest<'_>,
    purpose: probe::Purpose,
    view: &'v ResourceReservationView<'v>,
) -> probe::ProbeAttempt<'v> {
    let actual = sup::observe_native_dispatches(|| probe::run(request, purpose, view)).unwrap();
    assert!(actual.is_complete());
    assert_eq!(actual.dispatches().len(), 1);
    assert!(actual.dispatches()[0].process_id() > 0);
    actual.into_result()
}
#[test]
fn c01_zero_metadata_stdout_cap_rejects_before_native_start() {
    let budget = budget(22201);
    let view = budget.reservation.lease();
    let child = ChildRequest::fixture("metadata", b"", Duration::from_secs(10), 4096, 4096);
    let request = sup::RunRequest {
        stdout_limit: 0,
        ..child.request()
    };
    let trace = sup::observe_native_dispatches(|| {
        probe::run(request, probe::Purpose::PositiveControl, &view)
    })
    .unwrap();
    assert!(trace.is_complete());
    assert!(trace.dispatches().is_empty());
    let attempt = trace.into_result();
    assert!(matches!(
        attempt.failure(),
        Some(probe::Failure::InvalidCaptureLimits)
    ));
    assert!(attempt.raw().is_none());
    drop(attempt);
    drop(view);
    finish(budget);
}
#[test]
fn c02_cf_source_input_over_64k_rejects_before_native_start() {
    let budget = budget(22202);
    let view = budget.reservation.lease();
    let input = vec![b'i'; 65537];
    let child = ChildRequest::fixture("echo", &input, Duration::from_secs(10), 4096, 4096);
    let trace = sup::observe_native_dispatches(|| {
        probe::run(child.request(), probe::Purpose::PositiveControl, &view)
    })
    .unwrap();
    assert!(trace.is_complete());
    assert!(trace.dispatches().is_empty());
    let attempt = trace.into_result();
    assert!(matches!(
        attempt.failure(),
        Some(probe::Failure::InputLimit)
    ));
    drop(attempt);
    drop(view);
    finish(budget);
}
#[test]
fn c03_stderr_flood_partial_marker_never_reaches_complete_consumer() {
    let budget = budget(22203);
    let view = budget.reservation.lease();
    let child = ChildRequest::fixture("stderr-flood", b"", Duration::from_secs(10), 4096, 1024);
    let attempt = started(child.request(), probe::Purpose::NegativeCompile, &view);
    let raw = attempt
        .raw()
        .expect("actual partial capture must remain available");
    assert!(has_fixture_entry(&raw.stdout));
    assert!(raw.stderr_truncated);
    assert!(raw
        .stderr
        .windows(b"from_report".len())
        .any(|x| x == b"from_report"));
    assert_finished(raw);
    assert!(matches!(
        attempt.failure(),
        Some(probe::Failure::OutputLimit(probe::Stream::Stderr))
    ));
    assert!(matches!(
        attempt.with_complete(&view, |_| panic!("partial diagnostic consumed")),
        Err(probe::Failure::OutputLimit(probe::Stream::Stderr))
    ));
    drop(attempt);
    drop(view);
    finish(budget);
}
#[test]
fn c04_eof_without_full_json_line_is_not_complete_diagnostic() {
    let budget = budget(22204);
    let view = budget.reservation.lease();
    let child = ChildRequest::fixture("partial-json", b"", Duration::from_secs(10), 4096, 1024);
    let attempt = started(child.request(), probe::Purpose::NegativeCompile, &view);
    let raw = attempt.raw().unwrap();
    assert_finished(raw);
    assert_eq!(raw.exit_code, Some(1));
    assert!(!raw.stderr_truncated);
    assert_eq!(raw.stderr, fixture::PARTIAL);
    assert!(matches!(
        attempt.with_complete(&view, |_| ()),
        Err(probe::Failure::PartialDiagnostic)
    ));
    drop(attempt);
    drop(view);
    finish(budget);
}
#[test]
fn c05_ice_level_even_with_exit_one_is_not_ownership_diagnostic() {
    let budget = budget(22205);
    let view = budget.reservation.lease();
    let child = ChildRequest::fixture("ice", b"", Duration::from_secs(10), 4096, 4096);
    let attempt = started(child.request(), probe::Purpose::NegativeCompile, &view);
    let raw = attempt.raw().unwrap();
    assert_finished(raw);
    assert_eq!(raw.exit_code, Some(1));
    assert_eq!(raw.stderr, fixture::ICE);
    assert!(matches!(
        attempt.with_complete(&view, |_| ()),
        Err(probe::Failure::Ice)
    ));
    drop(attempt);
    drop(view);
    finish(budget);
}
#[test]
fn c06_unknown_exit_does_not_pass_even_if_marker_is_present() {
    let budget = budget(22206);
    let view = budget.reservation.lease();
    let child = ChildRequest::fixture("unknown-exit", b"", Duration::from_secs(10), 4096, 4096);
    let attempt = started(child.request(), probe::Purpose::NegativeCompile, &view);
    assert_eq!(attempt.raw().unwrap().exit_code, Some(42));
    assert!(matches!(
        attempt.with_complete(&view, |_| ()),
        Err(probe::Failure::UnexpectedExit(Some(42)))
    ));
    drop(attempt);
    drop(view);
    finish(budget);
}
#[test]
fn c07_real_timeout_and_cleanup_preserve_failure_not_success() {
    let budget = budget(22207);
    let view = budget.reservation.lease();
    let child = ChildRequest::fixture("sleep", b"", Duration::from_secs(2), 4096, 4096);
    let attempt = started(child.request(), probe::Purpose::NegativeCompile, &view);
    let raw = attempt.raw().unwrap();
    assert!(has_fixture_entry(&raw.stdout));
    assert!(raw.timed_out);
    assert_finished(raw);
    assert!(matches!(
        attempt.with_complete(&view, |_| ()),
        Err(probe::Failure::TimedOut)
    ));
    drop(attempt);
    drop(view);
    finish(budget);
}
#[test]
fn c08_actual_small_compiler_metadata_positive_keeps_nonzero_stdout() {
    let budget = budget(22208);
    let view = budget.reservation.lease();
    let child = ChildRequest::compiler();
    let attempt = started(child.request(), probe::Purpose::PositiveControl, &view);
    assert!(attempt.failure().is_none());
    assert!(attempt.supervisor_error().is_none());
    attempt
        .with_complete(&view, |complete| {
            assert_eq!(complete.exit_code(), 0);
            assert_eq!(complete.purpose(), probe::Purpose::PositiveControl);
            assert!(!complete.stdout().is_empty() && complete.stdout().len() <= 1024 * 1024);
            assert!(complete.stderr().len() <= 1024 * 1024);
        })
        .unwrap();
    drop(attempt);
    drop(view);
    finish(budget);
}
