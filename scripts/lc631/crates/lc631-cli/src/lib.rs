#![forbid(unsafe_code)]

pub mod binary_checkpoint_resume;
mod build_profile;
mod coding_commands;
pub mod bundle_inspect;
pub mod io;
mod tlcx_checked_tl_command;
pub mod tlcx_commands;
mod tlcx_path_command;

mod dgcl_journal;
mod dgcl_package;
mod dgcl_repair;
mod effect_error;
use effect_error::CliFailure;

#[cfg(feature = "gpu-cubecl-wgpu")]
use lc631_accelerator::{compare_lane_digests, CubeClWgpuBackend, MeasurementState};
use lc631_accelerator::{input_from_evaluations, AcceleratorBroker, CpuBackend, ExecutionReceipt};
use lc631_analysis::{
    analyze_path_with_resources as analyze_program_path, analyze_with_resources as analyze_program,
    build_coding_closure, build_dgcl_pipeline_with_runner as build_dgcl_pipeline,
    build_dgcl_standalone_candidate, evaluate_gold, issue_checkpoint_head_anchor,
    issue_observed_connection_evidence, load_dgcl_runtime_pins_with_resources,
    observe_connection_plan_from_cli_with_consumer, parse_controlled_instruction,
    run_language_worker_with_resources, verify_connection_plan_from_cli_with_resources,
    AnalysisRequest, CheckpointBody, CheckpointEventKind, CheckpointHeadRequest,
    CodingConnectionPlan, DeliveryState, DgclBackendSet, GoldCorpus, InstructionParseBudget,
    ProgramAnalysisReport,
};
use lc631_compat::{BaselineGuardReport, DivergenceKind, PairedObservation};
use lc631_core::{
    stable_sha256, ArtifactId, AuthorityRevision, EpochId, RevisionBinding, SourceSpan,
    LC631_VERSION,
};
use lc631_host::{
    append_durable_replay_verified_with_resources, bind_codex_stop_hook_attested,
    bind_host_output_attested, persist_stop_hook_observation_with_resources, CodexStopHookEvent,
    DurableReplayReceipt, HostBindingState, HostOutputCandidate, HostOutputReceipt,
    HostSeedEnvelope,
};
use lc631_media::{
    authorize_media_source, authorize_media_source_attested_with_resources, default_method_catalog,
    execute_local_media_backend_attested_with_resources, model_cache_manifest_sha256_bounded,
    BackendDescriptor, BackendFamily, BackendRegistry, MediaAnalysisGate, MediaBackendConfig,
    MediaResourceBudget, MediaSourceKind, MediaSourceRequest, ModelScanLimits,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReplayGuard, SubjectRevision, UntrustedReceipt,
};
use lc631_tl::{compare_legacy_route, ProjectionGateDecision};
use lc631_tldg::{
    analyze as analyze_tldg, analyze_dg1, build_structural_kernel as build_tldg_structural_kernel,
    build_tldg_pipeline, build_tldg_release_gate, build_tldg_release_gate_with_receipts,
    run_adversarial_evaluation, run_tldg_geometry_accelerator, CalibrationPartition, Dg1Budget,
    TldgPipelineReport,
};
use lc631_wire::{PromotionGate, ResidualRiskLedger, PROMOTION_PREDICATES, WIRE_SCHEMA};
use lc631_world::{
    evaluation_matrix, generate_distinct_worlds, materialize_evaluations, WorldArena, WorldBudget,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(feature = "gpu-cubecl-wgpu")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "gpu-cubecl-wgpu")]
use std::sync::{Arc, Barrier, Mutex};
#[cfg(feature = "gpu-cubecl-wgpu")]
use std::thread;
#[cfg(feature = "gpu-cubecl-wgpu")]
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_RPA_SOURCE_FILE_BYTES: u64 = 1024 * 1024;
const MAX_PROMOTION_EVIDENCE_FILE_BYTES: u64 = 1024 * 1024;
const MAX_PROMOTION_RECEIPT_AGE_SECONDS: u64 = 24 * 60 * 60;
const MAX_PROMPT_STDIN_BYTES: usize = 2 * 1024 * 1024;
const PROMOTION_EVIDENCE_SCHEMA: &str = "lc631-promotion-evidence.v1";
const V630_FROZEN_COMMIT: &str = "0d933982fa392043c2fe825644a3a560963b6615";

pub fn main_entry() {
    report_failure(CliFailure::not_attempted("cli_resource_owner_required"));
}

pub fn main_entry_with_resources(owner: &lc631_runtime::resource::ResourceBroker) {
    if coding_commands::dispatch_default(owner) {
        return;
    }
    let result = io::CommandIo::begin(owner)
        .map_err(CliFailure::not_attempted)
        .and_then(|resources| {
            let result = run(&resources);
            let cleanup = resources.finish();
            match (result, cleanup) {
                (Err(error), _) => Err(error),
                (Ok(()), Err(error)) => Err(CliFailure::from(error)),
                (Ok(()), Ok(())) => Ok(()),
            }
        });
    if let Err(error) = result {
        report_failure(error);
    }
}

fn report_failure(error: CliFailure) {
    #[derive(Serialize)]
    struct FailureEnvelope<'a> {
        schema_version: &'static str,
        error: &'a str,
        effect_state: &'a CliFailure,
        claim_boundary: &'static str,
    }
    let encode = |error: &CliFailure| {
        io::encode_pretty_bounded(&FailureEnvelope {
        schema_version: "lc631-cli-error.v1", error: &error.message, effect_state: error,
        claim_boundary: "diagnostic failure; effect state is explicit or unknown; no fallback success is implied",
    }, 16_384, None)
    };
    let bytes = match encode(&error) {
        Ok(bytes) => Ok(bytes),
        Err(_) => {
            let compact = CliFailure {
                message: "diagnostic exceeds fixed 16KiB budget; oversized references omitted"
                    .into(),
                request_digest: error.request_digest.filter(|value| value.len() <= 256),
                journal_location: error.journal_location.filter(|value| value.len() <= 4096),
                ..error
            };
            encode(&compact)
        }
    };
    let mut stderr = std::io::stderr().lock();
    match bytes {
        Ok(bytes) => {
            let _ = stderr.write_all(&bytes);
        }
        Err(_) => {
            let _ = stderr.write_all(b"{\"schema_version\":\"lc631-cli-error.v1\",\"error\":\"bounded diagnostic unavailable\",\"effect_state\":{\"message\":\"bounded diagnostic unavailable\",\"source_effect\":\"unknown\",\"persistence_effect\":\"unknown\",\"request_digest\":null,\"journal_location\":null},\"claim_boundary\":\"diagnostic failure; effects unresolved\"}\n");
        }
    }
    std::process::exit(2);
}

