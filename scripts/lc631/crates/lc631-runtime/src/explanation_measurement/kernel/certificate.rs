//! Actual forward replay; finite consistency reuses the public existing owner.
use super::*;
use crate::formal_boolean as f;
use lc631_solvers::typed_query::{BoolExpr, SmtStatusQuery};
pub(crate) struct CaseBudget {
    remaining: u64,
}
impl CaseBudget {
    pub(crate) fn new(remaining: u64) -> Self {
        Self { remaining }
    }
    pub(crate) fn take(&mut self, amount: u64) -> bool {
        if amount > self.remaining {
            return false;
        }
        self.remaining -= amount;
        true
    }
    pub(crate) fn tick(&mut self) -> bool {
        self.take(1)
    }
}
pub(crate) fn consistent(
    theory: &SmtStatusQuery,
    b: Binding,
    budget: &mut CaseBudget,
) -> Result<Option<()>, Error> {
    // Reserve whole bounded allowances BEFORE each existing checker invocation.
    // Unused allowance is not refunded; remainder is available to program/replay.
    let allowance = budget.remaining / 3;
    if allowance == 0 {
        return Ok(None);
    }
    let mut symbols = vector(theory.boolean_symbols().len())?;
    for id in theory.boolean_symbols() {
        symbols.push((*id, *id));
    }
    let predicate = BoolExpr::Constant(true);
    let mut request = f::Request {
        source: theory,
        target: theory,
        source_predicate: &predicate,
        target_predicate: &predicate,
        symbols: &symbols,
        unconstrained: f::UnconstrainedPolicy::ExplicitlyGiven,
        binding: f::Binding {
            source: b.selection.source,
            target: b.selection.source,
            source_context: b.context,
            target_context: b.context,
            probe_set: b.probe_set,
            probe_map: digest(b"uncomputed"),
        },
    };
    if !budget.take(allowance) {
        return Ok(None);
    }
    request.binding.probe_map = match f::probe_map_digest(&request, allowance) {
        Ok(value) => value,
        Err(f::Failure::ResourceExhausted) => return Ok(None),
        Err(_) => return Err(Error::UnsupportedFragment),
    };
    if !budget.take(allowance) {
        return Ok(None);
    }
    match f::evaluate(&request, allowance).map_err(|_| Error::UnsupportedFragment)? {
        f::Evaluation::Computed(_) => Ok(Some(())),
        f::Evaluation::Unknown { .. } => Ok(None),
        f::Evaluation::NoComparable { .. } => {
            Err(Error::NoComparable(NoComparable::ContextInconsistent))
        }
    }
}
pub(crate) fn replay(
    theory: &SmtStatusQuery,
    goal: &BoolExpr,
    submission: Submission<'_>,
    budget: &mut CaseBudget,
) -> Result<CaseState, Error> {
    let certificate = match submission {
        Submission::Missing => return Ok(CaseState::Unknown(Gap::MissingSubmission)),
        Submission::Certificate(value) => value,
    };
    if certificate.steps.is_empty() {
        return Ok(CaseState::CompletedInvalid(
            InvalidDerivation::EmptyCertificate,
        ));
    }
    for step in certificate.steps {
        if !budget.tick() {
            return Ok(CaseState::Unknown(Gap::WorkLimit));
        }
        let valid = match step.rule {
            Rule::Premise { index } => equal(
                step.conclusion,
                &theory.assertions()[usize::from(index)],
                budget,
            ),
            Rule::AndElim { prior, child } => {
                match certificate.steps[usize::from(prior.0)].conclusion {
                    BoolExpr::And(children) => {
                        equal(step.conclusion, &children[usize::from(child)], budget)
                    }
                    _ => Some(false),
                }
            }
            Rule::AndIntro { priors } => join(step.conclusion, priors, certificate.steps, budget),
        };
        match valid {
            None => return Ok(CaseState::Unknown(Gap::WorkLimit)),
            Some(false) => return Ok(CaseState::CompletedInvalid(InvalidDerivation::RuleMismatch)),
            Some(true) => {}
        }
    }
    let last = certificate
        .steps
        .last()
        .ok_or(Error::InvalidInput(InvalidInput::SizeLimit))?;
    Ok(match equal(last.conclusion, goal, budget) {
        Some(true) => CaseState::Pass,
        Some(false) => CaseState::CompletedInvalid(InvalidDerivation::GoalMismatch),
        None => CaseState::Unknown(Gap::WorkLimit),
    })
}
fn join(
    conclusion: &BoolExpr,
    priors: &[StepId],
    steps: &[Step<'_>],
    budget: &mut CaseBudget,
) -> Option<bool> {
    let BoolExpr::And(children) = conclusion else {
        return Some(false);
    };
    if priors.len() != children.len() {
        return Some(false);
    }
    for (prior, child) in priors.iter().zip(children) {
        if !equal(steps[usize::from(prior.0)].conclusion, child, budget)? {
            return Some(false);
        }
    }
    Some(true)
}
pub(crate) fn equal(a: &BoolExpr, b: &BoolExpr, budget: &mut CaseBudget) -> Option<bool> {
    if !budget.tick() {
        return None;
    }
    match (a, b) {
        (BoolExpr::Constant(x), BoolExpr::Constant(y)) => Some(x == y),
        (BoolExpr::Variable(x), BoolExpr::Variable(y)) => Some(x == y),
        (BoolExpr::Not(x), BoolExpr::Not(y)) => equal(x, y, budget),
        (BoolExpr::And(x), BoolExpr::And(y)) | (BoolExpr::Or(x), BoolExpr::Or(y)) => {
            if x.len() != y.len() {
                return Some(false);
            }
            for (a, b) in x.iter().zip(y) {
                if !equal(a, b, budget)? {
                    return Some(false);
                }
            }
            Some(true)
        }
        _ => Some(false),
    }
}
