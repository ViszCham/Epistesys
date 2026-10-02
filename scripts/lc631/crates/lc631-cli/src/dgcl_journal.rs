//! Bounded source repair journal. Anchors bind the root and request, not success.
use lc631_analysis::*;
use lc631_core::stable_sha256;
use lc631_host::HostReceiptContext;
use std::path::Path;

pub struct RepairJournal<'a> {
    ledger: &'a Path,
    heads: &'a Path,
    host: &'a HostReceiptContext,
    source: String,
    workflow: String,
    authority: String,
    action: String,
}

impl<'a> RepairJournal<'a> {
    pub fn new(
        ledger: &'a Path,
        heads: &'a Path,
        host: &'a HostReceiptContext,
        request: &DgclRepairRequest,
        authority: u64,
    ) -> Result<Self, String> {
        let root = request
            .repository_root_digest
            .as_deref()
            .ok_or("journal_root_binding_missing")?;
        let action = dgcl_repair_request_digest(request);
        let workflow = stable_sha256(&format!("dgcl/repair-journal.v1\0{root}\0{action}"));
        let journal = Self {
            ledger,
            heads,
            host,
            source: request.source_revision.clone(),
            workflow,
            authority: authority.to_string(),
            action,
        };
        // Validate disjoint real roots before any source write. Loading a genesis
        // anchor is unnecessary for a fresh ledger; an existing ledger is never
        // accepted without its separately persisted signature.
        let left = ledger
            .canonicalize()
            .map_err(|_| "journal_ledger_root_missing")?;
        let right = heads
            .canonicalize()
            .map_err(|_| "journal_head_root_missing")?;
        if left.starts_with(&right) || right.starts_with(&left) {
            return Err("journal_roots_must_be_disjoint".into());
        }
        Ok(journal)
    }

    pub fn inspect(&self) -> Result<ResumeObservation, String> {
        let now = super::current_epoch_seconds()?;
        let observation =
            inspect_resume(self.ledger, &self.source, &self.workflow, &self.authority)
                .map_err(|error| format!("repair_resume:{error:?}"))?;
        if observation.latest_sequence.is_none() {
            return Ok(observation);
        }
        let anchor = load_checkpoint_head(self.ledger, self.heads, self.host.verifier(), now)
            .map_err(|error| format!("repair_head_load:{error:?}"))?;
        inspect_resume_with_anchor(
            self.ledger,
            &self.source,
            &self.workflow,
            &self.authority,
            Some(&anchor),
            Some(self.host.verifier()),
            now,
        )
        .map_err(|error| format!("repair_resume:{error:?}"))
    }

    fn append(
        &self,
        kind: CheckpointEventKind,
        result: Option<String>,
        success: bool,
    ) -> Result<(), String> {
        let before = self.inspect()?;
        if before.latest_sequence.is_some()
            && before.integrity_state != CheckpointIntegrityState::TrustedHeadMatched
        {
            return Err("repair_journal_unanchored_tail".into());
        }
        let sequence = before.latest_sequence.map_or(Ok(0), |value| {
            value.checked_add(1).ok_or("repair_journal_overflow")
        })?;
        let now = super::current_epoch_seconds()?;
        let digest = append_checkpoint(
            self.ledger,
            CheckpointBody {
                schema_version: DGCL_CHECKPOINT_V2_SCHEMA.into(),
                sequence,
                source_revision: self.source.clone(),
                plan_digest: self.workflow.clone(),
                authority_revision: self.authority.clone(),
                action_key: self.action.clone(),
                event_kind: kind,
                delivery: if result.is_none() {
                    DeliveryState::UnknownDelivery
                } else if success {
                    DeliveryState::ConfirmedSuccess
                } else {
                    DeliveryState::ConfirmedFailed
                },
                result_digest: result,
                parent_digest: before.latest_digest,
            },
        )
        .map_err(|error| format!("repair_checkpoint:{error:?}"))?;
        let anchor = issue_checkpoint_head_anchor(
            self.host.issuer(),
            CheckpointHeadRequest {
                source_revision: &self.source,
                plan_digest: &self.workflow,
                authority_revision: &self.authority,
                sequence: Some(sequence),
                head_digest: Some(&digest),
                now_epoch: now,
                lease_seconds: 3600,
            },
        )
        .map_err(|error| format!("repair_head_issue:{error:?}"))?;
        persist_checkpoint_head(self.ledger, self.heads, &anchor, self.host.verifier(), now)
            .map_err(|error| format!("repair_head_persist:{error:?}"))
    }

