//! 同じ固定coding probeを両artifactで再実行し、現在のgainを選択へ渡す。
//! Reruns the same fixed coding probes on both artifacts and selects current gain.
#![forbid(unsafe_code)]

use super::transition::{parse_digest, HistoricalRepairTransition, TransitionError};
use crate::contextual_repair::{
    execution::{observe_current, ProbeInvocation},
    measurement::{
        paired_gain, select_current, CodingProbePlan, OwnedCodingProbeGain,
        OwnedCodingProbeObservation, ProbeError,
    },
    snapshot_gate::{CodingBuildPort, ExpectedCurrentBuild, SnapshotGateError},
};
use lc631_analysis::lifecycle_binds_target;
use lc631_analysis::declared_build_capture::{CurrentSnapshotAuthority, SnapshotCaptureAttempt};
use lc631_core::{context::ContextRevision, stable_sha256};
use lc631_substrate::{resource::ResourceReservationView, Digest32};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FreshResumeError {
    History(TransitionError),
    Probe(ProbeError),
    Build(SnapshotGateError),
    BindingMismatch,
    UnexpectedSelection,
}

/// The current application token binds its post-snapshot, not the complete
/// pre-repair build. A matching baseline target alone cannot close that gap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaselineBuildCorrespondence {
    NeedsEvidence,
    BoundByActualBaselineProducer,
}

/// An explicit gate observation. Even a late-rejected actual capture stays
/// owned here; capture presence alone never authorizes a current selection.
pub struct CodingResumeBuildAttempt<'res> {
    selection: Option<bool>,
    diagnostic: Option<FreshResumeError>,
    capture: Option<SnapshotCaptureAttempt<'res>>,
}
impl CodingResumeBuildAttempt<'_> {
    pub fn diagnostic(&self) -> Option<&FreshResumeError> { self.diagnostic.as_ref() }
    pub fn capture_attempt(&self) -> Option<&SnapshotCaptureAttempt<'_>> { self.capture.as_ref() }
    /// Selection sampled at this gate; rerun the gate for a later publication.
    pub fn selection(&self, resources: &ResourceReservationView<'_>) -> Result<bool, FreshResumeError> {
        if let Some(error) = &self.diagnostic { return Err(error.clone()); }
        let capture = self.capture.as_ref().ok_or(FreshResumeError::UnexpectedSelection)?;
        let actual = capture.current().map_err(|e| FreshResumeError::Build(SnapshotGateError::Capture(e)))?;
        actual.check_current(resources).map_err(|e| FreshResumeError::Build(SnapshotGateError::Capture(e)))?;
        self.selection.ok_or(FreshResumeError::UnexpectedSelection)
    }
}

pub struct FreshCodingResumeRequest<'a> {
    pub history: HistoricalRepairTransition,
    pub context: ContextRevision,
    pub comparison_scope: Digest32,
    pub plan: &'a CodingProbePlan,
    /// Retained immutable pre-repair source/native artifact, independently registered.
    pub baseline: ProbeInvocation<'a>,
    /// Current repaired source/native artifact, independently registered.
    pub current: ProbeInvocation<'a>,
}

/// Keeps the actual observations and their resource claims alive. No public
/// report/Deserialize constructor, restored permit, application token or quota.
pub struct OwnedCodingResumeSelection<'v> {
    history: HistoricalRepairTransition,
    baseline: OwnedCodingProbeObservation<'v>,
    current: OwnedCodingProbeObservation<'v>,
    baseline_gain: OwnedCodingProbeGain<'v>,
    gain: OwnedCodingProbeGain<'v>,
    selected: bool,
}
impl OwnedCodingResumeSelection<'_> {
    pub fn baseline_build_correspondence(&self) -> BaselineBuildCorrespondence {
        if self.history.before_build().is_some() {
            BaselineBuildCorrespondence::BoundByActualBaselineProducer
        } else {
            BaselineBuildCorrespondence::NeedsEvidence
        }
    }
    pub fn history(&self) -> &HistoricalRepairTransition { &self.history }
    pub fn baseline(&self) -> &OwnedCodingProbeObservation<'_> { &self.baseline }
    pub fn current(&self) -> &OwnedCodingProbeObservation<'_> { &self.current }
    pub fn gain(&self) -> &OwnedCodingProbeGain<'_> { &self.gain }
    /// Rechecks the declared current build and current Test authority through
    /// the same concrete port as live repair. Historical baseline stays fixed.
    pub fn selected_current<'res>(
        &self,
        plan: &CodingProbePlan,
        port: &mut CodingBuildPort<'_, '_>,
        authority: &mut dyn CurrentSnapshotAuthority,
        resources: &'res ResourceReservationView<'_>,
    ) -> CodingResumeBuildAttempt<'res> {
        let expected = (|| {
            let expected = ExpectedCurrentBuild::from_verified_history(&self.history, port)
                .map_err(FreshResumeError::Build)?;
            self.check_probe_owners(plan, resources)?;
            self.current.check_current_artifact(plan, resources).map_err(FreshResumeError::Probe)?;
            Ok(expected)
        })();
        let expected = match expected {
            Ok(expected) => expected,
            Err(error) => return CodingResumeBuildAttempt { selection: None, diagnostic: Some(error), capture: None },
        };
        let capture = port.capture_current(authority, resources);
        let result = expected.check_attempt(&capture, resources).map_err(FreshResumeError::Build)
            .and_then(|()| self.check_probe_owners(plan, resources))
            .and_then(|()| self.current.check_current_artifact(plan, resources).map_err(FreshResumeError::Probe));
        let (selection, diagnostic) = match result {
            Ok(()) => (Some(self.selected), None),
            Err(error) => (None, Some(error)),
        };
        CodingResumeBuildAttempt { selection, diagnostic, capture: Some(capture) }
    }
    fn check_probe_owners(&self, plan: &CodingProbePlan, resources: &ResourceReservationView<'_>) -> Result<(), FreshResumeError> {
        self.baseline.check_current(plan, resources).map_err(FreshResumeError::Probe)?;
        self.current.check_current(plan, resources).map_err(FreshResumeError::Probe)?;
        self.baseline_gain.check_current(plan, resources).map_err(FreshResumeError::Probe)?;
        self.gain.check_current(plan, resources).map_err(FreshResumeError::Probe)
    }
    pub fn claim_boundary(&self) -> &'static str {
        if self.history.before_build().is_some() {
            "same fixed finite coding probes reobserved; registered build digests matched; current build still requires the fresh declared-build gate; historical effects/delivery unknown; Unanchored; no F1/F2/total-TL, coding closure, quota renewal, apply or retransmit"
        } else {
            "NeedsEvidence: baseline complete-build correspondence unavailable; same fixed finite coding probes reobserved; historical effects/delivery unknown; Unanchored; no F1/F2/total-TL, coding closure, quota renewal, apply or retransmit"
        }
    }
}