fn run(resources: &io::CommandIo) -> Result<(), CliFailure> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Err(CliFailure::not_attempted("missing_lc631_command"));
    }
    let command = args.remove(0);
    if command == "epistesys-build-profile" {
        return build_profile::run(resources, &args);
    }
    if command == "epistesys-path-check" {
        if args.len() != 1 || args[0] != "--config-stdin" {
            return Err(CliFailure::not_attempted("path_expected_config_stdin"));
        }
        return tlcx_path_command::run(resources);
    }
    if command == "epistesys-tl-check" {
        if args.len() != 1 || args[0] != "--config-stdin" {
            return Err(CliFailure::not_attempted("tl_expected_config_stdin"));
        }
        return tlcx_checked_tl_command::run(resources);
    }
    if tlcx_commands::is_canonical_command(&command) {
        args.insert(0, command);
        return tlcx_commands::run(resources, &args);
    }
    if command == "lc631-journal-bundle-inspect" {
        let inspect_args = bundle_inspect::parse_args(&args).map_err(|error| {
            CliFailure::not_attempted(format!("bundle_inspect_arguments:{error:?}"))
        })?;
        return bundle_inspect::run(resources, &command, inspect_args);
    }
    if command == "lc631-checkpoint-resume-inspect" {
        let inspect_args = binary_checkpoint_resume::parse_binary_checkpoint_resume_inspect_args(
            &args,
        )
        .map_err(|error| {
            CliFailure::not_attempted(format!("checkpoint_resume_inspect_arguments:{error:?}"))
        })?;
        return run_checkpoint_resume_inspect(resources, &command, inspect_args);
    }
    let repo = take_option(&mut args, "--repo").map(PathBuf::from);
    let execute = take_flag(&mut args, "--execute");
    let prompt = take_option(&mut args, "--prompt");
    let prompt_stdin = take_flag(&mut args, "--prompt-stdin");
    let prompt = match (prompt, prompt_stdin) {
        (Some(_), true) => return Err("use_exactly_one_of_--prompt_or_--prompt-stdin".into()),
        (Some(prompt), false) => Some(prompt),
        (None, true) => Some(read_prompt_stdin(resources)?),
        (None, false) => None,
    };
    let v630_digest = take_option(&mut args, "--v630-digest");
    let v631_digest = take_option(&mut args, "--v631-digest");
    let iterations = take_option(&mut args, "--iterations")
        .map(|value| value.parse::<usize>().map_err(|_| "invalid_--iterations"))
        .transpose()?;
    let workers = take_option(&mut args, "--workers")
        .map(|value| value.parse::<usize>().map_err(|_| "invalid_--workers"))
        .transpose()?;
    let media = take_option(&mut args, "--media").map(PathBuf::from);
    let media_temp_bytes = take_option(&mut args, "--media-temp-bytes")
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| "invalid_--media-temp-bytes")
        })
        .transpose()?
        .unwrap_or(io::MEDIA_TEMP_DISK_CAP);
    if media_temp_bytes == 0 || media_temp_bytes > io::MEDIA_TEMP_DISK_CAP {
        return Err(CliFailure::not_attempted(
            "media_temp_profile_requires_receipt_key_headroom_within_256_mib_run_cap",
        ));
    }
    let python = take_option(&mut args, "--python").map(PathBuf::from);
    let ffmpeg = take_option(&mut args, "--ffmpeg").map(PathBuf::from);
    let backend_script = take_option(&mut args, "--backend-script").map(PathBuf::from);
    let backend_script_digest = take_option(&mut args, "--backend-script-digest");
    let python_digest = take_option(&mut args, "--python-digest");
    let model_cache = take_option(&mut args, "--model-cache").map(PathBuf::from);
    let language = take_option(&mut args, "--language");
    let model_manifest = take_option(&mut args, "--model-manifest").map(PathBuf::from);
    let model_manifest_digest = take_option(&mut args, "--model-manifest-digest");
    let model_manifest_en = take_option(&mut args, "--model-manifest-en").map(PathBuf::from);
    let model_manifest_ja = take_option(&mut args, "--model-manifest-ja").map(PathBuf::from);
    let model_digest_en = take_option(&mut args, "--model-digest-en");
    let model_digest_ja = take_option(&mut args, "--model-digest-ja");
    let gold_file = take_option(&mut args, "--gold-file").map(PathBuf::from);
    let connection_plan_file = take_option(&mut args, "--connection-plan").map(PathBuf::from);
    let authority_revision = take_option(&mut args, "--authority-revision");
    let rights_asserted = take_flag(&mut args, "--rights-asserted");
    let seed_file = take_option(&mut args, "--seed-file").map(PathBuf::from);
    let source_file = take_option(&mut args, "--source-file").map(PathBuf::from);
    let output_file = take_option(&mut args, "--output-file").map(PathBuf::from);
    let callback_file = take_option(&mut args, "--callback-file").map(PathBuf::from);
    let host_receipt_file = take_option(&mut args, "--host-receipt-file").map(PathBuf::from);
    let source_receipt_file = take_option(&mut args, "--source-receipt-file").map(PathBuf::from);
    let evidence_file = take_option(&mut args, "--evidence-file").map(PathBuf::from);
    let receipt_root = take_option(&mut args, "--receipt-root").map(PathBuf::from);
    let author_receipt_root = take_option(&mut args, "--author-receipt-root").map(PathBuf::from);
    let backend_id = take_option(&mut args, "--backend-id");
    let model_license_receipt = take_option(&mut args, "--model-license-receipt");
    let ledger_root = take_option(&mut args, "--ledger-root").map(PathBuf::from);
    let head_root = take_option(&mut args, "--head-root").map(PathBuf::from);
    let ledger = take_option(&mut args, "--ledger").map(PathBuf::from);
    let artifact = take_option(&mut args, "--artifact")
        .map(|value| value.parse::<u64>().map_err(|_| "invalid_--artifact"))
        .transpose()?;
    let turn = take_option(&mut args, "--turn")
        .map(|value| value.parse::<u64>().map_err(|_| "invalid_--turn"))
        .transpose()?;
    if !args.is_empty() {
        return Err(CliFailure::not_attempted(format!(
            "unknown_arguments:{args:?}"
        )));
    }
    match command.as_str() {
        "lc631-doctor" => {
            let repo = repo.ok_or("lc631-doctor_requires_--repo")?;
            emit(resources, &command, build_doctor(&repo, resources)?)
        }
        "lc631-v630-guard" => {
            let repo = repo.ok_or("lc631-v630-guard_requires_--repo")?;
            emit(resources, &command, resources.baseline_guard(&repo)?)
        }
        "lc631-tl-doctor" => emit(resources,
            &command,
            build_tl_doctor(prompt.as_deref().unwrap_or("Do not delete files."))?,
        ),
        "lc631-world-doctor" => emit(resources,
            &command,
            build_world_doctor(prompt.as_deref().unwrap_or("2 + 2 = 4"))?,
        ),
        "lc631-tldg-doctor" => emit(resources,
            &command,
            build_tldg_pipeline(
                prompt
                    .as_deref()
                    .unwrap_or("Parse natural language and program syntax together."),
                None,
                &CalibrationPartition::reference(),
            )
            .map_err(|error| format!("{error:?}"))?,
        ),
        "lc631-dg1-doctor" => {
            let source = prompt.as_deref().unwrap_or("");
            emit(resources,
                &command,
                analyze_dg1(source, Dg1Budget::default())
                    .map_err(|error| format!("dg1:{error:?}"))?,
            )
        }
        "lc631-instruction-parse" => {
            let source = source_file
                .as_ref()
                .map(|path| read_bounded_rpa_source(path, resources))
                .transpose()?
                .or(prompt)
                .ok_or("lc631-instruction-parse_requires_source")?;
            emit(resources,
                &command,
                parse_controlled_instruction(&source, InstructionParseBudget::default())
                    .map_err(|error| format!("instruction_parse:{error:?}"))?,
            )
        }
        "lc631-dgcl-shadow-parse" => {
            let source = source_file
                .as_ref()
                .map(|path| read_bounded_rpa_source(path, resources))
                .transpose()?
                .or(prompt)
                .ok_or("lc631-dgcl-shadow-parse_requires_source")?;
            emit(resources,
                &command,
                build_dgcl_pipeline(
                    &source,
                    DgclBackendSet {
                        english: None,
                        japanese: None,
                    },
                    &mut resources.analysis_runner(),
                )
                .map_err(|error| format!("dgcl_shadow_parse:{error:?}"))?,
            )
        }
        "lc631-dgcl-package-finalize"
        | "lc631-dgcl-package-repair"
        | "lc631-dgcl-package-resume" => {
            if python.is_some()
                || model_cache.is_some()
                || model_manifest_en.is_some()
                || model_manifest_ja.is_some()
                || backend_script.is_some()
                || media.is_some()
            {
                return Err(
                    "dgcl_package_profile_does_not_support_backend_or_media_options".into(),
                );
            }
            if !execute {
                return Err("dgcl_package_finalize_requires_--execute".into());
            }
            let root = repo.ok_or("dgcl_package_finalize_requires_--repo")?;
            let source = prompt.ok_or("dgcl_package_finalize_requires_--prompt")?;
            let bundle_path =
                evidence_file.ok_or("dgcl_package_finalize_requires_--evidence-file")?;
            let raw = read_bounded_rpa_source(&bundle_path, resources)?;
            let context = resources.host_context(
                &receipt_root.ok_or("dgcl_package_finalize_requires_--receipt-root")?,
                false,
            )
            .map_err(|error| format!("dgcl_package_receipt_root:{error:?}"))?;
            let author = resources.host_context(
                &author_receipt_root
                    .ok_or("dgcl_package_finalize_requires_--author-receipt-root")?,
                false,
            )
            .map_err(|error| format!("dgcl_package_author_root:{error:?}"))?;
            let result = if command == "lc631-dgcl-package-repair"
                || command == "lc631-dgcl-package-resume"
            {
                let bundle = serde_json::from_str::<dgcl_repair::PackageRepairBundle>(&raw)
                    .map_err(|error| format!("dgcl_repair_bundle:{error}"))?;
                let ledger = ledger_root
                    .as_deref()
                    .ok_or("dgcl_package_repair_requires_--ledger-root")?;
                let heads = head_root
                    .as_deref()
                    .ok_or("dgcl_package_repair_requires_--head-root")?;
                if command == "lc631-dgcl-package-resume" {
                    dgcl_journal::resume_package_with_resources(
                        &root, &source, bundle, &context, &author, (ledger, heads), resources,
                    )?
                } else {
                    dgcl_repair::repair_package_with_resources(
                        &root,
                        &source,
                        bundle,
                        &context,
                        &author,
                        Some((ledger, heads)),
                        resources,
                    )?
                }
            } else {
                let bundle = serde_json::from_str::<dgcl_package::PackageValidationBundle>(&raw)
                    .map_err(|error| format!("dgcl_package_bundle:{error}"))?;
                dgcl_package::finalize_package_with_resources(&root, &source, bundle, &context, &author, resources)?
            };
            emit(resources, &command, &result).map_err(|message| CliFailure::output_failure(message, &result))?;
            Ok(())
        }
        "lc631-dgcl-run" => {
            let source = source_file
                .as_ref()
                .map(|path| read_bounded_rpa_source(path, resources))
                .transpose()?
                .or(prompt)
                .ok_or("lc631-dgcl-run_requires_source")?;
            let configured_profile = repo.is_some()
                || python.is_some()
                || backend_script.is_some()
                || model_cache.is_some()
                || model_manifest_en.is_some()
                || model_manifest_ja.is_some();
            let (pipeline, input_profile) = if configured_profile {
                let root = repo
                    .as_deref()
                    .ok_or("lc631-dgcl-run_configured_profile_requires_--repo")?;
                let pins = resources.with_view(262_144, |view| load_dgcl_runtime_pins_with_resources(root, view)
                    .map_err(|error| format!("dgcl_runtime_pins:{error:?}")))?;
                let python = python
                    .as_deref()
                    .ok_or("lc631-dgcl-run_configured_profile_requires_--python")?;
                let script = backend_script
                    .as_deref()
                    .ok_or("lc631-dgcl-run_configured_profile_requires_--backend-script")?;
                let model_dir = model_cache
                    .as_deref()
                    .ok_or("lc631-dgcl-run_configured_profile_requires_--model-cache")?;
                let script_digest = backend_script_digest
                    .as_deref()
                    .ok_or("lc631-dgcl-run_configured_profile_requires_--backend-script-digest")?;
                let en_config = pins
                    .bind_worker(
                        "en",
                        python,
                        python_digest.clone(),
                        script,
                        script_digest.to_string(),
                        model_dir,
                        model_manifest_en.as_deref().ok_or(
                            "lc631-dgcl-run_configured_profile_requires_--model-manifest-en",
                        )?,
                        model_digest_en
                            .as_deref()
                            .ok_or("lc631-dgcl-run_configured_profile_requires_--model-digest-en")?
                            .to_string(),
                        60_000,
                    )
                    .map_err(|error| format!("dgcl_runtime_binding_en:{error:?}"))?;
                let ja_config = pins
                    .bind_worker(
                        "ja",
                        python,
                        python_digest,
                        script,
                        script_digest.to_string(),
                        model_dir,
                        model_manifest_ja.as_deref().ok_or(
                            "lc631-dgcl-run_configured_profile_requires_--model-manifest-ja",
                        )?,
                        model_digest_ja
                            .as_deref()
                            .ok_or("lc631-dgcl-run_configured_profile_requires_--model-digest-ja")?
                            .to_string(),
                        60_000,
                    )
                    .map_err(|error| format!("dgcl_runtime_binding_ja:{error:?}"))?;
                let pipeline = build_dgcl_pipeline(
                    &source,
                    DgclBackendSet {
                        english: Some(&en_config),
                        japanese: Some(&ja_config),
                    },
                    &mut resources.analysis_runner(),
                )
                .map_err(|error| format!("dgcl_run_pipeline:{error:?}"))?;
                (pipeline, "dgcl-rust-cli-ja-en-alpha2-configured-stanza.v1")
            } else {
                let pipeline = build_dgcl_pipeline(
                    &source,
                    DgclBackendSet {
                        english: None,
                        japanese: None,
                    },
                    &mut resources.analysis_runner(),
                )
                .map_err(|error| format!("dgcl_run_pipeline:{error:?}"))?;
                (pipeline, "dgcl-rust-cli-ja-en-alpha2-no-external-models.v1")
            };
            let candidate_bytes = serde_json::to_vec(&pipeline)
                .map_err(|error| format!("dgcl_run_candidate_json:{error}"))?;
            let candidate = build_dgcl_standalone_candidate(
                &source,
                &pipeline,
                &candidate_bytes,
                "epistesys-dgcl-pipeline.v1",
            )
            .map_err(|error| format!("dgcl_run_candidate:{error:?}"))?;
            emit(resources,
                &command,
                serde_json::json!({
                    "schema_version": "epistesys-dgcl-standalone-run.v1",
                    "input_profile": input_profile,
                    "pipeline": pipeline,
                    "candidate": candidate,
                    "host_send_authorized": false,
                    "output_commit_allowed": false,
                    "authority_created": false,
                    "claim_boundary": "source-to-pipeline-to-exact-standalone-candidate only; configured worker observations are not semantic correctness, and no host callback, sink delivery, or output authority is established"
                }),
            )
        }
        "lc631-dgcl-language" => {
            let root = repo
                .as_deref()
                .ok_or("lc631-dgcl-language_requires_--repo")?;
            let pins = resources.with_view(262_144, |view| load_dgcl_runtime_pins_with_resources(root, view)
                .map_err(|error| format!("dgcl_runtime_pins:{error:?}")))?;
            let source = source_file
                .as_ref()
                .map(|path| read_bounded_rpa_source(path, resources))
                .transpose()?
                .or(prompt)
                .ok_or("lc631-dgcl-language_requires_source")?;
            let language = language
                .as_deref()
                .ok_or("lc631-dgcl-language_requires_--language")?;
            let config = pins
                .bind_worker(
                    language,
                    python
                        .as_deref()
                        .ok_or("lc631-dgcl-language_requires_--python")?,
                    python_digest,
                    backend_script
                        .as_deref()
                        .ok_or("lc631-dgcl-language_requires_--backend-script")?,
                    backend_script_digest
                        .ok_or("lc631-dgcl-language_requires_--backend-script-digest")?,
                    model_cache
                        .as_deref()
                        .ok_or("lc631-dgcl-language_requires_--model-cache")?,
                    model_manifest
                        .as_deref()
                        .ok_or("lc631-dgcl-language_requires_--model-manifest")?,
                    model_manifest_digest
                        .ok_or("lc631-dgcl-language_requires_--model-manifest-digest")?,
                    60_000,
                )
                .map_err(|error| format!("dgcl_runtime_binding:{error:?}"))?;
            emit(resources,
                &command,
                resources.with_source_process_view(&source, 2_101_252, |view|
                    run_language_worker_with_resources(&source, language, &config, view)
                        .map_err(|error| format!("dgcl_language:{error:?}")))?,
            )
        }
        "lc631-dgcl-parse" => {
            let root = repo.as_deref().ok_or("lc631-dgcl-parse_requires_--repo")?;
            let pins = resources.with_view(262_144, |view| load_dgcl_runtime_pins_with_resources(root, view)
                .map_err(|error| format!("dgcl_runtime_pins:{error:?}")))?;
            let source = source_file
                .as_ref()
                .map(|path| read_bounded_rpa_source(path, resources))
                .transpose()?
                .or(prompt)
                .ok_or("lc631-dgcl-parse_requires_source")?;
            let python = python.ok_or("lc631-dgcl-parse_requires_--python")?;
            let script = backend_script.ok_or("lc631-dgcl-parse_requires_--backend-script")?;
            let model_dir = model_cache.ok_or("lc631-dgcl-parse_requires_--model-cache")?;
            let python_digest = python_digest.clone();
            let script_digest = backend_script_digest
                .clone()
                .ok_or("lc631-dgcl-parse_requires_--backend-script-digest")?;
            let en_config = pins
                .bind_worker(
                    "en",
                    &python,
                    python_digest.clone(),
                    &script,
                    script_digest.clone(),
                    &model_dir,
                    model_manifest_en
                        .as_deref()
                        .ok_or("lc631-dgcl-parse_requires_--model-manifest-en")?,
                    model_digest_en
                        .clone()
                        .ok_or("lc631-dgcl-parse_requires_--model-digest-en")?,
                    60_000,
                )
                .map_err(|error| format!("dgcl_runtime_binding_en:{error:?}"))?;
            let ja_config = pins
                .bind_worker(
                    "ja",
                    &python,
                    python_digest,
                    &script,
                    script_digest,
                    &model_dir,
                    model_manifest_ja
                        .as_deref()
                        .ok_or("lc631-dgcl-parse_requires_--model-manifest-ja")?,
                    model_digest_ja.ok_or("lc631-dgcl-parse_requires_--model-digest-ja")?,
                    60_000,
                )
                .map_err(|error| format!("dgcl_runtime_binding_ja:{error:?}"))?;
            emit(resources,
                &command,
                build_dgcl_pipeline(
                    &source,
                    DgclBackendSet {
                        english: Some(&en_config),
                        japanese: Some(&ja_config),
                    },
                    &mut resources.analysis_runner(),
                )
                .map_err(|error| format!("dgcl_parse:{error:?}"))?,
            )
        }
        "lc631-dgcl-evaluate" => {
            let path = gold_file.ok_or("lc631-dgcl-evaluate_requires_--gold-file")?;
            let metadata = fs::metadata(&path).map_err(|error| format!("gold_metadata:{error}"))?;
            if metadata.len() == 0 || metadata.len() > 16 * 1024 * 1024 {
                return Err("dgcl_gold_size_invalid".into());
            }
            let raw = resources.read_path(&path, 16 * 1024 * 1024)?;
            let corpus: GoldCorpus =
                serde_json::from_str(&raw).map_err(|error| format!("gold_json:{error}"))?;
            emit(resources,
                &command,
                evaluate_gold(&corpus).map_err(|error| format!("gold_eval:{error:?}"))?,
            )
        }
        "lc631-dgcl-verify" => {
            if !execute {
                return Err("lc631-dgcl-verify_requires_--execute".into());
            }
            let root = repo.ok_or("lc631-dgcl-verify_requires_--repo")?;
            let path =
                connection_plan_file.ok_or("lc631-dgcl-verify_requires_--connection-plan")?;
            let metadata =
                fs::metadata(&path).map_err(|error| format!("connection_plan_metadata:{error}"))?;
            if metadata.len() == 0 || metadata.len() > 1_048_576 {
                return Err("dgcl_connection_plan_size_invalid".into());
            }
            let raw = resources.read_path(&path, 1_048_576)?;
            let plan: CodingConnectionPlan = serde_json::from_str(&raw)
                .map_err(|error| format!("connection_plan_json:{error}"))?;
            emit(resources,
                &command,
                resources.with_source_process_view(&plan.source, 1_052_672, |view|
                    verify_connection_plan_from_cli_with_resources(&root, &plan, view)
                        .map_err(|error| format!("dgcl_connection:{error:?}")))?,
            )
        }
        "lc631-dgcl-finalize" => {
            if !execute {
                return Err("lc631-dgcl-finalize_requires_--execute".into());
            }
            let root = repo.ok_or("lc631-dgcl-finalize_requires_--repo")?;
            let path =
                connection_plan_file.ok_or("lc631-dgcl-finalize_requires_--connection-plan")?;
            let metadata =
                fs::metadata(&path).map_err(|error| format!("connection_plan_metadata:{error}"))?;
            if metadata.len() == 0 || metadata.len() > 1_048_576 {
                return Err("dgcl_connection_plan_size_invalid".into());
            }
            let raw = resources.read_path(&path, 1_048_576)?;
            let plan: CodingConnectionPlan = serde_json::from_str(&raw)
                .map_err(|error| format!("connection_plan_json:{error}"))?;
            let dgcl_pipeline = build_dgcl_pipeline(
                &plan.source,
                DgclBackendSet {
                    english: None,
                    japanese: None,
                },
                    &mut resources.analysis_runner(),
                )
            .map_err(|error| format!("dgcl_finalization_pipeline:{error:?}"))?;
            let ledger_root = ledger_root.ok_or("lc631-dgcl-finalize_requires_--ledger-root")?;
            let authority_revision =
                authority_revision.ok_or("lc631-dgcl-finalize_requires_--authority-revision")?;
            let receipt_root = receipt_root.ok_or("lc631-dgcl-finalize_requires_--receipt-root")?;
            let plan_digest = stable_sha256(&raw);
            let prior = journal_read(resources, &plan.source, |view| lc631_analysis::inspect_resume_with_resources(
                &ledger_root,
                &plan.source_revision,
                &plan_digest,
                &authority_revision,
                view,
            ))?;
            if prior.latest_sequence.is_some() {
                return Err("dgcl_prior_delivery_requires_manual_revalidation".into());
            }
            let context = resources.host_context(&receipt_root, false)
                .map_err(|error| format!("dgcl_receipt_root:{error:?}"))?;
            let action_key = stable_sha256(&format!(
                "{}\0{}\0{}",
                plan.source_revision, plan_digest, authority_revision
            ));
            let first_digest = journal_write(resources, &plan.source, &ledger_root,
                &ledger_root.join("checkpoint-00000000000000000000.json"), lc631_host::PersistentStoreKind::Checkpoint, |view| lc631_analysis::append_checkpoint_with_resources(
                &ledger_root,
                CheckpointBody {
                    schema_version: "epistesys-dgcl-checkpoint.v2".into(),
                    sequence: 0,
                    source_revision: plan.source_revision.clone(),
                    plan_digest: plan_digest.clone(),
                    authority_revision: authority_revision.clone(),
                    action_key: action_key.clone(),
                    event_kind: CheckpointEventKind::ActionStarted,
                    delivery: DeliveryState::UnknownDelivery,
                    result_digest: None,
                    parent_digest: None,
                },
                view,
            ))?;
            if let Some(heads) = head_root.as_deref() {
                let anchor = issue_checkpoint_head_anchor(
                    context.issuer(),
                    CheckpointHeadRequest {
                        source_revision: &plan.source_revision,
                        plan_digest: &plan_digest,
                        authority_revision: &authority_revision,
                        sequence: Some(0),
                        head_digest: Some(&first_digest),
                        now_epoch: current_epoch_seconds()?,
                        lease_seconds: 3600,
                    },
                )
                .map_err(|error| format!("dgcl_head:{error:?}"))?;
                let persist_epoch = current_epoch_seconds()?;
                journal_write(resources, &plan.source, heads, &checkpoint_head_target(heads, &anchor)?, lc631_host::PersistentStoreKind::CheckpointHead,
                    |view| lc631_analysis::persist_checkpoint_head_with_resources(
                    &ledger_root,
                    heads,
                    &anchor,
                    context.verifier(),
                    persist_epoch,
                    view,
                ))?;
            }
            let (observation, standalone_candidate) = resources.with_source_process_view(&plan.source, 1_052_672, |view|
                observe_connection_plan_from_cli_with_consumer(&root, &plan, view, |_, stdout| {
                    build_dgcl_standalone_candidate(&plan.source, &dgcl_pipeline, stdout, WIRE_SCHEMA)
                        .map_err(|error| lc631_analysis::ConnectionError::Resource(format!("standalone_candidate:{error:?}")))
                })
                    .map_err(|error| format!("dgcl_connection:{error:?}")))?;
            let connection = observation.report().clone();
            if connection.status != lc631_analysis::ConnectionStatus::Observed {
                return Err("dgcl_delivery_unconfirmed_requires_manual_revalidation".into());
            }
            let now_epoch = current_epoch_seconds()?;
            let claim = issue_observed_connection_evidence(
                observation,
                context.issuer(),
                context.verifier(),
                now_epoch,
            )
            .map_err(|error| format!("dgcl_evidence:{error:?}"))?;
            let observed_digest = journal_write(resources, &plan.source, &ledger_root,
                &ledger_root.join("checkpoint-00000000000000000001.json"), lc631_host::PersistentStoreKind::Checkpoint, |view| lc631_analysis::append_checkpoint_with_resources(
                &ledger_root,
                CheckpointBody {
                    schema_version: "epistesys-dgcl-checkpoint.v2".into(),
                    sequence: 1,
                    source_revision: plan.source_revision.clone(),
                    plan_digest: plan_digest.clone(),
                    authority_revision: authority_revision.clone(),
                    action_key,
                    event_kind: CheckpointEventKind::ActionObserved,
                    delivery: DeliveryState::ConfirmedSuccess,
                    result_digest: Some(connection.process.stdout_digest.clone()),
                    parent_digest: Some(first_digest),
                },
                view,
            ))?;
            if let Some(heads) = head_root.as_deref() {
                let anchor = issue_checkpoint_head_anchor(
                    context.issuer(),
                    CheckpointHeadRequest {
                        source_revision: &plan.source_revision,
                        plan_digest: &plan_digest,
                        authority_revision: &authority_revision,
                        sequence: Some(1),
                        head_digest: Some(&observed_digest),
                        now_epoch,
                        lease_seconds: 3600,
                    },
                )
                .map_err(|error| format!("dgcl_head:{error:?}"))?;
                journal_write(resources, &plan.source, heads, &checkpoint_head_target(heads, &anchor)?, lc631_host::PersistentStoreKind::CheckpointHead,
                    |view| lc631_analysis::persist_checkpoint_head_with_resources(
                    &ledger_root,
                    heads,
                    &anchor,
                    context.verifier(),
                    now_epoch,
                    view,
                ))?;
            }
            let closure = build_coding_closure(
                &plan.source,
                &dgcl_pipeline.dg1,
                &[claim],
                Some(context.verifier()),
                &mut ReplayGuard::default(),
                now_epoch,
            )
            .map_err(|error| format!("dgcl_closure:{error:?}"))?;
            emit(resources,
                &command,
                serde_json::json!({
                    "schema_version": "epistesys-dgcl-finalization.v1",
                    "connection": connection,
                    "closure": closure,
                    "standalone_candidate": standalone_candidate,
                    "checkpoint_integrity_state": prior.integrity_state,
                    "rollback_detection": "unavailable_without_external_trusted_head_anchor",
                    "host_output_bound": false,
                    "delivery_observed": false,
                    "final_decision": "hold_host_output_unbound"
                }),
            )
        }
        "lc631-dgcl-resume" => {
            let path =
                connection_plan_file.ok_or("lc631-dgcl-resume_requires_--connection-plan")?;
            let raw = resources.read_path(&path, 1_048_576)?;
            if raw.len() > 1_048_576 {
                return Err("dgcl_connection_plan_size_invalid".into());
            }
            let plan: CodingConnectionPlan = serde_json::from_str(&raw)
                .map_err(|error| format!("connection_plan_json:{error}"))?;
            let root = ledger_root.ok_or("lc631-dgcl-resume_requires_--ledger-root")?;
            let authority =
                authority_revision.ok_or("lc631-dgcl-resume_requires_--authority-revision")?;
            let resumed = if let Some(heads) = head_root.as_deref() {
                let context = resources.host_context(
                    &receipt_root.ok_or("dgcl_anchored_resume_requires_--receipt-root")?,
                    false,
                )
                .map_err(|error| format!("dgcl_receipt_root:{error:?}"))?;
                let now = current_epoch_seconds()?;
                let anchor = journal_read(resources, &plan.source, |view| lc631_analysis::load_checkpoint_head_with_resources(&root, heads, context.verifier(), now, view))?;
                journal_read(resources, &plan.source, |view| lc631_analysis::inspect_resume_with_anchor_with_resources(
                    &root,
                    lc631_analysis::CheckpointReadIdentity { source_revision: &plan.source_revision, plan_digest: &stable_sha256(&raw), authority_revision: &authority },
                    Some(&anchor),
                    Some(context.verifier()),
                    now,
                    view,
                ))
            } else {
                journal_read(resources, &plan.source, |view| lc631_analysis::inspect_resume_with_resources(
                    &root,
                    &plan.source_revision,
                    &stable_sha256(&raw),
                    &authority,
                    view,
                ))
            }
            ?;
            emit(resources, &command, resumed)
        }
        "lc631-coding-closure" => {
            let source = prompt.as_deref().unwrap_or("");
            let dg1 = analyze_dg1(source, Dg1Budget::default())
                .map_err(|error| format!("dg1:{error:?}"))?;
            let closure = build_coding_closure(
                source,
                &dg1,
                &[],
                None,
                &mut ReplayGuard::default(),
                current_epoch_seconds()?,
            )
            .map_err(|error| format!("coding_closure:{error:?}"))?;
            emit(resources, &command, closure)
        }
        "lc631-rpa-doctor" => {
            let source = source_file
                .as_ref()
                .map(|path| read_bounded_rpa_source(path, resources))
                .transpose()?;
            let source_name = source_file
                .as_ref()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str());
            let repo_text = repo.as_ref().map(|path| path.to_string_lossy().to_string());
            emit(resources,
                &command,
                analyze_program_path(
                    AnalysisRequest {
                        prompt: prompt.as_deref().unwrap_or("audit program analysis"),
                        repo: repo_text.as_deref(),
                        validation: None,
                        release: None,
                        source_name,
                        source: source.as_deref(),
                    },
                    source_file.as_deref(),
                    execute,
                    &mut resources.analysis_runner(),
                ).map_err(|error| format!("rpa_resource:{error:?}"))?,
            )
        }
        "lc631-tldg-adversarial" => emit(resources,
            &command,
            run_adversarial_evaluation().map_err(|error| format!("tldg_adversarial:{error}"))?,
        ),
        "lc631-tldg-gpu-doctor" => {
            let source = prompt
                .as_deref()
                .unwrap_or("Parse natural language and program syntax together.");
            let artifact = analyze_tldg(source).map_err(|error| format!("{error:?}"))?;
            let kernel =
                build_tldg_structural_kernel(&artifact).map_err(|error| format!("{error:?}"))?;
            emit(resources,
                &command,
                run_tldg_geometry_accelerator(&kernel, execute)
                    .map_err(|error| format!("{error:?}"))?,
            )
        }
        "lc631-tldg-release-gate" => {
            let source = prompt
                .as_deref()
                .unwrap_or("Parse natural language and program syntax together.");
            let report = build_tldg_pipeline(source, None, &CalibrationPartition::reference())
                .map_err(|error| format!("{error:?}"))?;
            let adversarial = run_adversarial_evaluation()
                .map_err(|error| format!("tldg_adversarial:{error}"))?;
            if execute {
                let artifact = analyze_tldg(source).map_err(|error| format!("{error:?}"))?;
                let kernel = build_tldg_structural_kernel(&artifact)
                    .map_err(|error| format!("{error:?}"))?;
                let acceleration = run_tldg_geometry_accelerator(&kernel, true)
                    .map_err(|error| format!("{error:?}"))?;
                emit(resources,
                    &command,
                    build_tldg_release_gate_with_receipts(
                        &report,
                        &adversarial,
                        acceleration.real_gpu_observed,
                        acceleration.all_lane_parity,
                        false,
                    ),
                )
            } else {
                emit(resources, &command, build_tldg_release_gate(&report, &adversarial))
            }
        }
        "lc631-gpu-doctor" => emit(resources, &command, build_accelerator_doctor(execute)?),
        "lc631-gpu-stress" => emit(resources,
            &command,
            build_gpu_stress(iterations.unwrap_or(16), workers.unwrap_or(4), resources)?,
        ),
        "lc631-media-doctor" => emit(resources, &command, build_media_doctor()),
        "lc631-media-execute" | "lc631-media-execute-v3" => {
            let media = media.ok_or("lc631-media-execute_requires_--media")?;
            let request = MediaSourceRequest {
                kind: MediaSourceKind::LocalUserProvided,
                locator: media.display().to_string(),
                user_asserts_rights: rights_asserted,
                media_bytes_requested: true,
                network_requested: false,
            };
            let source_receipt_file =
                source_receipt_file.ok_or("lc631-media-execute_requires_--source-receipt-file")?;
            let source_receipt: UntrustedReceipt = serde_json::from_str(
                &resources.read_path(&source_receipt_file, 65_536)?,
            )
            .map_err(|error| format!("source_receipt_json:{error}"))?;
            let receipt_root = receipt_root.ok_or("lc631-media-execute_requires_--receipt-root")?;
            let context = resources.host_context(&receipt_root, true)
                .map_err(|error| format!("media_receipt_root:{error:?}"))?;
            let now_epoch = current_epoch_seconds()?;
            let authority = resources.with_view(65_536, |view| authorize_media_source_attested_with_resources(
                &request,
                source_receipt,
                context.verifier(),
                &mut ReplayGuard::default(),
                now_epoch,
                view,
            )
            .map_err(|error| format!("media_source_authority:{error:?}")))?;
            let config = MediaBackendConfig {
                python_executable: python
                    .or_else(|| configured_path("LC631_PYTHON"))
                    .ok_or("lc631-media-execute_requires_--python_or_LC631_PYTHON")?,
                backend_script: backend_script
                    .or_else(|| configured_path("LC631_MEDIA_BACKEND"))
                    .ok_or(
                        "lc631-media-execute_requires_--backend-script_or_LC631_MEDIA_BACKEND",
                    )?,
                ffmpeg_executable: ffmpeg
                    .or_else(|| configured_path("LC631_FFMPEG"))
                    .ok_or("lc631-media-execute_requires_--ffmpeg_or_LC631_FFMPEG")?,
                model_cache: model_cache
                    .or_else(|| configured_path("LC631_MODEL_CACHE"))
                    .ok_or("lc631-media-execute_requires_--model-cache_or_LC631_MODEL_CACHE")?,
                timeout_seconds: 1_800,
            };
            let descriptor = BackendDescriptor::configured(
                BackendFamily::Decode,
                backend_id.ok_or("lc631-media-execute_requires_--backend-id")?,
                Some(config.backend_script.clone()),
                Some(
                    resources.with_view(8 * 1024 * 1024, |view|
                        model_cache_manifest_sha256_bounded(&config.model_cache, ModelScanLimits::default(), Some(view))
                            .map_err(|error| format!("model_cache_manifest:{error:?}")))?,
                ),
                Some(
                    model_license_receipt
                        .ok_or("lc631-media-execute_requires_--model-license-receipt")?,
                ),
            );
            let budget = MediaResourceBudget { max_temporary_bytes: media_temp_bytes, ..MediaResourceBudget::default() };
            let source_digest = io::parse_source_digest(authority.source_snapshot_sha256.as_deref().ok_or("media_source_digest_missing")?)?;
            let output_bytes = budget.max_stdout_bytes.checked_add(budget.max_stderr_bytes).ok_or("media_output_budget_overflow")? as u64;
            emit(resources,
                &command,
                resources.with_media_process_view(source_digest, output_bytes, media_temp_bytes, |view| {
                    let mut replay = ReplayGuard::default();
                    let receipt = if command == "lc631-media-execute-v3" {
                        // Source authorization and model inventory can consume
                        // time. The v3 execution anchor must start at entry,
                        // not at the earlier CLI preparation timestamp.
                        let execution_epoch = current_epoch_seconds()?;
                        lc631_media::execute_local_media_backend_attested_v3_with_resources(
                            &authority, &descriptor, &config, &budget,
                            context.issuer(), context.verifier(), &mut replay, execution_epoch, view,
                        )
                    } else {
                        execute_local_media_backend_attested_with_resources(
                            &authority, &descriptor, &config, &budget,
                            context.issuer(), context.verifier(), &mut replay, now_epoch, view,
                        )
                    };
                    receipt.map_err(|error| format!("{error:?}"))
                })?,
            )
        }
        "lc631-paired-shadow" => {
            let prompt = prompt.ok_or("lc631-paired-shadow_requires_--prompt")?;
            let v630 = v630_digest.ok_or("lc631-paired-shadow_requires_--v630-digest")?;
            let v631 = v631_digest.ok_or("lc631-paired-shadow_requires_--v631-digest")?;
            emit(resources,
                &command,
                PairedObservation::shadow(
                    stable_sha256(&prompt),
                    v630.clone(),
                    v631.clone(),
                    if v630 == v631 {
                        vec![DivergenceKind::None]
                    } else {
                        vec![DivergenceKind::TranslationWitness]
                    },
                ),
            )
        }
        "lc631-host-bind" => emit(resources,
            &command,
            build_host_bind(HostBindInput {
                seed_file: seed_file.ok_or("lc631-host-bind_requires_--seed-file")?,
                output_file: output_file.ok_or("lc631-host-bind_requires_--output-file")?,
                callback_file: callback_file.ok_or("lc631-host-bind_requires_--callback-file")?,
                host_receipt_file: host_receipt_file
                    .ok_or("lc631-host-bind_requires_--host-receipt-file")?,
                ledger_root: ledger_root.ok_or("lc631-host-bind_requires_--ledger-root")?,
                ledger: ledger.ok_or("lc631-host-bind_requires_--ledger")?,
                artifact: ArtifactId(artifact.ok_or("lc631-host-bind_requires_--artifact")?),
                turn: lc631_core::TurnId(turn.ok_or("lc631-host-bind_requires_--turn")?),
            }, resources)?,
        ),
        "lc631-host-stop-hook" => run_host_stop_hook(resources),
        "lc631-promotion-gate" => match (evidence_file, receipt_root) {
            (None, None) => emit(resources, &command, PromotionGate::alpha_default()),
            (Some(_), None) => Err("lc631-promotion-gate_requires_--receipt-root".into()),
            (None, Some(_)) => Err("lc631-promotion-gate_requires_--evidence-file".into()),
            (Some(evidence_file), Some(receipt_root)) => emit(resources,
                &command,
                build_promotion_gate_from_evidence(&evidence_file, &receipt_root, resources)?,
            ),
        },
        _ => Err(format!("unknown_lc631_command:{command}")),
    }.map_err(CliFailure::from)
}

