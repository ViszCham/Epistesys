//! Source-bound finite candidate generation and separately implemented model checking.
//! Given source/AST correspondence and shared IR are not independent world evidence.
#![forbid(unsafe_code)]
mod admission;
mod generate;
pub mod measurement;
pub mod semantic_configuration;
mod validator;
use lc631_analysis::creative_candidates::{
    CreativePreparation, PreparationError, PreparedCandidate,
};
use lc631_core::context::{ContextConsumer, OriginalContext};
use lc631_solvers::typed_query::{BoolExpr, QueryEmitError, SmtStatusQuery};
use lc631_solvers::{protocol::sexpr::SExprError, smt::ModelValidationError};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use lc631_substrate::source::{SourceError, SourceSpan, SourceStore};
use lc631_substrate::Digest32;
use std::sync::Arc;

pub const METADATA_RAM: u64 = 1024 * 1024;
pub const VALIDATION_SCRATCH_RAM: u64 = 8 * 1024 * 1024;
pub const CANDIDATE_RAM: u64 = 1024 * 1024;
pub const CANDIDATE_OUTPUT: u64 = 8192;
#[derive(Clone, Copy, Debug)]
pub struct BridgeLimits {
    pub max_candidates: usize,
    pub max_ast_nodes: usize,
    pub max_validation_checks: u32,
    pub max_payload_bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct GivenPremise<'a> {
    pub id: Digest32,
    pub span: SourceSpan,
    pub formula: &'a BoolExpr,
}
#[derive(Clone, Copy, Debug)]
pub struct GoalPolicy<'a> {
    pub revision: Digest32,
    pub query: &'a SmtStatusQuery,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Connective {
    And,
    Or,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operator {
    ReframeDeMorgan,
    Recombine {
        partner: Digest32,
        connective: Connective,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreativeError {
    InvalidConfig,
    ScopeMismatch,
    BindingMismatch,
    ForeignSource,
    HardInvariant,
    MissingPremise,
    Budget,
    UnsupportedFragment,
    UnsupportedOperator,
    AllocationFailed,
    NoFeasibleModels,
    GoalCounterexample { assignment: u16 },
    Resource(ResourceError),
    Preparation(PreparationError),
    Source(SourceError),
    Query(QueryEmitError),
    Syntax(SExprError),
    Model(ModelValidationError),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationBoundary {
    SeparateAlgorithmSharedIrGivenSource,
}
/// Private finite-profile certificate, not source truth, an execution receipt or Grant.
pub struct FiniteValidation {
    assignments: u16,
    feasible: u16,
}
impl FiniteValidation {
    pub fn assignments(&self) -> u16 {
        self.assignments
    }
    pub fn feasible_models(&self) -> u16 {
        self.feasible
    }
    pub fn boundary(&self) -> ValidationBoundary {
        ValidationBoundary::SeparateAlgorithmSharedIrGivenSource
    }
    pub fn mapping_basis(&self) -> crate::formal_boolean::MappingBasis {
        crate::formal_boolean::MappingBasis::Given
    }
    pub fn source_interpretation(&self) -> crate::formal_boolean::SourceInterpretation {
        crate::formal_boolean::SourceInterpretation::Unverified
    }
    pub fn checker_independence(&self) -> crate::formal_boolean::CheckerIndependence {
        crate::formal_boolean::CheckerIndependence::UnassessedSharedIr
    }
}

/// Caller-owned source/AST/goal backing remains borrowed. Context states drop
/// before the metadata claim. This is logical payload accounting, never RSS.
pub struct CreativeCandidateBridge<'input, 'view> {
    preparation: CreativePreparation,
    source: &'input SourceStore,
    target: Digest32,
    bindings: &'input [GivenPremise<'input>],
    goal: GoalPolicy<'input>,
    limits: BridgeLimits,
    owner: Arc<()>,
    metadata: ResourceClaim<'view>,
}
impl<'input, 'view> CreativeCandidateBridge<'input, 'view> {
    #[allow(clippy::too_many_arguments)] // Keep source, context, mapping and goal contracts explicit.
    pub fn with_resources(
        source: &'input SourceStore,
        original: OriginalContext,
        target: Digest32,
        bindings: &'input [GivenPremise<'input>],
        goal: GoalPolicy<'input>,
        limits: BridgeLimits,
        resources: &'view ResourceReservationView<'_>,
    ) -> Result<Self, CreativeError> {
        resources.check().map_err(CreativeError::Resource)?;
        if source.id().digest() != original.state().revision().source().digest()
            || resources.source_revision() != source.id().digest()
        {
            return Err(CreativeError::ForeignSource);
        }
        let metadata = claim(resources, METADATA_RAM, None)?;
        let scratch = claim(resources, VALIDATION_SCRATCH_RAM, None)?;
        admission::check(source, &original, target, bindings, goal, limits)?;
        let preparation = CreativePreparation::new(original, limits.max_candidates)
            .map_err(CreativeError::Preparation)?;
        scratch.check().map_err(CreativeError::Resource)?;
        metadata.check().map_err(CreativeError::Resource)?;
        Ok(Self {
            preparation,
            source,
            target,
            bindings,
            goal,
            limits,
            owner: Arc::new(()),
            metadata,
        })
    }
    pub fn contexts(&self) -> &ContextConsumer {
        self.preparation.contexts()
    }
    pub fn proposals(&self) -> &[PreparedCandidate] {
        self.preparation.proposals()
    }
    fn check_current(&self, resources: &ResourceReservationView<'_>) -> Result<(), CreativeError> {
        resources.check().map_err(CreativeError::Resource)?;
        self.metadata.check().map_err(CreativeError::Resource)?;
        if !self.metadata.same_reservation(resources) {
            return Err(CreativeError::BindingMismatch);
        }
        if resources.source_revision() != self.source.id().digest() {
            return Err(CreativeError::ForeignSource);
        }
        Ok(())
    }
}

/// Query/payload/certificate drop before the retained claim. No public report
/// injection or Deserialize constructor exists. Aliases retain core provenance.
pub struct ValidatedFormalCandidate<'input, 'view> {
    query: SmtStatusQuery,
    payload: Vec<u8>,
    validation: FiniteValidation,
    prepared: PreparedCandidate,
    operator: Operator,
    goal: GoalPolicy<'input>,
    target: Digest32,
    owner: Arc<()>,
    retained: ResourceClaim<'view>,
}
impl ValidatedFormalCandidate<'_, '_> {
    pub fn prepared(&self) -> PreparedCandidate {
        self.prepared
    }
    pub fn operator(&self) -> Operator {
        self.operator
    }
    pub fn validation(&self) -> &FiniteValidation {
        &self.validation
    }
    /// Historical bytes only; cancellation/expiry is not refreshed by reading.
    pub fn historical_payload(&self) -> &[u8] {
        &self.payload
    }
    pub fn project_current(
        &self,
        bridge: &CreativeCandidateBridge<'_, '_>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&SmtStatusQuery, CreativeError> {
        bridge.check_current(resources)?;
        self.retained.check().map_err(CreativeError::Resource)?;
        if !self.retained.same_reservation(resources)
            || !Arc::ptr_eq(&self.owner, &bridge.owner)
            || !std::ptr::eq(self.goal.query, bridge.goal.query)
            || self.goal.revision != bridge.goal.revision
        {
            return Err(CreativeError::BindingMismatch);
        }
        let state = bridge
            .contexts()
            .get(self.prepared.target())
            .ok_or(CreativeError::BindingMismatch)?;
        let premise = state
            .premises()
            .iter()
            .find(|p| p.id() == self.target)
            .ok_or(CreativeError::MissingPremise)?;
        if premise.statement().as_bytes() != self.payload {
            return Err(CreativeError::BindingMismatch);
        }
        Ok(&self.query)
    }
}
fn claim<'a>(
    view: &'a ResourceReservationView<'_>,
    ram: u64,
    output: Option<u64>,
) -> Result<ResourceClaim<'a>, CreativeError> {
    let ram = ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(ram).map_err(CreativeError::Resource)?,
    );
    match output {
        None => view.claim_resources(&[ram]),
        Some(bytes) => view.claim_resources(&[
            ram,
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(bytes).map_err(CreativeError::Resource)?,
            ),
        ]),
    }
    .map_err(CreativeError::Resource)
}
fn vector<T>(capacity: usize) -> Result<Vec<T>, CreativeError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| CreativeError::AllocationFailed)?;
    if values.capacity() > capacity {
        return Err(CreativeError::AllocationFailed);
    }
    Ok(values)
}