/// One read-only orchestration seam shared by explicit SDK/CLI resume callers.
/// Both native artifacts run again; history supplies no executable observations.
/// Native process effects are governed by the existing registered Test boundary,
/// not an OS sandbox claimed by this wrapper. Errors never trigger an apply/retry.
pub fn resume_reselect_coding<'v>(
    input: FreshCodingResumeRequest<'_>,
    resources: &'v ResourceReservationView<'_>,
) -> Result<OwnedCodingResumeSelection<'v>, FreshResumeError> {
    check_bindings(&input, resources)?;
    let baseline = observe_current(input.plan, input.baseline, resources)
        .map_err(FreshResumeError::Probe)?;
    let current = observe_current(input.plan, input.current, resources)
        .map_err(FreshResumeError::Probe)?;
    let gain = paired_gain(input.plan, &baseline, &current, resources)
        .map_err(FreshResumeError::Probe)?;
    let baseline_gain = paired_gain(input.plan, &baseline, &baseline, resources)
        .map_err(FreshResumeError::Probe)?;
    let selected = match select_current(input.plan, &[&baseline_gain, &gain], resources)
        .map_err(FreshResumeError::Probe)? {
        Some(1) => true,
        Some(0) | None => false,
        Some(_) => return Err(FreshResumeError::UnexpectedSelection),
    };
    Ok(OwnedCodingResumeSelection { history: input.history, baseline, current, baseline_gain, gain, selected })
}

fn check_bindings(
    input: &FreshCodingResumeRequest<'_>,
    resources: &ResourceReservationView<'_>,
) -> Result<(), FreshResumeError> {
    let history = &input.history;
    let parse = |value: &str| parse_digest(value).map_err(FreshResumeError::History);
    if input.context.source().digest() != history.task_source()
        || input.context.interpretation_digest() != history.context()
        || input.comparison_scope != history.comparison_scope()
        || input.plan.digest() != history.probe_plan()
        || input.plan.goal() != history.goal() || input.plan.policy() != history.policy()
        || resources.source_revision() != history.task_source()
        || input.baseline.source != input.current.source
        || parse(&stable_sha256(input.current.source))? != history.task_source()
    {
        return Err(FreshResumeError::BindingMismatch);
    }
    let before = &input.baseline.registration;
    let after = &input.current.registration;
    // A repair changes the physical target, not the task seed. Fresh authority
    // may change the snapshot's authority identity; compare actual build inputs
    // and target content separately instead of restoring an old snapshot permit.
    if before.target_ref != after.target_ref
        || parse(&stable_sha256(after.target_ref))? != history.target_ref_digest()
        || !lifecycle_binds_target(before.snapshot, before.target_ref,
            &super::transition::digest_text(history.before_target()))
        || !lifecycle_binds_target(after.snapshot, after.target_ref,
            &super::transition::digest_text(history.after_target()))
        || parse(&after.snapshot.build_input_digest)? != history.after_build()
        || history.before_build().is_some_and(|expected|
            parse_digest(&before.snapshot.build_input_digest) != Ok(expected))
    {
        return Err(FreshResumeError::BindingMismatch);
    }
    resources.check().map_err(|error| FreshResumeError::Probe(ProbeError::Resource(error)))
}
