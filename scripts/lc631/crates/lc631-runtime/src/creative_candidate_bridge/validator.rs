//! Separate finite checker component; does not call typed generation operators.
//! Text here is INTERNALLY ENUMERATED ASSIGNMENT INPUT, never solver stdout.
use super::{CreativeError, FiniteValidation, SmtStatusQuery};
use lc631_solvers::protocol::sexpr::{parse, SExprLimits};
use lc631_solvers::smt::{validate_model, ModelValidationError};
use lc631_solvers::typed_query::BoolSymbolId;
use std::fmt::Write;

pub(super) fn check(
    candidate: &SmtStatusQuery,
    goal: &SmtStatusQuery,
    max_checks: u32,
) -> Result<FiniteValidation, CreativeError> {
    if candidate.boolean_symbols() != goal.boolean_symbols()
        || !candidate.integer_symbols().is_empty()
        || !goal.integer_symbols().is_empty()
        || candidate.boolean_symbols().len() > 8
    {
        return Err(CreativeError::UnsupportedFragment);
    }
    let assignments = 1u16 << candidate.boolean_symbols().len();
    if u32::from(assignments) * 2 > max_checks {
        return Err(CreativeError::Budget);
    }
    let limits = SExprLimits::checked(1024, 128, 8).map_err(CreativeError::Syntax)?;
    let mut feasible = 0u16;
    for assignment in 0..assignments {
        let input = assignment_input(candidate.boolean_symbols(), assignment)?;
        let document = parse(input.as_bytes(), limits).map_err(CreativeError::Syntax)?;
        match validate_model(candidate, &document) {
            Ok(_) => {}
            Err(ModelValidationError::AssertionFalse { .. }) => continue,
            Err(error) => return Err(CreativeError::Model(error)),
        }
        match validate_model(goal, &document) {
            Ok(_) => {}
            Err(ModelValidationError::AssertionFalse { .. }) => {
                return Err(CreativeError::GoalCounterexample { assignment })
            }
            Err(error) => return Err(CreativeError::Model(error)),
        }
        feasible += 1; // <=256 enumerated assignments, so u16 cannot overflow.
    }
    if feasible == 0 {
        return Err(CreativeError::NoFeasibleModels);
    }
    Ok(FiniteValidation {
        assignments,
        feasible,
    })
}
fn assignment_input(symbols: &[BoolSymbolId], assignment: u16) -> Result<String, CreativeError> {
    let mut input = String::new();
    input
        .try_reserve_exact(1024)
        .map_err(|_| CreativeError::AllocationFailed)?;
    if input.capacity() > 1024 {
        return Err(CreativeError::AllocationFailed);
    }
    input.push_str("(model");
    // <=8 numeric u32 names and fixed grammar fit in 1024 bytes. No caller
    // symbols/text are interpolated, and no command/status/process receipt exists.
    for (bit, symbol) in symbols.iter().enumerate() {
        let value = if assignment & (1u16 << bit) != 0 {
            "true"
        } else {
            "false"
        };
        write!(input, " (define-fun b{} () Bool {})", symbol.get(), value)
            .map_err(|_| CreativeError::AllocationFailed)?;
    }
    input.push(')');
    if input.len() > 1024 {
        return Err(CreativeError::Budget);
    }
    Ok(input)
}
