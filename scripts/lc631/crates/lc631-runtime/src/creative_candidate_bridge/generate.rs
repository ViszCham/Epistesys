//! Typed operators do not evaluate truth; the separate SMT model checker does.
use super::{
    admission::{clone_expr, Counter},
    claim, vector, Arc, BoolExpr, Connective, CreativeCandidateBridge, CreativeError, Operator,
    ResourceReservationView, SmtStatusQuery, ValidatedFormalCandidate, CANDIDATE_OUTPUT,
    CANDIDATE_RAM, VALIDATION_SCRATCH_RAM,
};
use lc631_analysis::creative_candidates::{CreativeOperator, ProposalProvider};
use lc631_solvers::{runner::QuerySourceIdentity, typed_query::SmtEmitLimits};

impl<'input, 'view> CreativeCandidateBridge<'input, 'view> {
    pub fn generate(
        &mut self,
        operator: Operator,
        resources: &'view ResourceReservationView<'_>,
    ) -> Result<ValidatedFormalCandidate<'input, 'view>, CreativeError> {
        self.check_current(resources)?;
        if self.proposals().len() >= self.limits.max_candidates {
            return Err(CreativeError::Budget);
        }
        let scratch = claim(resources, VALIDATION_SCRATCH_RAM, None)?;
        let retained = claim(resources, CANDIDATE_RAM, Some(CANDIDATE_OUTPUT))?;
        let query = build_query(self, operator)?;
        let prepared = query
            .emit(QuerySourceIdentity::from_bytes(
                *self.source.id().digest().as_bytes(),
            ))
            .map_err(CreativeError::Query)?;
        let mut payload = vector(prepared.payload().len())?;
        payload.extend_from_slice(prepared.payload());
        let validation =
            super::validator::check(&query, self.goal.query, self.limits.max_validation_checks)?;
        let text = std::str::from_utf8(&payload).map_err(|_| CreativeError::UnsupportedFragment)?;
        let replacement = Arc::<str>::from(text);
        scratch.check().map_err(CreativeError::Resource)?;
        retained.check().map_err(CreativeError::Resource)?;
        self.check_current(resources)?;
        let anchor = self
            .bindings
            .iter()
            .find(|b| b.id == self.target)
            .ok_or(CreativeError::MissingPremise)?
            .span;
        let parent = self.contexts().original().state().revision();
        // Only a candidate checked by the separate model-evaluation algorithm reaches
        // the context consumer. Shared IR/source mapping remain unverified.
        // The change remains Proposed and does not promote the source/AST binding.
        let generated = self
            .preparation
            .prepare(
                self.source,
                anchor,
                parent,
                ProposalProvider::DeterministicOperator,
                CreativeOperator::RevisePremise {
                    target: self.target,
                    replacement,
                },
            )
            .map_err(CreativeError::Preparation)?;
        Ok(ValidatedFormalCandidate {
            query,
            payload,
            validation,
            prepared: generated,
            operator,
            goal: self.goal,
            target: self.target,
            owner: Arc::clone(&self.owner),
            retained,
        })
    }
}
fn build_query(
    bridge: &CreativeCandidateBridge<'_, '_>,
    operator: Operator,
) -> Result<SmtStatusQuery, CreativeError> {
    let mut count = Counter::new(bridge.limits.max_ast_nodes);
    let base = bridge
        .bindings
        .iter()
        .find(|b| b.id == bridge.target)
        .ok_or(CreativeError::MissingPremise)?
        .formula;
    let transformed = transform(bridge, base, operator, &mut count)?;
    let mut assertions = vector(bridge.bindings.len())?;
    for binding in bridge.bindings {
        if binding.id != bridge.target {
            assertions.push(clone_expr(binding.formula, &mut count, 1)?);
        }
    }
    assertions.push(transformed);
    let mut symbols = vector(bridge.goal.query.boolean_symbols().len())?;
    symbols.extend_from_slice(bridge.goal.query.boolean_symbols());
    let limits = SmtEmitLimits::checked(
        bridge.limits.max_payload_bytes,
        symbols.len(),
        0,
        bridge.bindings.len(),
        bridge.limits.max_ast_nodes,
        32,
    )
    .map_err(CreativeError::Query)?;
    SmtStatusQuery::checked(symbols, Vec::new(), assertions, limits).map_err(CreativeError::Query)
}
fn transform(
    bridge: &CreativeCandidateBridge<'_, '_>,
    base: &BoolExpr,
    operator: Operator,
    count: &mut Counter,
) -> Result<BoolExpr, CreativeError> {
    match operator {
        Operator::ReframeDeMorgan => {
            let children = match base {
                BoolExpr::And(children) | BoolExpr::Or(children) => children,
                _ => return Err(CreativeError::UnsupportedOperator),
            };
            count.visit(1)?;
            count.visit(2)?;
            let mut inner = vector(children.len())?;
            for child in children {
                count.visit(3)?;
                inner.push(BoolExpr::Not(Box::new(clone_expr(child, count, 4)?)));
            }
            let opposite = if matches!(base, BoolExpr::And(_)) {
                BoolExpr::Or(inner)
            } else {
                BoolExpr::And(inner)
            };
            Ok(BoolExpr::Not(Box::new(opposite)))
        }
        Operator::Recombine {
            partner,
            connective,
        } => {
            let other = bridge
                .bindings
                .iter()
                .find(|b| b.id == partner)
                .ok_or(CreativeError::MissingPremise)?
                .formula;
            count.visit(1)?;
            let mut children = vector(2)?;
            children.push(clone_expr(base, count, 2)?);
            children.push(clone_expr(other, count, 2)?);
            match connective {
                Connective::And => Ok(BoolExpr::And(children)),
                Connective::Or => Ok(BoolExpr::Or(children)),
            }
        }
    }
}
