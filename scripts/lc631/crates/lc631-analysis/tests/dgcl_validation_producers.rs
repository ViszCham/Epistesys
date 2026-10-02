use lc631_analysis::{check_cargo_test_result, CodingEvidenceKind};

use lc631_analysis::*;
use lc631_core::{
    authority_event_payload_digest, stable_sha256, Action, AuthorityEvent, AuthorityRevision,
    AuthoritySourceKind, CallerOrigin, DeonticPolarity, ExecutionPermit, ExecutionPermitContext,
    PrincipalBinding, SourceSpan, VerifiedAuthorityEvent,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard,
    SubjectRevision,
};
use sha2::{Digest, Sha256};

struct RealFileValidator<'a> {
    root: &'a std::path::Path,
    source: &'a str,
    pipeline: &'a DgclPipelineReport,
    issuer: &'a ReceiptIssuer,
    verifier: &'a ReceiptVerifier,
    author: &'a ReceiptIssuer,
    author_verifier: &'a ReceiptVerifier,
    cargo: std::path::PathBuf,
}

impl DgclFileEvidenceValidator for RealFileValidator<'_> {
    fn observe(
        &mut self,
        snapshot: &DgclExecutionSnapshot,
        budget: std::time::Duration,
    ) -> Result<Vec<CodingEvidenceClaim>, DgclRepairDriverError> {
        let parent = stable_sha256("test host ingress only; not a real Codex host observation");
        let principal =
            PrincipalBinding::from_attested_seed(&parent, self.verifier.key_fingerprint()).unwrap();
        let mut claims = Vec::new();
        for task in &self.pipeline.completion_plan.tasks {
            if task.evidence_kind == CodingEvidenceKind::ImplementationBinding {
                let receipt = self
                    .author
                    .issue(
                        ReceiptClass::Closure,
                        SubjectRevision::checked(&self.pipeline.source_revision).unwrap(),
                        ReceiptScope::checked(format!(
                            "dgcl/implementation-binding/{}",
                            task.task_id
                        ))
                        .unwrap(),
                        dgcl_implementation_binding_payload(self.pipeline, &task.task_id, snapshot),
                        100,
                        Some(400),
                        None,
                    )
                    .unwrap();
                claims.push(
                    issue_dgcl_implementation_binding(
                        self.source,
                        self.pipeline,
                        &task.task_id,
                        snapshot,
                        DgclBindingIssuerContext {
                            author_receipt: receipt,
                            author_verifier: self.author_verifier,
                            issuer: self.issuer,
                            verifier: self.verifier,
                            now_epoch: 100,
                            replay: &mut ReplayGuard::default(),
                        },
                    )
                    .unwrap(),
                );
                continue;
            }
            let plan = DgclCargoValidationPlan {
                schema_version: "epistesys-dgcl-cargo-validation-plan.v1".into(),
                source_revision: self.pipeline.source_revision.clone(),
                completion_plan_digest: self.pipeline.completion_plan.plan_digest.clone(),
                requirement_id: task.requirement_id.clone(),
                task_id: task.task_id.clone(),
                kind: task.evidence_kind,
                cargo_path: self.cargo.to_string_lossy().into_owned(),
                cargo_digest: file_hash(&self.cargo),
                manifest_path: "Cargo.toml".into(),
                manifest_digest: file_hash(&self.root.join("Cargo.toml")),
                repository_root_digest: dgcl_repository_root_digest(self.root).unwrap(),
                timeout_ms: budget.as_millis().min(60000) as u64,
                tool_registration: Some(registration(self.issuer, &self.cargo)),
                acceptance_target: (task.evidence_kind == CodingEvidenceKind::AcceptanceTest)
                    .then(|| "acceptance".into()),
                acceptance_test: (task.evidence_kind == CodingEvidenceKind::AcceptanceTest)
                    .then(|| "required_checked_arithmetic_contract".into()),
            };
            let scope = dgcl_cargo_validation_scope(&plan);
            let authorization = permit(self.source, &scope, self.issuer, self.verifier);
            let context = ExecutionPermitContext {
                principal: &principal,
                action: Action::Test,
                scope: &scope,
                source_revision: &self.pipeline.source_revision,
                authority_revision: AuthorityRevision(1),
                now_epoch: 100,
                revocation_revision: 0,
                revoked_permit_digests: &[],
                caller_origin: CallerOrigin::HostVerifiedUser,
                trusted_host_fingerprint: self.verifier.key_fingerprint(),
            };
            let observation = observe_dgcl_cargo_validation(
                self.root,
                self.source,
                self.pipeline,
                &plan,
                DgclCargoExecutionContext {
                    lifecycle: snapshot,
                    permit: &authorization,
                    authority: &context,
                    tool_verifier: self.verifier,
                    max_duration: budget,
                },
            )
            .map_err(|_| DgclRepairDriverError::Failed)?;
            claims
                .push(issue_dgcl_cargo_evidence(observation, self.issuer, self.verifier).unwrap());
        }
        Ok(claims)
    }
}

