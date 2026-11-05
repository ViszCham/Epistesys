//! Resource-owned, target-only finite structural reconstruction; no NL oracle.
mod consumer;
mod decoder;
mod features;
mod selection;
use crate::facet_ledger::ComparisonScope;
pub use features::{FeatureGap, FeatureKind, FeatureOutcome, ProbeSpec};
use lc631_analysis::creativity_measurement::{Rational, RationalInterval};
use lc631_core::context::ContextRevision;
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use lc631_substrate::{
    semantic_term::{self, Term, TermLimits},
    Digest32,
};
pub use selection::{select_current, OwnedSelection, SelectionGoal, SelectionSnapshot};
use sha2::{Digest, Sha256};

const MAX_INPUT: usize = 65_536;
const MAX_PROBES: usize = 64;
const SCRATCH: u64 = 16 * 1024 * 1024;
const RETAINED: u64 = 2 * 1024 * 1024;
fn limits() -> TermLimits {
    TermLimits {
        max_bytes: MAX_INPUT,
        max_nodes: 1024,
        max_depth: 64,
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Resource(ResourceError),
    Codec(semantic_term::TermError),
    InvalidPlan,
    Binding,
    Budget,
    Arithmetic,
    Ledger(crate::facet_ledger::LedgerError),
    Selection(crate::joint_selection::SelectionError),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gap {
    Unsupported,
    InvalidTarget,
    MissingDecoderCost,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputBasis {
    Given,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticStatus {
    Unverified,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Independence {
    SharedDependency,
}
struct Feature {
    spec: ProbeSpec,
    expected: Vec<u8>,
    canonical: Vec<u8>,
}
/// Source checker owns the fixed, immutable universe, expectations and weights.
/// Source interpretation is Given/Unverified, never inferred from context hashes.
pub struct ProtectedFeatureSet<'view> {
    features: Vec<Feature>,
    context: ContextRevision,
    scope: ComparisonScope,
    source_term: Digest32,
    query: Digest32,
    _claim: ResourceClaim<'view>,
}
impl<'view> ProtectedFeatureSet<'view> {
    pub fn capture(
        source_term: &[u8],
        specs: &[ProbeSpec],
        context: ContextRevision,
        scope: ComparisonScope,
        resources: &'view ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        check_source(resources, context)?;
        if source_term.len() > MAX_INPUT || specs.is_empty() || specs.len() > MAX_PROBES {
            return Err(Error::Budget);
        }
        let scratch = claim(resources, SCRATCH, false)?;
        let held = claim(resources, RETAINED, true)?;
        // Validate caller-owned paths/numeric sizes under preclaimed workspace.
        for (i, spec) in specs.iter().enumerate() {
            if spec.path.len() > 64
                || spec.path.iter().any(|v| *v > 1024)
                || spec.weight.numer().bits() > 1024
                || spec.weight.denom().bits() > 1024
                || lc631_substrate::measurement_numeric::check(&spec.weight).is_err()
                || spec.weight <= Rational::from_integer(0.into())
            {
                return Err(Error::InvalidPlan);
            }
            if specs[..i]
                .iter()
                .any(|old| old.path == spec.path && old.kind == spec.kind)
            {
                return Err(Error::InvalidPlan);
            }
        }
        let source = semantic_term::decode(source_term, limits()).map_err(Error::Codec)?;
        let mut features = Vec::new();
        features
            .try_reserve_exact(specs.len())
            .map_err(|_| Error::Budget)?;
        let mut hasher = Sha256::new();
        hasher.update(b"tlcx18-fixed-probes-v1");
        for spec in specs {
            let expected = features::extract(&source, spec)?.ok_or(Error::InvalidPlan)?;
            let canonical = features::canonical(spec);
            hasher.update((canonical.len() as u64).to_le_bytes());
            hasher.update(&canonical);
            hasher.update((expected.len() as u64).to_le_bytes());
            hasher.update(&expected);
            let weight = spec.weight.to_string();
            hasher.update((weight.len() as u64).to_le_bytes());
            hasher.update(weight.as_bytes());
            features.push(Feature {
                spec: spec.clone(),
                expected,
                canonical,
            });
        }
        let query = Digest32::from_bytes(hasher.finalize().into());
        scratch.check().map_err(Error::Resource)?;
        held.check().map_err(Error::Resource)?;
        Ok(Self {
            features,
            context,
            scope,
            source_term: hash(source_term),
            query,
            _claim: held,
        })
    }
    pub fn probe_digest(&self) -> Digest32 {
        self.query
    }
    pub fn source_term_digest(&self) -> Digest32 {
        self.source_term
    }
    pub fn feature_count(&self) -> usize {
        self.features.len()
    }
}
struct Report {
    bounds: Option<RationalInterval>,
    // Decode/cost outcome only; never a feature-extraction observation flag.
    gap: Option<Gap>,
    outcomes: Vec<FeatureOutcome>,
    bundle_bits: Option<u64>,
    target: Digest32,
    result: Digest32,
}
/// Private producer report and claim; no from_report/status/Deserialize entry.
pub struct OwnedReconstruction<'plan, 'view> {
    report: Report,
    plan: &'plan ProtectedFeatureSet<'view>,
    held: ResourceClaim<'view>,
}
impl OwnedReconstruction<'_, '_> {
    pub fn historical_bounds(&self) -> Option<&RationalInterval> {
        self.report.bounds.as_ref()
    }
    /// Historical decode/cost gap only. None does not certify feature
    /// observation; inspect current typed feature outcomes and loss bounds.
    pub fn historical_gap(&self) -> Option<Gap> {
        self.report.gap
    }
    pub fn bundle_bits(&self) -> Option<u64> {
        self.report.bundle_bits
    }
    pub fn input_basis(&self) -> InputBasis {
        InputBasis::Given
    }
    pub fn semantic_status(&self) -> SemanticStatus {
        SemanticStatus::Unverified
    }
    pub fn independence(&self) -> Independence {
        Independence::SharedDependency
    }
    pub fn common_runtime_digest(&self) -> Digest32 {
        hash(decoder::COMMON_RUNTIME.as_bytes())
    }
    /// Actual fixed-probe outcomes, including separately typed extraction gaps.
    /// The cost gap/bits are independent. Current reads require the exact owner,
    /// plan, target and resource lease; this issues no semantic truth or permit.
    pub fn feature_outcomes_current(
        &self,
        plan: &ProtectedFeatureSet<'_>,
        target: &[u8],
        context: ContextRevision,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&[FeatureOutcome], Error> {
        self.check(plan, target, context, resources)?;
        Ok(&self.report.outcomes)
    }
    fn check(
        &self,
        plan: &ProtectedFeatureSet<'_>,
        target: &[u8],
        context: ContextRevision,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        check_source(resources, context)?;
        self.held.check().map_err(Error::Resource)?;
        self.plan._claim.check().map_err(Error::Resource)?;
        if !self.held.same_reservation(resources)
            || !self.plan._claim.same_reservation(resources)
            || !std::ptr::eq(plan, self.plan)
            || context != plan.context
            || target.len() > MAX_INPUT
            || hash(target) != self.report.target
        {
            return Err(Error::Binding);
        }
        Ok(())
    }
}
pub fn produce<'plan, 'view>(
    plan: &'plan ProtectedFeatureSet<'view>,
    target: &[u8],
    resources: &'view ResourceReservationView<'_>,
) -> Result<OwnedReconstruction<'plan, 'view>, Error> {
    check_source(resources, plan.context)?;
    plan._claim.check().map_err(Error::Resource)?;
    if !plan._claim.same_reservation(resources) {
        return Err(Error::Binding);
    }
    if target.len() > MAX_INPUT {
        return Err(Error::Budget);
    }
    let scratch = claim(resources, SCRATCH, false)?;
    let held = claim(resources, RETAINED, true)?;
    // This is the ONLY decoder call. The decoder has no source/probe/plan
    // parameter, callback, source digest, dictionary lookup or I/O capability.
    let decoded = decoder::decode_target(target, limits());
    let mut report = Report {
        bounds: None,
        gap: None,
        outcomes: Vec::new(),
        bundle_bits: None,
        target: hash(target),
        result: Digest32::from_bytes([0; 32]),
    };
    match decoded {
        Err(semantic_term::TermError::Unsupported) => report.gap = Some(Gap::Unsupported),
        Err(_) => report.gap = Some(Gap::InvalidTarget),
        Ok(reconstructed) => {
            // Count actual pinned source text of the pure decoder plus its
            // explicit empty dictionary, under the same K6 bundle convention.
            let descriptor = Term::Symbol(decoder::CODE.to_owned());
            match semantic_term::bundle_bits(
                &reconstructed,
                &descriptor,
                &Term::Bool(false),
                limits(),
            ) {
                Err(_) => report.gap = Some(Gap::MissingDecoderCost),
                Ok(bits) => report.bundle_bits = Some(bits),
            }
            // Independent feature lane: cost failure does not remove valid
            // measurements, and capped extraction does not destroy valid K.
            // Global resource/allocation/arithmetic failures remain errors.
            resources.check().map_err(Error::Resource)?;
            let (outcomes, bounds) = features::measure(plan, &reconstructed)?;
            report.outcomes = outcomes;
            report.bounds = Some(bounds);
        }
    }
    let mut h = Sha256::new();
    h.update(b"tlcx18-reconstruction-result-v2-independent-cost-feature");
    h.update(plan.query.as_bytes());
    h.update(plan.source_term.as_bytes());
    h.update(report.target.as_bytes());
    h.update(decoder::CODE.as_bytes());
    h.update(hash(features::CODE.as_bytes()).as_bytes());
    h.update(hash(decoder::COMMON_RUNTIME.as_bytes()).as_bytes());
    h.update((report.outcomes.len() as u64).to_le_bytes());
    for outcome in &report.outcomes {
        h.update([match outcome {
            FeatureOutcome::Preserved => 0,
            FeatureOutcome::Violated => 1,
            FeatureOutcome::Unknown(FeatureGap::ExtractionBudget) => 2,
        }]);
    }
    match report.bundle_bits {
        Some(bits) => {
            h.update([1]);
            h.update(bits.to_le_bytes());
        }
        None => h.update([0]),
    }
    h.update([match report.gap {
        None => 0,
        Some(Gap::Unsupported) => 1,
        Some(Gap::InvalidTarget) => 2,
        Some(Gap::MissingDecoderCost) => 3,
    }]);
    report.result = Digest32::from_bytes(h.finalize().into());
    scratch.check().map_err(Error::Resource)?;
    held.check().map_err(Error::Resource)?;
    Ok(OwnedReconstruction { report, plan, held })
}
fn hash(bytes: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(bytes).into())
}
fn check_source(
    resources: &ResourceReservationView<'_>,
    context: ContextRevision,
) -> Result<(), Error> {
    resources.check().map_err(Error::Resource)?;
    if resources.source_revision() != context.source().digest() {
        return Err(Error::Binding);
    }
    Ok(())
}
fn claim<'a>(
    resources: &'a ResourceReservationView<'_>,
    ram: u64,
    output: bool,
) -> Result<ResourceClaim<'a>, Error> {
    let request = |kind| {
        ResourceAmount::checked(ram)
            .map(|n| ResourceRequest::new(kind, n))
            .map_err(Error::Resource)
    };
    if output {
        resources
            .claim_resources(&[request(ResourceKind::Ram)?, request(ResourceKind::Output)?])
            .map_err(Error::Resource)
    } else {
        resources
            .claim_resources(&[request(ResourceKind::Ram)?])
            .map_err(Error::Resource)
    }
}
