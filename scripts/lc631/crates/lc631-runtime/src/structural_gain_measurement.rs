//! 日本語: Given閉有限有向graphの実edge照合からCR-PRESERVEを測る。
//! English: Measure CR-PRESERVE by actual edge queries on Given closed finite directed graphs.
//! Declared correspondence is not source interpretation, independent evidence or authority.
#![forbid(unsafe_code)]
use lc631_analysis::creativity_measurement::{
    weighted_assessment, ComponentMetric, ComponentReport, ComponentState, IndependenceStatus,
    InputBasis, MeasurementBinding, MeasurementError, MeasurementGap, Rational, RationalInterval,
    SemanticValidation, WeightedProbe,
};
use lc631_core::{
    context::{ContextRevision, ContextScope},
    CandidateId,
};
use lc631_substrate::{
    measurement_numeric::BigInt,
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    source::SourceStore,
    Digest32,
};
use sha2::{Digest, Sha256};

mod selection;
pub mod owned_graph_join;
pub use selection::{select_current, CurrentSelection, PreservationRow};
pub const METRIC: ComponentMetric = ComponentMetric::Preserve;
pub const MAX_PROBES: usize = 64;
pub const OWNER_RAM: u64 = 128 * 1024;
pub const SCRATCH_RAM: u64 = 256 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidGraph,
    InvalidPlan,
    SourceMismatch,
    BindingMismatch,
    ForeignReservation,
    AllocationFailed,
    Resource(ResourceError),
    Measurement(MeasurementError),
    Selection(crate::joint_selection::SelectionError),
}
/// A local vertex number has no inferred semantic meaning. Its correspondence
/// is supplied explicitly by the immutable plan, never by ordering/ID proximity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Vertex(pub u16);
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Edge(pub Vertex, pub Vertex);

