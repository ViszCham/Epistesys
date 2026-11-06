//! 有限条件評価をcurrent task ownerに束縛する。source truthへ昇格しない。
//! Bind actual finite evaluation to its live current task owner, retaining source gaps.

use super::{CurrentCodingError, CurrentCodingInput, CurrentCodingRequirement};
use crate::{
    logical_lowering::FormulaId,
    solver_coding_bridge::{
        check_condition, dgcl_input::BoundConditionReview, ConditionBridgeError,
        ConditionObservation, ConditionReport, ConditionRequest, ReportedRelation,
        FINITE_CONDITION_REVISION,
    },
};
use lc631_core::context::ContextRevision;

/// The report is advisory; support is calculated by the existing bounded evaluator.
/// Live ownership prevents a detached same-content observation being substituted.
pub struct CurrentConditionQuery<'owner, 'report, 'context> {
    owner: &'owner CurrentCodingInput<'report, 'context>,
    requirement_index: usize,
    request: ConditionRequest<'owner>,
    report: ConditionReport<'owner>,
}

impl<'report, 'context> CurrentCodingRequirement<'_, 'report, 'context> {
    pub fn condition_query(
        &self,
        condition_index: usize,
        reported: ReportedRelation,
    ) -> Result<CurrentConditionQuery<'_, 'report, 'context>, CurrentCodingError> {
        if !self
            .requirement
            .condition_indices()
            .contains(&condition_index)
        {
            return Err(CurrentCodingError::ForeignConditionOwner);
        }
        let request = ConditionRequest::new(self.input.input.logical_task(), condition_index)
            .map_err(|error| {
                CurrentCodingError::Input(super::super::CodingInputError::TaskRoute(error))
            })?;
        let report = ConditionReport::reported(&request, reported);
        Ok(CurrentConditionQuery {
            owner: self.input,
            requirement_index: self.index,
            request,
            report,
        })
    }

    pub fn review_condition(
        &self,
        observed: &CurrentConditionObservation<'_, '_, '_, '_>,
    ) -> Result<BoundConditionReview, CurrentCodingError> {
        if !std::ptr::eq(self.input, observed.query.owner)
            || self.index != observed.query.requirement_index
        {
            return Err(CurrentCodingError::ForeignConditionOwner);
        }
        self.requirement
            .review_condition(&observed.observation)
            .map_err(CurrentCodingError::Input)
    }
}

impl<'owner, 'report, 'context> CurrentConditionQuery<'owner, 'report, 'context> {
    pub fn predicate(&self) -> FormulaId {
        self.request.predicate()
    }

    pub fn context_revision(&self) -> ContextRevision {
        self.owner.current.original().state().revision()
    }

    pub const fn measurement_revision(&self) -> &'static str {
        FINITE_CONDITION_REVISION
    }

    /// Enumerate the actual predicate and explicit premises, preserving budget,
    /// foreign-formula, unsupported-fragment and inconsistent-context failures.
    pub fn observe<'input>(
        &'input self,
        premises: &'input [(FormulaId, bool)],
        max_steps: usize,
    ) -> Result<CurrentConditionObservation<'input, 'owner, 'report, 'context>, ConditionBridgeError>
    {
        let observation = check_condition(&self.request, premises, &self.report, max_steps)?;
        Ok(CurrentConditionObservation {
            query: self,
            observation,
        })
    }
}

/// Model support is over explicit premises, not verified source facts. This
/// object cannot resolve legacy source conditions or mint a coding receipt.
pub struct CurrentConditionObservation<'input, 'owner, 'report, 'context> {
    query: &'input CurrentConditionQuery<'owner, 'report, 'context>,
    observation: ConditionObservation<'input, 'owner>,
}

impl CurrentConditionObservation<'_, '_, '_, '_> {
    pub fn observation(&self) -> &ConditionObservation<'_, '_> {
        &self.observation
    }

    pub fn query(&self) -> &CurrentConditionQuery<'_, '_, '_> {
        self.query
    }
}
