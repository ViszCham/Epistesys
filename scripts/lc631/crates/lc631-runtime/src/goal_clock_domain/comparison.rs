//! One registered comparison over the existing Goal artifact owner and its exact
//! four-output evaluator. Registration binds clocks, not semantic acceptance.
use super::{ClockFault, Error, ProbeInputs, RegisteredCapture, RegisteredObservation, Role, RoleSession};
use crate::{
    contextual_repair::{measurement::CodingProbePlan, snapshot_gate::CodingBuildPort},
    goal_suite_measurement as goal,
};
use lc631_analysis::creativity_measurement::{RationalInterval, SemanticValidation};
use lc631_substrate::resource::ResourceReservationView;
use std::ptr;

pub struct RegisteredRootInput<'o, 'p, 'v> {
    pub plan: &'o CodingProbePlan,
    pub pins: goal::ArtifactPins,
    pub limits: goal::Limits,
    pub baseline_session: &'o RoleSession<'o, 'p, 'v>,
    pub candidate_session: &'o RoleSession<'o, 'p, 'v>,
    pub baseline_before: &'o RegisteredCapture<'o, 'p, 'v>,
    pub candidate_expected: &'o RegisteredCapture<'o, 'p, 'v>,
}

pub struct RegisteredArtifactRoot<'o, 'p, 'v> {
    pub(crate) artifact: goal::ArtifactPairRoot<'o, 'v>,
    plan: &'o CodingProbePlan,
    pub(crate) baseline: &'o RoleSession<'o, 'p, 'v>,
    pub(crate) candidate: &'o RoleSession<'o, 'p, 'v>,
    pub(crate) before: &'o RegisteredCapture<'o, 'p, 'v>,
    pub(crate) expected: &'o RegisteredCapture<'o, 'p, 'v>,
}
impl<'o, 'p, 'v> RegisteredArtifactRoot<'o, 'p, 'v> {
    pub fn checked(input: RegisteredRootInput<'o, 'p, 'v>, view: &'v ResourceReservationView<'_>)
        -> Result<Self, Error>
    {
        view.check()?;
        if input.baseline_session.role() != Role::Baseline
            || input.candidate_session.role() != Role::Candidate
            || !ptr::eq(input.baseline_session.profile(), input.candidate_session.profile())
            || !ptr::eq(input.baseline_session.plan(), input.plan)
            || !ptr::eq(input.candidate_session.plan(), input.plan)
            || !ptr::eq(input.baseline_before.session(), input.baseline_session)
            || !ptr::eq(input.candidate_expected.session(), input.candidate_session)
            || !input.baseline_before.is_before()
            || !input.candidate_expected.is_before()
        {
            return Err(ClockFault::BindingMismatch.into());
        }
        input.baseline_session.check_current_binding(view)?;
        input.candidate_session.check_current_binding(view)?;
        let artifact = goal::ArtifactPairRoot::given(goal::ArtifactRootInput {
            plan: input.plan,
            pins: input.pins,
            limits: input.limits,
            baseline_before: Some(input.baseline_before.actual()),
            candidate_expected: input.candidate_expected.actual(),
        }, view)?;
        Ok(Self {
            artifact, plan: input.plan,
            baseline: input.baseline_session, candidate: input.candidate_session,
            before: input.baseline_before, expected: input.candidate_expected,
        })
    }
    pub fn plan(&self) -> &CodingProbePlan { self.plan }

    pub(crate) fn check_occurrences(
        &self, pair: RegisteredPair<'_, 'p, 'v>, view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        view.check()?;
        if !ptr::eq(pair.baseline.session(), self.baseline)
            || !ptr::eq(pair.candidate.session(), self.candidate)
            || !ptr::eq(pair.baseline.origin(), self.before)
            || !ptr::eq(pair.candidate.origin(), self.expected)
        {
            return Err(ClockFault::BindingMismatch.into());
        }
        self.baseline.check_current_binding(view)?;
        self.candidate.check_current_binding(view)
    }
}
#[derive(Clone, Copy)]
pub struct RegisteredPair<'o, 'p, 'v> {
    pub baseline: &'o RegisteredObservation<'o, 'p, 'v>,
    pub candidate: &'o RegisteredObservation<'o, 'p, 'v>,
}
impl<'o, 'p, 'v> RegisteredPair<'o, 'p, 'v> {
    pub(crate) fn actual(self) -> goal::CodingPair<'o, 'v> {
        goal::CodingPair {
            baseline: self.baseline.probe_observation(),
            candidate: self.candidate.probe_observation(),
        }
    }
}

