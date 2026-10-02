use crate::connection_graph::{
    build_connection_graph_from_trees, StaticConnectionGraphReport, StaticConnectionState,
};
use crate::{
    coding_evidence_payload_digest, coding_evidence_scope, not_started_request, run_bounded_tool,
    BoundedToolRequest, BoundedToolResult, CodingEvidenceClaim, CodingEvidenceKind,
    CodingEvidenceProducer, ToolRunState,
};
use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, SubjectRevision,
};
use lc631_tldg::{analyze_dg1, Dg1Budget, Dg1ConditionKind, Dg1RequirementPolarity};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CodingConnectionPlan {
    pub schema_version: String,
    pub source: String,
    pub source_revision: String,
    pub requirement_id: u32,
    #[serde(default)]
    pub coding_requirement_id: Option<String>,
    #[serde(default)]
    pub coding_task_id: Option<String>,
    #[serde(default)]
    pub coding_target_ref: Option<String>,
    #[serde(default)]
    pub coding_target_digest: Option<String>,
    pub expected_polarity: String,
    pub expected_condition_kind: Option<String>,
    pub expected_scope: Option<String>,
    pub implementation_path: String,
    pub implementation_digest: String,
    pub implementation_symbol: String,
    pub router_path: String,
    pub router_digest: String,
    pub executable_path: String,
    pub executable_digest: String,
    pub command: String,
    pub arguments: Vec<String>,
    pub expected_json_pointer: String,
    pub expected_json_value: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    Observed,
    Held,
}

