//! 現在のcontextと実validation planの接続。権限・証拠は別producerが検証する。
//! Bind the caller's current context and actual validation plans without issuing authority.

use super::{BoundCodingRequirement, CodingInputError, CodingInputReadiness, DgclCodingInput};
use crate::{
    context_lowering::LoweredContext,
    logical_lowering::{LogicalLoweringBudget, LogicalTask},
    observe_dgcl_cargo_validation, observe_dgcl_cargo_validation_with_runner,
    CheckedDgclCargoObservation, CodingEvidenceKind, DgclCargoExecutionContext,
    DgclCargoValidationPlan, DgclPipelineReport, DgclValidationProducerError, PlannedEvidenceTask,
    TaskApplicability, ToolProcessRunner,
};
use lc631_core::context::ContextRevision;
use std::path::Path;

mod condition;
pub use condition::{CurrentConditionObservation, CurrentConditionQuery};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentCodingError {
    Input(CodingInputError),
    ContextMismatch,
    NotReady(CodingInputReadiness),
    ValidationPlanMismatch,
    ImplementationBindingIsNotCargo,
    ForeignConditionOwner,
}

/// Own the lowered task and borrow the exact current context supplied by the caller.
/// Current means this explicit input, not observed host freshness or renewed authority.
pub struct CurrentCodingInput<'report, 'context> {
    input: DgclCodingInput<'report>,
    current: &'context LoweredContext,
}

impl<'report, 'context> CurrentCodingInput<'report, 'context> {
    pub fn lower(
        report: &'report DgclPipelineReport,
        current: &'context LoweredContext,
        budget: LogicalLoweringBudget,
    ) -> Result<Self, CurrentCodingError> {
        let input =
            DgclCodingInput::lower(report, current, budget).map_err(CurrentCodingError::Input)?;
        Self::checked(input, current)
    }

    /// Unlike the legacy advisory constructor, require the caller's current context.
    pub fn from_lowered(
        report: &'report DgclPipelineReport,
        current: &'context LoweredContext,
        logical: LogicalTask,
    ) -> Result<Self, CurrentCodingError> {
        let input =
            DgclCodingInput::from_lowered(report, logical).map_err(CurrentCodingError::Input)?;
        Self::checked(input, current)
    }

    fn checked(
        input: DgclCodingInput<'report>,
        current: &'context LoweredContext,
    ) -> Result<Self, CurrentCodingError> {
        let task = input.logical_task();
        if task.context_revision() != current.original().state().revision()
            || task.source_store().id() != current.source_store().id()
            || task.source_store().text() != current.source_store().text()
        {
            return Err(CurrentCodingError::ContextMismatch);
        }
        Ok(Self { input, current })
    }

    pub fn current_context(&self) -> &LoweredContext {
        self.current
    }

    pub fn input(&self) -> &DgclCodingInput<'report> {
        &self.input
    }

    pub fn bind_requirement(
        &self,
        index: usize,
    ) -> Result<CurrentCodingRequirement<'_, 'report, 'context>, CurrentCodingError> {
        let requirement = self
            .input
            .bind_requirement(index)
            .map_err(CurrentCodingError::Input)?;
        Ok(CurrentCodingRequirement {
            input: self,
            requirement,
            index,
        })
    }
}

pub struct CurrentCodingRequirement<'input, 'report, 'context> {
    input: &'input CurrentCodingInput<'report, 'context>,
    requirement: BoundCodingRequirement<'input, 'report>,
    index: usize,
}

impl<'report, 'context> CurrentCodingRequirement<'_, 'report, 'context> {
    pub fn requirement(&self) -> &BoundCodingRequirement<'_, 'report> {
        &self.requirement
    }

    /// The checked view is producer input only. Tool registration, repository
    /// snapshots, current permits, timeouts and acceptance are checked on observation.
    pub fn bind_validation_plan<'binding, 'plan>(
        &'binding self,
        plan: &'plan DgclCargoValidationPlan,
    ) -> Result<BoundValidationInput<'binding, 'report, 'context, 'plan>, CurrentCodingError> {
        let readiness = self.requirement.readiness();
        if readiness != CodingInputReadiness::ReadyForEvidence {
            return Err(CurrentCodingError::NotReady(readiness));
        }
        if plan.kind == CodingEvidenceKind::ImplementationBinding {
            return Err(CurrentCodingError::ImplementationBindingIsNotCargo);
        }
        let pipeline = self.input.input.pipeline();
        if plan.schema_version != "epistesys-dgcl-cargo-validation-plan.v1"
            || plan.source_revision != pipeline.source_revision
            || plan.completion_plan_digest != pipeline.completion_plan.plan_digest
            || plan.requirement_id != self.requirement.requirement().requirement_id
        {
            return Err(CurrentCodingError::ValidationPlanMismatch);
        }
        let task = self
            .requirement
            .planned_tasks()
            .iter()
            .find(|task| {
                task.task_id == plan.task_id
                    && task.requirement_id == plan.requirement_id
                    && task.evidence_kind == plan.kind
                    && task.applicability == TaskApplicability::Required
                    && task.not_applicable_basis.is_none()
                    && task.condition_ids.is_empty()
                    && !task.producer_revision.is_empty()
                    && !task.acceptance_criterion.is_empty()
            })
            .ok_or(CurrentCodingError::ValidationPlanMismatch)?;
        Ok(BoundValidationInput {
            input: self.input,
            task,
            plan,
        })
    }
}

/// A borrowed plan connected to an exact task and current context. No Clone,
/// Deserialize, receipt issuer or execution-permit constructor is exposed here.
pub struct BoundValidationInput<'input, 'report, 'context, 'plan> {
    input: &'input CurrentCodingInput<'report, 'context>,
    task: &'input PlannedEvidenceTask,
    plan: &'plan DgclCargoValidationPlan,
}

impl BoundValidationInput<'_, '_, '_, '_> {
    pub fn task(&self) -> &PlannedEvidenceTask {
        self.task
    }

    pub fn plan(&self) -> &DgclCargoValidationPlan {
        self.plan
    }

    pub fn context_revision(&self) -> ContextRevision {
        self.input.current.original().state().revision()
    }

    /// Run the actual existing non-executing filesystem/plan preflight. Passing
    /// does not establish registration, a valid permit, or observed Cargo output.
    pub fn preflight(&self, root: &Path) -> Result<(), DgclValidationProducerError> {
        crate::preflight_dgcl_cargo_plan(root, self.input.input.pipeline(), self.plan)
    }

    /// Use the existing permit-aware producer, preserving every original check.
    /// Merely binding this input never starts a process or issues evidence.
    pub fn observe(
        &self,
        root: &Path,
        execution: DgclCargoExecutionContext<'_>,
    ) -> Result<CheckedDgclCargoObservation, DgclValidationProducerError> {
        let pipeline = self.input.input.pipeline();
        observe_dgcl_cargo_validation(
            root,
            pipeline.source_store().text(),
            pipeline,
            self.plan,
            execution,
        )
    }

    /// The registered/injected runner follows the same existing observation path.
    pub fn observe_with_runner(
        &self,
        root: &Path,
        execution: DgclCargoExecutionContext<'_>,
        runner: &mut dyn ToolProcessRunner,
    ) -> Result<CheckedDgclCargoObservation, DgclValidationProducerError> {
        let pipeline = self.input.input.pipeline();
        observe_dgcl_cargo_validation_with_runner(
            root,
            pipeline.source_store().text(),
            pipeline,
            self.plan,
            execution,
            runner,
        )
    }
}
