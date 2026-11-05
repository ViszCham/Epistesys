//! Requests own no AST copies; the prepared program must outlive them.
use super::{claim, dependence, finite, vector, CheckKind, Error, PreparedRebindProgram, UnitId};
use crate::formal_boolean::{self as formal, Binding, Request, UnconstrainedPolicy};
use lc631_solvers::typed_query::BoolSymbolId;
use lc631_substrate::resource::{ResourceClaim, ResourceReservationView};

pub(super) struct QueryKey {
    pub unit: UnitId,
    pub kind: CheckKind,
    pub nodes: u64,
}
pub struct RebindQueries<'g, 'p, 'i, 'v> {
    pub(super) program: &'g PreparedRebindProgram<'p, 'i, 'v>,
    pub(super) requests: Vec<Request<'g>>,
    pub(super) keys: Vec<QueryKey>,
    pub(super) use_requests: Vec<Request<'g>>,
    pub(super) use_keys: Vec<dependence::QueryKey>,
    retained: ResourceClaim<'v>,
}
// Requests need a stable symbol-map backing owned BEFORE Request construction.
// The map itself is a fixed array stored on the prepared program, not self-owned
// Vec references. It is exposed only inside this subtree.
impl<'g, 'p, 'i, 'v> RebindQueries<'g, 'p, 'i, 'v> {
    pub fn capture(
        program: &'g PreparedRebindProgram<'p, 'i, 'v>,
        resources: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        program.check(resources)?;
        let plan = program.plan;
        let retained = claim(resources, plan.limits.queries_ram)?;
        let count = plan
            .probes
            .len()
            .checked_mul(3)
            .ok_or(Error::ArithmeticOverflow)?;
        let storage = count
            .checked_mul(std::mem::size_of::<Request<'g>>() + std::mem::size_of::<QueryKey>())
            .and_then(|n| {
                plan.probes
                    .len()
                    .checked_mul(
                        std::mem::size_of::<Request<'g>>()
                            + std::mem::size_of::<dependence::QueryKey>(),
                    )
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or(Error::ArithmeticOverflow)?;
        if storage as u64 > plan.limits.queries_ram {
            return Err(Error::PayloadLimit);
        }
        let mut requests = vector(count)?;
        let mut keys = vector(count)?;
        let symbols: &[(BoolSymbolId, BoolSymbolId)] = &program.symbol_map[..program.symbol_count];
        for p in plan.probes {
            let old = plan.baseline.binding(p.unit)?;
            let new = program.candidate.binding(p.unit)?;
            for (kind, source, target) in [
                (CheckKind::RoleDifference, old.body, new.body),
                (CheckKind::MapBack, p.root, program.composition(p.unit)?),
                (CheckKind::RootUse, p.root, program.root_use(p.unit)?),
            ] {
                let nodes = finite::scan(
                    source,
                    plan.baseline.theory.boolean_symbols(),
                    plan.limits.max_ast_nodes,
                    plan.limits.max_ast_depth,
                )? + finite::scan(
                    target,
                    plan.baseline.theory.boolean_symbols(),
                    plan.limits.max_ast_nodes,
                    plan.limits.max_ast_depth,
                )?;
                plan.charge(nodes)?;
                let mut request = Request {
                    source: plan.baseline.theory,
                    target: program.candidate.theory,
                    source_predicate: source,
                    target_predicate: target,
                    symbols,
                    unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
                    binding: Binding {
                        source: plan.pins.source,
                        target: plan.pins.source,
                        source_context: plan.baseline.context,
                        target_context: program.candidate.context,
                        probe_set: plan.digest,
                        probe_map: plan.digest,
                    },
                };
                request.binding.probe_map = formal::probe_map_digest(&request, 4096)?;
                requests.push(request);
                keys.push(QueryKey {
                    unit: p.unit,
                    kind,
                    nodes,
                });
            }
        }
        let (use_requests, use_keys) = dependence::requests(program)?;
        let result = Self {
            program,
            requests,
            keys,
            use_requests,
            use_keys,
            retained,
        };
        result.check(resources)?;
        Ok(result)
    }
    pub(super) fn check(&self, resources: &ResourceReservationView<'_>) -> Result<(), Error> {
        if !self.retained.same_reservation(resources) {
            return Err(Error::ForeignReservation);
        }
        self.retained.check()?;
        self.program.check(resources)?;
        if self.requests.len() != self.keys.len()
            || self.requests.len() != self.program.plan.probes.len() * 3
        {
            return Err(Error::ProbeSetMismatch);
        }
        dependence::check_requests(self)
    }
}