fn run_checkpoint_resume_inspect(
    resources: &io::CommandIo,
    command: &str,
    args: binary_checkpoint_resume::BinaryCheckpointResumeInspectArgs,
) -> Result<(), CliFailure> {
    let identity = lc631_analysis::CheckpointReadIdentity {
        source_revision: &args.source_revision,
        plan_digest: &args.plan_digest,
        authority_revision: &args.authority_revision,
    };
    let report = resources
        .with_view(18 * 1024 * 1024, |view| {
            let decision = binary_checkpoint_resume::inspect_existing_binary_checkpoint_root(
                &args.ledger_root,
                view,
                lc631_runtime::journal::JournalChainLimits::default(),
                identity,
            );
            Ok::<_, String>(
                binary_checkpoint_resume::report_binary_checkpoint_resume_inspection(&decision),
            )
        })
        .map_err(CliFailure::from)?;

    let envelope = lc631_wire::WireEnvelope::shadow(command, report);
    resources
        .with_view(4 * 1024, |view| {
            io::write_pretty_json(&envelope, view, &mut std::io::stdout().lock())
                .map_err(|error| format!("checkpoint_resume_inspect_output:{error:?}"))
        })
        .map_err(CliFailure::from)
}

fn run_host_stop_hook(resources: &io::CommandIo) -> Result<(), String> {
    let raw = resources.read_from(&mut std::io::stdin().lock(), MAX_PROMPT_STDIN_BYTES as u64)?;
    let event: CodexStopHookEvent =
        serde_json::from_str(&raw).map_err(|error| format!("stop_hook_json:{error}"))?;
    if event.stop_hook_active {
        write_stdout("{\"continue\":true}")?;
        return Ok(());
    }
    let plugin_data = env::var_os("PLUGIN_DATA")
        .or_else(|| env::var_os("CLAUDE_PLUGIN_DATA"))
        .map(PathBuf::from)
        .ok_or("stop_hook_PLUGIN_DATA_unavailable")?;
    let replay_root = plugin_data.join("host-output-hook");
    let context = resources
        .host_context(&replay_root.join("receipt-root"), true)
        .map_err(|error| format!("stop_hook_receipt_root:{error:?}"))?;
    let now_epoch = current_epoch_seconds()?;
    let (receipt, observation) = bind_codex_stop_hook_attested(
        &event,
        &raw,
        context.issuer(),
        context.verifier(),
        &mut ReplayGuard::default(),
        now_epoch,
    )
    .map_err(|error| format!("stop_hook_bind:{error:?}"))?;
    let ledger = replay_root.join("host-output-v2.jsonl");
    // Shared-root scans include an 8 MiB ledger plus a one-byte sentinel.
    let replay = resources.with_storage_view(8 * 1024 * 1024 + 1, 4_096, |view| {
        append_durable_replay_verified_with_resources(
            &replay_root,
            &ledger,
            &receipt,
            context.verifier(),
            &mut ReplayGuard::default(),
            now_epoch,
            view,
        )
        .map_err(|error| format!("stop_hook_replay:{error:?}"))
    })?;
    if !replay.chain_verified {
        return Err("stop_hook_replay_unverified".into());
    }
    resources.with_storage_view(8 * 1024 * 1024 + 1, 1_048_576, |view| {
        persist_stop_hook_observation_with_resources(&replay_root, &observation, view)
            .map_err(|error| format!("stop_hook_observation:{error:?}"))
    })?;
    write_stdout("{\"continue\":true}")?;
    Ok(())
}

