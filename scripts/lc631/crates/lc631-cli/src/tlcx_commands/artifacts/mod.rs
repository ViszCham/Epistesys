//! Explicit caller-owned local archive profile. No grant or current-value decode.
#![forbid(unsafe_code)]
use super::{digest_hex, GoalPolicyArgument, Kind, ParsedCommand, MAX_SUMMARY_BYTES};
use crate::{effect_error::CliFailure, io::CommandIo};
use lc631_runtime::{
    canonical_solve::{
        replay::{
            capture_current, persist_current, validate_store_root, with_fresh_resume, ArtifactKey,
            CanonicalArtifactReader, ReplayGoal, ReplayProfile,
        },
        CanonicalServices, DecisionBudget, ExecutionBudget, GpuPolicy, OwnedCanonicalDecision,
        SExprLimits,
    },
    resource::{
        ResourceAmount, ResourceBroker, ResourceKind, ResourceRequest, ResourceReservationView,
    },
};
use serde::Serialize;
use std::{io::Write, path::Path};

pub(super) fn profile(command: ParsedCommand<'_>) -> Result<ReplayProfile, String> {
    let goal = match command.goal_policy {
        GoalPolicyArgument::NoGoal => ReplayGoal::NoGoal,
        GoalPolicyArgument::SatisfyingModel => ReplayGoal::SatisfyingModel,
        GoalPolicyArgument::DeclaredSourceObjective => ReplayGoal::DeclaredSourceObjective,
    };
    ReplayProfile::checked(
        goal,
        command.evaluation_profile.is_some(),
        command.objective_step.is_some(),
    )
    .map_err(|_| "canonical_artifact:Profile".to_owned())
}
pub(super) fn key(command: ParsedCommand<'_>) -> Result<ArtifactKey, String> {
    ArtifactKey::parse(command.artifact.ok_or("canonical_artifact:MissingKey")?)
        .map_err(|_| "canonical_artifact:InvalidKey".to_owned())
}
pub(super) fn preadmit_save(
    command: ParsedCommand<'_>,
    source: &str,
    profile: ReplayProfile,
) -> Result<(), String> {
    if let Some(root) = command.store_root {
        validate_store_root(Path::new(root)).map_err(|_| "canonical_artifact:Root")?;
        if key(command)? != profile.key(source) {
            return Err("canonical_artifact:SourceProfileMismatch".to_owned());
        }
    }
    Ok(())
}
pub(super) fn save_current(
    command: ParsedCommand<'_>,
    decision: &OwnedCanonicalDecision<'_, '_>,
    source: &str,
    profile: ReplayProfile,
    broker: &ResourceBroker,
    view: &ResourceReservationView<'_>,
) -> Result<(), String> {
    if let Some(root) = command.store_root {
        preadmit_save(command, source, profile)?;
        let captured = capture_current(decision, source, profile, broker, view)
            .map_err(|e| format!("canonical_artifact:capture:{e:?}; no_storage_write"))?;
        persist_current(Path::new(root), &captured, view)
            .map_err(|_| "canonical_artifact:persistence_effect_unknown_no_retry".to_owned())?;
    }
    Ok(())
}

/// The same exact renderer contract is used by ordinary solve and fresh resume.
pub(super) fn render_current(
    decision: &OwnedCanonicalDecision<'_, '_>,
    source: &str,
    broker: &ResourceBroker,
    view: &ResourceReservationView<'_>,
) -> Result<Vec<u8>, String> {
    let amount = ResourceAmount::checked(MAX_SUMMARY_BYTES as u64)
        .map_err(|e| format!("canonical_copy:{e:?}"))?;
    let _claim = view
        .claim_resources(&[
            ResourceRequest::new(ResourceKind::Ram, amount),
            ResourceRequest::new(ResourceKind::Output, amount),
        ])
        .map_err(|e| format!("canonical_copy:{e:?}"))?;
    let rendered = decision
        .project_current(broker, view)
        .map_err(|e| format!("canonical_projection:{e:?}"))?;
    let original =
        lc631_runtime::canonical_original_pending::inspect_current(decision, broker, view)
            .map_err(|e| format!("canonical_original_inputs:{e:?}"))?;
    if !rendered.text().contains(original.japanese_line())
        || !rendered.text().contains(original.english_line())
    {
        return Err("canonical_output:OriginalInputStateOmitted".to_owned());
    }
    if rendered.text().len() > MAX_SUMMARY_BYTES {
        return Err("canonical_output:Limit".to_owned());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(rendered.text().len())
        .map_err(|_| "canonical_output:AllocationFailed")?;
    bytes.extend_from_slice(rendered.text().as_bytes());
    if !decision.matches_exact(source, &bytes) {
        return Err("canonical_output:ExactBindingMismatch".to_owned());
    }
    decision
        .project_current(broker, view)
        .map_err(|e| format!("canonical_projection:{e:?}"))?;
    Ok(bytes)
}

#[derive(Serialize)]
struct Inspection<'a> {
    schema_version: &'static str,
    state: &'static str,
    source_digest: String,
    profile_digest: String,
    source_bytes: usize,
    historical_text: &'a str,
    current_model: bool,
    authority_created: bool,
    completion_restored: bool,
    host_delivery_observed: bool,
    fresh_execution_required: bool,
}

