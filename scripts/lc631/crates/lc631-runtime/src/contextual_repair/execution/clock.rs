//! Clock-free invocation metadata and the two actual clock sources used by the
//! one native worker. Registration is supplied only by a checked role session.
use super::{CodingProbePlan, ObservedEpoch, ProbeError, ProbeInvocation, ProbeToolRegistration};
use crate::goal_clock_domain::{ClockFault, ClockGap, EndpointReader, Profile, Purpose, Role};
use lc631_analysis::DgclEvidenceLease;
use lc631_core::{ExecutionPermit, ExecutionPermitContext};
use lc631_receipt_kernel::{ReceiptVerifier, UntrustedReceipt};
use lc631_substrate::resource::ResourceReservationView;
use std::sync::Arc;

/// Borrows the exact input objects, deliberately excluding the legacy clock.
#[derive(Clone, Copy)]
pub struct ProbeInputs<'a> {
    pub(crate) source: &'a str,
    pub(crate) registration: &'a ProbeToolRegistration<'a>,
    pub(crate) lease: &'a DgclEvidenceLease,
    pub(crate) tool_receipt: &'a UntrustedReceipt,
    pub(crate) tool_verifier: &'a ReceiptVerifier,
    pub(crate) permit: &'a ExecutionPermit,
    pub(crate) authority: &'a ExecutionPermitContext<'a>,
    pub(crate) timeout_ms: u64,
}

impl<'a> ProbeInputs<'a> {
    pub fn without_clock(input: &'a ProbeInvocation<'_>) -> Self {
        Self {
            source: input.source,
            registration: &input.registration,
            lease: input.lease,
            tool_receipt: input.tool_receipt,
            tool_verifier: input.tool_verifier,
            permit: input.permit,
            authority: input.authority,
            timeout_ms: input.timeout_ms,
        }
    }
}

/// A registered gain borrows the same funded reader as its observation. It
/// acquires no grant and does not keep the mutable snapshot-grant actor.
#[derive(Clone)]
pub(crate) enum ObservationClock<'v> {
    Legacy(fn() -> u64),
    Registered(EndpointReader<'v>),
}

impl ObservationClock<'_> {
    pub(crate) fn is_legacy(&self) -> bool {
        matches!(self, Self::Legacy(_))
    }

    pub(crate) fn sample(
        &self,
        ledger: &ObservedEpoch,
        expires: u64,
        view: &ResourceReservationView<'_>,
    ) -> Result<u64, ProbeError> {
        match self {
            Self::Legacy(clock) => {
                let now = clock();
                ledger.observe(now, expires)?;
                Ok(now)
            }
            Self::Registered(reader) => reader
                .sample(view, |now| observe_registered(ledger, now, expires))
                .map_err(ProbeError::ClockDomain),
        }
    }

    pub(crate) fn check_pair(&self, other: &Self) -> Result<(), ProbeError> {
        match (self, other) {
            (Self::Legacy(_), Self::Legacy(_)) => Ok(()),
            (Self::Registered(a), Self::Registered(b)) => {
                if !a.same_profile(b) {
                    return Err(ProbeError::ClockDomain(ClockFault::NeedsEvidence(
                        ClockGap::IncomparableDomains,
                    )));
                }
                let x = a.binding();
                let y = b.binding();
                if x.role != Role::Baseline || y.role != Role::Candidate
                    || x.purpose != Purpose::Execution || y.purpose != Purpose::Execution
                    || x.source != y.source || x.probe_plan != y.probe_plan
                {
                    return Err(ProbeError::ClockDomain(ClockFault::BindingMismatch));
                }
                Ok(())
            }
            _ => Err(ProbeError::ClockDomain(ClockFault::NeedsEvidence(
                ClockGap::LegacyClockProvenanceUnassessed,
            ))),
        }
    }

    pub(crate) fn check_registered_profile(
        &self, profile: &Profile<'_>, view: &ResourceReservationView<'_>,
    ) -> Result<(), ClockFault> {
        match self {
            Self::Registered(reader) => reader.check_profile_current(profile, view),
            Self::Legacy(_) => Err(ClockFault::NeedsEvidence(ClockGap::LegacyClockProvenanceUnassessed)),
        }
    }
}

fn observe_registered(ledger: &ObservedEpoch, now: u64, expires: u64) -> Result<(), ClockFault> {
    match ledger.observe(now, expires) {
        Ok(()) => Ok(()),
        Err(ProbeError::ClockRollback) => Err(ClockFault::ClockRollback),
        Err(ProbeError::Stale) if now >= expires => Err(ClockFault::ObservedExpiry),
        // ObservedEpoch::observe currently produces only those two errors.
        Err(_) => Err(ClockFault::BindingMismatch),
    }
}

pub(super) enum NativeBirth<'v> {
    Legacy(fn() -> u64),
    Registered(EndpointReader<'v>, Arc<ObservedEpoch>),
}

impl<'v> NativeBirth<'v> {
    pub(super) fn start(
        self,
        expires: u64,
        view: &ResourceReservationView<'_>,
    ) -> Result<(ObservationClock<'v>, Arc<ObservedEpoch>, u64), ProbeError> {
        match self {
            Self::Legacy(clock) => {
                let birth = clock();
                Ok((ObservationClock::Legacy(clock), ObservedEpoch::shared(birth), birth))
            }
            Self::Registered(reader, ledger) => {
                let clock = ObservationClock::Registered(reader);
                let birth = clock.sample(&ledger, expires, view)?;
                Ok((clock, ledger, birth))
            }
        }
    }
}

impl super::OwnedCodingProbeObservation<'_> {
    pub(crate) fn registered_profile_current(
        &self,
        plan: &CodingProbePlan,
        profile: &Profile<'_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), ProbeError> {
        view.check()?;
        self.retained.check()?;
        if !self.retained.same_reservation(view) || self.source != view.source_revision()
            || self.plan != plan.digest()
        {
            return Err(ProbeError::BindingMismatch);
        }
        self.clock.check_registered_profile(profile, view).map_err(ProbeError::ClockDomain)?;
        self.check_current(plan, view)
    }
}
