//! 通常solveへの小さい入口。未接続状態から完成・権限を生成しない。
//! Compact canonical runtime entry; pending stages remain pending.
#![forbid(unsafe_code)]

use lc631_core::context::{
    ContextRevision, DecisionStatus, MissingStage, SourceRevision, TaskKind,
};
use lc631_runtime::connection_audit::{audit_current_entry, AuditError, ConnectionAudit};
use lc631_runtime::RuntimeReport;
use serde::Serialize;
use std::io::{Read, Write};

mod artifacts;
mod preparation;
pub mod services;

pub const MAX_SOURCE_BYTES: usize = 256 * 1024;
pub const MAX_SUMMARY_BYTES: usize = 64 * 1024;
const MAX_ARGUMENTS: usize = 16;
// Only the explicit paired artifact profile has this separately fixed ceiling.
const MAX_ARTIFACT_ARGUMENTS: usize = 24;
const MAX_ARTIFACT_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandError {
    MissingCommand,
    UnknownCommand,
    TooManyArguments,
    UnknownOption,
    MissingValue,
    DuplicateOption,
    ConflictingInput,
    MissingInput,
    InvalidTask,
    InvalidInputFormat,
    InvalidGoalPolicy,
    InvalidEvaluationProfile,
    InvalidObjectiveStep,
    UnsupportedFormalTask,
    ServicesRequired,
    InvalidArtifact,
    UnexpectedInput,
    EmptySource,
    SourceTooLarge,
    InvalidUtf8,
    AllocationFailed,
    ReadFailed,
    SourceMismatch,
    RuntimeAudit(AuditError),
    EncodeFailed,
    SummaryTooLarge,
    WriteFailed,
    WriteFailedAfterRuntime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Solve,
    Inspect,
    Resume,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GoalPolicyArgument {
    NoGoal,
    SatisfyingModel,
    DeclaredSourceObjective,
}

/// Borrowed arguments; parsing never clones the source or allocates a JSON tree.
#[derive(Clone, Copy, Debug)]
pub struct ParsedCommand<'a> {
    kind: Kind,
    prompt: Option<&'a str>,
    stdin: bool,
    task: TaskKind,
    artifact: Option<&'a str>,
    store_root: Option<&'a str>,
    input_format: Option<&'a str>,
    solver_registry: Option<&'a str>,
    goal_policy: GoalPolicyArgument,
    evaluation_profile: Option<&'a str>,
    objective_step: Option<&'a str>,
}

pub fn is_canonical_command(command: &str) -> bool {
    matches!(
        command,
        "epistesys-solve" | "epistesys-inspect" | "epistesys-resume"
    )
}

// Inspect at most the separately bounded profile, consuming values according
// to known arities. A flag-shaped prompt/path/value never selects a profile.
fn explicit_store_option(arguments: &[String]) -> bool {
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--store-root" => return true,
            "--prompt-stdin" => index += 1,
            "--prompt"
            | "--task"
            | "--artifact"
            | "--input-format"
            | "--solver-registry"
            | "--goal-policy"
            | "--evaluation-profile"
            | "--objective-step" => {
                if arguments.get(index + 1).is_none() {
                    return false;
                }
                index += 2;
            }
            // Do not invent an arity for an unknown option. The ordinary
            // bounded parser still emits its typed unknown/missing error.
            _ => return false,
        }
    }
    false
}