#[derive(Clone, Debug, Serialize)]
pub struct CodingConnectionReport {
    pub schema_version: &'static str,
    pub source_revision: String,
    pub requirement_id: u32,
    pub implementation_digest: String,
    pub router_digest: String,
    pub executable_digest: String,
    pub connection_graph: StaticConnectionGraphReport,
    pub implementation_symbol_found: bool,
    pub command_registered_candidate: bool,
    pub runtime_command_observed: bool,
    pub acceptance_matched: bool,
    pub runtime_trace: Option<ConnectionRuntimeTrace>,
    pub process: BoundedToolResult,
    pub status: ConnectionStatus,
    pub authority_created: bool,
    pub output_commit_allowed: bool,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConnectionAcceptanceOracle {
    pub schema_version: &'static str,
    pub oracle_revision: &'static str,
    pub source: &'static str,
    pub json_pointer: String,
    pub expected_value_digest: String,
    pub matched: bool,
    pub candidate_generated: bool,
    pub independent_semantic_gold: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConnectionRuntimeTrace {
    pub schema_version: &'static str,
    pub run_id: String,
    pub trace_digest: String,
    pub plan_digest: String,
    pub source_revision: String,
    pub requirement_id: u32,
    pub connection_graph_digest: String,
    pub route_edge_id: String,
    pub producer_node_id: String,
    pub consumer_node_id: String,
    pub executable_path: String,
    pub declared_executable_digest: String,
    pub observed_executable_digest: Option<String>,
    pub arguments_digest: String,
    pub input_digest: String,
    pub cwd_digest: String,
    pub environment_digest: String,
    pub stdout_digest: String,
    pub stderr_digest: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub network_policy: String,
    pub network_os_enforced: bool,
    pub filesystem_write_monitor: &'static str,
    pub acceptance_oracle: ConnectionAcceptanceOracle,
    pub claim_boundary: &'static str,
}

#[derive(Serialize)]
struct ConnectionRuntimeTraceDigestInput<'a> {
    schema_version: &'static str,
    run_id: &'a str,
    plan_digest: &'a str,
    source_revision: &'a str,
    requirement_id: u32,
    connection_graph_digest: &'a str,
    route_edge_id: &'a str,
    producer_node_id: &'a str,
    consumer_node_id: &'a str,
    executable_path: &'a str,
    declared_executable_digest: &'a str,
    observed_executable_digest: Option<&'a str>,
    arguments_digest: &'a str,
    input_digest: &'a str,
    cwd_digest: &'a str,
    environment_digest: &'a str,
    stdout_digest: &'a str,
    stderr_digest: &'a str,
    exit_code: Option<i32>,
    timed_out: bool,
    stdout_truncated: bool,
    stderr_truncated: bool,
    network_policy: &'a str,
    network_os_enforced: bool,
    filesystem_write_monitor: &'static str,
    acceptance_oracle: &'a ConnectionAcceptanceOracle,
}

/// Opaque local token created only after the registered command was executed
/// and its output matched the source-bound connection plan. It is not evidence
/// of compiler correctness or general runtime behavior.
pub struct VerifiedConnectionObservation {
    plan: CodingConnectionPlan,
    report: CodingConnectionReport,
    producer_run_digest: String,
    observation_digest: String,
}

impl VerifiedConnectionObservation {
    pub fn report(&self) -> &CodingConnectionReport {
        &self.report
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ConnectionError {
    EvidenceIssuerMismatch,
    ObservationNotQualified,
    PublicReportNotEvidence,
    InvalidPlan,
    PathUnsafe,
    CommandNotRegistered,
    ExecutableNotCurrent,
    FileUnavailable,
    StaleArtifact,
    RustSyntaxError,
    OutputInvalid,
    ReceiptIssuance,
    StaticConnectionGraph(String),
}

/// Legacy compatibility surface; public reports are caller-constructible and
/// therefore intentionally cannot issue evidence.
pub fn issue_connection_evidence(
    _plan: &CodingConnectionPlan,
    _report: &CodingConnectionReport,
    _issuer: &ReceiptIssuer,
    _now_epoch: u64,
) -> Result<Vec<CodingEvidenceClaim>, ConnectionError> {
    Err(ConnectionError::PublicReportNotEvidence)
}

pub fn observe_connection_plan_from_cli(
    project_root: &Path,
    plan: &CodingConnectionPlan,
) -> Result<VerifiedConnectionObservation, ConnectionError> {
    let root = project_root
        .canonicalize()
        .map_err(|_| ConnectionError::PathUnsafe)?;
    if !registered_read_only_command(plan) {
        return Err(ConnectionError::CommandNotRegistered);
    }
    let planned_executable = checked_path(&root, &plan.executable_path)?;
    let current_executable = env::current_exe()
        .map_err(|_| ConnectionError::ExecutableNotCurrent)?
        .canonicalize()
        .map_err(|_| ConnectionError::ExecutableNotCurrent)?;
    if planned_executable != current_executable {
        return Err(ConnectionError::ExecutableNotCurrent);
    }
    let report = verify_connection_plan_internal(&root, plan, true)?;
    if report.status != ConnectionStatus::Observed
        || !report.acceptance_matched
        || !report.runtime_command_observed
        || report.process.state != ToolRunState::Observed
        || report.process.exit_code != Some(0)
        || report.process.stdout_truncated
        || report.process.stderr_truncated
        || report.connection_graph.state != StaticConnectionState::Connected
    {
        return Err(ConnectionError::ObservationNotQualified);
    }
    let trace = report
        .runtime_trace
        .as_ref()
        .filter(|trace| {
            trace.acceptance_oracle.matched
                && trace.acceptance_oracle.oracle_revision == "lc631-dg1-doctor-output-contract.v1"
                && !trace.acceptance_oracle.candidate_generated
                && !trace.acceptance_oracle.independent_semantic_gold
        })
        .ok_or(ConnectionError::ObservationNotQualified)?;
    let producer_run_digest = trace.trace_digest.clone();
    let observation_digest = stable_sha256(&format!(
        "dgcl-production-connection-observation.v2\0{}\0{}\0{}",
        trace.run_id, trace.trace_digest, trace.connection_graph_digest
    ));
    Ok(VerifiedConnectionObservation {
        plan: plan.clone(),
        report,
        producer_run_digest,
        observation_digest,
    })
}

/// Issue exactly one production-connection claim from an opaque runtime
/// observation. The verifier must come from the caller's pre-provisioned trust
/// root; this API does not establish that root's provenance.
pub fn issue_observed_connection_evidence(
    observation: VerifiedConnectionObservation,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
) -> Result<CodingEvidenceClaim, ConnectionError> {
    if issuer.key_fingerprint() != verifier.key_fingerprint() {
        return Err(ConnectionError::EvidenceIssuerMismatch);
    }
    let plan = &observation.plan;
    let report = &observation.report;
    let Some(trace) = report.runtime_trace.as_ref() else {
        return Err(ConnectionError::ObservationNotQualified);
    };
    let expected_plan_digest =
        stable_sha256(&serde_json::to_string(plan).map_err(|_| ConnectionError::InvalidPlan)?);
    let expected_oracle_digest = stable_sha256(
        &serde_json::to_string(&plan.expected_json_value)
            .map_err(|_| ConnectionError::InvalidPlan)?,
    );
    if plan.source_revision != stable_sha256(&plan.source)
        || !valid_action_binding(plan)
        || report.status != ConnectionStatus::Observed
        || !report.acceptance_matched
        || report.source_revision != plan.source_revision
        || report.requirement_id != plan.requirement_id
        || report.implementation_digest != plan.implementation_digest
        || report.router_digest != plan.router_digest
        || report.executable_digest != plan.executable_digest
        || report.connection_graph.state != StaticConnectionState::Connected
        || !report.implementation_symbol_found
        || !report.command_registered_candidate
        || !report.runtime_command_observed
        || report.process.state != ToolRunState::Observed
        || report.process.exit_code != Some(0)
        || report.process.stdout_truncated
        || report.process.stderr_truncated
        || trace.plan_digest != expected_plan_digest
        || trace.source_revision != plan.source_revision
        || trace.requirement_id != plan.requirement_id
        || trace.connection_graph_digest != report.connection_graph.graph_digest
        || trace.input_digest != stable_sha256(&plan.source)
        || !trace.acceptance_oracle.matched
        || trace.acceptance_oracle.oracle_revision != "lc631-dg1-doctor-output-contract.v1"
        || trace.acceptance_oracle.json_pointer != plan.expected_json_pointer
        || trace.acceptance_oracle.expected_value_digest != expected_oracle_digest
        || trace.acceptance_oracle.candidate_generated
        || trace.acceptance_oracle.independent_semantic_gold
    {
        return Err(ConnectionError::ObservationNotQualified);
    }
    let mut claim = CodingEvidenceClaim {
        requirement_id: plan.requirement_id,
        coding_requirement_id: plan.coding_requirement_id.clone(),
        coding_task_id: plan.coding_task_id.clone(),
        kind: CodingEvidenceKind::ProductionConnection,
        source_revision: plan.source_revision.clone(),
        target: plan
            .coding_target_ref
            .clone()
            .unwrap_or_else(|| plan.router_path.clone()),
        target_digest: plan
            .coding_target_digest
            .clone()
            .unwrap_or_else(|| plan.router_digest.clone()),
        producer: Some(CodingEvidenceProducer::DeclaredConnectionObserver),
        producer_run_digest: Some(observation.producer_run_digest),
        observation_digest: Some(observation.observation_digest),
        lifecycle_lease: None,
        attestation: None,
    };
    let subject = SubjectRevision::checked(&claim.source_revision)
        .map_err(|_| ConnectionError::ReceiptIssuance)?;
    let scope = ReceiptScope::checked(coding_evidence_scope(&claim))
        .map_err(|_| ConnectionError::ReceiptIssuance)?;
    let payload = coding_evidence_payload_digest(&claim);
    let expires_at = now_epoch
        .checked_add(300)
        .ok_or(ConnectionError::ReceiptIssuance)?;
    let wire = issuer
        .issue(
            ReceiptClass::Closure,
            subject.clone(),
            scope.clone(),
            payload.clone(),
            now_epoch,
            Some(expires_at),
            None,
        )
        .map_err(|_| ConnectionError::ReceiptIssuance)?;
    claim.attestation = Some(wire);
    Ok(claim)
}

pub fn verify_connection_plan(
    project_root: &Path,
    plan: &CodingConnectionPlan,
) -> Result<CodingConnectionReport, ConnectionError> {
    verify_connection_plan_internal(project_root, plan, false)
}

/// Execute only the fixed read-only DG1 diagnostic through the currently
/// running LC631 CLI image. The static observer never spawns a plan-selected
/// process; a supplied executable hash is identity evidence, not permission.
pub fn verify_connection_plan_from_cli(
    project_root: &Path,
    plan: &CodingConnectionPlan,
) -> Result<CodingConnectionReport, ConnectionError> {
    Ok(observe_connection_plan_from_cli(project_root, plan)?.report)
}

fn registered_read_only_command(plan: &CodingConnectionPlan) -> bool {
    plan.command == "lc631-dg1-doctor"
        && plan.arguments.len() == 2
        && plan.arguments[0] == "--prompt"
        && plan.arguments[1] == plan.source
        && plan.expected_json_pointer == "/payload/schema_version"
        && plan.expected_json_value == Value::String("epistesys-dg1-parse.v1".into())
        && plan.implementation_path == "scripts/lc631/crates/lc631-tldg/src/dg1.rs"
        && plan.router_path == "scripts/lc631/crates/lc631-cli/src/lib.rs"
        && plan.implementation_symbol == "analyze_dg1"
}

fn verify_connection_plan_internal(
    project_root: &Path,
    plan: &CodingConnectionPlan,
    execute_registered_runtime: bool,
) -> Result<CodingConnectionReport, ConnectionError> {
    if plan.schema_version != "epistesys-coding-connection-plan.v1"
        || plan.source_revision != stable_sha256(&plan.source)
        || plan.requirement_id == 0
        || !valid_action_binding(plan)
        || plan.arguments.len() > 64
        || plan.command.trim().is_empty()
        || !plan.expected_json_pointer.starts_with("/payload/")
    {
        return Err(ConnectionError::InvalidPlan);
    }
    let dg1 = analyze_dg1(&plan.source, Dg1Budget::default())
        .map_err(|_| ConnectionError::InvalidPlan)?;
    let Some(requirement) = dg1
        .requirement_candidates
        .iter()
        .find(|candidate| candidate.id == plan.requirement_id)
    else {
        return Err(ConnectionError::InvalidPlan);
    };
    if plan.expected_polarity != polarity_name(requirement.polarity)
        || plan.expected_condition_kind.as_deref() != requirement.condition_kind.map(condition_name)
        || plan.expected_scope != requirement.scope
    {
        return Err(ConnectionError::InvalidPlan);
    }
    let root = project_root
        .canonicalize()
        .map_err(|_| ConnectionError::PathUnsafe)?;
    let implementation = checked_path(&root, &plan.implementation_path)?;
    let router = checked_path(&root, &plan.router_path)?;
    let executable = checked_path(&root, &plan.executable_path)?;
    if !executable.is_file() || !implementation.is_file() || !router.is_file() {
        return Err(ConnectionError::FileUnavailable);
    }
    if !valid_digest(&plan.implementation_digest)
        || !valid_digest(&plan.router_digest)
        || !valid_digest(&plan.executable_digest)
        || digest_file(&implementation)? != plan.implementation_digest
        || digest_file(&router)? != plan.router_digest
        || digest_file(&executable)? != plan.executable_digest
    {
        return Err(ConnectionError::StaleArtifact);
    }
    let implementation_text = read_bounded_rust(&implementation)?;
    let router_text = read_bounded_rust(&router)?;
    let implementation_tree = analyze_dg1(&implementation_text, Dg1Budget::default())
        .map_err(|_| ConnectionError::RustSyntaxError)?;
    let router_tree = analyze_dg1(&router_text, Dg1Budget::default())
        .map_err(|_| ConnectionError::RustSyntaxError)?;
    if implementation_tree
        .rust_trees
        .first()
        .is_none_or(|tree| tree.root_has_error)
        || router_tree
            .rust_trees
            .first()
            .is_none_or(|tree| tree.root_has_error)
    {
        return Err(ConnectionError::RustSyntaxError);
    }
    let implementation_tree = implementation_tree
        .rust_trees
        .first()
        .ok_or(ConnectionError::RustSyntaxError)?;
    let router_tree = router_tree
        .rust_trees
        .first()
        .ok_or(ConnectionError::RustSyntaxError)?;
    let connection_graph = build_connection_graph_from_trees(
        plan,
        &dg1,
        &implementation_text,
        implementation_tree,
        &router_text,
        router_tree,
    )
    .map_err(|error| ConnectionError::StaticConnectionGraph(format!("{error:?}")))?;
    let implementation_symbol_found = connection_graph.definition_count == 1;
    let command_registered_candidate = connection_graph.route_arm_count > 0;
    let executable_name = executable.to_str().ok_or(ConnectionError::PathUnsafe)?;
    let argv = std::iter::once(plan.command.as_str())
        .chain(plan.arguments.iter().map(String::as_str))
        .collect::<Vec<_>>();
    let process = if execute_registered_runtime
        && command_registered_candidate
        && connection_graph.state == StaticConnectionState::Connected
        && registered_read_only_command(plan)
    {
        run_bounded_tool(BoundedToolRequest {
            program: executable_name,
            args: &argv,
            stdin: None,
            timeout_ms: 60_000,
            stdout_limit: 1_048_576,
            stderr_limit: 4_096,
        })
    } else {
        let args = argv
            .iter()
            .map(|argument| (*argument).to_string())
            .collect::<Vec<_>>();
        not_started_request(
            executable_name,
            &args,
            if execute_registered_runtime {
                "static_connection_graph_not_closed"
            } else {
                "execution_not_requested_by_static_plan_observer"
            },
        )
    };
    if digest_file(&implementation)? != plan.implementation_digest
        || digest_file(&router)? != plan.router_digest
        || digest_file(&executable)? != plan.executable_digest
    {
        return Err(ConnectionError::StaleArtifact);
    }
    let runtime_command_observed = process.state == ToolRunState::Observed
        && process.exit_code == Some(0)
        && !process.timed_out
        && !process.stdout_truncated
        && !process.stderr_truncated;
    let response: Option<Value> = if runtime_command_observed {
        serde_json::from_slice(&process.stdout).ok()
    } else {
        None
    };
    let acceptance_matched = response.as_ref().is_some_and(|value| {
        value.get("command") == Some(&Value::String(plan.command.clone()))
            && value.pointer(&plan.expected_json_pointer) == Some(&plan.expected_json_value)
    });
    let runtime_trace = if runtime_command_observed {
        Some(build_connection_runtime_trace(
            plan,
            &connection_graph,
            &process,
            acceptance_matched,
        )?)
    } else {
        None
    };
    Ok(CodingConnectionReport {
        schema_version: "epistesys-coding-connection.v1",
        source_revision: plan.source_revision.clone(),
        requirement_id: plan.requirement_id,
        implementation_digest: plan.implementation_digest.clone(),
        router_digest: plan.router_digest.clone(),
        executable_digest: plan.executable_digest.clone(),
        connection_graph: connection_graph.clone(),
        implementation_symbol_found,
        command_registered_candidate,
        runtime_command_observed,
        acceptance_matched,
        runtime_trace,
        process,
        status: if connection_graph.state == StaticConnectionState::Connected
            && implementation_symbol_found
            && command_registered_candidate
            && runtime_command_observed && acceptance_matched {
            ConnectionStatus::Observed
        } else { ConnectionStatus::Held },
        authority_created: false,
        output_commit_allowed: false,
        claim_boundary: "source-revision-bound declared Rust CST path plus exact read-only command/output observation; cross-module typing, whole-repository reachability, semantic correctness, and host-wide enforcement remain separate",
    })
}

fn valid_action_binding(plan: &CodingConnectionPlan) -> bool {
    let fields = [
        plan.coding_requirement_id.is_some(),
        plan.coding_task_id.is_some(),
        plan.coding_target_ref.is_some(),
        plan.coding_target_digest.is_some(),
    ];
    if fields.iter().all(|present| !present) {
        return true;
    }
    if !fields.iter().all(|present| *present) {
        return false;
    }
    let requirement_id = plan.coding_requirement_id.as_deref().unwrap_or_default();
    let task_id = plan.coding_task_id.as_deref().unwrap_or_default();
    let target_ref = plan.coding_target_ref.as_deref().unwrap_or_default();
    let target_digest = plan.coding_target_digest.as_deref().unwrap_or_default();
    valid_digest(requirement_id)
        && valid_digest(task_id)
        && valid_digest(target_digest)
        && target_ref == format!("dgcl/action/{requirement_id}")
}

static CONNECTION_TRACE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn build_connection_runtime_trace(
    plan: &CodingConnectionPlan,
    graph: &StaticConnectionGraphReport,
    process: &BoundedToolResult,
    acceptance_matched: bool,
) -> Result<ConnectionRuntimeTrace, ConnectionError> {
    if graph.state != StaticConnectionState::Connected
        || !registered_read_only_command(plan)
        || process.state != ToolRunState::Observed
    {
        return Err(ConnectionError::ObservationNotQualified);
    }
    let route_edges = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == crate::ConnectionGraphEdgeKind::RouteDispatchesToCall
                && edge.state == StaticConnectionState::Connected
        })
        .collect::<Vec<_>>();
    if route_edges.len() != 1 {
        return Err(ConnectionError::ObservationNotQualified);
    }
    let route_edge = route_edges[0];
    let plan_digest =
        stable_sha256(&serde_json::to_string(plan).map_err(|_| ConnectionError::InvalidPlan)?);
    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ConnectionError::ObservationNotQualified)?
        .as_nanos();
    let sequence = CONNECTION_TRACE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let run_id = stable_sha256(&format!(
        "connection-runtime-run.v1\0{}\0{}\0{}\0{}",
        std::process::id(),
        now_nanos,
        sequence,
        plan_digest
    ));
    let oracle_revision = "lc631-dg1-doctor-output-contract.v1";
    let oracle = ConnectionAcceptanceOracle {
        schema_version: "epistesys-connection-acceptance-oracle.v1",
        oracle_revision,
        source: "bundled fixed read-only command profile; not a semantic gold oracle",
        json_pointer: plan.expected_json_pointer.clone(),
        expected_value_digest: stable_sha256(
            &serde_json::to_string(&plan.expected_json_value)
                .map_err(|_| ConnectionError::InvalidPlan)?,
        ),
        matched: acceptance_matched,
        candidate_generated: false,
        independent_semantic_gold: false,
    };
    let input_digest = stable_sha256(&plan.source);
    let trace_digest = stable_sha256(
        &serde_json::to_string(&ConnectionRuntimeTraceDigestInput {
            schema_version: "epistesys-connection-runtime-trace.v1",
            run_id: &run_id,
            plan_digest: &plan_digest,
            source_revision: &plan.source_revision,
            requirement_id: plan.requirement_id,
            connection_graph_digest: &graph.graph_digest,
            route_edge_id: &route_edge.edge_id,
            producer_node_id: &route_edge.from_node,
            consumer_node_id: &route_edge.to_node,
            executable_path: &plan.executable_path,
            declared_executable_digest: &plan.executable_digest,
            observed_executable_digest: process.executable_sha256.as_deref(),
            arguments_digest: &process.args_digest,
            input_digest: &input_digest,
            cwd_digest: &process.cwd_digest,
            environment_digest: &process.environment_digest,
            stdout_digest: &process.stdout_digest,
            stderr_digest: &process.stderr_digest,
            exit_code: process.exit_code,
            timed_out: process.timed_out,
            stdout_truncated: process.stdout_truncated,
            stderr_truncated: process.stderr_truncated,
            network_policy: &process.network_policy,
            network_os_enforced: process.network_isolation_os_enforced,
            filesystem_write_monitor: "not_observed",
            acceptance_oracle: &oracle,
        })
        .map_err(|_| ConnectionError::InvalidPlan)?,
    );
    Ok(ConnectionRuntimeTrace {
        schema_version: "epistesys-connection-runtime-trace.v1",
        run_id,
        trace_digest,
        plan_digest,
        source_revision: plan.source_revision.clone(),
        requirement_id: plan.requirement_id,
        connection_graph_digest: graph.graph_digest.clone(),
        route_edge_id: route_edge.edge_id.clone(),
        producer_node_id: route_edge.from_node.clone(),
        consumer_node_id: route_edge.to_node.clone(),
        executable_path: plan.executable_path.clone(),
        declared_executable_digest: plan.executable_digest.clone(),
        observed_executable_digest: process.executable_sha256.clone(),
        arguments_digest: process.args_digest.clone(),
        input_digest,
        cwd_digest: process.cwd_digest.clone(),
        environment_digest: process.environment_digest.clone(),
        stdout_digest: process.stdout_digest.clone(),
        stderr_digest: process.stderr_digest.clone(),
        exit_code: process.exit_code,
        timed_out: process.timed_out,
        stdout_truncated: process.stdout_truncated,
        stderr_truncated: process.stderr_truncated,
        network_policy: process.network_policy.clone(),
        network_os_enforced: process.network_isolation_os_enforced,
        filesystem_write_monitor: "not_observed",
        acceptance_oracle: oracle,
        claim_boundary: "one run-bound observation for the fixed read-only CLI route and bundled JSON contract; not candidate correctness, independent semantic gold, host delivery, write monitoring, or closure",
    })
}

