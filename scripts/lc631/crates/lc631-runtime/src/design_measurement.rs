//! 日本語: 固定Given scheduling MODELでpriority PROGRAMを実行するCR-DESIGN。
//! English: CR-DESIGN from actual priority PROGRAM execution on a fixed Given MODEL.
//! Five fields: exact paired normalized score gain[-1,1], dimensionless; event-jump
//! program execution and separate full tick replay; fixed positive-weight design
//! validation suite/maps/units/budgets; handgold and policy/trace/range mutants;
//! private current DesignGain feeds actual Pareto. No physical latency, design
//! truth, external oracle independence, hard-claim closure or Authority is claimed.
#![forbid(unsafe_code)]
mod domain;
mod measurement;
mod oracle;
mod scoring;
mod selection;
use crate::joint_selection::*;
use lc631_analysis::creativity_measurement::{
    paired_gain, ComponentMetric, ComponentReport, ComponentState, IndependenceStatus, InputBasis,
    MeasurementBinding, MeasurementError, MeasurementGap, ProbePair, SemanticValidation,
};
use lc631_core::{
    context::ContextRevision,
    measurement_numeric::{self as numeric, NumericError, Rational, RationalInterval},
    CandidateId,
};
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    Digest32,
};
pub use measurement::{execute, CandidateState, OwnedDesignGain, PendingReason};
pub use selection::{select_current, DesignRow, OwnedDesignSelection};
use sha2::{Digest, Sha256};
pub const MAX_WORKLOADS: usize = 4;
pub const MAX_JOBS: usize = 8;
pub const MAX_WORKERS: usize = 4;
pub const MAX_TICKS: u16 = 256;
const SUITE_RAM: u64 = 16 * 1024 * 1024;
const CANDIDATE_RAM: u64 = 8 * 1024 * 1024;
const RESULT_RAM: u64 = 8 * 1024 * 1024;
const SCRATCH: u64 = 32 * 1024 * 1024;
const PRODUCER: &[u8] =
    b"finite-formal-v1.4/CR-DESIGN/NonPreemptiveReadyListV1/event-tick-paired.v1";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobId(pub u16);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelTicks(pub u16);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TickUnit(pub Digest32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Minimize,
    Maximize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesignRole {
    DesignValidation,
    Training,
    FinalResearchHoldout,
    HardUniversal,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Saturation {
    None,
    SaturatedLow,
    SaturatedHigh,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActualMakespan {
    pub ticks: ModelTicks,
    pub unit: TickUnit,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoreObservation {
    pub raw: ActualMakespan,
    pub score: RationalInterval,
    pub saturation: Saturation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobTiming {
    pub job: JobId,
    pub worker: u8,
    pub start: ModelTicks,
    pub end: ModelTicks,
}
/// Historical export data only. A fabricated copy never constructs an owned gain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportedSchedule {
    pub jobs: Vec<JobTiming>,
    pub makespan: ModelTicks,
}
#[derive(Clone, Copy)]
pub struct Job<'a> {
    pub id: JobId,
    pub canonical_payload: &'a [u8],
    pub duration_ticks: u16,
}
#[derive(Clone, Copy)]
pub struct RawRange<'a> {
    pub unit: TickUnit,
    pub lower: &'a Rational,
    pub upper: &'a Rational,
}
#[derive(Clone, Copy)]
pub struct NormalizationMap<'a> {
    pub unit: TickUnit,
    pub lower: &'a Rational,
    pub upper: &'a Rational,
    pub revision: Digest32,
    pub direction: Direction,
}
#[derive(Clone, Copy)]
pub struct Workload<'a> {
    pub case: &'a [u8],
    pub jobs: &'a [Job<'a>],
    pub dependencies: &'a [(JobId, JobId)],
    pub workers: u8,
    pub unit: TickUnit,
    pub baseline_priority: &'a [JobId],
    pub accepted: RawRange<'a>,
    pub normalization: NormalizationMap<'a>,
    pub weight: &'a Rational,
}
#[derive(Clone, Copy)]
pub struct PriorityProgram<'a> {
    pub case: &'a [u8],
    pub order: &'a [JobId],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Events count time-boundary BATCHES, including initial and final completion.
