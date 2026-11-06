//! Four actual outputs and a wider-integer oracle, not renamed correctness counts.
use super::*;
use crate::goal_clock_domain as clock;
use lc631_analysis::creativity_measurement::{ComponentMetric, ComponentReport, RationalInterval};

pub struct OwnedArtifactPair<'r, 'o, 'origin, 'v> {
    root: &'r ArtifactPairRoot<'origin, 'v>,
    pair: CodingPair<'o, 'v>,
    baseline_success: [bool; 4],
    candidate_success: [bool; 4],
    baseline_goal: RationalInterval,
    candidate_goal: RationalInterval,
    coverage: ComponentReport,
    solve: ComponentReport,
    dependency: Digest32,
    retained: ResourceClaim<'v>,
}
pub struct ArtifactView<'current> {
    clock_basis: clock::ClockBasis,
    baseline_outputs: &'current [Option<u32>; 4],
    candidate_outputs: &'current [Option<u32>; 4],
    baseline_success: &'current [bool; 4],
    candidate_success: &'current [bool; 4],
    baseline_goal: &'current RationalInterval,
    candidate_goal: &'current RationalInterval,
    coverage: &'current ComponentReport,
    solve: &'current ComponentReport,
    dependency: Digest32,
}
fn successes(owner: &OwnedCodingProbeObservation<'_>) -> Result<[bool; 4], Error> {
    let inputs = [(0u32, 0u32), (1, 2), (u32::MAX, 1), (u32::MAX, 0)];
    let flags = std::array::from_fn(|i| {
        let (a, b) = inputs[i];
        let expected = u32::try_from(u64::from(a) + u64::from(b)).ok();
        owner.actual_outputs()[i] == expected
    });
    // A cross-check only. The primitive is the actual four outputs above.
    if flags.iter().filter(|s| **s).count() != usize::from(owner.correct_probes()) {
        return Err(Error::BindingMismatch);
    }
    Ok(flags)
}
/// Legacy Given-artifact diagnostic. Its clock inputs do not establish a registered domain.
pub fn measure_artifact_pair<'r, 'o, 'origin, 'v>(
    root: &'r ArtifactPairRoot<'origin, 'v>,
    pair: CodingPair<'o, 'v>,
    gate: &mut ArtifactCurrent<'_, '_, '_, '_, '_>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedArtifactPair<'r, 'o, 'origin, 'v>, Error> {
    let (retained, scratch) = admit_pair(root, view)?;
    current::check(root, pair, gate, view)?;
    let result = evaluate_pair(root, pair, retained, view)?;
    current::check(root, pair, gate, view)?;
    scratch.check()?;
    check_claim(&result.retained, view)?;
    Ok(result)
}

fn admit_pair<'v>(
    root: &ArtifactPairRoot<'_, 'v>,
    view: &'v ResourceReservationView<'_>,
) -> Result<(ResourceClaim<'v>, ResourceClaim<'v>), Error> {
    root.check(view)?;
    root.budget.pair_funding()?;
    // These claims precede all validation allocations, hashing and provider calls.
    let retained = claim(view, PAIR_RAM)?;
    let scratch = claim(view, VIEW_RAM)?;
    root.budget.pair()?;
    root.budget.read()?;
    Ok((retained, scratch))
}