fn checked_path(root: &Path, relative: &str) -> Result<PathBuf, ConnectionError> {
    let path = Path::new(relative);
    if path.is_absolute()
        || relative.is_empty()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(ConnectionError::PathUnsafe);
    }
    let resolved = root
        .join(path)
        .canonicalize()
        .map_err(|_| ConnectionError::FileUnavailable)?;
    if !resolved.starts_with(root) {
        return Err(ConnectionError::PathUnsafe);
    }
    Ok(resolved)
}

fn read_bounded_rust(path: &Path) -> Result<String, ConnectionError> {
    let metadata = fs::metadata(path).map_err(|_| ConnectionError::FileUnavailable)?;
    if metadata.len() > 262_144 {
        return Err(ConnectionError::InvalidPlan);
    }
    fs::read_to_string(path).map_err(|_| ConnectionError::RustSyntaxError)
}

fn digest_file(path: &Path) -> Result<String, ConnectionError> {
    let mut file = File::open(path).map_err(|_| ConnectionError::FileUnavailable)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 65_536];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| ConnectionError::FileUnavailable)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let mut output = String::from("sha256:");
    for byte in hasher.finalize() {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    Ok(output)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn polarity_name(value: Dg1RequirementPolarity) -> &'static str {
    match value {
        Dg1RequirementPolarity::Positive => "positive",
        Dg1RequirementPolarity::Negative => "negative",
        Dg1RequirementPolarity::Unknown => "unknown",
        Dg1RequirementPolarity::Conflict => "conflict",
    }
}

