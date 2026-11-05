//! Explicit bounded formal input, not inferred natural-language mathematics.
//! Checked constructors lower syntax; no solver report, proof or Grant is made.
use lc631_core::context::{ContextRevision, ContextScope};
use lc631_solvers::{
    smt::ObjectiveDirection,
    typed_query::{BoolSymbolId, IntSymbolId, SmtStatusQuery},
};
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    source::{SourceSpan, SourceStore},
};
mod commands;
mod expressions;

pub const MAX_SOURCE_BYTES: usize = 16 * 1024;
pub const MAX_NODES: usize = 4096;
pub const MAX_DEPTH: usize = 64;
pub const MAX_EMITTED_BYTES: usize = 64 * 1024;
// Conservative bounded parser/typed-AST/symbol/emission allowance. Not RSS or
// allocator-header evidence; nested standard Box allocation may still abort OOM.
const LOWERING_RAM: u64 = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredLogic {
    Boolean,
    QfLia,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecommendationState {
    NoRecommendation,
    ExplicitFiniteGoal,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormalSymbol {
    Boolean(BoolSymbolId),
    Integer(IntSymbolId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SymbolBinding {
    pub(super) span: SourceSpan,
    pub(super) declaration: SourceSpan,
    pub(super) symbol: FormalSymbol,
}
impl SymbolBinding {
    pub const fn span(self) -> SourceSpan {
        self.span
    }
    pub const fn declaration_span(self) -> SourceSpan {
        self.declaration
    }
    pub const fn symbol(self) -> FormalSymbol {
        self.symbol
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExplicitGoal {
    pub(super) direction: ObjectiveDirection,
    pub(super) symbol: IntSymbolId,
    pub(super) span: SourceSpan,
    pub(super) lower: i64,
    pub(super) upper: i64,
}
impl ExplicitGoal {
    pub const fn direction(self) -> ObjectiveDirection {
        self.direction
    }
    pub const fn symbol(self) -> IntSymbolId {
        self.symbol
    }
    pub const fn span(self) -> SourceSpan {
        self.span
    }
    pub const fn lower(self) -> i64 {
        self.lower
    }
    pub const fn upper(self) -> i64 {
        self.upper
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResidualReason {
    UnsupportedCommand,
    UnsupportedLogic,
    UnsupportedExpression,
    UnsupportedSymbol,
    CommandAfterCheckSat,
    UnboundedOrUnsupportedGoal,
    UnprocessedRemainder,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormalResidual {
    pub(super) span: SourceSpan,
    pub(super) detail: SourceSpan,
    pub(super) reason: ResidualReason,
}
impl FormalResidual {
    pub const fn span(self) -> SourceSpan {
        self.span
    }
    pub const fn detail_span(self) -> SourceSpan {
        self.detail
    }
    pub const fn reason(self) -> ResidualReason {
        self.reason
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormalErrorKind {
    SourceTooLarge,
    SourceMismatch,
    MissingLogic,
    MissingCheckSat,
    MalformedCommand,
    DuplicateSymbol,
    UndeclaredSymbol,
    SortMismatch,
    ReservedSymbol,
    MultipleGoals,
    NumericOutOfRange,
    LimitExceeded,
    AllocationFailed,
    Syntax(lc631_solvers::protocol::sexpr::SExprError),
    Emission(lc631_solvers::typed_query::QueryEmitError),
    Resource(ResourceError),
    Unsupported(ResidualReason),
}
#[derive(Debug)]
pub struct FormalError {
    pub(super) source: SourceStore,
    pub(super) span: SourceSpan,
    pub(super) kind: FormalErrorKind,
}
impl FormalError {
    pub fn kind(&self) -> &FormalErrorKind {
        &self.kind
    }
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
    pub fn source(&self) -> &SourceStore {
        &self.source
    }
}

pub struct FormalProblem<'view> {
    source: SourceStore,
    query: SmtStatusQuery,
    context: ContextRevision,
    scope: ContextScope,
    logic: DeclaredLogic,
    goal: Option<ExplicitGoal>,
    symbols: Vec<SymbolBinding>,
    assertion_spans: Vec<SourceSpan>,
    // Backing query/source references and metadata drop before refund.
    _claim: ResourceClaim<'view>,
}
impl FormalProblem<'_> {
    pub fn source(&self) -> &SourceStore {
        &self.source
    }
    pub fn query(&self) -> &SmtStatusQuery {
        &self.query
    }
    pub const fn context(&self) -> ContextRevision {
        self.context
    }
    pub const fn scope(&self) -> ContextScope {
        self.scope
    }
    pub const fn declared_logic(&self) -> DeclaredLogic {
        self.logic
    }
    pub fn goal(&self) -> Option<&ExplicitGoal> {
        self.goal.as_ref()
    }
    pub fn symbols(&self) -> &[SymbolBinding] {
        &self.symbols
    }
    pub fn assertion_spans(&self) -> &[SourceSpan] {
        &self.assertion_spans
    }
    pub const fn recommendation_state(&self) -> RecommendationState {
        if self.goal.is_some() {
            RecommendationState::ExplicitFiniteGoal
        } else {
            RecommendationState::NoRecommendation
        }
    }
    pub fn check_current(
        &self,
        current: &ResourceReservationView<'_>,
    ) -> Result<(), ResourceError> {
        self._claim.check()?;
        if !self._claim.same_reservation(current) {
            return Err(ResourceError::ForeignOwner);
        }
        current.check()
    }
}
pub struct FormalResidualReport<'view> {
    source: SourceStore,
    context: ContextRevision,
    scope: ContextScope,
    logic: Option<DeclaredLogic>,
    residuals: Vec<FormalResidual>,
    _claim: ResourceClaim<'view>,
}
impl FormalResidualReport<'_> {
    pub fn source(&self) -> &SourceStore {
        &self.source
    }
    pub const fn context(&self) -> ContextRevision {
        self.context
    }
    pub const fn scope(&self) -> ContextScope {
        self.scope
    }
    pub const fn declared_logic(&self) -> Option<DeclaredLogic> {
        self.logic
    }
    pub fn residuals(&self) -> &[FormalResidual] {
        &self.residuals
    }
    /// Revalidate active consumption; historical source inspection stays readable.
    pub fn check_current(
        &self,
        current: &ResourceReservationView<'_>,
    ) -> Result<(), ResourceError> {
        self._claim.check()?;
        if !self._claim.same_reservation(current) {
            return Err(ResourceError::ForeignOwner);
        }
        current.check()?;
        if current.source_revision() != self.context.source().digest()
            || self.source.id().digest() != self.context.source().digest()
        {
            return Err(ResourceError::SourceMismatch);
        }
        Ok(())
    }
}
#[allow(clippy::large_enum_variant)]
pub enum FormalLowering<'view> {
    Checked(FormalProblem<'view>),
    Unsupported(FormalResidualReport<'view>),
}

pub fn lower_explicit_formal<'view>(
    source: &SourceStore,
    context: ContextRevision,
    scope: ContextScope,
    current: &'view ResourceReservationView<'_>,
) -> Result<FormalLowering<'view>, FormalError> {
    // SourceStore construction guarantees valid UTF-8 and length <= u32::MAX.
    let whole = SourceSpan::checked(source, 0, source.text().len())
        .expect("whole SourceStore span is representable");
    let fail = |kind| FormalError {
        source: source.clone(),
        span: whole,
        kind,
    };
    if source.text().len() > MAX_SOURCE_BYTES {
        return Err(fail(FormalErrorKind::SourceTooLarge));
    }
    current
        .check()
        .map_err(|e| fail(FormalErrorKind::Resource(e)))?;
    if source.id().digest() != context.source().digest()
        || current.source_revision() != source.id().digest()
    {
        return Err(fail(FormalErrorKind::SourceMismatch));
    }
    let amount =
        ResourceAmount::checked(LOWERING_RAM).map_err(|e| fail(FormalErrorKind::Resource(e)))?;
    let claim = current
        .claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
        .map_err(|e| fail(FormalErrorKind::Resource(e)))?;
    commands::lower(source, context, scope, current, claim)
}
