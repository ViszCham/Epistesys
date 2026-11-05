//! 明示formal入力だけの単一services solve。準備・旧solveとの二重実行なし。
//! Explicit formal input, one actual services solve, exact current rendered text.
#![forbid(unsafe_code)]
use super::super::artifacts;
use super::super::{
    check_source, read_source, GoalPolicyArgument, ParsedCommand, MAX_SOURCE_BYTES,
    MAX_SUMMARY_BYTES,
};
use super::file::{read_registry_file, ConfigFile};
use crate::{effect_error::CliFailure, io::CommandIo};
use lc631_core::{
    context::{
        ContextLimits, ContextPremise, ContextScope, OriginalContext, PremiseBasis, SourceRevision,
    },
    AuthorityRevision,
};
use lc631_runtime::{
    canonical_solve::{
        solve_with_services, CanonicalRequest, CanonicalServices, DecisionBudget,
        DeclaredEvaluation, ExecutionBudget, GoalPolicy, GpuPolicy, OneStepBudget, SExprLimits,
        SourceSpan, SourceStore,
    },
    explicit_formal::{lower_explicit_formal, FormalLowering},
    resource::{ResourceAmount, ResourceKind, ResourceRequest},
};
use std::{io::Write, path::Path, sync::Arc};

pub(crate) fn run(resources: &CommandIo, command: ParsedCommand<'_>) -> Result<(), CliFailure> {
    resources
        .with_view((MAX_SOURCE_BYTES + 1) as u64, |_| {
            let owned;
            let source = match command.prompt {
                Some(source) => {
                    check_source(source).map_err(|e| format!("canonical_source:{e:?}"))?;
                    source
                }
                None => {
                    owned = read_source(&mut std::io::stdin().lock())
                        .map_err(|e| format!("canonical_source:{e:?}"))?;
                    &owned
                }
            };
            if source.len() > lc631_runtime::explicit_formal::MAX_SOURCE_BYTES {
                return Err("canonical_formal:SourceLimit".to_owned());
            }
            let archive_profile = artifacts::profile(command)?;
            artifacts::preadmit_save(command, source, archive_profile)?;
            let bytes = resources.with_canonical_services_for_archive(
                source,
                command.store_root.is_some(),
                |broker, _same_run, view| {
                    // Covers source Arc, one structural Given premise and bounded CLI
                    // metadata BEFORE creating them. Parser/native owners claim separately.
                    let amount = ResourceAmount::checked(1024 * 1024)
                        .map_err(|e| format!("canonical_source_claim:{e:?}"))?;
                    let _source_claim = view
                        .claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
                        .map_err(|e| format!("canonical_source_claim:{e:?}"))?;
                    let store = SourceStore::new(
                        Arc::from(source),
                        lc631_runtime::explicit_formal::MAX_SOURCE_BYTES,
                    )
                    .map_err(|e| format!("canonical_source_store:{e:?}"))?;
                    let base_scope = scope(command.goal_policy);
                    let goal = match command.goal_policy {
                        GoalPolicyArgument::NoGoal => GoalPolicy::NoGoal,
                        GoalPolicyArgument::SatisfyingModel => GoalPolicy::SatisfyingModel {
                            revision: base_scope.goal(),
                        },
                        GoalPolicyArgument::DeclaredSourceObjective => {
                            GoalPolicy::DeclaredSourceObjective {
                                revision: base_scope.goal(),
                            }
                        }
                    };
                    // A DECLARED runtime profile, before context and the ONE parse.
                    // This creates no measured value or successful probe receipt.
                    let evaluation = match command.evaluation_profile {
                        None => None,
                        Some("finite-boolean-v1") => Some(
                            DeclaredEvaluation::finite_boolean(
                                SourceRevision::of(source),
                                base_scope,
                                goal,
                            )
                            .map_err(|e| format!("canonical_evaluation:{e:?}"))?,
                        ),
                        Some(_) => return Err("canonical_evaluation:UnsupportedProfile".to_owned()),
                    };
                    let scope = evaluation.map_or(base_scope, |declared| declared.scope());
                    let span = SourceSpan::checked(&store, 0, source.len())
                        .map_err(|e| format!("canonical_source_span:{e:?}"))?;
                    let premise = ContextPremise::from_source(
                        SourceRevision::of("epistesys.cli.formal.v1\0whole-given-source").digest(),
                        &store,
                        span,
                        PremiseBasis::Given,
                        false,
                    )
                    .map_err(|e| format!("canonical_premise:{e:?}"))?;
                    let mut premises = Vec::new();
                    premises
                        .try_reserve_exact(1)
                        .map_err(|_| "canonical_context:AllocationFailed")?;
                    premises.push(premise);
                    // AuthorityRevision(0) is UNCONFIGURED metadata, never a Grant.
                    let context = OriginalContext::checked(
                        SourceRevision::of(source),
                        scope,
                        AuthorityRevision(0),
                        premises,
                        ContextLimits {
                            max_contexts: 1,
                            max_premises: 1,
                            max_edits: 1,
                            max_statement_bytes: 16384,
                        },
                    )
                    .map_err(|e| format!("canonical_context:{e:?}"))?;
                    let lowering =
                        lower_explicit_formal(&store, context.state().revision(), scope, view);
                    let loaded = match command.solver_registry {
                        Some(path) => Some(
                            read_registry_file(
                                ConfigFile::checked(Path::new(path))
                                    .map_err(|e| format!("canonical_registry_path:{e:?}"))?,
                                view,
                            )
                            .map_err(|e| format!("canonical_registry:{e:?}"))?,
                        ),
                        None => None,
                    };
                    let request = match &lowering {
                        Ok(FormalLowering::Checked(problem)) => {
                            CanonicalRequest::formal(problem, context.state(), goal)
                                .and_then(|request| match evaluation {
                                    Some(declared) => request.with_evaluation(declared),
                                    None => Ok(request),
                                })
                                .and_then(|request| {
                                    match command.objective_step {
                                None => Ok(request),
                                Some("single-v1") => {
                                    request.with_objective_step(OneStepBudget::checked(3, 15000)?)
                                }
                                Some(_) => Err(
                                    lc631_runtime::canonical_solve::AdmissionError::InvalidBudget,
                                ),
                            }
                                })
                        }
                        Ok(FormalLowering::Unsupported(report)) => {
                            CanonicalRequest::unsupported(report, context.state())
                        }
                        Err(error) => CanonicalRequest::clarify(error, context.state()),
                    }
                    .map_err(|e| format!("canonical_request:{e:?}"))?;
                    let cwd = std::env::current_dir()
                        .map_err(|e| format!("canonical_working_directory:{:?}", e.kind()))?;
                    let execution = ExecutionBudget::checked(
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
                    .map_err(|e| format!("canonical_execution_budget:{e:?}"))?;
                    let parser = SExprLimits::checked(65536, 4096, 64)
                        .map_err(|e| format!("canonical_parser_budget:{e:?}"))?;
                    let budget = DecisionBudget::checked(MAX_SUMMARY_BYTES)
                        .map_err(|e| format!("canonical_output_budget:{e:?}"))?;
                    // Exactly ONE actual call; all parser/context/config owners remain live.
                    let decision = solve_with_services(
                        request,
                        CanonicalServices {
                            registry: loaded.as_ref().map(|r| r.registry()),
                            broker,
                            current: view,
                            execution,
                            parser,
                            working_directory: &cwd,
                            budget,
                            optional_gpu: GpuPolicy::NotRequested,
                        },
                    )
                    .map_err(|e| {
                        format!("canonical_solve:{e:?}; actual_attempt_may_have_executed_no_retry")
                    })?;
                    let amount = ResourceAmount::checked(MAX_SUMMARY_BYTES as u64)
                        .map_err(|e| format!("canonical_copy:{e:?}"))?;
                    let _copy_claim = view
                        .claim_resources(&[
                            ResourceRequest::new(ResourceKind::Ram, amount),
                            ResourceRequest::new(ResourceKind::Output, amount),
                        ])
                        .map_err(|e| format!("canonical_copy:{e:?}"))?;
                    let rendered = decision
                        .project_current(broker, view)
                        .map_err(|e| format!("canonical_projection:{e:?}"))?;
                    let original = lc631_runtime::canonical_original_pending::inspect_current(
                        &decision, broker, view,
                    )
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
                    artifacts::save_current(
                        command,
                        &decision,
                        source,
                        archive_profile,
                        broker,
                        view,
                    )?;
                    Ok(bytes)
                },
            )?;
            // Parent retains an existing-owner summary loan after callback teardown.
            // This is one exact write, not observed host delivery or a retry fence.
            std::io::stdout()
                .lock()
                .write_all(&bytes)
                .map_err(|_| "canonical_write:UnknownDeliveryNoRetry".to_owned())?;
            Ok(())
        })
        .map_err(CliFailure::from)
}

fn scope(goal: GoalPolicyArgument) -> ContextScope {
    let goal = match goal {
        GoalPolicyArgument::NoGoal => "epistesys.cli.formal.v1\0no-goal",
        GoalPolicyArgument::SatisfyingModel => "epistesys.cli.formal.v1\0declared-satisfying-model",
        GoalPolicyArgument::DeclaredSourceObjective => {
            "epistesys.cli.formal.v1\0declared-source-objective"
        }
    };
    // Declared labels bind this CLI profile; no validated comparison/probe set,
    // source truth or general GoalProfile is invented by hashing these strings.
    ContextScope::declared(
        SourceRevision::of("epistesys.cli.formal.v1\0no-comparison").digest(),
        SourceRevision::of(goal).digest(),
        SourceRevision::of("epistesys.cli.formal.v1\0no-probes").digest(),
        SourceRevision::of("epistesys.cli.formal.v1\0explicit-qf-lia").digest(),
    )
}
