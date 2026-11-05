//! Root-bound finite reconstruction along an explicitly mapped artifact path.
//! This is not authority, source-language validation, or a global completion gate.
#![forbid(unsafe_code)]

use crate::facet_ledger::*;
use crate::joint_selection::AxisValue;
use crate::reconstruction_measurement::{
    self as recon, OwnedReconstruction, ProbeSpec, ProtectedFeatureSet,
};
use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::context::ContextRevision;
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

const MAX_BYTES: usize = 65_536;
const MAX_PROBES: usize = 8;
const MAX_EDGES: usize = 8;
const WRAPPER_RAM: u64 = 1024 * 1024;
// Conservative admission, in addition to the producer's own live claims.
const PRODUCER_SCRATCH: u64 = 16 * 1024 * 1024;
const PRODUCER_RETAINED: u64 = 2 * 1024 * 1024;

#[derive(Debug)]
pub enum Error {
    Resource(ResourceError),
    Reconstruction(recon::Error),
    Ledger(LedgerError),
    InvalidPlan,
    UnsupportedMap,
    BrokenPath,
    Binding,
    Budget,
}
impl From<recon::Error> for Error {
    fn from(e: recon::Error) -> Self {
        Self::Reconstruction(e)
    }
}
impl From<LedgerError> for Error {
    fn from(e: LedgerError) -> Self {
        Self::Ledger(e)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisteredMap {
    IdentityNamespace {
        namespace: Digest32,
        revision: Digest32,
    },
    Unsupported(Digest32),
}
impl RegisteredMap {
    pub fn identity_namespace(namespace: Digest32, revision: Digest32) -> Result<Self, Error> {
        if namespace.as_bytes() == &[0; 32] || revision.as_bytes() == &[0; 32] {
            return Err(Error::InvalidPlan);
        }
        Ok(Self::IdentityNamespace {
            namespace,
            revision,
        })
    }
    pub fn unsupported(id: Digest32) -> Self {
        Self::Unsupported(id)
    }
    fn supported(self) -> bool {
        match self {
            Self::IdentityNamespace {
                namespace,
                revision,
            } => namespace.as_bytes() != &[0; 32] && revision.as_bytes() != &[0; 32],
            Self::Unsupported(_) => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RootProbeSpec {
    pub obligation: ObligationId,
    pub property: PropertyId,
    pub feature: ProbeSpec,
    pub hard: bool,
}

/// The plan owns the source bytes, protected features and registered mapping.
pub struct RootPathPlan<'view> {
    features: ProtectedFeatureSet<'view>,
    root: Vec<u8>,
    probes: Vec<RootProbeSpec>,
    context: ContextRevision,
    scope: ComparisonScope,
    map: RegisteredMap,
    held: ResourceClaim<'view>,
}
impl<'view> RootPathPlan<'view> {
    pub fn capture(
        root: &[u8],
        probes: &[RootProbeSpec],
        context: ContextRevision,
        scope: ComparisonScope,
        map: RegisteredMap,
        resources: &'view ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        check_view(resources, context)?;
        if !map.supported() {
            return Err(Error::UnsupportedMap);
        }
        if root.len() > MAX_BYTES || probes.is_empty() || probes.len() > MAX_PROBES {
            return Err(Error::Budget);
        }
        for (i, p) in probes.iter().enumerate() {
            if p.feature.path.len() > 64
                || p.feature.weight.numer().bits() > 1024
                || p.feature.weight.denom().bits() > 1024
                || probes[..i]
                    .iter()
                    .any(|x| x.obligation == p.obligation && x.property == p.property)
            {
                return Err(Error::InvalidPlan);
            }
        }
        let scratch = claim(resources, WRAPPER_RAM + PRODUCER_SCRATCH, false)?;
        let held = claim(resources, WRAPPER_RAM + PRODUCER_RETAINED, true)?;
        let mut root_copy = Vec::new();
        root_copy
            .try_reserve_exact(root.len())
            .map_err(|_| Error::Budget)?;
        root_copy.extend_from_slice(root);
        let mut owned_probes = Vec::new();
        owned_probes
            .try_reserve_exact(probes.len())
            .map_err(|_| Error::Budget)?;
        owned_probes.extend_from_slice(probes);
        let mut features = Vec::new();
        features
            .try_reserve_exact(probes.len())
            .map_err(|_| Error::Budget)?;
        features.extend(probes.iter().map(|p| p.feature.clone()));
        let set = ProtectedFeatureSet::capture(root, &features, context, scope, resources)?;
        scratch.check().map_err(Error::Resource)?;
        held.check().map_err(Error::Resource)?;
        Ok(Self {
            features: set,
            root: root_copy,
            probes: owned_probes,
            context,
            scope,
            map,
            held,
        })
    }
    fn check(&self, resources: &ResourceReservationView<'_>) -> Result<(), Error> {
        check_view(resources, self.context)?;
        self.held.check().map_err(Error::Resource)?;
        if !self.held.same_reservation(resources) {
            return Err(Error::Binding);
        }
        Ok(())
    }
    pub fn scope(&self) -> ComparisonScope {
        self.scope
    }
}

#[derive(Clone, Copy)]
pub struct PathEdge<'a> {
    from: &'a [u8],
    to: &'a [u8],
    map: RegisteredMap,
}
impl<'a> PathEdge<'a> {
    pub fn new(from: &'a [u8], to: &'a [u8], map: RegisteredMap) -> Self {
        Self { from, to, map }
    }
}

pub struct Observation<'plan, 'view> {
    actual: OwnedReconstruction<'plan, 'view>,
    hard_defect: bool,
}

/// Validated endpoints of an actual byte edge, not a claimed semantic map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EdgeWitness {
    pub from: Digest32,
    pub to: Digest32,
    pub mapping: RegisteredMap,
}
impl Observation<'_, '_> {
    pub fn loss(&self) -> Option<&RationalInterval> {
        self.actual.historical_bounds()
    }
    pub fn gap(&self) -> Option<recon::Gap> {
        self.actual.historical_gap()
    }
}

/// Retains real producer results and the plan and lease they were produced under.
pub struct OwnedPathResult<'plan, 'view> {
    history: Vec<Observation<'plan, 'view>>,
    edges: Vec<EdgeWitness>,
    final_observation: Observation<'plan, 'view>,
    ledger: FacetLedger,
    final_bytes: Vec<u8>,
    plan: &'plan RootPathPlan<'view>,
    evaluations: usize,
    held: ResourceClaim<'view>,
}
impl OwnedPathResult<'_, '_> {
    pub fn history(&self) -> &[Observation<'_, '_>] {
        &self.history
    }
    pub fn edges(&self) -> &[EdgeWitness] {
        &self.edges
    }
    pub fn final_observation(&self) -> &Observation<'_, '_> {
        &self.final_observation
    }
    pub fn ledger(&self) -> &FacetLedger {
        &self.ledger
    }
    pub fn actual_evaluation_count(&self) -> usize {
        self.evaluations
    }
}

pub fn evaluate_path<'plan, 'view>(
    plan: &'plan RootPathPlan<'view>,
    edges: &[PathEdge<'_>],
    final_bytes: &[u8],
    resources: &'view ResourceReservationView<'_>,
) -> Result<OwnedPathResult<'plan, 'view>, Error> {
    plan.check(resources)?;
    if edges.len() > MAX_EDGES || final_bytes.len() > MAX_BYTES {
        return Err(Error::Budget);
    }
    let mut previous = plan.root.as_slice();
    for edge in edges {
        if !edge.map.supported() || edge.map != plan.map {
            return Err(Error::UnsupportedMap);
        }
        if edge.from.len() > MAX_BYTES || edge.to.len() > MAX_BYTES {
            return Err(Error::Budget);
        }
        if edge.from != previous {
            return Err(Error::BrokenPath);
        }
        previous = edge.to;
    }
    if previous != final_bytes {
        return Err(Error::BrokenPath);
    }
    let count = edges.len() + 1;
    let scratch = claim(resources, WRAPPER_RAM + PRODUCER_SCRATCH, false)?;
    let held = claim(
        resources,
        WRAPPER_RAM + PRODUCER_RETAINED * count as u64,
        true,
    )?;
    let mut history = Vec::new();
    history
        .try_reserve_exact(edges.len())
        .map_err(|_| Error::Budget)?;
    let mut final_copy = Vec::new();
    final_copy
        .try_reserve_exact(final_bytes.len())
        .map_err(|_| Error::Budget)?;
    final_copy.extend_from_slice(final_bytes);
    let mut edge_witnesses = Vec::new();
    edge_witnesses
        .try_reserve_exact(edges.len())
        .map_err(|_| Error::Budget)?;
    for edge in edges {
        edge_witnesses.push(EdgeWitness {
            from: hash(edge.from),
            to: hash(edge.to),
            mapping: edge.map,
        });
    }
    let mut ledger = FacetLedger::new(count, count * (plan.probes.len() + 1))?;
    let mut evaluations = 0;
    for (i, edge) in edges.iter().enumerate() {
        let obs = observe(plan, edge.to, resources, &mut ledger, i)?;
        if obs.loss().is_some() {
            evaluations += plan.probes.len();
        }
        history.push(obs);
    }
    // Always invoke a separate producer even when the last destination is identical.
    let final_observation = observe(plan, final_bytes, resources, &mut ledger, edges.len())?;
    if final_observation.loss().is_some() {
        evaluations += plan.probes.len();
    }
    scratch.check().map_err(Error::Resource)?;
    held.check().map_err(Error::Resource)?;
    Ok(OwnedPathResult {
        history,
        edges: edge_witnesses,
        final_observation,
        ledger,
        final_bytes: final_copy,
        plan,
        evaluations,
        held,
    })
}

fn observe<'plan, 'view>(
    plan: &'plan RootPathPlan<'view>,
    target: &[u8],
    resources: &'view ResourceReservationView<'_>,
    ledger: &mut FacetLedger,
    ordinal: usize,
) -> Result<Observation<'plan, 'view>, Error> {
    let actual = recon::produce(&plan.features, target, resources)?;
    let id = transform_id(ordinal);
    let mut temporary = FacetLedger::new(1, plan.probes.len() + 1)?;
    let original_binding = actual.binding(id);
    temporary.admit_transform(original_binding)?;
    actual.retain_current(
        &plan.features,
        target,
        plan.context,
        resources,
        &mut temporary,
        id,
        None,
    )?;
    let mut hard_defect = false;
    let mut checked = [false; MAX_PROBES];
    if matches!(
        actual.axis_current(&plan.features, target, plan.context, resources)?,
        AxisValue::Exact(_)
    ) {
        for (i, p) in plan.probes.iter().enumerate() {
            match actual.retain_current(
                &plan.features,
                target,
                plan.context,
                resources,
                &mut temporary,
                id,
                Some(i),
            ) {
                Ok(()) => {
                    checked[i] = true;
                    hard_defect |= p.hard;
                }
                Err(recon::Error::Binding) => {
                    actual.axis_current(&plan.features, target, plan.context, resources)?;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
    let mut binding = original_binding;
    binding.source = Revision(*hash(&plan.root).as_bytes());
    ledger.admit_transform(binding)?;
    for entry in temporary.entries() {
        if let Facet::Uncertainty(gap) = entry.facet {
            ledger.append(Entry {
                transform: id,
                facet: Facet::Uncertainty(gap),
            })?;
        }
    }
    for (i, p) in plan.probes.iter().enumerate() {
        if checked[i] {
            ledger.append(Entry {
                transform: id,
                facet: Facet::Defect(Defect {
                    key: DefectKey {
                        root: p.obligation,
                        property: p.property,
                        source: binding.source,
                        target: binding.target,
                    },
                    axis: LossAxis::Reconstruction,
                    witness: EvidenceRef(binding.result.0),
                }),
            })?;
        }
    }
    Ok(Observation {
        actual,
        hard_defect,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootGate {
    PassInGivenFiniteProfile,
    FailInGivenFiniteProfile,
    NeedsEvidence,
}

pub fn consume_root_gate(
    result: Option<&OwnedPathResult<'_, '_>>,
    plan: &RootPathPlan<'_>,
    final_bytes: &[u8],
    resources: &ResourceReservationView<'_>,
) -> Result<RootGate, Error> {
    plan.check(resources)?;
    let Some(result) = result else {
        return Ok(RootGate::NeedsEvidence);
    };
    result.held.check().map_err(Error::Resource)?;
    if !result.held.same_reservation(resources)
        || !std::ptr::eq(result.plan, plan)
        || result.final_bytes != final_bytes
    {
        return Err(Error::Binding);
    }
    match result.final_observation.actual.axis_current(
        &plan.features,
        final_bytes,
        plan.context,
        resources,
    )? {
        AxisValue::Exact(_) => Ok(if result.final_observation.hard_defect {
            RootGate::FailInGivenFiniteProfile
        } else {
            RootGate::PassInGivenFiniteProfile
        }),
        _ => Ok(RootGate::NeedsEvidence),
    }
}

fn check_view(
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
    ram_bytes: u64,
    output: bool,
) -> Result<ResourceClaim<'a>, Error> {
    let amount = ResourceAmount::checked(ram_bytes).map_err(Error::Resource)?;
    let ram = ResourceRequest::new(ResourceKind::Ram, amount);
    if output {
        resources
            .claim_resources(&[ram, ResourceRequest::new(ResourceKind::Output, amount)])
            .map_err(Error::Resource)
    } else {
        resources.claim_resources(&[ram]).map_err(Error::Resource)
    }
}
fn hash(bytes: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(bytes).into())
}
fn transform_id(ordinal: usize) -> TransformId {
    let mut h = Sha256::new();
    h.update(b"tl-path-observation-v1");
    h.update((ordinal as u64).to_le_bytes());
    TransformId(h.finalize().into())
}
