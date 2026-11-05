//! Bounded historical content identity; hashes never authorize a foreign owner.
use super::*;
use sha2::{Digest, Sha256};
pub(super) fn root(
    native: &RootPlan<'_, '_>,
    d: &Declaration<'_>,
    view: &ResourceReservationView<'_>,
) -> Result<Digest32, Error> {
    let implementation = [
        include_bytes!("../framework.rs").as_slice(),
        include_bytes!("types.rs").as_slice(),
        include_bytes!("admission.rs").as_slice(),
        include_bytes!("bounds.rs").as_slice(),
        include_bytes!("capability.rs").as_slice(),
        include_bytes!("current.rs").as_slice(),
        include_bytes!("execution.rs").as_slice(),
        include_bytes!("identity.rs").as_slice(),
        include_bytes!("lowering.rs").as_slice(),
        include_bytes!("native.rs").as_slice(),
        include_bytes!("plan.rs").as_slice(),
        include_bytes!("../canonical.rs").as_slice(),
        include_bytes!("../../finite_modal_observation.rs").as_slice(),
        include_bytes!("../../../../lc631-solvers/src/finite_modal.rs").as_slice(),
    ];
    let mut bytes_total = 4096usize;
    for bytes in implementation {
        bytes_total = bytes_total
            .checked_add(bytes.len())
            .ok_or(Error::WorkBudget)?;
    }
    bounds::debit(native, bytes_total)?; // complete exact UTF8 source preimage before hashing
    let mut h = Sha256::new();
    h.update(b"tlcx.framework.p1.given-closed.v1\0");
    h.update(native.historical_digest().as_bytes());
    for bytes in implementation {
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    }
    h.update(d.root_goal_revision.as_bytes());
    h.update(d.authority_revision.0.to_le_bytes());
    h.update(d.definition_revision.as_bytes());
    h.update(d.map_revision.as_bytes());
    // Closed registries already checked; encode canonical role/package order,
    // never caller display names, allocation pointers or array arrival order.
    h.update(b"C:6,Kc:1,Ki:1;roles10x2;family2^n/3^n;n0..3;embed-root-v1");
    for meaning in 0..native.state.declaration.atoms.len() {
        let m = d
            .map
            .atoms
            .iter()
            .find(|m| usize::from(m.meaning) == meaning)
            .ok_or(Error::InvalidMap)?;
        h.update([m.meaning, m.point_slot, m.chain_slot]);
    }
    let check = || native.check(view);
    let mut w = Work::new(
        &native.meter,
        native.state.declaration.limits.max_call_work,
        &check,
    );
    for p in d.probes {
        let bytes = canonical::construction(
            p.formula,
            native.state.declaration.atoms,
            native.state.declaration.limits,
            &mut w,
        )
        .map_err(context)?;
        h.update([p.role as u8, p.duty as u8]);
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    }
    match d.allowed_operator {
        Operator::ReframeDeMorgan => h.update([0]),
        Operator::Recombine {
            partner,
            connective,
        } => {
            h.update([
                1,
                if connective == crate::creative_candidate_bridge::Connective::And {
                    0
                } else {
                    1
                },
            ]);
            h.update(partner.as_bytes());
        }
    }
    let l = d.limits;
    for value in [
        u64::from(l.max_atoms),
        l.max_models as u64,
        l.max_cases as u64,
        l.max_queries as u64,
        l.max_lowered_nodes as u64,
        l.max_native_dispatches_per_case as u64,
        l.max_current_checks as u64,
        l.plan_ram,
        l.case_ram,
        l.scratch_ram,
        l.output_bytes,
    ] {
        h.update(value.to_le_bytes());
    }
    Ok(Digest32::from_bytes(h.finalize().into()))
}
pub(super) fn scope(digest: Digest32, goal: Digest32, kernel: Kernel) -> ContextScope {
    let mut h = Sha256::new();
    h.update(b"tlcx.framework.p1.native-scope.v1\0");
    h.update(digest.as_bytes());
    h.update([kernel as u8]);
    let semantics = Digest32::from_bytes(h.finalize().into());
    ContextScope::declared(digest, goal, digest, semantics)
}
