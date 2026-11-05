//! Bounded, append-only storage for source-bound solver process reports.
//!
//! A stored `SolverReported` status is still only a runner-reported protocol
//! observation. This in-memory store does not authenticate a process, check a
//! model or proof, establish semantic truth, or grant authority.

use crate::logical::LogicalResult;
use crate::protocol::clingo::{ClingoOutputSummary, ClingoTerminalStatus};
use crate::protocol::payload::{PayloadShape, PayloadSyntaxError};
use crate::protocol::payload_wire::{PayloadWireError, PayloadWireStatus};
use crate::protocol::sexpr::SExprError;
use crate::protocol::{QueryResultKind, SolverProtocol, ValidationBasis};
use crate::runner::{
    BinaryBindingState, InvocationBindingState, InvocationDigest, ProcessCleanupState,
    ProcessRunState, QueryDigest, QuerySourceIdentity, SolverExecutionReport, SolverExecutionState,
    SolverQuery,
};
use sha2::{Digest, Sha256};

pub const DEFAULT_SOLVER_RESULT_CAPACITY: usize = 4096;
pub const MAX_SOLVER_RESULT_CAPACITY: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// A local identifier for one append in one in-memory store instance.
/// It is not a durable replay ID, cryptographic authentication token, or
/// cross-process monotonic counter.
pub struct SolverResultId([u8; 32]);

impl SolverResultId {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Caller-provided current scope for a result lookup or insert.
///
/// This value binds source/query/invocation identity only; it is not a receipt
/// that the executable or solver semantics were independently verified. A
/// separate ContextRevision, goal revision, or projection revision is not
/// represented here; do not reuse a stored report as context evidence unless
/// that context identity is independently bound by the query payload/caller.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SolverResultBinding {
    source: QuerySourceIdentity,
    query: QueryDigest,
    invocation: Option<InvocationDigest>,
}

impl SolverResultBinding {
    pub const fn from_parts(
        source: QuerySourceIdentity,
        query: QueryDigest,
        invocation: Option<InvocationDigest>,
    ) -> Self {
        Self {
            source,
            query,
            invocation,
        }
    }

    pub const fn for_query(query: &SolverQuery<'_>, invocation: Option<InvocationDigest>) -> Self {
        Self::from_parts(query.source(), query.digest(), invocation)
    }

    pub const fn source(self) -> QuerySourceIdentity {
        self.source
    }

    pub const fn query(self) -> QueryDigest {
        self.query
    }