/// Both admitted routes use the same four real outputs, oracle, identity and reports.
fn evaluate_pair<'r, 'o, 'origin, 'v>(
    root: &'r ArtifactPairRoot<'origin, 'v>, pair: CodingPair<'o, 'v>,
    retained: ResourceClaim<'v>, view: &ResourceReservationView<'_>,
) -> Result<OwnedArtifactPair<'r, 'o, 'origin, 'v>, Error> {
    let a = successes(pair.baseline)?;
    let b = successes(pair.candidate)?;
    let dependency = hash(&[
        b"conditional-artifact-goal-primitive.v1",
        root.identity.as_bytes(),
        pair.baseline.observation_digest().as_bytes(),
        pair.candidate.observation_digest().as_bytes(),
    ]);
    let input = |metric| numeric::ReportInput {
        metric,
        scope: root.pins.scope,
        context: root.pins.context,
        source: view.source_revision(),
        target: pair.candidate.target(),
        baseline: pair.baseline.target(),
        candidate: hash(&[
            root.identity.as_bytes(),
            pair.candidate.target().as_bytes(),
            pair.candidate.observation_digest().as_bytes(),
        ]),
        baseline_observation: pair.baseline.observation_digest(),
        candidate_observation: pair.candidate.observation_digest(),
        dependency,
        oracle: root.pins.oracle,
    };
    let coverage = numeric::gain(input(ComponentMetric::Coverage), &a, &b)?;
    let solve = numeric::gain(input(ComponentMetric::Solve), &a, &b)?;
    let baseline_goal = numeric::score(&a)?;
    let candidate_goal = numeric::score(&b)?;
    Ok(OwnedArtifactPair {
        root,
        pair,
        baseline_success: a,
        candidate_success: b,
        baseline_goal,
        candidate_goal,
        coverage,
        solve,
        dependency,
        retained,
    })
}
impl<'o, 'p, 'v> clock::RegisteredArtifactRoot<'o, 'p, 'v> {
    pub(crate) fn measure_registered<'r>(
        &'r self, pair: clock::RegisteredPair<'o, 'p, 'v>,
        ports: &mut clock::CurrentPorts<'_, '_, '_, '_, '_>, view: &'v ResourceReservationView<'_>,
    ) -> Result<OwnedArtifactPair<'r, 'o, 'o, 'v>, clock::Error> {
        let (retained, scratch) = admit_pair(&self.artifact, view)?;
        current::check_registered(self, pair, ports, view)?;
        let result = evaluate_pair(&self.artifact, pair.actual(), retained, view)?;
        let after = current::check_registered(self, pair, ports, view)?;
        scratch.check()?;
        check_claim(&result.retained, view)?;
        self.artifact.epochs.set(after);
        Ok(result)
    }
}

impl<'r, 'o, 'origin, 'v> OwnedArtifactPair<'r, 'o, 'origin, 'v> {
    /// Identity-only cross-row precondition, NOT a fresh-capture eligibility
    /// observation. Every adopted row still requires its own with_current gate.
    /// Equal root pins/reports/digests cannot substitute another actual baseline.
    pub fn require_same_baseline(
        &self,
        other: &OwnedArtifactPair<'_, '_, '_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.root.check(view)?;
        other.root.check(view)?;
        check_claim(&self.retained, view)?;
        check_claim(&other.retained, view)?;
        if !same_owner(self.root, other.root)
            || !same_owner(self.pair.baseline, other.pair.baseline)
            || self.root.identity != other.root.identity
            || self.pair.baseline.observation_digest() != other.pair.baseline.observation_digest()
            || self.coverage.binding.baseline != other.coverage.binding.baseline
        {
            return Err(Error::BaselineMismatch);
        }
        Ok(())
    }
    pub fn with_current<R>(
        &self,
        root: &ArtifactPairRoot<'origin, 'v>,
        pair: CodingPair<'o, 'v>,
        gate: &mut ArtifactCurrent<'_, '_, '_, '_, '_>,
        view: &ResourceReservationView<'_>,
        consume: impl for<'current> FnOnce(ArtifactView<'current>) -> R,
    ) -> Result<R, Error> {
        let scratch = self.admit_read(root, pair, view)?;
        current::check(root, pair, gate, view)?;
        let result = consume(self.projection(clock::ClockBasis::LegacyClockProvenanceUnassessed));
        current::check(root, pair, gate, view)?;
        scratch.check()?;
        check_claim(&self.retained, view)?;
        Ok(result)
    }

    fn admit_read<'s>(
        &self, root: &ArtifactPairRoot<'origin, 'v>, pair: CodingPair<'o, 'v>,
        view: &'s ResourceReservationView<'_>,
    ) -> Result<ResourceClaim<'s>, Error> {
        root.check(view)?;
        if !std::ptr::eq(root, self.root) {
            return Err(Error::BindingMismatch);
        }
        if !std::ptr::eq(pair.baseline, self.pair.baseline)
            || !std::ptr::eq(pair.candidate, self.pair.candidate)
        {
            return Err(Error::RoleOrOriginMismatch);
        }
        check_claim(&self.retained, view)?;
        let scratch = claim(view, VIEW_RAM)?;
        root.budget.read()?;
        Ok(scratch)
    }

    fn projection(&self, clock_basis: clock::ClockBasis) -> ArtifactView<'_> {
        ArtifactView {
            clock_basis,
            baseline_outputs: self.pair.baseline.actual_outputs(),
            candidate_outputs: self.pair.candidate.actual_outputs(),
            baseline_success: &self.baseline_success,
            candidate_success: &self.candidate_success,
            baseline_goal: &self.baseline_goal,
            candidate_goal: &self.candidate_goal,
            coverage: &self.coverage,
            solve: &self.solve,
            dependency: self.dependency,
        }
    }
}

