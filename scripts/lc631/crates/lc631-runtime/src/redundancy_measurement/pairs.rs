//! 日本語: consistencyと全same-role pairを実native requestへ固定する。
//! English: Freeze consistency and every same-role pair into actual native requests.
use super::*;
use lc631_substrate::resource::ResourceReservationView;

#[derive(Clone, Copy)]
pub(super) struct CallCost {
    pub(super) work: u64,
    pub(super) map: u64,
}

impl<'candidate, 'plan, 'input, 'view> PairPlan<'candidate, 'plan, 'input, 'view> {
    pub fn prepare(
        root: &'plan RootRedundancyPlan<'input, 'view>,
        candidate: &'candidate BoundCandidateUnits<'plan, 'input, 'view>,
        view: &'view ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        root.check(view)?;
        candidate.check(view)?;
        if !std::ptr::eq(root, candidate.root) {
            return Err(Error::BindingMismatch);
        }
        // Charge bounded pair enumeration even when a later capacity/debit fails.
        root.debit(1024)?;
        let scratch = root::scratch(root.limits, view)?;
        let mut indices = [(0usize, 0usize); 120];
        let mut required = 0u64;
        let mut count = 0usize;
        for i in 0..root.input.slots.len() {
            for j in i + 1..root.input.slots.len() {
                if root.input.slots[i].role == root.input.slots[j].role {
                    required += 1;
                    if candidate.formulas[i].is_some() && candidate.formulas[j].is_some() {
                        indices[count] = (i, j);
                        count += 1;
                    }
                }
            }
        }
        let one = Stats {
            nodes: 1,
            variables: 0,
            bytes: 2,
        };
        let mut call_costs = [CallCost { work: 0, map: 0 }; 121];
        call_costs[0] = cost(root, one, one)?;
        // Complete relation check (<=4096 triples), partition and retained-result hashing.
        let mut work = syntax::add(call_costs[0].work, 16384)?;
        for (index, &(i, j)) in indices[..count].iter().enumerate() {
            call_costs[index + 1] = cost(root, candidate.stats[i], candidate.stats[j])?;
            work = syntax::add(work, call_costs[index + 1].work)?;
        }
        // Full dispatch/hash allowance is prepaid once, never refunded. Recreating
        // plans shares this same counter; producing one plan twice is refused.
        root.debit(work)?;
        let bytes = resources::backing::<formal_boolean::Request<'candidate>>(
            count + 1,
            std::mem::size_of::<Self>(),
        )?;
        let bytes = syntax::add(bytes, resources::backing::<(usize, usize)>(count, 0)?)?;
        let bytes = syntax::add(bytes, resources::backing::<CallCost>(count + 1, 0)?)?;
        let retained = ChainClaim::acquire(&root.budget, root.limits, bytes, 0, view)?;
        let mut requests = resources::vector(count + 1)?;
        let mut pairs = resources::vector(count)?;
        let mut costs = resources::vector(count + 1)?;
        requests.push(request(
            root,
            &root.consistency_true,
            &root.consistency_true,
        )?);
        costs.push(call_costs[0]);
        for (index, &(i, j)) in indices[..count].iter().enumerate() {
            let a = candidate.formulas[i].ok_or(Error::Conflict)?;
            let b = candidate.formulas[j].ok_or(Error::Conflict)?;
            requests.push(request(root, a, b)?);
            pairs.push((i, j));
            costs.push(call_costs[index + 1]);
        }
        use sha2::{Digest, Sha256};
        let mut digest = Sha256::new();
        digest.update(b"epistesys.redundancy.actual-pair-plan.v1\0");
        digest.update(root.comparison_digest.as_bytes());
        digest.update(candidate.digest.as_bytes());
        for r in &requests {
            digest.update(r.binding.probe_map.as_bytes());
        }
        scratch.check().map_err(Error::Resource)?;
        let result = Self {
            candidate,
            requests,
            pairs,
            costs,
            pairs_required: required,
            digest: Digest32::from_bytes(digest.finalize().into()),
            started: Cell::new(false),
            retained,
        };
        result.check(view)?;
        Ok(result)
    }
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.candidate.check(view)?;
        self.retained.check(view)
    }
}
fn cost(root: &RootRedundancyPlan<'_, '_>, a: Stats, b: Stats) -> Result<CallCost, Error> {
    let n = root.native_symbols.len() as u64;
    let rows = 1u64 << n;
    let nodes = syntax::add(
        syntax::mul(root.theory_stats.nodes, 2)?,
        syntax::add(a.nodes, b.nodes)?,
    )?;
    let visits = syntax::mul(nodes, rows + 1)?;
    let vars = syntax::add(
        syntax::mul(root.theory_stats.variables, 2)?,
        syntax::add(a.variables, b.variables)?,
    )?;
    let map = syntax::add(syntax::mul(syntax::mul(vars, n)?, rows)?, 8 * n * n + 16)?;
    // Includes standalone probe hashing, admission symbol scans and wire hashing.
    let wire = syntax::add(
        syntax::mul(root.theory_stats.bytes, 2)?,
        syntax::add(a.bytes, b.bytes)?,
    )?;
    let overhead = syntax::add(syntax::mul(nodes, 2 * n + 8)?, syntax::mul(wire, 2)? + 512)?;
    let work = syntax::add(visits, syntax::add(map, overhead)?)?;
    if work > root.limits.max_call_work {
        return Err(Error::PerCallBudget);
    }
    Ok(CallCost { work, map })
}
fn request<'a>(
    root: &'a RootRedundancyPlan<'_, '_>,
    a: &'a BoolExpr,
    b: &'a BoolExpr,
) -> Result<formal_boolean::Request<'a>, Error> {
    let model = root.input.model;
    let mut request = formal_boolean::Request {
        source: model.theory,
        target: model.theory,
        source_predicate: a,
        target_predicate: b,
        symbols: &root.native_symbols,
        unconstrained: model.unconstrained,
        binding: formal_boolean::Binding {
            source: model.context.source(),
            target: model.context.source(),
            source_context: model.context,
            target_context: model.context,
            probe_set: root.input.scope.probes(),
            probe_map: Digest32::from_bytes([0; 32]),
        },
    };
    request.binding.probe_map =
        formal_boolean::probe_map_digest(&request, root.limits.max_call_work)
            .map_err(Error::Formal)?;
    Ok(request)
}
