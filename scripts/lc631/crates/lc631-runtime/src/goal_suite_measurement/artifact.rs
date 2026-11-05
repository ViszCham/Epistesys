//! Separate conditional artifact roots. Bare captures never construct verified origins.
use super::*;
use crate::contextual_repair::{
    execution::ProbeInvocation,
    measurement::{CodingProbePlan, OwnedCodingProbeObservation},
    snapshot_gate::{CodingBuildPort, ExpectedCurrentBuild},
};
use lc631_analysis::declared_build_capture::{CurrentSnapshotAuthority, OwnedDeclaredBuildCapture};
mod current;
mod measure;
pub use measure::{measure_artifact_pair, ArtifactView, OwnedArtifactPair};

pub struct ArtifactRootInput<'o, 'v> {
    pub plan: &'o CodingProbePlan,
    pub pins: ArtifactPins,
    pub baseline_before: Option<&'o OwnedDeclaredBuildCapture<'v>>,
    pub candidate_expected: &'o OwnedDeclaredBuildCapture<'v>,
    pub limits: Limits,
}
pub struct ArtifactPairRoot<'origin, 'v> {
    plan: &'origin CodingProbePlan,
    pins: ArtifactPins,
    baseline_before: &'origin OwnedDeclaredBuildCapture<'v>,
    candidate_expected: &'origin OwnedDeclaredBuildCapture<'v>,
    budget: Budget,
    identity: Digest32,
    epochs: Cell<[u64; 2]>,
    retained: ResourceClaim<'v>,
}
pub struct SourceCodingRootInput<'o> {
    pub plan: &'o CodingProbePlan,
    pub pins: SourcePins,
    pub baseline_expected: Option<ExpectedCurrentBuild>,
    pub candidate_expected: Option<ExpectedCurrentBuild>,
    pub limits: Limits,
}
/// A frozen verified-origin prerequisite root only. There is deliberately NO
/// source-goal measurement/recommendation API until the independent baseline
/// validated-origin production fixture is exercised. No bare-capture overload.
pub struct SourceCodingRoot<'origin, 'v> {
    plan: &'origin CodingProbePlan,
    pins: SourcePins,
    expected: [ExpectedCurrentBuild; 2],
    budget: Budget,
    retained: ResourceClaim<'v>,
}
#[derive(Clone, Copy)]
pub struct CodingPair<'o, 'v> {
    pub baseline: &'o OwnedCodingProbeObservation<'v>,
    pub candidate: &'o OwnedCodingProbeObservation<'v>,
}
pub struct ArtifactCurrent<'g, 'bi, 'br, 'ci, 'cr> {
    pub baseline_invocation: ProbeInvocation<'g>,
    pub candidate_invocation: ProbeInvocation<'g>,
    pub baseline_build: &'g mut CodingBuildPort<'bi, 'br>,
    pub candidate_build: &'g mut CodingBuildPort<'ci, 'cr>,
    pub baseline_authority: &'g mut dyn CurrentSnapshotAuthority,
    pub candidate_authority: &'g mut dyn CurrentSnapshotAuthority,
}
impl<'origin, 'v> ArtifactPairRoot<'origin, 'v> {
    pub fn given(
        input: ArtifactRootInput<'origin, 'v>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        view.check()?;
        let baseline = input.baseline_before.ok_or(Error::MissingBaselineOrigin)?;
        let budget = Budget::checked(input.limits)?;
        budget.root_funding()?;
        pins(input.plan, input.pins.context, input.pins.scope, view)?;
        if input.pins.definition != Digest32::from_bytes([0xc1; 32])
            || input.pins.oracle != Digest32::from_bytes([0xc2; 32])
        {
            return Err(Error::OracleMismatch);
        }
        if !nonzero(input.pins.hard) || !nonzero(input.pins.evidence) {
            return Err(Error::BindingMismatch);
        }
        let retained = claim(view, ROOT_RAM)?;
        baseline.check_current(view).map_err(Error::BuildCapture)?;
        input
            .candidate_expected
            .check_current(view)
            .map_err(Error::BuildCapture)?;
        if baseline.root() == input.candidate_expected.root() {
            return Err(Error::RoleOrOriginMismatch);
        }
        if baseline.program() != input.candidate_expected.program()
            || baseline.plan() != input.candidate_expected.plan()
        {
            return Err(Error::GoalOrProbeMismatch);
        }
        let identity = hash(&[
            b"GivenArtifactCheckedU32AddFour-v1",
            input.plan.digest().as_bytes(),
            input.pins.scope.comparison().as_bytes(),
            baseline.root().as_bytes(),
            baseline.snapshot_digest().as_bytes(),
            input.candidate_expected.root().as_bytes(),
            input.candidate_expected.snapshot_digest().as_bytes(),
            input.pins.hard.as_bytes(),
            input.pins.evidence.as_bytes(),
        ]);
        let root = Self {
            plan: input.plan,
            pins: input.pins,
            baseline_before: baseline,
            candidate_expected: input.candidate_expected,
            budget,
            identity,
            epochs: Cell::new([0; 2]),
            retained,
        };
        root.check(view)?;
        Ok(root)
    }
    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        check_claim(&self.retained, view)?;
        pins(self.plan, self.pins.context, self.pins.scope, view)?;
        self.baseline_before
            .check_current(view)
            .map_err(Error::BuildCapture)?;
        self.candidate_expected
            .check_current(view)
            .map_err(Error::BuildCapture)?;
        check_claim(&self.retained, view)
    }
}
impl<'origin, 'v> SourceCodingRoot<'origin, 'v> {
    pub fn given(
        input: SourceCodingRootInput<'origin>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        view.check()?;
        let baseline = input
            .baseline_expected
            .ok_or(Error::MissingVerifiedOrigin)?;
        let candidate = input
            .candidate_expected
            .ok_or(Error::MissingVerifiedOrigin)?;
        let budget = Budget::checked(input.limits)?;
        budget.root_funding()?;
        pins(input.plan, input.pins.context, input.pins.scope, view)?;
        if input.pins.definition != Digest32::from_bytes([0xe1; 32])
            || input.pins.oracle != Digest32::from_bytes([0xe2; 32])
        {
            return Err(Error::OracleMismatch);
        }
        let root = Self {
            plan: input.plan,
            pins: input.pins,
            expected: [baseline, candidate],
            budget,
            retained: claim(view, ROOT_RAM)?,
        };
        root.check(view)?;
        Ok(root)
    }
    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        check_claim(&self.retained, view)?;
        pins(self.plan, self.pins.context, self.pins.scope, view)?;
        if self.budget.limits.max_pairs != 1
            || !nonzero(self.pins.hard)
            || !nonzero(self.pins.evidence)
            || self
                .expected
                .iter()
                .any(|e| !nonzero(e.root()) || !nonzero(e.program()) || !nonzero(e.plan()))
        {
            return Err(Error::BindingMismatch);
        }
        check_claim(&self.retained, view)
    }
}
fn pins(
    plan: &CodingProbePlan,
    context: ContextRevision,
    scope: ContextScope,
    view: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    if context.source().digest() != view.source_revision()
        || plan.goal() != scope.goal()
        || plan.policy() != scope.semantics()
        || !nonzero(scope.comparison())
        || !nonzero(scope.probes())
    {
        return Err(Error::GoalOrProbeMismatch);
    }
    Ok(())
}
