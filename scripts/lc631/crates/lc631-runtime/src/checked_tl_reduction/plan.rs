//! Actual source tuple and immutable pre-candidate policy; no caller mask or weights.
use super::*;
use crate::spatial_preservation::{SpatialModelInput, SpatialPreservationPolicy};
use crate::temporal_preservation::{TemporalModelInput, TemporalPreservationPolicy};
use lc631_core::context::{ContextRevision, ContextScope};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

/// Fixed named operational preference, not calibrated universal TL.
/// Reconstructed contexts may share the source revision but cannot change scope.
///
/// ```compile_fail
/// use lc631_runtime::checked_tl_reduction::RootPlan;
/// fn forge() { RootPlan::from_mask_and_weights(&[], &[0,0]); }
/// ```
pub struct RootPlan<'input, 'policy, 'view> {
    pub(super) temporal: TemporalModelInput<'input>,
    pub(super) spatial: SpatialModelInput,
    pub(super) temporal_policy: &'policy TemporalPreservationPolicy,
    pub(super) spatial_policy: &'policy SpatialPreservationPolicy,
    pub(super) digest: Digest32,
    retained: ResourceClaim<'view>,
}
impl<'input, 'policy, 'view> RootPlan<'input, 'policy, 'view> {
    /// Declare BEFORE candidate capture. Policies must bind source→same-source.
    /// Actual input association is Given, not an authenticated semantic bridge.
    pub fn capture(
        temporal: TemporalModelInput<'input>,
        spatial: SpatialModelInput,
        temporal_policy: &'policy TemporalPreservationPolicy,
        spatial_policy: &'policy SpatialPreservationPolicy,
        resources: &'view ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let scratch = claim(resources, SCRATCH)?;
        let retained = claim(resources, RETAINED)?;
        if temporal.scope != spatial.scope
            || temporal.query.context() != spatial.context
            || temporal.query.context().source().digest() != resources.source_revision()
        {
            return Err(Error::InvalidRoot);
        }
        // Constructors bind exact query/trace/context and rectangle/frame/unit payloads.
        let t = TemporalPreservationPolicy::declared(
            temporal,
            temporal,
            temporal_policy.probes().to_vec(),
            temporal_policy.mapping(),
        )
        .map_err(Error::Temporal)?;
        let s = SpatialPreservationPolicy::declared(
            spatial,
            spatial,
            spatial_policy.probes().to_vec(),
            spatial_policy.mapping(),
        )
        .map_err(Error::Spatial)?;
        if t.binding_digest() != temporal_policy.binding_digest()
            || s.binding_digest() != spatial_policy.binding_digest()
        {
            return Err(Error::BindingMismatch);
        }
        let mut hash = Sha256::new();
        hash.update(PROFILE.as_bytes());
        hash.update([1, 3]);
        hash.update(DENOMINATOR.to_le_bytes());
        hash.update(t.binding_digest().as_bytes());
        hash.update(s.binding_digest().as_bytes());
        let root = Self {
            temporal,
            spatial,
            temporal_policy,
            spatial_policy,
            digest: Digest32::from_bytes(hash.finalize().into()),
            retained,
        };
        scratch.check().map_err(Error::Resource)?;
        root.check(resources)?;
        Ok(root)
    }
    /// Borrow only the actual source tuple, independent of any candidate.
    pub(crate) fn inspect_product_source_current<R>(
        &self,
        resources: &ResourceReservationView<'_>,
        consume: impl for<'p> FnOnce(
            TemporalModelInput<'p>,
            SpatialModelInput,
            &'p TemporalPreservationPolicy,
            &'p SpatialPreservationPolicy,
            Digest32,
        ) -> R,
    ) -> Result<R, Error> {
        self.check(resources)?;
        let result = consume(
            self.temporal,
            self.spatial,
            self.temporal_policy,
            self.spatial_policy,
            self.digest,
        );
        self.check(resources)?;
        Ok(result)
    }

    pub fn digest(&self) -> Digest32 {
        self.digest
    }
    pub fn context(&self) -> ContextRevision {
        self.temporal.query.context()
    }
    pub fn scope(&self) -> ContextScope {
        self.temporal.scope
    }
    pub(super) fn check(&self, resources: &ResourceReservationView<'_>) -> Result<(), Error> {
        resources.check().map_err(Error::Resource)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(resources)
            || self.context().source().digest() != resources.source_revision()
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
}
/// Candidate IS an immutable formal target tuple, not a caller CandidateId/hash.
/// No Clone/Deserialize/mutable view; exact owner consumption establishes mapping.
pub struct TupleCandidate<'root, 'input, 'policy, 'view> {
    pub(super) root: &'root RootPlan<'input, 'policy, 'view>,
    pub(super) temporal: TemporalModelInput<'input>,
    pub(super) spatial: SpatialModelInput,
}
impl<'root, 'input, 'policy, 'view> TupleCandidate<'root, 'input, 'policy, 'view> {
    pub fn capture(
        root: &'root RootPlan<'input, 'policy, 'view>,
        temporal: TemporalModelInput<'input>,
        spatial: SpatialModelInput,
    ) -> Result<Self, Error> {
        if temporal.scope != root.scope()
            || spatial.scope != root.scope()
            || temporal.query.context() != spatial.context
            || temporal.query.context().source() != root.context().source()
        {
            return Err(Error::BindingMismatch);
        }
        Ok(Self {
            root,
            temporal,
            spatial,
        })
    }
    pub fn context(&self) -> ContextRevision {
        self.temporal.query.context()
    }
    pub fn root_digest(&self) -> Digest32 {
        self.root.digest
    }
}
