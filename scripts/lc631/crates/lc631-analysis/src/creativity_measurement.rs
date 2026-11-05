//! Preparatory finite-formal-v1.4 component measurement adapter.
//! Arithmetic, reported observations, semantic validation and authority remain separate.
//! This module does not issue evidence, permits or completion claims.
mod component_assessment;
pub mod formula;
mod leaves;
mod numeric;
mod structure;

pub use component_assessment::{CandidateSubject, ComponentAssessment};
pub use leaves::{paired_gain, weighted_assessment, ProbePair, WeightedProbe};
pub use numeric::{Rational, RationalInterval, MAX_RATIONAL_BITS};
pub use structure::{
    complexity_match, compression_gain, knownness, set_distance, Feature, PreservationPrerequisite,
};

use lc631_substrate::Digest32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeasurementError {
    DivisionByZero,
    ReversedInterval,
    NumericBudget,
    InvalidInput,
    EmptyDenominator,
    ScopeMismatch,
    DuplicateProbe,
    DuplicateMetric,
    MissingMetric,
    PrerequisitePending,
    PrerequisiteFailed,
    OracleMismatch,
    ProbeSetMismatch,
    AllocationFailed,
    UniverseMismatch,
    EmptyCorpus,
}

/// Fixed original component identifiers; numeric equality does not merge their meanings.
#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ComponentMetric {
    ContextDistance = 0x010b,
    FrameworkDistance = 0x010c,
    SemanticConfigurationDistance = 0x010d,
    Compatibility = 0x010e,
    ComplexityMatch = 0x010f,
    Knownness = 0x0110,
    NoveltyCompatibility = 0x0111,
    Coverage = 0x0112,
    Compression = 0x0113,
    Bridge = 0x0114,
    Prediction = 0x0115,
    Redundancy = 0x0116,
    StructuralGain = 0x0117,
    Preserve = 0x0118,
    Rebind = 0x0119,
    Reframe = 0x011a,
    BreakCost = 0x011b,
    ReorganizationGain = 0x011c,
    Explain = 0x011d,
    Design = 0x011e,
    Transfer = 0x011f,
    Solve = 0x0120,
    UtilityGain = 0x0121,
    DescriptionBits = 0x0122,
    ComplexityParity = 0x0123,
    CounterexampleRisk = 0x0124,
    BridgeCost = 0x0125,
    OriginalV1 = 0x0126,
    OriginalV2 = 0x0127,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeasurementBinding {
    pub scope: Digest32,
    pub candidate: Digest32,
    pub source: Digest32,
    pub target: Digest32,
    pub context: Digest32,
    pub baseline: Digest32,
    pub producer: Digest32,
    pub transform: Digest32,
    pub result: Digest32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputBasis {
    Given,
    Assumed,
    Observed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticValidation {
    Unassessed,
    SolverReported,
    ModelRechecked,
    ProofChecked,
    FiniteExhaustive,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndependenceStatus {
    Unassessed,
    SharedDependency,
    ReportedSeparation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeasurementGap {
    MissingObservation,
    MissingEvaluator,
    Unsupported,
    Timeout,
    Conflict,
    Uncalibrated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComponentState {
    Measured(RationalInterval),
    Estimated {
        bounds: RationalInterval,
        method: Digest32,
    },
    Unknown(MeasurementGap),
    NotApplicable {
        basis: Digest32,
    },
    Incomparable {
        scope: Digest32,
    },
    Undefined(MeasurementError),
    InvalidConfig(MeasurementError),
    ResourceExhausted {
        partial: Option<RationalInterval>,
    },
}

/// Reported metadata is retained verbatim. A digest or enum is not authenticated evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentReport {
    pub metric: ComponentMetric,
    pub definition: Digest32,
    pub binding: MeasurementBinding,
    pub probe_set: Digest32,
    pub oracle: Digest32,
    pub dependency_group: Digest32,
    pub evidence_payload: Digest32,
    pub basis: InputBasis,
    pub validation: SemanticValidation,
    pub independence: IndependenceStatus,
    pub state: ComponentState,
}

/// Fixed applicability is owned by the comparison, never reconstructed from one candidate.
pub struct AssessmentPolicy<'a> {
    pub scope: Digest32,
    pub baseline: Digest32,
    pub required: &'a [ComponentMetric],
}

pub struct CreativeAssessment<'a> {
    binding: MeasurementBinding,
    reports: &'a [ComponentReport],
}

impl<'a> CreativeAssessment<'a> {
    pub fn from_reports(
        policy: &AssessmentPolicy<'_>,
        reports: &'a [ComponentReport],
    ) -> Result<Self, MeasurementError> {
        if reports.is_empty() || reports.len() > 29 || policy.required.is_empty() {
            return Err(MeasurementError::InvalidInput);
        }
        let binding = reports[0].binding;
        if binding.scope != policy.scope || binding.baseline != policy.baseline {
            return Err(MeasurementError::ScopeMismatch);
        }
        for (i, report) in reports.iter().enumerate() {
            if report.binding != binding {
                return Err(MeasurementError::ScopeMismatch);
            }
            if reports[..i].iter().any(|r| r.metric == report.metric) {
                return Err(MeasurementError::DuplicateMetric);
            }
        }
        for (i, metric) in policy.required.iter().enumerate() {
            if policy.required[..i].contains(metric) {
                return Err(MeasurementError::DuplicateMetric);
            }
            if !reports.iter().any(|r| r.metric == *metric) {
                return Err(MeasurementError::MissingMetric);
            }
        }
        Ok(Self { binding, reports })
    }

    pub const fn binding(&self) -> MeasurementBinding {
        self.binding
    }
    pub fn reports(&self) -> &'a [ComponentReport] {
        self.reports
    }
    pub fn get(&self, metric: ComponentMetric) -> Option<&'a ComponentReport> {
        self.reports.iter().find(|report| report.metric == metric)
    }
}
