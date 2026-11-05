//! Actual private finite reports; no reported outcome/status enters this producer.
use super::*;
use lc631_solvers::typed_query::SmtEmitLimits;

#[derive(Clone, Copy)]
pub(super) enum PairKind {
    Hard(usize, ContextSide),
    Probe(usize, ContextSide),
}
pub(super) struct PairSpec<'a> {
    pub query: &'a SmtStatusQuery,
    pub predicate: &'a BoolExpr,
    pub context: ContextRevision,
    pub allowance: u64,
    pub kind: PairKind,
}
pub(super) struct PairObservation<'a> {
    pub evaluations: Evaluations<'a>,
    pub outcome: CanonicalOutcome,
    pub kind: PairKind,
}
pub(super) fn request<'a>(
    query: &'a SmtStatusQuery,
    before: &'a BoolExpr,
    after: &'a BoolExpr,
    context: ContextRevision,
    map: &'a [(BoolSymbolId, BoolSymbolId)],
    probes: Digest32,
) -> Result<f::Request<'a>, Error> {
    let mut request = f::Request {
        source: query,
        target: query,
        source_predicate: before,
        target_predicate: after,
        symbols: map,
        unconstrained: f::UnconstrainedPolicy::ExplicitlyGiven,
        binding: f::Binding {
            source: context.source(),
            target: context.source(),
            source_context: context,
            target_context: context,
            probe_set: probes,
            probe_map: Digest32::from_bytes([0; 32]),
        },
    };
    // Metadata hashing uses its explicit bounded cap after root payload/AST
    // admission. It does not consume or replace the f query's allowance (1 may
    // therefore produce a real Unknown). Neither meter is a global CPU quota.
    request.binding.probe_map = f::probe_map_digest(&request, 1_000_000).map_err(Error::Formal)?;
    Ok(request)
}
pub(super) fn derive(
    positive: &f::Evaluation<'_>,
    negative: &f::Evaluation<'_>,
) -> Result<CanonicalOutcome, Error> {
    use f::{DirectionResult as D, Evaluation as E};
    for value in [positive, negative] {
        if let E::NoComparable { reason, .. } = value {
            return Err(Error::NoComparable(*reason));
        }
    }
    if [positive, negative]
        .iter()
        .any(|value| matches!(value, E::Unknown { .. }))
    {
        return Ok(CanonicalOutcome::Unknown(OutcomeGap::WorkExhausted));
    }
    match (positive, negative) {
        (E::Computed(p), E::Computed(n)) => match (p.completeness(), n.completeness()) {
            (D::Preserved, D::Countermodel(_)) => Ok(CanonicalOutcome::Supported),
            (D::Countermodel(_), D::Preserved) => Ok(CanonicalOutcome::Refuted),
            (D::Countermodel(_), D::Countermodel(_)) => Ok(CanonicalOutcome::Undetermined),
            (D::Preserved, D::Preserved) => Err(Error::InvalidPlan),
        },
        _ => Err(Error::InvalidPlan), // Exceptions were handled above; never a zero fallback.
    }
}

/// All request/negative/Evaluation backing stays local through this HRTB callback.
/// Query-count updates describe actual f calls, including budget Unknown.
pub(super) fn with_batch<R>(
    specs: &[PairSpec<'_>],
    map: &[(BoolSymbolId, BoolSymbolId)],
    probe_set: Digest32,
    work: &mut util::Work,
    consume: impl for<'scope> FnOnce(&[PairObservation<'scope>], &mut util::Work) -> Result<R, Error>,
) -> Result<R, Error> {
    let mut negatives = util::vector(specs.len())?;
    for item in specs {
        negatives.push(BoolExpr::Not(Box::new(util::clone_expr(item.predicate)?)));
    }
    let truth = BoolExpr::Constant(true);
    let mut requests = util::vector(specs.len() * 2)?;
    for (item, negative) in specs.iter().zip(&negatives) {
        requests.push(request(
            item.query,
            &truth,
            item.predicate,
            item.context,
            map,
            probe_set,
        )?);
        requests.push(request(
            item.query,
            &truth,
            negative,
            item.context,
            map,
            probe_set,
        )?);
    }
    let mut results = util::vector(requests.len())?;
    for (index, item) in requests.iter().enumerate() {
        work.invoked()?;
        results.push(f::evaluate(item, specs[index / 2].allowance).map_err(Error::Formal)?);
    }
    let mut observations = util::vector(specs.len())?;
    for (index, item) in specs.iter().enumerate() {
        let positive = &results[index * 2];
        let negative = &results[index * 2 + 1];
        observations.push(PairObservation {
            evaluations: (positive, negative),
            outcome: derive(positive, negative)?,
            kind: item.kind,
        });
    }
    consume(&observations, work)
}

/// Empty-theory equivalence only. This never asks whether the actual Γ is
/// consistent or establishes source-language semantics during plan capture.
pub(super) fn equivalent(
    a: &BoolExpr,
    b: &BoolExpr,
    symbols: &[BoolSymbolId],
    context: ContextRevision,
    map: &[(BoolSymbolId, BoolSymbolId)],
    allowance: u64,
) -> Result<bool, Error> {
    let mut ids = util::vector(symbols.len())?;
    ids.extend_from_slice(symbols);
    let empty = SmtStatusQuery::checked(ids, Vec::new(), Vec::new(), SmtEmitLimits::default())
        .map_err(|e| Error::Creative(CreativeError::Query(e)))?;
    let r = request(&empty, a, b, context, map, Digest32::from_bytes([0; 32]))?;
    match f::evaluate(&r, allowance).map_err(Error::Formal)? {
        f::Evaluation::Computed(report) => Ok(report.soundness() == f::DirectionResult::Preserved
            && report.completeness() == f::DirectionResult::Preserved),
        f::Evaluation::Unknown { .. } => Err(Error::WorkBudget),
        f::Evaluation::NoComparable { reason, .. } => Err(Error::NoComparable(reason)),
    }
}
