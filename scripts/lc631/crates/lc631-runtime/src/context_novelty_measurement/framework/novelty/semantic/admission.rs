use super::*;

pub(super) fn limits(r: &Registration<'_>) -> Result<(), Error> {
    let l = r.limits;
    if l.max_weight_bits == 0
        || l.max_weight_bits > 1024
        || l.max_intermediate_bits == 0
        || l.max_intermediate_bits > 4096
        || l.precision != 64
    {
        return Err(Error::NumericBudget);
    }
    if l.max_probes > 3
        || l.max_bridge_probes > 3
        || l.max_native_dispatches > 78
        || !(1..=2).contains(&l.max_assessments)
        || !(1..=4).contains(&l.max_current_checks)
        || !(1..=128).contains(&l.max_lowered_nodes)
        || r.semantic.len() > l.max_probes
        || r.bridges.len() > l.max_bridge_probes
    {
        return Err(Error::InvalidLimits);
    }
    if l.plan_ram != m::budget::PLAN
        || l.assessment_ram != m::budget::ASSESSMENT
        || l.scratch_ram != m::budget::SCRATCH
    {
        return Err(Error::RamBudget);
    }
    if l.max_payload_bytes < 4096 || l.max_payload_bytes > 65536 {
        return Err(Error::PayloadBudget);
    }
    if l.output_bytes < 4096 || l.output_bytes > 65536 {
        return Err(Error::OutputBudget);
    }
    Ok(())
}
pub(super) fn probes(
    parent: &m::Root<'_, '_>,
    r: &Registration<'_>,
    work: &mut n::Work<'_>,
) -> Result<(), Error> {
    let fp = parent.framework.declaration;
    for (i, p) in r.semantic.iter().enumerate() {
        if r.semantic[..i].iter().any(|q| q.role == p.role) {
            return Err(Error::DuplicateProbe);
        }
        let actual = fp
            .probes
            .iter()
            .find(|q| q.role == p.role)
            .ok_or(Error::InvalidProbe)?;
        if actual.duty == f::Duty::Required {
            return Err(Error::RequiredSemanticProbe);
        }
        if !std::ptr::eq(actual.formula, p.formula) {
            return Err(Error::InvalidProbe);
        }
        metrics::weight(p.weight, r.limits, work)?;
    }
    for (i, p) in r.bridges.iter().enumerate() {
        if r.bridges[..i].iter().any(|q| q.role == p.role) {
            return Err(Error::DuplicateProbe);
        }
        if !fp.probes.iter().any(|q| q.role == p.role) {
            return Err(Error::InvalidProbe);
        }
        if p.rule == BridgeRule::RootGoalUseV1 && p.role != f::QueryRole::GoalEcho {
            return Err(Error::InvalidProbe);
        }
        metrics::weight(p.weight, r.limits, work)?;
    }
    // 日本語: 意味距離に選ばれないqueryも、実Not bankの行を必要とする。
    // English: Every query, even an unselected semantic metric, needs an actual Not bank row.
    let mut source = 0usize;
    let mut lowered = 0usize;
    for probe in fp.probes {
        work.charge(1024).map_err(native)?;
        let size = f::lowering::dimensions(probe.formula, 2).map_err(framework)?;
        source = source
            .checked_add(size.source + 1)
            .ok_or(Error::LoweringBudget)?;
        lowered = lowered
            .checked_add(size.native + 1)
            .ok_or(Error::LoweringBudget)?;
    }
    if source > r.limits.max_lowered_nodes || lowered > r.limits.max_lowered_nodes {
        return Err(Error::LoweringBudget);
    }
    Ok(())
}
