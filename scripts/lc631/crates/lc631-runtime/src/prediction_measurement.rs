//! 日本語: Given予測を局所凍結後、実有限MODEL評価でpaired Brierを計算する。
//! English: Locally frozen Given forecasts followed by actual finite-MODEL paired Brier.
//! Local phase order is not an authenticated timestamp, blindness or honesty proof.
//! Five fields: Δ=B0−Bz in[-1,1], dimensionless; fixed positive-weight paired binary
//! algorithm; same source/model/split/query mapping applicability, Validation only
//! selection; independent rational handgold/cuts; positive Δ favors PredictionGain.
//! Given input authenticity/timing remain unassessed: no full CR-PREDICT readiness.
#![forbid(unsafe_code)]
mod evaluation;
mod selection;
use crate::joint_selection::*;
pub use evaluation::{evaluate, LabelGap, LabelState, OwnedPredictionGain};
use lc631_analysis::creativity_measurement::{
    ComponentMetric, ComponentReport, ComponentState, IndependenceStatus, InputBasis,
    MeasurementBinding, MeasurementGap, SemanticValidation,
};
use lc631_core::{
    context::ContextRevision,
    measurement_numeric::{self as numeric, NumericError, Rational, RationalInterval},
    CandidateId,
};
use lc631_solvers::temporal::{FiniteTrace, TemporalError, TemporalQuery};
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    Digest32,
};
pub use selection::{select_current, OwnedPredictionSelection, PredictionRow};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
pub const MAX_SAMPLES: usize = 8;
pub const MAX_SAMPLE_PAYLOAD: usize = 65536;
pub const FROZEN_RAM: u64 = 32 * 1024 * 1024;
pub const SCRATCH_RAM: u64 = 32 * 1024 * 1024;
const PRODUCER: &[u8] = b"finite-formal-v1.4/CR-PREDICT/local-freeze-owned-finite-label.v1";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SplitRole {
    Training,
    Validation,
    FinalResearchHoldout,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ForecastLimits {
    pub max_node_steps: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ForecastBinding {
    pub selection: SelectionPins,
    pub candidate: CandidateId,
    pub candidate_digest: Digest32,
    pub context: ContextRevision,
    pub target: Digest32,
    pub transform: Digest32,
    pub definition: Digest32,
    pub probe_set: Digest32,
    pub oracle: Digest32,
    pub dependency_group: Digest32,
    pub baseline_model: Digest32,
    pub candidate_model: Digest32,
    pub split_digest: Digest32,
    pub role: SplitRole,
}
#[derive(Clone, Copy)]
pub struct Forecast<'a> {
    pub canonical_sample: &'a [u8],
    pub baseline: &'a Rational,
    pub candidate: &'a Rational,
    pub weight: &'a Rational,
    pub query: &'a TemporalQuery,
    pub trace: &'a FiniteTrace,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Resource(ResourceError),
    Numeric(NumericError),
    Store(crate::finite_results::FiniteResultError),
    LabelEvaluation(TemporalError),
    Selection(SelectionError),
    InvalidProbability,
    InvalidWeight,
    InvalidSamples,
    DuplicateSample,
    AllocationFailed,
    BindingMismatch,
    DefinitionMismatch,
    ForeignReservation,
    InvalidRows,
    SplitNotSelectable,
}
pub fn definition_revision() -> Digest32 {
    digest(PRODUCER)
}
pub(super) fn digest(b: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(b).into())
}
pub(super) fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    ram: u64,
    output: u64,
) -> Result<ResourceClaim<'v>, Error> {
    let ram = ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(ram).map_err(Error::Resource)?,
    );
    if output == 0 {
        view.claim_resources(&[ram]).map_err(Error::Resource)
    } else {
        let output = ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(output).map_err(Error::Resource)?,
        );
        view.claim_resources(&[ram, output])
            .map_err(Error::Resource)
    }
}
fn source(binding: ForecastBinding, view: &ResourceReservationView<'_>) -> Result<(), Error> {
    view.check().map_err(Error::Resource)?;
    if binding.selection.source.digest() != view.source_revision() {
        return Err(Error::Resource(ResourceError::SourceMismatch));
    }
    if binding.context.source() != binding.selection.source
        || binding.probe_set != binding.selection.scope.probes()
    {
        return Err(Error::BindingMismatch);
    }
    if binding.definition != definition_revision() {
        return Err(Error::DefinitionMismatch);
    }
    Ok(())
}
struct Sample {
    key: Vec<u8>,
    baseline: Rational,
    candidate: Rational,
    weight: Rational,
    query: TemporalQuery,
    trace: FiniteTrace,
}
/// Private owned values are immutable and precede this module's label API.
/// Trace Arc clones own immutable backing; the conservative claim includes its
/// bounded payload even though cloning shares existing allocation. Logical claims
/// exclude caller inputs, allocator overhead and RSS.
pub struct FrozenPredictions<'v> {
    binding: ForecastBinding,
    samples: Vec<Sample>,
    weight: Rational,
    digest: Digest32,
    limits: ForecastLimits,
    common: Digest32,
    retained: ResourceClaim<'v>,
}
impl FrozenPredictions<'_> {
    pub fn fixed_weight(&self) -> &Rational {
        &self.weight
    }
    pub fn historical_binding(&self) -> ForecastBinding {
        self.binding
    }
    pub fn historical_digest(&self) -> Digest32 {
        self.digest
    }
    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        source(self.binding, view)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(view) {
            return Err(Error::ForeignReservation);
        }
        Ok(())
    }
}
/// This API accepts neither a label, report, callback, preexisting result ID nor store.
/// Frozen probability values can no longer follow mutations to caller rationals.
/// Canonical sample payloads and query/trace bindings are Given mappings.
pub fn freeze<'v>(
    binding: ForecastBinding,
    inputs: &[Forecast<'_>],
    limits: ForecastLimits,
    view: &'v ResourceReservationView<'_>,
) -> Result<FrozenPredictions<'v>, Error> {
    source(binding, view)?;
    lc631_solvers::temporal::EvaluationBudget::checked(limits.max_node_steps)
        .map_err(Error::LabelEvaluation)?;
    if inputs.is_empty() || inputs.len() > MAX_SAMPLES {
        return Err(Error::InvalidSamples);
    }
    let scratch = claim(view, SCRATCH_RAM, 0)?;
    let retained = claim(view, FROZEN_RAM, 4 * 1024 * 1024)?;
    let zero = Rational::from_integer(0.into());
    let one = Rational::from_integer(1.into());
    let mut payload = 0usize;
    // Validate ALL raw rationals before any numeric clone, comparison or arithmetic.
    for i in inputs {
        for r in [i.baseline, i.candidate, i.weight] {
            numeric::check(r).map_err(Error::Numeric)?;
        }
        payload = payload
            .checked_add(i.canonical_sample.len())
            .ok_or(Error::InvalidSamples)?;
        if i.canonical_sample.is_empty() || payload > MAX_SAMPLE_PAYLOAD {
            return Err(Error::InvalidSamples);
        }
        if i.query.context() != binding.context {
            return Err(Error::BindingMismatch);
        }
    }
    for (n, i) in inputs.iter().enumerate() {
        for p in [i.baseline, i.candidate] {
            if numeric::checked_compare(p, &zero).map_err(Error::Numeric)? == Ordering::Less
                || numeric::checked_compare(p, &one).map_err(Error::Numeric)? == Ordering::Greater
            {
                return Err(Error::InvalidProbability);
            }
        }
        if numeric::checked_compare(i.weight, &zero).map_err(Error::Numeric)? != Ordering::Greater {
            return Err(Error::InvalidWeight);
        }
        if inputs[..n].iter().any(|j| {
            j.canonical_sample == i.canonical_sample
                || (j.query.digest() == i.query.digest() && j.trace.digest() == i.trace.digest())
        }) {
            return Err(Error::DuplicateSample);
        }
    }
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(inputs.len())
        .map_err(|_| Error::AllocationFailed)?;
    for i in inputs {
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(i.query.nodes().len())
            .map_err(|_| Error::AllocationFailed)?;
        nodes.extend_from_slice(i.query.nodes());
        let query =
            TemporalQuery::checked(binding.context, nodes, i.query.root(), i.query.profile())
                .map_err(Error::LabelEvaluation)?;
        if query.digest() != i.query.digest() {
            return Err(Error::BindingMismatch);
        }
        let mut key = Vec::new();
        key.try_reserve_exact(i.canonical_sample.len())
            .map_err(|_| Error::AllocationFailed)?;
        key.extend_from_slice(i.canonical_sample);
        samples.push(Sample {
            key,
            baseline: i.baseline.clone(),
            candidate: i.candidate.clone(),
            weight: i.weight.clone(),
            query,
            trace: i.trace.clone(),
        });
    }
    samples.sort_by(|a, b| a.key.cmp(&b.key));
    let mut weight = zero;
    for s in &samples {
        weight = numeric::checked_add(&weight, &s.weight).map_err(Error::Numeric)?;
    }
    let mut h = Sha256::new();
    h.update(PRODUCER);
    hash_binding(&mut h, binding);
    h.update((limits.max_node_steps as u64).to_le_bytes());
    for s in &samples {
        field(&mut h, &s.key);
        for r in [&s.baseline, &s.candidate, &s.weight] {
            hash_rational(&mut h, r);
        }
        h.update(s.query.digest().as_bytes());
        h.update(s.trace.digest().as_bytes());
        h.update(s.query.profile().digest().as_bytes());
    }
    hash_rational(&mut h, &weight);
    let common = common_profile(binding, limits, &samples);
    let frozen = FrozenPredictions {
        binding,
        samples,
        weight,
        digest: Digest32::from_bytes(h.finalize().into()),
        limits,
        common,
        retained,
    };
    scratch.check().map_err(Error::Resource)?;
    frozen.check(view)?;
    Ok(frozen)
}
pub(super) fn field(h: &mut Sha256, b: &[u8]) {
    h.update((b.len() as u64).to_le_bytes());
    h.update(b);
}
pub(super) fn hash_rational(h: &mut Sha256, r: &Rational) {
    field(h, &r.numer().to_signed_bytes_le());
    field(h, &r.denom().to_signed_bytes_le());
}
fn hash_binding(h: &mut Sha256, b: ForecastBinding) {
    for d in [
        b.selection.source.digest(),
        b.selection.scope.comparison(),
        b.selection.scope.goal(),
        b.selection.scope.probes(),
        b.selection.scope.semantics(),
        b.selection.baseline,
        b.selection.hard_invariants,
        b.selection.policy,
        b.selection.evidence_revision,
        b.candidate_digest,
        b.context.interpretation_digest(),
        b.target,
        b.transform,
        b.definition,
        b.probe_set,
        b.oracle,
        b.dependency_group,
        b.baseline_model,
        b.candidate_model,
        b.split_digest,
    ] {
        h.update(d.as_bytes());
    }
    match b.selection.goal_profile {
        Some(d) => {
            h.update([1]);
            h.update(d.as_bytes());
        }
        None => h.update([0]),
    };
    h.update([match b.role {
        SplitRole::Training => 0,
        SplitRole::Validation => 1,
        SplitRole::FinalResearchHoldout => 2,
    }]);
}

