//! DGCLの実planとtyped Coding義務の借用接続。条件truth・権限は別検証。
//! Borrowed connection between actual DGCL plans and typed coding obligations.

/// Current-context inputs and direct connections to existing validation producers.
pub mod current;

use super::{
    task_validation_route, ConditionBridgeError, ConditionObservation, FiniteSupport, ReportCheck,
    SourceConditionState,
};
use crate::{
    context_lowering::LoweredContext,
    logical_lowering::{
        lower_dgcl_logical, FormulaId, LogicProfile, LogicalLoweringBudget, LogicalLoweringError,
        LogicalTask, TaskObligation, TaskObligationApplicability, TaskObligationKind,
    },
    verify_dgcl_pipeline_identity, CodingEvidenceKind, CompletionPlanStatus, DgclPipelineReport,
    PlannedCodingRequirement, PlannedEvidenceTask,
};
use lc631_core::context::TaskKind;

const MAX_BINDING_OBLIGATIONS: usize = 8192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodingInputError {
    PipelineIdentity,
    WrongTaskKind,
    TaskIdentity,
    Lowering(LogicalLoweringError),
    TaskRoute(ConditionBridgeError),
    MissingRequirement,
    ObligationMismatch,
    ConditionMismatch,
    ForeignConditionObservation,
    BudgetExceeded,
}

/// Own the actual existing lowering while borrowing its sealed DGCL input.
/// Neither this container nor a bound requirement is an execution permit.
pub struct DgclCodingInput<'report> {
    report: &'report DgclPipelineReport,
    logical: LogicalTask,
}

impl<'report> DgclCodingInput<'report> {
    pub fn lower(
        report: &'report DgclPipelineReport,
        context: &LoweredContext,
        budget: LogicalLoweringBudget,
    ) -> Result<Self, CodingInputError> {
        if !verify_dgcl_pipeline_identity(report.source_store().text(), report) {
            return Err(CodingInputError::PipelineIdentity);
        }
        let task = lower_dgcl_logical(
            report,
            context,
            TaskKind::Coding,
            LogicProfile::ClassicalPropositionalV1,
            budget,
        )
        .map_err(CodingInputError::Lowering)?;
        Self::from_lowered(report, task)
    }

    pub fn from_lowered(
        report: &'report DgclPipelineReport,
        logical: LogicalTask,
    ) -> Result<Self, CodingInputError> {
        if !verify_dgcl_pipeline_identity(report.source_store().text(), report) {
            return Err(CodingInputError::PipelineIdentity);
        }
        if logical.task_kind() != TaskKind::Coding {
            return Err(CodingInputError::WrongTaskKind);
        }
        if logical.source_store().id() != report.source_store().id()
            || logical.source_store().text() != report.source_store().text()
            || logical.program_ir().legacy_program_digest() != report.program_ir.program_digest
            || logical.program_ir().parser_grammar_revision() != report.program_ir.grammar_revision
        {
            return Err(CodingInputError::TaskIdentity);
        }
        if logical.obligations().len() > MAX_BINDING_OBLIGATIONS {
            return Err(CodingInputError::BudgetExceeded);
        }
        task_validation_route(&logical).map_err(CodingInputError::TaskRoute)?;
        Ok(Self { report, logical })
    }

    pub fn logical_task(&self) -> &LogicalTask {
        &self.logical
    }
    pub fn pipeline(&self) -> &DgclPipelineReport {
        self.report
    }

