use super::*;
use crate::{context_lowering::LoweredSemantics, *};
use lc631_core::context::SourceRevision;
use lc631_substrate::{
    source::{SourceSpan, SourceStore},
    Digest32,
};
use lc631_tldg::Dg1SourceRole;
use std::{ops::Range, sync::Arc};

pub(super) fn lower(
    report: &DgclPipelineReport,
    context: &LoweredContext,
    task: TaskKind,
    profile: LogicProfile,
    budget: LogicalLoweringBudget,
) -> Result<LogicalTask, LogicalLoweringError> {
    validate_budget(budget)?;
    let source = report.source_store();
    if source.id() != context.source_store().id()
        || context.original().state().revision().source() != SourceRevision::of(source.text())
    {
        return Err(LogicalLoweringError::SourceMismatch);
    }
    if !verify_dgcl_pipeline_identity(source.text(), report) || !same_parses(report, context) {
        return Err(LogicalLoweringError::PipelineIdentity);
    }
    let owner = formula_owner(
        context.original().state().revision(),
        context.original().state().canonical_id(),
        task,
        profile,
        &report.program_ir.program_digest,
        report.program_ir.grammar_revision,
    );
    let mut builder = Builder {
        source,
        context,
        budget,
        owner,
        profile,
        formulas: Vec::new(),
        directives: Vec::new(),
        conditions: Vec::new(),
        residuals: Vec::new(),
        relations: Vec::new(),
    };
    builder.preserve_residuals()?;
    for parse in context.advisory_parses() {
        builder.parse(parse)?;
    }
    let mut partition = Vec::new();
    partition
        .try_reserve_exact(context.regions().len())
        .map_err(|_| LogicalLoweringError::AllocationFailed)?;
    partition.extend(context.regions().iter().map(|region| region.span()));
    let candidate = TaskProgramIr {
        owner,
        context: context.original().state().revision(),
        context_identity: context.original().state().canonical_id(),
        task,
        profile,
        legacy_program_digest: report.program_ir.program_digest.clone(),
        parser_grammar_revision: report.program_ir.grammar_revision,
        formulas: builder.formulas,
        directives: builder.directives,
        relations: builder.relations,
    };
    let program =
        crate::dgcl_projection::bind_logical_task_program(&report.program_ir, source, candidate)?;
    let plan = crate::completion_plan::build_logical_task_plan(&program, budget.max_obligations)?;
    Ok(LogicalTask {
        source: source.clone(),
        program,
        plan,
        conditions: builder.conditions,
        residuals: builder.residuals,
        partition,
        source_regions: shared_records(context.regions())?,
        source_residuals: shared_records(context.residuals())?,
        advisory_parses: shared_records(context.advisory_parses())?,
    })
}

fn same_parses(report: &DgclPipelineReport, context: &LoweredContext) -> bool {
    report.instruction_parses.len() == context.advisory_parses().len()
        && report
            .instruction_parses
            .iter()
            .zip(context.advisory_parses())
            .all(|(a, b)| {
                a.region_id == b.region_id
                    && a.source_span == b.source_span
                    && a.state == b.state
                    && a.root_source_revision == b.root_source_revision
                    && a.error == b.error
                    && match (&a.report, &b.report) {
                        (Some(x), Some(y)) => Arc::ptr_eq(x, y),
                        (None, None) => true,
                        _ => false,
                    }
            })
}

fn validate_budget(budget: LogicalLoweringBudget) -> Result<(), LogicalLoweringError> {
    if [
        budget.max_nodes,
        budget.max_obligations,
        budget.max_residuals,
    ]
    .iter()
    .any(|n| *n == 0 || *n > 100_000)
        || budget.max_depth == 0
        || budget.max_depth > 256
    {
        return Err(LogicalLoweringError::InvalidBudget);
    }
    Ok(())
}

fn shared_records<T: Clone>(input: &[T]) -> Result<Vec<T>, LogicalLoweringError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(input.len())
        .map_err(|_| LogicalLoweringError::AllocationFailed)?;
    values.extend_from_slice(input);
    Ok(values)
}

pub(super) fn push<T>(
    values: &mut Vec<T>,
    value: T,
    limit: usize,
) -> Result<(), LogicalLoweringError> {
    if values.len() >= limit {
        return Err(LogicalLoweringError::BudgetExceeded);
    }
    values
        .try_reserve(1)
        .map_err(|_| LogicalLoweringError::AllocationFailed)?;
    values.push(value);
    Ok(())
}

pub(super) struct Builder<'a> {
    pub(super) source: &'a SourceStore,
    pub(super) context: &'a LoweredContext,
    pub(super) budget: LogicalLoweringBudget,
    pub(super) owner: Digest32,
    pub(super) profile: LogicProfile,
    pub(super) formulas: Vec<LogicalFormula>,
    pub(super) directives: Vec<LogicalDirective>,
    pub(super) conditions: Vec<LogicalCondition>,
    pub(super) residuals: Vec<LogicalResidual>,
    pub(super) relations: Vec<LogicalRelation>,
}

