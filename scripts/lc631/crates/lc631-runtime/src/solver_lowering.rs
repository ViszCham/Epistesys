//! Fail-closed projection from the analysis formula IR to a typed SMT query.
//!
//! This is syntax/query lowering only. Boolean satisfiability over
//! uninterpreted symbols is not evidence that the source statement is true,
//! that its atoms have the intended meanings, or that Z3 was actually run.

use lc631_analysis::{
    logical_lowering::{
        AtomRole, FormulaId, FormulaKind, LogicProfile as AnalysisLogicProfile,
        LogicalResidualReason, LogicalTask, NegationKind,
    },
    ConditionEvidenceState, ConditionRelation,
};
use lc631_core::context::{ContextRevision, TaskKind};
use lc631_solvers::{
    runner::{QuerySourceIdentity, SolverQuery},
    typed_query::{
        BoolExpr, BoolSymbolId, PreparedSmtStatusQuery, QueryEmitError, SmtEmitLimits,
        SmtStatusQuery,
    },
};
use lc631_substrate::source::SourceSpan;

/// In-memory identity carried across the lowering boundary. `FormulaId` and
/// `ContextRevision` remain opaque; public APIs do not expose enough owner or
/// interpretation bytes to create a durable serialized owner digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormulaBinding<'task> {
    source: QuerySourceIdentity,
    context: ContextRevision,
    task: TaskKind,
    logic_profile: AnalysisLogicProfile,
    profile_revision: &'static str,
    program_digest: &'task str,
    grammar_revision: &'static str,
    formula: FormulaId,
    formula_span: Option<SourceSpan>,
}

impl<'task> FormulaBinding<'task> {
    pub const fn source_identity(self) -> QuerySourceIdentity {
        self.source
    }

    pub const fn context_revision(self) -> ContextRevision {
        self.context
    }

    pub const fn task_kind(self) -> TaskKind {
        self.task
    }

    pub const fn logic_profile(self) -> AnalysisLogicProfile {
        self.logic_profile
    }

    pub const fn profile_revision(self) -> &'static str {
        self.profile_revision
    }

    pub const fn program_digest(self) -> &'task str {
        self.program_digest
    }

    pub const fn grammar_revision(self) -> &'static str {
        self.grammar_revision
    }

    pub const fn formula_id(self) -> FormulaId {
        self.formula
    }

    pub const fn formula_span(self) -> Option<SourceSpan> {
        self.formula_span
    }
}

/// Why a source-bound request could not be lowered without guessing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormulaHoldReason {
    UnsupportedLogicProfile,
    SourceContextMismatch,
    ForeignFormula,
    EmptyFormulaSet,
    UnsupportedTaskResidual,
    UninterpretedPredicateNotBound,
    UninterpretedPredicateOutsideSelectedFormula,
    SourceSpanMismatch,
    MissingConditionEvidence,
    UnknownConditionEvidence,
    RefutedConditionEvidence,
    ConflictingConditionEvidence,
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
    FormulaIndexOutOfRange,
    AllocationFailed,
    QueryEmission(QueryEmitError),
}

/// A refusal retains the request's source and typed identity for diagnosis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormulaLoweringHold<'task> {
    binding: FormulaBinding<'task>,
    reason: FormulaHoldReason,
}

impl<'task> FormulaLoweringHold<'task> {
    pub const fn binding(self) -> FormulaBinding<'task> {
        self.binding
    }

    pub const fn reason(self) -> FormulaHoldReason {
        self.reason
    }

    pub const fn claim_boundary(self) -> &'static str {
        "source-bound in-memory typed Hold; no durable owner digest, SMT query, solver truth, actual Z3 observation, authority, or output commit"
    }
}

/// One source-bound, prepared syntax query. Bindings map formula identities to
/// opaque Boolean variables; they do not assign semantic meanings or truth.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedFormulaQuery<'task> {
    binding: FormulaBinding<'task>,
    symbols: Vec<FormulaSymbolBinding>,
    opaque_predicates: Vec<OpaquePredicateBinding>,
    query: PreparedSmtStatusQuery,
}

impl<'task> PreparedFormulaQuery<'task> {
    pub const fn binding(&self) -> FormulaBinding<'task> {
        self.binding
    }

    pub fn symbols(&self) -> &[FormulaSymbolBinding] {
        &self.symbols
    }

    pub fn opaque_predicates(&self) -> &[OpaquePredicateBinding] {
        &self.opaque_predicates
    }

    pub const fn prepared_smt(&self) -> &PreparedSmtStatusQuery {
        &self.query
    }

    /// Borrowing this query does not run a solver or validate source truth.
    pub fn solver_query(&self) -> Result<SolverQuery<'_>, QueryEmitError> {
        self.query.solver_query()
    }

    pub const fn claim_boundary(&self) -> &'static str {
        "syntax-only lowering to a bounded Boolean SMT satisfiability request; opaque in-memory binding only, no durable owner digest, source semantics, solver truth, actual Z3 execution, authority, or proof"
    }
}

