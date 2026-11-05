//! Fixed-plan admission over exact AST references and positive canonical weights.
use super::f;
use super::{BridgeError, FixedProbe, PlanScope, ProbePlan, Tracker, WorkBudget};
use lc631_analysis::creativity_measurement::MeasurementError;
use lc631_solvers::typed_query::{BoolExpr, SmtStatusQuery};
use lc631_substrate::measurement_numeric::{check, BigInt};

impl<'a> ProbePlan<'a> {
    pub fn checked(
        scope: PlanScope,
        source: &'a SmtStatusQuery,
        probes: &'a [FixedProbe<'a>],
        budget: WorkBudget,
    ) -> Result<Self, BridgeError> {
        let mut work = Tracker::new(budget)?;
        if probes.is_empty() {
            return Err(BridgeError::EmptyPlan);
        }
        if probes.len() > 256 {
            return Err(BridgeError::ProbeLimit);
        }
        if scope.context.source() != scope.source {
            return Err(BridgeError::LineageMismatch);
        }
        work.bytes(192)?; // Scope digests, source and the complete context identity.
        for (index, probe) in probes.iter().enumerate() {
            work.charge((index + 1) as u64)?;
            if probe.template.target.boolean_symbols()
                != probes[0].template.target.boolean_symbols()
            {
                return Err(BridgeError::DomainMismatch);
            }
            if probe.identity.is_empty() || probe.identity.len() > 4096 {
                return Err(BridgeError::ForeignProbe);
            }
            if probes[..index].iter().any(|p| p.identity == probe.identity) {
                return Err(BridgeError::DuplicateProbe);
            }
            work.bytes(probe.identity.len() as u64 + 265)?; // Length, request binding and policy.
            admit_weight(probe, &mut work)?;
            admit_template(scope, source, probe, &mut work)?;
        }
        Ok(Self {
            scope,
            source,
            probes,
            payload_bytes: work.bytes,
        })
    }
}
fn admit_weight(probe: &FixedProbe<'_>, work: &mut Tracker) -> Result<(), BridgeError> {
    let bytes = probe
        .weight
        .numer()
        .bits()
        .div_ceil(8)
        .checked_add(probe.weight.denom().bits().div_ceil(8))
        .and_then(|value| value.checked_add(17)) // Sign and two bounded lengths.
        .ok_or(BridgeError::PayloadBudget)?;
    work.bytes(bytes)?;
    check(probe.weight).map_err(|e| BridgeError::Arithmetic(MeasurementError::from(e)))?;
    if probe.weight.numer() <= &BigInt::from(0) {
        return Err(BridgeError::InvalidWeight);
    }
    Ok(())
}
fn admit_template(
    scope: PlanScope,
    source: &SmtStatusQuery,
    probe: &FixedProbe<'_>,
    work: &mut Tracker,
) -> Result<(), BridgeError> {
    let r = probe.template;
    if !std::ptr::eq(r.source, source)
        || r.binding.source != scope.source
        || r.binding.source_context != scope.context
        || r.binding.probe_set != scope.probe_set
    {
        return Err(BridgeError::LineageMismatch);
    }
    let before = work.work;
    count_query(r.source, work)?;
    count_query(r.target, work)?;
    count_expr(r.source_predicate, work, 1)?;
    count_expr(r.target_predicate, work, 1)?;
    work.bytes(
        (r.symbols.len() as u64)
            .checked_mul(8)
            .and_then(|value| value.checked_add(8))
            .ok_or(BridgeError::PayloadBudget)?,
    )?;
    let nodes = work.work - before;
    // Reserve the existing formal admission traversal separately; no hidden
    // per-probe million-step allowance or new semantic evaluator is introduced.
    work.charge(nodes)?;
    let digest = f::probe_map_digest(r, nodes.max(1)).map_err(BridgeError::Formal)?;
    if digest != r.binding.probe_map {
        return Err(BridgeError::Formal(f::Failure::ProbeMapMismatch));
    }
    Ok(())
}
pub(super) fn count_query(query: &SmtStatusQuery, work: &mut Tracker) -> Result<(), BridgeError> {
    if query.boolean_symbols().len() > 16 {
        return Err(BridgeError::Formal(f::Failure::AtomLimit));
    }
    if !query.integer_symbols().is_empty() {
        return Err(BridgeError::Formal(f::Failure::UnsupportedInteger));
    }
    work.bytes(16 + query.boolean_symbols().len() as u64 * 4)?;
    for expr in query.assertions() {
        count_expr(expr, work, 1)?;
    }
    Ok(())
}
fn count_expr(expr: &BoolExpr, work: &mut Tracker, depth: u16) -> Result<(), BridgeError> {
    work.charge(1)?;
    if depth > 128 {
        return Err(BridgeError::Formal(f::Failure::DepthLimit));
    }
    match expr {
        BoolExpr::Constant(_) => work.bytes(2)?,
        BoolExpr::Variable(_) => work.bytes(5)?,
        BoolExpr::Not(child) => {
            work.bytes(1)?;
            count_expr(child, work, depth + 1)?;
        }
        BoolExpr::And(children) | BoolExpr::Or(children) => {
            work.bytes(9)?;
            for child in children {
                count_expr(child, work, depth + 1)?;
            }
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            return Err(BridgeError::Formal(f::Failure::UnsupportedInteger));
        }
    }
    Ok(())
}
