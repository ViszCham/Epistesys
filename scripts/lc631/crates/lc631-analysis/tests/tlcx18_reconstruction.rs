use lc631_analysis::reconstruction_loss;

use lc631_substrate::Digest32;
use reconstruction_loss::*;

fn digest(byte: u8) -> Digest32 {
    Digest32::from_bytes([byte; 32])
}

#[test]
fn tlcx18_target_only_decoder_preserves_missing_feature_as_gap() {
    let decoder = TargetDecoder::new(digest(1), &[1, 0]).unwrap();
    let target = TargetPayload::new(digest(2), &[Some(true), None]).unwrap();
    let decoded = decoder.decode(&target).unwrap();
    let plan = ReconstructionPlan::new(digest(3), digest(1), &[true, false]).unwrap();
    let report = plan.compare(&decoded).unwrap();
    assert_eq!(
        report.outcomes(),
        &[ProbeOutcome::Unknown, ProbeOutcome::Violated]
    );
    assert_eq!(report.basis(), InputValidation::ReportedInputs);
    assert_eq!(report.decoder_revision(), digest(1));
    assert_eq!(report.target_revision(), digest(2));
    assert_eq!(report.probe_set(), digest(3));
}

#[test]
fn tlcx18_probe_count_cannot_shrink_per_candidate() {
    let plan = ReconstructionPlan::new(digest(3), digest(1), &[true, false]).unwrap();
    let decoder = TargetDecoder::new(digest(1), &[0]).unwrap();
    let target = TargetPayload::new(digest(2), &[Some(true)]).unwrap();
    assert_eq!(
        plan.compare(&decoder.decode(&target).unwrap()).unwrap_err(),
        LossError::ProbeSetChanged
    );
    assert_eq!(
        TargetDecoder::new(digest(1), &[]).unwrap_err(),
        LossError::EmptyProbeSet
    );
}

#[test]
fn tlcx18_decoder_cannot_lookup_unavailable_target_slot() {
    let decoder = TargetDecoder::new(digest(1), &[1]).unwrap();
    let target = TargetPayload::new(digest(2), &[Some(true)]).unwrap();
    assert_eq!(
        decoder.decode(&target).unwrap_err(),
        LossError::TargetSlotMissing
    );
}

#[test]
fn tlcx18_decoder_revision_change_requires_remeasurement() {
    let plan = ReconstructionPlan::new(digest(3), digest(1), &[true]).unwrap();
    let changed = TargetDecoder::new(digest(8), &[0]).unwrap();
    let target = TargetPayload::new(digest(2), &[Some(true)]).unwrap();
    assert_eq!(
        plan.compare(&changed.decode(&target).unwrap()).unwrap_err(),
        LossError::DecoderChanged
    );
}

#[test]
fn tlcx18_semantic_soundness_and_completeness_have_opposite_directions() {
    // Source predicate is true only at row 1; target predicate is a tautology.
    let source = FiniteTruthSet::new(digest(4), 1, &[false, true]).unwrap();
    let target = FiniteTruthSet::new(digest(4), 1, &[true, true]).unwrap();
    assert_eq!(
        compare_semantics(&source, &target, Preservation::Soundness)
            .unwrap()
            .outcome(),
        ProbeOutcome::Violated
    );
    assert_eq!(
        compare_semantics(&source, &target, Preservation::Completeness)
            .unwrap()
            .outcome(),
        ProbeOutcome::Preserved
    );
    assert_eq!(
        compare_semantics(&source, &target, Preservation::Equivalence)
            .unwrap()
            .counterexample(),
        Some(0)
    );
}

#[test]
fn tlcx18_semantics_rejects_scope_change_and_empty_context() {
    let source = FiniteTruthSet::new(digest(4), 1, &[false, true]).unwrap();
    let wrong = FiniteTruthSet::new(digest(5), 1, &[false, true]).unwrap();
    assert_eq!(
        compare_semantics(&source, &wrong, Preservation::Equivalence),
        Err(LossError::Incomparable)
    );
    assert_eq!(
        FiniteTruthSet::new(digest(4), 1, &[true]).unwrap_err(),
        LossError::TruthTableLength
    );
    let context = FiniteTruthSet::new(digest(4), 1, &[false, false]).unwrap();
    assert_eq!(
        compare_in_context(&context, &source, &source, Preservation::Equivalence),
        Err(LossError::ContextInconsistent)
    );
}

#[test]
fn tlcx18_negation_mutation_changes_probe_but_never_validates_shared_parser() {
    let source = FiniteTruthSet::new(digest(4), 1, &[false, true]).unwrap();
    let reversed = FiniteTruthSet::new(digest(4), 1, &[true, false]).unwrap();
    let pass = compare_semantics(&source, &source, Preservation::Equivalence).unwrap();
    let fail = compare_semantics(&source, &reversed, Preservation::Equivalence).unwrap();
    assert_eq!(pass.outcome(), ProbeOutcome::Preserved);
    assert_eq!(fail.outcome(), ProbeOutcome::Violated);
    assert_eq!(pass.basis(), InputValidation::ReportedInputs);
    assert_eq!(fail.basis(), InputValidation::ReportedInputs);
    assert_eq!(fail.preservation(), Preservation::Equivalence);
    assert_eq!(fail.signature(), digest(4));
}