#[test]
fn actual_file_repair_revalidates_new_build_inputs_and_retains_backups_for_two_faults() {
    let source = "Please edit the source and test the package.";
    let pipeline = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let cargo_output = std::process::Command::new("rustup")
        .args(["which", "cargo"])
        .output()
        .unwrap();
    let cargo = std::path::PathBuf::from(String::from_utf8(cargo_output.stdout).unwrap().trim());
    let issuer = ReceiptIssuer::from_key_bytes("producer-fixture", &[79; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("producer-fixture", &[79; 32]).unwrap();
    let author = ReceiptIssuer::from_key_bytes("binding-author-fixture", &[80; 32]).unwrap();
    let author_verifier =
        ReceiptVerifier::from_key_bytes("binding-author-fixture", &[80; 32]).unwrap();
    for fault in ["Some(a.wrapping_add(b))", "None"] {
        let root = fixture_package();
        let correct = std::fs::read_to_string(root.join("src/lib.rs")).unwrap();
        let broken = correct.replace("a.checked_add(b)", fault);
        std::fs::write(root.join("src/lib.rs"), &broken).unwrap();
        let parent = stable_sha256("test host ingress only; not a real Codex host observation");
        let principal =
            PrincipalBinding::from_attested_seed(&parent, verifier.key_fingerprint()).unwrap();
        let revision = stable_sha256(source);
        let root_digest = dgcl_repository_root_digest(&root).unwrap();
        let scope = format!("dgcl/repair/{root_digest}/src/lib.rs");
        let permission = permit_action(source, &scope, &issuer, &verifier, Action::Edit);
        let context = ExecutionPermitContext {
            principal: &principal,
            action: Action::Edit,
            scope: &scope,
            source_revision: &revision,
            authority_revision: AuthorityRevision(1),
            now_epoch: 100,
            revocation_revision: 0,
            revoked_permit_digests: &[],
            caller_origin: CallerOrigin::HostVerifiedUser,
            trusted_host_fingerprint: verifier.key_fingerprint(),
        };
        let snapshot = capture_dgcl_cargo_snapshot(&root, source, &pipeline, &context).unwrap();
        let initial_lease = issue_dgcl_evidence_lease(&snapshot, 100, 300).unwrap();
        let baseline = &pipeline.implementation_closure;
        let gaps = baseline
            .gaps
            .iter()
            .map(|gap| gap.gap_id.clone())
            .collect::<Vec<_>>();
        let task = baseline
            .gaps
            .iter()
            .find(|gap| gap.evidence_kind == Some(CodingEvidenceKind::RuntimeValidation))
            .unwrap();
        let request = DgclRepairRequest {
            repository_root_digest: Some(root_digest),
            attempt: 1,
            gap_id: task.gap_id.clone(),
            source_revision: revision.clone(),
            plan_digest: pipeline.completion_plan.plan_digest.clone(),
            target_ref: "src/lib.rs".into(),
            target_digest_before: stable_sha256(&broken),
            candidate_digest: stable_sha256(&correct),
            authorization_scope: scope.clone(),
            authorization_span: permission.source_span(),
        };
        let authorization = DgclRepairAuthorization {
            permit: &permission,
            authority_revision: AuthorityRevision(1),
            trusted_host_fingerprint: verifier.key_fingerprint(),
            now_epoch: 100,
            revocation_revision: 0,
            revoked_permit_digests: &[],
        };
        let mut validator = RealFileValidator {
            root: &root,
            source,
            pipeline: &pipeline,
            issuer: &issuer,
            verifier: &verifier,
            author: &author,
            author_verifier: &author_verifier,
            cargo: cargo.clone(),
        };
        let mut candidates = std::collections::BTreeMap::new();
        candidates.insert(stable_sha256(&correct), correct.clone());
        let mut driver = DgclFileRepairDriver::new(DgclFileRepairConfig {
            epoch_clock: None,
            root: &root,
            source,
            pipeline: &pipeline,
            initial_snapshot: &snapshot,
            authorization: &authorization,
            issuer: &issuer,
            verifier: &verifier,
            candidates,
            validator: &mut validator,
        })
        .unwrap();
        let report = run_dgcl_repair_loop(
            DgclRepairLoopInput {
                source,
                source_revision: &revision,
                plan_digest: &pipeline.completion_plan.plan_digest,
                program_ir: &pipeline.program_ir,
                translation: &pipeline.translation_projection,
                completion_plan: &pipeline.completion_plan,
                initial_snapshot: &snapshot,
                initial_lease: &initial_lease,
                initial_gap_ids: &gaps,
                requests: &[request],
                authorization: DgclRepairAuthorization {
                    permit: &permission,
                    authority_revision: AuthorityRevision(1),
                    trusted_host_fingerprint: verifier.key_fingerprint(),
                    now_epoch: 100,
                    revocation_revision: 0,
                    revoked_permit_digests: &[],
                },
                verifier: &verifier,
                replay: &mut ReplayGuard::default(),
                max_attempts: 4,
                max_wall_time: std::time::Duration::from_secs(120),
                max_delta_bytes: 1048576,
            },
            &mut driver,
        )
        .unwrap();
        assert_eq!(report.status, DgclRepairStatus::Completed, "{report:?}");
        assert!(report.remaining_gap_ids.is_empty());
        assert_ne!(report.initial_snapshot_digest, report.final_snapshot_digest);
        assert_eq!(
            std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
            correct
        );
        let backup = std::fs::read_dir(root.join(".dgcl-repair-backups"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(std::fs::read_to_string(backup).unwrap(), broken);
        assert!(!report.authority_created);
        assert!(!report.commit_push_merge_performed);
    }
}

fn file_hash(path: &std::path::Path) -> String {
    let digest = Sha256::digest(std::fs::read(path).unwrap());
    format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn registration(
    issuer: &ReceiptIssuer,
    cargo: &std::path::Path,
) -> lc631_receipt_kernel::UntrustedReceipt {
    let digest = file_hash(cargo);
    issuer
        .issue(
            ReceiptClass::ToolExecution,
            SubjectRevision::checked(&digest).unwrap(),
            ReceiptScope::checked(format!("dgcl/registered-tool/cargo/{digest}")).unwrap(),
            dgcl_cargo_registration_payload(&cargo.to_string_lossy(), &digest),
            100,
            Some(400),
            None,
        )
        .unwrap()
}

fn fixture_package() -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "dgcl real producer {} {unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../fixtures/dgcl/producer-package");
    for relative in [
        "Cargo.toml",
        "Cargo.lock",
        "src/lib.rs",
        "tests/acceptance.rs",
    ] {
        std::fs::copy(fixture.join(relative), root.join(relative)).unwrap();
    }
    root
}

fn permit(
    source: &str,
    scope: &str,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
) -> ExecutionPermit {
    permit_action(source, scope, issuer, verifier, Action::Test)
}

fn permit_action(
    source: &str,
    scope: &str,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    action: Action,
) -> ExecutionPermit {
    let parent = stable_sha256("test host ingress only; not a real Codex host observation");
    let event = AuthorityEvent {
        action,
        source: AuthoritySourceKind::VerifiedHostUserSpan,
        polarity: DeonticPolarity::Grant,
        span: SourceSpan {
            start: 0,
            end: source.len(),
        },
        scope: scope.into(),
    };
    let subject = SubjectRevision::checked(stable_sha256(source)).unwrap();
    let action_name = if action == Action::Edit {
        "edit"
    } else {
        "test"
    };
    let receipt_scope = ReceiptScope::checked(format!("authority/{action_name}/{scope}")).unwrap();
    let payload = authority_event_payload_digest(&event);
    let raw = issuer
        .issue(
            ReceiptClass::Authority,
            subject.clone(),
            receipt_scope.clone(),
            payload.clone(),
            100,
            Some(400),
            Some(parent.clone()),
        )
        .unwrap();
    let verified = verifier
        .verify(
            raw,
            &ReceiptPolicy::exact(
                ReceiptClass::Authority,
                subject,
                receipt_scope,
                payload,
                100,
            )
            .with_parent(&parent),
            &mut ReplayGuard::default(),
        )
        .unwrap();
    ExecutionPermit::from_verified_user_event(
        VerifiedAuthorityEvent::checked(event, verified).unwrap(),
        source,
        AuthorityRevision(1),
        100,
        400,
        0,
    )
    .unwrap()
}

#[test]
fn actual_separate_cargo_observations_close_only_the_declared_package_requirement() {
    let source = "Please test the package.";
    let root = fixture_package();
    let pipeline = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let cargo_output = std::process::Command::new("rustup")
        .args(["which", "cargo"])
        .output()
        .unwrap();
    assert!(cargo_output.status.success());
    let cargo = std::path::PathBuf::from(String::from_utf8(cargo_output.stdout).unwrap().trim());
    let issuer = ReceiptIssuer::from_key_bytes("producer-fixture", &[79; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("producer-fixture", &[79; 32]).unwrap();
    let author = ReceiptIssuer::from_key_bytes("binding-author-fixture", &[80; 32]).unwrap();
    let author_verifier =
        ReceiptVerifier::from_key_bytes("binding-author-fixture", &[80; 32]).unwrap();
    let parent = stable_sha256("test host ingress only; not a real Codex host observation");
    let principal =
        PrincipalBinding::from_attested_seed(&parent, verifier.key_fingerprint()).unwrap();
    let fingerprint = verifier.key_fingerprint();
    let revision = stable_sha256(source);
    let mut claims = Vec::new();
    let mut last_snapshot = None;
    for task in pipeline
        .completion_plan
        .tasks
        .iter()
        .filter(|task| task.evidence_kind != CodingEvidenceKind::ImplementationBinding)
    {
        let plan = DgclCargoValidationPlan {
            schema_version: "epistesys-dgcl-cargo-validation-plan.v1".into(),
            source_revision: revision.clone(),
            completion_plan_digest: pipeline.completion_plan.plan_digest.clone(),
            requirement_id: task.requirement_id.clone(),
            task_id: task.task_id.clone(),
            kind: task.evidence_kind,
            cargo_path: cargo.to_string_lossy().into_owned(),
            cargo_digest: file_hash(&cargo),
            manifest_path: "Cargo.toml".into(),
            manifest_digest: file_hash(&root.join("Cargo.toml")),
            repository_root_digest: dgcl_repository_root_digest(&root).unwrap(),
            acceptance_target: (task.evidence_kind == CodingEvidenceKind::AcceptanceTest)
                .then(|| "acceptance".into()),
            acceptance_test: (task.evidence_kind == CodingEvidenceKind::AcceptanceTest)
                .then(|| "required_checked_arithmetic_contract".into()),
            timeout_ms: 60000,
            tool_registration: Some(registration(&issuer, &cargo)),
        };
        let scope = dgcl_cargo_validation_scope(&plan);
        let mut unregistered = plan.clone();
        unregistered.tool_registration = None;
        assert_eq!(
            verify_dgcl_cargo_registration(&unregistered, &verifier, 100),
            Err(DgclValidationProducerError::UnregisteredTool)
        );
        let authorization = permit(source, &scope, &issuer, &verifier);
        let context = ExecutionPermitContext {
            principal: &principal,
            action: Action::Test,
            scope: &scope,
            source_revision: &revision,
            authority_revision: AuthorityRevision(1),
            now_epoch: 100,
            revocation_revision: 0,
            revoked_permit_digests: &[],
            caller_origin: CallerOrigin::HostVerifiedUser,
            trusted_host_fingerprint: fingerprint,
        };
        let snapshot = capture_dgcl_cargo_snapshot(&root, source, &pipeline, &context).unwrap();
        let invalid = ExecutionPermitContext {
            caller_origin: CallerOrigin::StandaloneCli,
            ..context
        };
        assert!(matches!(
            observe_dgcl_cargo_validation(
                &root,
                source,
                &pipeline,
                &plan,
                DgclCargoExecutionContext {
                    lifecycle: &snapshot,
                    permit: &authorization,
                    authority: &invalid,
                    tool_verifier: &verifier,
                    max_duration: std::time::Duration::from_secs(60)
                },
            ),
            Err(DgclValidationProducerError::Unauthorized)
        ));
        let observation = observe_dgcl_cargo_validation(
            &root,
            source,
            &pipeline,
            &plan,
            DgclCargoExecutionContext {
                lifecycle: &snapshot,
                permit: &authorization,
                authority: &context,
                tool_verifier: &verifier,
                max_duration: std::time::Duration::from_secs(60),
            },
        )
        .unwrap();
        assert_eq!(observation.process().exit_code, Some(0));
        claims.push(issue_dgcl_cargo_evidence(observation, &issuer, &verifier).unwrap());
        last_snapshot = Some(snapshot);
    }
    let snapshot = last_snapshot.unwrap();
    let task = pipeline
        .completion_plan
        .tasks
        .iter()
        .find(|task| task.evidence_kind == CodingEvidenceKind::ImplementationBinding)
        .unwrap();
    let intent = author
        .issue(
            ReceiptClass::Closure,
            SubjectRevision::checked(&revision).unwrap(),
            ReceiptScope::checked(format!("dgcl/implementation-binding/{}", task.task_id)).unwrap(),
            dgcl_implementation_binding_payload(&pipeline, &task.task_id, &snapshot),
            100,
            Some(400),
            None,
        )
        .unwrap();
    claims.push(
        issue_dgcl_implementation_binding(
            source,
            &pipeline,
            &task.task_id,
            &snapshot,
            DgclBindingIssuerContext {
                author_receipt: intent,
                author_verifier: &author_verifier,
                issuer: &issuer,
                verifier: &verifier,
                now_epoch: 100,
                replay: &mut ReplayGuard::default(),
            },
        )
        .unwrap(),
    );
    let candidate = build_verified_dgcl_completion_candidate(
        source,
        &pipeline,
        &claims,
        DgclImplementationClosureContext {
            current_lifecycle: Some(&snapshot),
            verifier: Some(&verifier),
            replay: &mut ReplayGuard::default(),
            now_epoch: 120,
        },
    )
    .unwrap();
    assert_eq!(
        candidate.implementation_closure_status,
        DgclImplementationStatus::ImplementationClosed
    );
    assert!(candidate.implementation_complete_candidate);
    assert!(candidate
        .requirement_realizations
        .iter()
        .all(|item| item.open_gap_ids.is_empty()));
    assert!(!candidate.output_commit_allowed);
    assert!(!candidate.host_send_authorized);
    let mut cut = claims.clone();
    cut.retain(|claim| claim.kind != CodingEvidenceKind::RuntimeValidation);
    let held = build_verified_dgcl_completion_candidate(
        source,
        &pipeline,
        &cut,
        DgclImplementationClosureContext {
            current_lifecycle: Some(&snapshot),
            verifier: Some(&verifier),
            replay: &mut ReplayGuard::default(),
            now_epoch: 120,
        },
    )
    .unwrap();
    assert!(!held.implementation_complete_candidate);
    assert!(!held.requirement_realizations[0].open_gap_ids.is_empty());
}

#[test]
fn exit_zero_and_zero_tests_do_not_produce_runtime_or_acceptance_evidence() {
    assert!(!check_cargo_test_result(
        CodingEvidenceKind::RuntimeValidation,
        b"test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
    ));
    assert!(!check_cargo_test_result(
        CodingEvidenceKind::AcceptanceTest,
        b"test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out"
    ));
    assert!(!check_cargo_test_result(
        CodingEvidenceKind::AcceptanceTest,
        b"test result: FAILED. 1 passed; 1 failed"
    ));
    assert!(check_cargo_test_result(
        CodingEvidenceKind::RuntimeValidation,
        b"test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
    ));
    assert!(check_cargo_test_result(
        CodingEvidenceKind::AcceptanceTest,
        b"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
    ));
    assert!(!check_cargo_test_result(
        CodingEvidenceKind::StaticValidation,
        b"test result: ok. 1 passed; 0 failed;"
    ));
}
