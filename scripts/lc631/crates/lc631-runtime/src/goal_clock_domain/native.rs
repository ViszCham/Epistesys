//! Registered native observations are born from the actual retained before
//! capture and the role's execution endpoint. A legacy observation cannot be
//! converted. The process runs through the same checked four-probe worker.
#![forbid(unsafe_code)]
use super::{ClockFault, Error, Profile, RegisteredCapture, RoleSession};
use crate::contextual_repair::{
    execution::{observe_registered_run, GoalEpochError, GoalInvocationGate, NativeRun},
    measurement::{CodingProbePlan, OwnedCodingProbeObservation, ProbeError},
};
use lc631_analysis::{
    declared_build_capture::{declared_capture_digest, DeclaredBuildContentPins},
    ResourceToolResult,
};
use lc631_substrate::{resource::ResourceReservationView, Digest32};
use std::path::{Component, Path};

pub use crate::contextual_repair::execution::ProbeInputs;

fn probe_error(error: ProbeError) -> Error {
    match error {
        ProbeError::ClockDomain(fault) => Error::Clock(fault),
        ProbeError::Resource(resource) => Error::Clock(ClockFault::Resource(resource)),
        other => Error::Probe(other),
    }
}

fn gate_error(error: GoalEpochError) -> Error {
    match error {
        GoalEpochError::ObservedExpiry => Error::Clock(ClockFault::ObservedExpiry),
        GoalEpochError::Probe(error) => probe_error(error),
    }
}

/// The failure is held by its role until that session is dropped. A missing
/// process result means the lower API returned no owned capture; it says
/// nothing about whether a native child had started.
pub struct RegisteredNativeFailure<'v> {
    cause: Error,
    run: NativeRun<'v>,
}

impl<'v> RegisteredNativeFailure<'v> {
    pub fn cause(&self) -> &Error {
        &self.cause
    }

    pub fn lower_dispatch_entered(&self) -> bool {
        self.run.entered_lower
    }

    pub fn process(&self) -> Option<&ResourceToolResult<'v>> {
        self.run.capture.as_ref()
    }

    pub fn parsed_outputs(&self) -> Option<&[Option<u32>; 4]> {
        self.run.parsed_outputs.as_ref()
    }
}

pub struct RegisteredObservation<'o, 'p, 'v> {
    session: &'o RoleSession<'o, 'p, 'v>,
    before: &'o RegisteredCapture<'o, 'p, 'v>,
    plan: &'o CodingProbePlan,
    probe: OwnedCodingProbeObservation<'v>,
    process: ResourceToolResult<'v>,
}

impl<'s, 'p, 'v> RoleSession<'s, 'p, 'v> {
    pub fn observe_native<'o>(
        &'o self,
        plan: &'o CodingProbePlan,
        input: ProbeInputs<'_>,
        before: &'o RegisteredCapture<'o, 'p, 'v>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<RegisteredObservation<'o, 'p, 'v>, Error>
    where
        's: 'o,
    {
        view.check()
            .map_err(|error| Error::Clock(ClockFault::Resource(error)))?;
        // An irreversible retained failure must refuse before registry/provider
        // callbacks. Recheck after admission as protection against reentry.
        if self
            .native_failure_slot()
            .try_borrow()
            .map_err(|_| Error::Clock(ClockFault::BudgetExhausted))?
            .is_some()
        {
            return Err(Error::Clock(ClockFault::BudgetExhausted));
        }
        let (reader, ledger) = self.native_clock(before, view)?;
        let mut failure_slot = self
            .native_failure_slot()
            .try_borrow_mut()
            .map_err(|_| Error::Clock(ClockFault::BudgetExhausted))?;
        if failure_slot.is_some() {
            return Err(Error::Clock(ClockFault::BudgetExhausted));
        }
        self.bind_native_origin(plan, input, before, view)?;
        let mut run = observe_registered_run(plan, input, view, reader, ledger);
        let result = match run.diagnostic.clone() {
            Some(error) => Err(probe_error(error)),
            None => self.native_clock(before, view).map(|_| ()),
        };
        if let Err(cause) = result {
            // Reserve one concrete slot before calling the lower producer.
            // Do not replace or reconstruct a previous uncertain effect.
            if run.entered_lower {
                *failure_slot = Some(RegisteredNativeFailure {
                    cause: cause.clone(),
                    run,
                });
            }
            return Err(cause);
        }
        // The common producer succeeds only with both real owners present.
        match (run.observed.take(), run.capture.take()) {
            (Some(probe), Some(process)) => Ok(RegisteredObservation {
                session: self,
                before,
                plan,
                probe,
                process,
            }),
            (observation, capture) => {
                run.observed = observation;
                run.capture = capture;
                let cause = Error::Probe(ProbeError::ProcessFailed);
                if run.entered_lower {
                    *failure_slot = Some(RegisteredNativeFailure {
                        cause: cause.clone(),
                        run,
                    });
                }
                Err(cause)
            }
        }
    }

