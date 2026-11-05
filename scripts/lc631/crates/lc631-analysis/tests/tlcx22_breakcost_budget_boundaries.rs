//! Negative-first boundary regressions; existing21 tests remain frozen.
//! Source-digest parity is a representation control, not a timing/allocation claim.
#[path = "tlcx22_breakcost_support/mod.rs"]
mod support;
use lc631_analysis::{breakcost_measurement::*, *};
use lc631_core::stable_sha256;
use lc631_substrate::{resource::Cancellation, source::SourceStore, Digest32};
use std::sync::Arc;
use support::*;

fn padded_probe(bytes: usize) -> DgclStandaloneCandidate {
    let mut input = probe();
    let initial = serde_json::to_vec(&input).unwrap().len();
    assert!(initial < bytes, "fixture must have room for ASCII padding");
    // Given metadata only: no verified Program IR or implementation claim.
    input
        .program_ir_digest
        .push_str(&"0".repeat(bytes - initial));
    assert_eq!(serde_json::to_vec(&input).unwrap().len(), bytes);
    input
}
fn assert_v1_absolute_zero_at_input_size(bytes: usize) {
    let source = source();
    let raw = padded_probe(bytes);
    let one = q(1, 1);
    let cases = [case(1, b"fixed-budget-boundary", &raw, &one)];
    let rules = rules();
    let (_issuer, live) = lease(Cancellation::default());
    let view = live.view();
    let root = register(
        &source,
        binding(),
        &cases,
        &rules,
        AdapterKind::LegacyProjectionV1,
        budget(),
        &view,
    )
    .expect("legal serialized input must register within unchanged4096-byte cap");
    let baseline = observe_baseline_current(&root, &view)
        .expect("actual V1 baseline at admitted input size must have a legal K1 key");
    assert_cost(baseline.historical_report(), 0, 1);
    let request = requests(AdapterKind::LegacyProjectionV1);
    let frozen = bind_candidate(&root, candidate(1), &request, &view).unwrap();
    let owner = observe_breakcost_current(&root, &baseline, &frozen, &view)
        .expect("actual V1 candidate must retain the admitted input domain");
    assert_cost(
        owner
            .report_current(inputs(&source, &root, &cases, &baseline, &frozen), &view)
            .unwrap(),
        0,
        1,
    );
}
#[test]
fn legal_4095_byte_input_is_a_real_zero_cost_control() {
    assert_v1_absolute_zero_at_input_size(4095);
}
#[test]
fn legal_4096_byte_input_remains_zero_cost_for_actual_baseline_and_candidate() {
    assert_v1_absolute_zero_at_input_size(4096);
}
fn cached_legacy_digest_oracle(digest: Digest32) -> String {
    // Test-owned exact legacy spelling oracle, not a new production entrypoint.
    let mut result = String::from("sha256:");
    for byte in digest.as_bytes() {
        use std::fmt::Write as _;
        write!(&mut result, "{byte:02x}").unwrap();
    }
    result
}
#[test]
fn immutable_cached_digest_has_exact_legacy_spelling_for_bounded_sources() {
    let larger = "a".repeat(16 * 1024);
    for text in ["", SOURCE, "日本語 Σ", larger.as_str()] {
        let store = SourceStore::new(Arc::from(text), 16 * 1024).unwrap();
        let cached = cached_legacy_digest_oracle(store.id().digest());
        assert_eq!(cached, stable_sha256(text));
        assert_eq!(cached.len(), 71);
    }
    // This passes before a repair too. It proves no production scan was removed.
}
