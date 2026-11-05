use crate::{
    context_lowering::{LoweredRegion, LoweredResidual},
    ConditionEvidenceState, ConditionRelation, DgclInstructionParse, InstructionConjunction,
    InstructionModality, InstructionPolarity,
};
use lc631_core::context::{ContextRevision, TaskKind};
use lc631_substrate::{
    source::{SourceSpan, SourceStore},
    Digest32,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicProfile {
    ClassicalPropositionalV1,
    AspGroundNormalV1,
}

impl LogicProfile {
    pub const fn revision(self) -> &'static str {
        match self {
            Self::ClassicalPropositionalV1 => "controlled-classical-conditions.v1",
            Self::AspGroundNormalV1 => "controlled-asp-conditions.v1",
        }
    }
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::ClassicalPropositionalV1 => 0,
            Self::AspGroundNormalV1 => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NegationKind {
    Classical,
    Default,
}

/// Private owner/index fields prevent callers constructing or rebinding IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormulaId {
    pub(super) owner: Digest32,
    pub(super) index: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomRole {
    UninterpretedAction,
    UninterpretedCondition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormulaKind {
    Atom {
        role: AtomRole,
    },
    Negation {
        kind: NegationKind,
        operand: FormulaId,
    },
    All(Vec<FormulaId>),
    Any(Vec<FormulaId>),
    Directive {
        action: FormulaId,
        modality: InstructionModality,
        polarity: InstructionPolarity,
    },
    Guarded {
        relation: ConditionRelation,
        predicate: FormulaId,
        body: FormulaId,
    },
    Unresolved,
}

#[derive(Clone, Debug)]
pub struct LogicalFormula {
    pub(super) id: FormulaId,
    pub(super) span: SourceSpan,
    pub(super) kind: FormulaKind,
}
impl LogicalFormula {
    pub const fn id(&self) -> FormulaId {
        self.id
    }
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
    pub fn kind(&self) -> &FormulaKind {
        &self.kind
    }
}

#[derive(Clone, Debug)]
pub struct LogicalCondition {
    pub(super) span: SourceSpan,
    pub(super) formula: FormulaId,
    pub(super) reported_state: ConditionEvidenceState,
}
impl LogicalCondition {
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
    pub const fn formula(&self) -> FormulaId {
        self.formula
    }
    /// Parser-reported support is advisory, never a verification token.
    pub const fn reported_state(&self) -> ConditionEvidenceState {
        self.reported_state
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicalResidualReason {
    UnknownCondition,
    UnresolvedCondition,
    UnresolvedReference,
    UnsupportedSource,
    AmbiguousParse,
    UnresolvedParse,
    UninterpretedPredicate,
    UnresolvedCoordination,
}
#[derive(Clone, Debug)]
pub struct LogicalResidual {
    pub(super) span: SourceSpan,
    pub(super) reason: LogicalResidualReason,
}
impl LogicalResidual {
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
    pub const fn reason(&self) -> LogicalResidualReason {
        self.reason
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskObligationKind {
    MathematicalDerivation,
    LogicalConsequence,
    ExplanationCorrespondence,
    SourceTimeRegionBinding,
    ImplementationBinding,
    ProductionConnection,
    StaticValidation,
    RuntimeValidation,
    AcceptanceTest,
}
impl TaskObligationKind {
    pub const fn is_coding_validation(self) -> bool {
        matches!(
            self,
            Self::ImplementationBinding
                | Self::ProductionConnection
                | Self::StaticValidation
                | Self::RuntimeValidation
                | Self::AcceptanceTest
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LogicalDirective {
    pub target: FormulaId,
    pub span: SourceSpan,
    pub condition_indices: Vec<usize>,
    pub needs_interpretation: bool,
    pub applicability: TaskObligationApplicability,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskObligationApplicability {
    Required,
    Conditional,
    AlternativePending,
    Unresolved,
}

pub struct LogicalRelation {
    pub(super) left: FormulaId,
    pub(super) right: FormulaId,
    pub(super) kind: InstructionConjunction,
    pub(super) span: SourceSpan,
}
impl LogicalRelation {
    pub const fn left(&self) -> FormulaId {
        self.left
    }
    pub const fn right(&self) -> FormulaId {
        self.right
    }
    pub const fn kind(&self) -> InstructionConjunction {
        self.kind
    }
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

pub struct TaskProgramIr {
    pub(crate) owner: Digest32,
    pub(crate) context: ContextRevision,
    pub(crate) context_identity: Digest32,
    pub(crate) task: TaskKind,
    pub(crate) profile: LogicProfile,
    pub(crate) legacy_program_digest: String,
    pub(crate) parser_grammar_revision: &'static str,
    pub(crate) formulas: Vec<LogicalFormula>,
    pub(crate) directives: Vec<LogicalDirective>,
    pub(crate) relations: Vec<LogicalRelation>,
}
impl TaskProgramIr {
    pub fn legacy_program_digest(&self) -> &str {
        &self.legacy_program_digest
    }
    pub fn formulas(&self) -> &[LogicalFormula] {
        &self.formulas
    }
    pub fn relations(&self) -> &[LogicalRelation] {
        &self.relations
    }
    pub const fn parser_grammar_revision(&self) -> &'static str {
        self.parser_grammar_revision
    }
    pub fn formula(&self, id: FormulaId) -> Result<&LogicalFormula, LogicalLoweringError> {
        if id.owner != self.owner
            || self.owner
                != super::formula_owner(
                    self.context,
                    self.context_identity,
                    self.task,
                    self.profile,
                    &self.legacy_program_digest,
                    self.parser_grammar_revision,
                )
            || self.context
                != ContextRevision::original(self.context.source())
                    .reconstructed(*self.context_identity.as_bytes())
        {
            return Err(LogicalLoweringError::ForeignFormula);
        }
        self.formulas
            .get(id.index as usize)
            .ok_or(LogicalLoweringError::ForeignFormula)
    }
}

pub struct TaskObligation {
    pub(crate) kind: TaskObligationKind,
    pub(crate) target: FormulaId,
    pub(crate) span: SourceSpan,
    pub(crate) condition_indices: Vec<usize>,
    pub(crate) needs_interpretation: bool,
    pub(crate) applicability: TaskObligationApplicability,
}
impl TaskObligation {
    pub const fn kind(&self) -> TaskObligationKind {
        self.kind
    }
    pub const fn target(&self) -> FormulaId {
        self.target
    }
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
    pub fn condition_indices(&self) -> &[usize] {
        &self.condition_indices
    }
    pub const fn needs_interpretation(&self) -> bool {
        self.needs_interpretation
    }
    pub const fn applicability(&self) -> TaskObligationApplicability {
        self.applicability
    }
}

pub struct TaskCompletionPlan {
    pub(crate) obligations: Vec<TaskObligation>,
}
impl TaskCompletionPlan {
    pub fn obligations(&self) -> &[TaskObligation] {
        &self.obligations
    }
    /// Non-vacuity only. This does not establish that any obligation is met.
    pub fn require_nonempty(&self) -> Result<(), LogicalLoweringError> {
        if self.obligations.is_empty() {
            return Err(LogicalLoweringError::NoTaskObligations);
        }
        Ok(())
    }
}

pub struct LogicalTask {
    pub(super) source: SourceStore,
    pub(super) program: TaskProgramIr,
    pub(super) plan: TaskCompletionPlan,
    pub(super) conditions: Vec<LogicalCondition>,
    pub(super) residuals: Vec<LogicalResidual>,
    pub(super) partition: Vec<SourceSpan>,
    pub(super) source_regions: Vec<LoweredRegion>,
    pub(super) source_residuals: Vec<LoweredResidual>,
    pub(super) advisory_parses: Vec<DgclInstructionParse>,
}
impl LogicalTask {
    pub fn source_store(&self) -> &SourceStore {
        &self.source
    }
    pub const fn context_revision(&self) -> ContextRevision {
        self.program.context
    }
    pub const fn task_kind(&self) -> TaskKind {
        self.program.task
    }
    pub const fn logic_profile(&self) -> LogicProfile {
        self.program.profile
    }
    pub fn program_ir(&self) -> &TaskProgramIr {
        &self.program
    }
    pub fn completion_plan(&self) -> &TaskCompletionPlan {
        &self.plan
    }
    pub fn formulas(&self) -> &[LogicalFormula] {
        self.program.formulas()
    }
    pub fn conditions(&self) -> &[LogicalCondition] {
        &self.conditions
    }
    pub fn obligations(&self) -> &[TaskObligation] {
        self.plan.obligations()
    }
    pub fn residuals(&self) -> &[LogicalResidual] {
        &self.residuals
    }
    pub fn source_partition(&self) -> &[SourceSpan] {
        &self.partition
    }
    pub fn source_regions(&self) -> &[LoweredRegion] {
        &self.source_regions
    }
    pub fn source_residuals(&self) -> &[LoweredResidual] {
        &self.source_residuals
    }
    pub fn advisory_parses(&self) -> &[DgclInstructionParse] {
        &self.advisory_parses
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LogicalLoweringBudget {
    pub max_nodes: usize,
    pub max_obligations: usize,
    pub max_residuals: usize,
    pub max_depth: usize,
}
impl Default for LogicalLoweringBudget {
    fn default() -> Self {
        Self {
            max_nodes: 8192,
            max_obligations: 8192,
            max_residuals: 8192,
            max_depth: 64,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicalLoweringError {
    PipelineIdentity,
    SourceMismatch,
    SourceSpan,
    InvalidBudget,
    BudgetExceeded,
    AllocationFailed,
    InvalidAst,
    ForeignFormula,
    NoTaskObligations,
}