    fn bind_native_origin(
        &self,
        plan: &CodingProbePlan,
        input: ProbeInputs<'_>,
        before: &RegisteredCapture<'_, 'p, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        view.check()
            .map_err(|e| Error::Clock(ClockFault::Resource(e)))?;
        if !std::ptr::eq(plan, self.plan()) {
            return Err(Error::Clock(ClockFault::BindingMismatch));
        }
        crate::contextual_repair::execution::preflight_registered_inputs(plan, input, view)
            .map_err(probe_error)?;
        let origin = before.actual();
        let binding = self.native_binding();
        let registration = input.registration;
        let snapshot = registration.snapshot;
        let root = registered_target_root(registration.target_ref, registration.target_path)?;
        if plan.digest() != binding.probe_plan
            || binding.source != view.source_revision()
            || binding.root != root
            || origin.root() != root
            || origin.program() != binding.program
            || origin.plan() != binding.completion_plan
            || origin.snapshot_digest()
                != declared_capture_digest(&snapshot.snapshot_digest).map_err(Error::Snapshot)?
        {
            return Err(Error::Clock(ClockFault::BindingMismatch));
        }
        let pins = DeclaredBuildContentPins::from_snapshot(snapshot).map_err(Error::Snapshot)?;
        if pins.source != binding.source {
            return Err(Error::Clock(ClockFault::BindingMismatch));
        }
        origin.check_current(view).map_err(Error::Snapshot)?;
        origin.matches_expected(pins).map_err(Error::Snapshot)
    }

    /// Access only historical failure facts; never publish an observation or
    /// permit through this getter, and never reopen the slot to a second child.
    pub fn with_native_failure<R>(
        &self,
        inspect: impl for<'read> FnOnce(Option<&'read RegisteredNativeFailure<'v>>) -> R,
    ) -> Result<R, Error> {
        let failure = self
            .native_failure_slot()
            .try_borrow()
            .map_err(|_| Error::Clock(ClockFault::BudgetExhausted))?;
        Ok(inspect(failure.as_ref()))
    }
}

fn registered_target_root(target_ref: &str, path: &Path) -> Result<Digest32, Error> {
    let relative = Path::new(target_ref);
    let mut root = path;
    if !path.is_absolute() || !path.ends_with(relative) || target_ref.is_empty() {
        return Err(Error::Clock(ClockFault::BindingMismatch));
    }
    for (i, part) in relative.components().enumerate() {
        if i >= 32 || !matches!(part, Component::Normal(_)) {
            return Err(Error::Clock(ClockFault::BindingMismatch));
        }
        root = root
            .parent()
            .ok_or(Error::Clock(ClockFault::BindingMismatch))?;
    }
    let digest = lc631_analysis::dgcl_repository_root_digest(root)
        .map_err(|_| Error::Clock(ClockFault::BindingMismatch))?;
    declared_capture_digest(&digest).map_err(Error::Snapshot)
}

impl<'o, 'p, 'v> RegisteredObservation<'o, 'p, 'v> {
    pub fn actual_outputs(&self) -> &[Option<u32>; 4] {
        self.probe.actual_outputs()
    }
    pub fn checked_epoch(&self) -> u64 {
        self.probe.checked_epoch()
    }
    pub fn observation_digest(&self) -> Digest32 {
        self.probe.observation_digest()
    }
    pub fn probe_observation(&self) -> &OwnedCodingProbeObservation<'v> {
        &self.probe
    }
    pub fn process(&self) -> &ResourceToolResult<'v> {
        &self.process
    }
    pub(crate) fn session(&self) -> &'o RoleSession<'o, 'p, 'v> {
        self.session
    }
    pub(crate) fn origin(&self) -> &'o RegisteredCapture<'o, 'p, 'v> {
        self.before
    }

    pub fn check_current(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.session.native_clock(self.before, view)?;
        self.probe
            .check_current(self.plan, view)
            .map_err(probe_error)
    }

    pub(crate) fn check_invocation_current(
        &self,
        plan: &CodingProbePlan,
        input: ProbeInputs<'_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<u64, Error> {
        if !std::ptr::eq(plan, self.plan) {
            return Err(Error::Clock(ClockFault::BindingMismatch));
        }
        self.session.native_clock(self.before, view)?;
        self.session
            .bind_native_origin(plan, input, self.before, view)?;
        let gate = GoalInvocationGate::admit_registered(plan, &self.probe, input, view)
            .map_err(probe_error)?;
        gate.sample_current_epoch()
            .map(|token| token.epoch())
            .map_err(gate_error)
    }

    pub(crate) fn check_artifact_current(
        &self,
        plan: &CodingProbePlan,
        input: ProbeInputs<'_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        if !std::ptr::eq(plan, self.plan) {
            return Err(Error::Clock(ClockFault::BindingMismatch));
        }
        self.session.native_clock(self.before, view)?;
        self.session
            .bind_native_origin(plan, input, self.before, view)?;
        let gate = GoalInvocationGate::admit_registered(plan, &self.probe, input, view)
            .map_err(probe_error)?;
        gate.check_artifact_current().map_err(gate_error)
    }
}

pub fn check_observation_for_registered_profile(
    observation: &OwnedCodingProbeObservation<'_>,
    plan: &CodingProbePlan,
    profile: &Profile<'_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    observation
        .registered_profile_current(plan, profile, view)
        .map_err(probe_error)
}
