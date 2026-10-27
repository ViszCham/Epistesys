use lc631_analysis::{
    build_dgcl_pipeline, capture_dgcl_cargo_snapshot, dgcl_cargo_validation_scope,
    dgcl_implementation_binding_payload, CodingEvidenceKind, DgclBackendSet,
    DgclCargoValidationPlan,
};
use lc631_core::{
    stable_sha256, Action, ArtifactId, AuthorityRevision, CallerOrigin, ExecutionPermitContext,
    SourceSpan, TurnId,
};
use lc631_host::{host_seed_scoped_payload_digest, HostReceiptContext, HostSeedEnvelope};
use lc631_receipt_kernel::{ReceiptClass, ReceiptScope, ReplayGuard, SubjectRevision};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn hash(path: &Path) -> String {
    let bytes = Sha256::digest(std::fs::read(path).unwrap());
    format!(
        "sha256:{}",
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn connection_spec(
    root: &Path,
    host: &HostReceiptContext,
    now: u64,
) -> lc631_analysis::DgclConnectionSpec {
    let mut spec = lc631_analysis::DgclConnectionSpec {
        schema_version: "epistesys-dgcl-connection-spec.v1".into(),
        integration_target: "connection".into(),
        test_name: "production_api_connection".into(),
        entrypoint: "dgcl_producer_fixture::checked_add".into(),
        implementation_path: "src/lib.rs".into(),
        driver_path: "tests/connection.rs".into(),
        driver_digest: hash(&root.join("tests/connection.rs")),
        observer_id: lc631_analysis::DIRECT_API_OBSERVER.into(),
        oracle_id: "checked-add-example.v1".into(),
        oracle_revision: stable_sha256("::std::assert_eq!(observed, Some(10))"),
        coverage: lc631_analysis::DIRECT_API_COVERAGE.into(),
        registration: None,
    };
    spec.registration = Some(
        host.issuer()
            .issue(
                ReceiptClass::VerifierCapability,
                SubjectRevision::checked(&spec.driver_digest).unwrap(),
                ReceiptScope::checked(lc631_analysis::dgcl_connection_registration_scope(&spec))
                    .unwrap(),
                lc631_analysis::dgcl_connection_registration_payload(&spec),
                now,
                Some(now + 300),
                None,
            )
            .unwrap(),
    );
    spec
}

fn epoch_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn renew_test_receipt(
    issuer: &lc631_receipt_kernel::ReceiptIssuer,
    original: &lc631_receipt_kernel::UntrustedReceipt,
    now: u64,
    parent: Option<String>,
) -> lc631_receipt_kernel::UntrustedReceipt {
    let claims = original.claims();
    issuer
        .issue(
            claims.class(),
            claims.subject_revision().clone(),
            claims.scope().clone(),
            claims.payload_digest(),
            now,
            Some(now.checked_add(300).unwrap()),
            parent,
        )
        .unwrap()
}

// Test-host reauthorization, never a production expiry bypass. Each new CLI
// operation receives fresh finite receipts; changing a plan's registrations
// changes its scope, so the user-span and parent-linked Grant are reissued too.
fn fresh_test_bundle(
    original: &Value,
    source: &str,
    host: &HostReceiptContext,
    author: &HostReceiptContext,
    now: u64,
) -> Value {
    let mut bundle = original.clone();
    let validation = if bundle.get("validation").is_some() {
        &mut bundle["validation"]
    } else {
        &mut bundle
    };
    let mut plans: Vec<DgclCargoValidationPlan> =
        serde_json::from_value(validation["plans"].clone()).unwrap();
    let mut seeds = validation["seeds"].as_array().unwrap().clone();
    for (plan, seed) in plans.iter_mut().zip(&mut seeds) {
        plan.tool_registration = Some(renew_test_receipt(
            host.issuer(),
            plan.tool_registration.as_ref().unwrap(),
            now,
            None,
        ));
        if let Some(connection) = &mut plan.connection {
            connection.registration = Some(renew_test_receipt(
                host.issuer(),
                connection.registration.as_ref().unwrap(),
                now,
                None,
            ));
        }
        let scope = dgcl_cargo_validation_scope(plan);
        renew_test_seed(seed, source, &scope, Action::Test, host, now);
    }
    validation["plans"] = serde_json::to_value(plans).unwrap();
    validation["seeds"] = json!(seeds);
    let binding = serde_json::from_value(validation["binding_receipt"].clone()).unwrap();
    validation["binding_receipt"] =
        serde_json::to_value(renew_test_receipt(author.issuer(), &binding, now, None)).unwrap();
    for binding in validation["binding_receipts"].as_array_mut().unwrap() {
        let old = serde_json::from_value(binding.clone()).unwrap();
        *binding =
            serde_json::to_value(renew_test_receipt(author.issuer(), &old, now, None)).unwrap();
    }
    if bundle.get("edit_seed").is_some() {
        let scope = bundle["request"]["authorization_scope"]
            .as_str()
            .unwrap()
            .to_owned();
        renew_test_seed(
            &mut bundle["edit_seed"],
            source,
            &scope,
            Action::Edit,
            host,
            now,
        );
    }
    bundle
}

fn renew_test_seed(
    seed: &mut Value,
    source: &str,
    scope: &str,
    action: Action,
    host: &HostReceiptContext,
    now: u64,
) {
    let artifact = ArtifactId(seed["artifact_id"].as_u64().unwrap());
    let turn = TurnId(seed["turn_id"].as_u64().unwrap());
    let span = SourceSpan {
        start: seed["user_start"].as_u64().unwrap() as usize,
        end: seed["user_end"].as_u64().unwrap() as usize,
    };
    let receipt = host
        .issuer()
        .issue(
            ReceiptClass::HostSeed,
            SubjectRevision::checked(stable_sha256(source)).unwrap(),
            ReceiptScope::checked(format!(
                "host/user-span/{}/{}/{}",
                artifact.0,
                turn.0,
                stable_sha256(scope)
            ))
            .unwrap(),
            host_seed_scoped_payload_digest(artifact, turn, source, span, scope),
            now,
            Some(now + 300),
            None,
        )
        .unwrap();
    let event = lc631_core::AuthorityEvent {
        action,
        source: lc631_core::AuthoritySourceKind::VerifiedHostUserSpan,
        polarity: lc631_core::DeonticPolarity::Grant,
        span,
        scope: scope.into(),
    };
    let authority = host
        .issuer()
        .issue(
            ReceiptClass::Authority,
            SubjectRevision::checked(stable_sha256(source)).unwrap(),
            ReceiptScope::checked(format!(
                "authority/{}/{}",
                match action {
                    Action::Edit => "edit",
                    Action::Test => "test",
                    _ => panic!("unsupported test action"),
                },
                scope,
            ))
            .unwrap(),
            lc631_core::authority_event_payload_digest(&event),
            now,
            Some(now + 300),
            Some(receipt.receipt_digest()),
        )
        .unwrap();
    seed["receipt"] = serde_json::to_value(receipt).unwrap();
    seed["authority_receipt"] = serde_json::to_value(authority).unwrap();
}

#[test]
fn epr_11_test_host_renews_operation_receipts_without_extending_original_leases() {
    use lc631_host::HostExecutionPermitAdmission;
    use lc631_receipt_kernel::{ReceiptPolicy, UntrustedReceipt};
    let root = std::env::temp_dir().join(format!(
        "dgcl-current-operation-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    let host = HostReceiptContext::load_or_create(&root.join("host")).unwrap();
    let author = HostReceiptContext::load_or_create(&root.join("author")).unwrap();
    let source = "Please test the package.";
    let issued = 1_000;
    let next_operation = issued + 300;
    let tool = host
        .issuer()
        .issue(
            ReceiptClass::ToolExecution,
            SubjectRevision::checked(stable_sha256("cargo")).unwrap(),
            ReceiptScope::checked("test-only-cargo-registration").unwrap(),
            stable_sha256("registered-cargo"),
            issued,
            Some(issued + 300),
            None,
        )
        .unwrap();
    let plan = DgclCargoValidationPlan {
        schema_version: "epistesys-dgcl-cargo-validation-plan.v1".into(),
        source_revision: stable_sha256(source),
        completion_plan_digest: stable_sha256("plan"),
        requirement_id: "requirement".into(),
        task_id: "task".into(),
        kind: CodingEvidenceKind::RuntimeValidation,
        cargo_path: "test-only-cargo".into(),
        cargo_digest: stable_sha256("cargo"),
        manifest_path: "Cargo.toml".into(),
        manifest_digest: stable_sha256("manifest"),
        repository_root_digest: stable_sha256("root"),
        timeout_ms: 60_000,
        acceptance_target: None,
        acceptance_test: None,
        connection: None,
        tool_registration: Some(tool),
    };
    let old_scope = dgcl_cargo_validation_scope(&plan);
    let mut seed = json!({"artifact_id":900,"turn_id":1,"user_start":0,"user_end":source.len()});
    renew_test_seed(&mut seed, source, &old_scope, Action::Test, &host, issued);
    let binding = author
        .issuer()
        .issue(
            ReceiptClass::Closure,
            SubjectRevision::checked(stable_sha256(source)).unwrap(),
            ReceiptScope::checked("test-only-binding").unwrap(),
            stable_sha256("intent"),
            issued,
            Some(issued + 300),
            None,
        )
        .unwrap();
    let old =
        json!({"plans":[plan],"seeds":[seed],"binding_receipt":binding,"binding_receipts":[]});
    let original_bytes = serde_json::to_vec(&old).unwrap();
    let renewed = fresh_test_bundle(&old, source, &host, &author, next_operation);
    assert_eq!(serde_json::to_vec(&old).unwrap(), original_bytes);
    let new_plan: DgclCargoValidationPlan =
        serde_json::from_value(renewed["plans"][0].clone()).unwrap();
    let new_scope = dgcl_cargo_validation_scope(&new_plan);
    assert_ne!(
        old_scope, new_scope,
        "renewing registrations must bind a new command scope"
    );
    let previous: UntrustedReceipt =
        serde_json::from_value(old["seeds"][0]["receipt"].clone()).unwrap();
    let current: UntrustedReceipt =
        serde_json::from_value(renewed["seeds"][0]["receipt"].clone()).unwrap();
    assert_eq!(previous.claims().expires_at_epoch(), Some(next_operation));
    assert_eq!(
        current.claims().expires_at_epoch(),
        Some(next_operation + 300)
    );
    assert!(
        host.verifier()
            .verify(
                previous.clone(),
                &ReceiptPolicy::exact(
                    ReceiptClass::HostSeed,
                    previous.claims().subject_revision().clone(),
                    previous.claims().scope().clone(),
                    previous.claims().payload_digest(),
                    next_operation,
                ),
                &mut ReplayGuard::default()
            )
            .is_err(),
        "the old exact-expiry receipt must stay rejected"
    );
    let envelope = HostSeedEnvelope::verify_attested_scoped(
        ArtifactId(900),
        TurnId(1),
        source.into(),
        SourceSpan {
            start: 0,
            end: source.len(),
        },
        new_scope.clone(),
        current.clone(),
        host.verifier(),
        &mut ReplayGuard::default(),
        next_operation,
    )
    .unwrap();
    let authority: UntrustedReceipt =
        serde_json::from_value(renewed["seeds"][0]["authority_receipt"].clone()).unwrap();
    assert_eq!(
        authority.claims().parent_receipt_digest(),
        Some(current.receipt_digest().as_str())
    );
    host.admit_execution_permit(
        &envelope,
        HostExecutionPermitAdmission {
            action: Action::Test,
            scope: new_scope,
            span: envelope.user_span,
            authority_revision: AuthorityRevision(1),
            revocation_revision: 0,
            now_epoch: next_operation,
            attestation: authority,
        },
        &mut ReplayGuard::default(),
    )
    .unwrap();
}

#[test]
fn packaged_cli_closes_a_real_package_and_rejects_substituted_ingress_before_execution() {
    package_cli_case(PackageCase::Finalize);
}

#[test]
fn production_cli_repairs_an_actual_file_and_revalidates_the_whole_requirement_set() {
    package_cli_case(PackageCase::Repair);
}

#[test]
fn interrupted_production_repair_resumes_with_fresh_evidence_and_without_reapplying() {
    package_cli_case(PackageCase::InterruptedRepair);
}

#[test]
fn epr_11_expired_authentic_history_requires_fresh_validation_before_production_resume() {
    package_cli_case(PackageCase::InterruptedExpiredHistory);
}

#[test]
fn epr_09_real_post_edit_head_failure_never_reports_no_source_effect() {
    package_cli_case(PackageCase::PostEditHeadFailure);
}

enum PackageCase {
    Finalize,
    Repair,
    InterruptedRepair,
    InterruptedExpiredHistory,
    PostEditHeadFailure,
}

fn package_cli_case(case: PackageCase) {
    let repair_mode = !matches!(case, PackageCase::Finalize);
    let interrupt = matches!(
        case,
        PackageCase::InterruptedRepair | PackageCase::InterruptedExpiredHistory
    );
    let expired_history = matches!(case, PackageCase::InterruptedExpiredHistory);
    let post_edit_failure = matches!(case, PackageCase::PostEditHeadFailure);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let base = std::env::temp_dir().join(format!(
        "dgcl CLI package {} {}",
        std::process::id(),
        now.as_nanos()
    ));
    let package = base.join("package");
    std::fs::create_dir_all(package.join("src")).unwrap();
    std::fs::create_dir_all(package.join("tests")).unwrap();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../fixtures/dgcl/producer-package");
    for path in [
        "Cargo.toml",
        "Cargo.lock",
        "src/lib.rs",
        "tests/acceptance.rs",
        "tests/connection.rs",
    ] {
        std::fs::copy(fixture.join(path), package.join(path)).unwrap();
    }
    let host_root = base.join("host-root");
    let author_root = base.join("author-root");
    let host = HostReceiptContext::load_or_create(&host_root).unwrap();
    let author = HostReceiptContext::load_or_create(&author_root).unwrap();
    let correct = std::fs::read_to_string(package.join("src/lib.rs")).unwrap();
    let broken = correct.replace("a.checked_add(b)", "Some(a.wrapping_add(b))");
    if repair_mode {
        std::fs::write(package.join("src/lib.rs"), &broken).unwrap();
    }
    let source = if repair_mode {
        "Please edit the source and test the package."
    } else {
        "Please test the package."
    };
    let revision = stable_sha256(source);
    let pipeline = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let cargo_output = Command::new("rustup")
        .args(["which", "cargo"])
        .output()
        .unwrap();
    assert!(cargo_output.status.success());
    let cargo = PathBuf::from(String::from_utf8(cargo_output.stdout).unwrap().trim());
    let plans = pipeline
        .completion_plan
        .tasks
        .iter()
        .filter(|task| task.evidence_kind != CodingEvidenceKind::ImplementationBinding)
        .map(|task| DgclCargoValidationPlan {
            schema_version: "epistesys-dgcl-cargo-validation-plan.v1".into(),
            source_revision: revision.clone(),
            completion_plan_digest: pipeline.completion_plan.plan_digest.clone(),
            requirement_id: task.requirement_id.clone(),
            task_id: task.task_id.clone(),
            kind: task.evidence_kind,
            cargo_path: cargo.to_string_lossy().into_owned(),
            cargo_digest: hash(&cargo),
            manifest_path: "Cargo.toml".into(),
            manifest_digest: hash(&package.join("Cargo.toml")),
            repository_root_digest: lc631_analysis::dgcl_repository_root_digest(&package).unwrap(),
            timeout_ms: 60000,
            tool_registration: Some(
                host.issuer()
                    .issue(
                        ReceiptClass::ToolExecution,
                        SubjectRevision::checked(hash(&cargo)).unwrap(),
                        ReceiptScope::checked(format!(
                            "dgcl/registered-tool/cargo/{}",
                            hash(&cargo)
                        ))
                        .unwrap(),
                        lc631_analysis::dgcl_cargo_registration_payload(
                            &cargo.to_string_lossy(),
                            &hash(&cargo),
                        ),
                        now.as_secs(),
                        Some(now.as_secs() + 300),
                        None,
                    )
                    .unwrap(),
            ),
            acceptance_target: (task.evidence_kind == CodingEvidenceKind::AcceptanceTest)
                .then(|| "acceptance".into()),
            acceptance_test: (task.evidence_kind == CodingEvidenceKind::AcceptanceTest)
                .then(|| "required_checked_arithmetic_contract".into()),
            connection: (task.evidence_kind == CodingEvidenceKind::ProductionConnection)
                .then(|| connection_spec(&package, &host, now.as_secs())),
        })
        .collect::<Vec<_>>();
    let time = now.as_secs();
    let span = SourceSpan {
        start: 0,
        end: source.len(),
    };
    let mut seeds = Vec::new();
    for (index, plan) in plans.iter().enumerate() {
        let scope = dgcl_cargo_validation_scope(plan);
        let artifact = ArtifactId(900 + index as u64);
        let turn = TurnId(1);
        let receipt = host
            .issuer()
            .issue(
                ReceiptClass::HostSeed,
                SubjectRevision::checked(&revision).unwrap(),
                ReceiptScope::checked(format!(
                    "host/user-span/{}/{}/{}",
                    artifact.0,
                    turn.0,
                    stable_sha256(&scope)
                ))
                .unwrap(),
                host_seed_scoped_payload_digest(artifact, turn, source, span, &scope),
                time,
                Some(time + 300),
                None,
            )
            .unwrap();
        let event = lc631_core::AuthorityEvent {
            action: Action::Test,
            source: lc631_core::AuthoritySourceKind::VerifiedHostUserSpan,
            polarity: lc631_core::DeonticPolarity::Grant,
            span,
            scope: scope.clone(),
        };
        let authority_receipt = host
            .issuer()
            .issue(
                ReceiptClass::Authority,
                SubjectRevision::checked(&revision).unwrap(),
                ReceiptScope::checked(format!("authority/test/{scope}")).unwrap(),
                lc631_core::authority_event_payload_digest(&event),
                time,
                Some(time + 300),
                Some(receipt.receipt_digest()),
            )
            .unwrap();
        seeds.push(json!({"artifact_id": artifact.0, "turn_id": turn.0, "user_start": 0, "user_end": source.len(), "receipt": receipt,
            "authority_receipt": authority_receipt }));
    }
    let first = serde_json::from_value(seeds[0]["receipt"].clone()).unwrap();
    let seed = HostSeedEnvelope::verify_attested_scoped(
        ArtifactId(900),
        TurnId(1),
        source.into(),
        span,
        dgcl_cargo_validation_scope(&plans[0]),
        first,
        host.verifier(),
        &mut ReplayGuard::default(),
        time,
    )
    .unwrap();
    let principal = seed.principal_binding().unwrap();
    let fingerprint = host.key_fingerprint();
    let scope = dgcl_cargo_validation_scope(&plans[0]);
    let snapshot = capture_dgcl_cargo_snapshot(
        &package,
        source,
        &pipeline,
        &ExecutionPermitContext {
            principal: &principal,
            action: Action::Test,
            scope: &scope,
            source_revision: &revision,
            authority_revision: AuthorityRevision(1),
            now_epoch: time,
            revocation_revision: 0,
            revoked_permit_digests: &[],
            caller_origin: CallerOrigin::HostVerifiedUser,
            trusted_host_fingerprint: &fingerprint,
        },
    )
    .unwrap();
    let snapshot = if repair_mode {
        lc631_analysis::project_dgcl_cargo_target_snapshot(
            &package,
            source,
            &pipeline,
            &snapshot,
            "src/lib.rs",
            &stable_sha256(&correct),
            1,
        )
        .unwrap()
    } else {
        snapshot
    };
    let binding_tasks = pipeline
        .completion_plan
        .tasks
        .iter()
        .filter(|task| task.evidence_kind == CodingEvidenceKind::ImplementationBinding);
    let mut bindings = binding_tasks
        .map(|task| {
            author
                .issuer()
                .issue(
                    ReceiptClass::Closure,
                    SubjectRevision::checked(&revision).unwrap(),
                    ReceiptScope::checked(format!("dgcl/implementation-binding/{}", task.task_id))
                        .unwrap(),
                    dgcl_implementation_binding_payload(&pipeline, &task.task_id, &snapshot),
                    time,
                    Some(time + 300),
                    None,
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    let binding = bindings.remove(0);
    let validation = json!({"schema_version": "epistesys-dgcl-package-validation-bundle.v1", "authority_revision": 1,
        "revocation_revision": 0, "plans": plans, "seeds": seeds, "binding_receipt": binding, "binding_receipts": bindings});
    let bundle = if repair_mode {
        let root_digest = lc631_analysis::dgcl_repository_root_digest(&package).unwrap();
        let edit_scope = format!("dgcl/repair/{root_digest}/src/lib.rs");
        let edit_seed = host
            .issuer()
            .issue(
                ReceiptClass::HostSeed,
                SubjectRevision::checked(&revision).unwrap(),
                ReceiptScope::checked(format!(
                    "host/user-span/899/1/{}",
                    stable_sha256(&edit_scope)
                ))
                .unwrap(),
                host_seed_scoped_payload_digest(
                    ArtifactId(899),
                    TurnId(1),
                    source,
                    span,
                    &edit_scope,
                ),
                time,
                Some(time + 300),
                None,
            )
            .unwrap();
        let event = lc631_core::AuthorityEvent {
            action: Action::Edit,
            source: lc631_core::AuthoritySourceKind::VerifiedHostUserSpan,
            polarity: lc631_core::DeonticPolarity::Grant,
            span,
            scope: edit_scope.clone(),
        };
        let edit_authority = host
            .issuer()
            .issue(
                ReceiptClass::Authority,
                SubjectRevision::checked(&revision).unwrap(),
                ReceiptScope::checked(format!("authority/edit/{edit_scope}")).unwrap(),
                lc631_core::authority_event_payload_digest(&event),
                time,
                Some(time + 300),
                Some(edit_seed.receipt_digest()),
            )
            .unwrap();
        let gap = pipeline
            .implementation_closure
            .gaps
            .iter()
            .find(|gap| gap.evidence_kind == Some(CodingEvidenceKind::RuntimeValidation))
            .unwrap();
        let request = lc631_analysis::DgclRepairRequest {
            attempt: 1,
            gap_id: gap.gap_id.clone(),
            source_revision: revision.clone(),
            plan_digest: pipeline.completion_plan.plan_digest.clone(),
            target_ref: "src/lib.rs".into(),
            target_digest_before: stable_sha256(&broken),
            candidate_digest: stable_sha256(&correct),
            authorization_scope: edit_scope,
            authorization_span: span,
            repository_root_digest: Some(root_digest),
        };
        json!({"schema_version": "epistesys-dgcl-package-repair-bundle.v1", "request": request, "replacement_utf8": correct,
            "edit_seed": {"artifact_id": 899, "turn_id": 1, "user_start": 0, "user_end": source.len(), "receipt": edit_seed, "authority_receipt": edit_authority},
            "validation": validation })
    } else {
        validation
    };
    let bundle_path = base.join("validation-bundle.json");
    let journal_root = base.join("repair-journal");
    let head_root = base.join("repair-heads");
    std::fs::create_dir_all(&journal_root).unwrap();
    std::fs::create_dir_all(&head_root).unwrap();
    let invoke = |path: &Path| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lc631"));
        command
            .current_dir(&base)
            .arg(if repair_mode {
                "lc631-dgcl-package-repair"
            } else {
                "lc631-dgcl-package-finalize"
            })
            .arg("--execute")
            .arg("--repo")
            .arg(&package)
            .arg("--prompt")
            .arg(source)
            .arg("--evidence-file")
            .arg(path)
            .arg("--receipt-root")
            .arg(&host_root)
            .arg("--author-receipt-root")
            .arg(&author_root)
            .arg("--ledger-root")
            .arg(&journal_root)
            .arg("--head-root")
            .arg(&head_root);
        if !post_edit_failure || path != bundle_path {
            return command.output().unwrap();
        }
        let mut child = command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                let output = child.wait_with_output().unwrap();
                panic!(
                    "repair ended before observed edit ({status}): {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("owned repair did not reach the edit boundary within its test deadline");
            }
            if std::fs::read_to_string(package.join("src/lib.rs"))
                .ok()
                .as_deref()
                == Some(correct.as_str())
            {
                // Real filesystem failure after the real edit. Occupy only the
                // next append-only head slot in this disposable, owned fixture.
                let mut fault = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(head_root.join("head-00000000000000000002.json"))
                    .unwrap();
                std::io::Write::write_all(&mut fault, b"occupied by post-edit journal fault test")
                    .unwrap();
                fault.sync_all().unwrap();
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        child.wait_with_output().unwrap()
    };
    if interrupt {
        let current = fresh_test_bundle(&bundle, source, &host, &author, epoch_now());
        std::fs::write(&bundle_path, serde_json::to_vec(&current).unwrap()).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_lc631"))
            .current_dir(&base)
            .args(["lc631-dgcl-package-repair", "--execute", "--prompt", source])
            .arg("--repo")
            .arg(&package)
            .arg("--evidence-file")
            .arg(&bundle_path)
            .arg("--receipt-root")
            .arg(&host_root)
            .arg("--author-receipt-root")
            .arg(&author_root)
            .arg("--ledger-root")
            .arg(&journal_root)
            .arg("--head-root")
            .arg(&head_root)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            assert!(
                std::time::Instant::now() < deadline,
                "repair must reach the write boundary"
            );
            assert!(
                child.try_wait().unwrap().is_none(),
                "child ended before interruption"
            );
            if std::fs::read_to_string(package.join("src/lib.rs"))
                .ok()
                .as_deref()
                == Some(correct.as_str())
            {
                child.kill().unwrap();
                child.wait().unwrap();
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let checkpoint: Value = serde_json::from_slice(
            &std::fs::read(journal_root.join("checkpoint-00000000000000000000.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(checkpoint["body"]["event_kind"], "action_started");
        if expired_history {
            // Controlled fixture clock: replace only the test-owned head with
            // an authentic expired attestation of the exact same stored action.
            let path = head_root.join("head-00000000000000000001.json");
            let original: lc631_analysis::CheckpointHeadAnchor =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            let expired = lc631_analysis::issue_checkpoint_head_anchor(
                host.issuer(),
                lc631_analysis::CheckpointHeadRequest {
                    source_revision: &original.source_revision,
                    plan_digest: &original.plan_digest,
                    authority_revision: &original.authority_revision,
                    sequence: original.sequence,
                    head_digest: original.head_digest.as_deref(),
                    now_epoch: time.saturating_sub(7200),
                    lease_seconds: 1,
                },
            )
            .unwrap();
            std::fs::write(&path, serde_json::to_vec(&expired).unwrap()).unwrap();
            assert!(lc631_analysis::load_checkpoint_head(
                &journal_root,
                &head_root,
                host.verifier(),
                time
            )
            .is_err());
            assert!(lc631_analysis::load_checkpoint_head_historical(
                &journal_root,
                &head_root,
                host.verifier(),
                time
            )
            .is_ok());
        }
    }
    let mut invalid = bundle.clone();
    let validation_key = if repair_mode { "/validation" } else { "" };
    invalid
        .pointer_mut(&format!("{validation_key}/seeds/0/artifact_id"))
        .map(|value| *value = json!(777))
        .unwrap();
    let invalid_path = base.join("substituted-ingress.json");
    std::fs::write(&invalid_path, serde_json::to_vec(&invalid).unwrap()).unwrap();
    let rejected = invoke(&invalid_path);
    assert!(!rejected.status.success());
    let mut unregistered = bundle.clone();
    *unregistered
        .pointer_mut(&format!("{validation_key}/plans/0/tool_registration"))
        .unwrap() = Value::Null;
    let unregistered_path = base.join("unregistered-tool.json");
    std::fs::write(
        &unregistered_path,
        serde_json::to_vec(&unregistered).unwrap(),
    )
    .unwrap();
    assert!(!invoke(&unregistered_path).status.success());
    let mut no_authority = bundle.clone();
    *no_authority
        .pointer_mut(&format!("{validation_key}/seeds/0/authority_receipt"))
        .unwrap() = Value::Null;
    let no_authority_path = base.join("seed-is-not-a-grant.json");
    std::fs::write(
        &no_authority_path,
        serde_json::to_vec(&no_authority).unwrap(),
    )
    .unwrap();
    assert!(!invoke(&no_authority_path).status.success());
    assert!(
        interrupt || !package.join("target").exists(),
        "forged ingress must be rejected before Cargo creates artifacts"
    );
    let bundle = fresh_test_bundle(&bundle, source, &host, &author, epoch_now());
    std::fs::write(&bundle_path, serde_json::to_vec(&bundle).unwrap()).unwrap();
    let resume = |path: &Path| {
        Command::new(env!("CARGO_BIN_EXE_lc631"))
            .current_dir(&base)
            .args(["lc631-dgcl-package-resume", "--execute", "--prompt", source])
            .arg("--repo")
            .arg(&package)
            .arg("--evidence-file")
            .arg(path)
            .arg("--receipt-root")
            .arg(&host_root)
            .arg("--author-receipt-root")
            .arg(&author_root)
            .arg("--ledger-root")
            .arg(&journal_root)
            .arg("--head-root")
            .arg(&head_root)
            .output()
            .unwrap()
    };
    let observed = if interrupt {
        // An existing journal rejects a second mutating invocation.
        let rejected = invoke(&bundle_path);
        assert!(!rejected.status.success());
        if expired_history {
            assert!(
                !resume(&no_authority_path).status.success(),
                "history must not replace fresh Test authority"
            );
            let unrenewed = lc631_analysis::load_checkpoint_head_historical(
                &journal_root,
                &head_root,
                host.verifier(),
                time,
            )
            .unwrap();
            assert!(
                unrenewed.expires_at_epoch < time,
                "failed validation renewed historical lease"
            );
        }
        resume(&bundle_path)
    } else {
        invoke(&bundle_path)
    };
    if post_edit_failure {
        assert!(
            !observed.status.success(),
            "journal failure must not be success"
        );
        assert_eq!(
            std::fs::read_to_string(package.join("src/lib.rs")).unwrap(),
            correct
        );
        let failure: Value = serde_json::from_slice(&observed.stderr).unwrap();
        let effect = failure["effect_state"]["source_effect"].as_str().unwrap();
        assert!(
            matches!(effect, "applied_observed" | "may_have_applied"),
            "{failure}"
        );
        assert!(!String::from_utf8_lossy(&observed.stderr).contains("no mutation"));
        return;
    }
    assert!(
        observed.status.success(),
        "{}",
        String::from_utf8_lossy(&observed.stderr)
    );
    let report: Value = serde_json::from_slice(&observed.stdout).unwrap();
    capture_schema_surface(&base, "bundle", &bundle);
    capture_schema_surface(&base, "output", &report["payload"]);
    if interrupt {
        assert_eq!(
            report["payload"]["candidate"]["implementation_complete_candidate"],
            true
        );
        assert_eq!(report["payload"]["repair_reapplied"], false);
        assert_eq!(
            report["payload"]["completion_restored_from_checkpoint"],
            false
        );
        assert_eq!(
            report["payload"]["journal"]["latest_event_kind"],
            "action_observed"
        );
        return;
    }
    if repair_mode {
        assert_eq!(report["payload"]["repair"]["status"], "completed");
        assert!(report["payload"]["repair"]["remaining_gap_ids"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(
            std::fs::read_to_string(package.join("src/lib.rs")).unwrap(),
            correct
        );
        assert_eq!(report["payload"]["output_commit_allowed"], false);
        assert_eq!(
            report["payload"]["journal"]["integrity_state"],
            "trusted_head_matched"
        );
        let current = fresh_test_bundle(&bundle, source, &host, &author, epoch_now());
        std::fs::write(&bundle_path, serde_json::to_vec(&current).unwrap()).unwrap();
        let resumed = Command::new(env!("CARGO_BIN_EXE_lc631"))
            .current_dir(&base)
            .args(["lc631-dgcl-package-resume", "--execute", "--prompt", source])
            .arg("--repo")
            .arg(&package)
            .arg("--evidence-file")
            .arg(&bundle_path)
            .arg("--receipt-root")
            .arg(&host_root)
            .arg("--author-receipt-root")
            .arg(&author_root)
            .arg("--ledger-root")
            .arg(&journal_root)
            .arg("--head-root")
            .arg(&head_root)
            .output()
            .unwrap();
        assert!(
            resumed.status.success(),
            "{}",
            String::from_utf8_lossy(&resumed.stderr)
        );
        let resumed: Value = serde_json::from_slice(&resumed.stdout).unwrap();
        capture_schema_surface(&base, "resume", &resumed["payload"]);
        assert_eq!(
            resumed["payload"]["candidate"]["implementation_complete_candidate"],
            true
        );
        assert_eq!(resumed["payload"]["repair_reapplied"], false);
        assert_eq!(
            resumed["payload"]["completion_restored_from_checkpoint"],
            false
        );
        assert_eq!(
            std::fs::read_to_string(package.join("src/lib.rs")).unwrap(),
            correct
        );
        return;
    }
    assert_eq!(
        report["payload"]["candidate"]["implementation_complete_candidate"],
        true
    );
    assert_eq!(
        report["payload"]["candidate"]["implementation_closure_status"],
        "implementation_closed"
    );
    assert_eq!(report["payload"]["claims"].as_array().unwrap().len(), 5);
    assert_eq!(report["payload"]["processes"].as_array().unwrap().len(), 4);
    assert_eq!(
        report["payload"]["candidate"]["output_commit_allowed"],
        false
    );
    assert_eq!(report["payload"]["host_observation"], "pending");
}

fn capture_schema_surface(base: &Path, kind: &str, payload: &Value) {
    if let Ok(output) = std::env::var("EPISTESYS_SCHEMA_CAPTURE_DIR") {
        let output = PathBuf::from(output);
        assert!(output.is_absolute() && output.is_dir());
        let name = base.file_name().unwrap().to_string_lossy();
        let path = output.join(format!("{name}-{kind}.json"));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .unwrap();
        use std::io::Write;
        file.write_all(&serde_json::to_vec(payload).unwrap())
            .unwrap();
    }
}
