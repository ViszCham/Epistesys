//! 固定有限coding probeの実出力。原式F1/F2やtotal TLではない。
//! Actual fixed finite coding probes; neither original F1/F2 nor total TL.
#![forbid(unsafe_code)]
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    Digest32,
};
use sha2::{Digest, Sha256};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

/// Owner-local epoch history, shared only with gains derived from that owner.
/// Relaxed ordering suffices: this atomic orders a single monotone number, not
/// publication of artifact payloads or authority. It does not certify a clock.
pub(super) struct ObservedEpoch(AtomicU64);
impl ObservedEpoch {
    pub(super) fn shared(birth: u64) -> Arc<Self> {
        Arc::new(Self(AtomicU64::new(birth)))
    }

    pub(super) fn observe(&self, now: u64, expires: u64) -> Result<(), ProbeError> {
        let previous = self.0.fetch_max(now, Ordering::Relaxed);
        if now < previous {
            return Err(ProbeError::ClockRollback);
        }
        // Preserve expiry as an observation too; rewinding must not revive it.
        if now >= expires {
            return Err(ProbeError::Stale);
        }
        Ok(())
    }
}

pub const PROBE_INPUT: &[u8] = b"0 0\n1 2\n4294967295 1\n4294967295 0\n";
pub const PROFILE: &[u8] = b"finite-formal-v1.4/CR-SOLVE/checked-u32-add/four-fixed-probes.v1";
pub fn digest(bytes: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(bytes).into())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProbeError {
    InvalidPlan,
    BindingMismatch,
    Unauthorized,
    UnregisteredTool,
    Stale,
    ClockRollback,
    InvalidOutput,
    ProcessFailed,
    Io(std::io::ErrorKind),
    Resource(ResourceError),
    PhysicalTargetChanged,
    NativeArtifactChanged,
    BuildSnapshot(super::snapshot_gate::SnapshotGateError),
}
impl From<ResourceError> for ProbeError {
    fn from(e: ResourceError) -> Self {
        Self::Resource(e)
    }
}

/// This profile fixes four inputs and the separate wider-integer oracle before
/// candidate capture. Given root goal/policy labels do not certify human intent.
pub struct CodingProbePlan {
    goal: Digest32,
    policy: Digest32,
    digest: Digest32,
}
impl CodingProbePlan {
    pub fn checked_add(goal: Digest32, policy: Digest32) -> Result<Self, ProbeError> {
        let zero = Digest32::from_bytes([0; 32]);
        if goal == zero || policy == zero {
            return Err(ProbeError::InvalidPlan);
        }
        let mut h = Sha256::new();
        h.update(PROFILE);
        h.update(goal.as_bytes());
        h.update(policy.as_bytes());
        h.update(PROBE_INPUT);
        Ok(Self {
            goal,
            policy,
            digest: Digest32::from_bytes(h.finalize().into()),
        })
    }
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }
    pub const fn goal(&self) -> Digest32 {
        self.goal
    }
    pub const fn policy(&self) -> Digest32 {
        self.policy
    }
}

