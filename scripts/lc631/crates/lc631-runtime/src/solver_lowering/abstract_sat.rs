use super::FormulaBinding;
use lc631_analysis::{
    logical_lowering::{
        AtomRole, FormulaId, FormulaKind, LogicProfile as AnalysisLogicProfile,
        LogicalResidualReason, LogicalTask, NegationKind,
    },
    ConditionEvidenceState, ConditionRelation,
};
use lc631_solvers::{
    runner::{QuerySourceIdentity, SolverQuery},
    typed_query::{
        BoolExpr, BoolSymbolId, PreparedSmtStatusQuery, QueryEmitError, SmtEmitLimits,
        SmtStatusQuery,
    },
};
use lc631_substrate::source::SourceSpan;

/// The interpretation for an abstract satisfiability query must be selected
/// explicitly. `FreeBooleanAtoms` gives each parsed condition atom an
/// unconstrained Boolean variable; it never assigns a truth value to the
/// source condition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbstractSatisfiabilityInterpretation {
    FreeBooleanAtoms,
}

/// Operational cap for provenance/scope traversal before abstract query
/// emission. This is not a TranslationLoss threshold or semantic acceptance
/// criterion.
pub const MAX_ABSTRACT_SCOPE_WORK: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbstractSatisfiabilityBudget {
    max_scope_work: usize,
}

impl AbstractSatisfiabilityBudget {
    /// Set the deterministic number of scope/provenance inspection steps.
    /// A zero limit intentionally makes every nonempty traversal Hold. Values
    /// above `MAX_ABSTRACT_SCOPE_WORK` are rejected when preparing a query.
    pub const fn with_max_scope_work(max_scope_work: usize) -> Self {
        Self { max_scope_work }
    }

    pub const fn max_scope_work(self) -> usize {
        self.max_scope_work
    }
}

impl Default for AbstractSatisfiabilityBudget {
    fn default() -> Self {
        Self {
            max_scope_work: MAX_ABSTRACT_SCOPE_WORK,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbstractSatisfiabilityHoldReason {
    UnsupportedLogicProfile,
    SourceContextMismatch,
    ForeignFormula,
    EmptyFormulaSet,
    NotConditionFormula,
    AmbiguousConditionFormula,
    ConditionEvidenceNotUnknown,
    MissingUnknownConditionResidual,
    AmbiguousConditionResidual,
    UnresolvedConditionScope,
    AmbiguousConditionScope,
    UnsupportedConditionRelation,
    UnsupportedConditionResidual,
    ResidualIndexOutOfRange,
    UninterpretedPredicateNotBound,
    UninterpretedPredicateOutsideSelectedFormula,
    SourceSpanMismatch,
    UnsupportedActionAtom,
    UnsupportedDefaultNegation,
    UnsupportedDirective,
    UnsupportedGuard,
    UnresolvedFormula,
    EmptyBooleanOperator,
    UnsupportedCausalSpatialTemporalForm,
    CyclicFormula,
    NodeLimitExceeded,
    DepthLimitExceeded,
    BooleanSymbolLimitExceeded,
    InvalidScopeBudget,
    ScopeWorkLimitExceeded,
    FormulaIndexOutOfRange,
    AllocationFailed,
    QueryEmission(QueryEmitError),
}

/// Provenance for an atom in the selected condition tree. The condition's
/// reported evidence state is retained as metadata; the atom is still free.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbstractConditionAtomBinding {
    formula: FormulaId,
    source_span: SourceSpan,
    condition_reported_state: ConditionEvidenceState,
    symbol: BoolSymbolId,
}

impl AbstractConditionAtomBinding {
    pub const fn formula_id(self) -> FormulaId {
        self.formula
    }

    pub const fn source_span(self) -> SourceSpan {
        self.source_span
    }

    pub const fn condition_reported_state(self) -> ConditionEvidenceState {
        self.condition_reported_state
    }

    pub const fn symbol(self) -> BoolSymbolId {
        self.symbol
    }
}

/// A selected residual's exact source binding. The unknown-condition marker
/// binds to the condition root; predicate residuals bind to their atom node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbstractConditionResidualBinding {
    residual_index: u32,
    reason: LogicalResidualReason,
    source_span: SourceSpan,
    formula: FormulaId,
}

impl AbstractConditionResidualBinding {
    pub const fn residual_index(self) -> u32 {
        self.residual_index
    }