fn current_epoch_seconds() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system_clock:{error}"))
        .map(|duration| duration.as_secs())
}

fn journal_read<T, E: std::fmt::Debug>(
    resources: &io::CommandIo,
    source: &str,
    operation: impl FnOnce(&lc631_runtime::resource::ResourceReservationView<'_>) -> Result<T, E>,
) -> Result<T, String> {
    dgcl_journal::resources::read(Some((resources, source)), |view| {
        let view = view.ok_or("journal_resource_view_missing")?;
        operation(view).map_err(|error| format!("journal_read:{error:?}"))
    })
}

fn journal_write<T, E: std::fmt::Debug>(
    resources: &io::CommandIo,
    source: &str,
    root: &Path,
    target: &Path,
    kind: lc631_host::PersistentStoreKind,
    operation: impl FnOnce(&lc631_runtime::resource::ResourceReservationView<'_>) -> Result<T, E>,
) -> Result<T, String> {
    dgcl_journal::resources::write((resources, source), root, target, kind, |view| {
        operation(view).map_err(|error| format!("journal_write:{error:?}"))
    })
}

fn checkpoint_head_target(
    root: &Path,
    anchor: &lc631_analysis::CheckpointHeadAnchor,
) -> Result<PathBuf, String> {
    let index = match anchor.sequence {
        None => 0,
        Some(sequence) => sequence
            .checked_add(1)
            .ok_or("checkpoint_sequence_exhausted")?,
    };
    Ok(root.join(format!("head-{index:020}.json")))
}

fn emit<T: Serialize>(resources: &io::CommandIo, command: &str, payload: T) -> Result<(), String> {
    resources.write_public(command, payload)
}

fn write_stdout(value: &str) -> Result<(), String> {
    // Only the fixed stop-hook control response uses this compatibility path.
    // Check the complete value before writing, including its trailing LF.
    if value.len() >= 1024 {
        return Err("fixed_stdout_budget_exceeded".into());
    }
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    writeln!(output, "{value}").map_err(|error| format!("stdout_write:{error}"))
}

fn read_bounded_rpa_source(path: &Path, resources: &io::CommandIo) -> Result<String, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("rpa_source_metadata:{error}"))?;
    if !metadata.is_file() {
        return Err("rpa_source_not_regular_file".to_string());
    }
    if metadata.len() > MAX_RPA_SOURCE_FILE_BYTES {
        return Err(format!(
            "rpa_source_too_large:{}>{MAX_RPA_SOURCE_FILE_BYTES}",
            metadata.len()
        ));
    }
    resources.read_path(path, MAX_RPA_SOURCE_FILE_BYTES)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PromotionEvidenceFile {
    schema_version: String,
    binding: PromotionEvidenceBinding,
    receipts: Vec<PromotionEvidenceReceipt>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PromotionEvidenceBinding {
    candidate_commit: String,
    worktree_clean: bool,
    engine_version: String,
    v630_baseline_commit: String,
    package_binary_sha256: String,
    installed_binary_sha256: String,
    host_identity_sha256: String,
    platform: String,
    device_identity_sha256: String,
    driver_sha256: String,
    corpus_sha256: String,
    benchmark_settings_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PromotionEvidenceReceipt {
    predicate: String,
    slot: String,
    receipt: UntrustedReceipt,
}

fn build_promotion_gate_from_evidence(
    evidence_file: &Path,
    receipt_root: &Path,
    resources: &io::CommandIo,
) -> Result<PromotionGate, String> {
    let evidence = read_bounded_promotion_evidence(evidence_file, resources)?;
    if evidence.schema_version != PROMOTION_EVIDENCE_SCHEMA {
        return Err("promotion_evidence_schema_version".into());
    }
    let binding_digest = promotion_binding_digest(&evidence.binding)?;
    let subject = SubjectRevision::checked(binding_digest.clone())
        .map_err(|_| "promotion_evidence_subject_revision")?;
    let context = resources
        .host_context(receipt_root, false)
        .map_err(|error| format!("promotion_receipt_root:{error:?}"))?;
    let now_epoch = current_epoch_seconds()?;
    let mut replay = ReplayGuard::default();
    let mut receipts = BTreeMap::<String, BTreeMap<String, UntrustedReceipt>>::new();

    for entry in evidence.receipts {
        let requirement = promotion_receipt_requirements(&entry.predicate)
            .and_then(|requirements| {
                requirements
                    .iter()
                    .find(|requirement| requirement.slot == entry.slot)
            })
            .ok_or_else(|| {
                format!(
                    "promotion_evidence_unknown_predicate_or_slot:{}:{}",
                    entry.predicate, entry.slot
                )
            })?;
        if entry.receipt.claims().class() != requirement.class {
            return Err(format!(
                "promotion_evidence_wrong_class:{}:{}",
                entry.predicate, entry.slot
            ));
        }
        if receipts
            .entry(entry.predicate.clone())
            .or_default()
            .insert(entry.slot.clone(), entry.receipt)
            .is_some()
        {
            return Err(format!(
                "promotion_evidence_duplicate_receipt_slot:{}:{}",
                entry.predicate, entry.slot
            ));
        }
    }

    let mut predicates = BTreeMap::new();
    let mut accepted_receipt_digests = Vec::new();
    let mut rejected_evidence = Vec::new();
    for predicate in PROMOTION_PREDICATES {
        let required =
            promotion_receipt_requirements(predicate).expect("fixed predicate requirement mapping");
        let observed = receipts.get(predicate);
        let passed = match observed {
            Some(observed)
                if required
                    .iter()
                    .all(|requirement| observed.contains_key(requirement.slot)) =>
            {
                for requirement in required {
                    let receipt = observed
                        .get(requirement.slot)
                        .expect("required receipt exists after all check")
                        .clone();
                    let claims = receipt.claims();
                    if claims.expires_at_epoch().is_none() {
                        return Err(format!(
                            "promotion_evidence_expiry_required:{predicate}:{}",
                            requirement.slot
                        ));
                    }
                    if claims
                        .issued_at_epoch()
                        .saturating_add(MAX_PROMOTION_RECEIPT_AGE_SECONDS)
                        < now_epoch
                    {
                        return Err(format!(
                            "promotion_evidence_stale:{predicate}:{}",
                            requirement.slot
                        ));
                    }
                    let scope = promotion_receipt_scope(predicate, requirement.slot)?;
                    let payload = promotion_receipt_payload_digest(
                        &binding_digest,
                        predicate,
                        requirement.slot,
                    );
                    let verified = context
                        .verifier()
                        .verify(
                            receipt,
                            &ReceiptPolicy::exact(
                                requirement.class,
                                subject.clone(),
                                scope,
                                payload,
                                now_epoch,
                            ),
                            &mut replay,
                        )
                        .map_err(|error| {
                            format!(
                                "promotion_evidence_verify:{predicate}:{}:{error:?}",
                                requirement.slot
                            )
                        })?;
                    accepted_receipt_digests.push(verified.receipt_digest());
                }
                true
            }
            _ => {
                for requirement in required {
                    if observed.is_none_or(|observed| !observed.contains_key(requirement.slot)) {
                        rejected_evidence.push(format!("{predicate}:missing:{}", requirement.slot));
                    }
                }
                false
            }
        };
        predicates.insert(predicate.to_string(), passed);
    }
    Ok(PromotionGate::evaluate(predicates).with_evidence(
        binding_digest,
        accepted_receipt_digests,
        rejected_evidence,
    ))
}

fn read_bounded_promotion_evidence(
    path: &Path,
    resources: &io::CommandIo,
) -> Result<PromotionEvidenceFile, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("promotion_evidence_metadata:{error}"))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err("promotion_evidence_not_regular_file".into());
    }
    if metadata.len() > MAX_PROMOTION_EVIDENCE_FILE_BYTES {
        return Err(format!(
            "promotion_evidence_too_large:{}>{MAX_PROMOTION_EVIDENCE_FILE_BYTES}",
            metadata.len()
        ));
    }
    serde_json::from_str(&resources.read_path(path, MAX_PROMOTION_EVIDENCE_FILE_BYTES)?)
        .map_err(|error| format!("promotion_evidence_json:{error}"))
}

