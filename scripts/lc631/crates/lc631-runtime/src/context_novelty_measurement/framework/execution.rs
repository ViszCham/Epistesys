//! Actual case rows, immutable role occurrences and explicit non-isomorphic map-back.
use super::*;
#[derive(Clone, Copy, Debug)]
pub struct Counts {
    pub lowered_source_nodes: usize,
    pub unmapped_source_nodes: usize,
    pub native_dispatches: usize,
    pub native_store_entries: usize,
}
pub struct ModelRow {
    pub(super) states: Vec<u8>,
    pub(super) admitted: bool,
    pub(super) queries: [u64; 3],
    pub(super) goal: u64,
}
impl ModelRow {
    pub fn valuation_states(&self) -> &[u8] {
        &self.states
    }
    pub fn admitted(&self) -> bool {
        self.admitted
    }
    pub fn query_bits(&self) -> &[u64] {
        &self.queries
    }
    pub fn goal_bits(&self) -> u64 {
        self.goal
    }
}
pub struct MapBackRow {
    assignment: Vec<bool>,
    forward: Vec<u64>,
    lost: Vec<u8>,
}
impl MapBackRow {
    pub fn root_assignment(&self) -> &[bool] {
        &self.assignment
    }
    pub fn forward_world_bits(&self) -> &[u64] {
        &self.forward
    }
    pub fn lost_future_atoms(&self) -> &[u8] {
        &self.lost
    }
}
pub(super) struct Bank<'s> {
    pub roots: Vec<&'s BoolExpr>,
    pub dimensions: lowering::Dimensions,
}
pub(super) fn roots<'s>(
    native: &'s RootPlan<'_, '_>,
    d: &'s Declaration<'_>,
    subject: &'s SmtStatusQuery,
) -> Result<Bank<'s>, Error> {
    let hard = native.state.mutable.iter().filter(|m| !**m).count();
    if subject.assertions().len() != 2
        || hard != 1
        || native.state.goal.query.assertions().len() != 1
    {
        return Err(Error::UnsupportedProfile);
    }
    if d.limits.max_queries < 24 {
        return Err(Error::RegistryIncomplete);
    }
    // Scan exact source AND native-expanded totals without a payload allocation.
    // A source occurrence cannot stand in for a native binary-fold node.
    let mut dimensions = lowering::Dimensions {
        source: 0,
        native: 0,
    };
    let hard_roots = native
        .state
        .premises
        .iter()
        .zip(&native.state.mutable)
        .filter_map(|(p, m)| if !*m { Some(p.formula) } else { None });
    for e in subject
        .assertions()
        .iter()
        .chain(hard_roots)
        .chain(native.state.goal.query.assertions())
        .chain(d.probes.iter().map(|p| p.formula))
    {
        let size = lowering::dimensions(e, 1)?;
        dimensions.source = dimensions
            .source
            .checked_add(size.source)
            .ok_or(Error::LoweringBudget)?;
        dimensions.native = dimensions
            .native
            .checked_add(size.native)
            .ok_or(Error::LoweringBudget)?;
        if dimensions.source > 128
            || dimensions.native > 128
            || dimensions.source > d.limits.max_lowered_nodes
            || dimensions.native > d.limits.max_lowered_nodes
        {
            return Err(Error::LoweringBudget);
        }
    }
    let mut bank = Bank {
        roots: bounds::reserve(7, 7)?,
        dimensions,
    };
    for e in subject.assertions() {
        bank.roots.push(e);
    }
    for (i, m) in native.state.mutable.iter().enumerate() {
        if !*m {
            bank.roots.push(native.state.premises[i].formula);
        }
    }
    bank.roots.push(&native.state.goal.query.assertions()[0]);
    for p in d.probes {
        bank.roots.push(p.formula);
    }
    Ok(bank)
}
pub(super) struct PointTrace {
    pub row: usize,
    pub formula: usize,
    pub value: bool,
}
pub(super) struct Data<'s, 'v> {
    pub rows: Vec<ModelRow>,
    pub mapbacks: Vec<MapBackRow>,
    pub bank: Bank<'s>,
    pub native: Option<native::NativeHistory<'v>>,
    pub point: Vec<PointTrace>,
    pub counts: Counts,
    pub hard_ok: bool,
}
pub(super) fn execute<'s, 'a, 'v>(
    root: &'s FrameworkRoot<'a, 'v>,
    e: ExpectedInputs<'s, 'a, 'v>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedFrameworkCase<'s, 'a, 'v>, Error> {
    let l = root.declaration.limits;
    let n = root.native.state.declaration.atoms.len();
    // Bound source/probe scans before materializing any lowerings or models.
    let scratch = bounds::claim(view, bounds::SCRATCH_RAM, None)?;
    let bank = roots(&root.native, root.declaration, e.subject_query)?;
    let count = bounds::models(e.kernel, n)?;
    let calls = count
        .checked_mul(bank.roots.len())
        .ok_or(Error::NativeBudget)?;
    if calls > l.max_native_dispatches_per_case {
        return Err(Error::NativeBudget);
    }
    let payload_nodes = bank
        .dimensions
        .source
        .checked_add(bank.dimensions.native)
        .ok_or(Error::PayloadBudget)?;
    let output = count
        .checked_mul(128)
        .and_then(|v| v.checked_add(payload_nodes.checked_mul(32)?))
        .ok_or(Error::OutputBudget)?;
    if u64::try_from(output).map_err(|_| Error::OutputBudget)? > l.output_bytes {
        return Err(Error::OutputBudget);
    }
    let work = calls
        .checked_mul(payload_nodes.checked_add(32).ok_or(Error::WorkBudget)?)
        .and_then(|v| v.checked_mul(8))
        .and_then(|v| v.checked_add(root.native.state.source.text().len().checked_mul(calls)?))
        .ok_or(Error::WorkBudget)?;
    if u64::try_from(work).map_err(|_| Error::WorkBudget)?
        > root.native.state.declaration.limits.max_call_work
    {
        return Err(Error::WorkBudget);
    }
    bounds::debit(&root.native, work)?; // native hashing/dispatch/maps/allocation prepaid
    let retained = bounds::claim(view, bounds::CASE_RAM, Some(l.output_bytes))?;
    let mut data = Data {
        rows: bounds::reserve(count, 27)?,
        mapbacks: bounds::reserve(count, 27)?,
        bank,
        native: None,
        point: bounds::reserve(
            if e.kernel == Kernel::ClassicalPointV1 {
                calls
            } else {
                0
            },
            189,
        )?,
        counts: Counts {
            lowered_source_nodes: 0,
            unmapped_source_nodes: 0,
            native_dispatches: 0,
            native_store_entries: 0,
        },
        hard_ok: true,
    };
    if e.kernel == Kernel::PersistentChain2V1 {
        let revision = match e.subject {
            Subject::Original => root.native.state.context,
            Subject::Generated(c) => c.prepared().target(),
        };
        let scope = identity::scope(root.digest, root.declaration.root_goal_revision, e.kernel);
        let mut history = native::NativeHistory::new(calls, revision, scope, view)?;
        history.lowered = bounds::reserve(7, 7)?;
        history.models = bounds::reserve(count, 27)?;
        for ast in &data.bank.roots {
            history.lowered.push(lowering::lower(
                ast,
                root.native.state.declaration.atoms,
                root.declaration.map.atoms,
            )?);
        }
        data.native = Some(history);
    }
    let mut point_atoms = bounds::reserve(n, 3)?;
    for a in root.native.state.declaration.atoms {
        let slot = root
            .declaration
            .map
            .atoms
            .iter()
            .find(|v| v.meaning == a.meaning)
            .ok_or(Error::InvalidMap)?
            .point_slot;
        point_atoms.push(super::super::AtomMap {
            local: a.local,
            meaning: slot,
            grounding: a.grounding,
        });
    }
    let check = || root.native.check(view);
    let mut work = Work::new(
        &root.native.meter,
        root.native.state.declaration.limits.max_call_work,
        &check,
    );
    for ordinal in 0..count {
        let states = native::states(
            ordinal,
            n,
            if e.kernel == Kernel::ClassicalPointV1 {
                2
            } else {
                3
            },
        )?;
        let mut values = [0u64; 7];
        if let Some(history) = &mut data.native {
            history
                .models
                .push(native::chain_model(&states, root.declaration.map.atoms)?);
            for (index, value) in values.iter_mut().enumerate() {
                *value = history.evaluate(root.native.state.source.text(), ordinal, index, view)?;
                data.counts.native_dispatches += 1;
            }
        } else {
            let assignment = native::point_assignment(&states, root.declaration.map.atoms)?;
            for (index, ast) in data.bank.roots.iter().enumerate() {
                let value =
                    canonical::eval(ast, assignment, &point_atoms, &mut work).map_err(context)?;
                values[index] = u64::from(value);
                data.point.push(PointTrace {
                    row: ordinal,
                    formula: index,
                    value,
                });
                data.counts.native_dispatches += 1;
            }
        }
        let admitted = values[0] & 1 != 0 && values[1] & 1 != 0;
        if admitted && (values[2] & 1 == 0 || values[3] & 1 == 0) {
            data.hard_ok = false;
        }
        for (i, p) in root.declaration.probes.iter().enumerate() {
            if admitted && p.duty == Duty::Required && values[4 + i] & 1 == 0 {
                data.hard_ok = false;
            }
        }
        data.mapbacks.push(mapback(&states, e.kernel)?);
        data.rows.push(ModelRow {
            states,
            admitted,
            queries: [values[4], values[5], values[6]],
            goal: values[3],
        });
    }
    if !data.rows.iter().any(|r| r.admitted) {
        return Err(Error::ContextInconsistent);
    }
    data.counts.lowered_source_nodes = data.bank.dimensions.source;
    if let Some(history) = &data.native {
        history.check(view)?;
        data.counts.native_store_entries = history.store.len();
    }
    scratch.check().map_err(Error::Resource)?;
    let owner = OwnedFrameworkCase {
        root,
        expected: e,
        data,
        checks: Cell::new(0),
        retained,
    };
    owner.check_binding(e, view)?;
    Ok(owner)
}
fn mapback(states: &[u8], kernel: Kernel) -> Result<MapBackRow, Error> {
    let mut assignment = bounds::reserve(states.len(), 3)?;
    let mut forward = bounds::reserve(states.len(), 3)?;
    let mut lost = bounds::reserve(states.len(), 3)?;
    for (i, s) in states.iter().enumerate() {
        let root = match kernel {
            Kernel::ClassicalPointV1 => *s == 1,
            Kernel::PersistentChain2V1 => *s == 2,
        };
        assignment.push(root);
        forward.push(if root {
            if kernel == Kernel::ClassicalPointV1 {
                1
            } else {
                3
            }
        } else {
            0
        });
        if kernel == Kernel::PersistentChain2V1 && *s == 1 {
            lost.push(u8::try_from(i).map_err(|_| Error::InvalidMap)?);
        }
    }
    Ok(MapBackRow {
        assignment,
        forward,
        lost,
    })
}
