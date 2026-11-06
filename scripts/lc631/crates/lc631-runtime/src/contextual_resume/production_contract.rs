//! One read-only resume request owns its actual pair, captures and call budget.
#![forbid(unsafe_code)]
mod packet;
use super::transition::{digest_text, parse_digest, HistoricalRepairTransition};
use crate::contextual_repair::{attempt::*, measurement::*, snapshot_gate::*};
use lc631_analysis::{
    declared_build_capture::{CurrentSnapshotAuthority, SnapshotCaptureAttempt},
    lifecycle_binds_target,
};
use lc631_core::context::ContextRevision;
use lc631_substrate::{
    resource::{ResourceClaim, ResourceError, ResourceReservationView},
    Digest32,
};
pub use packet::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodingResumeError {
    MissingProvider,
    BindingMismatch,
    NeedsEvidence,
    NeedsPublication,
    BudgetExhausted,
    Probe(ProbeError),
    Build(SnapshotGateError),
    Resource(ResourceError),
    UnexpectedSelection,
}
impl From<ProbeError> for CodingResumeError {
    fn from(e: ProbeError) -> Self {
        Self::Probe(e)
    }
}
impl From<ResourceError> for CodingResumeError {
    fn from(e: ResourceError) -> Self {
        Self::Resource(e)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodingResumeStage {
    Hold,
    ReadyForPublication,
    Published,
}
pub struct CodingResumeCallBudget {
    native: u8,
    captures: u8,
    used_native: u8,
    used_captures: u8,
}
impl CodingResumeCallBudget {
    pub fn checked(native: u8, captures: u8) -> Result<Self, CodingResumeError> {
        if native == 0 || native > 2 || captures == 0 || captures > 2 {
            return Err(CodingResumeError::BudgetExhausted);
        }
        Ok(Self {
            native,
            captures,
            used_native: 0,
            used_captures: 0,
        })
    }
    pub fn used_native(&self) -> u8 {
        self.used_native
    }
    pub fn used_captures(&self) -> u8 {
        self.used_captures
    }
    fn native(&mut self) -> Result<(), CodingResumeError> {
        if self.used_native >= self.native {
            return Err(CodingResumeError::BudgetExhausted);
        }
        self.used_native += 1;
        Ok(())
    }
    fn capture(&mut self) -> Result<(), CodingResumeError> {
        if self.used_captures >= self.captures {
            return Err(CodingResumeError::BudgetExhausted);
        }
        self.used_captures += 1;
        Ok(())
    }
}
pub struct CodingResumeAttemptInput<'a> {
    pub history: HistoricalRepairTransition,
    pub context: ContextRevision,
    pub comparison_scope: Digest32,
    pub plan: &'a CodingProbePlan,
    pub baseline: ProbeAttemptInput<'a>,
    pub current: ProbeAttemptInput<'a>,
    pub origin: &'a CodingResumeOrigin,
}
pub struct ResumeAttempt<'res> {
    history: Option<HistoricalRepairTransition>,
    baseline: Option<ProbeAttempt<'res>>,
    current: Option<ProbeAttempt<'res>>,
    baseline_gain: Option<OwnedCodingProbeGain<'res>>,
    gain: Option<OwnedCodingProbeGain<'res>>,
    captures: Vec<SnapshotCaptureAttempt<'res>>,
    // Snapshot remaining admission at creation. Caller budget/port replacement
    // cannot replenish this attempt's nonrefundable native or capture slots.
    allowance: CodingResumeCallBudget,
    expected: Option<ExpectedCurrentBuild>,
    selected: Option<bool>,
    stage: CodingResumeStage,
    diagnostic: Option<CodingResumeError>,
    _funding: Option<ResourceClaim<'res>>,
}
impl<'res> ResumeAttempt<'res> {
    pub fn baseline(&self) -> Option<&ProbeAttempt<'res>> {
        self.baseline.as_ref()
    }
    pub fn current_probe(&self) -> Option<&ProbeAttempt<'res>> {
        self.current.as_ref()
    }
    pub fn diagnostic(&self) -> Option<&CodingResumeError> {
        self.diagnostic.as_ref()
    }
    pub fn stage(&self) -> CodingResumeStage {
        self.stage
    }
    pub fn gain(&self) -> Option<&OwnedCodingProbeGain<'res>> {
        self.gain.as_ref()
    }
    pub fn build_attempts(&self) -> &[SnapshotCaptureAttempt<'res>] {
        &self.captures
    }
    fn check_pair(
        &self,
        plan: &CodingProbePlan,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), CodingResumeError> {
        view.check()?;
        self.baseline
            .as_ref()
            .ok_or(CodingResumeError::NeedsEvidence)?
            .current()?
            .check_current(plan, view)?;
        self.current
            .as_ref()
            .ok_or(CodingResumeError::NeedsEvidence)?
            .current()?
            .check_current(plan, view)?;
        self.baseline_gain
            .as_ref()
            .ok_or(CodingResumeError::NeedsEvidence)?
            .check_current(plan, view)?;
        self.gain
            .as_ref()
            .ok_or(CodingResumeError::NeedsEvidence)?
            .check_current(plan, view)?;
        Ok(())
    }
    pub fn current(&self, view: &ResourceReservationView<'_>) -> Result<bool, CodingResumeError> {
        if let Some(error) = &self.diagnostic {
            return Err(error.clone());
        }
        if self.stage != CodingResumeStage::Published {
            return Err(CodingResumeError::NeedsPublication);
        }
        let history = self
            .history
            .as_ref()
            .ok_or(CodingResumeError::NeedsEvidence)?;
        let plan = CodingProbePlan::checked_add(history.goal(), history.policy())?;
        self.check_pair(&plan, view)?;
        let capture = self
            .captures
            .last()
            .ok_or(CodingResumeError::NeedsEvidence)?;
        self.expected
            .as_ref()
            .ok_or(CodingResumeError::NeedsEvidence)?
            .check_attempt(capture, view)
            .map_err(CodingResumeError::Build)?;
        self.selected.ok_or(CodingResumeError::UnexpectedSelection)
    }
    pub fn publish_current(
        &mut self,
        plan: &CodingProbePlan,
        input: &ProbeAttemptInput<'_>,
        port: &mut CodingBuildPort<'_, '_>,
        probe: &mut dyn CurrentProbeAuthority,
        build: &mut dyn CurrentSnapshotAuthority,
        budget: &mut CodingResumeCallBudget,
        view: &'res ResourceReservationView<'_>,
    ) -> Result<bool, CodingResumeError> {
        let result = (|| {
            // Debit the retained owner before the caller ceiling, any provider
            // or IO, and any stored failure. Rejected calls keep their debit.
            self.allowance.capture()?;
            budget.capture()?;
            if let Some(error) = &self.diagnostic {
                return Err(error.clone());
            }
            self.check_pair(plan, view)?;
            let history = self
                .history
                .as_ref()
                .ok_or(CodingResumeError::NeedsEvidence)?;
            let expected = ExpectedCurrentBuild::from_verified_history(history, port)
                .map_err(CodingResumeError::Build)?;
            // Retain an actual late-rejected capture before checking its gate.
            let capture = port.capture_current(build, view);
            self.captures.push(capture);
            expected
                .check_attempt(
                    self.captures
                        .last()
                        .ok_or(CodingResumeError::NeedsEvidence)?,
                    view,
                )
                .map_err(CodingResumeError::Build)?;
            self.check_pair(plan, view)?;
            self.current
                .as_mut()
                .ok_or(CodingResumeError::NeedsEvidence)?
                .recheck_current(plan, input, ProbePurpose::Current, probe, view)?;
            self.expected = Some(expected);
            self.selected.ok_or(CodingResumeError::UnexpectedSelection)
        })();
        match &result {
            Ok(_) => self.stage = CodingResumeStage::Published,
            Err(error) => {
                self.stage = CodingResumeStage::Hold;
                // An exhausted retry must not erase the original late failure.
                if self.diagnostic.is_none() {
                    self.diagnostic = Some(error.clone());
                }
            }
        }
        result
    }
}
pub fn resume_coding_attempt<'res>(
    input: CodingResumeAttemptInput<'_>,
    port: &mut CodingBuildPort<'_, '_>,
    probe: Option<&mut dyn CurrentProbeAuthority>,
    build: Option<&mut dyn CurrentSnapshotAuthority>,
    budget: &mut CodingResumeCallBudget,
    view: &'res ResourceReservationView<'_>,
) -> ResumeAttempt<'res> {
    let mut a = ResumeAttempt {
        history: None,
        baseline: None,
        current: None,
        baseline_gain: None,
        gain: None,
        captures: Vec::new(),
        allowance: CodingResumeCallBudget {
            native: budget.native - budget.used_native,
            captures: budget.captures - budget.used_captures,
            used_native: 0,
            used_captures: 0,
        },
        expected: None,
        selected: None,
        stage: CodingResumeStage::Hold,
        diagnostic: None,
        _funding: None,
    };
    let result = prepare(&input, port, probe, build, budget, view, &mut a);
    // Preserve authenticated historical ownership even on a pre-child Hold.
    a.history = Some(input.history);
    match result {
        Ok(()) => a.stage = CodingResumeStage::ReadyForPublication,
        Err(error) => a.diagnostic = Some(error),
    }
    a
}
fn prepare<'res>(
    input: &CodingResumeAttemptInput<'_>,
    port: &mut CodingBuildPort<'_, '_>,
    probe: Option<&mut dyn CurrentProbeAuthority>,
    build: Option<&mut dyn CurrentSnapshotAuthority>,
    budget: &mut CodingResumeCallBudget,
    view: &'res ResourceReservationView<'_>,
    a: &mut ResumeAttempt<'res>,
) -> Result<(), CodingResumeError> {
    view.check()?;
    let probe = probe.ok_or(CodingResumeError::MissingProvider)?;
    let build = build.ok_or(CodingResumeError::MissingProvider)?;
    bindings(input, view)?;
    use lc631_substrate::resource::{ResourceAmount, ResourceKind, ResourceRequest};
    a._funding = Some(view.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(131072)?,
    )])?);
    a.captures
        .try_reserve_exact(2)
        .map_err(|_| CodingResumeError::Resource(ResourceError::AllocationFailed))?;
    if port.binding().program() != input.origin.pins[14] {
        return Err(CodingResumeError::BindingMismatch);
    }
    let expected = ExpectedCurrentBuild::from_verified_history(&input.history, port)
        .map_err(CodingResumeError::Build)?;
    a.allowance.capture()?;
    budget.capture()?;
    a.captures.push(port.capture_current(build, view));
    expected
        .check_attempt(&a.captures[0], view)
        .map_err(CodingResumeError::Build)?;
    a.expected = Some(expected);
    a.allowance.native()?;
    budget.native()?;
    a.baseline = Some(observe_current_attempt(
        input.plan,
        &input.baseline,
        ProbePurpose::Baseline,
        Some(&mut *probe),
        view,
    ));
    a.baseline
        .as_ref()
        .ok_or(CodingResumeError::NeedsEvidence)?
        .current()?;
    a.allowance.native()?;
    budget.native()?;
    a.current = Some(observe_current_attempt(
        input.plan,
        &input.current,
        ProbePurpose::Current,
        Some(&mut *probe),
        view,
    ));
    let baseline = a
        .baseline
        .as_ref()
        .ok_or(CodingResumeError::NeedsEvidence)?
        .current()?;
    let current = a
        .current
        .as_ref()
        .ok_or(CodingResumeError::NeedsEvidence)?
        .current()?;
    a.baseline_gain = Some(paired_gain(input.plan, baseline, baseline, view)?);
    a.gain = Some(paired_gain(input.plan, baseline, current, view)?);
    let baseline_gain = a
        .baseline_gain
        .as_ref()
        .ok_or(CodingResumeError::NeedsEvidence)?;
    let gain = a.gain.as_ref().ok_or(CodingResumeError::NeedsEvidence)?;
    a.selected = Some(
        match select_current(input.plan, &[baseline_gain, gain], view)? {
            Some(1) => true,
            Some(0) | None => false,
            Some(_) => return Err(CodingResumeError::UnexpectedSelection),
        },
    );
    a.check_pair(input.plan, view)
}
fn bindings(
    i: &CodingResumeAttemptInput<'_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), CodingResumeError> {
    let h = &i.history;
    let p = &i.origin.pins;
    let before = &i.baseline.registration;
    let after = &i.current.registration;
    let parse = |s: &str| parse_digest(s).map_err(|_| CodingResumeError::BindingMismatch);
    if h.before_build().is_none() || h.original_request_root().is_none() {
        return Err(CodingResumeError::NeedsEvidence);
    }
    if i.context.source().digest() != h.task_source()
        || i.context.interpretation_digest() != h.context()
        || i.comparison_scope != h.comparison_scope()
        || i.plan.digest() != h.probe_plan()
        || i.plan.goal() != h.goal()
        || i.plan.policy() != h.policy()
        || view.source_revision() != h.task_source()
        || i.baseline.source != i.current.source
        || digest(i.current.source.as_bytes()) != h.task_source()
        || i.baseline.origin.frame != i.origin.frame
        || i.current.origin.frame != i.origin.frame
        || before.target_ref != after.target_ref
        || digest(after.target_ref.as_bytes()) != h.target_ref_digest()
        || !lifecycle_binds_target(
            before.snapshot,
            before.target_ref,
            &digest_text(h.before_target()),
        )
        || !lifecycle_binds_target(
            after.snapshot,
            after.target_ref,
            &digest_text(h.after_target()),
        )
        || Some(parse(&before.snapshot.build_input_digest)?) != h.before_build()
        || parse(&after.snapshot.build_input_digest)? != h.after_build()
        || p[0]
            != h.original_request_root()
                .ok_or(CodingResumeError::NeedsEvidence)?
        || p[1] != h.task_source()
        || p[2] != h.context()
        || p[3] != h.comparison_scope()
        || p[4] != h.original_plan_digest()
        || p[5] != h.goal()
        || p[6] != h.policy()
        || p[7] != h.probe_plan()
        || p[8] != h.target_ref_digest()
        || Some(p[9]) != h.before_build()
        || p[10] != h.after_build()
        || p[11] != parse(before.executable_digest)?
        || p[12] != parse(after.executable_digest)?
    {
        return Err(CodingResumeError::BindingMismatch);
    }
    Ok(())
}
