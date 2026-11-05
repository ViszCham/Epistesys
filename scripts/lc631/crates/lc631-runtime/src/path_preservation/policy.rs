//! Immutable root probes and explicit, identity-only namespace composition.
use super::{claim, hash, recon, reserved, Error};
use crate::facet_ledger::{ComparisonScope, ObligationId, PropertyId};
use lc631_core::context::ContextRevision;
use lc631_substrate::{resource::{ResourceClaim, ResourceReservationView}, Digest32};

pub(super) const MAX_EDGES: usize = 16;
const MAX_BYTES: usize = 65_536;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredMap {
    namespace: Digest32,
    registration: Digest32,
    identity: bool,
}
impl RegisteredMap {
    /// Registration is declared policy, not authenticated physical semantics.
    pub fn identity_namespace(namespace: Digest32, registration: Digest32) -> Result<Self, Error> {
        if namespace.as_bytes() == &[0;32] || registration.as_bytes() == &[0;32] {
            return Err(Error::InvalidPlan);
        }
        Ok(Self {namespace, registration, identity:true})
    }
    pub fn unsupported(profile: Digest32) -> Self {
        Self {namespace:profile, registration:profile, identity:false}
    }
    pub(super) fn digest(self) -> Digest32 {
        let mut bytes=[0_u8;64];
        bytes[..32].copy_from_slice(self.namespace.as_bytes());
        bytes[32..].copy_from_slice(self.registration.as_bytes());
        hash(&bytes)
    }
}
#[derive(Clone, Copy)]
pub struct PathEdge<'artifact> {
    pub(super) from: &'artifact [u8],
    pub(super) to: &'artifact [u8],
    pub(super) mapping: RegisteredMap,
}
impl<'artifact> PathEdge<'artifact> {
    pub fn new(from: &'artifact [u8], to: &'artifact [u8], mapping: RegisteredMap) -> Self {
        Self {from,to,mapping}
    }
}
pub struct RootProbeSpec {
    pub obligation: ObligationId,
    pub property: PropertyId,
    pub feature: recon::ProbeSpec,
    pub hard: bool,
}
pub(super) struct RootProbe<'view> {
    pub(super) obligation: ObligationId,
    pub(super) property: PropertyId,
    pub(super) hard: bool,
    pub(super) weight: lc631_analysis::creativity_measurement::Rational,
    pub(super) protected: recon::ProtectedFeatureSet<'view>,
}
pub struct RootPathPlan<'root, 'view> {
    pub(super) root: &'root [u8],
    pub(super) context: ContextRevision,
    pub(super) scope: ComparisonScope,
    pub(super) mapping: RegisteredMap,
    pub(super) probes: Vec<RootProbe<'view>>,
    held: ResourceClaim<'view>,
}
impl<'root, 'view> RootPathPlan<'root, 'view> {
    pub fn capture(
        root: &'root [u8], specs: &[RootProbeSpec], context: ContextRevision,
        scope: ComparisonScope, mapping: RegisteredMap,
        resources: &'view ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        resources.check().map_err(Error::Resource)?;
        if resources.source_revision() != context.source().digest() {return Err(Error::Binding);}
        if !mapping.identity {return Err(Error::UnsupportedMap);}
        if root.len()>MAX_BYTES || specs.is_empty() || specs.len()>8
            || !specs.iter().any(|s| s.hard) {return Err(Error::InvalidPlan);}
        let scratch=claim(resources,256*1024,false)?;
        let held=claim(resources,128*1024,true)?;
        validate_specs(specs)?;
        let mut probes=reserved(specs.len())?;
        for spec in specs {
            let protected=recon::ProtectedFeatureSet::capture(root,
                std::slice::from_ref(&spec.feature),context,scope,resources)
                .map_err(Error::Reconstruction)?;
            probes.push(RootProbe {obligation:spec.obligation,property:spec.property,
                hard:spec.hard,weight:spec.feature.weight.clone(),protected});
        }
        scratch.check().map_err(Error::Resource)?;
        held.check().map_err(Error::Resource)?;
        Ok(Self {root,context,scope,mapping,probes,held})
    }
    pub fn probe_count(&self) -> usize {self.probes.len()}
    pub fn root_revision(&self) -> Digest32 {hash(self.root)}
    pub(super) fn current(&self, resources: &ResourceReservationView<'_>) -> Result<(), Error> {
        resources.check().map_err(Error::Resource)?;
        self.held.check().map_err(Error::Resource)?;
        if !self.held.same_reservation(resources)
            || resources.source_revision()!=self.context.source().digest() {
            return Err(Error::Binding);
        }
        Ok(())
    }
}
fn validate_specs(specs: &[RootProbeSpec]) -> Result<(), Error> {
    for (i,spec) in specs.iter().enumerate() {
        if specs[..i].iter().any(|prior| (prior.obligation==spec.obligation
            && prior.property==spec.property) || (prior.feature.path==spec.feature.path
            && prior.feature.kind==spec.feature.kind)) {
            return Err(Error::InvalidPlan);
        }
    }
    Ok(())
}
pub(super) fn check_path(
    plan: &RootPathPlan<'_, '_>, edges: &[PathEdge<'_>], final_bytes: &[u8],
) -> Result<(), Error> {
    if edges.len()>MAX_EDGES || final_bytes.len()>MAX_BYTES {return Err(Error::Budget);}
    let mut previous=plan.root;
    for edge in edges {
        if edge.from.len()>MAX_BYTES || edge.to.len()>MAX_BYTES {return Err(Error::Budget);}
        if !edge.mapping.identity || edge.mapping!=plan.mapping {return Err(Error::UnsupportedMap);}
        // Exact adjoining bytes AND their digest bind the registered map path.
        if edge.from!=previous || hash(edge.from)!=hash(previous) {return Err(Error::BrokenPath);}
        previous=edge.to;
    }
    if previous!=final_bytes || hash(previous)!=hash(final_bytes) {return Err(Error::BrokenPath);}
    Ok(())
}
