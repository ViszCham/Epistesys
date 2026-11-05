//! Bounded source repair journal. Anchors bind the root and request, not success.
use lc631_analysis::*;
use lc631_core::stable_sha256;
use lc631_host::HostReceiptContext;
use std::path::Path;

pub(crate) mod resources;

pub struct RepairJournal<'a> {
    ledger: &'a Path,
    heads: &'a Path,
    host: &'a HostReceiptContext,
    source: String,
    workflow: String,
    authority: String,
    action: String,
    resource_binding: Option<(&'a crate::io::CommandIo, &'a str)>,
}

impl<'a> RepairJournal<'a> {
    pub(super) fn journal_reference(&self) -> String {
        format!("checkpoint-journal/{}", self.action)
    }

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
            resource_binding: None,
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

    pub(super) fn new_in_mode(
        ledger: &'a Path,
        heads: &'a Path,
        host: &'a HostReceiptContext,
        request: &DgclRepairRequest,
        authority: u64,
        binding: Option<(&'a crate::io::CommandIo, &'a str)>,
    ) -> Result<Self, String> {
        if let Some((io, source)) = binding {
            if stable_sha256(source) != request.source_revision {
                return Err("journal_resource_source_mismatch".into());
            }
            resources::read(Some((io, source)), |view| {
                view.expect("selected resource mode")
                    .check()
                    .map_err(|e| format!("journal_resource:{e:?}"))
            })?;
        }
        let mut journal = Self::new(ledger, heads, host, request, authority)?;
        journal.resource_binding = binding;
        Ok(journal)
    }

    pub fn inspect(&self) -> Result<ResumeObservation, String> {
        resources::read(self.resource_binding, |view| self.inspect_in_view(view))
    }

    fn inspect_in_view(
        &self,
        view: Option<&lc631_runtime::resource::ResourceReservationView<'_>>,
    ) -> Result<ResumeObservation, String> {
        let now = super::current_epoch_seconds()?;
        let observation = match view {
            None => inspect_resume(self.ledger, &self.source, &self.workflow, &self.authority),
            Some(view) => inspect_resume_with_resources(
                self.ledger,
                &self.source,
                &self.workflow,
                &self.authority,
                view,
            ),
        }
        .map_err(|error| format!("repair_resume:{error:?}"))?;
        if observation.latest_sequence.is_none() {
            return Ok(observation);
        }
        let anchor = match view {
            None => load_checkpoint_head(self.ledger, self.heads, self.host.verifier(), now),
            Some(view) => load_checkpoint_head_with_resources(
                self.ledger,
                self.heads,
                self.host.verifier(),
                now,
                view,
            ),
        }
        .map_err(|error| format!("repair_head_load:{error:?}"))?;
        match view {
            None => inspect_resume_with_anchor(
                self.ledger,
                &self.source,
                &self.workflow,
                &self.authority,
                Some(&anchor),
                Some(self.host.verifier()),
                now,
            ),
            Some(view) => inspect_resume_with_anchor_with_resources(
                self.ledger,
                CheckpointReadIdentity {
                    source_revision: &self.source,
                    plan_digest: &self.workflow,
                    authority_revision: &self.authority,
                },
                Some(&anchor),
                Some(self.host.verifier()),
                now,
                view,
            ),
        }
        .map_err(|error| format!("repair_resume:{error:?}"))
    }

    fn inspect_historical(&self) -> Result<(ResumeObservation, CheckpointHeadAnchor), String> {
        resources::read(self.resource_binding, |view| {
            self.inspect_historical_in_view(view)
        })
    }

    fn inspect_historical_in_view(
        &self,
        view: Option<&lc631_runtime::resource::ResourceReservationView<'_>>,
    ) -> Result<(ResumeObservation, CheckpointHeadAnchor), String> {
        let now = super::current_epoch_seconds()?;
        let anchor = match view {
            None => {
                load_checkpoint_head_historical(self.ledger, self.heads, self.host.verifier(), now)
            }
            Some(view) => load_checkpoint_head_historical_with_resources(
                self.ledger,
                self.heads,
                self.host.verifier(),
                now,
                view,
            ),
        }
        .map_err(|error| format!("repair_historical_head:{error:?}"))?;
        let observation = match view {
            None => inspect_resume_with_historical_anchor(
                self.ledger,
                &self.source,
                &self.workflow,
                &anchor.authority_revision,
                Some(&anchor),
                Some(self.host.verifier()),
                now,
            ),
            Some(view) => inspect_resume_with_historical_anchor_with_resources(
                self.ledger,
                CheckpointReadIdentity {
                    source_revision: &self.source,
                    plan_digest: &self.workflow,
                    authority_revision: &anchor.authority_revision,
                },
                Some(&anchor),
                Some(self.host.verifier()),
                now,
                view,
            ),
        }
        .map_err(|error| format!("repair_historical_resume:{error:?}"))?;
        Ok((observation, anchor))
    }

