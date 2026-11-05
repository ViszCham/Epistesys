//! Fixed-plan finite-formal loss bridge. K1 arithmetic has one canonical owner.
#![forbid(unsafe_code)]
pub mod owned;
mod plan;
pub mod produced;
use super::formal_boolean as f;
use lc631_analysis::creativity_measurement::{
    weighted_assessment, MeasurementError, Rational, RationalInterval, WeightedProbe,
};
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_solvers::typed_query::SmtStatusQuery;
use lc631_substrate::Digest32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanScope {
    pub comparison: Digest32,
    pub definition: Digest32,
    pub probe_set: Digest32,
    pub source: SourceRevision,
    pub context: ContextRevision,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkBudget {
    pub max_work: u64,
    pub max_payload_bytes: u64,
}
/// These are logical input/admission caps, not an OS/RSS reservation.
#[derive(Debug)]
pub struct FixedProbe<'a> {
    pub identity: &'a [u8],
    pub weight: &'a Rational,
    pub template: &'a f::Request<'a>,
}
#[derive(Debug)]
pub struct ProbePlan<'a> {
    scope: PlanScope,
    source: &'a SmtStatusQuery,
    probes: &'a [FixedProbe<'a>],
    payload_bytes: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BridgeError {
    EmptyPlan,
    ProbeLimit,
    DuplicateProbe,
    ForeignProbe,
    InvalidWeight,
    InvalidBudget,
    WorkBudget,
    PayloadBudget,
    AllocationFailed,
    ScopeMismatch,
    CandidateMismatch,
    LineageMismatch,
    DomainMismatch,
    NonNumericGap(Gap),
    Formal(f::Failure),
    Arithmetic(MeasurementError),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Soundness,
    Completeness,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gap {
    Missing,
    Unknown,
    NoComparable(f::NoComparable),
    Unsupported(f::Failure),
}
#[derive(Debug)]
pub enum ObservationValue<'a> {
    Computed(&'a f::ComputedReport<'a>),
    Gap(Gap),
}
#[derive(Debug)]
pub struct ProbeObservation<'a> {
    pub identity: &'a [u8],
    pub candidate: Digest32,
    pub scope: Digest32,
    pub value: ObservationValue<'a>,
}
/// Candidate identity/source attribution is Given, not independently authenticated.
#[derive(Debug)]
pub struct Candidate<'a> {
    id: Digest32,
    comparison: Digest32,
    target: &'a SmtStatusQuery,
    revision: SourceRevision,
    context: ContextRevision,
}
impl<'a> Candidate<'a> {
    pub fn given(
        id: Digest32,
        comparison: Digest32,
        target: &'a SmtStatusQuery,
        revision: SourceRevision,
        context: ContextRevision,
    ) -> Result<Self, BridgeError> {
        if context.source() != revision {
            return Err(BridgeError::LineageMismatch);
        }
        if !target.integer_symbols().is_empty() {
            return Err(BridgeError::Formal(f::Failure::UnsupportedInteger));
        }
        if target.boolean_symbols().len() > 16 {
            return Err(BridgeError::Formal(f::Failure::AtomLimit));
        }
        Ok(Self {
            id,
            comparison,
            target,
            revision,
            context,
        })
    }
    pub fn id(&self) -> Digest32 {
        self.id
    }
    pub(crate) fn comparison(&self) -> Digest32 {
        self.comparison
    }
    pub fn context(&self) -> ContextRevision {
        self.context
    }
    pub fn revision(&self) -> SourceRevision {
        self.revision
    }
    pub fn target_query(&self) -> &'a SmtStatusQuery {
        self.target
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeState {
    Preserved,
    Countermodel(u16),
    Gap(Gap),
}
#[derive(Debug)]
pub struct ProbeLineage<'a> {
    fixed: &'a FixedProbe<'a>,
    observation: Option<&'a ProbeObservation<'a>>,
    state: ProbeState,
}
impl<'a> ProbeLineage<'a> {
    pub fn fixed(&self) -> &'a FixedProbe<'a> {
        self.fixed
    }
    pub fn observation(&self) -> Option<&'a ProbeObservation<'a>> {
        self.observation
    }
    pub fn state(&self) -> ProbeState {
        self.state
    }
}
/// Private construction requires full fixed-plan K1 assessment.
#[derive(Debug)]
pub struct MeasuredFormalLoss<'a> {
    plan: &'a ProbePlan<'a>,
    candidate: &'a Candidate<'a>,
    direction: Direction,
    bounds: RationalInterval,
    probes: Vec<ProbeLineage<'a>>,
}
impl<'a> MeasuredFormalLoss<'a> {
    pub fn plan(&self) -> &'a ProbePlan<'a> {
        self.plan
    }
    pub fn candidate(&self) -> &'a Candidate<'a> {
        self.candidate
    }
    pub fn direction(&self) -> Direction {
        self.direction
    }
    pub fn bounds(&self) -> &RationalInterval {
        &self.bounds
    }
    pub fn probes(&self) -> &[ProbeLineage<'a>] {
        &self.probes
    }
    pub fn mapping_basis(&self) -> f::MappingBasis {
        f::MappingBasis::Given
    }
    pub fn source_interpretation(&self) -> f::SourceInterpretation {
        f::SourceInterpretation::Unverified
    }
    pub fn checker_independence(&self) -> f::CheckerIndependence {
        f::CheckerIndependence::UnassessedSharedIr
    }
}
impl<'a> ProbePlan<'a> {
    pub fn scope(&self) -> PlanScope {
        self.scope
    }
    pub fn payload_bytes(&self) -> u64 {
        self.payload_bytes
    }
    pub fn fixed_probes(&self) -> &'a [FixedProbe<'a>] {
        self.probes
    }

    pub fn measure(
        &'a self,
        candidate: &'a Candidate<'a>,
        direction: Direction,
        observations: &'a [ProbeObservation<'a>],
        budget: WorkBudget,
    ) -> Result<MeasuredFormalLoss<'a>, BridgeError> {
        let mut work = Tracker::new(budget)?;
        if candidate.comparison != self.scope.comparison {
            return Err(BridgeError::ScopeMismatch);
        }
        if observations.len() > 256 {
            return Err(BridgeError::ProbeLimit);
        }
        work.bytes(self.payload_bytes)?;
        plan::count_query(candidate.target, &mut work)?;
        for fixed in self.probes {
            work.charge((candidate.target.boolean_symbols().len() + 1) as u64)?;
            if candidate.target.boolean_symbols() != fixed.template.target.boolean_symbols() {
                return Err(BridgeError::DomainMismatch);
            }
        }
        self.check_observations(candidate, observations, &mut work)?;
        let mut values = reserved(self.probes.len())?;
        let mut lineage = reserved(self.probes.len())?;
        for fixed in self.probes {
            work.charge((observations.len() + fixed.template.symbols.len() + 1) as u64)?;
            let observation = observations.iter().find(|o| o.identity == fixed.identity);
            let state = self.state(candidate, fixed, observation, direction)?;
            values.push(
                match state {
                    ProbeState::Preserved => RationalInterval::exact_ratio(0, 1),
                    ProbeState::Countermodel(_) => RationalInterval::exact_ratio(1, 1),
                    ProbeState::Gap(Gap::Missing | Gap::Unknown) => {
                        RationalInterval::from_ratio(0, 1, 1, 1)
                    }
                    ProbeState::Gap(gap @ (Gap::NoComparable(_) | Gap::Unsupported(_))) => {
                        return Err(BridgeError::NonNumericGap(gap));
                    }
                }
                .map_err(|error| BridgeError::Arithmetic(error.into()))?,
            );
            lineage.push(ProbeLineage {
                fixed,
                observation,
                state,
            });
        }
        let mut weighted = reserved(self.probes.len())?;
        for (fixed, value) in self.probes.iter().zip(&values) {
            weighted.push(WeightedProbe {
                canonical_payload: fixed.identity,
                weight: fixed.weight,
                value,
            });
        }
        let bounds = weighted_assessment(&weighted).map_err(BridgeError::Arithmetic)?;
        Ok(MeasuredFormalLoss {
            plan: self,
            candidate,
            direction,
            bounds,
            probes: lineage,
        })
    }

    fn check_observations(
        &self,
        candidate: &Candidate<'_>,
        observations: &[ProbeObservation<'_>],
        work: &mut Tracker,
    ) -> Result<(), BridgeError> {
        for (index, observation) in observations.iter().enumerate() {
            if observation.identity.is_empty() || observation.identity.len() > 4096 {
                return Err(BridgeError::ForeignProbe);
            }
            work.bytes(observation.identity.len() as u64 + 72)?; // Two digests and identity length.
            work.charge((self.probes.len() + index + 1) as u64)?;
            if observation.candidate != candidate.id {
                return Err(BridgeError::CandidateMismatch);
            }
            if observation.scope != self.scope.comparison {
                return Err(BridgeError::ScopeMismatch);
            }
            if observations[..index]
                .iter()
                .any(|o| o.identity == observation.identity)
            {
                return Err(BridgeError::DuplicateProbe);
            }
            if !self
                .probes
                .iter()
                .any(|p| p.identity == observation.identity)
            {
                return Err(BridgeError::ForeignProbe);
            }
        }
        Ok(())
    }

    fn state(
        &self,
        candidate: &Candidate<'_>,
        fixed: &FixedProbe<'_>,
        observation: Option<&ProbeObservation<'_>>,
        direction: Direction,
    ) -> Result<ProbeState, BridgeError> {
        let report = match observation.map(|o| &o.value) {
            None => return Ok(ProbeState::Gap(Gap::Missing)),
            Some(ObservationValue::Gap(gap)) => return Ok(ProbeState::Gap(*gap)),
            Some(ObservationValue::Computed(report)) => report,
        };
        let request = report.request();
        let expected = fixed.template;
        if !std::ptr::eq(request.source, self.source)
            || !std::ptr::eq(request.target, candidate.target)
            || !std::ptr::eq(request.source_predicate, expected.source_predicate)
            || !std::ptr::eq(request.target_predicate, expected.target_predicate)
            || request.symbols != expected.symbols
            || request.unconstrained != expected.unconstrained
        {
            return Err(BridgeError::LineageMismatch);
        }
        let b = request.binding;
        if b.source != self.scope.source
            || b.source_context != self.scope.context
            || b.target != candidate.revision
            || b.target_context != candidate.context
            || b.probe_set != self.scope.probe_set
            || b.probe_map != expected.binding.probe_map
        {
            return Err(BridgeError::LineageMismatch);
        }
        Ok(
            match match direction {
                Direction::Soundness => report.soundness(),
                Direction::Completeness => report.completeness(),
            } {
                f::DirectionResult::Preserved => ProbeState::Preserved,
                f::DirectionResult::Countermodel(row) => ProbeState::Countermodel(row),
            },
        )
    }
}
fn reserved<T>(count: usize) -> Result<Vec<T>, BridgeError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| BridgeError::AllocationFailed)?;
    if values.capacity() > count {
        return Err(BridgeError::AllocationFailed);
    }
    Ok(values)
}
struct Tracker {
    budget: WorkBudget,
    work: u64,
    bytes: u64,
}
impl Tracker {
    fn new(budget: WorkBudget) -> Result<Self, BridgeError> {
        if budget.max_work == 0
            || budget.max_work > 1_000_000
            || budget.max_payload_bytes == 0
            || budget.max_payload_bytes > 1_048_576
        {
            return Err(BridgeError::InvalidBudget);
        }
        Ok(Self {
            budget,
            work: 0,
            bytes: 0,
        })
    }
    fn charge(&mut self, amount: u64) -> Result<(), BridgeError> {
        self.work = self
            .work
            .checked_add(amount)
            .ok_or(BridgeError::WorkBudget)?;
        if self.work > self.budget.max_work {
            return Err(BridgeError::WorkBudget);
        }
        Ok(())
    }
    fn bytes(&mut self, amount: u64) -> Result<(), BridgeError> {
        self.bytes = self
            .bytes
            .checked_add(amount)
            .ok_or(BridgeError::PayloadBudget)?;
        if self.bytes > self.budget.max_payload_bytes {
            return Err(BridgeError::PayloadBudget);
        }
        Ok(())
    }
}
