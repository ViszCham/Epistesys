//! Bounded query execution contract. A runner observation is not a proof token.
use crate::logical::{LogicalResult, Satisfiability};
use crate::protocol::clingo::{
    parse_clingo_output, ClingoOutputLimits, ClingoOutputSummary, ClingoTerminalStatus,
    MAX_CLINGO_ANSWER_BLOCKS, MAX_CLINGO_ATOM_LINES,
};
use crate::protocol::payload::{
    parse_payload_syntax, parse_payload_syntax_reserved, PayloadSyntaxError, PayloadSyntaxSummary,
};
use crate::protocol::payload_wire::{
    observed_wire_status, parse_payload_wire_reserved, PayloadWireError, PayloadWireStatus,
    PayloadWireSummary,
};
use crate::protocol::sexpr::SExprLimits;
use crate::protocol::{
    parse_status, query_protocol_compatibility, ProtocolError, QueryProtocolCompatibility,
    QueryResultKind, SolverProtocol, ValidationBasis,
};
use crate::registry::{
    BackendId, LogicProfile, RegistryCompleteness, SolverRegistration, SolverRegistry,
    SolverRegistryError, SolverResourceCaps, MAX_SOLVER_ARGUMENTS, MAX_SOLVER_ARGUMENT_BYTES,
    MAX_SOLVER_CLEANUP_BYTES, MAX_SOLVER_DISK_BYTES, MAX_SOLVER_OUTPUT_BYTES, MAX_SOLVER_RAM_BYTES,
    MAX_SOLVER_STDIN_BYTES, MAX_SOLVER_TIMEOUT_MS,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceKind, ResourceRequest, ResourceReservationView,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct QuerySourceIdentity([u8; 32]);

impl QuerySourceIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct QueryDigest([u8; 32]);

impl QueryDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Identity of one registered solver invocation profile. Unlike `QueryDigest`,
/// this changes when the declared solver binary, version, options, or resource
/// caps change. It is an invocation key, not proof that the process matched it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct InvocationDigest([u8; 32]);

impl InvocationDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

fn compute_query_digest(
    backend: BackendId,
    source: QuerySourceIdentity,
    profile: LogicProfile,
    result_kind: QueryResultKind,
    payload: &[u8],
) -> Result<QueryDigest, QueryError> {
    use sha2::{Digest, Sha256};
    let payload_len = u64::try_from(payload.len()).map_err(|_| QueryError::ArithmeticOverflow)?;
    let mut digest = Sha256::new();
    digest.update(b"epistesys.solver-query.v1\0");
    digest.update([match backend {
        BackendId::Z3 => 0x01,
        BackendId::Clingo => 0x02,
    }]);
    digest.update(source.as_bytes());
    digest.update([match profile {
        LogicProfile::SmtBooleanQfLia => 0x01,
        LogicProfile::AspFiniteGroundNormal => 0x02,
    }]);
    digest.update([match result_kind {
        QueryResultKind::Satisfiability => 0x01,
        QueryResultKind::Entailment => 0x02,
        QueryResultKind::StableModelExistence => 0x03,
        QueryResultKind::ModelCheck => 0x04,
        QueryResultKind::Inference => 0x05,
        QueryResultKind::Optimization => 0x06,
        QueryResultKind::Intervention => 0x07,
        QueryResultKind::SpatialRelation => 0x08,
        QueryResultKind::ModelPayload => 0x09,
        QueryResultKind::ProofPayload => 0x0a,
        QueryResultKind::CounterexamplePayload => 0x0b,
    }]);
    digest.update(payload_len.to_le_bytes());
    digest.update(payload);
    Ok(QueryDigest(digest.finalize().into()))
}

fn compute_invocation_digest(
    registration: &SolverRegistration,
    budget: &ExecutionBudget,
) -> InvocationDigest {
    use sha2::{Digest, Sha256};

    fn add_bytes(digest: &mut Sha256, bytes: &[u8]) {
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }

    let mut digest = Sha256::new();
    digest.update(b"epistesys.solver-invocation.v1\0");
    digest.update([match registration.id() {
        BackendId::Z3 => 0x01,
        BackendId::Clingo => 0x02,
    }]);
    digest.update([match registration.profile() {
        LogicProfile::SmtBooleanQfLia => 0x01,
        LogicProfile::AspFiniteGroundNormal => 0x02,
    }]);
    digest.update([match registration.protocol() {
        SolverProtocol::SmtLibStatusV1 => 0x01,
        SolverProtocol::ClingoStatusV1 => 0x02,
        SolverProtocol::SmtLibPayloadV1 => 0x03,
        SolverProtocol::SmtLibPayloadWireV2 => 0x04,
    }]);
    add_bytes(&mut digest, registration.executable().as_bytes());
    add_bytes(&mut digest, registration.version().as_bytes());
    digest.update(registration.binary_digest());
    digest.update((registration.fixed_arguments().len() as u64).to_le_bytes());
    for argument in registration.fixed_arguments() {
        add_bytes(&mut digest, argument.as_bytes());
    }

    let caps = registration.caps();
    digest.update((caps.max_argv_count as u64).to_le_bytes());
    digest.update((caps.max_argv_bytes as u64).to_le_bytes());
    digest.update((caps.max_stdin_bytes as u64).to_le_bytes());
    digest.update((caps.max_stdout_bytes as u64).to_le_bytes());
    digest.update((caps.max_stderr_bytes as u64).to_le_bytes());
    digest.update(caps.max_timeout_ms.to_le_bytes());
    digest.update(caps.max_ram_bytes.to_le_bytes());
    digest.update(caps.max_disk_bytes.to_le_bytes());
    digest.update(caps.max_cleanup_bytes.to_le_bytes());
    digest.update(caps.max_child_count.to_le_bytes());
    digest.update(budget.timeout_ms.to_le_bytes());
    digest.update((budget.max_argv_count as u64).to_le_bytes());
    digest.update((budget.max_argv_bytes as u64).to_le_bytes());
    digest.update((budget.max_stdin_bytes as u64).to_le_bytes());
    digest.update((budget.max_stdout_bytes as u64).to_le_bytes());
    digest.update((budget.max_stderr_bytes as u64).to_le_bytes());
    digest.update(budget.ram_bytes.to_le_bytes());
    digest.update(budget.disk_bytes.to_le_bytes());
    digest.update(budget.cleanup_bytes.to_le_bytes());
    digest.update(budget.child_count.to_le_bytes());
    InvocationDigest(digest.finalize().into())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReservationFailure {
    Cancelled,
    DeadlineExceeded,
    Insufficient,
    SourceMismatch,
    Stale,
    Unknown,
}

/// A borrowed, live reservation seam. Production adapters must implement this
/// with the current resource broker lease; reports/DTOs are not reservations.
pub trait LiveResourceReservation {
    fn validate(
        &self,
        expected_source: QuerySourceIdentity,
        budget: &ExecutionBudget,
    ) -> Result<(), ReservationFailure>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetError {
    InvalidLimit,
    LimitTooLarge,
    ArithmeticOverflow,
}

/// One child admission and its simultaneous resource ceilings. The runner
/// receives this exact profile and must reserve it before starting a process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionBudget {
    timeout_ms: u64,
    max_argv_count: usize,
    max_argv_bytes: usize,
    max_stdin_bytes: usize,
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
    ram_bytes: u64,
    disk_bytes: u64,
    cleanup_bytes: u64,
    child_count: u32,
}

impl ExecutionBudget {
    #[allow(clippy::too_many_arguments)]
    pub fn checked(
        timeout_ms: u64,
        max_stdin_bytes: usize,
        max_stdout_bytes: usize,
        max_stderr_bytes: usize,
        max_argv_count: usize,
        max_argv_bytes: usize,
        ram_bytes: u64,
        disk_bytes: u64,
        cleanup_bytes: u64,
    ) -> Result<Self, BudgetError> {
        if timeout_ms == 0
            || max_argv_count == 0
            || max_argv_bytes == 0
            || max_stdin_bytes == 0
            || max_stdout_bytes == 0
            || max_stderr_bytes == 0
            || ram_bytes == 0
            || cleanup_bytes == 0
        {
            return Err(BudgetError::InvalidLimit);
        }
        if timeout_ms > MAX_SOLVER_TIMEOUT_MS
            || max_argv_count > MAX_SOLVER_ARGUMENTS
            || max_argv_bytes > MAX_SOLVER_ARGUMENT_BYTES
            || max_stdin_bytes > MAX_SOLVER_STDIN_BYTES
            || max_stdout_bytes > MAX_SOLVER_OUTPUT_BYTES
            || max_stderr_bytes > MAX_SOLVER_OUTPUT_BYTES
            || ram_bytes > MAX_SOLVER_RAM_BYTES
            || disk_bytes > MAX_SOLVER_DISK_BYTES
            || cleanup_bytes > MAX_SOLVER_CLEANUP_BYTES
        {
            return Err(BudgetError::LimitTooLarge);
        }
        let _simultaneous_output = max_stdout_bytes
            .checked_add(max_stderr_bytes)
            .ok_or(BudgetError::ArithmeticOverflow)?;
        Ok(Self {
            timeout_ms,
            max_argv_count,
            max_argv_bytes,
            max_stdin_bytes,
            max_stdout_bytes,
            max_stderr_bytes,
            ram_bytes,
            disk_bytes,
            cleanup_bytes,
            child_count: 1,
        })
    }

    pub const fn timeout_ms(self) -> u64 {
        self.timeout_ms
    }
    pub const fn max_argv_count(self) -> usize {
        self.max_argv_count
    }
    pub const fn max_argv_bytes(self) -> usize {
        self.max_argv_bytes
    }
    pub const fn max_stdin_bytes(self) -> usize {
        self.max_stdin_bytes
    }
    pub const fn max_stdout_bytes(self) -> usize {
        self.max_stdout_bytes
    }
    pub const fn max_stderr_bytes(self) -> usize {
        self.max_stderr_bytes
    }
    pub const fn ram_bytes(self) -> u64 {
        self.ram_bytes
    }
    pub const fn disk_bytes(self) -> u64 {
        self.disk_bytes
    }
    pub const fn cleanup_bytes(self) -> u64 {
        self.cleanup_bytes
    }
    pub const fn child_count(self) -> u32 {
        self.child_count
    }

    fn within(self, caps: SolverResourceCaps) -> bool {
        self.timeout_ms <= caps.max_timeout_ms
            && self.max_argv_count <= caps.max_argv_count
            && self.max_argv_bytes <= caps.max_argv_bytes
            && self.max_stdin_bytes <= caps.max_stdin_bytes
            && self.max_stdout_bytes <= caps.max_stdout_bytes
            && self.max_stderr_bytes <= caps.max_stderr_bytes
            && self.ram_bytes <= caps.max_ram_bytes
            && self.disk_bytes <= caps.max_disk_bytes
            && self.cleanup_bytes <= caps.max_cleanup_bytes
            && self.child_count <= caps.max_child_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryError {
    ArithmeticOverflow,
    EmptyPayload,
    PayloadTooLarge,
    UnsupportedBackendProfile,
}

/// Typed, source-bound bytes from a registered lowering. Payload bytes are not
/// executable permissions and are always sent on stdin, never concatenated
/// into a shell command or argv string.
#[derive(Clone, Copy, Debug)]
pub struct SolverQuery<'a> {
    backend: BackendId,
    source: QuerySourceIdentity,
    query: QueryDigest,
    profile: LogicProfile,
    result_kind: QueryResultKind,
    payload: &'a [u8],
}

impl<'a> SolverQuery<'a> {
    pub fn checked(
        backend: BackendId,
        source: QuerySourceIdentity,
        profile: LogicProfile,
        result_kind: QueryResultKind,
        payload: &'a [u8],
    ) -> Result<Self, QueryError> {
        if payload.is_empty() {
            return Err(QueryError::EmptyPayload);
        }
        if payload.len() > MAX_SOLVER_STDIN_BYTES {
            return Err(QueryError::PayloadTooLarge);
        }
        let supported = matches!(
            (backend, profile),
            (BackendId::Z3, LogicProfile::SmtBooleanQfLia)
                | (BackendId::Clingo, LogicProfile::AspFiniteGroundNormal)
        );
        if !supported {
            return Err(QueryError::UnsupportedBackendProfile);
        }
        let query = compute_query_digest(backend, source, profile, result_kind, payload)?;
        Ok(Self {
            backend,
            source,
            query,
            profile,
            result_kind,
            payload,
        })
    }

    pub const fn backend(&self) -> BackendId {
        self.backend
    }
    pub const fn source(&self) -> QuerySourceIdentity {
        self.source
    }
    pub const fn digest(&self) -> QueryDigest {
        self.query
    }
    pub const fn profile(&self) -> LogicProfile {
        self.profile
    }
    pub const fn result_kind(&self) -> QueryResultKind {
        self.result_kind
    }
    pub const fn payload(&self) -> &'a [u8] {
        self.payload
    }
}

/// The process adapter must pass these argv elements separately and attach the
/// borrowed live reservation. It must not invoke a shell or resource-free API.
pub struct ProcessRequest<'a> {
    pub backend: BackendId,
    pub profile: LogicProfile,
    pub protocol: SolverProtocol,
    pub executable: &'a str,
    pub expected_executable_digest: &'a [u8; 32],
    pub fixed_arguments: &'a [String],
    pub source: QuerySourceIdentity,
    pub query: QueryDigest,
    pub invocation: InvocationDigest,
    pub stdin: &'a [u8],
    pub budget: ExecutionBudget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessRunState {
    Completed,
    NotStarted,
    TimedOut,
    ResourceExhaustedBeforeStart,
    ResourceExhaustedAfterStartUnknown,
    Unsupported,
    Failed,
}

/// A report copied from the resource-owning supervisor. It is evidence only
/// that a caller reported these fields; it does not authenticate the caller.
pub struct RunnerReportedProcessObservation<'a> {
    pub state: ProcessRunState,
    pub source: QuerySourceIdentity,
    pub query: QueryDigest,
    pub invocation: InvocationDigest,
    /// The adapter's report of the exact argv slice passed without a shell.
    pub observed_arguments: &'a [String],
    pub executable_digest: [u8; 32],
    pub exit_code: Option<i32>,
    pub stdout: &'a [u8],
    pub stderr: &'a [u8],
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub cleanup_verified: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessFailure {
    Cancelled,
    DeadlineExceeded,
    Failed,
    NotStarted,
    ResourceExhaustedBeforeStart,
    ResourceExhaustedAfterStartUnknown,
    StartedUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessCleanupState {
    RunnerReportedNotStarted,
    RunnerReportedVerified,
    RunnerReportedUnknown,
}

/// Resource-backed process injection point. Production implementations should
/// adapt `run_bounded_with_resources` and return only while its live claim is
/// held. The trait is caller-implementable, so returned fields remain
/// runner-reported until a nonforgeable supervisor observation capability is
/// added. No overload without this reservation exists.
pub trait SolverProcessRunner {
    fn run<'a>(
        &'a mut self,
        request: ProcessRequest<'a>,
        reservation: &'a dyn LiveResourceReservation,
    ) -> Result<RunnerReportedProcessObservation<'a>, ProcessFailure>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolverExecutionState {
    SolverReported,
    UnsupportedProfile,
    ResourceRejected,
    NotStarted,
    StartedUnknown,
    TimedOut,
    Cancelled,
    Failed,
    OutputTruncated,
    CleanupUnknown,
    BindingMismatch,
    UnsupportedMapping,
    ProtocolFailure,
    PayloadSyntaxObserved,
    PayloadSyntaxRejected,
    PayloadWireObserved,
    PayloadWireRejected,
    AmbiguousRegistration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryBindingState {
    RunnerReportedDigestMatchesRegistration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvocationBindingState {
    RunnerReportedInvocationDigestMatchesRegistration,
}

/// A solver status or payload syntax summary is explicitly a runner report,
/// not an authenticated supervisor observation, checked proof, semantic truth
/// or Authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SolverExecutionReport {
    state: SolverExecutionState,
    registry_completeness: RegistryCompleteness,
    source: QuerySourceIdentity,
    query: QueryDigest,
    invocation: Option<InvocationDigest>,
    process_state: Option<ProcessRunState>,
    binary_binding: Option<BinaryBindingState>,
    invocation_binding: Option<InvocationBindingState>,
    exit_code: Option<i32>,
    cleanup_state: ProcessCleanupState,
    stdout_truncated: bool,
    stderr_truncated: bool,
    logical_result: Option<LogicalResult>,
    validation_basis: Option<ValidationBasis>,
    stdout_bytes: usize,
    stderr_bytes: usize,
    stdout_digest: Option<[u8; 32]>,
    stderr_digest: Option<[u8; 32]>,
    stdout_fully_observed: bool,
    stderr_fully_observed: bool,
    payload_syntax_summary: Option<PayloadSyntaxSummary>,
    payload_syntax_error: Option<PayloadSyntaxError>,
    clingo_output_summary: Option<ClingoOutputSummary>,
    payload_wire_summary: Option<PayloadWireSummary>,
    payload_wire_error: Option<PayloadWireError>,
    payload_wire_status: Option<PayloadWireStatus>,
}

impl SolverExecutionReport {
    fn empty(
        state: SolverExecutionState,
        registry_completeness: RegistryCompleteness,
        source: QuerySourceIdentity,
        query: QueryDigest,
    ) -> Self {
        Self {
            state,
            registry_completeness,
            source,
            query,
            invocation: None,
            process_state: None,
            binary_binding: None,
            invocation_binding: None,
            exit_code: None,
            cleanup_state: ProcessCleanupState::RunnerReportedNotStarted,
            stdout_truncated: false,
            stderr_truncated: false,
            logical_result: None,
            validation_basis: None,
            stdout_bytes: 0,
            stderr_bytes: 0,
            stdout_digest: None,
            stderr_digest: None,
            stdout_fully_observed: false,
            stderr_fully_observed: false,
            payload_syntax_summary: None,
            payload_syntax_error: None,
            clingo_output_summary: None,
            payload_wire_summary: None,
            payload_wire_error: None,
            payload_wire_status: None,
        }
    }
    pub const fn state(self) -> SolverExecutionState {
        self.state
    }
    pub const fn registry_completeness(self) -> RegistryCompleteness {
        self.registry_completeness
    }
    pub const fn source(self) -> QuerySourceIdentity {
        self.source
    }
    pub const fn query(self) -> QueryDigest {
        self.query
    }
    pub const fn invocation_digest(self) -> Option<InvocationDigest> {
        self.invocation
    }
    pub const fn process_state(self) -> Option<ProcessRunState> {
        self.process_state
    }
    pub const fn binary_binding_state(self) -> Option<BinaryBindingState> {
        self.binary_binding
    }
    pub const fn invocation_binding_state(self) -> Option<InvocationBindingState> {
        self.invocation_binding
    }
    pub const fn exit_code(self) -> Option<i32> {
        self.exit_code
    }
    pub const fn cleanup_state(self) -> ProcessCleanupState {
        self.cleanup_state
    }
    pub const fn stdout_truncated(self) -> bool {
        self.stdout_truncated
    }
    pub const fn stderr_truncated(self) -> bool {
        self.stderr_truncated
    }
    pub const fn logical_result(self) -> Option<LogicalResult> {
        self.logical_result
    }
    pub const fn validation_basis(self) -> Option<ValidationBasis> {
        self.validation_basis
    }
    pub const fn stdout_bytes(self) -> usize {
        self.stdout_bytes
    }
    pub const fn stderr_bytes(self) -> usize {
        self.stderr_bytes
    }
    /// Bounded digest of exact observed stdout bytes, or a digest of the
    /// bounded prefix plus reported total length when the runner returned an
    /// oversized buffer. This does not authenticate the runner.
    pub const fn stdout_digest(self) -> Option<[u8; 32]> {
        self.stdout_digest
    }
    /// Bounded digest of exact observed stderr bytes, or a digest of the
    /// bounded prefix plus reported total length when the runner returned an
    /// oversized buffer. This does not authenticate the runner.
    pub const fn stderr_digest(self) -> Option<[u8; 32]> {
        self.stderr_digest
    }
    /// True only when the process returned a completed state, reported an exit
    /// code, and the complete stream fit within the configured output bound.
    pub const fn stdout_fully_observed(self) -> bool {
        self.stdout_fully_observed
    }
    /// True only when the process returned a completed state, reported an exit
    /// code, and the complete stream fit within the configured output bound.
    pub const fn stderr_fully_observed(self) -> bool {
        self.stderr_fully_observed
    }
    pub const fn payload_syntax_summary(self) -> Option<PayloadSyntaxSummary> {
        self.payload_syntax_summary
    }
    pub const fn payload_syntax_error(self) -> Option<PayloadSyntaxError> {
        self.payload_syntax_error
    }
    pub const fn payload_wire_status(self) -> Option<PayloadWireStatus> {
        self.payload_wire_status
    }
    pub const fn payload_wire_summary(self) -> Option<PayloadWireSummary> {
        self.payload_wire_summary
    }
    pub const fn payload_wire_error(self) -> Option<PayloadWireError> {
        self.payload_wire_error
    }
    pub const fn clingo_output_summary(self) -> Option<ClingoOutputSummary> {
        self.clingo_output_summary
    }
    pub const fn proof_checked(self) -> bool {
        false
    }
    pub const fn authority_created(self) -> bool {
        false
    }
    pub const fn output_commit_allowed(self) -> bool {
        false
    }
    pub const fn claim_boundary(self) -> &'static str {
        "caller/runner-reported bounded status or payload syntax only; declared version is not independently observed; no nonforgeable supervisor observation capability, model/proof validation, semantic truth, authority, host output or commit"
    }
}

/// Structural upper bound from byte positions, not a semantic heuristic: each
/// node has a unique nonempty start byte, so nodes and roots cannot exceed
/// the complete stdout byte budget. Depth retains its independent declared cap.
pub(crate) fn native_payload_limits(
    max_output_bytes: usize,
) -> Result<SExprLimits, crate::protocol::sexpr::SExprError> {
    SExprLimits::checked(max_output_bytes, 65_536.min(max_output_bytes), 128)
}

/// Run one query only through a registry entry and caller-owned reservation.
/// All protocol successes retain SolverReported as their validation basis.
pub fn execute_registered<R: SolverProcessRunner>(
    registry: &SolverRegistry,
    query: &SolverQuery<'_>,
    budget: &ExecutionBudget,
    reservation: &dyn LiveResourceReservation,
    runner: &mut R,
) -> SolverExecutionReport {
    execute_registered_inner(registry, query, budget, reservation, runner, None, false)
}

/// Native adapter only: real reservation is required for payload parser scratch.
pub(crate) fn execute_registered_native<R: SolverProcessRunner>(
    registry: &SolverRegistry,
    query: &SolverQuery<'_>,
    budget: &ExecutionBudget,
    reservation: &ResourceReservationView<'_>,
    runner: &mut R,
    owned_v2: bool,
) -> SolverExecutionReport {
    execute_registered_inner(
        registry,
        query,
        budget,
        reservation,
        runner,
        Some(reservation),
        owned_v2,
    )
}

/// Unit-test seam: exercises native scratch preflight with a synthetic runner.
/// It is absent from non-test builds and never issues an owned observation.
#[cfg(test)]
pub(crate) fn execute_registered_native_for_test<R: SolverProcessRunner>(
    registry: &SolverRegistry,
    query: &SolverQuery<'_>,
    budget: &ExecutionBudget,
    reservation: &ResourceReservationView<'_>,
    runner: &mut R,
) -> SolverExecutionReport {
    execute_registered_native(registry, query, budget, reservation, runner, true)
}

fn execute_registered_inner<R: SolverProcessRunner>(
    registry: &SolverRegistry,
    query: &SolverQuery<'_>,
    budget: &ExecutionBudget,
    reservation: &dyn LiveResourceReservation,
    runner: &mut R,
    actual: Option<&ResourceReservationView<'_>>,
    owned_v2: bool,
) -> SolverExecutionReport {
    let registry_completeness = registry.completeness();
    let empty = |state| {
        SolverExecutionReport::empty(state, registry_completeness, query.source, query.query)
    };
    let registration =
        match registry.resolve_for_query(query.backend, query.profile, query.result_kind) {
            Ok(registration) => registration,
            Err(SolverRegistryError::UnsupportedProfile) => {
                return empty(SolverExecutionState::UnsupportedProfile);
            }
            Err(SolverRegistryError::UnsupportedMapping) => {
                return empty(SolverExecutionState::UnsupportedMapping);
            }
            Err(SolverRegistryError::AmbiguousRegistration) => {
                return empty(SolverExecutionState::AmbiguousRegistration);
            }
            Err(_) => return empty(SolverExecutionState::UnsupportedProfile),
        };
    let invocation = compute_invocation_digest(registration, budget);
    let empty_registered = |state| {
        let mut report = empty(state);
        report.invocation = Some(invocation);
        report
    };
    if query_protocol_compatibility(registration.protocol(), query.result_kind)
        == QueryProtocolCompatibility::Unsupported
    {
        return empty_registered(SolverExecutionState::UnsupportedMapping);
    }
    // A nominal, caller-implementable reservation cannot fund V2 scratch.
    if registration.protocol() == SolverProtocol::SmtLibPayloadWireV2
        && (actual.is_none() || !owned_v2)
    {
        return empty_registered(SolverExecutionState::UnsupportedMapping);
    }
    // Claim before process dispatch and retain until the syntax tree has dropped.
    let _scratch_claim = if matches!(
        registration.protocol(),
        SolverProtocol::SmtLibPayloadV1 | SolverProtocol::SmtLibPayloadWireV2
    ) {
        if let Some(view) = actual {
            let Some(bytes) = native_payload_limits(budget.max_stdout_bytes)
                .ok()
                .and_then(crate::protocol::sexpr::scratch_bytes)
            else {
                return empty_registered(SolverExecutionState::ResourceRejected);
            };
            let Ok(amount) = ResourceAmount::checked(bytes) else {
                return empty_registered(SolverExecutionState::ResourceRejected);
            };
            match view.claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)]) {
                Ok(claim) => Some(claim),
                Err(_) => return empty_registered(SolverExecutionState::ResourceRejected),
            }
        } else {
            None
        }
    } else {
        None
    };
    let argv_count = registration.fixed_arguments().len();
    let argv_bytes = registration
        .fixed_arguments()
        .iter()
        .try_fold(registration.executable().len(), |total, argument| {
            total.checked_add(argument.len())
        });
    if budget.max_stdin_bytes < query.payload.len()
        || argv_count > budget.max_argv_count
        || argv_bytes.is_none_or(|bytes| bytes > budget.max_argv_bytes)
        || !budget.within(registration.caps())
    {
        return empty_registered(SolverExecutionState::ResourceRejected);
    }
    if reservation.validate(query.source, budget).is_err() {
        return empty_registered(SolverExecutionState::ResourceRejected);
    }

    let request = ProcessRequest {
        backend: registration.id(),
        profile: registration.profile(),
        protocol: registration.protocol(),
        executable: registration.executable(),
        expected_executable_digest: registration.binary_digest(),
        fixed_arguments: registration.fixed_arguments(),
        source: query.source,
        query: query.query,
        invocation,
        stdin: query.payload,
        budget: *budget,
    };
    let process = match runner.run(request, reservation) {
        Ok(process) => process,
        Err(error) => {
            let (state, cleanup) = match error {
                ProcessFailure::NotStarted => (
                    SolverExecutionState::NotStarted,
                    ProcessCleanupState::RunnerReportedNotStarted,
                ),
                ProcessFailure::StartedUnknown => (
                    SolverExecutionState::StartedUnknown,
                    ProcessCleanupState::RunnerReportedUnknown,
                ),
                ProcessFailure::ResourceExhaustedBeforeStart => (
                    SolverExecutionState::ResourceRejected,
                    ProcessCleanupState::RunnerReportedNotStarted,
                ),
                ProcessFailure::ResourceExhaustedAfterStartUnknown => (
                    SolverExecutionState::StartedUnknown,
                    ProcessCleanupState::RunnerReportedUnknown,
                ),
                ProcessFailure::DeadlineExceeded => (
                    SolverExecutionState::TimedOut,
                    ProcessCleanupState::RunnerReportedUnknown,
                ),
                ProcessFailure::Cancelled => (
                    SolverExecutionState::Cancelled,
                    ProcessCleanupState::RunnerReportedUnknown,
                ),
                ProcessFailure::Failed => (
                    SolverExecutionState::Failed,
                    ProcessCleanupState::RunnerReportedUnknown,
                ),
            };
            let mut report = empty_registered(state);
            report.cleanup_state = cleanup;
            return report;
        }
    };
    let mut report = SolverExecutionReport {
        state: SolverExecutionState::Failed,
        registry_completeness,
        source: query.source,
        query: query.query,
        invocation: Some(invocation),
        process_state: Some(process.state),
        binary_binding: None,
        invocation_binding: None,
        exit_code: process.exit_code,
        cleanup_state: if process.cleanup_verified {
            ProcessCleanupState::RunnerReportedVerified
        } else {
            ProcessCleanupState::RunnerReportedUnknown
        },
        stdout_truncated: process.stdout_truncated,
        stderr_truncated: process.stderr_truncated,
        logical_result: None,
        validation_basis: None,
        stdout_bytes: process.stdout.len(),
        stderr_bytes: process.stderr.len(),
        stdout_digest: output_witness_digest(
            b"stdout",
            process.stdout,
            budget.max_stdout_bytes,
            process.stdout_truncated,
        ),
        stderr_digest: output_witness_digest(
            b"stderr",
            process.stderr,
            budget.max_stderr_bytes,
            process.stderr_truncated,
        ),
        stdout_fully_observed: process.state == ProcessRunState::Completed
            && process.exit_code.is_some()
            && process.cleanup_verified
            && !process.stdout_truncated
            && process.stdout.len() <= budget.max_stdout_bytes,
        stderr_fully_observed: process.state == ProcessRunState::Completed
            && process.exit_code.is_some()
            && process.cleanup_verified
            && !process.stderr_truncated
            && process.stderr.len() <= budget.max_stderr_bytes,
        payload_syntax_summary: None,
        payload_syntax_error: None,
        clingo_output_summary: None,
        payload_wire_summary: None,
        payload_wire_error: None,
        payload_wire_status: None,
    };
    match process.state {
        ProcessRunState::NotStarted | ProcessRunState::ResourceExhaustedBeforeStart => {
            report.cleanup_state = ProcessCleanupState::RunnerReportedNotStarted;
        }
        ProcessRunState::ResourceExhaustedAfterStartUnknown => {
            report.cleanup_state = ProcessCleanupState::RunnerReportedUnknown;
        }
        _ => {}
    }
    if process.source != query.source
        || process.query != query.query
        || process.invocation != invocation
        || process.observed_arguments != registration.fixed_arguments()
        || process.executable_digest != *registration.binary_digest()
    {
        report.state = SolverExecutionState::BindingMismatch;
        return report;
    }
    report.binary_binding = Some(BinaryBindingState::RunnerReportedDigestMatchesRegistration);
    report.invocation_binding =
        Some(InvocationBindingState::RunnerReportedInvocationDigestMatchesRegistration);
    if registration.protocol() == SolverProtocol::SmtLibPayloadWireV2
        && report.stdout_fully_observed
        && report.stderr_fully_observed
    {
        report.payload_wire_status = observed_wire_status(process.stdout);
    }

    if process.state == ProcessRunState::TimedOut {
        report.state = SolverExecutionState::TimedOut;
        return report;
    }
    if process.state == ProcessRunState::ResourceExhaustedBeforeStart {
        report.state = SolverExecutionState::ResourceRejected;
        report.cleanup_state = ProcessCleanupState::RunnerReportedNotStarted;
        return report;
    }
    if process.state == ProcessRunState::ResourceExhaustedAfterStartUnknown {
        report.state = SolverExecutionState::StartedUnknown;
        report.cleanup_state = ProcessCleanupState::RunnerReportedUnknown;
        return report;
    }
    if process.state == ProcessRunState::Unsupported {
        report.state = SolverExecutionState::UnsupportedProfile;
        return report;
    }
    if process.state == ProcessRunState::NotStarted {
        report.state = SolverExecutionState::NotStarted;
        return report;
    }
    if process.state != ProcessRunState::Completed || process.exit_code != Some(0) {
        report.state = SolverExecutionState::Failed;
        return report;
    }
    if !process.cleanup_verified {
        report.state = SolverExecutionState::CleanupUnknown;
        return report;
    }
    if process.stdout_truncated
        || process.stderr_truncated
        || process.stdout.len() > budget.max_stdout_bytes
        || process.stderr.len() > budget.max_stderr_bytes
    {
        report.state = SolverExecutionState::OutputTruncated;
        return report;
    }
    match registration.protocol() {
        SolverProtocol::SmtLibStatusV1 => match parse_status(
            SolverProtocol::SmtLibStatusV1,
            query.result_kind,
            process.stdout,
            budget.max_stdout_bytes,
        ) {
            Ok(result) => {
                report.state = SolverExecutionState::SolverReported;
                report.logical_result = Some(result);
                report.validation_basis = Some(ValidationBasis::SolverReported);
            }
            Err(ProtocolError::UnsupportedQueryKind) => {
                report.state = SolverExecutionState::UnsupportedMapping;
            }
            Err(ProtocolError::OutputTooLarge) => {
                report.state = SolverExecutionState::OutputTruncated;
            }
            Err(_) => {
                report.state = SolverExecutionState::ProtocolFailure;
            }
        },
        SolverProtocol::SmtLibPayloadV1 => {
            let limits = native_payload_limits(budget.max_stdout_bytes);
            let parsed = limits
                .map_err(PayloadSyntaxError::Syntax)
                .and_then(|limits| {
                    if actual.is_some() {
                        parse_payload_syntax_reserved(
                            SolverProtocol::SmtLibPayloadV1,
                            query.result_kind,
                            process.stdout,
                            limits,
                        )
                    } else {
                        parse_payload_syntax(
                            SolverProtocol::SmtLibPayloadV1,
                            query.result_kind,
                            process.stdout,
                            limits,
                        )
                    }
                });
            match parsed {
                Ok(parsed) => match parsed.summary() {
                    Ok(summary) => {
                        report.state = SolverExecutionState::PayloadSyntaxObserved;
                        report.payload_syntax_summary = Some(summary);
                    }
                    Err(error) => {
                        report.state = SolverExecutionState::PayloadSyntaxRejected;
                        report.payload_syntax_error = Some(error);
                    }
                },
                Err(error @ PayloadSyntaxError::UnsupportedMapping { .. }) => {
                    report.state = SolverExecutionState::UnsupportedMapping;
                    report.payload_syntax_error = Some(error);
                }
                Err(error) => {
                    report.state = SolverExecutionState::PayloadSyntaxRejected;
                    report.payload_syntax_error = Some(error);
                }
            }
        }
        SolverProtocol::SmtLibPayloadWireV2 => {
            let parsed = native_payload_limits(budget.max_stdout_bytes)
                .map_err(PayloadWireError::Syntax)
                .and_then(|limits| {
                    parse_payload_wire_reserved(query.result_kind, process.stdout, limits)
                });
            match parsed {
                Ok(view) => {
                    report.state = SolverExecutionState::PayloadWireObserved;
                    report.payload_wire_summary = Some(view.summary());
                    report.validation_basis = Some(ValidationBasis::SolverReported);
                }
                Err(error @ PayloadWireError::UnsupportedMapping) => {
                    report.state = SolverExecutionState::UnsupportedMapping;
                    report.payload_wire_error = Some(error);
                }
                Err(error) => {
                    report.state = SolverExecutionState::PayloadWireRejected;
                    report.payload_wire_error = Some(error);
                }
            }
        }
        SolverProtocol::ClingoStatusV1 => {
            let limits = ClingoOutputLimits::checked(
                budget.max_stdout_bytes,
                MAX_CLINGO_ANSWER_BLOCKS,
                MAX_CLINGO_ATOM_LINES,
            );
            let parsed = limits.and_then(|limits| parse_clingo_output(process.stdout, limits));
            match parsed {
                Ok(parsed) => match parsed.summary() {
                    Ok(summary) => {
                        let result = match summary.status() {
                            ClingoTerminalStatus::Satisfiable => Satisfiability::Sat,
                            ClingoTerminalStatus::Unsatisfiable => Satisfiability::Unsat,
                            ClingoTerminalStatus::Unknown => Satisfiability::Unknown,
                        };
                        report.state = SolverExecutionState::SolverReported;
                        report.logical_result = Some(LogicalResult::Satisfiability(result));
                        report.validation_basis = Some(ValidationBasis::SolverReported);
                        report.clingo_output_summary = Some(summary);
                    }
                    Err(_) => {
                        report.state = SolverExecutionState::ProtocolFailure;
                    }
                },
                Err(_) => {
                    report.state = SolverExecutionState::ProtocolFailure;
                }
            }
        }
    }
    report
}

pub(crate) fn output_witness_digest(
    stream: &[u8],
    bytes: &[u8],
    max_bytes: usize,
    reported_truncation: bool,
) -> Option<[u8; 32]> {
    use sha2::{Digest, Sha256};

    let total_len = u64::try_from(bytes.len()).ok()?;
    let prefix_len = bytes.len().min(max_bytes);
    let mut digest = Sha256::new();
    digest.update(b"epistesys.solver-output-witness.v1\0");
    digest.update((stream.len() as u64).to_le_bytes());
    digest.update(stream);
    digest.update(total_len.to_le_bytes());
    digest.update((prefix_len as u64).to_le_bytes());
    digest.update([u8::from(reported_truncation)]);
    digest.update(&bytes[..prefix_len]);
    Some(digest.finalize().into())
}