    pub const fn reason(self) -> LogicalResidualReason {
        self.reason
    }

    pub const fn source_span(self) -> SourceSpan {
        self.source_span
    }

    pub const fn formula_id(self) -> FormulaId {
        self.formula
    }
}

/// A task residual outside the selected condition. It is exposed only as an
/// unvalidated source summary and is not bound into the abstract query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnvalidatedTaskResidualBinding {
    residual_index: u32,
    reason: LogicalResidualReason,
    source_span: SourceSpan,
}

impl UnvalidatedTaskResidualBinding {
    pub const fn residual_index(self) -> u32 {
        self.residual_index
    }

    pub const fn reason(self) -> LogicalResidualReason {
        self.reason
    }

    pub const fn source_span(self) -> SourceSpan {
        self.source_span
    }
}

/// A parent guard is retained as scope provenance, but is not included in the
/// abstract condition-tree satisfiability assertion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbstractConditionGuardBinding {
    formula: FormulaId,
    relation: ConditionRelation,
    source_span: SourceSpan,
}

impl AbstractConditionGuardBinding {
    pub const fn formula_id(self) -> FormulaId {
        self.formula
    }

    pub const fn relation(self) -> ConditionRelation {
        self.relation
    }

    pub const fn source_span(self) -> SourceSpan {
        self.source_span
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbstractSatisfiabilityHold<'task> {
    binding: FormulaBinding<'task>,
    reason: AbstractSatisfiabilityHoldReason,
}

impl<'task> AbstractSatisfiabilityHold<'task> {
    pub const fn binding(self) -> FormulaBinding<'task> {
        self.binding
    }

    pub const fn reason(self) -> AbstractSatisfiabilityHoldReason {
        self.reason
    }

    pub const fn claim_boundary(self) -> &'static str {
        "source-bound typed Hold for abstract satisfiability; no source truth, predicate evidence, entailment, obligation completion, authority, or output commit"
    }
}

/// A source-bound Boolean syntax query under an explicit free-atom
/// interpretation. It deliberately exposes no bare `SolverQuery` or prepared
/// SMT object: the existing runner/result-store handoff drops context/task/
/// formula lineage. Only read-only payload inspection is available here.
///
/// ```compile_fail
/// use lc631_runtime::solver_lowering::PreparedAbstractSatisfiabilityQuery;
/// fn detach(query: &PreparedAbstractSatisfiabilityQuery<'_>) {
///     let _ = query.solver_query();
/// }
/// ```
///
/// ```compile_fail
/// use lc631_runtime::solver_lowering::PreparedAbstractSatisfiabilityQuery;
/// fn bind_result_store(query: &PreparedAbstractSatisfiabilityQuery<'_>) {
///     let _ = lc631_solvers::result_store::SolverResultBinding::for_query(query, None);
/// }
/// ```
///
/// ```compile_fail
/// use lc631_runtime::solver_lowering::PreparedAbstractSatisfiabilityQuery;
/// fn detach_prepared_smt(query: &PreparedAbstractSatisfiabilityQuery<'_>) {
///     let _ = query.prepared_smt();
/// }
/// ```
///
/// This does not assert that the parsed structure is the source's complete
/// meaning, nor that any predicate is true.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedAbstractSatisfiabilityQuery<'task> {
    binding: FormulaBinding<'task>,
    interpretation: AbstractSatisfiabilityInterpretation,
    condition_span: SourceSpan,
    condition_reported_state: ConditionEvidenceState,
    relation: ConditionRelation,
    guard_uses: Vec<AbstractConditionGuardBinding>,
    atoms: Vec<AbstractConditionAtomBinding>,
    selected_condition_residuals: Vec<AbstractConditionResidualBinding>,
    unvalidated_task_residuals: Vec<UnvalidatedTaskResidualBinding>,
    query: PreparedSmtStatusQuery,
}

