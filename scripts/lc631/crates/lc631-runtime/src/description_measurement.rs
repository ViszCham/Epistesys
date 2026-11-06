//! 日本語: 実reconstruction ownerのCR-Kと派生C_match値。全F1/F2接続ではない。
//! English: Actual reconstruction-owned CR-K plus a derived C_match VALUE.
//! Five fields: K=8*len(K6 Bundle), positive integer bits; current owner costs
//! include semantic term, actual identity-decoder text and empty dictionary;
//! same actual plan/context/lease, encoding/runtime and fixed source baseline;
//! independent literal byte-count/cut tests; derived complexity preference only.
//! Given/Unassessed/SharedDependency stay visible. K does not establish semantic
//! preservation, compression, truth, goal utility, permission or original scores.
//! Caller TransformIds are DECLARED GIVEN references: reconstruction owners do
//! not authenticate/store those IDs. Their full returned bindings stay unchanged.
#![forbid(unsafe_code)]
mod preference;
use crate::{
    facet_ledger::{TransformBinding, TransformId},
    reconstruction_measurement::{
        self as reconstruction, OwnedReconstruction, ProtectedFeatureSet,
    },
};
use lc631_analysis::creativity_measurement::{
    complexity_match, ComponentMetric, ComponentReport, ComponentState, IndependenceStatus,
    InputBasis, MeasurementBinding, MeasurementError, MeasurementGap, RationalInterval,
    SemanticValidation,
};
use lc631_core::context::ContextRevision;
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    semantic_term::GRAMMAR_REVISION,
    Digest32,
};
pub use preference::{compare_current, ComplexityPreference};
use sha2::{Digest, Sha256};

const OWNER_RAM: u64 = 64 * 1024;
const PRODUCER: &[u8] = b"finite-formal-v1.4/CR-K/current-reconstruction-bundle-derived.v1";
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Resource(ResourceError),
    Reconstruction(reconstruction::Error),
    Arithmetic(MeasurementError),
    InvalidScale,
    BaselineMismatch,
    BaselineUnavailable,
    RootMismatch,
    BindingMismatch,
    ForeignReservation,
    InconsistentOwner,
}

/// Borrow the actual owner, immutable target bytes and exact original plan.
/// `transform` is a caller-declared reference, never an owner-authenticated ID.
#[derive(Clone, Copy)]
pub struct ReconstructionInput<'a, 'p, 'v> {
    pub owner: &'a OwnedReconstruction<'p, 'v>,
    pub plan: &'p ProtectedFeatureSet<'v>,
    pub target: &'a [u8],
    pub context: ContextRevision,
    pub transform: TransformId,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransformReferenceBasis {
    DeclaredGiven,
}
/// Explicit derivation lineage, not a replacement/equalization of parent facts.
/// All full reconstruction bindings are stored verbatim, including their own
/// producer/query/result/direction and candidate-specific source/target/context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OriginalLineage {
    pub baseline: TransformBinding,
    pub candidate: TransformBinding,
    pub source_term: Digest32,
    pub encoding: Digest32,
    pub common_runtime: Digest32,
    pub baseline_bits: u64,
    pub candidate_bits: Option<u64>,
    pub scale_bits: u64,
    pub transform_references: TransformReferenceBasis,
}
#[derive(Clone, Copy, Eq, PartialEq)]
struct Fact {
    binding: TransformBinding,
    bits: Option<u64>,
    gap: Option<reconstruction::Gap>,
    runtime: Digest32,
}
/// Fixed before candidate assessment. The integer-positive bit scale is a
/// bounded declared-preference profile, not calibrated F1 production weights.
/// K0 comes only from an actual owner whose target is the original source term.
pub struct RootPlan<'a, 'p, 'v> {
    baseline: ReconstructionInput<'a, 'p, 'v>,
    fact: Fact,
    baseline_bits: u64,
    scale_bits: u64,
    encoding: Digest32,
    digest: Digest32,
    retained: ResourceClaim<'v>,
}
impl<'a, 'p, 'v> RootPlan<'a, 'p, 'v> {
    pub fn capture(
        baseline: ReconstructionInput<'a, 'p, 'v>,
        scale_bits: u64,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        view.check().map_err(Error::Resource)?;
        if scale_bits == 0 {
            return Err(Error::InvalidScale);
        }
        let retained = claim(view)?;
        let fact = observe(baseline, view)?;
        if digest(baseline.target) != baseline.plan.source_term_digest() {
            return Err(Error::BaselineMismatch);
        }
        let baseline_bits = fact.bits.ok_or(Error::BaselineUnavailable)?;
        let encoding = digest(GRAMMAR_REVISION.as_bytes());
        let mut h = Sha256::new();
        h.update(b"CR-K.fixed-original-source-baseline-and-match-scale.v1\0");
        hash_binding(&mut h, fact.binding);
        h.update(baseline.plan.source_term_digest().as_bytes());
        h.update(encoding.as_bytes());
        h.update(fact.runtime.as_bytes());
        h.update(baseline_bits.to_le_bytes());
        h.update(scale_bits.to_le_bytes());
        let root = Self {
            baseline,
            fact,
            baseline_bits,
            scale_bits,
            encoding,
            digest: Digest32::from_bytes(h.finalize().into()),
            retained,
        };
        root.check(view)?;
        Ok(root)
    }
    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        view.check().map_err(Error::Resource)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(view) {
            return Err(Error::ForeignReservation);
        }
        if observe(self.baseline, view)? != self.fact
            || digest(self.baseline.target) != self.baseline.plan.source_term_digest()
            || self.encoding != digest(GRAMMAR_REVISION.as_bytes())
        {
            return Err(Error::BindingMismatch);
        }
        self.retained.check().map_err(Error::Resource)
    }
}

