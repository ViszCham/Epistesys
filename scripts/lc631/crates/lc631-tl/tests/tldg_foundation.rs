#![forbid(unsafe_code)]

use lc631_core::{ObligationId, SourceSpan};
use lc631_tl::{
    CorrespondenceWitnessV2, MeasurementRegime, MeasurementRegimeSet, Obligation, ObligationKind,
    ObligationPolarity, ObligationStrength, PreservationState, ProjectionDefectGraph,
    ProjectionGateDecision, SourceAnchor, TargetAnchor, VerificationReceipt, VerificationStatus,
    VerifierCapability, VerifierKind,
};

fn obligation(source: &str, id: u64, kind: ObligationKind) -> Obligation {
    Obligation {
        id: ObligationId(id),
        kind,
        strength: ObligationStrength::Must,
        polarity: ObligationPolarity::Positive,
        scope: "answer".into(),
        source_span: SourceSpan::checked(source, 0, source.len()).unwrap(),
        source_text: source.into(),
    }
}

#[test]
fn tldg_00_02_source_anchor_is_revision_and_span_bound() {
    let source = "日本語 + Rust";
    let anchor = SourceAnchor::checked("source-r1", source, 0, source.len()).unwrap();
    assert_eq!(anchor.span.slice(source).unwrap(), source);
    assert!(SourceAnchor::checked("source-r1", source, 1, source.len()).is_err());
}

#[test]
fn tldg_03_many_target_correspondence_is_preserved() {
    let source = "answer both parts";
    let mut graph = ProjectionDefectGraph::default();
    graph
        .add_obligation(
            source,
            obligation(source, 1, ObligationKind::OutputContract),
        )
        .unwrap();
    graph
        .add_correspondence(CorrespondenceWitnessV2 {
            obligation_id: ObligationId(1),
            source: vec![SourceAnchor::checked("seed-r1", source, 0, source.len()).unwrap()],
            target: vec![
                TargetAnchor {
                    artifact: "output".into(),
                    path: "answers/0".into(),
                    span: None,
                },
                TargetAnchor {
                    artifact: "output".into(),
                    path: "answers/1".into(),
                    span: None,
                },
            ],
            state: PreservationState::Refinement,
            rule_id: "split-answer.v1".into(),
            verifier: VerificationReceipt {
                verifier: VerifierKind::Schema,
                status: VerificationStatus::Passed,
                revision: "schema-r1".into(),
                evidence_digest: Some("sha256:evidence".into()),
            },
            counterexample: None,
        })
        .unwrap();
    assert_eq!(graph.correspondences().count(), 1);
}

#[test]
fn tldg_03_introduced_without_source_is_a_defect_not_an_invalid_record() {
    let source = "answer";
    let mut graph = ProjectionDefectGraph::default();
    graph
        .add_obligation(source, obligation(source, 2, ObligationKind::Semantic))
        .unwrap();
    graph
        .add_correspondence(CorrespondenceWitnessV2 {
            obligation_id: ObligationId(2),
            source: Vec::new(),
            target: vec![TargetAnchor {
                artifact: "candidate".into(),
                path: "claims/extra".into(),
                span: None,
            }],
            state: PreservationState::IntroducedWithoutSource,
            rule_id: "candidate-extra.v1".into(),
            verifier: VerificationReceipt {
                verifier: VerifierKind::Schema,
                status: VerificationStatus::Failed,
                revision: "schema-r1".into(),
                evidence_digest: Some("sha256:extra".into()),
            },
            counterexample: None,
        })
        .unwrap();
    assert_eq!(graph.gate(), ProjectionGateDecision::HardHold);
}

#[test]
fn tldg_04_exact_requires_obligation_specific_capability() {
    let source = "preserve temporal order";
    let mut graph = ProjectionDefectGraph::default();
    graph
        .add_obligation(source, obligation(source, 3, ObligationKind::TemporalFrame))
        .unwrap();
    graph
        .register_capability(VerifierCapability {
            verifier: VerifierKind::SourceSpan,
            obligation_kind: ObligationKind::Structural,
            exact_allowed: true,
            revision: "span-r1".into(),
        })
        .unwrap();
    let result = graph.add_correspondence(CorrespondenceWitnessV2 {
        obligation_id: ObligationId(3),
        source: vec![SourceAnchor::checked("seed-r1", source, 0, source.len()).unwrap()],
        target: vec![TargetAnchor {
            artifact: "program".into(),
            path: "temporal/0".into(),
            span: None,
        }],
        state: PreservationState::Exact,
        rule_id: "temporal.v1".into(),
        verifier: VerificationReceipt {
            verifier: VerifierKind::SourceSpan,
            status: VerificationStatus::Passed,
            revision: "span-r1".into(),
            evidence_digest: Some("sha256:span".into()),
        },
        counterexample: None,
    });
    assert!(result.is_err());
}

#[test]
fn tldg_05_regimes_are_multilabel_selectors_not_scores() {
    let regimes = MeasurementRegimeSet::from_iter([
        MeasurementRegime::NaturalLanguage,
        MeasurementRegime::Program,
        MeasurementRegime::GeneralAction,
    ]);
    assert!(regimes.contains(MeasurementRegime::NaturalLanguage));
    assert!(regimes.contains(MeasurementRegime::Program));
    assert_eq!(regimes.len(), 3);
    assert!(!regimes.can_authorize());
}
