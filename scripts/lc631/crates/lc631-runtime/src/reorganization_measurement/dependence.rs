//! Private finite input-dependence evidence; no caller result or truth interpreter.
use super::{finite, vector, Error, MapBackInput, PreparedRebindProgram, RebindQueries, UnitId};
use crate::formal_boolean::{
    self as formal, Binding, DirectionResult, Evaluation, NoComparable, Request,
    UnconstrainedPolicy,
};
use lc631_analysis::creativity_measurement::MeasurementGap;
use lc631_substrate::{resource::ResourceReservationView, Digest32};
use sha2::{Digest, Sha256};

pub(super) struct QueryKey {
    unit: UnitId,
    nodes: u64,
    probe_set: Digest32,
    probe_map: Digest32,
}
pub(super) struct MapUseProof<'q> {
    unit: UnitId,
    map: &'q MapBackInput<'q>,
    request: &'q Request<'q>,
    evaluation: Evaluation<'q>,
    charged_work: u64,
}
pub(super) enum UseStatus {
    Dependent,
    Unknown,
    NoComparable,
}

pub(super) fn requests<'g, 'p, 'i, 'v>(
    program: &'g PreparedRebindProgram<'p, 'i, 'v>,
) -> Result<(Vec<Request<'g>>, Vec<QueryKey>), Error> {
    let plan = program.plan;
    let mut requests = vector(plan.probes.len())?;
    let mut keys = vector(plan.probes.len())?;
    let symbols = &program.symbol_map[..program.symbol_count];
    for probe in plan.probes {
        let admission = plan
            .limits
            .max_ast_nodes
            .checked_mul(4)
            .and_then(|n| n.checked_add(1))
            .ok_or(Error::ArithmeticOverflow)?;
        plan.charge(admission)?;
        let (at_false, at_true) = program.input_pair(probe.unit)?;
        let nodes = finite::scan(
            at_false,
            plan.baseline.theory.boolean_symbols(),
            plan.limits.max_ast_nodes,
            plan.limits.max_ast_depth,
        )?
        .checked_add(finite::scan(
            at_true,
            plan.baseline.theory.boolean_symbols(),
            plan.limits.max_ast_nodes,
            plan.limits.max_ast_depth,
        )?)
        .ok_or(Error::ArithmeticOverflow)?;
        let probe_set = query_identity(program.digest, probe.unit);
        let mut request = Request {
            source: plan.baseline.theory,
            target: program.candidate.theory,
            source_predicate: at_false,
            target_predicate: at_true,
            symbols,
            unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
            binding: Binding {
                source: plan.pins.source,
                target: plan.pins.source,
                source_context: plan.baseline.context,
                target_context: program.candidate.context,
                probe_set,
                probe_map: probe_set,
            },
        };
        request.binding.probe_map = formal::probe_map_digest(&request, 4096)?;
        keys.push(QueryKey {
            unit: probe.unit,
            nodes,
            probe_set,
            probe_map: request.binding.probe_map,
        });
        requests.push(request);
    }
    Ok((requests, keys))
}

pub(super) fn check_requests(queries: &RebindQueries<'_, '_, '_, '_>) -> Result<(), Error> {
    let program = queries.program;
    let plan = program.plan;
    if queries.use_requests.is_empty()
        || queries.use_requests.len() != plan.probes.len()
        || queries.use_keys.len() != plan.probes.len()
    {
        return Err(Error::ProbeSetMismatch);
    }
    plan.charge(queries.use_requests.len() as u64)?;
    for ((request, key), probe) in queries
        .use_requests
        .iter()
        .zip(&queries.use_keys)
        .zip(plan.probes)
    {
        let (at_false, at_true) = program.input_pair(probe.unit)?;
        if key.unit != probe.unit
            || key.probe_set != query_identity(program.digest, probe.unit)
            || !std::ptr::eq(request.source_predicate, at_false)
            || !std::ptr::eq(request.target_predicate, at_true)
            || !std::ptr::eq(request.source, plan.baseline.theory)
            || !std::ptr::eq(request.target, program.candidate.theory)
            || !std::ptr::eq(request.symbols, &program.symbol_map[..program.symbol_count])
            || request.unconstrained != UnconstrainedPolicy::ExplicitlyGiven
            || request.binding.source != plan.pins.source
            || request.binding.target != plan.pins.source
            || request.binding.source_context != plan.baseline.context
            || request.binding.target_context != program.candidate.context
            || request.binding.probe_set != key.probe_set
            || request.binding.probe_map != key.probe_map
            || !request.source.assertions().is_empty()
            || !request.target.assertions().is_empty()
        {
            return Err(Error::ForeignQueries);
        }
    }
    Ok(())
}

pub(super) fn evaluate<'q, 'g, 'p, 'i, 'v>(
    queries: &'q RebindQueries<'g, 'p, 'i, 'v>,
    resources: &ResourceReservationView<'_>,
) -> Result<Vec<MapUseProof<'q>>, Error> {
    queries.check(resources)?;
    let plan = queries.program.plan;
    let mut proofs = vector(plan.probes.len())?;
    for (request, key) in queries.use_requests.iter().zip(&queries.use_keys) {
        queries.check(resources)?;
        let cost = key
            .nodes
            .checked_mul(1 + (1u64 << request.symbols.len()))
            .ok_or(Error::ArithmeticOverflow)?;
        let charged_work = cost.min(plan.limits.max_check_work);
        plan.charge(charged_work)?;
        let evaluation = formal::evaluate(request, plan.limits.max_check_work)?;
        // Both preserved directions on the FULL unary-Bool domain mean that
        // the map ignores its input, including masked parameter occurrences.
        let proof = MapUseProof {
            unit: key.unit,
            map: queries.program.map_input(key.unit)?,
            request,
            evaluation,
            charged_work,
        };
        proof.status()?;
        proofs.push(proof);
    }
    validate(&proofs, queries)?;
    Ok(proofs)
}

