//! Fixed four-case universe and atomic Boolean query admission, not prose semantics.
use super::*;
use lc631_solvers::typed_query::BoolExpr;

pub(super) fn check(
    input: PlanInput<'_>,
) -> Result<([usize; 4], BoolSymbolId, BoolSymbolId), Error> {
    for query in [
        input.root_goal,
        input.source_theory,
        input.target_theory,
        input.source_use,
        input.target_use,
    ] {
        if !query.integer_symbols().is_empty() {
            return Err(Error::Unsupported(Unsupported::Integer));
        }
        if query.boolean_symbols().len() != 3 {
            return Err(Error::Unsupported(Unsupported::BridgeProfile));
        }
    }
    let source = input.source_theory.boolean_symbols();
    let target = input.target_theory.boolean_symbols();
    if input.root_goal.boolean_symbols() != source
        || input.source_use.boolean_symbols() != source
        || input.target_use.boolean_symbols() != target
        || literal(input.root_goal)? != input.source_hard
        || literal(input.source_theory)? != input.source_hard
        || literal(input.target_theory)? != input.target_hard
    {
        return Err(Error::InvalidInput);
    }
    let p = literal(input.source_use)?;
    let u = literal(input.target_use)?;
    if p == input.source_hard || u == input.target_hard {
        return Err(Error::InvalidInput);
    }
    let (expected, _) = program::mapping(source, target, input.expected_mapping)?;
    if !expected
        .iter()
        .any(|m| m.source == input.source_hard && m.target == input.target_hard)
        || !expected.iter().any(|m| m.source == p && m.target == u)
    {
        return Err(Error::InvalidInput);
    }
    let q = source
        .iter()
        .copied()
        .find(|s| *s != p && *s != input.source_hard)
        .ok_or(Error::InvalidInput)?;
    let order = cases(input.cases, source, input.source_hard, p, q)?;
    Ok((order, p, u))
}
fn literal(query: &SmtStatusQuery) -> Result<BoolSymbolId, Error> {
    match query.assertions() {
        [BoolExpr::Variable(symbol)] => Ok(*symbol),
        _ => Err(Error::Unsupported(Unsupported::UseFormula)),
    }
}
fn cases(
    cases: &[ProbeCase<'_>],
    symbols: &[BoolSymbolId],
    hard: BoolSymbolId,
    p: BoolSymbolId,
    q: BoolSymbolId,
) -> Result<[usize; 4], Error> {
    if cases.len() != 4 {
        return Err(Error::InvalidInput);
    }
    let mut seen = [false; 4];
    let mut order = [0usize; 4];
    for (index, case) in cases.iter().enumerate() {
        if case.weight != 1 || case.input.len() != 3 {
            return Err(Error::InvalidInput);
        }
        for (i, assignment) in case.input.iter().enumerate() {
            if !symbols.contains(&assignment.symbol)
                || case.input[..i]
                    .iter()
                    .any(|a| a.symbol == assignment.symbol)
            {
                return Err(Error::InvalidInput);
            }
        }
        if !value(case.input, hard)? {
            return Err(Error::InvalidInput);
        }
        let key = usize::from(value(case.input, p)?) * 2 + usize::from(value(case.input, q)?);
        if seen[key] {
            return Err(Error::InvalidInput);
        }
        seen[key] = true;
        order[key] = index;
    }
    // Four unique keys in 0..4 establish exact coverage, not inferred semantic independence.
    Ok(order)
}
fn value(frame: &[Assignment], symbol: BoolSymbolId) -> Result<bool, Error> {
    frame
        .iter()
        .find(|a| a.symbol == symbol)
        .map(|a| a.value)
        .ok_or(Error::InvalidInput)
}
pub(super) fn schedule(schedule: &[Dispatch]) -> Result<(), Error> {
    if schedule.len() > 4 {
        return Err(Error::InvalidInput);
    }
    let mut seen = [false; 4];
    for item in schedule {
        let visited = seen.get_mut(item.case).ok_or(Error::InvalidInput)?;
        if *visited {
            return Err(Error::InvalidInput);
        }
        *visited = true;
    }
    Ok(())
}
