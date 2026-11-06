//! A role has two registered endpoints, one actual-sample history and one capture origin.
//! Separate roles do not compare their numerical high-water marks.
#![forbid(unsafe_code)]

mod capture;
mod grant;
mod provider;

pub use capture::{CaptureAttempt, RegisteredCapture};
pub use grant::{SnapshotGrantSource, SnapshotGrantView};

use super::{
    native::RegisteredNativeFailure, ClockFault, ClockGap, EndpointBinding, EndpointReader, Error,
    Profile, Protocol, Purpose, RegisteredEndpoint, Role,
};
use crate::contextual_repair::measurement::{CodingProbePlan, ObservedEpoch, ProbeError};
use lc631_analysis::declared_build_capture::SourceReadBinding;
use lc631_substrate::resource::ResourceReservationView;
use std::{
    cell::{Cell, RefCell},
    sync::{Arc, OnceLock},
};

/// The profile funds one role's bookkeeping before this owner is published.
/// A failed capture keeps any epoch actually read; a new attempt never resets it.
pub struct RoleSession<'o, 'p, 'v> {
    profile: &'p Profile<'v>,
    execution: &'o RegisteredEndpoint<'p, 'v>,
    capture: &'o RegisteredEndpoint<'p, 'v>,
    plan: &'o CodingProbePlan,
    grant_scope: String,
    history: OnceLock<Arc<ObservedEpoch>>,
    origin_ready: Cell<bool>,
    capture_in_progress: Cell<bool>,
    native_failure: RefCell<Option<RegisteredNativeFailure<'v>>>,
}

impl<'o, 'p, 'v> RoleSession<'o, 'p, 'v> {
    pub fn bind(
        profile: &'p Profile<'v>,
        execution: &'o RegisteredEndpoint<'p, 'v>,
        capture: &'o RegisteredEndpoint<'p, 'v>,
        binding: &SourceReadBinding<'_>,
        plan: &'o CodingProbePlan,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        profile.check(view).map_err(Error::Clock)?;
        for endpoint in [execution, capture] {
            if endpoint.profile().domain() != profile.domain() {
                return Err(Error::Clock(ClockFault::NeedsEvidence(
                    ClockGap::IncomparableDomains,
                )));
            }
            if !matches!(endpoint.profile().protocol(), Protocol::EpochSecondsV1) {
                return Err(Error::Clock(ClockFault::NeedsEvidence(
                    ClockGap::UnknownProtocol,
                )));
            }
            if !std::ptr::eq(endpoint.profile(), profile)
                || !matches_source(endpoint.binding(), binding, plan)
            {
                return Err(Error::Clock(ClockFault::BindingMismatch));
            }
        }
        let exec = execution.binding();
        let cap = capture.binding();
        if exec.purpose != Purpose::Execution
            || cap.purpose != Purpose::Capture
            || exec.role != cap.role
            || capture.snapshot_grant().is_none()
        {
            return Err(Error::Clock(ClockFault::BindingMismatch));
        }
        execution.check_registration_current(view)?;
        capture.check_registration_current(view)?;
        binding
            .check_goal_profile_inputs(view)
            .map_err(Error::Snapshot)?;
        let grant_scope = binding.execution_scope();
        profile.reserve_role(exec.role, view).map_err(Error::Clock)?;
        Ok(Self {
            profile,
            execution,
            capture,
            plan,
            grant_scope,
            history: OnceLock::new(),
            origin_ready: Cell::new(false),
            capture_in_progress: Cell::new(false),
            native_failure: RefCell::new(None),
        })
    }

    pub(crate) fn role(&self) -> Role {
        self.execution.binding().role
    }

    pub(crate) fn profile(&self) -> &'p Profile<'v> {
        self.profile
    }

    pub(crate) fn plan(&self) -> &'o CodingProbePlan {
        self.plan
    }

    pub(crate) fn native_binding(&self) -> EndpointBinding {
        *self.execution.binding()
    }

    pub(crate) fn native_failure_slot(&self) -> &RefCell<Option<RegisteredNativeFailure<'v>>> {
        &self.native_failure
    }

    /// These are registry/lease checks only. They do not call either raw clock.
    pub(crate) fn check_current_binding(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.profile.check(view).map_err(Error::Clock)?;
        self.execution.check_registration_current(view)?;
        self.capture.check_registration_current(view)
    }

    pub(crate) fn native_clock(
        &self,
        before: &RegisteredCapture<'_, 'p, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(EndpointReader<'v>, Arc<ObservedEpoch>), Error> {
        if !before.is_before() || !std::ptr::eq(before.session(), self) || !self.origin_ready.get()
        {
            return Err(Error::Clock(ClockFault::BindingMismatch));
        }
        self.check_current_binding(view)?;
        before
            .actual()
            .check_current(view)
            .map_err(Error::Snapshot)?;
        let history = self
            .history
            .get()
            .ok_or(Error::Clock(ClockFault::BindingMismatch))?;
        Ok((self.execution.reader(), Arc::clone(history)))
    }

    fn record_sample(&self, epoch: u64, expires: u64) -> Result<(), ClockFault> {
        let ledger = self.history.get_or_init(|| ObservedEpoch::shared(epoch));
        ledger
            .observe(epoch, expires)
            .map_err(|reason| match reason {
                ProbeError::ClockRollback => ClockFault::ClockRollback,
                ProbeError::Stale => ClockFault::ObservedExpiry,
                ProbeError::Resource(error) => ClockFault::Resource(error),
                ProbeError::ClockDomain(fault) => fault,
                // ObservedEpoch only emits rollback/expiry for these operations.
                _ => ClockFault::BindingMismatch,
            })
    }
}

fn matches_source(
    endpoint: &EndpointBinding,
    binding: &SourceReadBinding<'_>,
    plan: &CodingProbePlan,
) -> bool {
    endpoint.root == binding.root()
        && endpoint.source == binding.source()
        && endpoint.program == binding.program()
        && endpoint.completion_plan == binding.plan()
        && endpoint.probe_plan == plan.digest()
}