/// Immutable complete edge registry, not a caller-provided pass/fail table.
/// The source/graph interpretation remains Given. Sorted unique edges make
/// graph identity independent of acquisition order without hidden allocation.
pub struct ClosedGraph<'a> {
    source: &'a SourceStore,
    context: ContextRevision,
    nodes: u16,
    edges: &'a [Edge],
    digest: Digest32,
}
impl<'a> ClosedGraph<'a> {
    pub fn checked(
        source: &'a SourceStore,
        context: ContextRevision,
        nodes: u16,
        edges: &'a [Edge],
    ) -> Result<Self, Error> {
        if source.id().digest() != context.source().digest() {
            return Err(Error::SourceMismatch);
        }
        if nodes == 0
            || nodes > 64
            || edges.len() > 4096
            || edges.iter().any(|e| e.0 .0 >= nodes || e.1 .0 >= nodes)
            || edges.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(Error::InvalidGraph);
        }
        let mut hash = Sha256::new();
        hash.update(b"epistesys.given-closed-directed-graph.v1\0");
        hash.update(source.id().digest().as_bytes());
        hash.update(context.interpretation_digest().as_bytes());
        hash.update(nodes.to_le_bytes());
        hash.update((edges.len() as u16).to_le_bytes());
        for edge in edges {
            hash.update(edge.0 .0.to_le_bytes());
            hash.update(edge.1 .0.to_le_bytes());
        }
        Ok(Self {
            source,
            context,
            nodes,
            edges,
            digest: Digest32::from_bytes(hash.finalize().into()),
        })
    }
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }
    pub const fn context(&self) -> ContextRevision {
        self.context
    }
    fn contains(&self, edge: Edge) -> bool {
        self.edges.binary_search(&edge).is_ok()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedEdge {
    pub edge: Edge,
    pub weight: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Correspondence {
    pub source: Vertex,
    pub target: Vertex,
}
/// Fixed before candidate assessment; no candidate-specific probe dropping.
pub struct PreservationPlan<'a> {
    source: &'a ClosedGraph<'a>,
    scope: ContextScope,
    target_nodes: u16,
    probes: &'a [ProtectedEdge],
    mapping: Option<&'a [Correspondence]>,
    complete_mapping: bool,
    denominator: u64,
    digest: Digest32,
}
impl<'a> PreservationPlan<'a> {
    pub fn declared(
        source: &'a ClosedGraph<'a>,
        scope: ContextScope,
        target_nodes: u16,
        probes: &'a [ProtectedEdge],
        mapping: Option<&'a [Correspondence]>,
    ) -> Result<Self, Error> {
        if target_nodes == 0 || target_nodes > 64 || probes.is_empty() || probes.len() > MAX_PROBES
        {
            return Err(Error::InvalidPlan);
        }
        let mut denominator = 0_u64;
        for (i, probe) in probes.iter().enumerate() {
            if probe.weight == 0
                || probe.weight > 1_000_000
                || !source.contains(probe.edge)
                || probes[..i].iter().any(|p| p.edge == probe.edge)
            {
                return Err(Error::InvalidPlan);
            }
            denominator = denominator
                .checked_add(u64::from(probe.weight))
                .ok_or(Error::InvalidPlan)?;
        }
        if let Some(map) = mapping {
            if map.len() > 64 {
                return Err(Error::InvalidPlan);
            }
            for (i, pair) in map.iter().enumerate() {
                if pair.source.0 >= source.nodes
                    || pair.target.0 >= target_nodes
                    || map[..i]
                        .iter()
                        .any(|p| p.source == pair.source || p.target == pair.target)
                {
                    return Err(Error::InvalidPlan);
                }
            }
        }
        let complete_mapping = mapping.is_some_and(|map| map.len() == usize::from(source.nodes));
        let mut hash = Sha256::new();
        hash.update(b"epistesys.cr-preserve.fixed-edge-plan.v1\0");
        for digest in [
            source.digest,
            scope.comparison(),
            scope.goal(),
            scope.probes(),
            scope.semantics(),
        ] {
            hash.update(digest.as_bytes());
        }
        hash.update(target_nodes.to_le_bytes());
        hash.update(denominator.to_le_bytes());
        for probe in probes {
            hash.update(probe.edge.0 .0.to_le_bytes());
            hash.update(probe.edge.1 .0.to_le_bytes());
            hash.update(probe.weight.to_le_bytes());
        }
        // Canonical source-vertex order, not caller map-array order.
        match mapping {
            None => hash.update([0]),
            Some(map) => {
                hash.update([1]);
                for n in 0..source.nodes {
                    match map.iter().find(|p| p.source == Vertex(n)) {
                        None => hash.update([0]),
                        Some(pair) => {
                            hash.update([1]);
                            hash.update(pair.target.0.to_le_bytes());
                        }
                    }
                }
            }
        }
        Ok(Self {
            source,
            scope,
            target_nodes,
            probes,
            mapping,
            complete_mapping,
            denominator,
            digest: Digest32::from_bytes(hash.finalize().into()),
        })
    }
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }
    pub const fn denominator(&self) -> u64 {
        self.denominator
    }
    fn mapped(&self, edge: Edge) -> Result<Edge, Error> {
        let map = self.mapping.ok_or(Error::InvalidPlan)?;
        let a = map
            .iter()
            .find(|p| p.source == edge.0)
            .ok_or(Error::InvalidPlan)?;
        let b = map
            .iter()
            .find(|p| p.source == edge.1)
            .ok_or(Error::InvalidPlan)?;
        Ok(Edge(a.target, b.target))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateBinding {
    pub id: CandidateId,
    pub identity: Digest32,
    pub context: ContextRevision,
    pub pins: crate::joint_selection::SelectionPins,
}
/// Private fresh producer owns actual query outcomes and immutable backing.
/// No detached report/status insertion, Clone, Deref or Deserialize exists.
///
/// ```compile_fail
/// use lc631_runtime::structural_gain_measurement::OwnedPreservation;
/// use lc631_analysis::creativity_measurement::ComponentReport;
/// fn inject(report: ComponentReport) { OwnedPreservation::from_report(report); }
/// ```
pub struct OwnedPreservation<'input, 'view> {
    plan: &'input PreservationPlan<'input>,
    target: Option<&'input ClosedGraph<'input>>,
    binding: CandidateBinding,
    outcomes: Vec<Option<bool>>,
    report: ComponentReport,
    retained: ResourceClaim<'view>,
}
impl<'input, 'view> OwnedPreservation<'input, 'view> {
    /// Missing target is the explicitly registered missing-observation model:
    /// known fixed binary edge queries, no target edge observations. It yields
    /// [0,1], never zero; incomplete/unregistered maps instead remain NoScalar.
    pub fn produce(
        plan: &'input PreservationPlan<'input>,
        target: Option<&'input ClosedGraph<'input>>,
        binding: CandidateBinding,
        resources: &'view ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        resources.check().map_err(Error::Resource)?;
        if binding.context.source() != binding.pins.source
            || plan.source.source.id().digest() != resources.source_revision()
            || binding.pins.source.digest() != resources.source_revision()
            || plan.scope != binding.pins.scope
            || target.is_some_and(|t| {
                t.context != binding.context
                    || t.nodes != plan.target_nodes
                    || t.source.id().digest() != resources.source_revision()
            })
        {
            return Err(Error::BindingMismatch);
        }
        let retained = claim(resources, OWNER_RAM)?;
        let scratch = claim(resources, SCRATCH_RAM)?;
        let mut outcomes = vector(plan.probes.len())?;
        let state = if !plan.complete_mapping {
            ComponentState::Unknown(MeasurementGap::Unsupported)
        } else {
            for probe in plan.probes {
                let mapped = plan.mapped(probe.edge)?;
                outcomes.push(target.map(|graph| graph.contains(mapped)));
            }
            ComponentState::Measured(aggregate(plan, &outcomes)?)
        };
        let mut hash = Sha256::new();
        hash.update(b"epistesys.cr-preserve.actual-edge-results.v1\0");
        hash.update(plan.digest.as_bytes());
        for value in &outcomes {
            hash.update([match value {
                None => 0,
                Some(false) => 1,
                Some(true) => 2,
            }]);
        }
        hash.update([u8::from(plan.complete_mapping)]);
        let result = Digest32::from_bytes(hash.finalize().into());
        let definition = revision(
            b"OperationalMeasurement/finite-formal-v1.4\0CR-PRESERVE\0closed-directed-edges-v1",
        );
        let report = ComponentReport {
            metric: METRIC,
            definition,
            binding: MeasurementBinding {
                scope: plan.scope.comparison(),
                candidate: binding.identity,
                source: plan.source.digest,
                target: target.map_or_else(
                    || revision(b"missing-target-observation-v1"),
                    ClosedGraph::digest,
                ),
                context: binding.context.interpretation_digest(),
                baseline: binding.pins.baseline,
                producer: revision(b"actual-closed-edge-membership-v1"),
                transform: plan.digest,
                result,
            },
            probe_set: plan.scope.probes(),
            oracle: revision(b"closed-directed-edge-membership-binary-search-v1"),
            dependency_group: revision(b"Given-graph-and-declared-map-shared-dependency-v1"),
            evidence_payload: result,
            basis: InputBasis::Given,
            validation: SemanticValidation::Unassessed,
            independence: IndependenceStatus::SharedDependency,
            state,
        };
        scratch.check().map_err(Error::Resource)?;
        retained.check().map_err(Error::Resource)?;
        Ok(Self {
            plan,
            target,
            binding,
            outcomes,
            report,
            retained,
        })
    }
    pub fn historical_report(&self) -> &ComponentReport {
        &self.report
    }
    pub fn report_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, Error> {
        self.check_current(resources)?;
        Ok(&self.report)
    }
    /// Match every declared selection pin while retaining the actual owner.
    /// Pin equality is not semantic validation or authenticated evidence.
    pub fn report_for_pins_current(
        &self,
        pins: crate::joint_selection::SelectionPins,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, Error> {
        self.check_current(resources)?;
        if self.binding.pins != pins {
            return Err(Error::BindingMismatch);
        }
        Ok(&self.report)
    }
    pub fn axis_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<crate::joint_selection::ReportedComponentAxis<'_>, Error> {
        self.check_current(resources)?;
        crate::joint_selection::project_component_reported(&self.report).map_err(Error::Selection)
    }
    pub fn measurement_binding(&self) -> Digest32 {
        self.report.binding.result
    }
    fn check_current(&self, resources: &ResourceReservationView<'_>) -> Result<(), Error> {
        resources.check().map_err(Error::Resource)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(resources) {
            return Err(Error::ForeignReservation);
        }
        if self.binding.pins.source.digest() != resources.source_revision()
            || self.binding.pins.scope != self.plan.scope
            || (self.plan.complete_mapping && self.outcomes.len() != self.plan.probes.len())
            || self
                .target
                .is_some_and(|t| t.context != self.binding.context)
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
}
fn aggregate(
    plan: &PreservationPlan<'_>,
    outcomes: &[Option<bool>],
) -> Result<RationalInterval, Error> {
    let mut values = vector(outcomes.len())?;
    let mut weights = vector(outcomes.len())?;
    for (probe, outcome) in plan.probes.iter().zip(outcomes) {
        values.push(
            match outcome {
                Some(true) => RationalInterval::exact_ratio(1, 1),
                Some(false) => RationalInterval::exact_ratio(0, 1),
                None => RationalInterval::from_ratio(0, 1, 1, 1),
            }
            .map_err(|e| Error::Measurement(e.into()))?,
        );
        weights.push(Rational::from_integer(BigInt::from(probe.weight)));
    }
    let mut keys = vector(plan.probes.len())?;
    for probe in plan.probes {
        let a = probe.edge.0 .0.to_le_bytes();
        let b = probe.edge.1 .0.to_le_bytes();
        keys.push([a[0], a[1], b[0], b[1]]);
    }
    let mut views = vector(plan.probes.len())?;
    for ((value, weight), key) in values.iter().zip(&weights).zip(&keys) {
        views.push(WeightedProbe {
            canonical_payload: key,
            weight,
            value,
        });
    }
    weighted_assessment(&views).map_err(Error::Measurement)
}
fn revision(value: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(value).into())
}
fn claim<'v>(
    resources: &'v ResourceReservationView<'_>,
    bytes: u64,
) -> Result<ResourceClaim<'v>, Error> {
    resources
        .claim_resources(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(bytes).map_err(Error::Resource)?,
        )])
        .map_err(Error::Resource)
}
fn vector<T>(capacity: usize) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| Error::AllocationFailed)?;
    if values.capacity() > capacity {
        return Err(Error::AllocationFailed);
    }
    Ok(values)
}