    pub const fn invocation(self) -> Option<InvocationDigest> {
        self.invocation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultStoreError {
    InvalidCapacity,
    BindingMismatch,
    CapacityReached,
    AllocationFailure,
    ArithmeticOverflow,
    InvalidSummary,
    ResultIdCollision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// The receipt for one append; its sequence restarts with a fresh store.
pub struct ResultStoreInsert {
    id: SolverResultId,
    sequence: u64,
}

impl ResultStoreInsert {
    pub const fn id(self) -> SolverResultId {
        self.id
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultStoreLookupError {
    NotFound,
    StaleBinding,
    AmbiguousResults,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoredSolverResult {
    id: SolverResultId,
    sequence: u64,
    report_fingerprint: [u8; 32],
    binding: SolverResultBinding,
    report: SolverExecutionReport,
}

impl StoredSolverResult {
    pub const fn id(&self) -> SolverResultId {
        self.id
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Deterministic fingerprint of the stored report fields, not a complete
    /// raw-output identity when a stdout/stderr stream was truncated or only
    /// its bounded prefix was returned.
    pub const fn report_fingerprint(&self) -> &[u8; 32] {
        &self.report_fingerprint
    }

    pub const fn binding(&self) -> SolverResultBinding {
        self.binding
    }

    pub const fn report(&self) -> SolverExecutionReport {
        self.report
    }

    pub const fn claim_boundary(&self) -> &'static str {
        "bounded in-memory source/query/invocation binding for a runner-reported process result; separate context/goal/projection lineage is not bound here; not solver authenticity, proof validation, semantic truth, authority, output commit or durable replay"
    }
}

/// A bounded store that never evicts prior process evidence on capacity
/// pressure. Callers must choose a new store or explicitly manage retention in
/// a later durable-storage layer; this slice does not delete prior records.
/// The limit is per-instance entry count over fixed-size records, not a
/// ResourceBroker lease or process-wide RAM quota. A production owner must
/// reserve/account this storage separately before creating a store.
#[derive(Debug)]
pub struct ResultStore {
    entries: Vec<StoredSolverResult>,
    capacity: usize,
    next_sequence: u64,
}

impl Default for ResultStore {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            capacity: DEFAULT_SOLVER_RESULT_CAPACITY,
            next_sequence: 0,
        }
    }
}

impl ResultStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Result<Self, ResultStoreError> {
        if capacity == 0 || capacity > MAX_SOLVER_RESULT_CAPACITY {
            return Err(ResultStoreError::InvalidCapacity);
        }
        Ok(Self {
            entries: Vec::new(),
            capacity,
            next_sequence: 0,
        })
    }

    /// Reserve this store's complete entry backing before child execution.
    /// A bounded owner must separately account for this allocation.
    pub fn try_reserve_backing(&mut self) -> Result<(), ResultStoreError> {
        let additional = self
            .capacity
            .checked_sub(self.entries.len())
            .ok_or(ResultStoreError::ArithmeticOverflow)?;
        self.entries
            .try_reserve_exact(additional)
            .map_err(|_| ResultStoreError::AllocationFailure)?;
        if self.entries.capacity() > self.capacity {
            return Err(ResultStoreError::AllocationFailure);
        }
        Ok(())
    }

    /// Reserve one backing slot before executing a child. Exclusive access
    /// must be retained until append; reservation issues no evidence receipt.
    pub fn try_reserve_append(&mut self) -> Result<(), ResultStoreError> {
        if self.entries.len() >= self.capacity {
            return Err(ResultStoreError::CapacityReached);
        }
        self.next_sequence
            .checked_add(1)
            .ok_or(ResultStoreError::ArithmeticOverflow)?;
        self.entries
            .try_reserve(1)
            .map_err(|_| ResultStoreError::AllocationFailure)?;
        Ok(())
    }

    /// Bind and retain one report against the exact query supplied by the
    /// caller. Every accepted observation receives a fresh in-memory sequence
    /// and result ID; repeats and contradictory outcomes remain separately
    /// inspectable rather than being selected or overwritten.
    pub fn append(
        &mut self,
        binding: SolverResultBinding,
        report: SolverExecutionReport,
    ) -> Result<ResultStoreInsert, ResultStoreError> {
        if report.source() != binding.source()
            || report.query() != binding.query()
            || report.invocation_digest() != binding.invocation()
        {
            return Err(ResultStoreError::BindingMismatch);
        }

        let report_fingerprint = report_fingerprint(report)?;
        if self.entries.len() >= self.capacity {
            return Err(ResultStoreError::CapacityReached);
        }
        let sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(ResultStoreError::ArithmeticOverflow)?;
        let id = result_id(binding, sequence, report_fingerprint);
        if self.entries.iter().any(|entry| entry.id == id) {
            return Err(ResultStoreError::ResultIdCollision);
        }
        self.entries
            .try_reserve(1)
            .map_err(|_| ResultStoreError::AllocationFailure)?;
        self.entries.push(StoredSolverResult {
            id,
            sequence,
            report_fingerprint,
            binding,
            report,
        });
        self.next_sequence = sequence;
        Ok(ResultStoreInsert { id, sequence })
    }

    /// Convenience API for a live query. The report's invocation digest is
    /// included in the key; the query's source and payload digest are checked
    /// independently against the report before storage.
    pub fn append_for_query(
        &mut self,
        query: &SolverQuery<'_>,
        report: SolverExecutionReport,
    ) -> Result<ResultStoreInsert, ResultStoreError> {
        self.append(
            SolverResultBinding::for_query(query, report.invocation_digest()),
            report,
        )
    }

    pub fn lookup(
        &self,
        expected: SolverResultBinding,
    ) -> Result<&StoredSolverResult, ResultStoreLookupError> {
        let mut matches = self
            .entries
            .iter()
            .filter(|entry| entry.binding == expected);
        let Some(first) = matches.next() else {
            return if self.entries.iter().any(|entry| {
                entry.binding.query() == expected.query()
                    && entry.binding.source() == expected.source()
            }) {
                Err(ResultStoreLookupError::StaleBinding)
            } else {
                Err(ResultStoreLookupError::NotFound)
            };
        };
        if matches.next().is_some() {
            Err(ResultStoreLookupError::AmbiguousResults)
        } else {
            Ok(first)
        }
    }

    pub fn lookup_by_id(&self, id: SolverResultId) -> Option<&StoredSolverResult> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub fn results_for(
        &self,
        binding: SolverResultBinding,
    ) -> impl Iterator<Item = &StoredSolverResult> {
        self.entries
            .iter()
            .filter(move |entry| entry.binding == binding)
    }

    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub const fn capacity_limit(&self) -> usize {
        self.capacity
    }
}

fn result_id(
    binding: SolverResultBinding,
    sequence: u64,
    report_fingerprint: [u8; 32],
) -> SolverResultId {
    let mut digest = Sha256::new();
    digest.update(b"epistesys.tlcx.solver-result-key.v1\0");
    digest.update(binding.source().as_bytes());
    digest.update(binding.query().as_bytes());
    match binding.invocation() {
        Some(invocation) => {
            digest.update([1]);
            digest.update(invocation.as_bytes());
        }
        None => digest.update([0]),
    }
    digest.update(sequence.to_le_bytes());
    digest.update(report_fingerprint);
    SolverResultId(digest.finalize().into())
}

fn report_fingerprint(report: SolverExecutionReport) -> Result<[u8; 32], ResultStoreError> {
    let mut digest = Sha256::new();
    digest.update(b"epistesys.tlcx.solver-result-report.v1\0");
    digest.update(report.source().as_bytes());
    digest.update(report.query().as_bytes());
    if let Some(invocation) = report.invocation_digest() {
        digest.update([1]);
        digest.update(invocation.as_bytes());
    } else {
        digest.update([0]);
    }
    digest.update([execution_state_code(report.state())]);
    let completeness = report.registry_completeness();
    digest.update([
        u8::from(completeness.z3_qf_lia_registered),
        u8::from(completeness.clingo_ground_asp_registered),
    ]);
    add_option_code(&mut digest, report.process_state().map(process_state_code));
    add_option_code(
        &mut digest,
        report.binary_binding_state().map(binary_binding_code),
    );
    add_option_code(
        &mut digest,
        report
            .invocation_binding_state()
            .map(invocation_binding_code),
    );
    match report.exit_code() {
        Some(code) => {
            digest.update([1]);
            digest.update(code.to_le_bytes());
        }
        None => digest.update([0]),
    }
    digest.update([cleanup_state_code(report.cleanup_state())]);
    digest.update([
        u8::from(report.stdout_truncated()),
        u8::from(report.stderr_truncated()),
    ]);
    add_usize(&mut digest, report.stdout_bytes())?;
    add_usize(&mut digest, report.stderr_bytes())?;
    add_optional_digest(&mut digest, report.stdout_digest());
    add_optional_digest(&mut digest, report.stderr_digest());
    digest.update([
        u8::from(report.stdout_fully_observed()),
        u8::from(report.stderr_fully_observed()),
    ]);
    match report.logical_result() {
        Some(result) => {
            digest.update([1]);
            logical_result_code(result, &mut digest);
        }
        None => digest.update([0]),
    }
    match report.validation_basis() {
        Some(ValidationBasis::SolverReported) => digest.update([1]),
        None => digest.update([0]),
    }
    add_syntax_observations(&mut digest, report)?;
    Ok(digest.finalize().into())
}

/// New syntax summaries are appended only when present so the fingerprint of
/// legacy status-only reports remains byte-for-byte stable.
fn add_syntax_observations(
    digest: &mut Sha256,
    report: SolverExecutionReport,
) -> Result<(), ResultStoreError> {
    if let Some(summary) = report.payload_syntax_summary() {
        digest.update([0xa1, payload_shape_code(summary.requested_shape())]);
        digest.update(summary.root_count().to_le_bytes());
        digest.update(summary.node_count().to_le_bytes());
    }
    if let Some(error) = report.payload_syntax_error() {
        digest.update([0xa2]);
        payload_syntax_error_code(error, digest)?;
    }
    if let Some(summary) = report.clingo_output_summary() {
        add_clingo_summary(digest, summary)?;
    }
    if let Some(status) = report.payload_wire_status() {
        let code = match status {
            PayloadWireStatus::Sat => 1,
            PayloadWireStatus::Unsat => 2,
            PayloadWireStatus::Unknown => 3,
        };
        digest.update([0xa6, code]);
    }
    if let Some(summary) = report.payload_wire_summary() {
        let status = match summary.status() {
            PayloadWireStatus::Sat => 1,
            PayloadWireStatus::Unsat => 2,
            PayloadWireStatus::Unknown => 3,
        };
        digest.update([0xa4, status, payload_shape_code(summary.shape())]);
        digest.update(summary.status_span().start().to_le_bytes());
        digest.update(summary.status_span().end().to_le_bytes());
        digest.update(summary.payload_span().start().to_le_bytes());
        digest.update(summary.payload_span().end().to_le_bytes());
        digest.update(summary.stdout_len().to_le_bytes());
        digest.update(summary.stdout_digest());
    }
    if let Some(error) = report.payload_wire_error() {
        digest.update([0xa5]);
        match error {
            PayloadWireError::UnsupportedMapping => digest.update([1]),
            PayloadWireError::Syntax(error) => {
                digest.update([2]);
                sexpr_error_code(error, digest)?;
            }
            PayloadWireError::UnexpectedRootCount => digest.update([3]),
            PayloadWireError::InvalidStatus => digest.update([4]),
            PayloadWireError::StatusKindMismatch => digest.update([5]),
            PayloadWireError::UnsupportedShape => digest.update([6]),
            PayloadWireError::OffsetOverflow => digest.update([7]),
        }
    }

    Ok(())
}

fn add_clingo_summary(
    digest: &mut Sha256,
    summary: ClingoOutputSummary,
) -> Result<(), ResultStoreError> {
    let status = match summary.status() {
        ClingoTerminalStatus::Satisfiable => 1,
        ClingoTerminalStatus::Unsatisfiable => 2,
        ClingoTerminalStatus::Unknown => 3,
    };
    digest.update([0xa3, status]);
    digest.update(summary.answer_count().to_le_bytes());
    digest.update(summary.atom_line_count().to_le_bytes());
    digest.update([
        summary.recorded_answer_span_count(),
        u8::from(summary.spans_truncated()),
    ]);
    for index in 0..usize::from(summary.recorded_answer_span_count()) {
        let span = summary
            .answer_span(index)
            .ok_or(ResultStoreError::InvalidSummary)?;
        digest.update(span.number().to_le_bytes());
        add_usize(digest, span.span().start)?;
        add_usize(digest, span.span().end)?;
    }
    Ok(())
}

fn payload_shape_code(shape: PayloadShape) -> u8 {
    match shape {
        PayloadShape::Model => 1,
        PayloadShape::Proof => 2,
        PayloadShape::Counterexample => 3,
    }
}

fn payload_syntax_error_code(
    error: PayloadSyntaxError,
    digest: &mut Sha256,
) -> Result<(), ResultStoreError> {
    match error {
        PayloadSyntaxError::UnsupportedMapping {
            protocol,
            result_kind,
        } => {
            let protocol = match protocol {
                SolverProtocol::SmtLibStatusV1 => 1,
                SolverProtocol::ClingoStatusV1 => 2,
                SolverProtocol::SmtLibPayloadV1 => 3,
                SolverProtocol::SmtLibPayloadWireV2 => 4,
            };
            digest.update([1, protocol, result_kind_code(result_kind)]);
        }
        PayloadSyntaxError::Syntax(error) => {
            digest.update([2]);
            sexpr_error_code(error, digest)?;
        }
        PayloadSyntaxError::SummaryCountOverflow => digest.update([3]),
    }
    Ok(())
}

fn sexpr_error_code(error: SExprError, digest: &mut Sha256) -> Result<(), ResultStoreError> {
    let (code, offset) = match error {
        SExprError::InvalidLimits => (1, None),
        SExprError::InputTooLarge { actual, limit } => {
            digest.update([2]);
            add_usize(digest, actual)?;
            add_usize(digest, limit)?;
            return Ok(());
        }
        SExprError::InvalidUtf8 { valid_up_to } => (3, Some(valid_up_to)),
        SExprError::NodeLimitExceeded => (4, None),
        SExprError::DepthLimitExceeded => (5, None),
        SExprError::AllocationFailed => (6, None),
        SExprError::UnexpectedClose { offset } => (7, Some(offset)),
        SExprError::UnclosedList { offset } => (8, Some(offset)),
        SExprError::UnclosedString { offset } => (9, Some(offset)),
        SExprError::UnclosedQuotedSymbol { offset } => (10, Some(offset)),
        SExprError::InvalidQuotedSymbol { offset } => (11, Some(offset)),
        SExprError::MissingSeparator { offset } => (12, Some(offset)),
        SExprError::InvalidToken { offset } => (13, Some(offset)),
    };
    digest.update([code]);
    if let Some(offset) = offset {
        add_usize(digest, offset)?;
    }
    Ok(())
}

fn result_kind_code(kind: QueryResultKind) -> u8 {
    match kind {
        QueryResultKind::Satisfiability => 1,
        QueryResultKind::Entailment => 2,
        QueryResultKind::StableModelExistence => 3,
        QueryResultKind::ModelCheck => 4,
        QueryResultKind::Inference => 5,
        QueryResultKind::Optimization => 6,
        QueryResultKind::Intervention => 7,
        QueryResultKind::SpatialRelation => 8,
        QueryResultKind::ModelPayload => 9,
        QueryResultKind::ProofPayload => 10,
        QueryResultKind::CounterexamplePayload => 11,
    }
}

fn add_option_code(digest: &mut Sha256, code: Option<u8>) {
    match code {
        Some(code) => digest.update([1, code]),
        None => digest.update([0]),
    }
}

fn add_optional_digest(digest: &mut Sha256, value: Option<[u8; 32]>) {
    match value {
        Some(value) => {
            digest.update([1]);
            digest.update(value);
        }
        None => digest.update([0]),
    }
}

fn add_usize(digest: &mut Sha256, value: usize) -> Result<(), ResultStoreError> {
    let value = u64::try_from(value).map_err(|_| ResultStoreError::ArithmeticOverflow)?;
    digest.update(value.to_le_bytes());
    Ok(())
}

fn execution_state_code(state: SolverExecutionState) -> u8 {
    match state {
        SolverExecutionState::SolverReported => 1,
        SolverExecutionState::UnsupportedProfile => 2,
        SolverExecutionState::ResourceRejected => 3,
        SolverExecutionState::NotStarted => 4,
        SolverExecutionState::StartedUnknown => 5,
        SolverExecutionState::TimedOut => 6,
        SolverExecutionState::Cancelled => 7,
        SolverExecutionState::Failed => 8,
        SolverExecutionState::OutputTruncated => 9,
        SolverExecutionState::CleanupUnknown => 10,
        SolverExecutionState::BindingMismatch => 11,
        SolverExecutionState::UnsupportedMapping => 12,
        SolverExecutionState::ProtocolFailure => 13,
        SolverExecutionState::PayloadSyntaxObserved => 14,
        SolverExecutionState::PayloadSyntaxRejected => 15,
        SolverExecutionState::PayloadWireObserved => 17,
        SolverExecutionState::PayloadWireRejected => 18,
        SolverExecutionState::AmbiguousRegistration => 16,
    }
}

fn process_state_code(state: ProcessRunState) -> u8 {
    match state {
        ProcessRunState::Completed => 1,
        ProcessRunState::NotStarted => 2,
        ProcessRunState::TimedOut => 3,
        ProcessRunState::ResourceExhaustedBeforeStart => 4,
        ProcessRunState::ResourceExhaustedAfterStartUnknown => 5,
        ProcessRunState::Unsupported => 6,
        ProcessRunState::Failed => 7,
    }
}

fn binary_binding_code(_: BinaryBindingState) -> u8 {
    1
}

fn invocation_binding_code(_: InvocationBindingState) -> u8 {
    1
}

fn cleanup_state_code(state: ProcessCleanupState) -> u8 {
    match state {
        ProcessCleanupState::RunnerReportedNotStarted => 1,
        ProcessCleanupState::RunnerReportedVerified => 2,
        ProcessCleanupState::RunnerReportedUnknown => 3,
    }
}

fn logical_result_code(result: LogicalResult, digest: &mut Sha256) {
    match result {
        LogicalResult::Satisfiability(value) => {
            digest.update([
                1,
                match value {
                    crate::logical::Satisfiability::Sat => 1,
                    crate::logical::Satisfiability::Unsat => 2,
                    crate::logical::Satisfiability::Unknown => 3,
                },
            ]);
        }
        LogicalResult::Entailment { support, models } => {
            digest.update([
                2,
                support_code(support),
                match models {
                    crate::logical::Quantification::Exists => 1,
                    crate::logical::Quantification::ForAll => 2,
                },
            ]);
        }
        LogicalResult::ModelCheck { support, traces } => {
            digest.update([
                3,
                support_code(support),
                match traces {
                    crate::logical::TraceDomain::Finite => 1,
                    crate::logical::TraceDomain::Infinite => 2,
                },
            ]);
        }
        LogicalResult::Inference { support, mode } => {
            digest.update([
                4,
                support_code(support),
                match mode {
                    crate::logical::ConsequenceMode::Brave => 1,
                    crate::logical::ConsequenceMode::Cautious => 2,
                },
            ]);
        }
        LogicalResult::Optimization(objective) => match objective {
            crate::logical::ObjectiveResult::Bounded { lower, upper } => {
                digest.update([5, 1]);
                digest.update(lower.to_le_bytes());
                digest.update(upper.to_le_bytes());
            }
            crate::logical::ObjectiveResult::Unbounded => digest.update([5, 2]),
            crate::logical::ObjectiveResult::Unknown => digest.update([5, 3]),
        },
        LogicalResult::Intervention(support) => digest.update([6, support_code(support)]),
        LogicalResult::SpatialRelation(support) => digest.update([7, support_code(support)]),
    }
}

fn support_code(support: crate::logical::Support) -> u8 {
    match support {
        crate::logical::Support::Supported => 1,
        crate::logical::Support::Refuted => 2,
        crate::logical::Support::Neither => 3,
        crate::logical::Support::Both => 4,
    }
}
