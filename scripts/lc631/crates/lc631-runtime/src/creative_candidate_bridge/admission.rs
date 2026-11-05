//! Input admission for the declared source-to-Boolean mapping; not a prose parser.
use super::{
    BoolExpr, BridgeLimits, CreativeError, Digest32, GivenPremise, GoalPolicy, OriginalContext,
    SourceStore,
};

pub(super) fn check(
    source: &SourceStore,
    original: &OriginalContext,
    target: Digest32,
    bindings: &[GivenPremise<'_>],
    goal: GoalPolicy<'_>,
    limits: BridgeLimits,
) -> Result<(), CreativeError> {
    if limits.max_candidates == 0
        || limits.max_candidates > 16
        || limits.max_ast_nodes == 0
        || limits.max_ast_nodes > 512
        || limits.max_validation_checks == 0
        || limits.max_validation_checks > 100_000
        || limits.max_payload_bytes == 0
        || limits.max_payload_bytes > 8192
        || bindings.is_empty()
        || bindings.len() > 16
        || original.limits().max_contexts > 17
        || original.limits().max_premises > 16
        || original.limits().max_statement_bytes > 8192
    {
        return Err(CreativeError::InvalidConfig);
    }
    if goal.revision != original.state().scope().goal() {
        return Err(CreativeError::ScopeMismatch);
    }
    if goal.query.boolean_symbols().len() > 8
        || !goal.query.integer_symbols().is_empty()
        || goal.query.assertions().is_empty()
    {
        return Err(CreativeError::UnsupportedFragment);
    }
    let premises = original.state().premises();
    if bindings.len() != premises.len() {
        return Err(CreativeError::BindingMismatch);
    }
    let mutable = premises
        .iter()
        .find(|p| p.id() == target)
        .ok_or(CreativeError::MissingPremise)?;
    if !mutable.changeable() {
        return Err(CreativeError::HardInvariant);
    }
    let mut count = Counter::new(limits.max_ast_nodes);
    for (index, binding) in bindings.iter().enumerate() {
        if bindings[..index].iter().any(|p| p.id == binding.id) {
            return Err(CreativeError::BindingMismatch);
        }
        let p = premises
            .iter()
            .find(|p| p.id() == binding.id)
            .ok_or(CreativeError::MissingPremise)?;
        if p.source_span() != Some(binding.span)
            || source.slice(binding.span).map_err(CreativeError::Source)? != p.statement()
        {
            return Err(CreativeError::BindingMismatch);
        }
        scan(binding.formula, goal, &mut count, 1)?;
    }
    for assertion in goal.query.assertions() {
        scan(assertion, goal, &mut count, 1)?;
    }
    Ok(())
}
pub(super) struct Counter {
    remaining: usize,
}
impl Counter {
    pub(super) fn new(limit: usize) -> Self {
        Self { remaining: limit }
    }
    pub(super) fn visit(&mut self, depth: usize) -> Result<(), CreativeError> {
        if self.remaining == 0 || depth > 32 {
            return Err(CreativeError::Budget);
        }
        self.remaining -= 1;
        Ok(())
    }
}
fn scan(
    expr: &BoolExpr,
    goal: GoalPolicy<'_>,
    counter: &mut Counter,
    depth: usize,
) -> Result<(), CreativeError> {
    counter.visit(depth)?;
    match expr {
        BoolExpr::Constant(_) => {}
        BoolExpr::Variable(symbol) => {
            if !goal.query.boolean_symbols().contains(symbol) {
                return Err(CreativeError::BindingMismatch);
            }
        }
        BoolExpr::Not(child) => scan(child, goal, counter, depth + 1)?,
        BoolExpr::And(children) | BoolExpr::Or(children) => {
            if children.is_empty() {
                return Err(CreativeError::UnsupportedFragment);
            }
            for child in children {
                scan(child, goal, counter, depth + 1)?;
            }
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            return Err(CreativeError::UnsupportedFragment)
        }
    }
    Ok(())
}
pub(super) fn clone_expr(
    expr: &BoolExpr,
    counter: &mut Counter,
    depth: usize,
) -> Result<BoolExpr, CreativeError> {
    counter.visit(depth)?;
    match expr {
        BoolExpr::Constant(value) => Ok(BoolExpr::Constant(*value)),
        BoolExpr::Variable(symbol) => Ok(BoolExpr::Variable(*symbol)),
        BoolExpr::Not(child) => Ok(BoolExpr::Not(Box::new(clone_expr(
            child,
            counter,
            depth + 1,
        )?))),
        BoolExpr::And(children) | BoolExpr::Or(children) => {
            let mut cloned = super::vector(children.len())?;
            for child in children {
                cloned.push(clone_expr(child, counter, depth + 1)?);
            }
            if matches!(expr, BoolExpr::And(_)) {
                Ok(BoolExpr::And(cloned))
            } else {
                Ok(BoolExpr::Or(cloned))
            }
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            Err(CreativeError::UnsupportedFragment)
        }
    }
}
