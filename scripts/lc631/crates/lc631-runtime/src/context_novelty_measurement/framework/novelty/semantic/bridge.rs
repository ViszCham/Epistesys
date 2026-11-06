//! 日本語: 実有限source/target行に対する方向付き検査。
//! English: A directional check on the actual finite source and target rows.
use super::*;

pub struct BridgeWitness {
    states: [u8; 3],
    arity: usize,
    lost: [u8; 3],
    lost_len: usize,
    source: bool,
    target: bool,
}
impl BridgeWitness {
    pub fn target_states(&self) -> &[u8] { &self.states[..self.arity] }
    pub fn lost_future_atoms(&self) -> &[u8] { &self.lost[..self.lost_len] }
    pub fn source_root_forced(&self) -> bool { self.source }
    pub fn target_root_forced(&self) -> bool { self.target }
}
pub struct BridgeResult {
    pub(super) role: f::QueryRole,
    pub(super) rule: BridgeRule,
    mismatches: Vec<BridgeWitness>,
}
impl BridgeResult {
    pub fn passed(&self) -> bool { self.mismatches.is_empty() }
    pub fn mismatches(&self) -> &[BridgeWitness] { &self.mismatches }
}
pub(super) fn inspect(probe: BridgeProbe<'_>, pair: m::ExpectedPair<'_, '_, '_>,
    work: &mut n::Work<'_>) -> Result<BridgeResult, Error> {
    let src = pair.source.owner;
    let dst = pair.target.owner;
    let role = usize::from(probe.role as u8);
    let mut mismatches = reserve(dst.data.rows.len(), 27)?;
    for (i, target) in dst.data.rows.iter().enumerate().filter(|(_, r)| r.admitted()) {
        work.charge((src.data.rows.len() as u64 + 1) * 16).map_err(native)?;
        let target_map = dst.data.mapbacks.get(i).ok_or(Error::ExpectedInputMismatch)?;
        let states = target.valuation_states();
        let mut mapped = [0u8; 3];
        if states.len() > 3 || target_map.root_assignment().len() != states.len() {
            return Err(Error::ExpectedInputMismatch);
        }
        let mut lost: &[u8] = &[];
        match (pair.source.expected.kernel, pair.target.expected.kernel) {
            (a, b) if a == b => mapped[..states.len()].copy_from_slice(states),
            (f::Kernel::ClassicalPointV1, f::Kernel::PersistentChain2V1) => {
                for (out, &value) in mapped.iter_mut().zip(target_map.root_assignment()) {
                    *out = u8::from(value);
                }
                lost = target_map.lost_future_atoms();
            }
            (f::Kernel::PersistentChain2V1, f::Kernel::ClassicalPointV1) => {
                for (out, &value) in mapped.iter_mut().zip(target_map.root_assignment()) {
                    *out = if value { 2 } else { 0 };
                }
            }
            _ => return Err(Error::ExpectedInputMismatch),
        }
        let source = src.data.rows.iter().find(|r| r.valuation_states() == &mapped[..states.len()])
            .ok_or(Error::ExpectedInputMismatch)?;
        // 日本語: 実写像先の付値で、source族全体を参照する。両contextの実行可能性は
        // 異なり得るため、この方向付きbridgeは同型ではない。
        // English: Use the complete source family at the actual mapped valuation. The two
        // contexts may differ in feasibility; the directional bridge is not an isomorphism.
        let (s, t) = match probe.rule {
            BridgeRule::RootGoalUseV1 => (source.goal_bits() & 1 != 0, target.goal_bits() & 1 != 0),
            BridgeRule::MappedRootForcingV1 => (
                source.query_bits().get(role).ok_or(Error::InvalidProbe)? & 1 != 0,
                target.query_bits().get(role).ok_or(Error::InvalidProbe)? & 1 != 0),
        };
        let pass = match probe.rule { BridgeRule::RootGoalUseV1 => s && t, BridgeRule::MappedRootForcingV1 => s == t };
        if !pass {
            if lost.len() > 3 { return Err(Error::ExpectedInputMismatch); }
            let mut row = BridgeWitness { states: [0; 3], arity: states.len(), lost: [0; 3],
                lost_len: lost.len(), source: s, target: t };
            row.states[..states.len()].copy_from_slice(states);
            row.lost[..lost.len()].copy_from_slice(lost);
            mismatches.push(row);
        }
    }
    Ok(BridgeResult { role: probe.role, rule: probe.rule, mismatches })
}
