//! Concrete repair owner; accepted validation precedes measured publication.
use super::{parse_digest, CodingProbePlan, OwnedCodingProbeObservation, ProbeError};
use crate::contextual_repair::measurement::{paired_gain, select_current, OwnedCodingProbeGain};
use crate::contextual_repair::snapshot_gate::{ExpectedCurrentBuild, SnapshotGateError};
use crate::contextual_resume::budget::BudgetedFileRepairDriver;
use lc631_analysis::declared_build_capture::SnapshotCaptureAttempt;
use lc631_analysis::{
    DgclRepairApplication, DgclRepairDriver, DgclRepairDriverError, DgclRepairRequest,
    DgclRepairValidation, DgclVerifiedRepairApplication, DgclVerifiedRepairValidation,
};
use lc631_core::ExecutionPermit;
use lc631_substrate::resource::{ResourceClaim, ResourceReservationView};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodingBuildProfile {
    PhysicalOnly,
    DeclaredInputsChecked,
    DeclaredInputsRejected,
}

type ApplicationObserver<'observer> = dyn for<'borrow, 'token> FnMut(
        &'borrow DgclVerifiedRepairApplication<'token>,
    ) -> Result<(), ProbeError>
    + 'observer;

/// The observation closure must invoke observe_current with its own fresh Test
/// permission/registered artifact. It cannot return a fabricated public report.
/// Only the concrete durable file driver is accepted, so the real application
/// token continues through its existing private settlement boundary.
pub struct RemeasuringRepairDriver<'a, 'v, 'driver, 'budget, 'trust> {
    inner: BudgetedFileRepairDriver<'driver, 'budget, 'trust>,
    plan: &'a CodingProbePlan,
    baseline: &'a OwnedCodingProbeObservation<'v>,
    resources: &'v ResourceReservationView<'v>,
    observe: &'a mut dyn FnMut(
        &DgclVerifiedRepairValidation<'_>,
    ) -> Result<OwnedCodingProbeObservation<'v>, ProbeError>,
    application_observer: Option<&'a mut ApplicationObserver<'a>>,
    observations: Vec<OwnedCodingProbeObservation<'v>>,
    gains: Vec<OwnedCodingProbeGain<'v>>,
    diagnostic: Option<ProbeError>,
    expected_build: Option<ExpectedCurrentBuild>,
    captures: Vec<SnapshotCaptureAttempt<'v>>,
    build_profile: CodingBuildProfile,
    _metadata: ResourceClaim<'v>,
}
impl<'a, 'v, 'driver, 'budget, 'trust> RemeasuringRepairDriver<'a, 'v, 'driver, 'budget, 'trust> {
    pub fn new(
        inner: BudgetedFileRepairDriver<'driver, 'budget, 'trust>,
        plan: &'a CodingProbePlan,
        baseline: &'a OwnedCodingProbeObservation<'v>,
        resources: &'v ResourceReservationView<'v>,
        observe: &'a mut dyn FnMut(
            &DgclVerifiedRepairValidation<'_>,
        ) -> Result<OwnedCodingProbeObservation<'v>, ProbeError>,
    ) -> Result<Self, ProbeError> {
        baseline.check_current(plan, resources)?;
        let metadata = crate::contextual_repair::measurement::claim(resources, 16 * 1024)?;
        let mut observations = Vec::new();
        let mut gains = Vec::new();
        let mut captures = Vec::new();
        observations
            .try_reserve_exact(4)
            .map_err(|_| ProbeError::InvalidPlan)?;
        gains
            .try_reserve_exact(4)
            .map_err(|_| ProbeError::InvalidPlan)?;
        captures
            .try_reserve_exact(8)
            .map_err(|_| ProbeError::InvalidPlan)?;
        Ok(Self {
            inner,
            plan,
            baseline,
            resources,
            observe,
            application_observer: None,
            observations,
            gains,
            diagnostic: None,
            expected_build: None,
            captures,
            build_profile: CodingBuildProfile::PhysicalOnly,
            _metadata: metadata,
        })
    }
    /// Observe historical identities before the real token is consumed. Even an
    /// observer failure still forwards the actual effect to the budget owner.
    pub fn with_application_observer(mut self, observer: &'a mut ApplicationObserver<'a>) -> Self {
        self.application_observer = Some(observer);
        self
    }
    pub fn diagnostic(&self) -> Option<&ProbeError> {
        self.diagnostic.as_ref()
    }
    pub fn budget_summary(
        &self,
    ) -> Result<
        crate::contextual_resume::budget::BudgetSummary,
        crate::contextual_resume::budget::BudgetError,
    > {
        self.inner.budget_summary()
    }
    pub fn observations(&self) -> &[OwnedCodingProbeObservation<'v>] {
        &self.observations
    }
    pub fn gains(&self) -> &[OwnedCodingProbeGain<'v>] {
        &self.gains
    }
    pub fn build_profile(&self) -> CodingBuildProfile {
        self.build_profile
    }
    pub fn build_captures(&self) -> &[SnapshotCaptureAttempt<'v>] {
        &self.captures
    }
    /// Strong profile: missing authority never falls back to physical-only.
    pub fn recommendation_declared_current(&mut self) -> Result<Option<usize>, ProbeError> {
        if !self
            .observations
            .last()
            .is_some_and(|owner| owner.has_build_authority())
        {
            return Err(ProbeError::BuildSnapshot(
                SnapshotGateError::MissingBuildAuthority,
            ));
        }
        self.recommendation_current()
    }
    fn recapture_build(&mut self) -> Result<(), ProbeError> {
        self.build_profile = CodingBuildProfile::DeclaredInputsRejected;
        let expected = self.expected_build.ok_or(ProbeError::BuildSnapshot(
            SnapshotGateError::BindingMismatch,
        ))?;
        if self.captures.len() >= 8 {
            return Err(ProbeError::InvalidPlan);
        }
        let current = self
            .observations
            .last_mut()
            .ok_or(ProbeError::InvalidPlan)?;
        let authority = current
            .build_authority
            .as_deref_mut()
            .ok_or(ProbeError::BuildSnapshot(
                SnapshotGateError::MissingBuildAuthority,
            ))?;
        let attempt = self.inner.recapture_current_build(
            expected.root(),
            expected.program(),
            expected.plan(),
            authority,
            self.resources,
        );
        // Save actual capture and late rejection before inspecting its result.
        self.captures.push(attempt);
        expected
            .check_attempt(
                self.captures.last().ok_or(ProbeError::InvalidPlan)?,
                self.resources,
            )
            .map_err(ProbeError::BuildSnapshot)?;
        self.build_profile = CodingBuildProfile::DeclaredInputsChecked;
        Ok(())
    }
    pub fn recommendation_current(&mut self) -> Result<Option<usize>, ProbeError> {
        self._metadata.check()?;
        if self.diagnostic.is_some() {
            return Err(ProbeError::Stale);
        }
        // Earlier owners/gains are retained historical alternatives. Only the
        // last actual repaired target is advertised as physically current.
        if let Some(current) = self.observations.last() {
            if let Err(error) = current.check_current_artifact(self.plan, self.resources) {
                self.diagnostic = Some(error.clone());
                return Err(error);
            }
        }
        // Legacy physical-only callers expose that bounded profile explicitly.
        // An attached provider always invokes the actual concrete source reader.
        if self
            .observations
            .last()
            .is_some_and(|owner| owner.has_build_authority())
        {
            if let Err(error) = self.recapture_build() {
                self.diagnostic = Some(error.clone());
                return Err(error);
            }
            if let Err(error) = self
                .observations
                .last()
                .ok_or(ProbeError::InvalidPlan)?
                .check_current_artifact(self.plan, self.resources)
            {
                self.build_profile = CodingBuildProfile::DeclaredInputsRejected;
                self.diagnostic = Some(error.clone());
                return Err(error);
            }
        }
        // Explicit bounded views avoid an unaccounted temporary allocation.
        match self.gains.as_slice() {
            [] => select_current(self.plan, &[], self.resources),
            [a] => select_current(self.plan, &[a], self.resources),
            [a, b] => select_current(self.plan, &[a, b], self.resources),
            [a, b, c] => select_current(self.plan, &[a, b, c], self.resources),
            [a, b, c, d] => select_current(self.plan, &[a, b, c, d], self.resources),
            _ => Err(ProbeError::InvalidPlan),
        }
    }
}
impl DgclRepairDriver for RemeasuringRepairDriver<'_, '_, '_, '_, '_> {
    fn observation_epoch(&self) -> Option<u64> {
        self.inner.observation_epoch()
    }
    fn apply(
        &mut self,
        r: &DgclRepairRequest,
        p: &ExecutionPermit,
        t: Duration,
        d: u64,
    ) -> Result<DgclRepairApplication, DgclRepairDriverError> {
        self.inner.apply(r, p, t, d)
    }
    fn on_verified_application(
        &mut self,
        t: DgclVerifiedRepairApplication<'_>,
    ) -> Result<(), DgclRepairDriverError> {
        let observed = match self.application_observer.as_mut() {
            Some(observer) => observer(&t),
            None => Ok(()),
        };
        self.inner.on_verified_application(t)?;
        observed.map_err(|error| {
            self.diagnostic = Some(error);
            DgclRepairDriverError::Failed
        })
    }
    fn revalidate(
        &mut self,
        r: &DgclRepairRequest,
        a: &DgclRepairApplication,
        t: Duration,
    ) -> Result<DgclRepairValidation, DgclRepairDriverError> {
        self.inner.revalidate(r, a, t)
    }
    fn on_verified_validation(
        &mut self,
        token: DgclVerifiedRepairValidation<'_>,
    ) -> Result<(), DgclRepairDriverError> {
        let result = (|| {
            self.resources.check()?;
            self._metadata.check()?;
            if self.diagnostic.is_some() {
                return Err(ProbeError::Stale);
            }
            if self.observations.len() >= 4 {
                return Err(ProbeError::InvalidPlan);
            }
            let observed = (self.observe)(&token)?;
            // Keep genuine observed history even if the postcallback physical
            // check rejects publication. Never erase it or refund real effects.
            self.observations.push(observed);
            let observed = self.observations.last().ok_or(ProbeError::InvalidPlan)?;
            observed.check_current(self.plan, self.resources)?;
            observed.binds_repair_target(token.request())?;
            if observed.snapshot()
                != parse_digest(&token.application().after_snapshot.snapshot_digest)?
                || observed.target() != parse_digest(&token.request().candidate_digest)?
            {
                return Err(ProbeError::BindingMismatch);
            }
            let gain = paired_gain(self.plan, self.baseline, observed, self.resources)?;
            self.gains.push(gain);
            observed.check_current_artifact(self.plan, self.resources)?;
            self.expected_build = Some(
                ExpectedCurrentBuild::from_verified_validation(
                    &token,
                    self.inner.declared_program_digest().map_err(|error| {
                        ProbeError::BuildSnapshot(SnapshotGateError::Capture(error))
                    })?,
                )
                .map_err(ProbeError::BuildSnapshot)?,
            );
            if observed.has_build_authority() {
                self.recapture_build()?;
                self.observations
                    .last()
                    .ok_or(ProbeError::InvalidPlan)?
                    .check_current_artifact(self.plan, self.resources)?;
            }
            Ok(())
        })();
        result.map_err(|error| {
            if self.build_profile == CodingBuildProfile::DeclaredInputsChecked {
                self.build_profile = CodingBuildProfile::DeclaredInputsRejected;
            }
            self.diagnostic = Some(error);
            DgclRepairDriverError::Failed
        })
    }
}