/// No raw report/serialized baseline admission exists. Cross-process consumers
/// rerun both retained baseline artifact and current artifact under fresh Test
/// permission, then call paired_gain with these live owners.
/// ```compile_fail
/// use lc631_runtime::contextual_repair::measurement::OwnedCodingProbeObservation;
/// let _: OwnedCodingProbeObservation<'_> = serde_json::from_str("{}").unwrap();
/// ```
pub struct OwnedCodingProbeObservation<'v> {
    pub(super) plan: Digest32,
    pub(super) source: Digest32,
    pub(super) snapshot: Digest32,
    pub(super) build_input: Digest32,
    pub(super) target: Digest32,
    pub(super) observation: Digest32,
    pub(super) outputs: [Option<u32>; 4],
    pub(super) correct: u8,
    pub(super) checked_epoch: u64,
    pub(super) expires_epoch: u64,
    pub(super) clock: fn() -> u64,
    pub(super) observed_epoch: Arc<ObservedEpoch>,
    pub(super) artifact: super::execution::ProbeArtifactWitness,
    // Caller-allocated provider owns configuration, not additional authority.
    // Its allocation is the caller's explicit configuration budget obligation.
    pub(super) build_authority:
        Option<Box<dyn lc631_analysis::declared_build_capture::CurrentSnapshotAuthority + 'v>>,
    pub(super) retained: ResourceClaim<'v>,
}
impl OwnedCodingProbeObservation<'_> {
    pub const fn correct_probes(&self) -> u8 {
        self.correct
    }
    pub const fn snapshot(&self) -> Digest32 {
        self.snapshot
    }
    pub const fn build_input_digest(&self) -> Digest32 {
        self.build_input
    }
    pub const fn task_source(&self) -> Digest32 {
        self.source
    }
    pub const fn checked_epoch(&self) -> u64 {
        self.checked_epoch
    }
    pub const fn target(&self) -> Digest32 {
        self.target
    }
    pub const fn plan_digest(&self) -> Digest32 {
        self.plan
    }
    pub const fn observation_digest(&self) -> Digest32 {
        self.observation
    }
    pub const fn actual_outputs(&self) -> &[Option<u32>; 4] {
        &self.outputs
    }
    /// Physical-only observations remain historical/finite probe evidence.
    /// Presence is configuration, not a successful declared-build capture.
    pub fn has_build_authority(&self) -> bool {
        self.build_authority.is_some()
    }
    /// Rechecks funding, fixed-plan binding and caller clock/expiry only. The
    /// physical artifact and revocation view were checked during observation;
    /// this method is not a continuous path/host-revocation watcher.
    pub fn check_current(
        &self,
        plan: &CodingProbePlan,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), ProbeError> {
        resources.check()?;
        self.retained.check()?;
        if !self.retained.same_reservation(resources)
            || self.source != resources.source_revision()
            || self.plan != plan.digest
        {
            return Err(ProbeError::BindingMismatch);
        }
        let now = (self.clock)();
        self.observed_epoch.observe(now, self.expires_epoch)?;
        if now < self.checked_epoch {
            return Err(ProbeError::ClockRollback);
        }
        if now >= self.expires_epoch {
            return Err(ProbeError::Stale);
        }
        Ok(())
    }
    /// Reopen and stream-hash this observation's exact physical target and native
    /// artifact. No read handle is kept across the owner's lifetime: a historical
    /// baseline must not lock the target against its own authorized repair.
    /// Call only for an artifact advertised as physically current, not merely to
    /// inspect intentionally historical baseline/gain evidence.
    /// This rereads target/native bytes, not every package/build input. Other
    /// build-snapshot drift remains unresolved; no fresh full-package closure or
    /// hermetic source-to-binary proof follows from this artifact check.
    pub fn check_current_artifact(
        &self,
        plan: &CodingProbePlan,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), ProbeError> {
        self.check_current(plan, resources)?;
        self.artifact.check(self.target, resources)?;
        self.check_current(plan, resources)
    }
    pub(super) fn binds_repair_target(
        &self,
        request: &lc631_analysis::DgclRepairRequest,
    ) -> Result<(), ProbeError> {
        self.artifact.binds_repair_target(request)
    }
}

