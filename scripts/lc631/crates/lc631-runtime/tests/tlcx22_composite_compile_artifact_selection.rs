//! 日本語: 選択helperの負例先行。仮metadataの判定はcompiler証明ではない。
//! English: Negative-first artifact selection; synthetic metadata checks are not compiler proof.
#[path = "tlcx22_composite_compile_support/mod.rs"]
mod support;
use serde_json::{json, Value};
use support::fingerprint::{hex_fingerprint, parse, select, verify_chain, Candidate, Kind};
use support::{Error, LinkedArtifacts};

fn value(deps: Vec<Value>) -> Value {
    json!({"rustc":7,"features":"[]","declared_features":"[]","target":11,
        "profile":13,"path":17,"deps":deps,
        "local":[{"CheckDepInfo":{"dep_info":"debug/.fingerprint/unit/dep-lib-x","checksum":false}}],
        "rustflags":[],"config":19,"compile_kind":0})
}
fn parsed(v: Value) -> support::fingerprint::Fingerprint {
    parse(&serde_json::to_vec(&v).unwrap()).unwrap()
}
fn root() -> support::fingerprint::Fingerprint {
    parsed(value(vec![
        json!([1, "lc631_runtime", false, 100]),
        json!([2, "lc631_analysis", false, 200]),
        json!([3, "lc631_substrate", false, 300]),
        json!([4, "serde_json", false, 400]),
    ]))
}
fn candidate(kind: Kind, marker: u64) -> Candidate {
    Candidate {
        kind,
        unit: "0000000000000001".to_owned(),
        marker,
        metadata: parsed(value(vec![])),
    }
}
#[test]
fn s01_api_compatible_shape_with_foreign_fingerprint_is_not_selected() {
    let root = root();
    let stale = candidate(Kind::Runtime, 99);
    assert!(matches!(
        select(&root, Kind::Runtime, std::slice::from_ref(&stale)),
        Err(Error::MissingArtifact(_))
    ));
    let current = candidate(Kind::Runtime, 100);
    assert_eq!(select(&root, Kind::Runtime, &[stale, current]).unwrap(), 1);
}
#[test]
fn s02_duplicate_matching_artifacts_are_ambiguous_not_timestamp_ranked() {
    let a = candidate(Kind::Runtime, 100);
    assert!(matches!(
        select(&root(), Kind::Runtime, &[a.clone(), a]),
        Err(Error::AmbiguousArtifact(_))
    ));
}
#[test]
fn s03_duplicate_dependency_names_and_missing_expected_dependency_fail() {
    let dup = value(vec![
        json!([1, "lc631_runtime", false, 100]),
        json!([1, "lc631_runtime", false, 100]),
    ]);
    assert!(matches!(
        parse(&serde_json::to_vec(&dup).unwrap()),
        Err(Error::DuplicateDependency)
    ));
    let empty = parsed(value(vec![]));
    assert!(matches!(
        select(&empty, Kind::Runtime, &[]),
        Err(Error::MissingDependency(_))
    ));
}
#[test]
fn s04_unknown_json_schema_local_kind_and_duplicate_top_level_keys_fail() {
    let mut unknown = value(vec![]);
    unknown["new_cargo_field"] = json!(true);
    assert!(matches!(
        parse(&serde_json::to_vec(&unknown).unwrap()),
        Err(Error::UnsupportedFormat)
    ));
    let mut checksum = value(vec![]);
    checksum["local"][0]["CheckDepInfo"]["checksum"] = json!(true);
    assert!(matches!(
        parse(&serde_json::to_vec(&checksum).unwrap()),
        Err(Error::UnsupportedFormat)
    ));
    let raw = serde_json::to_string(&value(vec![])).unwrap();
    let duplicate = format!("{{\"rustc\":7,{}", &raw[1..]);
    assert!(matches!(
        parse(duplicate.as_bytes()),
        Err(Error::DuplicateKey)
    ));
}
#[test]
fn s05_marker_is_exact_sixteen_lower_hex_little_endian() {
    assert_eq!(
        hex_fingerprint(b"18fd90117b6bf84f").unwrap(),
        5762473899540413720
    );
    for bad in [
        b"18fd90117b6bf84f\n".as_slice(),
        b"18FD90117B6BF84F",
        b"not-a-fingerprint",
    ] {
        assert!(matches!(
            hex_fingerprint(bad),
            Err(Error::UnsupportedFormat)
        ));
    }
}
#[test]
fn s06_compiler_config_kind_flags_and_runtime_features_mismatch_fail() {
    for field in ["rustc", "config", "compile_kind"] {
        let mut a = candidate(Kind::Runtime, 100);
        let mut v = value(vec![]);
        v[field] = json!(999);
        a.metadata = parsed(v);
        assert!(matches!(
            select(&root(), Kind::Runtime, &[a]),
            Err(Error::ContextMismatch)
        ));
    }
    for field in ["features", "declared_features"] {
        let mut a = candidate(Kind::Runtime, 100);
        let mut v = value(vec![]);
        v[field] = json!("[\"different\"]");
        a.metadata = parsed(v);
        assert!(matches!(
            select(&root(), Kind::Runtime, &[a]),
            Err(Error::ContextMismatch)
        ));
    }
    let mut a = candidate(Kind::Runtime, 100);
    let mut v = value(vec![]);
    v["rustflags"] = json!(["--cfg=stale"]);
    a.metadata = parsed(v);
    assert!(matches!(
        select(&root(), Kind::Runtime, &[a]),
        Err(Error::ContextMismatch)
    ));
}
#[test]
fn s07_profiles_and_features_of_distinct_packages_are_not_assumed_identical() {
    let mut v = value(vec![]);
    v["profile"] = json!(29);
    v["features"] = json!("[\"default\",\"std\"]");
    let mut a = candidate(Kind::Json, 400);
    a.metadata = parsed(v);
    assert_eq!(select(&root(), Kind::Json, &[a]).unwrap(), 0);
}
#[test]
fn s08_transitive_selected_dependency_row_mismatch_is_not_hidden() {
    let mut a = candidate(Kind::Runtime, 100);
    a.metadata = parsed(value(vec![json!([2, "lc631_analysis", false, 201])]));
    assert!(matches!(
        verify_chain(&root(), &[a]),
        Err(Error::DependencyMismatch)
    ));
}
#[test]
fn s09_content_or_source_digest_change_is_not_a_current_receipt() {
    assert!(matches!(
        support::check_digest([1; 32], [2; 32]),
        Err(Error::ArtifactChanged)
    ));
}
#[test]
fn s10_real_current_test_selects_and_seals_exact_linked_artifacts() {
    const _: Option<lc631_runtime::composite_original::OriginalFormula> = None;
    let linked = LinkedArtifacts::current("tlcx22_composite_compile_artifact_selection")
        .expect("current Cargo fingerprint format or exact compiled artifacts unavailable");
    linked
        .verify_current()
        .expect("compiled artifacts or source snapshot changed");
    assert_eq!(linked.externs().count(), 4);
    assert!(linked.deps().ends_with("deps"));
    println!("{}", linked.receipt().expect("bounded artifact receipt"));
}