impl<'r, 'o, 'v> OwnedArtifactPair<'r, 'o, 'o, 'v> {
    pub(crate) fn with_registered<'p, R>(
        &self, root: &'r clock::RegisteredArtifactRoot<'o, 'p, 'v>,
        pair: clock::RegisteredPair<'o, 'p, 'v>,
        ports: &mut clock::CurrentPorts<'_, '_, '_, '_, '_>, view: &'v ResourceReservationView<'_>,
        consume: impl for<'current> FnOnce(ArtifactView<'current>) -> R,
    ) -> Result<R, clock::Error> {
        let scratch = self.admit_read(&root.artifact, pair.actual(), view)?;
        current::check_registered(root, pair, ports, view)?;
        let result = consume(self.projection(clock::ClockBasis::GivenLocalRegistered));
        let after = current::check_registered(root, pair, ports, view)?;
        scratch.check()?;
        check_claim(&self.retained, view)?;
        root.artifact.epochs.set(after);
        Ok(result)
    }
}

impl<'current> ArtifactView<'current> {
    pub fn clock_basis(&self) -> clock::ClockBasis { self.clock_basis }
    pub fn baseline_outputs(&self) -> &'current [Option<u32>; 4] {
        self.baseline_outputs
    }
    pub fn candidate_outputs(&self) -> &'current [Option<u32>; 4] {
        self.candidate_outputs
    }
    pub fn baseline_successes(&self) -> &'current [bool; 4] {
        self.baseline_success
    }
    pub fn candidate_successes(&self) -> &'current [bool; 4] {
        self.candidate_success
    }
    pub fn baseline_artifact_goal(&self) -> &'current RationalInterval {
        self.baseline_goal
    }
    pub fn candidate_artifact_goal(&self) -> &'current RationalInterval {
        self.candidate_goal
    }
    pub fn coverage(&self) -> &'current ComponentReport {
        self.coverage
    }
    pub fn solve(&self) -> &'current ComponentReport {
        self.solve
    }
    pub fn goal_dependency(&self) -> Digest32 {
        self.dependency
    }
    pub fn requested_source_goal(&self) -> Option<&'current RationalInterval> {
        None
    }
    pub fn source_correspondence(&self) -> SourceCorrespondence {
        SourceCorrespondence::UnverifiedNeedsEvidence
    }
    pub fn source_eligibility(&self) -> SourceEligibility {
        SourceEligibility::Pending
    }
    pub fn source_recommendation(&self) -> Option<usize> {
        None
    }
    pub fn require_overlap(&self, declared: DeclaredOverlap) -> Result<(), Error> {
        overlap(declared)
    }
}
