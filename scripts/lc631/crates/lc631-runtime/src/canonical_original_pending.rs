//! 日本語: 実canonical ownerから原式入力不足を保持する。総合TLを捏造しない。
//! English: Preserve missing original-score inputs from actual canonical ownership.
//! This snapshot is not F1/F2 evaluation, a verified measurement, or authority.
//! The normal canonical request has neither an Original FormulaProfile nor a
//! checked temporal/spatial tuple reduction. A Boolean semantic axis is not TL-TOTAL.
#![forbid(unsafe_code)]
use crate::canonical_solve::{
    AdmissionError, BooleanEvaluationState, NativeEvidenceKind, OwnedCanonicalDecision,
};
use crate::resource::ResourceBroker;
use lc631_core::{measurement_numeric::RationalInterval, CandidateId};
use lc631_substrate::{resource::ResourceReservationView, Digest32};

/// Exact bounded fact-only text for insertion INSIDE canonical rendering. Never
/// append it to already-bound bytes or infer coefficients/measurements from it.
pub const ENGLISH_LINE: &str =
    "original F1/F2: NoScalar; profile: Missing; total TL owner: Missing; recommendation: None";
pub const JAPANESE_LINE: &str =
    "原式F1/F2: 採点不能; profile: 未設定; 総合TL owner: 不足; 推奨: なし";

/// A historical observation after an actual current-owner check, not a live
/// commit fence. Fields are private; no report/boolean/scalar input constructor,
/// Clone, Deserialize, eligibility conversion or execution-permit API exists.
pub struct Observation {
    source: Digest32,
    native: NativeEvidenceKind,
    semantic: Option<BooleanEvaluationState>,
    candidates: usize,
}
impl Observation {
    pub const fn source_revision(&self) -> Digest32 {
        self.source
    }
    pub const fn native_kind(&self) -> NativeEvidenceKind {
        self.native
    }
    /// Actual finite-Boolean receipt state, not an Original-score component.
    pub const fn boolean_measurement_state(&self) -> Option<BooleanEvaluationState> {
        self.semantic
    }
    pub const fn canonical_candidate_count(&self) -> usize {
        self.candidates
    }
    pub const fn profile_missing(&self) -> bool {
        true
    }
    pub const fn total_tl_owner_missing(&self) -> bool {
        true
    }
    pub fn scalar(&self) -> Option<&RationalInterval> {
        None
    }
    /// Original-score recommendation only; does not overwrite another policy's
    /// selection or turn a reported goal into an OriginalScore objective.
    pub const fn recommendation(&self) -> Option<CandidateId> {
        None
    }
    pub const fn formula_evaluated(&self) -> bool {
        false
    }
    pub const fn english_line(&self) -> &'static str {
        ENGLISH_LINE
    }
    pub const fn japanese_line(&self) -> &'static str {
        JAPANESE_LINE
    }
}

/// Consume the SAME actual canonical decision and its owning source-bound view;
/// do not reparse, rerun a solver, create a tuple, or manufacture coefficients.
/// Every use must call this again after cancellation/lease/authority changes;
/// this local resource check does not establish host authority or revocation.
pub fn inspect_current(
    decision: &OwnedCanonicalDecision<'_, '_>,
    broker: &ResourceBroker,
    view: &ResourceReservationView<'_>,
) -> Result<Observation, AdmissionError> {
    decision.project_current(broker, view)?;
    let observation = Observation {
        source: view.source_revision(),
        native: decision.native_kind(),
        semantic: decision.boolean_measurement().map(|owner| owner.state()),
        candidates: decision
            .selection_summary()
            .map_or(0, |selection| selection.candidate_count),
    };
    // Keep current validation on both sides of the snapshot, without claiming
    // an atomic cancellation/publication fence or renewal of historical bytes.
    decision.project_current(broker, view)?;
    Ok(observation)
}
