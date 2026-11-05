//! Closed finite programs generate real certificates, then the SAME forward checker.
use super::*;
use k::certificate::{equal, CaseBudget};
use lc631_solvers::typed_query::{BoolExpr, SmtStatusQuery};
enum Stop {
    NoDerivation,
    Work,
    Limit,
    Allocation,
    Input(Error),
}
pub(super) fn run(
    case: TransferCase<'_>,
    b: e::Binding,
    program: Program,
    limit: u64,
) -> Result<types::Outcome, Error> {
    let mut budget = CaseBudget::new(limit);
    if k::consistent(case.theory, b, &mut budget)
        .map_err(mapped)?
        .is_none()
    {
        return unknown(Gap::WorkLimit);
    }
    let result = generate(case, program, &mut budget);
    let certificate = match result {
        Ok(trace) => trace,
        Err(Stop::Work) => return unknown(Gap::WorkLimit),
        Err(Stop::Limit) => return unknown(Gap::ProgramLimit),
        Err(Stop::Allocation) => return Err(Error::AllocationFailed),
        Err(Stop::Input(error)) => return Err(error),
        Err(Stop::NoDerivation) => {
            return Ok(types::Outcome {
                state: CaseState::NoDerivationInThisProgram,
                node: ScoreNode::state(e::CaseState::CompletedInvalid(
                    e::InvalidDerivation::RuleMismatch,
                ))
                .map_err(mapped)?,
                certificate: None,
            })
        }
    };
    let state = {
        let mut steps = k::vector(certificate.len()).map_err(mapped)?;
        for s in &certificate {
            let rule = match &s.rule {
                e::OwnedRule::Premise { index } => e::Rule::Premise { index: *index },
                e::OwnedRule::AndElim { prior, child } => e::Rule::AndElim {
                    prior: *prior,
                    child: *child,
                },
                e::OwnedRule::AndIntro { priors } => e::Rule::AndIntro { priors },
            };
            steps.push(e::Step {
                conclusion: &s.conclusion,
                rule,
            });
        }
        let submission = e::Submission::Certificate(e::Certificate { steps: &steps });
        k::admit_submission(case.theory, submission).map_err(mapped)?;
        k::replay(case.theory, case.goal, submission, &mut budget).map_err(mapped)?
    };
    match state {
        e::CaseState::Pass => Ok(types::Outcome {
            state: CaseState::Pass,
            node: ScoreNode::state(state).map_err(mapped)?,
            certificate: Some(certificate),
        }),
        e::CaseState::Unknown(_) => unknown(Gap::WorkLimit),
        e::CaseState::CompletedInvalid(_) => Err(Error::BindingMismatch),
    }
}
fn unknown(gap: Gap) -> Result<types::Outcome, Error> {
    Ok(types::Outcome {
        state: CaseState::Unknown(gap),
        node: ScoreNode::state(e::CaseState::Unknown(e::Gap::WorkLimit)).map_err(mapped)?,
        certificate: None,
    })
}
fn generate(
    case: TransferCase<'_>,
    program: Program,
    budget: &mut CaseBudget,
) -> Result<Vec<e::OwnedStep>, Stop> {
    let mut order = Vec::new();
    order
        .try_reserve_exact(case.theory.assertions().len())
        .map_err(|_| Stop::Allocation)?;
    for (i, expr) in case.theory.assertions().iter().enumerate() {
        let cost = nodes(expr).checked_mul(64).ok_or(Stop::Limit)?;
        if !budget.take(cost as u64) {
            return Err(Stop::Work);
        }
        order.push((
            split::order_key(expr, case.theory.boolean_symbols()).map_err(Stop::Input)?,
            i,
        ));
    }
    order.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    let mut trace = Vec::new();
    trace.try_reserve_exact(64).map_err(|_| Stop::Allocation)?;
    let mut builder = Builder {
        theory: case.theory,
        order,
        trace,
        nodes: 0,
        budget,
    };
    builder.derive(case.goal, program, 0)?;
    Ok(builder.trace)
}
struct Builder<'t, 'b> {
    theory: &'t SmtStatusQuery,
    order: Vec<(Vec<u8>, usize)>,
    trace: Vec<e::OwnedStep>,
    nodes: usize,
    budget: &'b mut CaseBudget,
}
impl Builder<'_, '_> {
    fn append(&mut self, conclusion: &BoolExpr, rule: e::OwnedRule) -> Result<e::StepId, Stop> {
        let count = nodes(conclusion);
        let after = self.nodes.checked_add(count).ok_or(Stop::Limit)?;
        if self.trace.len() >= 64 || after > 256 {
            return Err(Stop::Limit);
        }
        if !self.budget.take(count as u64) {
            return Err(Stop::Work);
        }
        let id = u16::try_from(self.trace.len()).map_err(|_| Stop::Limit)?;
        // Whole bounded AST and cumulative output admitted BEFORE this clone.
        self.trace.push(e::OwnedStep {
            conclusion: conclusion.clone(),
            rule,
        });
        self.nodes = after;
        Ok(e::StepId(id))
    }
    fn derive(
        &mut self,
        goal: &BoolExpr,
        program: Program,
        depth: usize,
    ) -> Result<e::StepId, Stop> {
        if depth > 8 {
            return Err(Stop::Limit);
        }
        if !self.budget.tick() {
            return Err(Stop::Work);
        }
        if let Some(id) = self.extract(goal)? {
            return Ok(id);
        }
        let (Program::ComposeV1, BoolExpr::And(children)) = (program, goal) else {
            return Err(Stop::NoDerivation);
        };
        let mut priors = Vec::new();
        priors
            .try_reserve_exact(children.len())
            .map_err(|_| Stop::Allocation)?;
        for child in children {
            priors.push(self.derive(child, program, depth + 1)?);
        }
        self.append(goal, e::OwnedRule::AndIntro { priors })
    }
    fn extract(&mut self, goal: &BoolExpr) -> Result<Option<e::StepId>, Stop> {
        for position in 0..self.order.len() {
            let index = self.order[position].1;
            let mut path = Vec::new();
            path.try_reserve_exact(8).map_err(|_| Stop::Allocation)?;
            if !find(
                &self.theory.assertions()[index],
                goal,
                &mut path,
                self.budget,
                0,
            )? {
                continue;
            }
            let theory = self.theory;
            let mut formula = &theory.assertions()[index];
            let mut prior = self.append(formula, e::OwnedRule::Premise { index: index as u8 })?;
            for child in path {
                let BoolExpr::And(children) = formula else {
                    return Err(Stop::NoDerivation);
                };
                formula = &children[usize::from(child)];
                prior = self.append(formula, e::OwnedRule::AndElim { prior, child })?;
            }
            return Ok(Some(prior));
        }
        Ok(None)
    }
}
fn find(
    expr: &BoolExpr,
    goal: &BoolExpr,
    path: &mut Vec<u8>,
    budget: &mut CaseBudget,
    depth: usize,
) -> Result<bool, Stop> {
    if depth > 8 {
        return Err(Stop::Limit);
    }
    if equal(expr, goal, budget).ok_or(Stop::Work)? {
        return Ok(true);
    }
    if let BoolExpr::And(children) = expr {
        if depth >= 8 {
            return Err(Stop::Limit);
        }
        for (i, child) in children.iter().enumerate() {
            path.push(i as u8);
            if find(child, goal, path, budget, depth + 1)? {
                return Ok(true);
            }
            path.pop();
        }
    }
    Ok(false)
}
fn nodes(expr: &BoolExpr) -> usize {
    match expr {
        BoolExpr::Not(child) => 1 + nodes(child),
        BoolExpr::And(children) | BoolExpr::Or(children) => {
            1 + children.iter().map(nodes).sum::<usize>()
        }
        _ => 1,
    }
}