pub fn parse(arguments: &[String]) -> Result<ParsedCommand<'_>, CommandError> {
    let name = arguments.first().ok_or(CommandError::MissingCommand)?;
    let kind = match name.as_str() {
        "epistesys-solve" => Kind::Solve,
        "epistesys-inspect" => Kind::Inspect,
        "epistesys-resume" => Kind::Resume,
        _ => return Err(CommandError::UnknownCommand),
    };
    if arguments.len() > MAX_ARTIFACT_ARGUMENTS {
        return Err(CommandError::TooManyArguments);
    }
    let explicit_store = explicit_store_option(arguments);
    let argument_limit = if explicit_store {
        MAX_ARTIFACT_ARGUMENTS
    } else {
        MAX_ARGUMENTS
    };
    if arguments.len() > argument_limit {
        return Err(CommandError::TooManyArguments);
    }
    let mut parsed = ParsedCommand {
        kind,
        prompt: None,
        stdin: false,
        task: TaskKind::Reasoning,
        artifact: None,
        store_root: None,
        input_format: None,
        solver_registry: None,
        goal_policy: GoalPolicyArgument::NoGoal,
        evaluation_profile: None,
        objective_step: None,
    };
    let mut task_seen = false;
    let mut goal_seen = false;
    let mut index = 1;
    while index < arguments.len() {
        let option = arguments[index].as_str();
        if option == "--prompt-stdin" {
            if parsed.stdin {
                return Err(CommandError::DuplicateOption);
            }
            parsed.stdin = true;
            index += 1;
            continue;
        }
        if !matches!(
            option,
            "--prompt"
                | "--task"
                | "--artifact"
                | "--store-root"
                | "--input-format"
                | "--solver-registry"
                | "--goal-policy"
                | "--evaluation-profile"
                | "--objective-step"
        ) {
            return Err(CommandError::UnknownOption);
        }
        let value = arguments
            .get(index + 1)
            .ok_or(CommandError::MissingValue)?
            .as_str();
        match option {
            "--prompt" => {
                if parsed.prompt.replace(value).is_some() {
                    return Err(CommandError::DuplicateOption);
                }
            }
            "--task" => {
                if task_seen {
                    return Err(CommandError::DuplicateOption);
                }
                parsed.task = parse_task(value)?;
                task_seen = true;
            }
            "--artifact" => {
                if parsed.artifact.replace(value).is_some() {
                    return Err(CommandError::DuplicateOption);
                }
            }
            "--store-root" => {
                if parsed.store_root.replace(value).is_some() {
                    return Err(CommandError::DuplicateOption);
                }
                if value.is_empty() || value.len() > 4096 {
                    return Err(CommandError::InvalidArtifact);
                }
                lc631_runtime::canonical_solve::replay::validate_store_root(std::path::Path::new(
                    value,
                ))
                .map_err(|_| CommandError::InvalidArtifact)?;
            }
            "--input-format" => {
                if parsed.input_format.replace(value).is_some() {
                    return Err(CommandError::DuplicateOption);
                }
                if value != "smtlib-qf-lia-v1" {
                    return Err(CommandError::InvalidInputFormat);
                }
            }
            "--solver-registry" => {
                if parsed.solver_registry.replace(value).is_some() {
                    return Err(CommandError::DuplicateOption);
                }
                if value.is_empty() {
                    return Err(CommandError::MissingValue);
                }
            }
            "--goal-policy" => {
                if goal_seen {
                    return Err(CommandError::DuplicateOption);
                }
                parsed.goal_policy = match value {
                    "no-goal" => GoalPolicyArgument::NoGoal,
                    "satisfying-model" => GoalPolicyArgument::SatisfyingModel,
                    "declared-source-objective" => GoalPolicyArgument::DeclaredSourceObjective,
                    _ => return Err(CommandError::InvalidGoalPolicy),
                };
                goal_seen = true;
            }
            "--evaluation-profile" => {
                if parsed.evaluation_profile.replace(value).is_some() {
                    return Err(CommandError::DuplicateOption);
                }
                if value != "finite-boolean-v1" {
                    return Err(CommandError::InvalidEvaluationProfile);
                }
            }
            "--objective-step" => {
                if parsed.objective_step.replace(value).is_some() {
                    return Err(CommandError::DuplicateOption);
                }
                if value != "single-v1" {
                    return Err(CommandError::InvalidObjectiveStep);
                }
            }
            _ => return Err(CommandError::UnknownOption),
        }
        index += 2;
    }
    if parsed.input_format.is_none()
        && ((parsed.solver_registry.is_some()
            && !(parsed.store_root.is_some() && parsed.kind == Kind::Resume))
            || goal_seen
            || parsed.evaluation_profile.is_some()
            || parsed.objective_step.is_some())
    {
        return Err(CommandError::UnexpectedInput);
    }
    if parsed.objective_step.is_some()
        && parsed.goal_policy != GoalPolicyArgument::DeclaredSourceObjective
    {
        return Err(CommandError::InvalidObjectiveStep);
    }
    parsed.validate(task_seen)?;
    Ok(parsed)
}

