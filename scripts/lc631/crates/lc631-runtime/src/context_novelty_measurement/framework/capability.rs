//! Executed finite law recipes, not caller capability flags or human-wide logic truth.
use super::*;
const RECIPES: usize = 24;
pub(super) struct Capability<'v> {
    pub point: [usize; 4],
    pub chain: [usize; 4],
    pub history: native::NativeHistory<'v>,
    pub point_recipe_values: Vec<[u64; RECIPES]>,
}
impl Capability<'_> {
    pub fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        if self.point_recipe_values.len() != 15
            || self.point != [1, 2, 4, 8]
            || self.chain != [1, 3, 9, 27]
            || self.point_recipe_values.iter().flatten().any(|v| *v > 1)
        {
            return Err(Error::ProfileContractMismatch);
        }
        self.history.check(view)
    }
}
pub(super) fn execute<'v>(
    root: &RootPlan<'_, '_>,
    scope: ContextScope,
    view: &'v ResourceReservationView<'_>,
) -> Result<Capability<'v>, Error> {
    // 40 chain models x24 bounded roots, their hashes/model checks and complete
    // lowering backing are prepaid separately from the existing Boolean Work.
    let native_work = root
        .state
        .source
        .text()
        .len()
        .checked_mul(960)
        .and_then(|n| n.checked_add(303_072))
        .ok_or(Error::WorkBudget)?;
    if u64::try_from(native_work).map_err(|_| Error::WorkBudget)?
        > root.state.declaration.limits.max_call_work
    {
        return Err(Error::WorkBudget);
    }
    bounds::debit(root, native_work)?; // existing Modal evaluate hashes exact source per dispatch
    let mut history = native::NativeHistory::new(960, root.state.context, scope, view)?;
    history.lowered = bounds::reserve(96, 96)?;
    history.models = bounds::reserve(40, 40)?;
    let mut result = Capability {
        point: [0; 4],
        chain: [0; 4],
        history,
        point_recipe_values: bounds::reserve(15, 15)?,
    };
    let check = || root.check(view);
    let mut work = Work::new(
        &root.meter,
        root.state.declaration.limits.max_call_work,
        &check,
    );
    for n in 0..=3 {
        let grounding = lc631_substrate::source::SourceSpan::checked(root.state.source, 0, 0)
            .map_err(|_| Error::InvalidMap)?;
        let mut atoms = bounds::reserve(n, 3)?;
        let mut map = bounds::reserve(n, 3)?;
        for i in 0..n {
            let meaning = u8::try_from(i).map_err(|_| Error::InvalidMap)?;
            atoms.push(super::super::AtomMap {
                local: BoolSymbolId::new(u32::from(meaning)),
                meaning,
                grounding,
            });
            map.push(AtomCorrespondence {
                meaning,
                point_slot: meaning,
                chain_slot: meaning,
            });
        }
        let recipes = recipes(n);
        let base = result.history.lowered.len();
        for e in &recipes {
            result
                .history
                .lowered
                .push(lowering::lower(e, &atoms, &map)?);
        }
        for kernel in KERNELS {
            let count = bounds::models(kernel, n)?;
            let mut saw_lem_gap = false;
            let mut saw_dne_gap = false;
            for ordinal in 0..count {
                let radix = if kernel == Kernel::ClassicalPointV1 {
                    2
                } else {
                    3
                };
                let states = native::states(ordinal, n, radix)?;
                let mut values = [0u64; RECIPES];
                if kernel == Kernel::ClassicalPointV1 {
                    for (i, e) in recipes.iter().enumerate() {
                        values[i] = u64::from(
                            canonical::eval(e, ordinal, &atoms, &mut work).map_err(context)?,
                        );
                    }
                } else {
                    let model_index = result.history.models.len();
                    result
                        .history
                        .models
                        .push(native::chain_model(&states, &map)?);
                    for (i, value) in values.iter_mut().enumerate() {
                        *value = result.history.evaluate(
                            root.state.source.text(),
                            model_index,
                            base + i,
                            view,
                        )?;
                    }
                }
                forcing(kernel, &states, &values)?;
                laws(kernel, &values, &mut saw_lem_gap, &mut saw_dne_gap)?;
                match kernel {
                    Kernel::ClassicalPointV1 => {
                        if result.point_recipe_values.len() >= 15 {
                            return Err(Error::ProfileContractMismatch);
                        }
                        result.point_recipe_values.push(values);
                        result.point[n] += 1;
                    }
                    Kernel::PersistentChain2V1 => result.chain[n] += 1,
                }
            }
            if kernel == Kernel::PersistentChain2V1 && n > 0 && (!saw_lem_gap || !saw_dne_gap) {
                return Err(Error::ProfileContractMismatch);
            }
        }
    }
    result.check(view)?;
    Ok(result)
}
fn and(a: BoolExpr, b: BoolExpr) -> BoolExpr {
    BoolExpr::And(vec![a, b])
}
fn or(a: BoolExpr, b: BoolExpr) -> BoolExpr {
    BoolExpr::Or(vec![a, b])
}
fn not(a: BoolExpr) -> BoolExpr {
    BoolExpr::Not(Box::new(a))
}
fn forcing(kernel: Kernel, states: &[u8], values: &[u64; RECIPES]) -> Result<(), Error> {
    let bit = |state| match (kernel, state) {
        (Kernel::ClassicalPointV1, 0) | (Kernel::PersistentChain2V1, 0) => Ok(0),
        (Kernel::ClassicalPointV1, 1) => Ok(1),
        (Kernel::PersistentChain2V1, 1) => Ok(2),
        (Kernel::PersistentChain2V1, 2) => Ok(3),
        _ => Err(Error::ProfileContractMismatch),
    };
    let expected = if states.is_empty() {
        [
            0,
            if kernel == Kernel::ClassicalPointV1 {
                1
            } else {
                3
            },
            0,
        ]
    } else {
        [
            bit(states[0])?,
            bit(states[if states.len() > 1 { 1 } else { 0 }])?,
            bit(states[if states.len() > 2 { 2 } else { 0 }])?,
        ]
    };
    if values[..3] != expected {
        return Err(Error::ProfileContractMismatch);
    }
    Ok(())
}
fn recipes(n: usize) -> [BoolExpr; RECIPES] {
    // Small bounded recipe constructors, not copies of arbitrary caller ASTs.
    let p = || {
        if n == 0 {
            BoolExpr::Constant(false)
        } else {
            BoolExpr::Variable(BoolSymbolId::new(0))
        }
    };
    let q = || {
        if n == 0 {
            BoolExpr::Constant(true)
        } else {
            BoolExpr::Variable(BoolSymbolId::new(if n > 1 { 1 } else { 0 }))
        }
    };
    let r = || {
        if n == 0 {
            BoolExpr::Constant(false)
        } else {
            BoolExpr::Variable(BoolSymbolId::new(if n > 2 { 2 } else { 0 }))
        }
    };
    [
        p(),
        q(),
        r(),
        BoolExpr::Constant(false),
        BoolExpr::Constant(true),
        not(p()),
        and(p(), q()),
        or(p(), q()),
        and(q(), p()),
        or(q(), p()),
        and(p(), p()),
        or(p(), p()),
        and(and(p(), q()), r()),
        and(p(), and(q(), r())),
        or(or(p(), q()), r()),
        or(p(), or(q(), r())),
        and(p(), or(q(), r())),
        or(and(p(), q()), and(p(), r())),
        or(p(), and(q(), r())),
        and(or(p(), q()), or(p(), r())),
        and(p(), not(p())),
        or(p(), not(p())),
        not(not(p())),
        not(BoolExpr::Constant(true)),
    ]
}
fn laws(
    kernel: Kernel,
    v: &[u64; RECIPES],
    lem_gap: &mut bool,
    dne_gap: &mut bool,
) -> Result<(), Error> {
    let top = if kernel == Kernel::ClassicalPointV1 {
        1
    } else {
        3
    };
    let expected_not = match (kernel, v[0]) {
        (Kernel::ClassicalPointV1, 0) => 1,
        (Kernel::ClassicalPointV1, 1) => 0,
        (Kernel::PersistentChain2V1, 0) => 3,
        (Kernel::PersistentChain2V1, 2 | 3) => 0,
        _ => return Err(Error::ProfileContractMismatch),
    };
    if v[3] != 0
        || v[4] != top
        || v[5] != expected_not
        || v[23] != 0
        || v[6] != (v[0] & v[1])
        || v[7] != (v[0] | v[1])
        || v[6] != v[8]
        || v[7] != v[9]
        || v[10] != v[0]
        || v[11] != v[0]
        || v[12] != v[13]
        || v[14] != v[15]
        || v[16] != v[17]
        || v[18] != v[19]
        || v[6] & !v[0] != 0
        || v[6] & !v[1] != 0
        || v[0] & !v[7] != 0
        || v[1] & !v[7] != 0
        || v[20] != 0
        || v[21] != (v[0] | v[5])
    {
        return Err(Error::ProfileContractMismatch);
    }
    // All finite positive-rule premise/conclusion combinations at each world.
    if (v[2] & !v[0] == 0 && v[2] & !v[1] == 0 && v[2] & !v[6] != 0)
        || (v[0] & !v[2] == 0 && v[1] & !v[2] == 0 && v[7] & !v[2] != 0)
    {
        return Err(Error::ProfileContractMismatch);
    }
    if kernel == Kernel::ClassicalPointV1 {
        if v[21] != top || v[22] != v[0] {
            return Err(Error::ProfileContractMismatch);
        }
    } else {
        if v[21] != top {
            *lem_gap = true;
        }
        if v[22] & !v[0] != 0 {
            *dne_gap = true;
        }
        if v[22] != (if v[0] == 0 { 0 } else { 3 }) {
            return Err(Error::ProfileContractMismatch);
        }
    }
    Ok(())
}