#[derive(Clone, Copy)]
struct PromotionReceiptRequirement {
    slot: &'static str,
    class: ReceiptClass,
}

fn promotion_receipt_requirements(
    predicate: &str,
) -> Option<&'static [PromotionReceiptRequirement]> {
    match predicate {
        "v630_frozen" => Some(&[PromotionReceiptRequirement {
            slot: "baseline",
            class: ReceiptClass::Baseline,
        }]),
        "namespace_collision_free" => Some(&[PromotionReceiptRequirement {
            slot: "validation",
            class: ReceiptClass::Validation,
        }]),
        "translation_witness_end_to_end" => Some(&[PromotionReceiptRequirement {
            slot: "closure",
            class: ReceiptClass::Closure,
        }]),
        "world_budget_exact_2048" => Some(&[PromotionReceiptRequirement {
            slot: "validation",
            class: ReceiptClass::Validation,
        }]),
        "world_distinctness" => Some(&[PromotionReceiptRequirement {
            slot: "validation",
            class: ReceiptClass::Validation,
        }]),
        "gpu_mandatory_lane_coverage" => Some(&[PromotionReceiptRequirement {
            slot: "accelerator",
            class: ReceiptClass::Accelerator,
        }]),
        "gpu_fault_contained" => Some(&[PromotionReceiptRequirement {
            slot: "fault",
            class: ReceiptClass::Accelerator,
        }]),
        "media_source_authority" => Some(&[PromotionReceiptRequirement {
            slot: "source",
            class: ReceiptClass::MediaSource,
        }]),
        "media_backends_observed" => Some(&[PromotionReceiptRequirement {
            slot: "backend",
            class: ReceiptClass::MediaBackend,
        }]),
        "host_output_bound" => Some(&[PromotionReceiptRequirement {
            slot: "host-output",
            class: ReceiptClass::HostOutput,
        }]),
        "paired_v630_v631_regression" => Some(&[PromotionReceiptRequirement {
            slot: "evaluation",
            class: ReceiptClass::ReleaseEvaluation,
        }]),
        "full_validation" => Some(&[
            PromotionReceiptRequirement {
                slot: "local-validation",
                class: ReceiptClass::Validation,
            },
            PromotionReceiptRequirement {
                slot: "remote-ci",
                class: ReceiptClass::RemoteCi,
            },
        ]),
        "package_installed_host_pickup" => Some(&[
            PromotionReceiptRequirement {
                slot: "package",
                class: ReceiptClass::Package,
            },
            PromotionReceiptRequirement {
                slot: "host-pickup",
                class: ReceiptClass::ToolExecution,
            },
        ]),
        _ => None,
    }
}

