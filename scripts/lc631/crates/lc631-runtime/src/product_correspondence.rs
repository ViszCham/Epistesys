//! Bounded Stage B syntax and Given byte accounting; never a strict score.
//! Native identity envelopes are separate from semantic K6 role references.
//! Logical claims exclude allocator overhead/RSS; standard-library OOM remains
//! outside this bounded accounting. No process, storage, authority or formula.
#![forbid(unsafe_code)]
mod bundle;
pub(crate) mod codec;
mod observation;
pub use bundle::{
    CodeRole, ProvidedBundleAccounting, ProvidedBundleInput, ProvidedCodeUnit, ProvidedPayload,
};
pub use observation::{
    inspect_native_current, CurrentNativeInspection, NativeOwnerRefs, OwnedNativeInspection,
};

use crate::{
    adversarial_measurement::{Response, SemanticAtom},
    spatial_preservation::SpatialModelInput,
    temporal_preservation::TemporalModelInput,
};
use lc631_core::{
    context::{ContextRevision, ContextScope},
    measurement_numeric::Rational,
};
use lc631_solvers::typed_query::BoolSymbolId;
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    semantic_term::{Term, TermLimits},
    Digest32,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeferredReason {
    FullCostOwnerMissing,
    MissingInlineCode,
    MissingSemanticDictionary,
    ExternalLookupRequired,
}
#[derive(Debug)]
pub enum ProductError {
    Resource(ResourceError),
    UnsupportedProjection,
    InvalidCanonicalEncoding,
    AmbiguousCaseSpec,
    MissingMeaningDeclaration,
    IdentityMismatch,
    CorrespondenceMismatch,
    Deferred(DeferredReason),
}
impl From<ResourceError> for ProductError {
    fn from(value: ResourceError) -> Self {
        Self::Resource(value)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TlProfile {
    TemporalSpatial1To3,
    OtherRequiredAxes,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IdentityKind {
    Clock,
    Origin,
    Reference,
    Frame,
}

#[derive(Clone, Copy, Debug)]
pub struct ProductLimits {
    pub max_bytes: usize,
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_cases: usize,
    pub max_atoms: usize,
    pub max_response_nodes: usize,
    pub max_temporal_nodes: usize,
    pub max_trace_steps: usize,
    pub max_temporal_atoms: usize,
}
impl Default for ProductLimits {
    fn default() -> Self {
        Self {
            max_bytes: 65_536,
            max_nodes: 1_024,
            max_depth: 64,
            max_cases: 2,
            max_atoms: 4,
            max_response_nodes: 32,
            max_temporal_nodes: 32,
            max_trace_steps: 8,
            max_temporal_atoms: 4,
        }
    }
}
impl ProductLimits {
    pub(crate) fn checked(self) -> Result<Self, ProductError> {
        let reference = Self::default();
        let actual = [
            self.max_bytes,
            self.max_nodes,
            self.max_depth,
            self.max_cases,
            self.max_atoms,
            self.max_response_nodes,
            self.max_temporal_nodes,
            self.max_trace_steps,
            self.max_temporal_atoms,
        ];
        let caps = [
            reference.max_bytes,
            reference.max_nodes,
            reference.max_depth,
            reference.max_cases,
            reference.max_atoms,
            reference.max_response_nodes,
            reference.max_temporal_nodes,
            reference.max_trace_steps,
            reference.max_temporal_atoms,
        ];
        if actual.iter().zip(caps).any(|(v, cap)| *v == 0 || *v > cap) {
            return Err(ProductError::UnsupportedProjection);
        }
        Ok(self)
    }
    pub(crate) fn term(self) -> TermLimits {
        TermLimits {
            max_bytes: self.max_bytes,
            max_nodes: self.max_nodes,
            max_depth: self.max_depth,
        }
    }
}
pub struct CaseDeclaration<'a> {
    pub native_key: &'a [u8],
    pub native_atoms: &'a [SemanticAtom<'a>],
    pub semantic_spec: &'a [u8],
    pub reference_contract: &'a [u8],
    pub oracle: Digest32,
    pub weight: &'a Rational,
}
pub struct IdentityDeclaration<'a> {
    pub kind: IdentityKind,
    pub local_role: u16,
    pub native_id: [u8; 32],
    pub meaning: &'a [u8],
}
pub struct PlanSpec<'a> {
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub goal_profile: Option<Digest32>,
    pub tl_profile: TlProfile,
    pub cases: &'a [CaseDeclaration<'a>],
    pub identities: &'a [IdentityDeclaration<'a>],
    pub temporal_atom_meanings: &'a [&'a [u8]],
    pub limits: ProductLimits,
}
pub struct ProductInput<'a> {
    pub responses: &'a [Response<'a>],
    pub temporal: TemporalModelInput<'a>,
    pub spatial: SpatialModelInput,
}
pub(crate) struct OwnedAtom {
    pub(crate) symbol: BoolSymbolId,
    pub(crate) meaning: Vec<u8>,
}
pub struct FixedCaseSlot {
    pub(crate) native_key: Vec<u8>,
    pub(crate) native_atoms: Vec<OwnedAtom>,
    pub(crate) semantic_spec: Vec<u8>,
    pub(crate) spec: Term,
    pub(crate) reference_contract: Vec<u8>,
    pub(crate) reference: Term,
    pub(crate) oracle: Digest32,
    pub(crate) weight: Rational,
}
impl FixedCaseSlot {
    pub fn native_key(&self) -> &[u8] {
        &self.native_key
    }
    pub fn semantic_spec(&self) -> &[u8] {
        &self.semantic_spec
    }
    pub fn reference_contract(&self) -> &[u8] {
        &self.reference_contract
    }
    pub fn oracle(&self) -> Digest32 {
        self.oracle
    }
    pub fn weight(&self) -> &Rational {
        &self.weight
    }
}
pub(crate) struct OwnedIdentity {
    pub(crate) kind: IdentityKind,
    pub(crate) local_role: u16,
    pub(crate) native_id: [u8; 32],
    pub(crate) meaning_bytes: Vec<u8>,
    pub(crate) meaning: Term,
}
pub(crate) struct PlanData {
    pub(crate) context: ContextRevision,
    pub(crate) scope: ContextScope,
    pub(crate) goal_profile: Option<Digest32>,
    pub(crate) tl_profile: TlProfile,
    pub(crate) limits: ProductLimits,
    pub(crate) cases: Vec<FixedCaseSlot>,
    pub(crate) identities: Vec<OwnedIdentity>,
    pub(crate) temporal_atom_meanings: Vec<Term>,
    pub(crate) binding: Digest32,
}
pub struct ProductPlan<'view> {
    pub(crate) data: Arc<PlanData>,
    retained: ResourceClaim<'view>,
}
impl<'view> ProductPlan<'view> {
    pub fn declared(
        spec: PlanSpec<'_>,
        view: &'view ResourceReservationView<'_>,
    ) -> Result<Self, ProductError> {
        view.check()?;
        if spec.context.source().digest() != view.source_revision() {
            return Err(ResourceError::SourceMismatch.into());
        }
        let limits = spec.limits.checked()?;
        if spec.tl_profile != TlProfile::TemporalSpatial1To3 {
            return Err(ProductError::UnsupportedProjection);
        }
        codec::cases::preflight(&spec, limits)?;
        codec::roles::preflight(spec.identities, limits)?;
        let retained = claim(view, 256 * 1024)?;
        let scratch = claim(view, SCRATCH_BYTES)?;
        let cases = codec::cases::capture(spec.cases, limits)?;
        let identities = codec::roles::capture(spec.identities, limits)?;
        let mut temporal_atom_meanings = reserved(spec.temporal_atom_meanings.len())?;
        if spec.temporal_atom_meanings.len() > limits.max_temporal_atoms {
            return Err(ProductError::UnsupportedProjection);
        }
        for bytes in spec.temporal_atom_meanings {
            let term = codec::decode(bytes, limits)?;
            if !matches!(&term,Term::Symbol(s) if !s.is_empty() && s.len() <= 256)
                || temporal_atom_meanings.contains(&term)
            {
                return Err(ProductError::UnsupportedProjection);
            }
            temporal_atom_meanings.push(term);
        }
        let mut data = PlanData {
            context: spec.context,
            scope: spec.scope,
            goal_profile: spec.goal_profile,
            tl_profile: spec.tl_profile,
            limits,
            cases,
            identities,
            temporal_atom_meanings,
            binding: Digest32::from_bytes([0; 32]),
        };
        data.binding = codec::plan_binding(&data)?;
        scratch.check()?;
        retained.check()?;
        Ok(Self {
            data: Arc::new(data),
            retained,
        })
    }
    pub fn fixed_slots(&self) -> &[FixedCaseSlot] {
        &self.data.cases
    }
    pub(crate) fn check_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), ProductError> {
        view.check()?;
        self.retained.check()?;
        if self.data.tl_profile != TlProfile::TemporalSpatial1To3 {
            return Err(ProductError::UnsupportedProjection);
        }
        if !self.retained.same_reservation(view) {
            return Err(ResourceError::ForeignOwner.into());
        }
        if self.data.context.source().digest() != view.source_revision() {
            return Err(ResourceError::SourceMismatch.into());
        }
        Ok(())
    }
    pub fn project_reported(
        &self,
        input: ProductInput<'_>,
        view: &'view ResourceReservationView<'_>,
    ) -> Result<ReportedProduct<'view>, ProductError> {
        self.check_current(view)?;
        let retained = claim(view, 512 * 1024)?;
        let scratch = claim(view, SCRATCH_BYTES)?;
        let term = codec::project(&self.data, input)?;
        let bytes = codec::encode(&term, self.data.limits)?;
        let envelope = codec::roles::envelope(&self.data)?;
        scratch.check()?;
        self.check_current(view)?;
        retained.check()?;
        Ok(ReportedProduct {
            plan: Arc::clone(&self.data),
            term,
            bytes,
            envelope,
            retained,
        })
    }
    pub fn decode_reported(
        &self,
        bytes: &[u8],
        envelope: &[u8],
        view: &'view ResourceReservationView<'_>,
    ) -> Result<ReportedProduct<'view>, ProductError> {
        self.check_current(view)?;
        if bytes.len() > self.data.limits.max_bytes || envelope.len() > self.data.limits.max_bytes {
            return Err(ProductError::UnsupportedProjection);
        }
        let retained = claim(view, 512 * 1024)?;
        let scratch = claim(view, SCRATCH_BYTES)?;
        if codec::roles::envelope(&self.data)? != envelope {
            return Err(ProductError::IdentityMismatch);
        }
        let term = codec::decode(bytes, self.data.limits)?;
        codec::validate(&self.data, &term)?;
        let bytes = copy(bytes)?;
        let envelope = copy(envelope)?;
        scratch.check()?;
        self.check_current(view)?;
        retained.check()?;
        Ok(ReportedProduct {
            plan: Arc::clone(&self.data),
            term,
            bytes,
            envelope,
            retained,
        })
    }
}
/// Reported canonical syntax only. Current native correspondence is a distinct
/// owner in observation; neither object can construct a full-cost K owner.
pub struct ReportedProduct<'view> {
    plan: Arc<PlanData>,
    term: Term,
    bytes: Vec<u8>,
    envelope: Vec<u8>,
    retained: ResourceClaim<'view>,
}
impl ReportedProduct<'_> {
    pub fn semantic_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn identity_envelope_bytes(&self) -> &[u8] {
        &self.envelope
    }
    pub fn scalar_status(&self) -> DeferredReason {
        DeferredReason::FullCostOwnerMissing
    }
    pub const fn formula_evaluated(&self) -> bool {
        false
    }
    pub(crate) fn term(&self) -> &Term {
        &self.term
    }
    pub(crate) fn check_current(
        &self,
        plan: &ProductPlan<'_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), ProductError> {
        plan.check_current(view)?;
        self.retained.check()?;
        if !self.retained.same_reservation(view) {
            return Err(ResourceError::ForeignOwner.into());
        }
        if !Arc::ptr_eq(&self.plan, &plan.data) {
            return Err(ProductError::CorrespondenceMismatch);
        }
        Ok(())
    }
}
pub(crate) const SCRATCH_BYTES: u64 = 4 * 1024 * 1024;
pub(crate) fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    bytes: u64,
) -> Result<ResourceClaim<'v>, ProductError> {
    view.check()?;
    Ok(view.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(bytes)?,
    )])?)
}
pub(crate) fn reserved<T>(n: usize) -> Result<Vec<T>, ProductError> {
    let mut out = Vec::new();
    out.try_reserve_exact(n)
        .map_err(|_| ProductError::Resource(ResourceError::AllocationFailed))?;
    Ok(out)
}
pub(crate) fn copy(bytes: &[u8]) -> Result<Vec<u8>, ProductError> {
    let mut out = reserved(bytes.len())?;
    out.extend_from_slice(bytes);
    Ok(out)
}
pub(crate) fn digest(domain: &[u8], bytes: &[u8]) -> Digest32 {
    let mut h = Sha256::new();
    h.update(domain);
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
    Digest32::from_bytes(h.finalize().into())
}