impl Builder<'_> {
    pub(super) fn node(
        &mut self,
        span: SourceSpan,
        kind: FormulaKind,
    ) -> Result<FormulaId, LogicalLoweringError> {
        self.source
            .slice(span)
            .map_err(|_| LogicalLoweringError::SourceSpan)?;
        let index =
            u32::try_from(self.formulas.len()).map_err(|_| LogicalLoweringError::BudgetExceeded)?;
        let id = FormulaId {
            owner: self.owner,
            index,
        };
        push(
            &mut self.formulas,
            LogicalFormula { id, span, kind },
            self.budget.max_nodes,
        )?;
        Ok(id)
    }
    pub(super) fn residual(
        &mut self,
        span: SourceSpan,
        reason: LogicalResidualReason,
    ) -> Result<(), LogicalLoweringError> {
        push(
            &mut self.residuals,
            LogicalResidual { span, reason },
            self.budget.max_residuals,
        )
    }
    pub(super) fn span(
        &self,
        anchor: SourceSpan,
        relative: &Range<usize>,
    ) -> Result<SourceSpan, LogicalLoweringError> {
        if relative.start > relative.end || relative.end > anchor.end() - anchor.start() {
            return Err(LogicalLoweringError::SourceSpan);
        }
        let start = anchor
            .start()
            .checked_add(relative.start)
            .ok_or(LogicalLoweringError::SourceSpan)?;
        let end = anchor
            .start()
            .checked_add(relative.end)
            .ok_or(LogicalLoweringError::SourceSpan)?;
        SourceSpan::checked(self.source, start, end).map_err(|_| LogicalLoweringError::SourceSpan)
    }
    fn preserve_residuals(&mut self) -> Result<(), LogicalLoweringError> {
        for residual in self.context.residuals() {
            self.residual(residual.span(), LogicalResidualReason::UnsupportedSource)?;
        }
        for region in self.context.regions() {
            if !matches!(
                region.semantics(),
                LoweredSemantics::AdvisoryParsed | LoweredSemantics::NotAsserted
            ) {
                self.residual(region.span(), LogicalResidualReason::UnsupportedSource)?;
            }
        }
        Ok(())
    }
    fn parse(&mut self, parse: &DgclInstructionParse) -> Result<(), LogicalLoweringError> {
        let anchor =
            SourceSpan::checked(self.source, parse.source_span.start, parse.source_span.end)
                .map_err(|_| LogicalLoweringError::SourceSpan)?;
        // A quote/code/opaque region is retained in the source partition, never
        // transformed into a proposition merely because it contains a marker.
        if self.context.regions().iter().any(|r| {
            r.span().start() < anchor.end()
                && anchor.start() < r.span().end()
                && matches!(
                    r.role(),
                    Dg1SourceRole::Quoted | Dg1SourceRole::Code | Dg1SourceRole::Opaque
                )
        }) {
            return self.residual(anchor, LogicalResidualReason::UnsupportedSource);
        }
        let selected = parse.report.as_ref().and_then(|r| r.selected.as_ref());
        if parse.state != DgclInstructionParseState::Parsed || selected.is_none() {
            return self.residual(
                anchor,
                match parse.state {
                    DgclInstructionParseState::Ambiguous => LogicalResidualReason::AmbiguousParse,
                    DgclInstructionParseState::Unresolved => LogicalResidualReason::UnresolvedParse,
                    DgclInstructionParseState::Parsed
                    | DgclInstructionParseState::Unsupported
                    | DgclInstructionParseState::BudgetRejected => {
                        LogicalResidualReason::UnsupportedSource
                    }
                },
            );
        }
        if let Some(ast) = selected {
            self.ast(anchor, ast)?;
        }
        Ok(())
    }
    fn ast(
        &mut self,
        anchor: SourceSpan,
        ast: &InstructionAst,
    ) -> Result<(), LogicalLoweringError> {
        let condition_start = self.add_conditions(anchor, ast)?;
        for reference in &ast.references {
            self.residual(
                self.span(anchor, &reference.source_span)?,
                LogicalResidualReason::UnresolvedReference,
            )?;
        }
        let uncertain = !ast.references.is_empty()
            || ast.coordination_state == InstructionCoordinationState::Unresolved
            || ast
                .scopes
                .iter()
                .any(|s| s.state != InstructionScopeState::Candidate)
            || ast.shared_argument_state == SharedArgumentState::UnresolvedCandidate;
        if ast.coordination_state == InstructionCoordinationState::Unresolved {
            self.residual(anchor, LogicalResidualReason::UnresolvedCoordination)?;
        }
        let directive_start = self.directives.len();
        for (index, action) in ast.actions.iter().enumerate() {
            let span = self.span(anchor, &action.source_span)?;
            let atom = self.node(
                span,
                FormulaKind::Atom {
                    role: AtomRole::UninterpretedAction,
                },
            )?;
            let mut target = self.node(
                span,
                FormulaKind::Directive {
                    action: atom,
                    modality: action.modality,
                    polarity: action.polarity,
                },
            )?;
            let mut condition_indices = Vec::new();
            for (offset, condition) in ast.conditions.iter().enumerate() {
                if condition.applies_to_action_indices.contains(&index) {
                    let ci = condition_start
                        .checked_add(offset)
                        .ok_or(LogicalLoweringError::BudgetExceeded)?;
                    let predicate = self.conditions[ci].formula;
                    target = self.node(
                        span,
                        FormulaKind::Guarded {
                            relation: condition.relation,
                            predicate,
                            body: target,
                        },
                    )?;
                    push(&mut condition_indices, ci, self.budget.max_nodes)?;
                }
            }
            self.residual(span, LogicalResidualReason::UninterpretedPredicate)?;
            let applicability =
                self.action_applicability(ast, index, !condition_indices.is_empty());
            push(
                &mut self.directives,
                LogicalDirective {
                    target,
                    span,
                    condition_indices,
                    needs_interpretation: uncertain
                        || action.modality == InstructionModality::Unknown
                        || matches!(
                            action.polarity,
                            InstructionPolarity::Unknown | InstructionPolarity::Conflict
                        ),
                    applicability,
                },
                self.budget.max_nodes,
            )?;
        }
        self.add_relations(anchor, ast, directive_start)?;
        Ok(())
    }
}