fn condition_name(value: Dg1ConditionKind) -> &'static str {
    match value {
        Dg1ConditionKind::Necessary => "necessary",
        Dg1ConditionKind::Conditional => "conditional",
        Dg1ConditionKind::Temporal => "temporal",
        Dg1ConditionKind::Exception => "exception",
    }
}

#[cfg(test)]
mod evidence_issuance_tests {
    use super::*;
    use lc631_receipt_kernel::ReceiptIssuer;

    #[test]
    fn caller_constructed_observed_report_cannot_issue_closure_receipts() {
        let source = "Please test the package.";
        let plan = CodingConnectionPlan {
            schema_version: "epistesys-coding-connection-plan.v1".into(),
            source: source.into(),
            source_revision: stable_sha256(source),
            requirement_id: 1,
            coding_requirement_id: None,
            coding_task_id: None,
            coding_target_ref: None,
            coding_target_digest: None,
            expected_polarity: "positive".into(),
            expected_condition_kind: None,
            expected_scope: None,
            implementation_path: "src/lib.rs".into(),
            implementation_digest: stable_sha256("implementation"),
            implementation_symbol: "target".into(),
            router_path: "src/main.rs".into(),
            router_digest: stable_sha256("router"),
            executable_path: "target/debug/lc631.exe".into(),
            executable_digest: stable_sha256("executable"),
            command: "lc631-dg1-doctor".into(),
            arguments: vec!["--prompt".into(), source.into()],
            expected_json_pointer: "/payload/schema_version".into(),
            expected_json_value: Value::String("epistesys-dg1-parse.v1".into()),
        };
        let connection_graph = crate::connection_graph::analyze_connection_graph_sources(
            &plan,
            "pub fn target() {}",
            "use unknown_module::target; fn route(c: &str) { match c { \"lc631-dg1-doctor\" => { target(); }, _ => {} } }",
        )
        .unwrap();
        let args = Vec::new();
        let report = CodingConnectionReport {
            schema_version: "epistesys-coding-connection.v1",
            source_revision: plan.source_revision.clone(),
            requirement_id: plan.requirement_id,
            implementation_digest: plan.implementation_digest.clone(),
            router_digest: plan.router_digest.clone(),
            executable_digest: plan.executable_digest.clone(),
            connection_graph,
            implementation_symbol_found: true,
            command_registered_candidate: true,
            runtime_command_observed: true,
            acceptance_matched: true,
            runtime_trace: None,
            process: not_started_request("caller-claimed", &args, "caller_constructed_report"),
            status: ConnectionStatus::Observed,
            authority_created: false,
            output_commit_allowed: false,
            claim_boundary: "caller constructed test report",
        };
        let issuer =
            ReceiptIssuer::from_key_bytes("self-asserted-test-issuer", &[7_u8; 32]).unwrap();
        assert_eq!(
            issue_connection_evidence(&plan, &report, &issuer, 10),
            Err(ConnectionError::PublicReportNotEvidence)
        );
    }
}