impl ParsedCommand<'_> {
    fn validate(&self, task_seen: bool) -> Result<(), CommandError> {
        match self.kind {
            Kind::Solve => {
                if self.artifact.is_some() && self.store_root.is_none() {
                    return Err(CommandError::UnexpectedInput);
                }
                if self.store_root.is_some() {
                    if self.input_format.is_none() || self.solver_registry.is_none() {
                        return Err(CommandError::ServicesRequired);
                    }
                    lc631_runtime::canonical_solve::replay::ArtifactKey::parse(
                        self.artifact.ok_or(CommandError::MissingInput)?,
                    )
                    .map_err(|_| CommandError::InvalidArtifact)?;
                }
                // The explicit formal engine currently implements conditional
                // reasoning/mathematics only. It has no coding, media, or
                // explanation task consumer: never silently ignore that input.
                if self.input_format.is_some()
                    && !matches!(self.task, TaskKind::Reasoning | TaskKind::Mathematics)
                {
                    return Err(CommandError::UnsupportedFormalTask);
                }
                match (self.prompt, self.stdin) {
                    (Some(_), true) => return Err(CommandError::ConflictingInput),
                    (None, false) => return Err(CommandError::MissingInput),
                    (Some(source), false) => check_source(source)?,
                    (None, true) => {}
                }
            }
            Kind::Inspect | Kind::Resume => {
                if self.prompt.is_some()
                    || self.stdin
                    || task_seen
                    || self.input_format.is_some()
                    || (self.solver_registry.is_some() && self.store_root.is_none())
                {
                    return Err(CommandError::UnexpectedInput);
                }
                let artifact = self.artifact.ok_or(CommandError::MissingInput)?;
                if self.store_root.is_some() {
                    lc631_runtime::canonical_solve::replay::ArtifactKey::parse(artifact)
                        .map_err(|_| CommandError::InvalidArtifact)?;
                    if self.kind == Kind::Inspect && self.solver_registry.is_some() {
                        return Err(CommandError::UnexpectedInput);
                    }
                }
                if artifact.is_empty()
                    || artifact.len() > MAX_ARTIFACT_BYTES
                    || !artifact
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_:./".contains(&b))
                {
                    return Err(CommandError::InvalidArtifact);
                }
            }
        }
        Ok(())
    }
}

fn parse_task(value: &str) -> Result<TaskKind, CommandError> {
    match value {
        "mathematics" => Ok(TaskKind::Mathematics),
        "reasoning" => Ok(TaskKind::Reasoning),
        "explanation" => Ok(TaskKind::Explanation),
        "coding" => Ok(TaskKind::Coding),
        "media" => Ok(TaskKind::Media),
        _ => Err(CommandError::InvalidTask),
    }
}

fn check_source(source: &str) -> Result<(), CommandError> {
    if source.is_empty() {
        return Err(CommandError::EmptySource);
    }
    if source.len() > MAX_SOURCE_BYTES {
        return Err(CommandError::SourceTooLarge);
    }
    Ok(())
}

fn read_source(reader: &mut impl Read) -> Result<String, CommandError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_SOURCE_BYTES + 1)
        .map_err(|_| CommandError::AllocationFailed)?;
    reader
        .take((MAX_SOURCE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| CommandError::ReadFailed)?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(CommandError::SourceTooLarge);
    }
    let source = String::from_utf8(bytes).map_err(|_| CommandError::InvalidUtf8)?;
    check_source(&source)?;
    Ok(source)
}

/// Runtime execution is distinct from stdout delivery. This observation grants
/// no authority, resume capability, model validation or completion evidence.
#[derive(Debug)]
pub struct CommandObservation {
    audit: Option<ConnectionAudit>,
}
impl CommandObservation {
    pub fn runtime_report(&self) -> Option<RuntimeReport> {
        self.audit.map(|audit| audit.entry().report())
    }
    pub const fn connection_audit(&self) -> Option<&ConnectionAudit> {
        self.audit.as_ref()
    }
}

/// Explicit legacy diagnostic embedding adapter and focused tests. Production
/// solve uses the resource-owned preparation path instead of this pure entry.
pub fn run_io(
    arguments: &[String],
    stdin: &mut impl Read,
    output: &mut impl Write,
) -> Result<CommandObservation, CommandError> {
    let command = parse(arguments)?;
    if command.input_format.is_some() || command.store_root.is_some() {
        // This legacy diagnostic adapter owns no operational services. Never
        // silently substitute its pure runtime for an explicit formal request.
        return Err(CommandError::ServicesRequired);
    }
    let observation = match command.kind {
        Kind::Solve => {
            let owned;
            let source = match command.prompt {
                Some(source) => source,
                None => {
                    owned = read_source(stdin)?;
                    &owned
                }
            };
            let context = ContextRevision::original(SourceRevision::of(source));
            // The only runtime dispatch. No doctor, fallback, implicit build or retry.
            // The audit owns the actual single solve return, not a second solve.
            let audit = audit_current_entry(source, context, command.task)
                .map_err(CommandError::RuntimeAudit)?;
            CommandObservation { audit: Some(audit) }
        }
        // No current runtime artifact-store read/resume capability exists here.
        // Retain explicit pending; never claim a saved candidate was restored.
        Kind::Inspect | Kind::Resume => CommandObservation { audit: None },
    };
    let bytes = encode_summary(&observation)?;
    output.write_all(&bytes).map_err(|_| {
        if observation.audit.is_some() {
            CommandError::WriteFailedAfterRuntime
        } else {
            CommandError::WriteFailed
        }
    })?;
    Ok(observation)
}