/// An actual paired primitive, not a reconstructed F2 or aggregate TL.
pub struct OwnedCodingProbeGain<'v> {
    plan: Digest32,
    source: Digest32,
    baseline: Digest32,
    current: Digest32,
    gain_quarters: i8,
    correct: u8,
    checked_epoch: u64,
    expires_epoch: u64,
    clocks: [fn() -> u64; 2],
    observed_epochs: [Arc<ObservedEpoch>; 2],
    retained: ResourceClaim<'v>,
}
impl OwnedCodingProbeGain<'_> {
    pub const fn gain_quarters(&self) -> i8 {
        self.gain_quarters
    }
    pub const fn correct_probes(&self) -> u8 {
        self.correct
    }
    pub const fn current_observation(&self) -> Digest32 {
        self.current
    }
    pub const fn baseline_observation(&self) -> Digest32 {
        self.baseline
    }
    pub fn check_current(
        &self,
        plan: &CodingProbePlan,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), ProbeError> {
        resources.check()?;
        self.retained.check()?;
        if !self.retained.same_reservation(resources)
            || self.source != resources.source_revision()
            || self.plan != plan.digest
        {
            return Err(ProbeError::BindingMismatch);
        }
        for (role, clock) in self.clocks.iter().enumerate() {
            let now = clock();
            self.observed_epochs[role].observe(now, self.expires_epoch)?;
            if now < self.checked_epoch {
                return Err(ProbeError::ClockRollback);
            }
            if now >= self.expires_epoch {
                return Err(ProbeError::Stale);
            }
        }
        Ok(())
    }
}
pub fn paired_gain<'v>(
    plan: &CodingProbePlan,
    baseline: &OwnedCodingProbeObservation<'_>,
    current: &OwnedCodingProbeObservation<'_>,
    resources: &'v ResourceReservationView<'_>,
) -> Result<OwnedCodingProbeGain<'v>, ProbeError> {
    baseline.check_current(plan, resources)?;
    current.check_current(plan, resources)?;
    if baseline.source != current.source {
        return Err(ProbeError::BindingMismatch);
    }
    let retained = claim(resources, 4096)?;
    let gain = OwnedCodingProbeGain {
        plan: plan.digest,
        source: current.source,
        baseline: baseline.observation,
        current: current.observation,
        gain_quarters: current.correct as i8 - baseline.correct as i8,
        correct: current.correct,
        checked_epoch: baseline.checked_epoch.max(current.checked_epoch),
        expires_epoch: baseline.expires_epoch.min(current.expires_epoch),
        clocks: [baseline.clock, current.clock],
        observed_epochs: [
            Arc::clone(&baseline.observed_epoch),
            Arc::clone(&current.observed_epoch),
        ],
        retained,
    };
    gain.check_current(plan, resources)?;
    Ok(gain)
}
/// Narrow declared finite goal policy: maximize actual correct-probe fraction.
/// Hard coding closure remains a separate coordinator gate. Ties/empty inputs
/// yield no unique recommendation. No missing TL or original-score axes appear.
pub fn select_current(
    plan: &CodingProbePlan,
    rows: &[&OwnedCodingProbeGain<'_>],
    resources: &ResourceReservationView<'_>,
) -> Result<Option<usize>, ProbeError> {
    resources.check()?;
    if rows.len() > 4 {
        return Err(ProbeError::InvalidPlan);
    }
    let mut best: Option<usize> = None;
    let mut tied = false;
    for (i, row) in rows.iter().enumerate() {
        row.check_current(plan, resources)?;
        if rows
            .first()
            .is_some_and(|first| first.baseline != row.baseline)
        {
            return Err(ProbeError::BindingMismatch);
        }
        match best {
            None => {
                best = Some(i);
                tied = false;
            }
            Some(j) if row.gain_quarters > rows[j].gain_quarters => {
                best = Some(i);
                tied = false;
            }
            Some(j) if row.gain_quarters == rows[j].gain_quarters => tied = true,
            Some(_) => {}
        }
    }
    if tied {
        Ok(None)
    } else {
        Ok(best)
    }
}
pub(super) fn claim<'v>(
    resources: &'v ResourceReservationView<'_>,
    bytes: u64,
) -> Result<ResourceClaim<'v>, ProbeError> {
    Ok(resources.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(bytes)?,
    )])?)
}
pub(super) fn parse_output(bytes: &[u8]) -> Result<([Option<u32>; 4], u8), ProbeError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ProbeError::InvalidOutput)?;
    let mut outputs = [None; 4];
    let mut lines = text.split_inclusive('\n');
    for slot in &mut outputs {
        let line = lines.next().ok_or(ProbeError::InvalidOutput)?;
        *slot = if line == "none\n" {
            None
        } else {
            let raw = line
                .strip_prefix("some:")
                .and_then(|s| s.strip_suffix('\n'))
                .ok_or(ProbeError::InvalidOutput)?;
            let n = raw.parse::<u32>().map_err(|_| ProbeError::InvalidOutput)?;
            if n.to_string() != raw {
                return Err(ProbeError::InvalidOutput);
            }
            Some(n)
        };
    }
    if lines.next().is_some() {
        return Err(ProbeError::InvalidOutput);
    }
    let inputs = [(0_u32, 0_u32), (1, 2), (u32::MAX, 1), (u32::MAX, 0)];
    let mut correct = 0;
    for (actual, (a, b)) in outputs.iter().zip(inputs) {
        let sum = u64::from(a) + u64::from(b);
        let expected = u32::try_from(sum).ok();
        if *actual == expected {
            correct += 1;
        }
    }
    Ok((outputs, correct))
}