/// Replay ticks count service intervals; final completion at the limit is allowed.
pub struct DesignLimits {
    pub max_events_per_run: u16,
    pub max_replay_ticks_per_run: u16,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesignBinding {
    pub selection: SelectionPins,
    pub context: ContextRevision,
    pub suite: Digest32,
    pub oracle: Digest32,
    pub definition: Digest32,
    pub probe_set: Digest32,
    pub dependency_group: Digest32,
    pub role: DesignRole,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateBinding {
    pub id: CandidateId,
    pub candidate_digest: Digest32,
    pub context: ContextRevision,
    pub target: Digest32,
    pub transform: Digest32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Resource(ResourceError),
    Numeric(NumericError),
    Measurement(MeasurementError),
    Selection(SelectionError),
    EmptySuite,
    InvalidWorkload,
    InvalidProgram,
    InvalidMap,
    UnitMismatch,
    UnsupportedDirection,
    UnsupportedRole,
    InvalidLimits,
    InvalidWeight,
    DuplicateCase,
    DuplicateJob,
    InvalidDependency,
    Cycle,
    OutsideAcceptedRange,
    PrerequisitePending,
    ReplayMismatch,
    AllocationFailed,
    BindingMismatch,
    DefinitionMismatch,
    ForeignReservation,
    InvalidRows,
}
pub fn definition_revision() -> Digest32 {
    digest(PRODUCER)
}
fn digest(bytes: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(bytes).into())
}
fn claim<'v>(
    v: &'v ResourceReservationView<'_>,
    ram: u64,
    out: u64,
) -> Result<ResourceClaim<'v>, Error> {
    let r = ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(ram).map_err(Error::Resource)?,
    );
    if out == 0 {
        v.claim_resources(&[r]).map_err(Error::Resource)
    } else {
        let o = ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(out).map_err(Error::Resource)?,
        );
        v.claim_resources(&[r, o]).map_err(Error::Resource)
    }
}
fn source(b: DesignBinding, v: &ResourceReservationView<'_>) -> Result<(), Error> {
    v.check().map_err(Error::Resource)?;
    if b.selection.source.digest() != v.source_revision() {
        return Err(Error::Resource(ResourceError::SourceMismatch));
    }
    if b.context.source() != b.selection.source || b.probe_set != b.selection.scope.probes() {
        return Err(Error::BindingMismatch);
    }
    if b.definition != definition_revision() {
        return Err(Error::DefinitionMismatch);
    }
    Ok(())
}
struct OwnedJob {
    id: JobId,
    payload: Vec<u8>,
    duration: u16,
    dependencies: u8,
}
struct OwnedWorkload {
    key: Vec<u8>,
    jobs: Vec<OwnedJob>,
    workers: u8,
    unit: TickUnit,
    baseline: Vec<usize>,
    accepted_low: Rational,
    accepted_high: Rational,
    m: Rational,
    upper: Rational,
    map: Digest32,
    weight: Rational,
}
struct Outcome {
    schedule: ReportedSchedule,
    observation: ScoreObservation,
    node: Digest32,
}
/// Actual baseline traces/scores remain owned under the LAST retained claim.
/// Scheduling inputs and durations are Given, not measured physical execution.
pub struct OwnedDesignSuite<'v> {
    binding: DesignBinding,
    workloads: Vec<OwnedWorkload>,
    baselines: Vec<Outcome>,
    limits: DesignLimits,
    digest: Digest32,
    input_profile: Digest32,
    retained: ResourceClaim<'v>,
}
impl OwnedDesignSuite<'_> {
    pub fn baseline_observation(&self, i: usize) -> Option<&ScoreObservation> {
        self.baselines.get(i).map(|o| &o.observation)
    }
    pub fn baseline_schedule(&self, i: usize) -> Option<&ReportedSchedule> {
        self.baselines.get(i).map(|o| &o.schedule)
    }
    /// Historical node identity metadata, never a certificate.
    pub fn baseline_score_node(&self, i: usize) -> Option<Digest32> {
        self.baselines.get(i).map(|o| o.node)
    }
    pub fn historical_digest(&self) -> Digest32 {
        self.digest
    }
    fn check(&self, v: &ResourceReservationView<'_>) -> Result<(), Error> {
        source(self.binding, v)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(v) {
            return Err(Error::ForeignReservation);
        }
        Ok(())
    }
}
pub fn register<'v>(
    binding: DesignBinding,
    inputs: &[Workload<'_>],
    limits: DesignLimits,
    v: &'v ResourceReservationView<'_>,
) -> Result<OwnedDesignSuite<'v>, Error> {
    source(binding, v)?;
    if binding.role != DesignRole::DesignValidation {
        return Err(Error::UnsupportedRole);
    }
    if inputs.is_empty() {
        return Err(Error::EmptySuite);
    }
    if inputs.len() > MAX_WORKLOADS
        || limits.max_events_per_run > 32
        || limits.max_replay_ticks_per_run > MAX_TICKS
    {
        return Err(Error::InvalidLimits);
    }
    // FULL baseline traces are charged before admission copies AND either oracle run.
    let scratch = claim(v, SCRATCH, 0)?;
    let retained = claim(v, SUITE_RAM, 4 * 1024 * 1024)?;
    domain::preflight(inputs)?;
    let mut workloads = Vec::new();
    workloads
        .try_reserve_exact(inputs.len())
        .map_err(|_| Error::AllocationFailed)?;
    for w in inputs {
        workloads.push(domain::own(w)?);
    }
    workloads.sort_by(|a, b| a.key.cmp(&b.key));
    let input_profile = scoring::input_profile(binding, limits, &workloads);
    let mut baselines = Vec::new();
    baselines
        .try_reserve_exact(workloads.len())
        .map_err(|_| Error::AllocationFailed)?;
    for w in &workloads {
        v.check().map_err(Error::Resource)?;
        let schedule =
            oracle::event_run(w, &w.baseline, limits)?.ok_or(Error::PrerequisitePending)?;
        if !oracle::tick_replay(w, &w.baseline, &schedule, limits)? {
            return Err(Error::PrerequisitePending);
        }
        let observation = scoring::normalize(w, schedule.makespan)?;
        let node = scoring::node(
            w,
            &w.baseline,
            &schedule,
            &observation,
            scoring::ScorePhase::Baseline(input_profile),
        );
        baselines.push(Outcome {
            schedule,
            observation,
            node,
        });
    }
    let mut h = Sha256::new();
    h.update(PRODUCER);
    for d in [
        binding.selection.source.digest(),
        binding.context.interpretation_digest(),
        binding.selection.scope.comparison(),
        binding.selection.scope.goal(),
        binding.selection.scope.probes(),
        binding.selection.scope.semantics(),
        binding.selection.baseline,
        binding.selection.policy,
        binding.selection.hard_invariants,
        binding.selection.evidence_revision,
        binding.suite,
        binding.oracle,
        binding.definition,
        binding.probe_set,
        binding.dependency_group,
    ] {
        h.update(d.as_bytes());
    }
    match binding.selection.goal_profile {
        Some(d) => {
            h.update([1]);
            h.update(d.as_bytes());
        }
        None => h.update([0]),
    };
    h.update(b"DesignValidation/Minimize/all-completions-before-dispatch/fill-all-idle/skip-blocked/lowest-free-worker/half-open");
    h.update(limits.max_events_per_run.to_le_bytes());
    h.update(limits.max_replay_ticks_per_run.to_le_bytes());
    for n in [
        MAX_WORKLOADS as u64,
        MAX_JOBS as u64,
        MAX_WORKERS as u64,
        MAX_TICKS as u64,
        numeric::MAX_RATIONAL_BITS,
        SUITE_RAM,
        CANDIDATE_RAM,
        RESULT_RAM,
        SCRATCH,
    ] {
        h.update(n.to_le_bytes());
    }
    for (w, b) in workloads.iter().zip(&baselines) {
        scoring::hash_workload(&mut h, w);
        h.update(b.node.as_bytes());
    }
    let out = OwnedDesignSuite {
        binding,
        workloads,
        baselines,
        limits,
        digest: Digest32::from_bytes(h.finalize().into()),
        input_profile,
        retained,
    };
    scratch.check().map_err(Error::Resource)?;
    out.check(v)?;
    Ok(out)
}
pub struct FrozenDesign<'a, 'v> {
    suite: &'a OwnedDesignSuite<'v>,
    binding: CandidateBinding,
    programs: Vec<Vec<usize>>,
    digest: Digest32,
    retained: ResourceClaim<'v>,
}
impl FrozenDesign<'_, '_> {
    pub fn historical_digest(&self) -> Digest32 {
        self.digest
    }
    fn check(&self, v: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.suite.check(v)?;
        if self.binding.context != self.suite.binding.context {
            return Err(Error::BindingMismatch);
        }
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(v) {
            return Err(Error::ForeignReservation);
        }
        Ok(())
    }
}
pub fn bind_candidate<'a, 'v>(
    suite: &'a OwnedDesignSuite<'v>,
    binding: CandidateBinding,
    inputs: &[PriorityProgram<'_>],
    v: &'v ResourceReservationView<'_>,
) -> Result<FrozenDesign<'a, 'v>, Error> {
    suite.check(v)?;
    if binding.context != suite.binding.context || inputs.len() != suite.workloads.len() {
        return Err(Error::BindingMismatch);
    }
    let scratch = claim(v, SCRATCH, 0)?;
    let retained = claim(v, CANDIDATE_RAM, 2 * 1024 * 1024)?;
    for p in inputs {
        if p.case.is_empty() || p.case.len() > 4096 || p.order.len() > MAX_JOBS {
            return Err(Error::InvalidProgram);
        }
    }
    for (i, p) in inputs.iter().enumerate() {
        if inputs[..i].iter().any(|q| p.case == q.case) {
            return Err(Error::DuplicateCase);
        }
    }
    let mut programs = Vec::new();
    programs
        .try_reserve_exact(inputs.len())
        .map_err(|_| Error::AllocationFailed)?;
    let mut h = Sha256::new();
    h.update(PRODUCER);
    h.update(suite.digest.as_bytes());
    for d in [
        binding.candidate_digest,
        binding.context.interpretation_digest(),
        binding.target,
        binding.transform,
    ] {
        h.update(d.as_bytes());
    }
    for w in &suite.workloads {
        let p = inputs
            .iter()
            .find(|p| p.case == w.key)
            .ok_or(Error::InvalidProgram)?;
        let order = domain::program(w, p.order)?;
        for i in &order {
            h.update((*i as u16).to_le_bytes());
        }
        programs.push(order);
    }
    let out = FrozenDesign {
        suite,
        binding,
        programs,
        digest: Digest32::from_bytes(h.finalize().into()),
        retained,
    };
    scratch.check().map_err(Error::Resource)?;
    out.check(v)?;
    Ok(out)
}
/// Validates historical DATA only, never constructs an owned measurement.
pub fn replay_reported(
    suite: &OwnedDesignSuite<'_>,
    case: &[u8],
    order: &[JobId],
    reported: &ReportedSchedule,
    v: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    suite.check(v)?;
    if case.len() > 4096 || order.len() > MAX_JOBS || reported.jobs.len() > MAX_JOBS {
        return Err(Error::InvalidProgram);
    }
    let scratch = claim(v, 4 * 1024 * 1024, 0)?;
    let w = suite
        .workloads
        .iter()
        .find(|w| w.key == case)
        .ok_or(Error::InvalidProgram)?;
    let p = domain::program(w, order)?;
    if !oracle::tick_replay(w, &p, reported, suite.limits)? {
        return Err(Error::PrerequisitePending);
    }
    scratch.check().map_err(Error::Resource)?;
    Ok(())
}
fn field(h: &mut Sha256, b: &[u8]) {
    h.update((b.len() as u64).to_le_bytes());
    h.update(b);
}
fn hash_rational(h: &mut Sha256, r: &Rational) {
    field(h, &r.numer().to_signed_bytes_le());
    field(h, &r.denom().to_signed_bytes_le());
}
