use super::{CheckedModel, ModelValidationError};
use crate::typed_query::{BoolExpr, IntExpr};

pub(super) fn boolean(
    expression: &BoolExpr,
    model: &CheckedModel<'_>,
) -> Result<bool, ModelValidationError> {
    match expression {
        BoolExpr::Constant(value) => Ok(*value),
        BoolExpr::Variable(symbol) => model
            .boolean(*symbol)
            .ok_or(ModelValidationError::MissingAssignment),
        BoolExpr::Not(inner) => Ok(!boolean(inner, model)?),
        BoolExpr::And(children) => {
            let mut result = true;
            for child in children {
                result &= boolean(child, model)?;
            }
            Ok(result)
        }
        BoolExpr::Or(children) => {
            let mut result = false;
            for child in children {
                result |= boolean(child, model)?;
            }
            Ok(result)
        }
        BoolExpr::Equal(left, right) => Ok(integer(left, model)? == integer(right, model)?),
        BoolExpr::LessEqual(left, right) => Ok(integer(left, model)? <= integer(right, model)?),
        BoolExpr::LessThan(left, right) => Ok(integer(left, model)? < integer(right, model)?),
    }
}

pub(super) fn integer(
    expression: &IntExpr,
    model: &CheckedModel<'_>,
) -> Result<i128, ModelValidationError> {
    match expression {
        IntExpr::Constant(value) => Ok(i128::from(*value)),
        IntExpr::Variable(symbol) => model
            .integer(*symbol)
            .ok_or(ModelValidationError::MissingAssignment),
        IntExpr::Add(children) => children.iter().try_fold(0i128, |sum, child| {
            sum.checked_add(integer(child, model)?)
                .ok_or(ModelValidationError::ArithmeticOverflow)
        }),
        IntExpr::Subtract(left, right) => integer(left, model)?
            .checked_sub(integer(right, model)?)
            .ok_or(ModelValidationError::ArithmeticOverflow),
        IntExpr::Negate(inner) => integer(inner, model)?
            .checked_neg()
            .ok_or(ModelValidationError::ArithmeticOverflow),
    }
}
