//! Preserve controlled condition/coordination structure; do not evaluate it.
use super::{
    builder::{push, Builder},
    *,
};
use crate::{ConditionExpression, ConditionScopeState, InstructionAst, InstructionConjunction};
use lc631_substrate::source::SourceSpan;

impl Builder<'_> {
    pub(super) fn add_conditions(
        &mut self,
        anchor: SourceSpan,
        ast: &InstructionAst,
    ) -> Result<usize, LogicalLoweringError> {
        let start = self.conditions.len();
        for condition in &ast.conditions {
            let span = self.span(anchor, &condition.source_span)?;
            let formula = self.expression(anchor, &condition.expression, 0)?;
            push(
                &mut self.conditions,
                LogicalCondition {
                    span,
                    formula,
                    reported_state: condition.evidence_state,
                },
                self.budget.max_nodes,
            )?;
            if condition.evidence_state == crate::ConditionEvidenceState::Unknown {
                self.residual(span, LogicalResidualReason::UnknownCondition)?;
            }
            if condition.scope_state == ConditionScopeState::Unresolved {
                self.residual(span, LogicalResidualReason::UnresolvedCondition)?;
            }
            if condition.applies_to_action_indices.start > condition.applies_to_action_indices.end
                || condition.applies_to_action_indices.end > ast.actions.len()
            {
                return Err(LogicalLoweringError::InvalidAst);
            }
        }
        Ok(start)
    }

    pub(super) fn action_applicability(
        &self,
        ast: &InstructionAst,
        action: usize,
        conditional: bool,
    ) -> TaskObligationApplicability {
        let involved = |edge: &&crate::InstructionCoordination| {
            edge.left_action_index as usize == action || edge.right_action_index as usize == action
        };
        if ast.coordination.iter().filter(involved).any(|edge| {
            matches!(
                edge.kind,
                InstructionConjunction::Or | InstructionConjunction::Xor
            )
        }) {
            TaskObligationApplicability::AlternativePending
        } else if ast.coordination.iter().filter(involved).any(|edge| {
            matches!(
                edge.kind,
                InstructionConjunction::Then
                    | InstructionConjunction::JapaneseTe
                    | InstructionConjunction::JapaneseComma
            )
        }) {
            TaskObligationApplicability::Unresolved
        } else if conditional {
            TaskObligationApplicability::Conditional
        } else {
            TaskObligationApplicability::Required
        }
    }

    pub(super) fn add_relations(
        &mut self,
        anchor: SourceSpan,
        ast: &InstructionAst,
        start: usize,
    ) -> Result<(), LogicalLoweringError> {
        for edge in &ast.coordination {
            let left = start
                .checked_add(edge.left_action_index as usize)
                .ok_or(LogicalLoweringError::BudgetExceeded)?;
            let right = start
                .checked_add(edge.right_action_index as usize)
                .ok_or(LogicalLoweringError::BudgetExceeded)?;
            let left = self
                .directives
                .get(left)
                .ok_or(LogicalLoweringError::InvalidAst)?
                .target;
            let right = self
                .directives
                .get(right)
                .ok_or(LogicalLoweringError::InvalidAst)?
                .target;
            let span = self.span(anchor, &edge.source_span)?;
            push(
                &mut self.relations,
                LogicalRelation {
                    left,
                    right,
                    kind: edge.kind,
                    span,
                },
                self.budget.max_nodes,
            )?;
        }
        Ok(())
    }

    fn expression(
        &mut self,
        anchor: SourceSpan,
        expression: &ConditionExpression,
        depth: usize,
    ) -> Result<FormulaId, LogicalLoweringError> {
        if depth >= self.budget.max_depth {
            return Err(LogicalLoweringError::BudgetExceeded);
        }
        match expression {
            ConditionExpression::Predicate(p) | ConditionExpression::Unresolved(p) => {
                let span = self.span(anchor, &p.source_span)?;
                if self
                    .source
                    .slice(span)
                    .map_err(|_| LogicalLoweringError::SourceSpan)?
                    != p.text
                {
                    return Err(LogicalLoweringError::InvalidAst);
                }
                self.residual(span, LogicalResidualReason::UninterpretedPredicate)?;
                let kind = if matches!(expression, ConditionExpression::Unresolved(_)) {
                    FormulaKind::Unresolved
                } else {
                    FormulaKind::Atom {
                        role: AtomRole::UninterpretedCondition,
                    }
                };
                self.node(span, kind)
            }
            ConditionExpression::Not(child) => {
                let operand = self.expression(anchor, child, depth + 1)?;
                let span = self.formulas[operand.index as usize].span;
                let kind = match self.profile {
                    LogicProfile::ClassicalPropositionalV1 => NegationKind::Classical,
                    LogicProfile::AspGroundNormalV1 => NegationKind::Default,
                };
                self.node(span, FormulaKind::Negation { kind, operand })
            }
            ConditionExpression::All(children) | ConditionExpression::Any(children) => {
                if children.is_empty() {
                    return Err(LogicalLoweringError::InvalidAst);
                }
                let mut ids = Vec::new();
                for child in children {
                    let id = self.expression(anchor, child, depth + 1)?;
                    push(&mut ids, id, self.budget.max_nodes)?;
                }
                let start = ids
                    .iter()
                    .map(|id| self.formulas[id.index as usize].span.start())
                    .min()
                    .ok_or(LogicalLoweringError::InvalidAst)?;
                let end = ids
                    .iter()
                    .map(|id| self.formulas[id.index as usize].span.end())
                    .max()
                    .ok_or(LogicalLoweringError::InvalidAst)?;
                let span = SourceSpan::checked(self.source, start, end)
                    .map_err(|_| LogicalLoweringError::SourceSpan)?;
                self.node(
                    span,
                    if matches!(expression, ConditionExpression::All(_)) {
                        FormulaKind::All(ids)
                    } else {
                        FormulaKind::Any(ids)
                    },
                )
            }
        }
    }
}
