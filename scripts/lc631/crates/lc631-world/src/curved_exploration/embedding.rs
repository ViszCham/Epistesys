//! 日本語: Given遷移graphからの限定1D座標。算術検査は意味・真正性・権限を認証しない。
//! English: Bounded coordinates from Given transitions; checks certify profile arithmetic only.
#![forbid(unsafe_code)]
use super::arithmetic::{one, point, Interval, NumericError, Rational};
use super::graph::{BaselineDistance, GraphError, NodeId, ReportedTransition, TransitionGraph};
use super::{prepare_poincare, BallPolicy, GeometryError, PreparedPoincare};
use lc631_core::measurement_numeric::{
    checked_add, checked_compare, checked_divide, checked_multiply, checked_subtract,
};
use std::cmp::Ordering;

const MAX_NODES: usize = 128;
const MAX_EDGES: usize = 512;
const MAX_HELDOUT: usize = 256;
const MAX_PIN_HEX: usize = 1_048_576;
pub const DEFINITION: &str = "RootedShortestLayerRadialV1/closed-form-v1";
pub const OBJECTIVE: &str = "preserve shortest-root-hop layer order; same-layer collapse allowed";
pub const INITIALIZATION: &str = "given root at zero";
pub const OPTIMIZER: &str = "RootedClosedFormV1; no stochastic optimizer; seed=0 unused";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Algorithm {
    RootedShortestLayerRadialV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MappingBasis {
    GivenCanonicalContent,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceStatus {
    ReportedUnauthenticated,
}
#[derive(Clone, Copy, Debug)]
pub struct NodeBinding<'a> {
    pub id: NodeId,
    pub canonical_content: &'a [u8],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayerRelation {
    Less,
    Equal,
    Greater,
}
impl LayerRelation {
    fn ordering(self) -> Ordering {
        match self {
            Self::Less => Ordering::Less,
            Self::Equal => Ordering::Equal,
            Self::Greater => Ordering::Greater,
        }
    }
    fn tag(self) -> u8 {
        match self {
            Self::Less => 0,
            Self::Equal => 1,
            Self::Greater => 2,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct HeldoutRelation {
    pub from: NodeId,
    pub to: NodeId,
    pub expected: LayerRelation,
    pub evidence_payload_digest: [u8; 32],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddingError {
    Limit,
    UnknownNode,
    DuplicateContent,
    InvalidHeldout,
    DuplicateHeldoutPair,
    HeldoutTrainingOverlap,
    MissingEvidence,
    AllocationFailed,
    Graph(GraphError),
    Numeric(NumericError),
    Geometry(GeometryError),
}
impl From<NumericError> for EmbeddingError {
    fn from(e: NumericError) -> Self {
        Self::Numeric(e)
    }
}
impl From<GraphError> for EmbeddingError {
    fn from(e: GraphError) -> Self {
        Self::Graph(e)
    }
}
impl From<GeometryError> for EmbeddingError {
    fn from(e: GeometryError) -> Self {
        Self::Geometry(e)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingReason {
    MissingHeldoutChecks,
    UnsupportedCycle,
    UnsupportedDisconnected,
    WorkBudget,
    HeldoutFailed { checked: usize, failed: usize },
}
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum EmbeddingAttempt<'a> {
    Checked(CheckedEmbedding<'a>),
    Pending(PendingReason),
}

#[derive(Clone, Debug)]
pub struct Registration {
    root: NodeId,
    ball: BallPolicy,
    feature_revision: [u8; 32],
    heldout_revision: [u8; 32],
    work_limit: u64,
}
impl Registration {
    pub fn checked(
        root: NodeId,
        ball: BallPolicy,
        revisions: ([u8; 32], [u8; 32]),
        work_limit: u64,
    ) -> Result<Self, EmbeddingError> {
        if revisions.0 == [0; 32] || revisions.1 == [0; 32] {
            return Err(EmbeddingError::MissingEvidence);
        }
        if work_limit == 0 || work_limit > 1_000_000 {
            return Err(EmbeddingError::Limit);
        }
        Ok(Self {
            root,
            ball,
            feature_revision: revisions.0,
            heldout_revision: revisions.1,
            work_limit,
        })
    }
    pub fn dimension(&self) -> usize {
        1
    }
    pub fn seed(&self) -> u64 {
        0
    }
    pub fn algorithm(&self) -> Algorithm {
        Algorithm::RootedShortestLayerRadialV1
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddingPins {
    pub graph_snapshot: [u8; 32],
    pub edge_evidence: [u8; 32],
    pub feature_correspondence: [u8; 32],
    pub policy: [u8; 32],
    pub coordinates: [u8; 32],
    pub heldout_checks: [u8; 32],
}
#[derive(Debug)]
pub struct EmbeddedNode<'a> {
    id: NodeId,
    content: &'a [u8],
    depth: u32,
    coordinate: Interval,
}
impl EmbeddedNode<'_> {
    pub fn id(&self) -> NodeId {
        self.id
    }
    pub fn canonical_content(&self) -> &[u8] {
        self.content
    }
    pub fn coordinate(&self) -> &Interval {
        &self.coordinate
    }
}
/// Private construction checks graph, ball margin and Given heldout relations only.
#[derive(Debug)]
pub struct CheckedEmbedding<'a> {
    nodes: Vec<EmbeddedNode<'a>>,
    ball: BallPolicy,
    pins: EmbeddingPins,
    heldout_count: usize,
}
impl<'a> CheckedEmbedding<'a> {
    pub fn nodes(&self) -> &[EmbeddedNode<'a>] {
        &self.nodes
    }
    pub fn pins(&self) -> EmbeddingPins {
        self.pins
    }
    pub fn ball_policy(&self) -> &BallPolicy {
        &self.ball
    }
    pub fn heldout_count(&self) -> usize {
        self.heldout_count
    }
    pub fn mapping_basis(&self) -> MappingBasis {
        MappingBasis::GivenCanonicalContent
    }
    pub fn evidence_status(&self) -> EvidenceStatus {
        EvidenceStatus::ReportedUnauthenticated
    }
    pub fn coordinate(&self, id: NodeId) -> Option<&Interval> {
        self.nodes
            .iter()
            .find(|n| n.id == id)
            .map(|n| &n.coordinate)
    }
    pub fn prepare_distance(
        &self,
        a: NodeId,
        b: NodeId,
    ) -> Result<PreparedPoincare, EmbeddingError> {
        let a = self.coordinate(a).ok_or(EmbeddingError::UnknownNode)?;
        let b = self.coordinate(b).ok_or(EmbeddingError::UnknownNode)?;
        Ok(prepare_poincare(
            &self.ball,
            std::slice::from_ref(a),
            std::slice::from_ref(b),
        )?)
    }
}

pub fn produce<'a>(
    registration: &Registration,
    nodes: &[NodeBinding<'a>],
    edges: &[ReportedTransition],
    heldout: &[HeldoutRelation],
) -> Result<EmbeddingAttempt<'a>, EmbeddingError> {
    if nodes.is_empty()
        || nodes.len() > MAX_NODES
        || edges.len() > MAX_EDGES
        || heldout.len() > MAX_HELDOUT
    {
        return Err(EmbeddingError::Limit);
    }
    let sorted = sorted_nodes(nodes)?;
    content(&sorted, registration.root)?;
    validate_heldout(&sorted, edges, heldout)?;
    if heldout.is_empty() {
        return Ok(EmbeddingAttempt::Pending(
            PendingReason::MissingHeldoutChecks,
        ));
    }
    let breadth = (nodes.len() + edges.len()) as u64;
    if breadth.checked_mul(breadth).ok_or(EmbeddingError::Limit)? > registration.work_limit {
        return Ok(EmbeddingAttempt::Pending(PendingReason::WorkBudget));
    }
    let mut ids = reserved(nodes.len())?;
    ids.extend(nodes.iter().map(|n| n.id));
    let graph = TransitionGraph::new(&ids, edges)?;
    for edge in edges {
        if matches!(
            graph.baseline_hops(edge.to, edge.from)?,
            BaselineDistance::ObservedHops(_)
        ) {
            return Ok(EmbeddingAttempt::Pending(PendingReason::UnsupportedCycle));
        }
    }
    let mut embedded = reserved(nodes.len())?;
    let radius = checked_divide(
        &checked_subtract(&one(), &registration.ball.margin)?,
        &checked_multiply(
            &Rational::from_integer(2.into()),
            &checked_add(&one(), &registration.ball.curvature)?,
        )?,
    )?;
    for node in &sorted {
        let depth = match graph.baseline_hops(registration.root, node.id)? {
            BaselineDistance::ObservedHops(depth) => depth,
            BaselineDistance::UnknownNoObservedPath => {
                return Ok(EmbeddingAttempt::Pending(
                    PendingReason::UnsupportedDisconnected,
                ))
            }
        };
        let ratio = Rational::new(depth.into(), (depth + 1).into());
        let coordinate = point(checked_multiply(&radius, &ratio)?)?;
        registration
            .ball
            .validate_box(std::slice::from_ref(&coordinate))?;
        embedded.push(EmbeddedNode {
            id: node.id,
            content: node.canonical_content,
            depth,
            coordinate,
        });
    }
    let mut failed = 0;
    for check in heldout {
        let a = embedded
            .iter()
            .find(|n| n.id == check.from)
            .ok_or(EmbeddingError::UnknownNode)?;
        let b = embedded
            .iter()
            .find(|n| n.id == check.to)
            .ok_or(EmbeddingError::UnknownNode)?;
        if checked_compare(a.coordinate.lower(), b.coordinate.lower())? != check.expected.ordering()
        {
            failed += 1;
        }
    }
    if failed > 0 {
        return Ok(EmbeddingAttempt::Pending(PendingReason::HeldoutFailed {
            checked: heldout.len(),
            failed,
        }));
    }
    let pins = make_pins(registration, &sorted, edges, heldout, &embedded)?;
    Ok(EmbeddingAttempt::Checked(CheckedEmbedding {
        nodes: embedded,
        ball: registration.ball.clone(),
        pins,
        heldout_count: heldout.len(),
    }))
}

fn sorted_nodes<'a, 'b>(
    nodes: &'b [NodeBinding<'a>],
) -> Result<Vec<&'b NodeBinding<'a>>, EmbeddingError> {
    let mut sorted = reserved(nodes.len())?;
    for node in nodes {
        if node.canonical_content.is_empty() || node.canonical_content.len() > 256 {
            return Err(EmbeddingError::Limit);
        }
        sorted.push(node);
    }
    sorted.sort_unstable_by_key(|node| node.canonical_content);
    if sorted
        .windows(2)
        .any(|p| p[0].canonical_content == p[1].canonical_content)
    {
        return Err(EmbeddingError::DuplicateContent);
    }
    Ok(sorted)
}
fn content<'a>(nodes: &[&NodeBinding<'a>], id: NodeId) -> Result<&'a [u8], EmbeddingError> {
    nodes
        .iter()
        .find(|n| n.id == id)
        .map(|n| n.canonical_content)
        .ok_or(EmbeddingError::UnknownNode)
}
fn same_pair(a: NodeId, b: NodeId, c: NodeId, d: NodeId) -> bool {
    (a == c && b == d) || (a == d && b == c)
}
fn validate_heldout(
    nodes: &[&NodeBinding<'_>],
    edges: &[ReportedTransition],
    heldout: &[HeldoutRelation],
) -> Result<(), EmbeddingError> {
    for (i, check) in heldout.iter().enumerate() {
        content(nodes, check.from)?;
        content(nodes, check.to)?;
        if check.from == check.to {
            return Err(EmbeddingError::InvalidHeldout);
        }
        if check.evidence_payload_digest == [0; 32] {
            return Err(EmbeddingError::MissingEvidence);
        }
        if edges
            .iter()
            .any(|e| same_pair(e.from, e.to, check.from, check.to))
        {
            return Err(EmbeddingError::HeldoutTrainingOverlap);
        }
        if heldout[..i]
            .iter()
            .any(|p| same_pair(p.from, p.to, check.from, check.to))
        {
            return Err(EmbeddingError::DuplicateHeldoutPair);
        }
    }
    Ok(())
}
fn reserved<T>(n: usize) -> Result<Vec<T>, EmbeddingError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(n)
        .map_err(|_| EmbeddingError::AllocationFailed)?;
    Ok(values)
}

struct PinText {
    text: String,
}
impl PinText {
    fn new(domain: &[u8]) -> Result<Self, EmbeddingError> {
        let mut value = Self {
            text: String::new(),
        };
        value.field(domain)?;
        Ok(value)
    }
    fn raw(&mut self, bytes: &[u8]) -> Result<(), EmbeddingError> {
        let extra = bytes.len().checked_mul(2).ok_or(EmbeddingError::Limit)?;
        if self
            .text
            .len()
            .checked_add(extra)
            .ok_or(EmbeddingError::Limit)?
            > MAX_PIN_HEX
        {
            return Err(EmbeddingError::Limit);
        }
        self.text
            .try_reserve(extra)
            .map_err(|_| EmbeddingError::AllocationFailed)?;
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in bytes {
            self.text.push(char::from(HEX[usize::from(byte >> 4)]));
            self.text.push(char::from(HEX[usize::from(byte & 15)]));
        }
        Ok(())
    }
    fn field(&mut self, bytes: &[u8]) -> Result<(), EmbeddingError> {
        self.raw(&(bytes.len() as u64).to_le_bytes())?;
        self.raw(bytes)
    }
    fn rational(&mut self, value: &Rational) -> Result<(), EmbeddingError> {
        lc631_core::measurement_numeric::check(value)?;
        self.field(&value.numer().to_signed_bytes_le())?;
        self.field(&value.denom().to_signed_bytes_le())
    }
    fn finish(&self) -> [u8; 32] {
        *lc631_core::context::SourceRevision::of(&self.text)
            .digest()
            .as_bytes()
    }
}

fn make_pins(
    reg: &Registration,
    nodes: &[&NodeBinding<'_>],
    edges: &[ReportedTransition],
    heldout: &[HeldoutRelation],
    coordinates: &[EmbeddedNode<'_>],
) -> Result<EmbeddingPins, EmbeddingError> {
    let mut features = PinText::new(b"embedding.features.v1")?;
    features.field(&reg.feature_revision)?;
    for node in nodes {
        features.field(node.canonical_content)?;
    }
    let mut indexed = reserved(edges.len())?;
    for edge in edges {
        indexed.push((content(nodes, edge.from)?, content(nodes, edge.to)?, edge));
    }
    indexed.sort_unstable_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    let mut graph = PinText::new(b"embedding.graph.v1")?;
    let feature_pin = features.finish();
    graph.field(&feature_pin)?;
    let mut evidence = PinText::new(b"embedding.reported-edge-evidence.v1")?;
    for (from, to, edge) in indexed {
        graph.field(from)?;
        graph.field(to)?;
        graph.field(&edge.cost.slots().to_le_bytes())?;
        graph.field(&edge.evidence_payload_digest)?;
        evidence.field(from)?;
        evidence.field(to)?;
        evidence.field(&edge.evidence_payload_digest)?;
    }
    let mut positions = PinText::new(b"embedding.coordinates.v1")?;
    for node in coordinates {
        positions.field(node.content)?;
        positions.field(&node.depth.to_le_bytes())?;
        positions.rational(node.coordinate.lower())?;
    }
    let mut checks = reserved(heldout.len())?;
    for check in heldout {
        checks.push((
            content(nodes, check.from)?,
            content(nodes, check.to)?,
            check,
        ));
    }
    checks.sort_unstable_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    let mut checked = PinText::new(b"embedding.checked-heldout.v1")?;
    checked.field(&reg.heldout_revision)?;
    for (from, to, check) in checks {
        checked.field(from)?;
        checked.field(to)?;
        checked.field(&[check.expected.tag()])?;
        checked.field(&check.evidence_payload_digest)?;
        checked.field(&[check.expected.tag()])?;
    }
    let graph_pin = graph.finish();
    let evidence_pin = evidence.finish();
    let coordinate_pin = positions.finish();
    let check_pin = checked.finish();
    let mut policy = PinText::new(b"embedding.policy.v1")?;
    for bytes in [
        DEFINITION.as_bytes(),
        OBJECTIVE.as_bytes(),
        INITIALIZATION.as_bytes(),
        OPTIMIZER.as_bytes(),
    ] {
        policy.field(bytes)?;
    }
    for pin in [
        &graph_pin,
        &evidence_pin,
        &feature_pin,
        &coordinate_pin,
        &check_pin,
    ] {
        policy.field(pin)?;
    }
    policy.field(content(nodes, reg.root)?)?;
    policy.field(&1u64.to_le_bytes())?;
    policy.field(&0u64.to_le_bytes())?;
    policy.field(&reg.work_limit.to_le_bytes())?;
    policy.field(&(reg.ball.dimension_limit as u64).to_le_bytes())?;
    for value in [
        &reg.ball.curvature,
        &reg.ball.margin,
        &reg.ball.novelty_scale,
    ] {
        policy.rational(value)?;
    }
    Ok(EmbeddingPins {
        graph_snapshot: graph_pin,
        edge_evidence: evidence_pin,
        feature_correspondence: feature_pin,
        policy: policy.finish(),
        coordinates: coordinate_pin,
        heldout_checks: check_pin,
    })
}
