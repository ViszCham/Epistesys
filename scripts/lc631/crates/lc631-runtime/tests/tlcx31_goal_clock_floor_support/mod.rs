//! Only actual invocation clocks differ; owner/registration/source/Test stay real.
//! No scalar setter, fake owner, host-clock authentication or authority issuance.
use lc631_runtime::{
    contextual_repair::execution::{ProbeInvocation, ProbeToolRegistration},
    goal_suite_measurement::ArtifactCurrent,
};
use std::cell::Cell;
thread_local! {
    static BASE_READS:Cell<usize>=const{Cell::new(0)};
    static NEXT_READS:Cell<usize>=const{Cell::new(0)};
}
pub fn reset() {
    BASE_READS.with(|x| x.set(0));
    NEXT_READS.with(|x| x.set(0));
}
pub fn reads() -> (usize, usize) {
    (BASE_READS.with(Cell::get), NEXT_READS.with(Cell::get))
}
fn base(n: u64) -> u64 {
    BASE_READS.with(|x| x.set(x.get() + 1));
    n
}
fn next(n: u64) -> u64 {
    NEXT_READS.with(|x| x.set(x.get() + 1));
    n
}
pub fn base100() -> u64 {
    base(100)
}
pub fn base200() -> u64 {
    base(200)
}
pub fn base400() -> u64 {
    base(400)
}
pub fn next100() -> u64 {
    next(100)
}
pub fn next200() -> u64 {
    next(200)
}
fn invocation<'a>(old: &'a ProbeInvocation<'_>, clock: fn() -> u64) -> ProbeInvocation<'a> {
    ProbeInvocation {
        source: old.source,
        registration: ProbeToolRegistration {
            program: old.registration.program,
            executable_digest: old.registration.executable_digest,
            target_path: old.registration.target_path,
            target_ref: old.registration.target_ref,
            snapshot: old.registration.snapshot,
        },
        lease: old.lease,
        tool_receipt: old.tool_receipt,
        tool_verifier: old.tool_verifier,
        permit: old.permit,
        authority: old.authority,
        clock,
        timeout_ms: old.timeout_ms,
    }
}
pub fn with_clocks<'bi, 'br, 'ci, 'cr, R>(
    old: &mut ArtifactCurrent<'_, 'bi, 'br, 'ci, 'cr>,
    baseline: fn() -> u64,
    candidate: fn() -> u64,
    consume: impl for<'cut> FnOnce(&mut ArtifactCurrent<'cut, 'bi, 'br, 'ci, 'cr>) -> R,
) -> R {
    let bi = invocation(&old.baseline_invocation, baseline);
    let ci = invocation(&old.candidate_invocation, candidate);
    let mut current = ArtifactCurrent {
        baseline_invocation: bi,
        candidate_invocation: ci,
        baseline_build: &mut *old.baseline_build,
        candidate_build: &mut *old.candidate_build,
        baseline_authority: &mut *old.baseline_authority,
        candidate_authority: &mut *old.candidate_authority,
    };
    consume(&mut current)
}