pub(super) fn run(resources: &CommandIo, command: ParsedCommand<'_>) -> Result<(), CliFailure> {
    let operation = || -> Result<(), String> {
        let key = key(command)?;
        let root = command.store_root.ok_or("canonical_artifact:MissingRoot")?;
        validate_store_root(Path::new(root)).map_err(|_| "canonical_artifact:Root")?;
        if command.kind == Kind::Resume && command.solver_registry.is_none() {
            return Err("canonical_artifact:FreshRegistryRequired".to_owned());
        }
        let bytes = resources.with_canonical_artifact_services(
            key.source(),
            false,
            |broker, _, view| {
                let reader = CanonicalArtifactReader::open(Path::new(root), key, view)
                    .map_err(|_| "canonical_artifact:StorageReadHold")?;
                let artifact = reader
                    .read(view)
                    .map_err(|_| "canonical_artifact:InvalidHistoricalInput")?;
                let historical = artifact
                    .inspect_current(view)
                    .map_err(|_| "canonical_artifact:ReadCapabilityExpired")?;
                if command.kind == Kind::Inspect {
                    let amount = ResourceAmount::checked((MAX_SUMMARY_BYTES * 2) as u64)
                        .map_err(|_| "canonical_artifact:InspectionBudget")?;
                    let _claim = view
                        .claim_resources(&[
                            ResourceRequest::new(ResourceKind::Ram, amount),
                            ResourceRequest::new(ResourceKind::Output, amount),
                        ])
                        .map_err(|_| "canonical_artifact:InspectionBudget")?;
                    let report = Inspection {
                        schema_version: "epistesys-canonical-artifact-inspection.v1",
                        state: "historical_unanchored",
                        source_digest: digest_hex(key.source().as_bytes())
                            .map_err(|_| "canonical_artifact:Encode")?,
                        profile_digest: digest_hex(key.profile().as_bytes())
                            .map_err(|_| "canonical_artifact:Encode")?,
                        source_bytes: historical.source().len(),
                        historical_text: historical.historical_text(),
                        current_model: false,
                        authority_created: false,
                        completion_restored: false,
                        host_delivery_observed: false,
                        fresh_execution_required: true,
                    };
                    let bytes =
                        crate::io::encode_pretty_bounded(&report, MAX_SUMMARY_BYTES, Some(view))
                            .map_err(|_| "canonical_artifact:InspectionOutputLimit")?;
                    artifact
                        .inspect_current(view)
                        .map_err(|_| "canonical_artifact:ReadCapabilityExpired")?;
                    return Ok(bytes);
                }
                let loaded = super::services::file::read_registry_file(
                    super::services::file::ConfigFile::checked(Path::new(
                        command
                            .solver_registry
                            .ok_or("canonical_artifact:FreshRegistryRequired")?,
                    ))
                    .map_err(|_| "canonical_artifact:RegistryPath")?,
                    view,
                )
                .map_err(|e| format!("canonical_artifact:FreshRegistry:{e:?}"))?;
                let cwd =
                    std::env::current_dir().map_err(|_| "canonical_artifact:WorkingDirectory")?;
                let services = services(broker, view, Some(loaded.registry()), &cwd)?;
                with_fresh_resume(&artifact, services, |result| {
                    let decision = result.map_err(|e| {
                        format!("canonical_artifact:FreshExecution:{e:?}; no_retry")
                    })?;
                    render_current(&decision, historical.source(), broker, view)
                })
                .map_err(|e| format!("canonical_artifact:ReAdmission:{e:?}"))?
            },
        )?;
        std::io::stdout()
            .lock()
            .write_all(&bytes)
            .map_err(|_| "canonical_artifact:UnknownDeliveryNoRetry".to_owned())?;
        Ok(())
    };
    operation().map_err(CliFailure::from)
}
pub(super) fn services<'v, 'lease>(
    broker: &'v ResourceBroker,
    view: &'v ResourceReservationView<'lease>,
    registry: Option<&'v lc631_runtime::canonical_solve::SolverRegistry>,
    cwd: &'v Path,
) -> Result<CanonicalServices<'v, 'lease>, String> {
    Ok(CanonicalServices {
        registry,
        broker,
        current: view,
        execution: execution()?,
        parser: SExprLimits::checked(65536, 4096, 64).map_err(|_| "canonical_parser_budget")?,
        working_directory: cwd,
        budget: DecisionBudget::checked(MAX_SUMMARY_BYTES)
            .map_err(|_| "canonical_output_budget")?,
        optional_gpu: GpuPolicy::NotRequested,
    })
}
pub(super) fn execution() -> Result<ExecutionBudget, String> {
    ExecutionBudget::checked(
        5000,
        16384,
        65536,
        16384,
        16,
        4096,
        64 * 1024 * 1024,
        0,
        65536,
    )
    .map_err(|e| format!("canonical_execution_budget:{e:?}"))
}