#[derive(Serialize)]
struct CompactSummary {
    schema_version: &'static str,
    status: &'static str,
    missing_stage: Option<&'static str>,
    task: Option<&'static str>,
    source_bytes: Option<usize>,
    source_digest: Option<String>,
    implementation_complete: bool,
    authority_created: bool,
    host_delivery_observed: bool,
}

fn encode_summary(observation: &CommandObservation) -> Result<Vec<u8>, CommandError> {
    encode_report_summary(observation.runtime_report())
}

fn encode_report_summary(report: Option<RuntimeReport>) -> Result<Vec<u8>, CommandError> {
    let mut summary = CompactSummary {
        schema_version: "epistesys-canonical-summary.v1",
        status: "not_connected",
        missing_stage: Some("artifact_store_capability"),
        task: None,
        source_bytes: None,
        source_digest: None,
        implementation_complete: false,
        authority_created: false,
        host_delivery_observed: false,
    };
    if let Some(report) = report {
        let (status, stage) = status_fields(report.status);
        summary.status = status;
        summary.missing_stage = stage;
        summary.task = Some(task_name(report.task));
        summary.source_bytes = Some(report.source_bytes);
        summary.source_digest = Some(digest_hex(report.context.source().digest().as_bytes())?);
    }
    crate::io::encode_pretty_bounded(&summary, MAX_SUMMARY_BYTES, None)
        .map_err(|_| CommandError::EncodeFailed)
}

fn digest_hex(digest: &[u8; 32]) -> Result<String, CommandError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::new();
    text.try_reserve_exact(64)
        .map_err(|_| CommandError::AllocationFailed)?;
    for byte in digest {
        text.push(char::from(HEX[usize::from(byte >> 4)]));
        text.push(char::from(HEX[usize::from(byte & 15)]));
    }
    Ok(text)
}

fn task_name(task: TaskKind) -> &'static str {
    match task {
        TaskKind::Mathematics => "mathematics",
        TaskKind::Reasoning => "reasoning",
        TaskKind::Explanation => "explanation",
        TaskKind::Coding => "coding",
        TaskKind::Media => "media",
    }
}

fn status_fields(status: DecisionStatus) -> (&'static str, Option<&'static str>) {
    match status {
        DecisionStatus::NotConnected(stage) => (
            "not_connected",
            Some(match stage {
                MissingStage::ContextLowering => "context_lowering",
                MissingStage::SolverPortfolio => "solver_portfolio",
                MissingStage::Measurement => "measurement",
                MissingStage::Selection => "selection",
                MissingStage::HostObservation => "host_observation",
            }),
        ),
        DecisionStatus::NeedsEvidence => ("needs_evidence", None),
        DecisionStatus::Clarify => ("clarify", None),
        DecisionStatus::Hold => ("hold", None),
        DecisionStatus::Conflict => ("conflict", None),
    }
}

/// Parent-owned dispatcher registers this before legacy argument extraction.
pub(crate) fn run(
    resources: &crate::io::CommandIo,
    arguments: &[String],
) -> Result<(), crate::effect_error::CliFailure> {
    use crate::effect_error::CliFailure;
    let command = parse(arguments)
        .map_err(|error| CliFailure::not_attempted(format!("canonical_arguments:{error:?}")))?;
    if command.kind == Kind::Solve {
        if command.input_format.is_some() {
            return services::run::run(resources, command);
        }
        return preparation::run(resources, command);
    }
    if command.store_root.is_some() {
        return artifacts::run(resources, command);
    }
    resources
        .with_view((MAX_SOURCE_BYTES + 1) as u64, |_| {
            run_io(
                arguments,
                &mut std::io::stdin().lock(),
                &mut std::io::stdout().lock(),
            )
            .map(|_| ())
            .map_err(|error| format!("canonical_command:{error:?}; delivery_unobserved_no_retry"))
        })
        .map_err(CliFailure::from)
}