/// Private construction retains actual root/owner/plan/target and resource leases.
/// No caller scalar, report, state, gate or replacement provenance is accepted.
/// ```compile_fail
/// use lc631_runtime::description_measurement::OwnedDescription;
/// use lc631_analysis::creativity_measurement::ComponentReport;
/// fn inject(r: ComponentReport) { OwnedDescription::from_report(r); }
/// ```
/// ```compile_fail
/// use lc631_runtime::description_measurement::OwnedDescription;
/// let _: OwnedDescription<'_, '_, '_, '_> = serde_json::from_str("{}").unwrap();
/// ```
pub struct OwnedDescription<'r, 'a, 'p, 'v> {
    root: &'r RootPlan<'a, 'p, 'v>,
    candidate: ReconstructionInput<'a, 'p, 'v>,
    fact: Fact,
    lineage: OriginalLineage,
    report: ComponentReport,
    // VALUE only, not a second ComponentReport or a JointPareto gain axis.
    match_value: Option<RationalInterval>,
    retained: ResourceClaim<'v>,
}
impl OwnedDescription<'_, '_, '_, '_> {
    pub fn historical_report(&self) -> &ComponentReport {
        &self.report
    }
    /// Crate-private source inspection; never input to a target-only decoder.
    pub(crate) fn inspect_product_bytes_current<R>(
        &self,
        view: &ResourceReservationView<'_>,
        consume: impl for<'p> FnOnce(
            &'p [u8],
            &'p [u8],
            ContextRevision,
            &'p OriginalLineage,
            &'p ComponentReport,
        ) -> R,
    ) -> Result<R, Error> {
        self.check(view)?;
        let result = consume(
            self.root.baseline.target,
            self.candidate.target,
            self.candidate.context,
            &self.lineage,
            &self.report,
        );
        self.check(view)?;
        Ok(result)
    }
    pub fn report_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, Error> {
        self.check(view)?;
        Ok(&self.report)
    }
    pub fn lineage_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<&OriginalLineage, Error> {
        self.check(view)?;
        Ok(&self.lineage)
    }
    pub fn complexity_match_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<Option<&RationalInterval>, Error> {
        self.check(view)?;
        Ok(self.match_value.as_ref())
    }
    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.root.check(view)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(view) {
            return Err(Error::ForeignReservation);
        }
        check_membership(self.root, self.candidate)?;
        if observe(self.candidate, view)? != self.fact {
            return Err(Error::BindingMismatch);
        }
        self.retained.check().map_err(Error::Resource)
    }
}