// Common comparison identity is derived from ACTUAL owned paired data, not supplied
// split/model digest equality. Candidate probabilities/models/mapping slots are excluded.
fn common_profile(b: ForecastBinding, limits: ForecastLimits, samples: &[Sample]) -> Digest32 {
    let mut h = Sha256::new();
    h.update(b"CR-PREDICT.actual-common-paired-profile.v1");
    for d in [
        b.selection.source.digest(),
        b.context.interpretation_digest(),
        b.selection.scope.comparison(),
        b.selection.scope.goal(),
        b.selection.scope.probes(),
        b.selection.scope.semantics(),
        b.selection.baseline,
        b.selection.hard_invariants,
        b.selection.policy,
        b.selection.evidence_revision,
        b.baseline_model,
        b.split_digest,
        b.definition,
        b.probe_set,
        b.oracle,
    ] {
        h.update(d.as_bytes());
    }
    match b.selection.goal_profile {
        Some(d) => {
            h.update([1]);
            h.update(d.as_bytes());
        }
        None => h.update([0]),
    };
    h.update([match b.role {
        SplitRole::Training => 0,
        SplitRole::Validation => 1,
        SplitRole::FinalResearchHoldout => 2,
    }]);
    h.update((limits.max_node_steps as u64).to_le_bytes());
    for s in samples {
        field(&mut h, &s.key);
        hash_rational(&mut h, &s.baseline);
        hash_rational(&mut h, &s.weight);
        h.update(s.query.digest().as_bytes());
        h.update(s.trace.digest().as_bytes());
        h.update(s.query.profile().digest().as_bytes());
    }
    Digest32::from_bytes(h.finalize().into())
}
