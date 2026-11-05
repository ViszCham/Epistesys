//! Validate bounded AST/weights BEFORE hashing, cloning, comparing or sorting.
use super::*;
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtStatusQuery};
pub(crate) fn check_weight(weight: &Rational) -> Result<(), Error> {
    numeric::check(weight).map_err(Error::Numeric)?;
    if weight <= &Rational::from_integer(0.into()) {
        return Err(Error::InvalidInput(InvalidInput::InvalidWeight));
    }
    Ok(())
}
pub(crate) fn weight_sum<'a>(
    weights: impl Iterator<Item = &'a Rational>,
) -> Result<Rational, Error> {
    let mut sum = Rational::from_integer(0.into());
    for weight in weights {
        sum = numeric::checked_add(&sum, weight).map_err(Error::Numeric)?;
    }
    Ok(sum)
}
pub(crate) fn formula(
    expr: &BoolExpr,
    symbols: &[BoolSymbolId],
    nodes: &mut usize,
    depth: usize,
) -> Result<(), Error> {
    if depth > 16 {
        return Err(Error::InvalidInput(InvalidInput::DepthLimit));
    }
    *nodes = nodes
        .checked_add(1)
        .ok_or(Error::InvalidInput(InvalidInput::SizeLimit))?;
    if *nodes > 256 {
        return Err(Error::InvalidInput(InvalidInput::SizeLimit));
    }
    match expr {
        BoolExpr::Constant(_) => Ok(()),
        BoolExpr::Variable(id) => {
            if symbols.contains(id) {
                Ok(())
            } else {
                Err(Error::InvalidInput(InvalidInput::UndeclaredAtom))
            }
        }
        BoolExpr::Not(child) => formula(child, symbols, nodes, depth + 1),
        BoolExpr::And(children) | BoolExpr::Or(children) => {
            if children.is_empty() || children.len() > 8 {
                return Err(Error::InvalidInput(InvalidInput::InvalidArity));
            }
            for child in children {
                formula(child, symbols, nodes, depth + 1)?;
            }
            Ok(())
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            Err(Error::UnsupportedFragment)
        }
    }
}
pub(crate) fn admit_case(
    key: &[u8],
    theory: &SmtStatusQuery,
    goal: &BoolExpr,
    weight: &Rational,
) -> Result<(), Error> {
    if key.is_empty()
        || key.len() > 4096
        || theory.boolean_symbols().len() > 8
        || theory.assertions().is_empty()
        || theory.assertions().len() > 8
    {
        return Err(Error::InvalidInput(InvalidInput::SizeLimit));
    }
    if !theory.integer_symbols().is_empty() {
        return Err(Error::UnsupportedFragment);
    }
    let mut nodes = 0;
    for assertion in theory.assertions() {
        formula(assertion, theory.boolean_symbols(), &mut nodes, 0)?;
    }
    formula(goal, theory.boolean_symbols(), &mut nodes, 0)?;
    check_weight(weight)
}
pub(crate) fn admit_submission(
    theory: &SmtStatusQuery,
    submission: Submission<'_>,
) -> Result<(), Error> {
    let certificate = match submission {
        Submission::Missing => return Ok(()),
        Submission::Certificate(c) => c,
    };
    if certificate.steps.len() > 64 {
        return Err(Error::InvalidInput(InvalidInput::SizeLimit));
    }
    let mut nodes = 0;
    for (index, step) in certificate.steps.iter().enumerate() {
        formula(step.conclusion, theory.boolean_symbols(), &mut nodes, 0)?;
        match step.rule {
            Rule::Premise { index } => {
                if usize::from(index) >= theory.assertions().len() {
                    return Err(Error::InvalidInput(InvalidInput::PremiseIndex));
                }
            }
            Rule::AndElim { prior, child } => {
                prior_index(index, prior)?;
                if let BoolExpr::And(children) = certificate.steps[usize::from(prior.0)].conclusion
                {
                    if usize::from(child) >= children.len() {
                        return Err(Error::InvalidInput(InvalidInput::ChildIndex));
                    }
                }
            }
            Rule::AndIntro { priors } => {
                if priors.is_empty() || priors.len() > 8 {
                    return Err(Error::InvalidInput(InvalidInput::InvalidArity));
                }
                for prior in priors {
                    prior_index(index, *prior)?;
                }
            }
        }
    }
    Ok(())
}
fn prior_index(index: usize, prior: StepId) -> Result<(), Error> {
    let prior = usize::from(prior.0);
    if prior == index {
        return Err(Error::InvalidInput(InvalidInput::Cycle));
    }
    if prior > index {
        return Err(Error::InvalidInput(InvalidInput::ForwardReference));
    }
    Ok(())
}