    /// Called only after current Test authority, independent implementation
    /// binding, actual Cargo observations, and exact candidate checks succeed.
    /// The renewed object is a history lease, never restored execution authority.
    fn renew_after_fresh_validation(
        &self,
        previous: &CheckpointHeadAnchor,
    ) -> Result<bool, String> {
        let now = super::current_epoch_seconds()?;
        if now <= previous.issued_at_epoch {
            return Ok(false);
        }
        let renewed = issue_checkpoint_head_anchor(
            self.host.issuer(),
            CheckpointHeadRequest {
                source_revision: &self.source,
                plan_digest: &self.workflow,
                authority_revision: &self.authority,
                sequence: previous.sequence,
                head_digest: previous.head_digest.as_deref(),
                now_epoch: now,
                lease_seconds: 3600,
            },
        )
        .map_err(|error| format!("repair_head_renew_issue:{error:?}"))?;
        match self.resource_binding {
            None => persist_checkpoint_head_renewal(
                self.ledger,
                self.heads,
                &renewed,
                self.host.verifier(),
                now,
            )
            .map_err(|error| format!("repair_head_renew_persist:{error:?}")),
            Some(binding) => {
                let index = renewed
                    .sequence
                    .map_or(0, |sequence| sequence.saturating_add(1));
                let target = self.heads.join(format!(
                    "head-{index:020}.renew-{:020}.json",
                    renewed.issued_at_epoch
                ));
                resources::write(
                    binding,
                    self.heads,
                    &target,
                    lc631_host::PersistentStoreKind::CheckpointHead,
                    |view| {
                        persist_checkpoint_head_renewal_with_resources(
                            self.ledger,
                            self.heads,
                            &renewed,
                            self.host.verifier(),
                            now,
                            view,
                        )
                        .map_err(|e| format!("repair_head_renew_persist:{e:?}"))
                    },
                )
            }
        }?;
        Ok(true)
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
        let body = CheckpointBody {
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
        };
        let digest = match self.resource_binding {
            None => {
                append_checkpoint(self.ledger, body).map_err(|e| format!("repair_checkpoint:{e:?}"))
            }
            Some(binding) => resources::write(
                binding,
                self.ledger,
                &self.ledger.join(format!("checkpoint-{sequence:020}.json")),
                lc631_host::PersistentStoreKind::Checkpoint,
                |view| {
                    append_checkpoint_with_resources(self.ledger, body, view)
                        .map_err(|e| format!("repair_checkpoint:{e:?}"))
                },
            ),
        }?;
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
        match self.resource_binding {
            None => {
                persist_checkpoint_head(self.ledger, self.heads, &anchor, self.host.verifier(), now)
                    .map_err(|e| format!("repair_head_persist:{e:?}"))
            }
            Some(binding) => {
                let index = sequence.saturating_add(1);
                resources::write(
                    binding,
                    self.heads,
                    &self.heads.join(format!("head-{index:020}.json")),
                    lc631_host::PersistentStoreKind::CheckpointHead,
                    |view| {
                        persist_checkpoint_head_with_resources(
                            self.ledger,
                            self.heads,
                            &anchor,
                            self.host.verifier(),
                            now,
                            view,
                        )
                        .map_err(|e| format!("repair_head_persist:{e:?}"))
                    },
                )
            }
        }
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
pub(super) fn resume_package_with_resources(
    root: &Path,
    source: &str,
    bundle: super::dgcl_repair::PackageRepairBundle,
    host: &HostReceiptContext,
    author: &HostReceiptContext,
    roots: (&Path, &Path),
    resources: &crate::io::CommandIo,
) -> Result<serde_json::Value, String> {
    resume_package_mode(root, source, bundle, (host, author), roots, Some(resources))
}

fn resume_package_mode(
    root: &Path,
    source: &str,
    bundle: super::dgcl_repair::PackageRepairBundle,
    identities: (&HostReceiptContext, &HostReceiptContext),
    roots: (&Path, &Path),
    resources: Option<&crate::io::CommandIo>,
) -> Result<serde_json::Value, String> {
    let (host, author) = identities;
    let (ledger, heads) = roots;
    let pipeline = super::dgcl_package::PackageIo(resources).pipeline(source)?;
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
    let mut journal = RepairJournal::new_in_mode(
        ledger,
        heads,
        host,
        &bundle.request,
        bundle.validation.authority_revision,
        resources.map(|io| (io, source)),
    )?;
    let (observation, historical_head) = journal.inspect_historical()?;
    if observation.integrity_state != CheckpointIntegrityState::HistoricalHeadMatched {
        return Err("repair_resume_authenticated_history_required".into());
    }
    // Keep historical authority identity attached to the old action. The new
    // bundle's authority revision is independently checked by fresh finalization.
    journal.authority = historical_head.authority_revision.clone();
    // The current target is checked through the same safe snapshot producer.
    // finalize_with_pipeline captures and rechecks every current file, issuer,
    // independent intent and command permit; no prior completion is reused.
    let report = match resources {
        None => super::dgcl_package::finalize_with_pipeline(
            root,
            source,
            bundle.validation,
            host,
            author,
            &pipeline,
            std::time::Duration::from_secs(120),
        ),
        Some(resources) => super::dgcl_package::finalize_with_pipeline_with_resources(
            root,
            source,
            bundle.validation,
            host,
            author,
            &pipeline,
            super::dgcl_package::PackageExecution {
                budget: std::time::Duration::from_secs(120),
                resources,
            },
        ),
    }?;
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
    let head_renewed = journal.renew_after_fresh_validation(&historical_head)?;
    if observation.latest_event_kind == Some(CheckpointEventKind::ActionStarted) {
        journal.finish(&report, true)?;
    }
    Ok(
        serde_json::json!({"schema_version": "epistesys-dgcl-package-resume.v1",
        "candidate": report["candidate"], "claims": report["claims"], "processes": report["processes"],
        "journal": journal.inspect()?, "repair_reapplied": false, "completion_restored_from_checkpoint": false,
        "history_authenticated": true, "historical_head_renewed_after_fresh_validation": head_renewed,
        "authority_created": false, "host_send_authorized": false, "output_commit_allowed": false,
        "host_observation": "pending", "research_evaluation": "pending_no_corpus"}),
    )
}