pub(super) fn validate(
    proofs: &[MapUseProof<'_>],
    queries: &RebindQueries<'_, '_, '_, '_>,
) -> Result<(), Error> {
    if proofs.is_empty()
        || proofs.len() != queries.use_requests.len()
        || proofs.len() != queries.program.plan.probes.len()
    {
        return Err(Error::ProbeSetMismatch);
    }
    queries.program.plan.charge(proofs.len() as u64)?;
    for ((proof, request), key) in proofs
        .iter()
        .zip(&queries.use_requests)
        .zip(&queries.use_keys)
    {
        if proof.unit != key.unit
            || !std::ptr::eq(proof.request, request)
            || !std::ptr::eq(proof.map, queries.program.map_input(key.unit)?)
        {
            return Err(Error::ForeignQueries);
        }
        if let Evaluation::Computed(result) = &proof.evaluation {
            if !std::ptr::eq(result.request(), request) {
                return Err(Error::ForeignQueries);
            }
        }
        proof.status()?;
    }
    Ok(())
}
impl MapUseProof<'_> {
    pub(super) fn unit(&self) -> UnitId {
        self.unit
    }
    pub(super) fn pending(&self) -> bool {
        matches!(
            &self.evaluation,
            Evaluation::Unknown { .. } | Evaluation::NoComparable { .. }
        )
    }
    pub(super) fn status(&self) -> Result<UseStatus, Error> {
        match &self.evaluation {
            Evaluation::Computed(result) => match (result.soundness(), result.completeness()) {
                (DirectionResult::Preserved, DirectionResult::Preserved) => {
                    Err(Error::MissingActualUse)
                }
                (DirectionResult::Countermodel(_), _) | (_, DirectionResult::Countermodel(_)) => {
                    Ok(UseStatus::Dependent)
                }
            },
            Evaluation::Unknown { .. } => Ok(UseStatus::Unknown),
            Evaluation::NoComparable { .. } => Ok(UseStatus::NoComparable),
        }
    }
    pub(super) fn no_comparable_gap(&self) -> Option<MeasurementGap> {
        match &self.evaluation {
            Evaluation::NoComparable {
                reason: NoComparable::MissingTheoryScope,
                ..
            } => Some(MeasurementGap::MissingObservation),
            Evaluation::NoComparable {
                reason:
                    NoComparable::SourceContextInconsistent | NoComparable::TargetContextInconsistent,
                ..
            } => Some(MeasurementGap::Conflict),
            Evaluation::Computed(_) | Evaluation::Unknown { .. } => None,
        }
    }
}

pub(super) fn append_digest(h: &mut Sha256, proofs: &[MapUseProof<'_>]) {
    h.update(b"epistesys.rebind.input-dependence.full-bool.v1\0");
    h.update([0, 1]); // Both admitted counterfactual input values, in fixed order.
    h.update((proofs.len() as u64).to_le_bytes());
    for proof in proofs {
        h.update(proof.unit.0.to_le_bytes());
        h.update(proof.charged_work.to_le_bytes());
        h.update(proof.map.parameter.get().to_le_bytes());
        finite::expression(h, proof.map.body);
        h.update(proof.request.binding.probe_set.as_bytes());
        h.update(proof.request.binding.probe_map.as_bytes());
        match &proof.evaluation {
            Evaluation::Computed(result) => {
                h.update([0]);
                h.update(result.assignments().to_le_bytes());
                h.update(result.ast_visits().to_le_bytes());
                direction(h, result.soundness());
                direction(h, result.completeness());
                h.update(result.source_theory_digest().as_bytes());
                h.update(result.target_theory_digest().as_bytes());
            }
            Evaluation::Unknown { ast_visits } => {
                h.update([1]);
                h.update(ast_visits.to_le_bytes());
            }
            Evaluation::NoComparable { reason, ast_visits } => {
                h.update([
                    2,
                    match reason {
                        NoComparable::MissingTheoryScope => 0,
                        NoComparable::SourceContextInconsistent => 1,
                        NoComparable::TargetContextInconsistent => 2,
                    },
                ]);
                h.update(ast_visits.to_le_bytes());
            }
        }
    }
}
fn query_identity(program: Digest32, unit: UnitId) -> Digest32 {
    let mut h = Sha256::new();
    h.update(b"epistesys.rebind.map-input-query.v1\0");
    h.update(program.as_bytes());
    h.update(unit.0.to_le_bytes());
    h.update([0, 1]);
    finite::finish(h)
}
fn direction(h: &mut Sha256, result: DirectionResult) {
    match result {
        DirectionResult::Preserved => h.update([0]),
        DirectionResult::Countermodel(row) => {
            h.update([1]);
            h.update(row.to_le_bytes());
        }
    }
}