impl<'task> PreparedAbstractSatisfiabilityQuery<'task> {
    pub const fn binding(&self) -> FormulaBinding<'task> {
        self.binding
    }

    pub const fn interpretation(&self) -> AbstractSatisfiabilityInterpretation {
        self.interpretation
    }

    pub const fn condition_span(&self) -> SourceSpan {
        self.condition_span
    }

    pub const fn condition_reported_state(&self) -> ConditionEvidenceState {
        self.condition_reported_state
    }

    pub const fn relation(&self) -> ConditionRelation {
        self.relation
    }

    pub fn guard_uses(&self) -> &[AbstractConditionGuardBinding] {
        &self.guard_uses
    }

    pub fn atom_bindings(&self) -> &[AbstractConditionAtomBinding] {
        &self.atoms
    }

    pub fn selected_condition_residuals(&self) -> &[AbstractConditionResidualBinding] {
        &self.selected_condition_residuals
    }

    /// Residuals elsewhere in the task are reported for visibility only. They
    /// are not validated or represented by this selected-condition query.
    pub fn unvalidated_task_residuals(&self) -> &[UnvalidatedTaskResidualBinding] {
        &self.unvalidated_task_residuals
    }

    pub fn unvalidated_task_residual_count(&self) -> usize {
        self.unvalidated_task_residuals.len()
    }

    /// Read-only emitted payload for inspection; this is not a typed solver
    /// handoff and carries no detached-result or evidence capability.
    pub fn smt_payload(&self) -> &[u8] {
        self.query.payload()
    }

    /// Private same-object handoff used only by the runtime's scoped owner.
    pub(crate) fn scoped_solver_query(&self) -> Result<SolverQuery<'_>, QueryEmitError> {
        self.query.solver_query()
    }

    pub const fn claim_boundary(&self) -> &'static str {
        "syntax-only satisfiability under explicit FreeBooleanAtoms; selected-condition residuals remain bound; unvalidated task residuals are exposed but outside this query; not source truth, evidence, entailment, obligation completion, authority, solver execution, or output commit"
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AbstractSatisfiabilityDecision<'task> {
    Prepared(PreparedAbstractSatisfiabilityQuery<'task>),
    Hold(AbstractSatisfiabilityHold<'task>),
}

struct AbstractScopeWork {
    remaining: usize,
}

impl AbstractScopeWork {
    const fn new(budget: AbstractSatisfiabilityBudget) -> Self {
        Self {
            remaining: budget.max_scope_work(),
        }
    }

    fn charge(&mut self, amount: usize) -> Result<(), AbstractSatisfiabilityHoldReason> {
        self.remaining = self
            .remaining
            .checked_sub(amount)
            .ok_or(AbstractSatisfiabilityHoldReason::ScopeWorkLimitExceeded)?;
        Ok(())
    }
}

/// Prepare the selected public condition tree as a bounded abstract
/// satisfiability query. The interpretation is mandatory: each uninterpreted
/// condition atom becomes a free Boolean variable, not an assignment and not
/// observed predicate evidence. The existing `lower_logical_formula` remains
/// evidence-gated and continues to Hold on Unknown. `scope_budget` bounds
/// provenance joins and scope traversal only; it is an operational cap, not a
/// semantic or TranslationLoss threshold.
pub fn prepare_abstract_condition_satisfiability<'task>(
    task: &'task LogicalTask,
    formula: FormulaId,
    interpretation: AbstractSatisfiabilityInterpretation,
    limits: SmtEmitLimits,
    scope_budget: AbstractSatisfiabilityBudget,
) -> AbstractSatisfiabilityDecision<'task> {
    let source_digest = task.source_store().id().digest();
    let formula_span = task
        .program_ir()
        .formula(formula)
        .ok()
        .map(|formula| formula.span());
    let binding = FormulaBinding {
        source: QuerySourceIdentity::from_bytes(*source_digest.as_bytes()),
        context: task.context_revision(),
        task: task.task_kind(),
        logic_profile: task.logic_profile(),
        profile_revision: task.logic_profile().revision(),
        program_digest: task.program_ir().legacy_program_digest(),
        grammar_revision: task.program_ir().parser_grammar_revision(),
        formula,
        formula_span,
    };
    let hold = |reason| {
        AbstractSatisfiabilityDecision::Hold(AbstractSatisfiabilityHold { binding, reason })
    };
    if scope_budget.max_scope_work() > MAX_ABSTRACT_SCOPE_WORK {
        return hold(AbstractSatisfiabilityHoldReason::InvalidScopeBudget);
    }
    let mut scope_work = AbstractScopeWork::new(scope_budget);

    if task.logic_profile() != AnalysisLogicProfile::ClassicalPropositionalV1 {
        return hold(AbstractSatisfiabilityHoldReason::UnsupportedLogicProfile);
    }
    if task.context_revision().source().digest() != source_digest {
        return hold(AbstractSatisfiabilityHoldReason::SourceContextMismatch);
    }
    if task.formulas().is_empty() {
        return hold(AbstractSatisfiabilityHoldReason::EmptyFormulaSet);
    }
    let program = task.program_ir();
    let selected_formula = match program.formula(formula) {
        Ok(formula) => formula,
        Err(_) => return hold(AbstractSatisfiabilityHoldReason::ForeignFormula),
    };
    let Some(condition_formula_span) = binding.formula_span else {
        return hold(AbstractSatisfiabilityHoldReason::SourceSpanMismatch);
    };
    if task.source_store().slice(condition_formula_span).is_err() {
        return hold(AbstractSatisfiabilityHoldReason::SourceSpanMismatch);
    }

    let mut condition_selection = None;
    for (index, candidate) in task.conditions().iter().enumerate() {
        if let Err(reason) = scope_work.charge(1) {
            return hold(reason);
        }
        if candidate.formula() == formula {
            if condition_selection.is_some() {
                return hold(AbstractSatisfiabilityHoldReason::AmbiguousConditionFormula);
            }
            condition_selection = Some((index, candidate));
        }
    }
    let Some((condition_index, condition)) = condition_selection else {
        return hold(unsupported_formula_root(selected_formula.kind()));
    };
    if condition.reported_state() != ConditionEvidenceState::Unknown {
        return hold(AbstractSatisfiabilityHoldReason::ConditionEvidenceNotUnknown);
    }
    if task.source_store().slice(condition.span()).is_err() {
        return hold(AbstractSatisfiabilityHoldReason::SourceSpanMismatch);
    }

    let mut selected_condition_residuals = Vec::new();
    let mut unvalidated_task_residuals = Vec::new();
    let mut unknown_condition_count = 0_usize;
    for (residual_index, residual) in task.residuals().iter().enumerate() {
        if let Err(reason) = scope_work.charge(1) {
            return hold(reason);
        }
        let residual_index = match u32::try_from(residual_index) {
            Ok(index) => index,
            Err(_) => return hold(AbstractSatisfiabilityHoldReason::ResidualIndexOutOfRange),
        };
        let span = residual.span();
        let overlaps_condition =
            span.start() < condition.span().end() && condition.span().start() < span.end();
        if !overlaps_condition {
            if unvalidated_task_residuals.try_reserve(1).is_err() {
                return hold(AbstractSatisfiabilityHoldReason::AllocationFailed);
            }
            unvalidated_task_residuals.push(UnvalidatedTaskResidualBinding {
                residual_index,
                reason: residual.reason(),
                source_span: span,
            });
            continue;
        }
        if span.start() < condition.span().start() || span.end() > condition.span().end() {
            return hold(match residual.reason() {
                LogicalResidualReason::AmbiguousParse
                | LogicalResidualReason::UnresolvedCoordination => {
                    AbstractSatisfiabilityHoldReason::AmbiguousConditionScope
                }
                LogicalResidualReason::UnresolvedCondition
                | LogicalResidualReason::UnresolvedParse => {
                    AbstractSatisfiabilityHoldReason::UnresolvedConditionScope
                }
                _ => AbstractSatisfiabilityHoldReason::UnsupportedConditionResidual,
            });
        }
        if task.source_store().slice(span).is_err() {
            return hold(AbstractSatisfiabilityHoldReason::SourceSpanMismatch);
        }
        match residual.reason() {
            LogicalResidualReason::UnknownCondition => {
                if span != condition.span() {
                    return hold(AbstractSatisfiabilityHoldReason::UnsupportedConditionResidual);
                }
                unknown_condition_count = match unknown_condition_count.checked_add(1) {
                    Some(count) => count,
                    None => {
                        return hold(AbstractSatisfiabilityHoldReason::AmbiguousConditionResidual)
                    }
                };
                if selected_condition_residuals.try_reserve(1).is_err() {
                    return hold(AbstractSatisfiabilityHoldReason::AllocationFailed);
                }
                selected_condition_residuals.push(AbstractConditionResidualBinding {
                    residual_index,
                    reason: residual.reason(),
                    source_span: span,
                    formula,
                });
            }
            LogicalResidualReason::UninterpretedPredicate => {
                let mut matching_atom = None;
                for candidate in task.formulas() {
                    if let Err(reason) = scope_work.charge(1) {
                        return hold(reason);
                    }
                    if candidate.span() == span
                        && matches!(
                            candidate.kind(),
                            FormulaKind::Atom {
                                role: AtomRole::UninterpretedCondition
                            }
                        )
                    {
                        if matching_atom.is_some() {
                            return hold(
                                AbstractSatisfiabilityHoldReason::AmbiguousConditionResidual,
                            );
                        }
                        matching_atom = Some(candidate);
                    }
                }
                let Some(atom) = matching_atom else {
                    return hold(AbstractSatisfiabilityHoldReason::UninterpretedPredicateNotBound);
                };
                if program.formula(atom.id()).is_err() {
                    return hold(AbstractSatisfiabilityHoldReason::ForeignFormula);
                }
                if selected_condition_residuals.try_reserve(1).is_err() {
                    return hold(AbstractSatisfiabilityHoldReason::AllocationFailed);
                }
                selected_condition_residuals.push(AbstractConditionResidualBinding {
                    residual_index,
                    reason: residual.reason(),
                    source_span: span,
                    formula: atom.id(),
                });
            }
            LogicalResidualReason::UnresolvedCondition | LogicalResidualReason::UnresolvedParse => {
                return hold(AbstractSatisfiabilityHoldReason::UnresolvedConditionScope);
            }
            LogicalResidualReason::AmbiguousParse
            | LogicalResidualReason::UnresolvedCoordination => {
                return hold(AbstractSatisfiabilityHoldReason::AmbiguousConditionScope);
            }
            LogicalResidualReason::UnresolvedReference
            | LogicalResidualReason::UnsupportedSource => {
                return hold(AbstractSatisfiabilityHoldReason::UnsupportedConditionResidual);
            }
        }
    }
    if unknown_condition_count == 0 {
        return hold(AbstractSatisfiabilityHoldReason::MissingUnknownConditionResidual);
    }
    if unknown_condition_count > 1 {
        return hold(AbstractSatisfiabilityHoldReason::AmbiguousConditionResidual);
    }

    let mut guard_uses = Vec::new();
    for candidate in task.formulas() {
        if let Err(reason) = scope_work.charge(1) {
            return hold(reason);
        }
        if let FormulaKind::Guarded {
            relation,
            predicate,
            ..
        } = candidate.kind()
        {
            if *predicate != formula {
                continue;
            }
            if task.source_store().slice(candidate.span()).is_err() {
                return hold(AbstractSatisfiabilityHoldReason::SourceSpanMismatch);
            }
            if guard_uses.try_reserve(1).is_err() {
                return hold(AbstractSatisfiabilityHoldReason::AllocationFailed);
            }
            guard_uses.push(AbstractConditionGuardBinding {
                formula: candidate.id(),
                relation: *relation,
                source_span: candidate.span(),
            });
        }
    }
    let Some(first_guard) = guard_uses.first() else {
        return hold(AbstractSatisfiabilityHoldReason::UnresolvedConditionScope);
    };
    let relation = first_guard.relation();
    if relation == ConditionRelation::Temporal {
        return hold(AbstractSatisfiabilityHoldReason::UnsupportedConditionRelation);
    }
    for guard in &guard_uses {
        if let Err(reason) = scope_work.charge(1) {
            return hold(reason);
        }
        if guard.relation() != relation {
            return hold(AbstractSatisfiabilityHoldReason::AmbiguousConditionScope);
        }
    }

    let mut linked_obligations = 0_usize;
    for obligation in task.obligations() {
        if let Err(reason) = scope_work.charge(1) {
            return hold(reason);
        }
        if let Err(reason) = scope_work.charge(obligation.condition_indices().len()) {
            return hold(reason);
        }
        if !obligation.condition_indices().contains(&condition_index) {
            continue;
        }
        linked_obligations = match linked_obligations.checked_add(1) {
            Some(count) => count,
            None => return hold(AbstractSatisfiabilityHoldReason::AmbiguousConditionScope),
        };
        match obligation.applicability() {
            lc631_analysis::logical_lowering::TaskObligationApplicability::Required
            | lc631_analysis::logical_lowering::TaskObligationApplicability::Conditional => {}
            lc631_analysis::logical_lowering::TaskObligationApplicability::AlternativePending
            | lc631_analysis::logical_lowering::TaskObligationApplicability::Unresolved => {
                return hold(AbstractSatisfiabilityHoldReason::AmbiguousConditionScope);
            }
        }
    }
    if linked_obligations == 0 {
        return hold(AbstractSatisfiabilityHoldReason::UnresolvedConditionScope);
    }

    let mut visited_nodes = 0;
    let mut active = Vec::new();
    let mut atoms = Vec::new();
    let expression = match lower_abstract_condition_formula(
        task,
        limits,
        &selected_condition_residuals,
        condition.reported_state(),
        &mut scope_work,
        &mut visited_nodes,
        &mut active,
        &mut atoms,
        formula,
        1,
    ) {
        Ok(expression) => expression,
        Err(reason) => return hold(reason),
    };
    for residual in &selected_condition_residuals {
        if let Err(reason) = scope_work.charge(1) {
            return hold(reason);
        }
        if residual.reason() != LogicalResidualReason::UninterpretedPredicate {
            continue;
        }
        let mut atom_is_bound = false;
        for atom in &atoms {
            if let Err(reason) = scope_work.charge(1) {
                return hold(reason);
            }
            if atom.formula_id() == residual.formula_id() {
                atom_is_bound = true;
                break;
            }
        }
        if !atom_is_bound {
            return hold(
                AbstractSatisfiabilityHoldReason::UninterpretedPredicateOutsideSelectedFormula,
            );
        }
    }

    let mut declared_symbols = Vec::new();
    if let Err(reason) = scope_work.charge(atoms.len()) {
        return hold(reason);
    }
    if declared_symbols.try_reserve(atoms.len()).is_err() {
        return hold(AbstractSatisfiabilityHoldReason::AllocationFailed);
    }
    declared_symbols.extend(atoms.iter().map(|atom| atom.symbol()));
    let mut assertions = Vec::new();
    if assertions.try_reserve(1).is_err() {
        return hold(AbstractSatisfiabilityHoldReason::AllocationFailed);
    }
    assertions.push(expression);

    let typed = match SmtStatusQuery::checked(declared_symbols, Vec::new(), assertions, limits) {
        Ok(query) => query,
        Err(error) => return hold(AbstractSatisfiabilityHoldReason::QueryEmission(error)),
    };
    let query = match typed.emit(binding.source_identity()) {
        Ok(query) => query,
        Err(error) => return hold(AbstractSatisfiabilityHoldReason::QueryEmission(error)),
    };

    AbstractSatisfiabilityDecision::Prepared(PreparedAbstractSatisfiabilityQuery {
        binding,
        interpretation,
        condition_span: condition.span(),
        condition_reported_state: condition.reported_state(),
        relation,
        guard_uses,
        atoms,
        selected_condition_residuals,
        unvalidated_task_residuals,
        query,
    })
}

