//! 日本語: Given閉単一時点Boolean instanceを既存SMT model checkerへ限定接続する。
//! English: Connect one Given closed Boolean point to the existing SMT model checker.
//! This is trace-instance compatibility, not Temporal/SMT theory equivalence,
//! a native SMT process receipt, natural-language truth, proof or authority.
#![forbid(unsafe_code)]
use crate::{
    finite_results::{FiniteResultError, FiniteResultId, FiniteResultStore},
    formal_boolean::{CheckerIndependence, MappingBasis, SourceInterpretation},
    formal_measurement::{Candidate, ProbePlan},
    materialized_world_bridge::{CurrentObservedTemporalWorld, SourceMeaning, WorldBridgeError},
};
use lc631_solvers::{
    protocol::sexpr::SExprError,
    smt::{CheckedModel, ModelValidationError},
    temporal::{FiniteTrace, TemporalEvaluation, TemporalQuery, Truth},
    typed_query::BoolSymbolId,
};
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    source::{SourceError, SourceSpan, SourceStore},
};
mod projection;
#[cfg(feature = "semantic-gpu")]
pub mod world_port;

pub const MAX_ATOMS: usize = 8;
pub const MAX_TEMPORAL_NODES: usize = 64;
pub const MAX_DEPTH: usize = 32;
pub const RETAINED_RAM: u64 = 64 * 1024;
pub const SCRATCH_RAM: u64 = 64 * 1024;
pub const MAX_MODEL_BYTES: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedPoint {
    MultipleSteps,
    IncompleteMap,
    AtomLimit,
    TemporalNodeLimit,
    TemporalOperator,
    InexactClock,
    TheoryFragment,
    DepthLimit,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingPoint {
    UnknownAtom(u16),
    NativeUnknown,
    MissingTheoryScope,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PointError {
    InvalidBudget,
    WorkBudget,
    AllocationFailed,
    ModelEncoding,
    InvalidMap,
    SourceBinding,
    ScopeBinding,
    MissingProbe,
    PredicateMismatch,
    ForeignPlan,
    ForeignCandidate,
    ForeignObservation,
    Unsupported(UnsupportedPoint),
    NeedsEvidence(MissingPoint),
    Resource(ResourceError),
    Store(FiniteResultError),
    World(WorldBridgeError),
    Source(SourceError),
    ModelSyntax(SExprError),
    NotModel(ModelValidationError),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointBudget {
    max_ast_visits: u64,
}
impl PointBudget {
    pub fn checked(max_ast_visits: u64) -> Result<Self, PointError> {
        if max_ast_visits == 0 || max_ast_visits > 1_000_000 {
            return Err(PointError::InvalidBudget);
        }
        Ok(Self { max_ast_visits })
    }
}
/// Explicit Given interpretation. Equal numeric IDs do not create this map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomPair {
    pub temporal: u16,
    pub formal: BoolSymbolId,
}
#[derive(Clone, Copy)]
pub struct NativePoint<'owner, 'input, 'view> {
    pub store: &'owner FiniteResultStore<'input, 'view>,
    pub id: FiniteResultId,
    pub query: &'input TemporalQuery,
    pub trace: &'input FiniteTrace,
    pub scope: lc631_core::context::ContextScope,
}
/// Inputs only, never supplied success flags, model values or detached reports.
#[derive(Clone, Copy)]
pub struct PointRequest<'owner, 'input, 'view> {
    pub source: &'input SourceStore,
    pub span: SourceSpan,
    pub plan: &'input ProbePlan<'input>,
    pub candidate: &'input Candidate<'input>,
    pub probe: usize,
    pub native: NativePoint<'owner, 'input, 'view>,
    pub map: &'input [AtomPair],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointKind {
    GivenClosedPointBooleanInstance,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointValidation {
    TraceAssignedLocalModelRechecked,
}
/// Private construction, no generic semantic-equivalence or eligibility bool.
pub struct PointObservation {
    truth: Truth,
    assertions: usize,
    visits: u64,
}
impl PointObservation {
    pub const fn native_truth(&self) -> Truth {
        self.truth
    }
    pub const fn assertions_checked(&self) -> usize {
        self.assertions
    }
    pub const fn ast_visits(&self) -> u64 {
        self.visits
    }
    pub const fn kind(&self) -> PointKind {
        PointKind::GivenClosedPointBooleanInstance
    }
    pub const fn validation(&self) -> PointValidation {
        PointValidation::TraceAssignedLocalModelRechecked
    }
    pub const fn source_meaning(&self) -> SourceMeaning {
        SourceMeaning::NeedsEvidence
    }
    pub const fn mapping_basis(&self) -> MappingBasis {
        MappingBasis::Given
    }
    pub const fn source_interpretation(&self) -> SourceInterpretation {
        SourceInterpretation::Unverified
    }
    pub const fn checker_independence(&self) -> CheckerIndependence {
        CheckerIndependence::UnassessedSharedIr
    }
}

/// Actual native store/input, formal plan/candidate and trace-assigned checked
/// model remain live. No Clone/Deref/Deserialize or historical-report insertion.
/// Neither a fresh link claim nor matching source IDs renews a stale native store.
///
/// ```compile_fail
/// use lc631_runtime::temporal_smt_point::{PointObservation, OwnedPointCorrespondence};
/// fn inject(old: PointObservation) { OwnedPointCorrespondence::from_report(old); }
/// ```
pub struct OwnedPointCorrespondence<'owner, 'input, 'view> {
    request: PointRequest<'owner, 'input, 'view>,
    model: CheckedModel<'input>,
    model_bytes: Vec<u8>,
    observation: PointObservation,
    retained: ResourceClaim<'view>,
}
impl OwnedPointCorrespondence<'_, '_, '_> {
    pub fn historical_report(&self) -> &PointObservation {
        &self.observation
    }
    /// Generated Given model syntax, not stdout from an external SMT process.
    pub fn historical_model_bytes(&self) -> &[u8] {
        &self.model_bytes
    }
    pub fn report_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&PointObservation, PointError> {
        self.current_native(resources)?;
        Ok(&self.observation)
    }
    /// World consumers must call this with their actual current native wrapper
    /// AND exact formal owners. A cloned native report cannot enter this port.
    /// Only this checked point-instance relation is established; no permission,
    /// whole-world recommendation, all-trace or natural-language truth follows.
    pub fn check_world_current(
        &self,
        world: &CurrentObservedTemporalWorld<'_, '_, '_, '_>,
        plan: &ProbePlan<'_>,
        candidate: &Candidate<'_>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&PointObservation, PointError> {
        if !std::ptr::eq(plan, self.request.plan) {
            return Err(PointError::ForeignPlan);
        }
        if !std::ptr::eq(candidate, self.request.candidate) {
            return Err(PointError::ForeignCandidate);
        }
        let own = self.current_native(resources)?;
        let actual = world.report_current(resources).map_err(PointError::World)?;
        if !std::ptr::eq(own, actual) {
            return Err(PointError::ForeignObservation);
        }
        self.current_native(resources)?;
        Ok(&self.observation)
    }
    fn current_native(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&TemporalEvaluation, PointError> {
        resources.check().map_err(PointError::Resource)?;
        self.retained.check().map_err(PointError::Resource)?;
        if !self.retained.same_reservation(resources) {
            return Err(PointError::Resource(ResourceError::ForeignOwner));
        }
        admit_lineage(self.request, resources)?;
        if !std::ptr::eq(self.model.query(), self.request.candidate.target_query()) {
            return Err(PointError::ForeignCandidate);
        }
        let report = read_native(self.request.native, resources)?;
        if report.truth() != self.observation.truth {
            return Err(PointError::ForeignObservation);
        }
        self.retained.check().map_err(PointError::Resource)?;
        Ok(report)
    }
}

