//! Same kernels/adapter as P1;archive Γ is not strengthened by hard/goal queries.
use super::*;
pub(super) struct Plan<'a> {
    roots: Vec<&'a BoolExpr>,
    assertions: usize,
    source_nodes: usize,
    native_nodes: usize,
    pub calls: usize,
    models: usize,
}
pub(super) fn plan<'a>(
    root: &Root<'a, '_>,
    e: &'a ArchiveEntry<'a>,
    w: &mut Work<'_>,
) -> Result<Plan<'a>, Error> {
    w.charge(4096).map_err(native)?;
    let native_root = &root.framework.native;
    let hard = native_root
        .state
        .premises
        .iter()
        .zip(&native_root.state.mutable)
        .filter_map(|(p, m)| if !*m { Some(p.formula) } else { None });
    let mut source_nodes = 0usize;
    let mut native_nodes = 0usize;
    for ast in e
        .query
        .assertions()
        .iter()
        .chain(hard)
        .chain(native_root.state.goal.query.assertions())
    {
        let d = f::lowering::dimensions(ast, 1).map_err(framework)?;
        source_nodes = source_nodes
            .checked_add(d.source)
            .ok_or(Error::PayloadBudget)?;
        native_nodes = native_nodes
            .checked_add(d.native)
            .ok_or(Error::PayloadBudget)?;
        if source_nodes > 128
            || native_nodes > 128
            || source_nodes > root.framework.declaration.limits.max_lowered_nodes
            || native_nodes > root.framework.declaration.limits.max_lowered_nodes
        {
            return Err(Error::Framework(f::Error::LoweringBudget));
        }
    }
    let count = e
        .query
        .assertions()
        .len()
        .checked_add(2)
        .ok_or(Error::PayloadBudget)?;
    let models = f::bounds::models(e.kernel, e.atoms.len()).map_err(framework)?;
    let calls = count.checked_mul(models).ok_or(Error::WorkBudget)?;
    let mut roots = budget::vector(count, 18)?;
    roots.extend(e.query.assertions());
    for (p, m) in native_root
        .state
        .premises
        .iter()
        .zip(&native_root.state.mutable)
    {
        if !*m {
            roots.push(p.formula);
        }
    }
    roots.push(&native_root.state.goal.query.assertions()[0]);
    if roots.len() != count {
        return Err(Error::InvalidMap);
    }
    Ok(Plan {
        roots,
        assertions: e.query.assertions().len(),
        source_nodes,
        native_nodes,
        calls,
        models,
    })
}
pub(super) struct Point<'a> {
    roots: Vec<&'a BoolExpr>,
    assertions: usize,
    archive_atoms: Vec<n::AtomMap>,
    root_atoms: Vec<n::AtomMap>,
    map: &'a [f::AtomCorrespondence],
    values: Vec<Vec<bool>>,
}
pub(super) enum Backend<'a, 'v> {
    Point(Point<'a>),
    Chain(f::native::NativeHistory<'v>),
}
pub(super) struct Held<'a, 'v> {
    pub entry: &'a ArchiveEntry<'a>,
    pub backend: Backend<'a, 'v>,
}
pub(super) struct Execution<'a, 'v> {
    pub held: Held<'a, 'v>,
    pub comparable: bool,
    pub calls: usize,
    pub entries: usize,
}
fn slots(atoms: &[n::AtomMap], map: &[f::AtomCorrespondence]) -> Result<Vec<n::AtomMap>, Error> {
    let mut out = budget::vector(atoms.len(), 3)?;
    for a in atoms {
        let meaning = map
            .iter()
            .find(|m| m.meaning == a.meaning)
            .ok_or(Error::InvalidMap)?
            .point_slot;
        out.push(n::AtomMap {
            local: a.local,
            meaning,
            grounding: a.grounding,
        });
    }
    Ok(out)
}
pub(super) fn execute<'a, 'v>(
    root: &Root<'a, 'v>,
    e: &'a ArchiveEntry<'a>,
    p: Plan<'a>,
    view: &'v ResourceReservationView<'_>,
    w: &mut Work<'_>,
) -> Result<Execution<'a, 'v>, Error> {
    let cost = p
        .calls
        .checked_mul(
            p.source_nodes
                .checked_add(p.native_nodes)
                .and_then(|v| v.checked_add(64))
                .ok_or(Error::WorkBudget)?,
        )
        .and_then(|v| v.checked_mul(8))
        .and_then(|v| v.checked_add(e.source.text().len().checked_mul(p.calls)?))
        .ok_or(Error::WorkBudget)?;
    w.charge(u64::try_from(cost).map_err(|_| Error::WorkBudget)?)
        .map_err(native)?;
    let map = root.framework.declaration.map.atoms;
    let mut backend = match e.kernel {
        f::Kernel::ClassicalPointV1 => Backend::Point(Point {
            roots: p.roots,
            assertions: p.assertions,
            archive_atoms: slots(e.atoms, map)?,
            root_atoms: slots(root.framework.native.state.declaration.atoms, map)?,
            map,
            values: budget::vector(p.models, 27)?,
        }),
        f::Kernel::PersistentChain2V1 => {
            let scope = f::identity::scope(
                root.digest,
                root.framework.declaration.root_goal_revision,
                e.kernel,
            );
            let mut history = f::native::NativeHistory::new(
                p.calls,
                root.framework.native.state.context,
                scope,
                view,
            )
            .map_err(framework)?;
            history.lowered = budget::vector(p.roots.len(), 18)?;
            history.models = budget::vector(p.models, 27)?;
            for (i, expr) in p.roots.iter().enumerate() {
                let atoms = if i < p.assertions {
                    e.atoms
                } else {
                    root.framework.native.state.declaration.atoms
                };
                history
                    .lowered
                    .push(f::lowering::lower(expr, atoms, map).map_err(framework)?);
            }
            Backend::Chain(history)
        }
    };
    let mut feasible = 0usize;
    let mut preserved = true;
    let mut actual_calls = 0usize;
    for ordinal in 0..p.models {
        let radix = if e.kernel == f::Kernel::ClassicalPointV1 {
            2
        } else {
            3
        };
        let states = f::native::states(ordinal, e.atoms.len(), radix).map_err(framework)?;
        let mut values = budget::vector(p.assertions + 2, 18)?;
        match &mut backend {
            Backend::Point(point) => {
                let assignment = f::native::point_assignment(&states, map).map_err(framework)?;
                let mut row = budget::vector(point.roots.len(), 18)?;
                for (i, expr) in point.roots.iter().enumerate() {
                    let atoms = if i < point.assertions {
                        &point.archive_atoms
                    } else {
                        &point.root_atoms
                    };
                    let value = n::canonical::eval(expr, assignment, atoms, w).map_err(native)?;
                    row.push(value);
                    values.push(u64::from(value));
                    actual_calls += 1;
                }
                point.values.push(row);
            }
            Backend::Chain(history) => {
                history
                    .models
                    .push(f::native::chain_model(&states, map).map_err(framework)?);
                for index in 0..history.lowered.len() {
                    values.push(
                        history
                            .evaluate(e.source.text(), ordinal, index, view)
                            .map_err(framework)?,
                    );
                    actual_calls += 1;
                }
            }
        }
        let admitted = values[..p.assertions].iter().all(|v| v & 1 != 0);
        if admitted {
            feasible += 1;
            if values[p.assertions] & 1 == 0 || values[p.assertions + 1] & 1 == 0 {
                preserved = false;
            }
        }
    }
    if actual_calls != p.calls {
        return Err(Error::ExpectedInputMismatch);
    }
    let entries = match &backend {
        Backend::Point(_) => 0,
        Backend::Chain(h) => {
            h.check(view).map_err(framework)?;
            h.store.len()
        }
    };
    Ok(Execution {
        held: Held { entry: e, backend },
        comparable: feasible > 0 && preserved,
        calls: actual_calls,
        entries,
    })
}
impl Held<'_, '_> {
    pub fn bind(&self, h: &mut n::native_hash::Hash<'_, '_>) -> Result<(), Error> {
        h.n(self.entry.kernel as u64).map_err(native)?;
        h.query(self.entry.query).map_err(native)?;
        match &self.backend {
            Backend::Point(p) => {
                for row in &p.values {
                    for value in row {
                        h.n(u64::from(*value)).map_err(native)?;
                    }
                }
            }
            Backend::Chain(history) => {
                for r in &history.records {
                    h.n(r.model as u64).map_err(native)?;
                    h.n(r.formula as u64).map_err(native)?;
                    h.bytes(&r.input).map_err(native)?;
                    h.n(r.bits).map_err(native)?;
                }
            }
        }
        Ok(())
    }
    pub fn check(&self, view: &ResourceReservationView<'_>, w: &mut Work<'_>) -> Result<(), Error> {
        w.charge(4096).map_err(native)?;
        match &self.backend {
            Backend::Chain(history) => {
                let rows = history
                    .records
                    .len()
                    .checked_mul(64)
                    .ok_or(Error::WorkBudget)?;
                let nodes = history.lowered.iter().try_fold(0usize, |sum, l| {
                    sum.checked_add(l.trace.len()).ok_or(Error::WorkBudget)
                })?;
                w.charge(
                    u64::try_from(
                        rows.checked_add(nodes.checked_mul(8).ok_or(Error::WorkBudget)?)
                            .ok_or(Error::WorkBudget)?,
                    )
                    .map_err(|_| Error::WorkBudget)?,
                )
                .map_err(native)?;
                history.check(view).map_err(framework)
            }
            Backend::Point(point) => {
                for (ordinal, row) in point.values.iter().enumerate() {
                    let states =
                        f::native::states(ordinal, self.entry.atoms.len(), 2).map_err(framework)?;
                    let assignment =
                        f::native::point_assignment(&states, point.map).map_err(framework)?;
                    if row.len() != point.roots.len() {
                        return Err(Error::ExpectedInputMismatch);
                    }
                    for (i, expr) in point.roots.iter().enumerate() {
                        let atoms = if i < point.assertions {
                            &point.archive_atoms
                        } else {
                            &point.root_atoms
                        };
                        if n::canonical::eval(expr, assignment, atoms, w).map_err(native)? != row[i]
                        {
                            return Err(Error::ExpectedInputMismatch);
                        }
                    }
                }
                Ok(())
            }
        }
    }
}