    pub fn start(&self) -> Result<(), String> {
        if self.inspect()?.latest_sequence.is_some() {
            return Err("repair_journal_exists_use_resume".into());
        }
        self.append(CheckpointEventKind::ActionStarted, None, false)
    }

    pub fn finish(&self, report: &serde_json::Value, success: bool) -> Result<(), String> {
        self.append(
            CheckpointEventKind::ActionObserved,
            Some(stable_sha256(
                &serde_json::to_string(report).map_err(|_| "repair_journal_encode")?,
            )),
            success,
        )
    }
}

/// Resume never repeats a write. Even a signed completed journal is only a
/// historical observation: current external Test permits and fresh Cargo runs
/// are required before returning a new completion candidate.
#[allow(clippy::too_many_arguments)]
pub fn resume_package(
    root: &Path,
    source: &str,
    bundle: super::dgcl_repair::PackageRepairBundle,
    host: &HostReceiptContext,
    author: &HostReceiptContext,
    ledger: &Path,
    heads: &Path,
) -> Result<serde_json::Value, String> {
    let pipeline = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .map_err(|error| format!("repair_resume_pipeline:{error:?}"))?;
    if bundle.schema_version != "epistesys-dgcl-package-repair-bundle.v1"
        || bundle.request.source_revision != pipeline.source_revision
        || bundle.request.plan_digest != pipeline.completion_plan.plan_digest
        || bundle.request.repository_root_digest.as_deref()
            != Some(
                dgcl_repository_root_digest(root)
                    .map_err(|_| "repair_resume_root")?
                    .as_str(),
            )
    {
        return Err("repair_resume_identity".into());
    }
    let journal = RepairJournal::new(
        ledger,
        heads,
        host,
        &bundle.request,
        bundle.validation.authority_revision,
    )?;
    let observation = journal.inspect()?;
    if observation.integrity_state != CheckpointIntegrityState::TrustedHeadMatched {
        return Err("repair_resume_trusted_head_required".into());
    }
    // The current target is checked through the same safe snapshot producer.
    // finalize_with_pipeline captures and rechecks every current file, issuer,
    // independent intent and command permit; no prior completion is reused.
    let report = super::dgcl_package::finalize_with_pipeline(
        root,
        source,
        bundle.validation,
        host,
        author,
        &pipeline,
        std::time::Duration::from_secs(120),
    )?;
    let snapshot: DgclExecutionSnapshot =
        serde_json::from_value(report["snapshot"].clone()).map_err(|_| "repair_resume_snapshot")?;
    if !snapshot.targets.iter().any(|target| {
        target.target_ref == bundle.request.target_ref
            && target.target_digest == bundle.request.candidate_digest
    }) {
        return Err("repair_resume_candidate_not_materialized".into());
    }
    if !report["candidate"]["implementation_complete_candidate"]
        .as_bool()
        .unwrap_or(false)
    {
        return Err("repair_resume_fresh_validation_hold".into());
    }
    if observation.latest_event_kind == Some(CheckpointEventKind::ActionStarted) {
        journal.finish(&report, true)?;
    }
    Ok(
        serde_json::json!({"schema_version": "epistesys-dgcl-package-resume.v1",
        "candidate": report["candidate"], "claims": report["claims"], "processes": report["processes"],
        "journal": journal.inspect()?, "repair_reapplied": false, "completion_restored_from_checkpoint": false,
        "authority_created": false, "host_send_authorized": false, "output_commit_allowed": false,
        "host_observation": "pending", "research_evaluation": "pending_no_corpus"}),
    )
}