/// Fresh construction from the actual current finite producer, structural
/// Boolean projection, and existing SMT assertion/model reevaluation.
/// Checks are time-of-call observations, never an atomic revocation fence.
pub fn evaluate_point<'owner, 'input, 'view>(
    request: PointRequest<'owner, 'input, 'view>,
    resources: &'view ResourceReservationView<'_>,
    budget: PointBudget,
) -> Result<OwnedPointCorrespondence<'owner, 'input, 'view>, PointError> {
    resources.check().map_err(PointError::Resource)?;
    // Both claims precede AST traversals, payload/parser allocation and model checking.
    let retained = claim(resources, RETAINED_RAM)?;
    let scratch = claim(resources, SCRATCH_RAM)?;
    admit_lineage(request, resources)?;
    let native = read_native(request.native, resources)?;
    let mut work = projection::Work::new(budget.max_ast_visits);
    projection::admit(request, &mut work)?;
    let (model_bytes, model) = projection::model(request)?;
    let predicate = request
        .plan
        .fixed_probes()
        .get(request.probe)
        .ok_or(PointError::MissingProbe)?
        .template
        .target_predicate;
    let value = projection::evaluate(predicate, &model, &mut work, 0)?;
    let expected = match native.truth() {
        Truth::True => true,
        Truth::False => false,
        Truth::Unknown => return Err(PointError::NeedsEvidence(MissingPoint::NativeUnknown)),
    };
    if value != expected {
        return Err(PointError::PredicateMismatch);
    }
    let observation = PointObservation {
        truth: native.truth(),
        assertions: model.assertions_checked(),
        visits: work.used(),
    };
    scratch.check().map_err(PointError::Resource)?;
    retained.check().map_err(PointError::Resource)?;
    read_native(request.native, resources)?;
    let owned = OwnedPointCorrespondence {
        request,
        model,
        model_bytes,
        observation,
        retained,
    };
    owned.current_native(resources)?;
    Ok(owned)
}

fn admit_lineage(
    request: PointRequest<'_, '_, '_>,
    resources: &ResourceReservationView<'_>,
) -> Result<(), PointError> {
    let context = request.native.query.context();
    let source = request.source.id().digest();
    if request
        .source
        .slice(request.span)
        .map_err(PointError::Source)?
        .is_empty()
        || context.source().digest() != source
        || resources.source_revision() != source
        || request.plan.scope().source.digest() != source
        || request.plan.scope().context.source().digest() != source
        || request.candidate.revision().digest() != source
        || request.candidate.context() != context
    {
        return Err(PointError::SourceBinding);
    }
    let scope = request.plan.scope();
    if scope.comparison != request.native.scope.comparison()
        || request.candidate.comparison() != scope.comparison
        || scope.probe_set != request.native.scope.probes()
    {
        return Err(PointError::ScopeBinding);
    }
    if request.plan.fixed_probes().get(request.probe).is_none() {
        return Err(PointError::MissingProbe);
    }
    Ok(())
}
fn read_native<'store>(
    native: NativePoint<'store, '_, '_>,
    resources: &ResourceReservationView<'_>,
) -> Result<&'store TemporalEvaluation, PointError> {
    native
        .store
        .lookup_temporal_current(
            native.id,
            native.query,
            native.trace,
            native.query.context(),
            native.scope,
            resources,
        )
        .map_err(PointError::Store)
}
fn claim<'v>(
    resources: &'v ResourceReservationView<'_>,
    bytes: u64,
) -> Result<ResourceClaim<'v>, PointError> {
    resources
        .claim_resources(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(bytes).map_err(PointError::Resource)?,
        )])
        .map_err(PointError::Resource)
}
