use super::CheckedModel;
use crate::typed_query::{BoolExpr, IntExpr, IntSymbolId, QueryEmitError, SmtStatusQuery};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectiveDirection {
    Minimize,
    Maximize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptimizationProbeError {
    UndeclaredObjective,
    IncumbentOutOfRange,
    AllocationFailed,
    Query(QueryEmitError),
}

/// A strictly-better feasibility query for one declared integer objective.
/// Construction does not run a backend, bound a gap, detect unboundedness or
/// claim optimality. Repeated SAT improvement can continue until a budget;
/// lack of improvement evidence is never proof that an optimum exists.
pub struct OptimizationProbe<'model, 'query> {
    incumbent_model: &'model CheckedModel<'query>,
    objective: IntSymbolId,
    direction: ObjectiveDirection,
    incumbent: i64,
    better_query: SmtStatusQuery,
}

impl<'model, 'query> OptimizationProbe<'model, 'query> {
    pub fn checked(
        model: &'model CheckedModel<'query>,
        objective: IntSymbolId,
        direction: ObjectiveDirection,
    ) -> Result<Self, OptimizationProbeError> {
        let value = model
            .integer(objective)
            .ok_or(OptimizationProbeError::UndeclaredObjective)?;
        let incumbent =
            i64::try_from(value).map_err(|_| OptimizationProbeError::IncumbentOutOfRange)?;
        let query = model.query();
        let mut assertions = Vec::new();
        let count =
            query
                .assertions()
                .len()
                .checked_add(1)
                .ok_or(OptimizationProbeError::Query(
                    QueryEmitError::ArithmeticOverflow,
                ))?;
        if count > query.limits().max_assertions() {
            return Err(OptimizationProbeError::Query(
                QueryEmitError::TooManyAssertions,
            ));
        }
        assertions
            .try_reserve_exact(count)
            .map_err(|_| OptimizationProbeError::AllocationFailed)?;
        assertions.extend(query.assertions().iter().cloned());
        let variable = IntExpr::Variable(objective);
        let constant = IntExpr::Constant(incumbent);
        assertions.push(match direction {
            ObjectiveDirection::Minimize => BoolExpr::LessThan(variable, constant),
            ObjectiveDirection::Maximize => BoolExpr::LessThan(constant, variable),
        });
        let booleans = copy_slice(query.boolean_symbols())?;
        let integers = copy_slice(query.integer_symbols())?;
        let better_query = SmtStatusQuery::checked(booleans, integers, assertions, query.limits())
            .map_err(OptimizationProbeError::Query)?;
        Ok(Self {
            incumbent_model: model,
            objective,
            direction,
            incumbent,
            better_query,
        })
    }

    pub fn incumbent_model(&self) -> &CheckedModel<'query> {
        self.incumbent_model
    }

    pub const fn objective(&self) -> IntSymbolId {
        self.objective
    }

    pub const fn direction(&self) -> ObjectiveDirection {
        self.direction
    }

    pub const fn incumbent(&self) -> i64 {
        self.incumbent
    }

    pub fn better_query(&self) -> &SmtStatusQuery {
        &self.better_query
    }
}

fn copy_slice<T: Copy>(source: &[T]) -> Result<Vec<T>, OptimizationProbeError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(source.len())
        .map_err(|_| OptimizationProbeError::AllocationFailed)?;
    result.extend_from_slice(source);
    Ok(result)
}