fn promotion_receipt_scope(predicate: &str, slot: &str) -> Result<ReceiptScope, String> {
    ReceiptScope::checked(format!("promotion/{predicate}/{slot}"))
        .map_err(|_| "promotion_evidence_scope".into())
}

fn promotion_receipt_payload_digest(binding_digest: &str, predicate: &str, slot: &str) -> String {
    stable_sha256(&format!(
        "{PROMOTION_EVIDENCE_SCHEMA}\0{binding_digest}\0{predicate}\0{slot}\0pass"
    ))
}

fn promotion_binding_digest(binding: &PromotionEvidenceBinding) -> Result<String, String> {
    if !binding.worktree_clean {
        return Err("promotion_evidence_dirty_worktree".into());
    }
    if binding.engine_version != LC631_VERSION {
        return Err("promotion_evidence_engine_version".into());
    }
    if binding.v630_baseline_commit != V630_FROZEN_COMMIT {
        return Err("promotion_evidence_v630_baseline_commit".into());
    }
    if !is_lower_sha256(&binding.package_binary_sha256)
        || binding.package_binary_sha256 != binding.installed_binary_sha256
        || !is_lower_sha256(&binding.installed_binary_sha256)
        || !is_lower_sha256(&binding.host_identity_sha256)
        || !is_lower_sha256(&binding.device_identity_sha256)
        || !is_lower_sha256(&binding.driver_sha256)
        || !is_lower_sha256(&binding.corpus_sha256)
        || !is_lower_sha256(&binding.benchmark_settings_sha256)
    {
        return Err("promotion_evidence_binding_digest".into());
    }
    if binding.candidate_commit.len() != 40
        || !binding
            .candidate_commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || binding.platform.trim().is_empty()
        || binding.platform.len() > 128
        || binding.platform.chars().any(char::is_control)
    {
        return Err("promotion_evidence_binding_identity".into());
    }
    Ok(stable_sha256(&format!(
        "{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
        binding.candidate_commit,
        binding.worktree_clean,
        binding.engine_version,
        binding.v630_baseline_commit,
        binding.package_binary_sha256,
        binding.installed_binary_sha256,
        binding.host_identity_sha256,
        binding.platform,
        binding.device_identity_sha256,
        binding.driver_sha256,
        binding.corpus_sha256,
        binding.benchmark_settings_sha256,
    )))
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn read_prompt_stdin(resources: &io::CommandIo) -> Result<String, String> {
    let prompt =
        resources.read_from(&mut std::io::stdin().lock(), MAX_PROMPT_STDIN_BYTES as u64)?;
    if prompt.trim().is_empty() {
        return Err("prompt_stdin_is_empty".into());
    }
    Ok(prompt)
}

fn take_option(args: &mut Vec<String>, name: &str) -> Option<String> {
    let index = args.iter().position(|arg| arg == name)?;
    if index + 1 >= args.len() {
        return None;
    }
    args.remove(index);
    Some(args.remove(index))
}

fn take_flag(args: &mut Vec<String>, name: &str) -> bool {
    args.iter()
        .position(|arg| arg == name)
        .map(|index| {
            args.remove(index);
            true
        })
        .unwrap_or(false)
}

#[derive(Serialize)]
struct TlDoctorReport {
    obligation_count: usize,
    witness_count: usize,
    chain_complete: bool,
    gate: ProjectionGateDecision,
    legacy_divergence: bool,
    fixed_numeric_threshold_used: bool,
    source_roundtrip_exact: bool,
    parse_defect_count: usize,
    geometry_direct_promotions: usize,
    tldg_local_source_ready: bool,
    claim_boundary: &'static str,
}

fn build_tl_doctor(source: &str) -> Result<TlDoctorReport, String> {
    if source.is_empty() {
        return Err("empty_tl_source".into());
    }
    let report = build_tldg_pipeline(source, None, &CalibrationPartition::reference())
        .map_err(|error| format!("tldg_tl_doctor:{error:?}"))?;
    let gate = report.output.defect_gate;
    let comparison = compare_legacy_route("review", gate);
    Ok(TlDoctorReport {
        obligation_count: 1,
        witness_count: 1,
        chain_complete: report.output.exact_host_output_bound,
        gate,
        legacy_divergence: comparison.divergence,
        fixed_numeric_threshold_used: false,
        source_roundtrip_exact: report.parse.source_roundtrip == source,
        parse_defect_count: report.empirical_risk.prediction_set.len(),
        geometry_direct_promotions: report.distillation.geometry_direct_promotions,
        tldg_local_source_ready: report.local_source_ready,
        claim_boundary:
            "source roundtrip is observed; Program IR, host output, and semantic exactness remain unresolved",
    })
}

#[derive(Serialize)]
struct WorldDoctorReport {
    budget: WorldBudget,
    distinct_worlds: usize,
    evaluations: usize,
    materialized_evaluations: usize,
    duplicate_padding: usize,
    exact_ceiling: bool,
}

fn build_world_doctor(prompt: &str) -> Result<WorldDoctorReport, String> {
    let budget = WorldBudget::checked(256, 8).map_err(|error| format!("{error:?}"))?;
    let worlds = generate_distinct_worlds(prompt).map_err(|error| format!("{error:?}"))?;
    let evaluations = evaluation_matrix(&worlds).map_err(|error| format!("{error:?}"))?;
    let materialized = materialize_evaluations(&worlds).map_err(|error| format!("{error:?}"))?;
    Ok(WorldDoctorReport {
        budget,
        distinct_worlds: worlds.len(),
        evaluations: evaluations.len(),
        materialized_evaluations: materialized.len(),
        duplicate_padding: 0,
        exact_ceiling: evaluations.len() == 2_048 && materialized.len() == 2_048,
    })
}

#[derive(Serialize)]
struct AcceleratorDoctorReport {
    execute_requested: bool,
    cpu_receipt: ExecutionReceipt,
    gpu_receipt: Option<ExecutionReceipt>,
    lane_parity: BTreeMap<lc631_accelerator::LaneKind, bool>,
    gpu_error: Option<String>,
    contract_lane_coverage_complete: bool,
    gpu_logical_lane_coverage_complete: bool,
    hardware_saturation_claim: bool,
}

fn binding(device: &str, kernel: &str) -> RevisionBinding {
    RevisionBinding {
        artifact: ArtifactId(631),
        source_sha256: stable_sha256("lc631-doctor-source"),
        program_sha256: stable_sha256("lc631-doctor-program"),
        authority_revision: AuthorityRevision(1),
        engine_version: env!("CARGO_PKG_VERSION").into(),
        kernel_revision: kernel.into(),
        device_identity: device.into(),
    }
}

fn build_accelerator_doctor(execute: bool) -> Result<AcceleratorDoctorReport, String> {
    let arena =
        WorldArena::new("lc631-accelerator-doctor").map_err(|error| format!("{error:?}"))?;
    let evaluations =
        materialize_evaluations(arena.worlds()).map_err(|error| format!("{error:?}"))?;
    let input = input_from_evaluations(&evaluations).map_err(|error| format!("{error:?}"))?;
    let mut cpu = AcceleratorBroker::new(CpuBackend);
    let cpu_receipt = cpu
        .execute_epoch(EpochId(1), &binding("cpu", "cpu-v1"), &input)
        .map_err(|error| format!("{error:?}"))?;
    let (gpu_receipt, gpu_error, lane_parity) = run_gpu_doctor(execute, &input, &cpu_receipt);
    let contract_lane_coverage_complete =
        cpu_receipt.logical_lane_coverage == cpu_receipt.required_logical_lane_count;
    let gpu_logical_lane_coverage_complete = gpu_receipt.as_ref().is_some_and(|receipt| {
        receipt.logical_lane_coverage == receipt.required_logical_lane_count
            && lane_parity.values().all(|matched| *matched)
    });
    Ok(AcceleratorDoctorReport {
        execute_requested: execute,
        cpu_receipt,
        gpu_receipt,
        lane_parity,
        gpu_error,
        contract_lane_coverage_complete,
        gpu_logical_lane_coverage_complete,
        hardware_saturation_claim: false,
    })
}

#[cfg(feature = "gpu-cubecl-wgpu")]
fn run_gpu_doctor(
    execute: bool,
    input: &lc631_accelerator::NumericEpochInput,
    cpu_receipt: &ExecutionReceipt,
) -> (
    Option<ExecutionReceipt>,
    Option<String>,
    BTreeMap<lc631_accelerator::LaneKind, bool>,
) {
    if !execute {
        return (None, None, BTreeMap::new());
    }
    let mut gpu = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
    match gpu.execute_epoch(
        EpochId(1),
        &binding("cubecl-wgpu", "lc631-four-lane.v1"),
        input,
    ) {
        Ok(receipt) => {
            let parity = compare_lane_digests(cpu_receipt, &receipt);
            (Some(receipt), None, parity)
        }
        Err(error) => (None, Some(format!("{error:?}")), BTreeMap::new()),
    }
}

#[cfg(not(feature = "gpu-cubecl-wgpu"))]
fn run_gpu_doctor(
    execute: bool,
    _input: &lc631_accelerator::NumericEpochInput,
    _cpu_receipt: &ExecutionReceipt,
) -> (
    Option<ExecutionReceipt>,
    Option<String>,
    BTreeMap<lc631_accelerator::LaneKind, bool>,
) {
    (
        None,
        execute.then(|| "gpu_feature_not_compiled".to_string()),
        BTreeMap::new(),
    )
}

#[cfg(feature = "gpu-cubecl-wgpu")]
#[derive(Clone, Debug, Default, Serialize)]
struct NvidiaUtilizationReceipt {
    nvidia_smi_available: bool,
    sample_count: usize,
    max_gpu_utilization_percent: Option<u32>,
    max_memory_utilization_percent: Option<u32>,
    peak_device_memory_used_mib: Option<u64>,
    device_memory_total_mib: Option<u64>,
    hardware_saturation_observed: bool,
    shader_occupancy: MeasurementState,
    claim_boundary: &'static str,
}

#[cfg(feature = "gpu-cubecl-wgpu")]
#[derive(Serialize)]
struct GpuStressReport {
    workers: usize,
    iterations_per_worker: usize,
    completed_epochs: usize,
    failed_epochs: usize,
    wall_elapsed_ms: u128,
    host_inflight_overlap_observed: bool,
    full_readback_observed: bool,
    maximum_readback_values: usize,
    all_seven_lanes_observed: bool,
    all_required_lanes_observed: bool,
    device_loss_callback_registered: bool,
    utilization: NvidiaUtilizationReceipt,
    claim_boundary: &'static str,
}

#[cfg(feature = "gpu-cubecl-wgpu")]
fn build_gpu_stress(
    iterations: usize,
    workers: usize,
    resources: &io::CommandIo,
) -> Result<GpuStressReport, String> {
    if iterations == 0 || iterations > 1_024 {
        return Err("gpu_stress_iterations_out_of_range".into());
    }
    if workers == 0 || workers > lc631_accelerator::MAX_QUEUE_CAPACITY {
        return Err("gpu_stress_workers_out_of_range".into());
    }
    let arena = WorldArena::new("lc631-gpu-stress").map_err(|error| format!("{error:?}"))?;
    let evaluations =
        materialize_evaluations(arena.worlds()).map_err(|error| format!("{error:?}"))?;
    let input =
        Arc::new(input_from_evaluations(&evaluations).map_err(|error| format!("{error:?}"))?);

    // Warm shader compilation and install the real WGPU device-loss callback
    // before timing or utilization sampling begins.
    let mut warm = AcceleratorBroker::new(CubeClWgpuBackend::default());
    warm.execute_epoch(
        EpochId(1),
        &binding("cubecl-wgpu", "lc631-bounded-summary.v1"),
        &input,
    )
    .map_err(|error| format!("gpu_stress_warmup:{error:?}"))?;

    let stop = Arc::new(AtomicBool::new(false));
    let samples = resources.sample_storage()?;
    let sampler = spawn_nvidia_sampler(
        Arc::clone(&stop),
        Arc::clone(&samples),
        resources.reserve_sampler()?,
    );
    let barrier = Arc::new(Barrier::new(workers));
    let started = Instant::now();
    let mut handles = Vec::with_capacity(workers);
    for worker in 0..workers {
        let input = Arc::clone(&input);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            let mut broker = AcceleratorBroker::new(CubeClWgpuBackend::default());
            let mut receipts = Vec::with_capacity(iterations);
            barrier.wait();
            for iteration in 0..iterations {
                let epoch = worker
                    .checked_mul(iterations)
                    .and_then(|base| base.checked_add(iteration + 1))
                    .ok_or_else(|| "gpu_stress_epoch_overflow".to_string())?;
                let receipt = broker
                    .execute_epoch(
                        EpochId(epoch as u64),
                        &binding("cubecl-wgpu", "lc631-bounded-summary.v1"),
                        &input,
                    )
                    .map_err(|error| format!("worker_{worker}_epoch_{epoch}:{error:?}"))?;
                receipts.push(receipt);
            }
            Ok::<_, String>(receipts)
        }));
    }
    let mut receipts = Vec::with_capacity(workers * iterations);
    let mut failed_epochs = 0_usize;
    for handle in handles {
        match handle.join() {
            Ok(Ok(mut worker_receipts)) => receipts.append(&mut worker_receipts),
            Ok(Err(_)) | Err(_) => failed_epochs = failed_epochs.saturating_add(iterations),
        }
    }
    let wall_elapsed_ms = started.elapsed().as_millis();
    stop.store(true, Ordering::Release);
    sampler.join().map_err(|_| "nvidia_sampler_panicked")??;
    let utilization = summarize_nvidia_samples(&samples);
    let host_inflight_overlap_observed = receipts.iter().any(|receipt| {
        receipt.overlap_observed || receipt.telemetry.host_inflight_overlap_observed
    });
    let full_readback_observed = receipts.iter().any(|receipt| receipt.full_readback);
    let maximum_readback_values = receipts
        .iter()
        .map(|receipt| receipt.readback_values)
        .max()
        .unwrap_or_default();
    let all_seven_lanes_observed = !receipts.is_empty()
        && receipts
            .iter()
            .all(|receipt| receipt.logical_lane_coverage == receipt.required_logical_lane_count);
    let device_loss_callback_registered = !receipts.is_empty()
        && receipts
            .iter()
            .all(|receipt| receipt.telemetry.device_loss_callback_registered);
    Ok(GpuStressReport {
        workers,
        iterations_per_worker: iterations,
        completed_epochs: receipts.len(),
        failed_epochs,
        wall_elapsed_ms,
        host_inflight_overlap_observed,
        full_readback_observed,
        maximum_readback_values,
        all_seven_lanes_observed,
        all_required_lanes_observed: all_seven_lanes_observed,
        device_loss_callback_registered,
        utilization,
        claim_boundary: "host in-flight overlap and nvidia-smi SM activity are measured observations; neither is a shader occupancy counter or a universal performance guarantee",
    })
}

