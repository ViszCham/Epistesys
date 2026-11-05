//! 日本語: 候補の同一性と、各測定器のprovenanceを分離する借用view。
//! English: Borrowed view separating candidate identity from per-metric provenance.
//! This validates reported correspondence only, not authenticity, freshness,
//! semantic truth, independent corroboration, permission or completion.
use super::{
    AssessmentPolicy, ComponentMetric, ComponentReport, MeasurementBinding, MeasurementError,
};
use lc631_substrate::Digest32;

/// Candidate-specific identity shared by measurements of ONE representation.
/// Source/target are not discarded: heterogeneous representations need a
/// separately checked correspondence and cannot enter through this constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateSubject {
    pub scope: Digest32,
    pub candidate: Digest32,
    pub source: Digest32,
    pub target: Digest32,
    pub context: Digest32,
    pub baseline: Digest32,
}

impl CandidateSubject {
    pub const fn from_binding(binding: MeasurementBinding) -> Self {
        Self {
            scope: binding.scope,
            candidate: binding.candidate,
            source: binding.source,
            target: binding.target,
            context: binding.context,
            baseline: binding.baseline,
        }
    }
}

/// No allocation, clone, synthesized common producer or rewritten report.
/// All full bindings and uncertainty/basis/validation fields remain borrowed.
pub struct ComponentAssessment<'a> {
    subject: CandidateSubject,
    reports: &'a [&'a ComponentReport],
}

impl<'a> ComponentAssessment<'a> {
    pub fn from_reports(
        policy: &AssessmentPolicy<'_>,
        subject: CandidateSubject,
        reports: &'a [&'a ComponentReport],
    ) -> Result<Self, MeasurementError> {
        if reports.is_empty()
            || reports.len() > 29
            || policy.required.is_empty()
            || policy.required.len() > 29
        {
            return Err(MeasurementError::InvalidInput);
        }
        if subject.scope != policy.scope || subject.baseline != policy.baseline {
            return Err(MeasurementError::ScopeMismatch);
        }
        for (index, report) in reports.iter().enumerate() {
            if CandidateSubject::from_binding(report.binding) != subject {
                return Err(MeasurementError::ScopeMismatch);
            }
            if reports[..index].iter().any(|r| r.metric == report.metric) {
                return Err(MeasurementError::DuplicateMetric);
            }
        }
        for (index, metric) in policy.required.iter().enumerate() {
            if policy.required[..index].contains(metric) {
                return Err(MeasurementError::DuplicateMetric);
            }
            if !reports.iter().any(|report| report.metric == *metric) {
                return Err(MeasurementError::MissingMetric);
            }
        }
        Ok(Self { subject, reports })
    }

    pub const fn subject(&self) -> CandidateSubject {
        self.subject
    }

    pub fn reports(&self) -> &'a [&'a ComponentReport] {
        self.reports
    }

    pub fn get(&self, metric: ComponentMetric) -> Option<&'a ComponentReport> {
        self.reports
            .iter()
            .copied()
            .find(|report| report.metric == metric)
    }
}