/// Source spans remain attached to every declared symbol. Opaque predicate
/// residuals are bound only by exact source-span identity to condition atoms;
/// no semantic predicate text or connective is inferred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormulaSymbolBinding {
    formula: FormulaId,
    source_span: SourceSpan,
    reported_state: ConditionEvidenceState,
    symbol: BoolSymbolId,
}

impl FormulaSymbolBinding {
    pub const fn formula_id(self) -> FormulaId {
        self.formula
    }

    pub const fn source_span(self) -> SourceSpan {
        self.source_span
    }

    pub const fn reported_state(self) -> ConditionEvidenceState {
        self.reported_state
    }

    pub const fn symbol(self) -> BoolSymbolId {
        self.symbol
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpaquePredicateBinding {
    residual_index: u32,
    formula: FormulaId,
    source_span: SourceSpan,
}

impl OpaquePredicateBinding {
    pub const fn residual_index(self) -> u32 {
        self.residual_index
    }

    pub const fn formula_id(self) -> FormulaId {
        self.formula
    }

    pub const fn source_span(self) -> SourceSpan {
        self.source_span
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormulaLoweringDecision<'task> {
    Prepared(PreparedFormulaQuery<'task>),
    Hold(FormulaLoweringHold<'task>),
}

mod abstract_sat;
pub use abstract_sat::{
    prepare_abstract_condition_satisfiability, AbstractConditionAtomBinding,
    AbstractConditionGuardBinding, AbstractConditionResidualBinding, AbstractSatisfiabilityBudget,
    AbstractSatisfiabilityDecision, AbstractSatisfiabilityHold, AbstractSatisfiabilityHoldReason,
    AbstractSatisfiabilityInterpretation, PreparedAbstractSatisfiabilityQuery,
    UnvalidatedTaskResidualBinding, MAX_ABSTRACT_SCOPE_WORK,
};

/// Lower one selected formula only when its public identity, evidence states,
/// residuals, and source spans satisfy the conservative projection boundary.
///
/// The currently shipped grammar reports condition evidence as Unknown, so
/// ordinary parsed tasks intentionally produce a typed Hold. The pure mapping
/// below is ready for a future producer of independently supported typed
/// conditions; no public task constructor or synthetic evidence is invented.
pub fn lower_logical_formula<'task>(
    task: &'task LogicalTask,
    formula: FormulaId,
    limits: SmtEmitLimits,
) -> FormulaLoweringDecision<'task> {
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

    let hold = |reason| FormulaLoweringDecision::Hold(FormulaLoweringHold { binding, reason });

    if task.logic_profile() != AnalysisLogicProfile::ClassicalPropositionalV1 {
        return hold(FormulaHoldReason::UnsupportedLogicProfile);
    }
    if task.context_revision().source().digest() != source_digest {
        return hold(FormulaHoldReason::SourceContextMismatch);
    }
    if task.formulas().is_empty() {
        return hold(FormulaHoldReason::EmptyFormulaSet);
    }
    let program = task.program_ir();
    if program.formula(formula).is_err() {
        return hold(FormulaHoldReason::ForeignFormula);
    }
    if !matches!(
        binding.formula_span,
        Some(span) if task.source_store().slice(span).is_ok()
    ) {
        return hold(FormulaHoldReason::SourceSpanMismatch);
    }
    for residual in task.residuals() {
        if task.source_store().slice(residual.span()).is_err() {
            return hold(FormulaHoldReason::SourceSpanMismatch);
        }
    }

    if task.conditions().is_empty() {
        if task
            .residuals()
            .iter()
            .any(|residual| residual.reason() != LogicalResidualReason::UninterpretedPredicate)
        {
            return hold(FormulaHoldReason::UnsupportedTaskResidual);
        }
        return hold(FormulaHoldReason::MissingConditionEvidence);
    }
    for condition in task.conditions() {
        match condition.reported_state() {
            ConditionEvidenceState::Supported => {}
            ConditionEvidenceState::Refuted => {
                return hold(FormulaHoldReason::RefutedConditionEvidence);
            }
            ConditionEvidenceState::Unknown => {
                return hold(FormulaHoldReason::UnknownConditionEvidence);
            }
            ConditionEvidenceState::Conflict => {
                return hold(FormulaHoldReason::ConflictingConditionEvidence);
            }
        }
    }
    if task
        .residuals()
        .iter()
        .any(|residual| residual.reason() != LogicalResidualReason::UninterpretedPredicate)
    {
        return hold(FormulaHoldReason::UnsupportedTaskResidual);
    }

    let mut opaque_predicates = Vec::new();
    if opaque_predicates
        .try_reserve(task.residuals().len())
        .is_err()
    {
        return hold(FormulaHoldReason::AllocationFailed);
    }
    for (residual_index, residual) in task.residuals().iter().enumerate() {
        let Some(formula) = task.formulas().iter().find(|candidate| {
            candidate.span() == residual.span()
                && matches!(
                    candidate.kind(),
                    FormulaKind::Atom {
                        role: AtomRole::UninterpretedCondition
                    }
                )
        }) else {
            return hold(FormulaHoldReason::UninterpretedPredicateNotBound);
        };
        if program.formula(formula.id()).is_err() {
            return hold(FormulaHoldReason::ForeignFormula);
        }
        let Ok(residual_index) = u32::try_from(residual_index) else {
            return hold(FormulaHoldReason::FormulaIndexOutOfRange);
        };
        opaque_predicates.push(OpaquePredicateBinding {
            residual_index,
            formula: formula.id(),
            source_span: residual.span(),
        });
    }

    let mut visited_nodes = 0;
    let mut active = Vec::new();
    let mut symbols = Vec::new();
    let expression = match lower_formula(
        task,
        limits,
        &mut visited_nodes,
        &mut active,
        &mut symbols,
        formula,
        1,
    ) {
        Ok(expression) => expression,
        Err(reason) => return hold(reason),
    };
    if opaque_predicates.iter().any(|predicate| {
        !symbols
            .iter()
            .any(|symbol| symbol.formula_id() == predicate.formula_id())
    }) {
        return hold(FormulaHoldReason::UninterpretedPredicateOutsideSelectedFormula);
    }

    let mut declared_symbols = Vec::new();
    if declared_symbols.try_reserve(symbols.len()).is_err() {
        return hold(FormulaHoldReason::AllocationFailed);
    }
    declared_symbols.extend(symbols.iter().map(|binding| binding.symbol()));

    let mut assertions = Vec::new();
    if assertions.try_reserve(1).is_err() {
        return hold(FormulaHoldReason::AllocationFailed);
    }
    assertions.push(expression);

    let typed = match SmtStatusQuery::checked(declared_symbols, Vec::new(), assertions, limits) {
        Ok(query) => query,
        Err(error) => return hold(FormulaHoldReason::QueryEmission(error)),
    };
    let query = match typed.emit(binding.source_identity()) {
        Ok(query) => query,
        Err(error) => return hold(FormulaHoldReason::QueryEmission(error)),
    };

    FormulaLoweringDecision::Prepared(PreparedFormulaQuery {
        binding,
        symbols,
        opaque_predicates,
        query,
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_formula(
    task: &LogicalTask,
    limits: SmtEmitLimits,
    visited_nodes: &mut usize,
    active: &mut Vec<FormulaId>,
    symbols: &mut Vec<FormulaSymbolBinding>,
    id: FormulaId,
    depth: usize,
) -> Result<BoolExpr, FormulaHoldReason> {
    if depth > limits.max_depth() {
        return Err(FormulaHoldReason::DepthLimitExceeded);
    }
    *visited_nodes = (*visited_nodes)
        .checked_add(1)
        .ok_or(FormulaHoldReason::NodeLimitExceeded)?;
    if *visited_nodes > limits.max_nodes() {
        return Err(FormulaHoldReason::NodeLimitExceeded);
    }
    if active.contains(&id) {
        return Err(FormulaHoldReason::CyclicFormula);
    }
    let formula = task
        .program_ir()
        .formula(id)
        .map_err(|_| FormulaHoldReason::ForeignFormula)?;
    if active.try_reserve(1).is_err() {
        return Err(FormulaHoldReason::AllocationFailed);
    }
    active.push(id);
    let result = lower_kind(
        task,
        limits,
        visited_nodes,
        active,
        symbols,
        formula.kind(),
        id,
        depth,
    );
    active.pop();
    result
}

#[allow(clippy::too_many_arguments)]
fn lower_kind(
    task: &LogicalTask,
    limits: SmtEmitLimits,
    visited_nodes: &mut usize,
    active: &mut Vec<FormulaId>,
    symbols: &mut Vec<FormulaSymbolBinding>,
    kind: &FormulaKind,
    id: FormulaId,
    depth: usize,
) -> Result<BoolExpr, FormulaHoldReason> {
    match kind {
        FormulaKind::Atom {
            role: AtomRole::UninterpretedCondition,
        } => condition_symbol(task, limits, symbols, id),
        FormulaKind::Atom {
            role: AtomRole::UninterpretedAction,
        } => Err(FormulaHoldReason::UnsupportedActionAtom),
        FormulaKind::Negation {
            kind: NegationKind::Classical,
            operand,
        } => lower_formula(
            task,
            limits,
            visited_nodes,
            active,
            symbols,
            *operand,
            next_depth(depth)?,
        )
        .map(|operand| BoolExpr::Not(Box::new(operand))),
        FormulaKind::Negation {
            kind: NegationKind::Default,
            ..
        } => Err(FormulaHoldReason::UnsupportedDefaultNegation),
        FormulaKind::All(children) => lower_boolean_group(
            task,
            limits,
            visited_nodes,
            active,
            symbols,
            children,
            depth,
            true,
        ),
        FormulaKind::Any(children) => lower_boolean_group(
            task,
            limits,
            visited_nodes,
            active,
            symbols,
            children,
            depth,
            false,
        ),
        FormulaKind::Directive { .. } => Err(FormulaHoldReason::UnsupportedDirective),
        FormulaKind::Guarded {
            relation: ConditionRelation::Temporal,
            ..
        } => Err(FormulaHoldReason::UnsupportedCausalSpatialTemporalForm),
        FormulaKind::Guarded { .. } => Err(FormulaHoldReason::UnsupportedGuard),
        FormulaKind::Unresolved => Err(FormulaHoldReason::UnresolvedFormula),
    }
}

fn condition_symbol(
    task: &LogicalTask,
    limits: SmtEmitLimits,
    symbols: &mut Vec<FormulaSymbolBinding>,
    id: FormulaId,
) -> Result<BoolExpr, FormulaHoldReason> {
    let condition = task
        .conditions()
        .iter()
        .find(|condition| condition.formula() == id)
        .ok_or(FormulaHoldReason::MissingConditionEvidence)?;
    if let Some(binding) = symbols.iter().find(|binding| binding.formula_id() == id) {
        return Ok(BoolExpr::Variable(binding.symbol));
    }
    if symbols.len() >= limits.max_boolean_symbols() {
        return Err(FormulaHoldReason::BooleanSymbolLimitExceeded);
    }
    let index = task
        .formulas()
        .iter()
        .position(|formula| formula.id() == id)
        .ok_or(FormulaHoldReason::ForeignFormula)?;
    let index = u32::try_from(index).map_err(|_| FormulaHoldReason::FormulaIndexOutOfRange)?;
    let source_span = task
        .program_ir()
        .formula(id)
        .map_err(|_| FormulaHoldReason::ForeignFormula)?
        .span();
    if task.source_store().slice(source_span).is_err() {
        return Err(FormulaHoldReason::SourceSpanMismatch);
    }
    if symbols.try_reserve(1).is_err() {
        return Err(FormulaHoldReason::AllocationFailed);
    }
    let symbol = BoolSymbolId::new(index);
    symbols.push(FormulaSymbolBinding {
        formula: id,
        source_span,
        reported_state: condition.reported_state(),
        symbol,
    });
    Ok(BoolExpr::Variable(symbol))
}

#[allow(clippy::too_many_arguments)]
fn lower_boolean_group(
    task: &LogicalTask,
    limits: SmtEmitLimits,
    visited_nodes: &mut usize,
    active: &mut Vec<FormulaId>,
    symbols: &mut Vec<FormulaSymbolBinding>,
    children: &[FormulaId],
    depth: usize,
    conjunction: bool,
) -> Result<BoolExpr, FormulaHoldReason> {
    if children.is_empty() {
        return Err(FormulaHoldReason::EmptyBooleanOperator);
    }
    if children.len() > limits.max_nodes().saturating_sub(*visited_nodes) {
        return Err(FormulaHoldReason::NodeLimitExceeded);
    }
    if children.len() == 1 {
        return lower_formula(
            task,
            limits,
            visited_nodes,
            active,
            symbols,
            children[0],
            next_depth(depth)?,
        );
    }
    let mut lowered = Vec::new();
    if lowered.try_reserve(children.len()).is_err() {
        return Err(FormulaHoldReason::AllocationFailed);
    }
    for child in children {
        lowered.push(lower_formula(
            task,
            limits,
            visited_nodes,
            active,
            symbols,
            *child,
            next_depth(depth)?,
        )?);
    }
    Ok(if conjunction {
        BoolExpr::And(lowered)
    } else {
        BoolExpr::Or(lowered)
    })
}

fn next_depth(depth: usize) -> Result<usize, FormulaHoldReason> {
    depth
        .checked_add(1)
        .ok_or(FormulaHoldReason::DepthLimitExceeded)
}