#[cfg(feature = "gpu-cubecl-wgpu")]
fn spawn_nvidia_sampler(
    stop: Arc<AtomicBool>,
    samples: Arc<Mutex<Vec<[u64; 4]>>>,
    mut reservation: lc631_runtime::resource::Reservation,
) -> thread::JoinHandle<Result<(), String>> {
    thread::spawn(move || {
        while !stop.load(Ordering::Acquire) {
            let arguments = [
                std::ffi::OsString::from(
                    "--query-gpu=utilization.gpu,utilization.memory,memory.used,memory.total",
                ),
                std::ffi::OsString::from("--format=csv,noheader,nounits"),
            ];
            let cwd = std::env::current_dir().map_err(|error| format!("sampler_cwd:{error}"))?;
            let environment = std::env::vars_os()
                .filter(|(key, _)| {
                    matches!(
                        key.to_str(),
                        Some("PATH" | "PATHEXT" | "SystemRoot" | "WINDIR")
                    )
                })
                .collect::<Vec<_>>();
            let view = reservation.lease();
            let observed = lc631_process_supervisor::run_bounded_with_resources(
                lc631_process_supervisor::RunRequest {
                    executable: std::ffi::OsStr::new("nvidia-smi"),
                    arguments: &arguments,
                    current_dir: &cwd,
                    environment: &environment,
                    stdin: None,
                    timeout: Duration::from_secs(2),
                    stdout_limit: 65_536,
                    stderr_limit: 16_384,
                },
                &view,
            )
            .map_err(|error| format!("sampler_resource:{error:?}"))?;
            if !observed.cleanup_complete() {
                view.record_cleanup_unknown();
                return Err("sampler_cleanup_unverified".into());
            }
            if observed.cancelled() {
                return Err("sampler_cancelled".into());
            }
            let output = observed.outcome();
            if output.exit_code == Some(0) && !output.stdout_truncated && !output.stderr_truncated {
                if let Some(line) = String::from_utf8_lossy(&output.stdout).lines().next() {
                    let parsed = line
                        .split(',')
                        .map(|value| value.trim().parse::<u64>())
                        .collect::<Result<Vec<_>, _>>();
                    if let Ok(values) = parsed {
                        if values.len() == 4 {
                            if let Ok(mut guard) = samples.lock() {
                                if guard.len() < MAX_NVIDIA_SAMPLES {
                                    guard.push([values[0], values[1], values[2], values[3]]);
                                }
                            }
                        }
                    }
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
        reservation
            .release_confirmed()
            .map_err(|error| format!("sampler_release:{error:?}"))
    })
}

#[cfg(feature = "gpu-cubecl-wgpu")]
use io::MAX_NVIDIA_SAMPLES;

#[cfg(feature = "gpu-cubecl-wgpu")]
fn summarize_nvidia_samples(samples: &Arc<Mutex<Vec<[u64; 4]>>>) -> NvidiaUtilizationReceipt {
    let Ok(samples) = samples.lock() else {
        return NvidiaUtilizationReceipt {
            claim_boundary: "nvidia-smi sampler lock failed; no utilization inference",
            ..NvidiaUtilizationReceipt::default()
        };
    };
    let max_gpu = samples.iter().map(|sample| sample[0]).max();
    let max_memory = samples.iter().map(|sample| sample[1]).max();
    let peak_used = samples.iter().map(|sample| sample[2]).max();
    let total = samples.last().map(|sample| sample[3]);
    NvidiaUtilizationReceipt {
        nvidia_smi_available: !samples.is_empty(),
        sample_count: samples.len(),
        max_gpu_utilization_percent: max_gpu.and_then(|value| u32::try_from(value).ok()),
        max_memory_utilization_percent: max_memory.and_then(|value| u32::try_from(value).ok()),
        peak_device_memory_used_mib: peak_used,
        device_memory_total_mib: total,
        hardware_saturation_observed: max_gpu.is_some_and(|value| value >= 90),
        shader_occupancy: MeasurementState::Unavailable,
        claim_boundary: "nvidia-smi utilization.gpu is sampled SM activity and memory.used is device-wide, not achieved occupancy, per-kernel lane residency, or process-attributed VRAM",
    }
}

#[cfg(not(feature = "gpu-cubecl-wgpu"))]
fn build_gpu_stress(
    _iterations: usize,
    _workers: usize,
    _resources: &io::CommandIo,
) -> Result<serde_json::Value, String> {
    Err("gpu_feature_not_compiled".into())
}

#[derive(Serialize)]
struct MediaDoctorReport {
    source_gate: MediaAnalysisGate,
    backend_count: usize,
    method_candidate_count: usize,
    executed_backend_count: usize,
    youtube_download_enabled: bool,
    identity_inference_enabled: bool,
    emotion_as_internal_state_enabled: bool,
}

fn configured_path(name: &str) -> Option<PathBuf> {
    env::var_os(name).map(PathBuf::from)
}

fn build_media_doctor() -> MediaDoctorReport {
    let authority = authorize_media_source(&MediaSourceRequest {
        kind: MediaSourceKind::YoutubeMetadataOnly,
        locator: "youtube-metadata".into(),
        user_asserts_rights: false,
        media_bytes_requested: false,
        network_requested: false,
    });
    let mut registry = BackendRegistry::default();
    for (family, env_name, backend) in [
        (BackendFamily::Decode, "LC631_FFMPEG", "ffmpeg"),
        (
            BackendFamily::ShotBoundary,
            "LC631_SHOT_MODEL",
            "shot-boundary",
        ),
        (
            BackendFamily::ObjectDetection,
            "LC631_OBJECT_MODEL",
            "open-vocabulary-object",
        ),
        (
            BackendFamily::SegmentationTracking,
            "LC631_SEGMENT_MODEL",
            "video-segmentation",
        ),
        (BackendFamily::Pose, "LC631_POSE_MODEL", "pose-landmarks"),
        (
            BackendFamily::FaceActionUnit,
            "LC631_FACE_AU_MODEL",
            "face-action-units",
        ),
        (
            BackendFamily::ActionRecognition,
            "LC631_ACTION_MODEL",
            "action-recognition",
        ),
        (BackendFamily::Ocr, "LC631_OCR_MODEL", "ocr"),
        (
            BackendFamily::DepthMotion,
            "LC631_DEPTH_MODEL",
            "relative-depth-motion",
        ),
        (BackendFamily::Asr, "LC631_ASR_MODEL", "asr"),
        (
            BackendFamily::Diarization,
            "LC631_DIARIZATION_MODEL",
            "diarization",
        ),
        (
            BackendFamily::AudioEvent,
            "LC631_AUDIO_EVENT_MODEL",
            "audio-events",
        ),
        (
            BackendFamily::CrossModalAlignment,
            "LC631_ALIGNMENT_MODEL",
            "cross-modal-alignment",
        ),
    ] {
        let path = configured_path(env_name);
        let hash = env::var(format!("{env_name}_SHA256")).ok();
        let license = env::var(format!("{env_name}_LICENSE_RECEIPT")).ok();
        registry
            .register(BackendDescriptor::configured(
                family, backend, path, hash, license,
            ))
            .expect("unique backend family table");
    }
    let backend_count = registry.entries().count();
    let executed_backend_count = registry
        .entries()
        .filter(|entry| {
            matches!(
                entry.state,
                lc631_media::BackendState::ExecutedCandidate
                    | lc631_media::BackendState::ValidatedObservation
            )
        })
        .count();
    MediaDoctorReport {
        source_gate: lc631_media::build_media_gate(&authority, &registry),
        backend_count,
        method_candidate_count: default_method_catalog().len(),
        executed_backend_count,
        youtube_download_enabled: false,
        identity_inference_enabled: false,
        emotion_as_internal_state_enabled: false,
    }
}

#[derive(Serialize)]
struct HostBindReport {
    output: HostOutputReceipt,
    replay: DurableReplayReceipt,
    exact_candidate_output_binding: bool,
    automatic_host_callback_observed: bool,
    claim_boundary: &'static str,
}

struct HostBindInput {
    seed_file: PathBuf,
    output_file: PathBuf,
    callback_file: PathBuf,
    host_receipt_file: PathBuf,
    ledger_root: PathBuf,
    ledger: PathBuf,
    artifact: ArtifactId,
    turn: lc631_core::TurnId,
}

#[derive(Deserialize)]
struct HostBindAttestations {
    seed: UntrustedReceipt,
    output: UntrustedReceipt,
}

fn build_host_bind(
    input: HostBindInput,
    resources: &io::CommandIo,
) -> Result<HostBindReport, String> {
    let seed_text = resources.read_path(&input.seed_file, MAX_PROMPT_STDIN_BYTES as u64)?;
    let output_text = resources.read_path(&input.output_file, MAX_PROMPT_STDIN_BYTES as u64)?;
    let callback_payload =
        resources.read_path(&input.callback_file, MAX_PROMPT_STDIN_BYTES as u64)?;
    let host_receipt = resources.read_path(&input.host_receipt_file, 65_536)?;
    let attestations: HostBindAttestations = serde_json::from_str(&host_receipt)
        .map_err(|error| format!("host_receipt_json:{error}"))?;
    let context = resources
        .host_context(&input.ledger_root.join("receipt-root"), true)
        .map_err(|error| format!("host_receipt_root:{error:?}"))?;
    let now_epoch = current_epoch_seconds()?;
    let source_span = SourceSpan::checked(&seed_text, 0, seed_text.len())
        .map_err(|error| format!("seed_span:{error:?}"))?;
    let seed = HostSeedEnvelope::verify_attested(
        input.artifact,
        input.turn,
        seed_text,
        source_span,
        attestations.seed,
        context.verifier(),
        &mut ReplayGuard::default(),
        now_epoch,
    )
    .map_err(|error| format!("host_seed:{error:?}"))?;
    let output = bind_host_output_attested(
        &seed,
        &HostOutputCandidate {
            artifact_id: input.artifact,
            turn_id: input.turn,
            candidate_digest: stable_sha256(&output_text),
            output_text,
        },
        &callback_payload,
        Some(attestations.output),
        context.verifier(),
        &mut ReplayGuard::default(),
        now_epoch,
    )
    .map_err(|error| format!("host_output:{error:?}"))?;
    let replay = resources.with_storage_view(8 * 1024 * 1024 + 1, 4_096, |view| {
        append_durable_replay_verified_with_resources(
            &input.ledger_root,
            &input.ledger,
            &output,
            context.verifier(),
            &mut ReplayGuard::default(),
            now_epoch,
            view,
        )
        .map_err(|error| format!("host_replay:{error:?}"))
    })?;
    Ok(HostBindReport {
        exact_candidate_output_binding: output.exact_binding,
        automatic_host_callback_observed: output.automatic_host_callback_observed,
        output,
        replay,
        claim_boundary: "exact local bytes are bound and durably replayed; Codex post-send callback remains unavailable unless the host supplies one",
    })
}

#[derive(Serialize)]
struct DoctorReport {
    baseline: BaselineGuardReport,
    tl: TlDoctorReport,
    tldg: TldgPipelineReport,
    program_analysis: ProgramAnalysisReport,
    world: WorldDoctorReport,
    accelerator: AcceleratorDoctorReport,
    media: MediaDoctorReport,
    host_binding: HostBindingState,
    promotion_gate: PromotionGate,
    residual_risks: ResidualRiskLedger,
}

fn build_doctor(repo: &Path, resources: &io::CommandIo) -> Result<DoctorReport, String> {
    let baseline = resources.baseline_guard(repo)?;
    let tl = build_tl_doctor("Do not delete files.")?;
    let tldg = build_tldg_pipeline(
        "Explain and parse fn main() { return; }",
        None,
        &CalibrationPartition::reference(),
    )
    .map_err(|error| format!("tldg_doctor:{error:?}"))?;
    let world = build_world_doctor("2 + 2 = 4 under an unspecified algebra")?;
    let repo_text = repo.to_string_lossy();
    let program_analysis = analyze_program(
        AnalysisRequest {
            prompt: "audit Rust Python assembly program-analysis wiring",
            repo: Some(repo_text.as_ref()),
            validation: None,
            release: None,
            source_name: None,
            source: None,
        },
        &mut resources.analysis_runner(),
    )
    .map_err(|error| format!("doctor_resource:{error:?}"))?;
    let accelerator = build_accelerator_doctor(false)?;
    let media = build_media_doctor();
    let host =
        HostSeedEnvelope::self_attested(ArtifactId(631), lc631_core::TurnId(1), "doctor".into());
    let promotion_gate = PromotionGate::alpha_default();
    let mut residual_risks = ResidualRiskLedger::default();
    residual_risks.record(
        "host_output_unbound",
        "no verified host callback receipt was supplied",
    );
    residual_risks.record(
        "rpa_release_incomplete",
        "RPA-00..39 wiring is present but all compiler, validation, host, package, and CI receipts are not complete",
    );
    residual_risks.record(
        "media_backends_unexecuted",
        "registry readiness is not media inference",
    );
    residual_risks.record(
        "gpu_saturation_unmeasured",
        "logical coverage is not hardware saturation",
    );
    residual_risks.record(
        "paired_quality_pending",
        "v6.3.0/v6.3.1 semantic comparison is not yet observed",
    );
    residual_risks.record(
        "tldg_external_backends_unobserved",
        "unified adapter registry is connected but external grammar and semantic backends remain unavailable",
    );
    residual_risks.record(
        "tldg_geometry_gpu_unobserved",
        "mixed-curvature scalar reference is connected; real GPU geometry parity is unavailable",
    );
    Ok(DoctorReport {
        baseline,
        tl,
        tldg,
        program_analysis,
        world,
        accelerator,
        media,
        host_binding: host.binding_state,
        promotion_gate,
        residual_risks,
    })
}