/// Derive a new CR-K measurement without modifying either reconstruction fact.
/// Costs remain measurable even when protected features fail: no preservation
/// gate is silently inserted into K, and no compression prerequisite is issued.
pub fn produce<'r, 'a, 'p, 'v>(
    root: &'r RootPlan<'a, 'p, 'v>,
    candidate: ReconstructionInput<'a, 'p, 'v>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedDescription<'r, 'a, 'p, 'v>, Error> {
    root.check(view)?;
    check_membership(root, candidate)?;
    let retained = claim(view)?;
    let fact = observe(candidate, view)?;
    if fact.runtime != root.fact.runtime {
        return Err(Error::RootMismatch);
    }
    let lineage = OriginalLineage {
        baseline: root.fact.binding,
        candidate: fact.binding,
        source_term: root.baseline.plan.source_term_digest(),
        encoding: root.encoding,
        common_runtime: fact.runtime,
        baseline_bits: root.baseline_bits,
        candidate_bits: fact.bits,
        scale_bits: root.scale_bits,
        transform_references: TransformReferenceBasis::DeclaredGiven,
    };
    let state = state(fact)?;
    let match_value = fact
        .bits
        .map(|bits| complexity_match(root.baseline_bits, bits, root.scale_bits))
        .transpose()
        .map_err(Error::Arithmetic)?;
    let result = result_digest(root, fact, lineage);
    let original = fact.binding;
    let report = ComponentReport {
        metric: ComponentMetric::DescriptionBits,
        definition: definition_revision(),
        binding: MeasurementBinding {
            scope: Digest32::from_bytes(original.scope.id.0),
            candidate: Digest32::from_bytes(original.target.0),
            source: Digest32::from_bytes(original.source.0),
            target: Digest32::from_bytes(original.target.0),
            context: Digest32::from_bytes(original.target_context.0),
            baseline: Digest32::from_bytes(original.scope.baseline.0),
            producer: digest(PRODUCER),
            transform: Digest32::from_bytes(original.id.0),
            result,
        },
        probe_set: candidate.plan.probe_digest(),
        oracle: digest(b"K6/full-self-contained-bundle-bit-count.v1"),
        dependency_group: root.digest,
        evidence_payload: result,
        basis: InputBasis::Given,
        validation: SemanticValidation::Unassessed,
        independence: IndependenceStatus::SharedDependency,
        state,
    };
    let owned = OwnedDescription {
        root,
        candidate,
        fact,
        lineage,
        report,
        match_value,
        retained,
    };
    owned.check(view)?;
    Ok(owned)
}
pub fn definition_revision() -> Digest32 {
    digest(
        b"OperationalMeasurement/finite-formal-v1.4\0CR-K\0actual-identity-decoder-full-bundle-v1",
    )
}
fn check_membership(
    root: &RootPlan<'_, '_, '_>,
    input: ReconstructionInput<'_, '_, '_>,
) -> Result<(), Error> {
    if !std::ptr::eq(root.baseline.plan, input.plan) || root.baseline.context != input.context {
        return Err(Error::RootMismatch);
    }
    Ok(())
}
fn observe(
    input: ReconstructionInput<'_, '_, '_>,
    view: &ResourceReservationView<'_>,
) -> Result<Fact, Error> {
    // The owner is immutable and retained by borrow. Check current eligibility
    // on BOTH sides of historical field reads; no stale accessor alone is used.
    input
        .owner
        .axis_current(input.plan, input.target, input.context, view)
        .map_err(Error::Reconstruction)?;
    let fact = Fact {
        binding: input.owner.binding(input.transform),
        bits: input.owner.bundle_bits(),
        gap: input.owner.historical_gap(),
        runtime: input.owner.common_runtime_digest(),
    };
    input
        .owner
        .axis_current(input.plan, input.target, input.context, view)
        .map_err(Error::Reconstruction)?;
    if fact.bits == Some(0) || (fact.bits.is_some() && fact.gap.is_some()) {
        return Err(Error::InconsistentOwner);
    }
    Ok(fact)
}
fn state(fact: Fact) -> Result<ComponentState, Error> {
    match (fact.bits, fact.gap) {
        (Some(bits), None) => Ok(ComponentState::Measured(
            RationalInterval::exact_ratio(
                i64::try_from(bits).map_err(|_| Error::InconsistentOwner)?,
                1,
            )
            .map_err(|e| Error::Arithmetic(e.into()))?,
        )),
        (None, Some(reconstruction::Gap::Unsupported)) => {
            Ok(ComponentState::Unknown(MeasurementGap::Unsupported))
        }
        (None, Some(reconstruction::Gap::InvalidTarget)) => Ok(ComponentState::InvalidConfig(
            MeasurementError::InvalidInput,
        )),
        (None, Some(reconstruction::Gap::MissingDecoderCost)) => {
            Ok(ComponentState::Unknown(MeasurementGap::MissingEvaluator))
        }
        (None, None) | (Some(_), Some(_)) => Err(Error::InconsistentOwner),
    }
}
fn digest(bytes: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(bytes).into())
}
fn result_digest(root: &RootPlan<'_, '_, '_>, fact: Fact, lineage: OriginalLineage) -> Digest32 {
    let mut h = Sha256::new();
    h.update(PRODUCER);
    h.update(root.digest.as_bytes());
    hash_binding(&mut h, lineage.baseline);
    hash_binding(&mut h, lineage.candidate);
    h.update(lineage.source_term.as_bytes());
    h.update(lineage.encoding.as_bytes());
    h.update(lineage.common_runtime.as_bytes());
    h.update(lineage.baseline_bits.to_le_bytes());
    h.update(lineage.scale_bits.to_le_bytes());
    match fact.bits {
        Some(bits) => {
            h.update([1]);
            h.update(bits.to_le_bytes());
        }
        None => h.update([0]),
    }
    h.update([match fact.gap {
        None => 0,
        Some(reconstruction::Gap::Unsupported) => 1,
        Some(reconstruction::Gap::InvalidTarget) => 2,
        Some(reconstruction::Gap::MissingDecoderCost) => 3,
    }]);
    Digest32::from_bytes(h.finalize().into())
}
fn hash_binding(h: &mut Sha256, b: TransformBinding) {
    for bytes in [
        b.id.0,
        b.source.0,
        b.target.0,
        b.source_context.0,
        b.target_context.0,
        b.scope.id.0,
        b.scope.baseline.0,
        b.scope.root_goal.0,
        b.scope.probes.0,
        b.scope.policy.0,
        b.producer.0,
        b.query.0,
        b.result.0,
    ] {
        h.update(bytes);
    }
    h.update([match b.direction {
        crate::facet_ledger::Direction::Forward => 0,
        crate::facet_ledger::Direction::Reverse => 1,
    }]);
}
fn claim<'v>(view: &'v ResourceReservationView<'_>) -> Result<ResourceClaim<'v>, Error> {
    view.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(OWNER_RAM).map_err(Error::Resource)?,
    )])
    .map_err(Error::Resource)
}
