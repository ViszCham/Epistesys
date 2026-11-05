//! Total typed registry and source/root anchors before executable capability work.
use super::*;
pub(super) fn declaration(native: &RootPlan<'_, '_>, d: &Declaration<'_>) -> Result<(), Error> {
    bounds::limits(d.limits, native.state.declaration.atoms.len())?;
    if !native.bridge.proposals().is_empty() {
        return Err(Error::AlreadyGenerated);
    }
    if d.root_scope != native.state.scope
        || d.root_goal_revision != native.state.goal.revision
        || d.authority_revision != native.bridge.contexts().original().authority()
    {
        return Err(Error::RootScopeMismatch);
    }
    if d.profiles.len() != 2 {
        return Err(Error::RegistryIncomplete);
    }
    for kernel in KERNELS {
        let mut entries = d.profiles.iter().filter(|p| p.kernel == kernel);
        let p = entries.next().ok_or(Error::RegistryIncomplete)?;
        if entries.next().is_some() {
            return Err(Error::RegistryIncomplete);
        }
        if p.display_name.len() > 128 {
            return Err(Error::PayloadBudget);
        }
        if !matches!(
            (kernel, p.contract),
            (Kernel::ClassicalPointV1, Contract::ClassicalPointV1)
                | (Kernel::PersistentChain2V1, Contract::PersistentChain2V1)
        ) {
            return Err(Error::ProfileContractMismatch);
        }
    }
    partition(d)?;
    mapping(native, d)?;
    if d.probes.len() != 3 || d.partition.len() > d.limits.max_queries {
        return Err(Error::RegistryIncomplete);
    }
    for (i, role) in [
        QueryRole::GoalEcho,
        QueryRole::Placement,
        QueryRole::BridgeLaw,
    ]
    .into_iter()
    .enumerate()
    {
        if d.probes[i].role != role {
            return Err(Error::RegistryIncomplete);
        }
    }
    if native.state.goal.query.assertions().len() != 1
        || !std::ptr::eq(
            d.probes[0].formula,
            &native.state.goal.query.assertions()[0],
        )
    {
        return Err(Error::RootScopeMismatch);
    }
    if d.comparison.source
        != (CellKey {
            kernel: Kernel::ClassicalPointV1,
            subject: SubjectKind::Original,
        })
        || d.comparison.target
            != (CellKey {
                kernel: Kernel::PersistentChain2V1,
                subject: SubjectKind::Generated,
            })
    {
        return Err(Error::RootScopeMismatch);
    }
    // A registered generator remains an existing native operator with its actual checks.
    match d.allowed_operator {
        Operator::Recombine { partner, .. }
            if native.state.premises.iter().any(|p| p.id == partner) => {}
        Operator::ReframeDeMorgan => {}
        Operator::Recombine { .. } => return Err(Error::UnsupportedOperator),
    }
    Ok(())
}
fn partition(d: &Declaration<'_>) -> Result<(), Error> {
    if d.partition.len() != 20 || d.universe.len() != 3 {
        return Err(Error::InvalidPartition);
    }
    for (i, package) in [Package::C, Package::Kc, Package::Ki]
        .into_iter()
        .enumerate()
    {
        let expected = Rational::from_integer((if i == 0 { 6 } else { 1 }).into());
        if d.universe[i].package != package || *d.universe[i].weight != expected {
            return Err(Error::InvalidPartition);
        }
    }
    for kernel in KERNELS {
        for (i, role) in ROLES.into_iter().enumerate() {
            let expected = if i < 6 {
                Package::C
            } else {
                match kernel {
                    Kernel::ClassicalPointV1 => Package::Kc,
                    Kernel::PersistentChain2V1 => Package::Ki,
                }
            };
            let mut values = d
                .partition
                .iter()
                .filter(|b| b.kernel == kernel && b.role == role);
            if values.next().map(|b| b.package) != Some(expected) || values.next().is_some() {
                return Err(Error::InvalidPartition);
            }
        }
    }
    Ok(())
}
fn mapping(native: &RootPlan<'_, '_>, d: &Declaration<'_>) -> Result<(), Error> {
    let n = native.state.declaration.atoms.len();
    if d.map.atoms.len() != n
        || d.map.operators.len() != 5
        || d.map.point_world != 0
        || d.map.chain_root != 0
        || d.map.chain_future != 1
    {
        return Err(Error::InvalidMap);
    }
    let mut meanings = [false; 3];
    let mut point = [false; 3];
    let mut chain = [false; 3];
    for m in d.map.atoms {
        let (a, b, c) = (
            usize::from(m.meaning),
            usize::from(m.point_slot),
            usize::from(m.chain_slot),
        );
        if a >= n
            || b >= n
            || c >= n
            || meanings[a]
            || point[b]
            || chain[c]
            || !native
                .state
                .declaration
                .atoms
                .iter()
                .any(|v| v.meaning == m.meaning)
        {
            return Err(Error::InvalidMap);
        }
        meanings[a] = true;
        point[b] = true;
        chain[c] = true;
    }
    for (source, target) in OPERATORS {
        let mut mapped = d.map.operators.iter().filter(|m| m.source == source);
        if mapped.next().map(|m| m.target) != Some(target) || mapped.next().is_some() {
            return Err(Error::InvalidMap);
        }
    }
    Ok(())
}
