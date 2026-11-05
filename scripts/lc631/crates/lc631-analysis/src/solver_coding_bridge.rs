//! 有限モデル内の条件検査。自然言語の意味対応と実行権限は未検証。
//! Finite-model condition checks; source interpretation and authority stay unverified.
//!
//! This preparatory adapter is not the TLCX-28 production producer. It checks
//! the actual borrowed logical predicate under explicit model premises, then
//! compares a caller-reported result. Reports do not authenticate execution.
//! No receipt, execution permit, closure token, or source-truth constructor
//! exists here. The legacy condition/completion pipeline remains unchanged.

use crate::logical_lowering::{
    AtomRole, FormulaId, FormulaKind, LogicProfile, LogicalTask, NegationKind, TaskObligationKind,
};
use lc631_core::context::TaskKind;

/// 既存DGCL入力とCoding taskを対応付ける。証拠・権限は発行しない。
/// Bind existing DGCL input to coding tasks without issuing evidence or authority.
pub mod dgcl_input;

const MAX_ATOMS: usize = 16;
const MAX_DEPTH: usize = 64;
pub const MAX_CONDITION_CHECK_STEPS: usize = 1_000_000;
pub const FINITE_CONDITION_REVISION: &str = "OperationalMeasurement/finite-formal-v1.4";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionBridgeError {
    MissingCondition,
    ForeignFormula,
    ForeignReport,
    UnsupportedLogic,
    UnsupportedFormula,
    AtomLimit,
    DepthLimit,
    BudgetExhausted,
    InvalidBudget,
    NoObligations,
    WrongTaskEvidence,
}

/// Source-bound borrowed predicate; not a verified semantic projection.
pub struct ConditionRequest<'task> {
    task: &'task LogicalTask,
    index: usize,
    predicate: FormulaId,
}

impl<'task> ConditionRequest<'task> {
    pub fn new(task: &'task LogicalTask, index: usize) -> Result<Self, ConditionBridgeError> {
        if task.logic_profile() != LogicProfile::ClassicalPropositionalV1 {
            return Err(ConditionBridgeError::UnsupportedLogic);
        }
        let predicate = task
            .conditions()
            .get(index)
            .ok_or(ConditionBridgeError::MissingCondition)?
            .formula();
        task.program_ir()
            .formula(predicate)
            .map_err(|_| ConditionBridgeError::ForeignFormula)?;
        Ok(Self {
            task,
            index,
            predicate,
        })
    }

