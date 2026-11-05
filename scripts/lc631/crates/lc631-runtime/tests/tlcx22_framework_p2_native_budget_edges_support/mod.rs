//! Literal finite oracles only; no production encoder/evaluator creates gold.
use super::*;

pub fn signature(current: &m::Current<'_>) {
    point(
        current.report(ComponentMetric::FrameworkDistance).unwrap(),
        1,
        4,
    );
    assert_eq!(current.signature_union_weight(), &q(8, 1));
    assert_eq!(current.signature_positive_rows(), 8);
    assert_eq!(current.signature_role_counts(), &[[1u16; 10]; 2]);
    assert_eq!(current.signature_payload_lengths(), [61, 124, 296]);
    assert_eq!(
        current.signature_payload(f::Package::C),
        Some(COMMON_SIGNATURE)
    );
    assert_eq!(
        current.signature_payload(f::Package::Kc),
        Some(CLASSICAL_CLOSURE)
    );
    assert_eq!(
        current.signature_payload(f::Package::Ki),
        Some(PERSISTENT_CLOSURE)
    );
    let report = current.report(ComponentMetric::Knownness).unwrap();
    assert_eq!(report.basis, InputBasis::Given);
    assert_eq!(report.validation, SemanticValidation::Unassessed);
    assert_eq!(report.independence, IndependenceStatus::SharedDependency);
}

pub fn partial(current: &m::Current<'_>, numerator: i64, denominator: i64) {
    assert_eq!(current.archive_status(), m::ArchiveStatus::Partial);
    assert_eq!(
        current.report(ComponentMetric::Knownness).unwrap().state,
        ComponentState::ResourceExhausted {
            partial: Some(RationalInterval::from_ratio(numerator, denominator, 1, 1).unwrap())
        }
    );
}

pub fn no_completed_comparison(current: &m::Current<'_>) {
    assert_eq!(current.counts().archive_comparisons, 0);
    assert!(
        current.knownness_union_weights().is_empty(),
        "an unfinished native artifact must not publish a completed K3 union"
    );
}
