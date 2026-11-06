//! 日本語: 既存supervisor APIだけの実child観測。CF bridge成功とはしない。
//! English: Actual-child observations using existing supervisor APIs only, not CF-bridge success.
#[path = "tlcx22_cf_capture_support/mod.rs"]
mod support;
use std::time::Duration;
use support::*;

// Structural entry guard is not a bounded-I/O positive. Child runs this exact
// test with a private mode and exits before any libtest success postlude.
#[test]
fn cf_capture_fixture_entry() {
    support::fixture_entry();
}
fn clean(out: &sup::RunOutcome, code: i32) {
    assert_finished(out);
    assert_eq!(out.exit_code, Some(code));
    assert!(!out.timed_out && !out.stdout_truncated && !out.stderr_truncated);
    assert!(
        out.diagnostic.is_none(),
        "not a completed IO observation: {out:?}"
    );
}
#[test]
fn r01_binary_fixture_stdout_keeps_harness_prelude_separate() {
    let budget = budget(22101);
    let view = budget.reservation.lease();
    let child = ChildRequest::fixture(
        "metadata",
        b"actual fixture stdin",
        Duration::from_secs(10),
        4096,
        4096,
    );
    let owner = captured(child.request(), &view);
    let out = owner.outcome();
    clean(out, 0);
    let (harness_prelude, fixture_payload) = complete_fixture_payload(&out.stdout);
    assert!(harness_prelude.len() <= 1024);
    let expected: Vec<u8> = (0..=255).collect();
    assert_eq!(fixture_payload, expected.as_slice());
    assert!(fixture_payload.contains(&0)); // binary fixture data, not rustc metadata
    drop(owner);
    let _ = view;
    finish(budget);
}
#[test]
fn r02_actual_stdin_delivery_and_eof_are_observed_by_echo() {
    let budget = budget(22102);
    let view = budget.reservation.lease();
    let input = b"bounded actual stdin bytes, not the frozen CF prelude";
    let child = ChildRequest::fixture("echo", input, Duration::from_secs(10), 4096, 4096);
    let owner = captured(child.request(), &view);
    let out = owner.outcome();
    clean(out, 0);
    let (_, payload) = complete_fixture_payload(&out.stdout);
    assert_eq!(payload, input);
    drop(owner);
    let _ = view;
    finish(budget);
}
#[test]
fn r03_stderr_flood_is_capped_even_when_prefix_contains_expected_marker() {
    let budget = budget(22103);
    let view = budget.reservation.lease();
    let child = ChildRequest::fixture("stderr-flood", b"", Duration::from_secs(10), 4096, 1024);
    let owner = captured(child.request(), &view);
    let out = owner.outcome();
    assert!(
        has_fixture_entry(&out.stdout),
        "fixture must actually enter before flood"
    );
    assert!(out.stderr_truncated);
    assert_eq!(out.stderr.len(), 1024);
    assert!(out
        .stderr
        .windows(b"from_report".len())
        .any(|x| x == b"from_report"));
    assert_eq!(out.diagnostic, Some("supervisor_output_budget_exceeded"));
    assert!(out.process_tree_kill_attempted);
    assert_finished(out);
    // Captured marker bytes are partial data, never ownership evidence.
    drop(owner);
    let _ = view;
    finish(budget);
}
#[test]
fn r04_stdout_flood_uses_a_cap_larger_than_the_harness_prelude() {
    let budget = budget(22104);
    let view = budget.reservation.lease();
    let child = ChildRequest::fixture("stdout-flood", b"", Duration::from_secs(10), 8192, 4096);
    let owner = captured(child.request(), &view);
    let out = owner.outcome();
    assert!(has_fixture_entry(&out.stdout));
    assert!(out.stdout_truncated);
    assert_eq!(out.stdout.len(), 8192);
    assert_eq!(out.diagnostic, Some("supervisor_output_budget_exceeded"));
    assert_finished(out);
    drop(owner);
    let _ = view;
    finish(budget);
}
#[test]
fn r05_clean_process_eof_can_still_contain_only_partial_diagnostic_bytes() {
    let budget = budget(22105);
    let view = budget.reservation.lease();
    let child = ChildRequest::fixture("partial-json", b"", Duration::from_secs(10), 4096, 1024);
    let owner = captured(child.request(), &view);
    let out = owner.outcome();
    clean(out, 1);
    assert_eq!(out.stderr, fixture::PARTIAL);
    assert!(!out.stderr.ends_with(b"\n"));
    // Supervisor transport completion alone cannot authorize CF matching.
    drop(owner);
    let _ = view;
    finish(budget);
}
#[test]
fn r06_ice_shaped_fixture_bytes_with_exit_one_are_not_compiler_success() {
    let budget = budget(22106);
    let view = budget.reservation.lease();
    let child = ChildRequest::fixture("ice", b"", Duration::from_secs(10), 4096, 4096);
    let owner = captured(child.request(), &view);
    let out = owner.outcome();
    clean(out, 1);
    assert_eq!(out.stderr, fixture::ICE);
    assert!(has_fixture_entry(&out.stdout));
    drop(owner);
    let _ = view;
    finish(budget);
}
#[test]
fn r07_unknown_exit_preserves_real_code_and_bytes_without_negative_success() {
    let budget = budget(22107);
    let view = budget.reservation.lease();
    let child = ChildRequest::fixture("unknown-exit", b"", Duration::from_secs(10), 4096, 4096);
    let owner = captured(child.request(), &view);
    let out = owner.outcome();
    clean(out, 42);
    assert!(out.stderr.windows(b"borrow".len()).any(|x| x == b"borrow"));
    drop(owner);
    let _ = view;
    finish(budget);
}
#[test]
fn r08_timeout_requires_actual_entry_tree_exit_and_io_cleanup() {
    let budget = budget(22108);
    let view = budget.reservation.lease();
    let child = ChildRequest::fixture("sleep", b"", Duration::from_secs(2), 4096, 4096);
    let owner = captured(child.request(), &view);
    let out = owner.outcome();
    assert!(has_fixture_entry(&out.stdout));
    assert!(out.timed_out);
    assert!(out.process_tree_kill_attempted);
    assert_finished(out);
    assert_eq!(out.diagnostic, Some("supervisor_deadline_exceeded"));
    drop(owner);
    let _ = view;
    finish(budget);
}
#[test]
fn r09_repeated_blocked_stdin_cleans_actual_writers_without_new_slots() {
    let budget = budget(22109);
    let view = budget.reservation.lease();
    let input = vec![b'z'; 2 * 1024 * 1024];
    for _ in 0..3 {
        let child =
            ChildRequest::fixture("blocked-stdin", &input, Duration::from_secs(2), 4096, 4096);
        let owner = captured(child.request(), &view);
        let out = owner.outcome();
        assert!(has_fixture_entry(&out.stdout));
        assert!(out.timed_out);
        assert_finished(out);
        drop(owner);
    }
    let _ = view;
    finish(budget);
}
#[test]
fn r10_real_small_rustc_source_produces_nonempty_bounded_metadata_stdout() {
    let budget = budget(22110);
    let view = budget.reservation.lease();
    let child = ChildRequest::compiler();
    assert!(sup::required_ram_bytes(&child.request()).unwrap() <= 32 * 1024 * 1024);
    let owner = captured(child.request(), &view);
    let out = owner.outcome();
    clean(out, 0);
    assert!(
        !out.stdout.is_empty(),
        "positive metadata must not be treated as cap-zero"
    );
    assert!(out.stdout.len() <= 1024 * 1024 && out.stderr.len() <= 1024 * 1024);
    assert!(
        !has_fixture_entry(&out.stdout),
        "rustc output is separate from libtest fixture frames"
    );
    drop(owner);
    let _ = view;
    finish(budget);
}
#[test]
fn r11_kernel_oversized_stdin_is_rejected_before_actual_native_dispatch() {
    let budget = budget(22111);
    let view = budget.reservation.lease();
    let input = vec![b'q'; 2 * 1024 * 1024 + 1];
    let child = ChildRequest::fixture("echo", &input, Duration::from_secs(10), 4096, 4096);
    let trace = sup::observe_native_dispatches(|| {
        sup::run_bounded_with_resources_observed(child.request(), &view)
    })
    .unwrap();
    assert!(trace.is_complete());
    assert!(trace.dispatches().is_empty());
    assert!(matches!(
        trace.into_result(),
        Err(sup::ResourceRunError::Resource(ResourceError::Io(
            std::io::ErrorKind::InvalidInput
        )))
    ));
    let _ = view;
    finish(budget);
}