    pub const fn predicate(&self) -> FormulaId {
        self.predicate
    }
    pub const fn condition_index(&self) -> usize {
        self.index
    }
    pub const fn task(&self) -> &LogicalTask {
        self.task
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportedRelation {
    Sat,
    Unsat,
    ExistentialSupport,
    UniversalSupport,
    UniversalRefutation,
    Undetermined,
    ContextInconsistent,
    Unknown,
    Timeout,
    Unsupported,
    Unavailable,
}

/// A detached solver claim retains its binding, but is never execution evidence.
/// No Deserialize or public fields allow promotion into ConditionObservation.
pub struct ConditionReport<'task> {
    task: &'task LogicalTask,
    predicate: FormulaId,
    index: usize,
    relation: ReportedRelation,
}

impl<'task> ConditionReport<'task> {
    pub fn reported(request: &ConditionRequest<'task>, relation: ReportedRelation) -> Self {
        Self {
            task: request.task,
            predicate: request.predicate,
            index: request.index,
            relation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FiniteSupport {
    Supported,
    Refuted,
    Undetermined,
    ContextInconsistent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportCheck {
    Agrees,
    Disagrees,
    WrongResultKind,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceConditionState {
    NeedsEvidence,
}

/// Exact bounded enumeration of this formal model only. Borrowing retains the
/// source, predicate, premises and reported result without minting a receipt.
pub struct ConditionObservation<'input, 'task> {
    request: &'input ConditionRequest<'task>,
    premises: &'input [(FormulaId, bool)],
    report: &'input ConditionReport<'task>,
    support: FiniteSupport,
    report_check: ReportCheck,
    satisfying: u32,
    assignments: u32,
}

impl ConditionObservation<'_, '_> {
    pub const fn support(&self) -> FiniteSupport {
        self.support
    }
    pub const fn report_relation(&self) -> ReportCheck {
        self.report_check
    }
    pub const fn satisfying_assignments(&self) -> u32 {
        self.satisfying
    }
    pub const fn enumerated_assignments(&self) -> u32 {
        self.assignments
    }
    pub const fn source_condition_state(&self) -> SourceConditionState {
        SourceConditionState::NeedsEvidence
    }
    pub fn request(&self) -> &ConditionRequest<'_> {
        self.request
    }
    pub fn explicit_model_premises(&self) -> &[(FormulaId, bool)] {
        self.premises
    }
    pub const fn reported_relation(&self) -> ReportedRelation {
        self.report.relation
    }
}

/// Enumerate all assignments to the distinct atoms actually reached from the
/// predicate and explicit premises. Premises are Given/Assumed model inputs,
/// never observed source facts. Every tree visit consumes one caller step.
pub fn check_condition<'input, 'task>(
    request: &'input ConditionRequest<'task>,
    premises: &'input [(FormulaId, bool)],
    report: &'input ConditionReport<'task>,
    max_steps: usize,
) -> Result<ConditionObservation<'input, 'task>, ConditionBridgeError> {
    if max_steps > MAX_CONDITION_CHECK_STEPS {
        return Err(ConditionBridgeError::InvalidBudget);
    }
    if !std::ptr::eq(request.task, report.task)
        || request.predicate != report.predicate
        || request.index != report.index
    {
        return Err(ConditionBridgeError::ForeignReport);
    }
    let mut evaluator = Evaluator {
        task: request.task,
        atoms: [None; MAX_ATOMS],
        count: 0,
        steps: max_steps,
    };
    evaluator.collect(request.predicate, 0)?;
    for (formula, _) in premises {
        evaluator.collect(*formula, 0)?;
    }
    let assignments = 1u32
        .checked_shl(evaluator.count as u32)
        .ok_or(ConditionBridgeError::AtomLimit)?;
    let (mut satisfying, mut yes, mut no) = (0, false, false);
    for assignment in 0..assignments {
        let mut admissible = true;
        for (formula, expected) in premises {
            // Evaluate all premises; a false premise cannot hide invalid input.
            admissible &= evaluator.evaluate(*formula, assignment, 0)? == *expected;
        }
        let holds = evaluator.evaluate(request.predicate, assignment, 0)?;
        if admissible {
            satisfying += 1;
            yes |= holds;
            no |= !holds;
        }
    }
    let support = match (yes, no) {
        (true, false) => FiniteSupport::Supported,
        (false, true) => FiniteSupport::Refuted,
        (true, true) => FiniteSupport::Undetermined,
        (false, false) => FiniteSupport::ContextInconsistent,
    };
    Ok(ConditionObservation {
        request,
        premises,
        report,
        support,
        satisfying,
        assignments,
        report_check: compare_report(report.relation, support),
    })
}

fn compare_report(report: ReportedRelation, support: FiniteSupport) -> ReportCheck {
    let expected = match report {
        ReportedRelation::Sat | ReportedRelation::Unsat | ReportedRelation::ExistentialSupport => {
            return ReportCheck::WrongResultKind
        }
        ReportedRelation::Unknown
        | ReportedRelation::Timeout
        | ReportedRelation::Unsupported
        | ReportedRelation::Unavailable => {
            return ReportCheck::Unavailable;
        }
        ReportedRelation::UniversalSupport => FiniteSupport::Supported,
        ReportedRelation::UniversalRefutation => FiniteSupport::Refuted,
        ReportedRelation::Undetermined => FiniteSupport::Undetermined,
        ReportedRelation::ContextInconsistent => FiniteSupport::ContextInconsistent,
    };
    if support == expected {
        ReportCheck::Agrees
    } else {
        ReportCheck::Disagrees
    }
}

struct Evaluator<'task> {
    task: &'task LogicalTask,
    atoms: [Option<FormulaId>; MAX_ATOMS],
    count: usize,
    steps: usize,
}

impl Evaluator<'_> {
    fn visit(&mut self, depth: usize) -> Result<(), ConditionBridgeError> {
        if depth >= MAX_DEPTH {
            return Err(ConditionBridgeError::DepthLimit);
        }
        self.steps = self
            .steps
            .checked_sub(1)
            .ok_or(ConditionBridgeError::BudgetExhausted)?;
        Ok(())
    }

    fn collect(&mut self, id: FormulaId, depth: usize) -> Result<(), ConditionBridgeError> {
        self.visit(depth)?;
        let node = self
            .task
            .program_ir()
            .formula(id)
            .map_err(|_| ConditionBridgeError::ForeignFormula)?;
        match node.kind() {
            FormulaKind::Atom {
                role: AtomRole::UninterpretedCondition,
            } => {
                if !self.atoms[..self.count].contains(&Some(id)) {
                    if self.count == MAX_ATOMS {
                        return Err(ConditionBridgeError::AtomLimit);
                    }
                    self.atoms[self.count] = Some(id);
                    self.count += 1;
                }
            }
            FormulaKind::Negation {
                kind: NegationKind::Classical,
                operand,
            } => {
                self.collect(*operand, depth + 1)?;
            }
            FormulaKind::All(children) | FormulaKind::Any(children) => {
                if children.is_empty() {
                    return Err(ConditionBridgeError::UnsupportedFormula);
                }
                for child in children {
                    self.collect(*child, depth + 1)?;
                }
            }
            FormulaKind::Atom {
                role: AtomRole::UninterpretedAction,
            }
            | FormulaKind::Negation {
                kind: NegationKind::Default,
                ..
            }
            | FormulaKind::Directive { .. }
            | FormulaKind::Guarded { .. }
            | FormulaKind::Unresolved => return Err(ConditionBridgeError::UnsupportedFormula),
        }
        Ok(())
    }

    fn evaluate(
        &mut self,
        id: FormulaId,
        assignment: u32,
        depth: usize,
    ) -> Result<bool, ConditionBridgeError> {
        self.visit(depth)?;
        let node = self
            .task
            .program_ir()
            .formula(id)
            .map_err(|_| ConditionBridgeError::ForeignFormula)?;
        match node.kind() {
            FormulaKind::Atom {
                role: AtomRole::UninterpretedCondition,
            } => {
                let bit = self.atoms[..self.count]
                    .iter()
                    .position(|item| *item == Some(id))
                    .ok_or(ConditionBridgeError::ForeignFormula)?;
                Ok(assignment & (1u32 << bit) != 0)
            }
            FormulaKind::Negation {
                kind: NegationKind::Classical,
                operand,
            } => Ok(!self.evaluate(*operand, assignment, depth + 1)?),
            FormulaKind::All(children) => {
                let mut value = true;
                for child in children {
                    value &= self.evaluate(*child, assignment, depth + 1)?;
                }
                Ok(value)
            }
            FormulaKind::Any(children) => {
                let mut value = false;
                for child in children {
                    value |= self.evaluate(*child, assignment, depth + 1)?;
                }
                Ok(value)
            }
            FormulaKind::Atom {
                role: AtomRole::UninterpretedAction,
            }
            | FormulaKind::Negation {
                kind: NegationKind::Default,
                ..
            }
            | FormulaKind::Directive { .. }
            | FormulaKind::Guarded { .. }
            | FormulaKind::Unresolved => Err(ConditionBridgeError::UnsupportedFormula),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskValidationRoute {
    TaskSpecificEvidence,
    ExistingFiveKindCodingValidation,
}

/// Consume the existing TLCX-09 task plan. This selects a validation family,
/// never completes evidence tasks or authorizes Cargo. Only a selected coding
/// implementation may later enter existing current-authority producers.
pub fn task_validation_route(
    task: &LogicalTask,
) -> Result<TaskValidationRoute, ConditionBridgeError> {
    if task.obligations().is_empty() {
        return Err(ConditionBridgeError::NoObligations);
    }
    let expected: &[TaskObligationKind] = match task.task_kind() {
        TaskKind::Mathematics => &[TaskObligationKind::MathematicalDerivation],
        TaskKind::Reasoning => &[TaskObligationKind::LogicalConsequence],
        TaskKind::Explanation => &[TaskObligationKind::ExplanationCorrespondence],
        TaskKind::Media => &[TaskObligationKind::SourceTimeRegionBinding],
        TaskKind::Coding => &[
            TaskObligationKind::ImplementationBinding,
            TaskObligationKind::ProductionConnection,
            TaskObligationKind::StaticValidation,
            TaskObligationKind::RuntimeValidation,
            TaskObligationKind::AcceptanceTest,
        ],
    };
    for obligation in task.obligations() {
        task.program_ir()
            .formula(obligation.target())
            .map_err(|_| ConditionBridgeError::ForeignFormula)?;
        if !expected.contains(&obligation.kind()) {
            return Err(ConditionBridgeError::WrongTaskEvidence);
        }
    }
    for kind in expected {
        if !task.obligations().iter().any(|item| item.kind() == *kind) {
            return Err(ConditionBridgeError::WrongTaskEvidence);
        }
    }
    Ok(if task.task_kind() == TaskKind::Coding {
        TaskValidationRoute::ExistingFiveKindCodingValidation
    } else {
        TaskValidationRoute::TaskSpecificEvidence
    })
}
