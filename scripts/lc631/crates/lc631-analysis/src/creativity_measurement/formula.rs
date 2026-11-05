//! Original F1/F2 arithmetic over scope-bound component measurements.
//! No oracle execution, independent validation, calibration or permission is inferred.
// Retain the complete unknown state or partial numeric enclosure inline.
// An additional Box solely for error size would add another allocation on the
// failure path. This exception is limited to the new formula-evaluator module;
// it is not a claim that BigRational cloning or total RSS is allocation-free.
#![allow(clippy::result_large_err)]
mod evaluate;
mod profile;
mod range;

pub use evaluate::{evaluate_components, evaluate_original, evaluate_original_pair};
pub use profile::{CoefficientSet, FormulaBinding, FormulaProfile, ScaleParameters};
pub use range::DeclaredFormulaRange;

use super::{
    CandidateSubject, ComponentAssessment, ComponentMetric, ComponentState, CreativeAssessment,
    MeasurementBinding, RationalInterval,
};
use lc631_substrate::{measurement_numeric::NumericError, Digest32};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OriginalFormula {
    F1V1,
    F2V1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormulaError {
    Numeric(NumericError),
    ScopeMismatch,
    BaselineMismatch,
    DefinitionMismatch(ComponentMetric),
    MissingMetric(ComponentMetric),
    MissingDefinition(ComponentMetric),
    DuplicateDefinition,
    InvalidParameter,
    DegenerateWeights,
    DeclaredRangeViolation(ComponentMetric),
    AllocationFailed,
    NoScalar {
        metric: Option<ComponentMetric>,
        retained: ComponentState,
    },
    ResourceExhausted {
        partial: Option<RationalInterval>,
    },
}
impl From<NumericError> for FormulaError {
    fn from(error: NumericError) -> Self {
        Self::Numeric(error)
    }
}

/// TL-TOTAL is a distinct measurement, never creative D_sem or a second penalty.
pub struct TranslationLossInput<'a> {
    pub binding: MeasurementBinding,
    pub reduction_policy: Digest32,
    pub state: &'a ComponentState,
}

pub struct FormulaInput<'a> {
    pub assessment: &'a CreativeAssessment<'a>,
    pub translation_loss: TranslationLossInput<'a>,
}

/// Additive input for metrics from distinct producers, with no synthetic shared
/// MeasurementBinding. The legacy FormulaInput remains strict and unchanged.
pub struct ComponentFormulaInput<'a> {
    pub assessment: &'a ComponentAssessment<'a>,
    pub translation_loss: TranslationLossInput<'a>,
}

/// Arithmetic plus the exact borrowed input lineage. The TL binding remains
/// separate from every component binding. This is not an owned current receipt.
pub struct ComponentEvaluation<'a> {
    pub subject: CandidateSubject,
    pub input: &'a ComponentFormulaInput<'a>,
    pub arithmetic: OriginalArithmetic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginalArithmetic {
    pub formula: OriginalFormula,
    pub profile_digest: Digest32,
    pub raw: RationalInterval,
    pub declared_range: DeclaredFormulaRange,
    pub derived: DerivedComponents,
    pub includes_estimates: bool,
    pub preference_basis: PreferenceBasis,
    pub degenerate_weights: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreferenceBasis {
    DeclaredUncalibrated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedComponents {
    pub complexity_match: Option<RationalInterval>,
    pub novelty_compatibility: Option<RationalInterval>,
    pub structural_gain: Option<RationalInterval>,
    pub reorganization_gain: Option<RationalInterval>,
    pub utility_gain: Option<RationalInterval>,
    pub complexity_parity: Option<RationalInterval>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginalEvaluation {
    pub formula: OriginalFormula,
    pub profile_digest: Digest32,
    pub binding: MeasurementBinding,
    pub raw: RationalInterval,
    /// Coefficient-derived algebraic enclosure, not calibration or observation.
    pub declared_range: DeclaredFormulaRange,
    pub derived: DerivedComponents,
    pub includes_estimates: bool,
    pub preference_basis: PreferenceBasis,
    pub degenerate_weights: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginalPairEvaluation {
    pub candidate: OriginalEvaluation,
    pub baseline: OriginalEvaluation,
    pub delta: RationalInterval,
    /// Self-difference keeps the uncertain raw values rather than erasing them.
    pub same_input_nodes: bool,
}