#[test]
fn s11_empty_foreign_marker_is_rejected_and_diagnostics_are_bounded() {
    use support::diagnostics::{classify_marker, MarkerState, Reason, Rejections};
    assert_eq!(classify_marker(b"", 100).unwrap(), MarkerState::Malformed);
    assert_eq!(
        classify_marker(b"6300000000000000", 100).unwrap(),
        MarkerState::Foreign
    );
    assert_eq!(
        classify_marker(b"6400000000000000", 100).unwrap(),
        MarkerState::Exact
    );
    assert!(matches!(
        hex_fingerprint(b""),
        Err(Error::UnsupportedFormat)
    ));
    let mut rejected = Rejections::default();
    for index in 0..20 {
        rejected
            .record(
                Kind::Runtime,
                &format!("foreign-{index}"),
                Reason::Malformed,
            )
            .unwrap();
    }
    let summary = rejected.summary();
    assert_eq!(summary["malformed"], 20);
    assert_eq!(summary["samples"].as_array().unwrap().len(), 16);
    assert_eq!(summary["unknown_candidates_admitted_as_current"], false);
    assert!(matches!(
        select(&root(), Kind::Runtime, &[]),
        Err(Error::MissingArtifact(_))
    ));
}
#[test]
fn s12_exact_marker_does_not_rescue_unknown_selected_metadata() {
    assert_eq!(
        support::diagnostics::classify_marker(b"6400000000000000", 100).unwrap(),
        support::diagnostics::MarkerState::Exact
    );
    let mut raw = value(vec![]);
    raw["unknown_current_field"] = json!(true);
    let error = support::diagnostics::parse_selected_metadata(
        &serde_json::to_vec(&raw).unwrap(),
        "debug",
        "unit",
        "lib-x",
    )
    .unwrap_err();
    match error {
        Error::AtStage { stage, cause, .. } => {
            assert_eq!(stage, "selected_metadata_json");
            assert!(matches!(cause.as_ref(), Error::UnsupportedFormat));
        }
        other => panic!("wrong selected-metadata failure: {other:?}"),
    }
}