pub struct CurrentPorts<'g, 'bi, 'br, 'ci, 'cr> {
    pub baseline_probe: ProbeInputs<'g>,
    pub candidate_probe: ProbeInputs<'g>,
    pub baseline_build: &'g mut CodingBuildPort<'bi, 'br>,
    pub candidate_build: &'g mut CodingBuildPort<'ci, 'cr>,
}

pub struct OwnedRegisteredPair<'r, 'o, 'p, 'v> {
    root: &'r RegisteredArtifactRoot<'o, 'p, 'v>,
    pair: RegisteredPair<'o, 'p, 'v>,
    artifact: goal::OwnedArtifactPair<'r, 'o, 'o, 'v>,
}
pub fn measure_clock_bound<'r, 'o, 'p, 'v>(
    root: &'r RegisteredArtifactRoot<'o, 'p, 'v>,
    pair: RegisteredPair<'o, 'p, 'v>,
    ports: &mut CurrentPorts<'_, '_, '_, '_, '_>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedRegisteredPair<'r, 'o, 'p, 'v>, Error> {
    root.check_occurrences(pair, view)?;
    let artifact = root.measure_registered(pair, ports, view)?;
    Ok(OwnedRegisteredPair { root, pair, artifact })
}
impl<'r, 'o, 'p, 'v> OwnedRegisteredPair<'r, 'o, 'p, 'v> {
    pub fn with_current<R>(
        &self, root: &'r RegisteredArtifactRoot<'o, 'p, 'v>, pair: RegisteredPair<'o, 'p, 'v>,
        ports: &mut CurrentPorts<'_, '_, '_, '_, '_>, view: &'v ResourceReservationView<'_>,
        consume: impl for<'current> FnOnce(RegisteredView<'current>) -> R,
    ) -> Result<R, Error> {
        if !ptr::eq(self.root, root) || !ptr::eq(self.pair.baseline, pair.baseline)
            || !ptr::eq(self.pair.candidate, pair.candidate)
        {
            return Err(ClockFault::BindingMismatch.into());
        }
        root.check_occurrences(pair, view)?;
        self.artifact.with_registered(root, pair, ports, view, |actual| consume(RegisteredView(actual)))
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockBasis { LegacyClockProvenanceUnassessed, GivenLocalRegistered }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostAuthenticity { Unavailable }
pub struct RegisteredView<'a>(goal::ArtifactView<'a>);
impl<'a> RegisteredView<'a> {
    pub fn baseline_outputs(&self) -> &'a [Option<u32>;4] { self.0.baseline_outputs() }
    pub fn candidate_outputs(&self) -> &'a [Option<u32>;4] { self.0.candidate_outputs() }
    pub fn requested_source_goal(&self) -> Option<&'a RationalInterval> { self.0.requested_source_goal() }
    pub fn source_eligibility(&self) -> goal::SourceEligibility { self.0.source_eligibility() }
    pub fn clock_basis(&self) -> ClockBasis { self.0.clock_basis() }
    pub fn host_authenticity(&self) -> HostAuthenticity { HostAuthenticity::Unavailable }
    pub fn semantic_acceptance(&self) -> SemanticValidation { SemanticValidation::Unassessed }
    /// The actual existing conditional Goal view, with its original reports and bindings.
    pub fn artifact(&self) -> &goal::ArtifactView<'a> { &self.0 }
}
