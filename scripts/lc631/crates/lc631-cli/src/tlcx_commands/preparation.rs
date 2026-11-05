//! 実prepareだけを呼ぶproduction入口。scope labelは未設定metadataで権限ではない。
//! Production preparation only; internal unconfigured scope labels are not authority.
#![forbid(unsafe_code)]

use super::{check_source, encode_report_summary, read_source, ParsedCommand, MAX_SOURCE_BYTES};
use crate::{effect_error::CliFailure, io::CommandIo};
use lc631_analysis::logical_lowering::{LogicProfile, LogicalLoweringBudget};
use lc631_core::{
    context::{ContextLimits, ContextScope, DecisionStatus, MissingStage, SourceRevision},
    AuthorityRevision,
};
use lc631_runtime::solve_preparation::{
    prepare, PreparationBudget, PreparationError, PreparationRequest, PreparationState,
    ProvidedPreparation, MAX_PREPARATION_SOURCE_BYTES, PREPARATION_RAM_FLOOR,
};
use lc631_runtime::RuntimeReport;
use std::io::Write;

/// The existing CommandIo owns ingress and actual-source reservations. No new
/// ResourceBroker, legacy solve, process backend, retry or fallback is created.
pub(super) fn run(resources: &CommandIo, command: ParsedCommand<'_>) -> Result<(), CliFailure> {
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
            // This is a distinct preparation profile, without truncation or
            // fallback. Legacy DGCL/embedding admission remains 256KiB.
            if source.len() > MAX_PREPARATION_SOURCE_BYTES {
                return Err(format!("canonical_preparation:{:?}", PreparationError::SourceLimit));
            }
            resources.with_preparation_view(source, |view| {
                let budget = PreparationBudget::checked(PREPARATION_RAM_FLOOR)
                    .map_err(|e| format!("canonical_preparation:{e:?}"))?;
                // EXACTLY ONE actual prepare invocation. It owns actual source
                // parsing, context lowering and logical lowering internally.
                let prepared = prepare(
                    PreparationRequest {
                        source,
                        source_revision: SourceRevision::of(source),
                        expected_lowered_context: None,
                        provided: metadata(command),
                    },
                    budget,
                    view,
                )
                .map_err(|e| format!("canonical_preparation:{e:?}"))?;
                let current = prepared
                    .project_current(view)
                    .map_err(|e| format!("canonical_preparation_projection:{e:?}"))?;
                let report = RuntimeReport {
                    context: current.context().original().state().revision(),
                    task: current.logical().task_kind(),
                    status: match current.state() {
                        PreparationState::Clarify => DecisionStatus::Clarify,
                        PreparationState::NextRequiredStageSolverPortfolio => {
                            DecisionStatus::NotConnected(MissingStage::SolverPortfolio)
                        }
                    },
                    source_bytes: current.context().source_store().text().len(),
                };
                // Current owner/source/context checks immediately precede the
                // SAME v1 encoder. This is not an atomic cancellation/send fence.
                prepared
                    .project_current(view)
                    .map_err(|e| format!("canonical_preparation_projection:{e:?}"))?;
                let bytes = encode_report_summary(Some(report))
                    .map_err(|e| format!("canonical_preparation_summary:{e:?}"))?;
                std::io::stdout().lock().write_all(&bytes).map_err(|_| {
                    "canonical_preparation:WriteFailedAfterRuntime; delivery_unobserved_no_retry".to_owned()
                })?;
                Ok(())
            })
        })
        .map_err(CliFailure::from)
}

/// These explicit domain-separated internal labels represent missing user
/// comparison/goal/probe registries. Hashing labels does not validate a scope,
/// create a GoalProfile, infer condition truth or establish current authority.
/// AuthorityRevision(0) is UNCONFIGURED metadata, not an execution receipt.
fn metadata(command: ParsedCommand<'_>) -> ProvidedPreparation {
    ProvidedPreparation {
        scope: ContextScope::declared(
            SourceRevision::of("epistesys.cli.prepare.v1\0canonical/no-comparison").digest(),
            SourceRevision::of("epistesys.cli.prepare.v1\0canonical/no-goal-profile").digest(),
            SourceRevision::of("epistesys.cli.prepare.v1\0canonical/no-probes").digest(),
            SourceRevision::of(
                "epistesys.cli.prepare.v1\0declared/controlled-classical-conditions.v1",
            )
            .digest(),
        ),
        authority_revision: AuthorityRevision(0),
        task: command.task,
        logic_profile: LogicProfile::ClassicalPropositionalV1,
        context_limits: ContextLimits {
            max_contexts: 4,
            max_premises: 1024,
            max_edits: 8,
            max_statement_bytes: MAX_PREPARATION_SOURCE_BYTES,
        },
        logical_budget: LogicalLoweringBudget::default(),
    }
}
