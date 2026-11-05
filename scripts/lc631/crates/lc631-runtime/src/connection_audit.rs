//! Bounded audit of declared connections and the actual runtime solve entry.
//! Registry integrity is not evidence that any declared edge executed.
#![forbid(unsafe_code)]

use crate::{connection_contracts, solve, ConnectionContract, RuntimeReport, SolveRequest};
use lc631_core::context::{ContextRevision, SourceRevision, TaskKind};

pub const REQUIRED_EDGE_COUNT: usize = 20;
pub const MAX_AUDIT_SOURCE_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EdgeId(u8);

impl EdgeId {
    pub const fn checked(number: u8) -> Result<Self, AuditError> {
        if number >= 1 && number <= 20 {
            Ok(Self(number))
        } else {
            Err(AuditError::UnknownEdge)
        }
    }

    pub const fn number(self) -> u8 {
        self.0
    }

    fn parse(text: &str) -> Result<Self, AuditError> {
        match text.as_bytes() {
            [b'E', tens @ b'0'..=b'2', ones @ b'0'..=b'9'] => {
                Self::checked((tens - b'0') * 10 + (ones - b'0'))
            }
            _ => Err(AuditError::UnknownEdge),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditError {
    RegistryCount,
    UnknownEdge,
    DuplicateEdge(EdgeId),
    EndpointMismatch(EdgeId),
    EmptySource,
    SourceBudget,
    SourceContextMismatch,
    EntryBindingMismatch,
    StaleObservation,
}

/// Versioned endpoint shape, independently matched against the runtime registry.
/// States are deliberately read from the current registry, never fixed here.
const ENDPOINTS_V1: [(&str, &str); REQUIRED_EDGE_COUNT] = [
    ("host_ingress", "source_store"),
    ("source_store", "deep_grammar_context_ir"),
    ("context_ir", "context_graph"),
    ("context_graph", "solver_projection"),
    ("solver_projection", "supervised_process"),
    ("solver_result", "result_validator"),
    ("validated_observation", "translation_ledger"),
    ("candidate_provider", "creative_assessment"),
    ("loss_and_gain", "frontier"),
    ("frontier_and_geometry", "scheduler"),
    ("scheduler", "resource_broker_backend"),
    ("continuous_proposal", "discrete_validation_next_epoch"),
    ("selection", "program_ir_task_plan"),
    ("coding_candidate_permit", "validation_producers"),
    ("repair_checkpoint", "resume"),
    ("media_rpa_nlp", "candidate_premises"),
    ("observed_numeric_batch", "gpu_cpu_final_check"),
    ("decision_artifact", "exact_renderer"),
    ("render", "host_stages_and_replay"),
    ("artifact_store", "cache_journal_security_consumers"),
];

#[derive(Clone, Copy, Debug)]
pub struct DeclaredInventory<'a> {
    contracts: &'a [ConnectionContract],
    positions: [usize; REQUIRED_EDGE_COUNT],
}

impl<'a> DeclaredInventory<'a> {
    pub fn contracts(self) -> &'a [ConnectionContract] {
        self.contracts
    }

    pub fn contract(self, id: EdgeId) -> &'a ConnectionContract {
        &self.contracts[self.positions[usize::from(id.0 - 1)]]
    }

    pub const fn claim_boundary(self) -> &'static str {
        "declared endpoint shape only; no edge execution evidence"
    }
}

/// Checks complete bounded endpoint shape only; reports cannot mint observations.
pub fn check_reported_registry(
    contracts: &[ConnectionContract],
) -> Result<DeclaredInventory<'_>, AuditError> {
    if contracts.len() != REQUIRED_EDGE_COUNT {
        return Err(AuditError::RegistryCount);
    }
    let mut positions = [0; REQUIRED_EDGE_COUNT];
    let mut seen = [false; REQUIRED_EDGE_COUNT];
    for (position, contract) in contracts.iter().enumerate() {
        let id = EdgeId::parse(contract.id)?;
        let index = usize::from(id.0 - 1);
        if seen[index] {
            return Err(AuditError::DuplicateEdge(id));
        }
        if (contract.producer, contract.consumer) != ENDPOINTS_V1[index] {
            return Err(AuditError::EndpointMismatch(id));
        }
        seen[index] = true;
        positions[index] = position;
    }
    // Twenty unique IDs in 1..=20 imply full coverage; the empty case failed above.
    Ok(DeclaredInventory {
        contracts,
        positions,
    })
}

/// Actual entry return from this process; no public reported constructor.
/// This does not authenticate a host, prove correctness, or observe edge traces.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeEntryObservation {
    report: RuntimeReport,
    source: SourceRevision,
}

impl RuntimeEntryObservation {
    pub const fn report(self) -> RuntimeReport {
        self.report
    }

    /// Check whether the caller is still consuming the same source/context/task.
    /// Matching bytes do not restore freshness of external tools or authority.
    pub fn inspect_current(
        self,
        source: &str,
        context: ContextRevision,
        task: TaskKind,
    ) -> Result<(), AuditError> {
        check_source(source)?;
        if SourceRevision::of(source) != self.source
            || self.report.context != context
            || self.report.task != task
        {
            return Err(AuditError::StaleObservation);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EdgeObservation {
    Unobserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditGap {
    RequiredEdgeExecutionsUnobserved,
}

/// This report keeps registry declarations separate from a real entry return.
#[derive(Clone, Copy, Debug)]
pub struct ConnectionAudit {
    registry: DeclaredInventory<'static>,
    entry: RuntimeEntryObservation,
}

impl ConnectionAudit {
    pub const fn registry(self) -> DeclaredInventory<'static> {
        self.registry
    }
    pub const fn entry(self) -> RuntimeEntryObservation {
        self.entry
    }

    pub const fn edge_observation(self, _id: EdgeId) -> EdgeObservation {
        // solve's RuntimeReport contains no E01..E20 execution witnesses.
        EdgeObservation::Unobserved
    }

    pub const fn gap(self) -> AuditGap {
        AuditGap::RequiredEdgeExecutionsUnobserved
    }
}

/// Invoke the real public runtime entry exactly once, under the source bound.
/// No doctor, simulated driver, caller-provided status or entry callback is used.
pub fn audit_current_entry(
    source: &str,
    context: ContextRevision,
    task: TaskKind,
) -> Result<ConnectionAudit, AuditError> {
    check_source(source)?;
    let registry = check_reported_registry(connection_contracts())?;
    let request = SolveRequest::checked(source, context, task)
        .map_err(|_| AuditError::SourceContextMismatch)?;
    let report = solve(request);
    if report.context != context || report.task != task || report.source_bytes != source.len() {
        return Err(AuditError::EntryBindingMismatch);
    }
    Ok(ConnectionAudit {
        registry,
        entry: RuntimeEntryObservation {
            report,
            source: context.source(),
        },
    })
}

fn check_source(source: &str) -> Result<(), AuditError> {
    if source.len() > MAX_AUDIT_SOURCE_BYTES {
        return Err(AuditError::SourceBudget);
    }
    if source.is_empty() {
        return Err(AuditError::EmptySource);
    }
    Ok(())
}