    /// Bind an explicit caller-selected requirement. Its index is only an
    /// address in the immutable plan, never a recommendation/ranking score.
    pub fn bind_requirement(
        &self,
        index: usize,
    ) -> Result<BoundCodingRequirement<'_, 'report>, CodingInputError> {
        let requirement = self
            .report
            .completion_plan
            .requirements
            .get(index)
            .ok_or(CodingInputError::MissingRequirement)?;
        if requirement.tasks.len() != 5 {
            return Err(CodingInputError::ObligationMismatch);
        }
        let mut first: Option<&TaskObligation> = None;
        for planned in &requirement.tasks {
            let mut found = self.logical.obligations().iter().filter(|obligation| {
                obligation.span().start() == requirement.source_span.start
                    && obligation.span().end() == requirement.source_span.end
                    && obligation.kind() == typed_kind(planned.evidence_kind)
            });
            let obligation = found.next().ok_or(CodingInputError::ObligationMismatch)?;
            if found.next().is_some() {
                return Err(CodingInputError::ObligationMismatch);
            }
            if let Some(anchor) = first {
                if anchor.target() != obligation.target()
                    || anchor.condition_indices() != obligation.condition_indices()
                {
                    return Err(CodingInputError::ObligationMismatch);
                }
            } else {
                first = Some(obligation);
            }
        }
        let anchor = first.ok_or(CodingInputError::ObligationMismatch)?;
        if anchor.condition_indices().len() != requirement.conditions.len() {
            return Err(CodingInputError::ConditionMismatch);
        }
        for (index, planned) in anchor
            .condition_indices()
            .iter()
            .zip(&requirement.conditions)
        {
            let actual = self
                .logical
                .conditions()
                .get(*index)
                .ok_or(CodingInputError::ConditionMismatch)?;
            if actual.span().start() != planned.source_span.start
                || actual.span().end() != planned.source_span.end
            {
                return Err(CodingInputError::ConditionMismatch);
            }
        }
        Ok(BoundCodingRequirement {
            input: self,
            requirement,
            anchor,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodingInputReadiness {
    ReadyForEvidence,
    NeedsConditionEvidence,
    NeedsInterpretation,
    PlanPending,
}

/// Borrowed producer inputs preserve original applicability, target, revision,
/// and acceptance criteria. Calling producers still needs fresh authority.
pub struct BoundCodingRequirement<'input, 'report> {
    input: &'input DgclCodingInput<'report>,
    requirement: &'report PlannedCodingRequirement,
    anchor: &'input TaskObligation,
}

impl BoundCodingRequirement<'_, '_> {
    pub fn requirement(&self) -> &PlannedCodingRequirement {
        self.requirement
    }
    pub fn planned_tasks(&self) -> &[PlannedEvidenceTask] {
        &self.requirement.tasks
    }
    pub fn target(&self) -> FormulaId {
        self.anchor.target()
    }
    pub fn condition_indices(&self) -> &[usize] {
        self.anchor.condition_indices()
    }

    pub fn readiness(&self) -> CodingInputReadiness {
        if !self.anchor.condition_indices().is_empty() {
            CodingInputReadiness::NeedsConditionEvidence
        } else if self.anchor.needs_interpretation()
            || self.anchor.applicability() != TaskObligationApplicability::Required
        {
            CodingInputReadiness::NeedsInterpretation
        } else if !self.requirement.ready_for_evidence
            || self.input.report.completion_plan.status != CompletionPlanStatus::ReadyForEvidence
        {
            CodingInputReadiness::PlanPending
        } else {
            CodingInputReadiness::ReadyForEvidence
        }
    }

    /// Consume a finite observation only for this owned task and an associated
    /// condition. Model support cannot rewrite legacy condition applicability.
    pub fn review_condition(
        &self,
        observation: &ConditionObservation<'_, '_>,
    ) -> Result<BoundConditionReview, CodingInputError> {
        let request = observation.request();
        if !std::ptr::eq(request.task(), self.input.logical_task())
            || !self
                .anchor
                .condition_indices()
                .contains(&request.condition_index())
        {
            return Err(CodingInputError::ForeignConditionObservation);
        }
        Ok(BoundConditionReview {
            model_support: observation.support(),
            report_check: observation.report_relation(),
            source_state: observation.source_condition_state(),
            validation_readiness: self.readiness(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundConditionReview {
    pub model_support: FiniteSupport,
    pub report_check: ReportCheck,
    pub source_state: SourceConditionState,
    pub validation_readiness: CodingInputReadiness,
}

fn typed_kind(kind: CodingEvidenceKind) -> TaskObligationKind {
    match kind {
        CodingEvidenceKind::ImplementationBinding => TaskObligationKind::ImplementationBinding,
        CodingEvidenceKind::ProductionConnection => TaskObligationKind::ProductionConnection,
        CodingEvidenceKind::StaticValidation => TaskObligationKind::StaticValidation,
        CodingEvidenceKind::RuntimeValidation => TaskObligationKind::RuntimeValidation,
        CodingEvidenceKind::AcceptanceTest => TaskObligationKind::AcceptanceTest,
    }
}
