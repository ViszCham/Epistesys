use lc631_analysis::{build_dgcl_artifact, DgclArtifactError, DgclArtifactRequest};
use lc631_tldg::Dg1Budget;

fn build(
    source: &str,
    grammar: &str,
    profile: &str,
) -> Result<lc631_analysis::DgclArtifact, DgclArtifactError> {
    build_dgcl_artifact(DgclArtifactRequest {
        source,
        grammar_manifest: grammar,
        profile_manifest: profile,
        budget: Dg1Budget::default(),
    })
}

#[test]
fn same_snapshot_and_manifests_produce_stable_artifact_and_requirement_ids() {
    let source = "Please test. Do not publish.";
    let first = build(source, "grammar-ja-en-v1", "profile-alpha2-v1").unwrap();
    let second = build(source, "grammar-ja-en-v1", "profile-alpha2-v1").unwrap();
    assert_eq!(first.artifact_id(), second.artifact_id());
    assert_eq!(first.source_revision(), second.source_revision());
    assert_eq!(first.requirement_ids(), second.requirement_ids());
    assert_eq!(
        first.requirement_ids().len(),
        first.dg1().requirement_candidates.len()
    );
}

#[test]
fn source_grammar_or_profile_change_creates_a_new_identity() {
    let baseline = build("Please test.", "grammar-v1", "profile-v1").unwrap();
    let source_changed = build("Please test safely.", "grammar-v1", "profile-v1").unwrap();
    let grammar_changed = build("Please test.", "grammar-v2", "profile-v1").unwrap();
    let profile_changed = build("Please test.", "grammar-v1", "profile-v2").unwrap();
    assert_ne!(baseline.source_revision(), source_changed.source_revision());
    assert_ne!(baseline.artifact_id(), source_changed.artifact_id());
    assert_ne!(
        baseline.grammar_revision(),
        grammar_changed.grammar_revision()
    );
    assert_ne!(baseline.artifact_id(), grammar_changed.artifact_id());
    assert_ne!(
        baseline.profile_revision(),
        profile_changed.profile_revision()
    );
    assert_ne!(baseline.artifact_id(), profile_changed.artifact_id());
}

#[test]
fn budget_change_is_part_of_the_parse_artifact_identity() {
    let source = "Please test.";
    let standard = build(source, "grammar-v1", "profile-v1").unwrap();
    let bounded = Dg1Budget {
        max_requirements: 1,
        ..Dg1Budget::default()
    };
    let constrained = build_dgcl_artifact(DgclArtifactRequest {
        source,
        grammar_manifest: "grammar-v1",
        profile_manifest: "profile-v1",
        budget: bounded,
    })
    .unwrap();
    assert_eq!(standard.source_revision(), constrained.source_revision());
    assert_ne!(standard.budget_digest(), constrained.budget_digest());
    assert_ne!(standard.artifact_id(), constrained.artifact_id());
}

#[test]
fn requirement_identity_is_artifact_bound_and_spans_roundtrip_utf8() {
    let source = "公開しないでください。\nPlease test.";
    let artifact = build(source, "grammar-ja-en-v1", "profile-alpha2-v1").unwrap();
    assert!(!artifact.requirement_ids().is_empty());
    for (candidate, identity) in artifact
        .dg1()
        .requirement_candidates
        .iter()
        .zip(artifact.requirement_ids())
    {
        assert!(source.is_char_boundary(candidate.source_span.start));
        assert!(source.is_char_boundary(candidate.source_span.end));
        assert_eq!(
            source.get(candidate.source_span.clone()),
            Some(candidate.source_text.as_str())
        );
        assert_eq!(identity.artifact_id(), artifact.artifact_id());
    }
}

#[test]
fn invalid_manifest_or_source_budget_fails_without_partial_artifact() {
    assert!(matches!(
        build("x", "", "profile"),
        Err(DgclArtifactError::ManifestInvalid)
    ));
    let oversized = "x".repeat(300_000);
    assert!(matches!(
        build(&oversized, "grammar", "profile"),
        Err(DgclArtifactError::Parse(_))
    ));
}