fn unsupported_formula_root(kind: &FormulaKind) -> AbstractSatisfiabilityHoldReason {
    match kind {
        FormulaKind::Atom {
            role: AtomRole::UninterpretedAction,
        } => AbstractSatisfiabilityHoldReason::UnsupportedActionAtom,
        FormulaKind::Negation {
            kind: NegationKind::Default,
            ..
        } => AbstractSatisfiabilityHoldReason::UnsupportedDefaultNegation,
        FormulaKind::Directive { .. } => AbstractSatisfiabilityHoldReason::UnsupportedDirective,
        FormulaKind::Guarded {
            relation: ConditionRelation::Temporal,
            ..
        } => AbstractSatisfiabilityHoldReason::UnsupportedConditionRelation,
        FormulaKind::Guarded { .. } => AbstractSatisfiabilityHoldReason::UnsupportedGuard,
        FormulaKind::Unresolved => AbstractSatisfiabilityHoldReason::UnresolvedFormula,
        _ => AbstractSatisfiabilityHoldReason::NotConditionFormula,
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_abstract_condition_formula(
    task: &LogicalTask,
    limits: SmtEmitLimits,
    residuals: &[AbstractConditionResidualBinding],
    condition_reported_state: ConditionEvidenceState,
    scope_work: &mut AbstractScopeWork,
    visited_nodes: &mut usize,
    active: &mut Vec<FormulaId>,
    atoms: &mut Vec<AbstractConditionAtomBinding>,
    id: FormulaId,
    depth: usize,
) -> Result<BoolExpr, AbstractSatisfiabilityHoldReason> {
    if depth > limits.max_depth() {
        return Err(AbstractSatisfiabilityHoldReason::DepthLimitExceeded);
    }
    *visited_nodes = (*visited_nodes)
        .checked_add(1)
        .ok_or(AbstractSatisfiabilityHoldReason::NodeLimitExceeded)?;
    if *visited_nodes > limits.max_nodes() {
        return Err(AbstractSatisfiabilityHoldReason::NodeLimitExceeded);
    }
    scope_work.charge(1)?;
    scope_work.charge(active.len())?;
    if active.contains(&id) {
        return Err(AbstractSatisfiabilityHoldReason::CyclicFormula);
    }
    let formula = task
        .program_ir()
        .formula(id)
        .map_err(|_| AbstractSatisfiabilityHoldReason::ForeignFormula)?;
    if active.try_reserve(1).is_err() {
        return Err(AbstractSatisfiabilityHoldReason::AllocationFailed);
    }
    active.push(id);
    let result = lower_abstract_condition_kind(
        task,
        limits,
        residuals,
        condition_reported_state,
        scope_work,
        visited_nodes,
        active,
        atoms,
        formula.kind(),
        depth,
    );
    active.pop();
    result
}

#[allow(clippy::too_many_arguments)]
fn lower_abstract_condition_kind(
    task: &LogicalTask,
    limits: SmtEmitLimits,
    residuals: &[AbstractConditionResidualBinding],
    condition_reported_state: ConditionEvidenceState,
    scope_work: &mut AbstractScopeWork,
    visited_nodes: &mut usize,
    active: &mut Vec<FormulaId>,
    atoms: &mut Vec<AbstractConditionAtomBinding>,
    kind: &FormulaKind,
    depth: usize,
) -> Result<BoolExpr, AbstractSatisfiabilityHoldReason> {
    match kind {
        FormulaKind::Atom {
            role: AtomRole::UninterpretedCondition,
        } => abstract_condition_atom(
            task,
            limits,
            residuals,
            condition_reported_state,
            scope_work,
            atoms,
            active
                .last()
                .copied()
                .ok_or(AbstractSatisfiabilityHoldReason::ForeignFormula)?,
        ),
        FormulaKind::Atom {
            role: AtomRole::UninterpretedAction,
        } => Err(AbstractSatisfiabilityHoldReason::UnsupportedActionAtom),
        FormulaKind::Negation {
            kind: NegationKind::Classical,
            operand,
        } => lower_abstract_condition_formula(
            task,
            limits,
            residuals,
            condition_reported_state,
            scope_work,
            visited_nodes,
            active,
            atoms,
            *operand,
            next_abstract_depth(depth)?,
        )
        .map(|operand| BoolExpr::Not(Box::new(operand))),
        FormulaKind::Negation {
            kind: NegationKind::Default,
            ..
        } => Err(AbstractSatisfiabilityHoldReason::UnsupportedDefaultNegation),
        FormulaKind::All(children) => lower_abstract_boolean_group(
            task,
            limits,
            residuals,
            condition_reported_state,
            scope_work,
            visited_nodes,
            active,
            atoms,
            children,
            depth,
            true,
        ),
        FormulaKind::Any(children) => lower_abstract_boolean_group(
            task,
            limits,
            residuals,
            condition_reported_state,
            scope_work,
            visited_nodes,
            active,
            atoms,
            children,
            depth,
            false,
        ),
        FormulaKind::Directive { .. } => {
            Err(AbstractSatisfiabilityHoldReason::UnsupportedDirective)
        }
        FormulaKind::Guarded {
            relation: ConditionRelation::Temporal,
            ..
        } => Err(AbstractSatisfiabilityHoldReason::UnsupportedConditionRelation),
        FormulaKind::Guarded { .. } => Err(AbstractSatisfiabilityHoldReason::UnsupportedGuard),
        FormulaKind::Unresolved => Err(AbstractSatisfiabilityHoldReason::UnresolvedFormula),
    }
}

fn abstract_condition_atom(
    task: &LogicalTask,
    limits: SmtEmitLimits,
    residuals: &[AbstractConditionResidualBinding],
    condition_reported_state: ConditionEvidenceState,
    scope_work: &mut AbstractScopeWork,
    atoms: &mut Vec<AbstractConditionAtomBinding>,
    id: FormulaId,
) -> Result<BoolExpr, AbstractSatisfiabilityHoldReason> {
    let formula = task
        .program_ir()
        .formula(id)
        .map_err(|_| AbstractSatisfiabilityHoldReason::ForeignFormula)?;
    let source_span = formula.span();
    if task.source_store().slice(source_span).is_err() {
        return Err(AbstractSatisfiabilityHoldReason::SourceSpanMismatch);
    }
    let mut atom_residual_count = 0_usize;
    for residual in residuals {
        scope_work.charge(1)?;
        if residual.reason() == LogicalResidualReason::UninterpretedPredicate
            && residual.formula_id() == id
            && residual.source_span() == source_span
        {
            atom_residual_count = atom_residual_count
                .checked_add(1)
                .ok_or(AbstractSatisfiabilityHoldReason::AmbiguousConditionResidual)?;
        }
    }
    if atom_residual_count == 0 {
        return Err(AbstractSatisfiabilityHoldReason::UninterpretedPredicateNotBound);
    }
    if atom_residual_count > 1 {
        return Err(AbstractSatisfiabilityHoldReason::AmbiguousConditionResidual);
    }
    for binding in atoms.iter() {
        scope_work.charge(1)?;
        if binding.formula_id() == id {
            return Ok(BoolExpr::Variable(binding.symbol()));
        }
    }
    if atoms.len() >= limits.max_boolean_symbols() {
        return Err(AbstractSatisfiabilityHoldReason::BooleanSymbolLimitExceeded);
    }
    let mut formula_index = None;
    for (index, candidate) in task.formulas().iter().enumerate() {
        scope_work.charge(1)?;
        if candidate.id() == id {
            formula_index = Some(index);
            break;
        }
    }
    let index = formula_index.ok_or(AbstractSatisfiabilityHoldReason::ForeignFormula)?;
    let index = u32::try_from(index)
        .map_err(|_| AbstractSatisfiabilityHoldReason::FormulaIndexOutOfRange)?;
    if atoms.try_reserve(1).is_err() {
        return Err(AbstractSatisfiabilityHoldReason::AllocationFailed);
    }
    let symbol = BoolSymbolId::new(index);
    atoms.push(AbstractConditionAtomBinding {
        formula: id,
        source_span,
        condition_reported_state,
        symbol,
    });
    Ok(BoolExpr::Variable(symbol))
}

#[allow(clippy::too_many_arguments)]
fn lower_abstract_boolean_group(
    task: &LogicalTask,
    limits: SmtEmitLimits,
    residuals: &[AbstractConditionResidualBinding],
    condition_reported_state: ConditionEvidenceState,
    scope_work: &mut AbstractScopeWork,
    visited_nodes: &mut usize,
    active: &mut Vec<FormulaId>,
    atoms: &mut Vec<AbstractConditionAtomBinding>,
    children: &[FormulaId],
    depth: usize,
    conjunction: bool,
) -> Result<BoolExpr, AbstractSatisfiabilityHoldReason> {
    if children.is_empty() {
        return Err(AbstractSatisfiabilityHoldReason::EmptyBooleanOperator);
    }
    if children.len() > limits.max_nodes().saturating_sub(*visited_nodes) {
        return Err(AbstractSatisfiabilityHoldReason::NodeLimitExceeded);
    }
    if children.len() == 1 {
        return lower_abstract_condition_formula(
            task,
            limits,
            residuals,
            condition_reported_state,
            scope_work,
            visited_nodes,
            active,
            atoms,
            children[0],
            next_abstract_depth(depth)?,
        );
    }
    let mut lowered = Vec::new();
    if lowered.try_reserve(children.len()).is_err() {
        return Err(AbstractSatisfiabilityHoldReason::AllocationFailed);
    }
    for child in children {
        lowered.push(lower_abstract_condition_formula(
            task,
            limits,
            residuals,
            condition_reported_state,
            scope_work,
            visited_nodes,
            active,
            atoms,
            *child,
            next_abstract_depth(depth)?,
        )?);
    }
    Ok(if conjunction {
        BoolExpr::And(lowered)
    } else {
        BoolExpr::Or(lowered)
    })
}

fn next_abstract_depth(depth: usize) -> Result<usize, AbstractSatisfiabilityHoldReason> {
    depth
        .checked_add(1)
        .ok_or(AbstractSatisfiabilityHoldReason::DepthLimitExceeded)
}
