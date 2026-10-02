use lc631_analysis::*;
use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};

// Explicit development receipts test binding/closure mechanics. Actual Cargo
// observations are tested separately in dgcl_validation_producers; this is not
// independent semantic gold and does not attest real compiler execution.
fn fixture(
    source: &str,
) -> (
    DgclPipelineReport,
    DgclExecutionSnapshot,
    Vec<CodingEvidenceClaim>,
    ReceiptVerifier,
) {
    let pipeline = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let issuer = ReceiptIssuer::from_key_bytes("disconnect-fixture", &[99; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("disconnect-fixture", &[99; 32]).unwrap();
    let targets = pipeline
        .completion_plan
        .tasks
        .iter()
        .map(|task| (task.target_ref.clone(), task.target_digest.clone()))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_iter()
        .map(|(target_ref, target_digest)| DgclTargetIdentity {
            target_ref,
            target_digest,
        })
        .collect();
    let snapshot = build_dgcl_execution_snapshot(
        source,
        b"explicit development build fixture",
        "profile.v1",
        "validator.v1",
        "authority.v1",
        targets,
    )
    .unwrap();
    let claims = pipeline
        .completion_plan
        .tasks
        .iter()
        .filter(|task| task.applicability != TaskApplicability::NotApplicable)
        .map(|task| {
            let mut claim = CodingEvidenceClaim {
                requirement_id: 1,
                coding_requirement_id: Some(task.requirement_id.clone()),
                coding_task_id: Some(task.task_id.clone()),
                kind: task.evidence_kind,
                source_revision: pipeline.source_revision.clone(),
                target: task.target_ref.clone(),
                target_digest: task.target_digest.clone(),
                producer: Some(task.producer),
                producer_run_digest: Some(stable_sha256(&format!("fixture-run:{}", task.task_id))),
                observation_digest: Some(stable_sha256(&format!(
                    "fixture-observation:{}",
                    task.task_id
                ))),
                lifecycle_lease: Some(issue_dgcl_evidence_lease(&snapshot, 100, 300).unwrap()),
                attestation: None,
            };
            claim.attestation = Some(
                issuer
                    .issue(
                        ReceiptClass::Closure,
                        SubjectRevision::checked(&claim.source_revision).unwrap(),
                        ReceiptScope::checked(coding_evidence_scope(&claim)).unwrap(),
                        coding_evidence_payload_digest(&claim),
                        100,
                        Some(400),
                        None,
                    )
                    .unwrap(),
            );
            claim
        })
        .collect();
    (pipeline, snapshot, claims, verifier)
}

fn candidate(
    source: &str,
    pipeline: &DgclPipelineReport,
    snapshot: &DgclExecutionSnapshot,
    claims: &[CodingEvidenceClaim],
    verifier: &ReceiptVerifier,
) -> Result<DgclStandaloneCandidate, DgclStandaloneCandidateError> {
    build_verified_dgcl_completion_candidate(
        source,
        pipeline,
        claims,
        DgclImplementationClosureContext {
            current_lifecycle: Some(snapshot),
            verifier: Some(verifier),
            replay: &mut ReplayGuard::default(),
            now_epoch: 110,
        },
    )
}

#[test]
fn sixteen_cuts_are_observed_then_original_requirement_identity_is_restored() {
    let source = "Please test file README.md.";
    let (original, snapshot, claims, verifier) = fixture(source);
    let positive = candidate(source, &original, &snapshot, &claims, &verifier).unwrap();
    assert!(positive.implementation_complete_candidate);
    let ids = original
        .completion_plan
        .requirements
        .iter()
        .map(|requirement| requirement.requirement_id.clone())
        .collect::<Vec<_>>();
    let mut recovered = 0;
    for id in 1..=16 {
        let mut cut = original.clone();
        let mut cut_claims = claims.clone();
        let mut cut_snapshot = snapshot.clone();
        match id {
            1 => cut.source_revision = stable_sha256("substituted source anchor"),
            2 => cut.program_ir.grammar_revision = "substituted-grammar.v1",
            3 => cut.authority_created = true,
            4 => {
                cut.program_ir.requirements[0].alternatives[0].ast.actions[0].polarity =
                    InstructionPolarity::Forbidden
            }
            5 => {
                let conditional = "Please publish only if tests pass.";
                let (before, state, receipts, trusted) = fixture(conditional);
                assert!(!before.program_ir.requirements[0].alternatives[0]
                    .ast
                    .conditions
                    .is_empty());
                let mut missing_condition = before.clone();
                missing_condition.program_ir.requirements[0].alternatives[0]
                    .ast
                    .conditions
                    .clear();
                assert!(
                    candidate(conditional, &missing_condition, &state, &receipts, &trusted)
                        .is_err()
                );
                let restored =
                    candidate(conditional, &before, &state, &receipts, &trusted).unwrap();
                assert_eq!(
                    restored.requirement_realizations[0].requirement_id,
                    before.completion_plan.requirements[0].requirement_id
                );
                assert_eq!(
                    restored.implementation_closure_status,
                    DgclImplementationStatus::Hold,
                    "restoring condition structure must not invent condition truth"
                );
                recovered += 1;
                continue;
            }
            6 => {
                cut.program_ir.requirements[0].alternatives[0].ast.scopes[0].value =
                    "all files".into()
            }
            7 => cut.completion_plan.requirements.clear(),
            8 => cut.translation_projection.loss_events.clear(),
            9 => {
                cut_claims
                    .iter_mut()
                    .find(|claim| claim.kind == CodingEvidenceKind::ImplementationBinding)
                    .unwrap()
                    .target_digest = stable_sha256("wrong code binding")
            }
            10 => cut_claims.retain(|claim| claim.kind != CodingEvidenceKind::ProductionConnection),
            11 => {
                cut_claims
                    .iter_mut()
                    .find(|claim| claim.kind == CodingEvidenceKind::RuntimeValidation)
                    .unwrap()
                    .producer_run_digest = None
            }
            12 => {
                cut_claims
                    .iter_mut()
                    .find(|claim| claim.kind == CodingEvidenceKind::AcceptanceTest)
                    .unwrap()
                    .observation_digest = Some(stable_sha256("substituted oracle"))
            }
            13 => {
                cut_claims[0].producer = Some(
                    if cut_claims[0].producer == Some(CodingEvidenceProducer::RuntimeObserved) {
                        CodingEvidenceProducer::CompilerObserved
                    } else {
                        CodingEvidenceProducer::RuntimeObserved
                    },
                )
            }
            14 => {
                cut_snapshot = build_dgcl_execution_snapshot(
                    source,
                    b"changed build inputs",
                    "profile.v1",
                    "validator.v1",
                    "authority.v1",
                    snapshot.targets.clone(),
                )
                .unwrap();
            }
            15 => cut_claims.push(cut_claims[0].clone()),
            16 => {
                let mut public = original.clone();
                public
                    .implementation_closure
                    .implementation_complete_candidate = true;
                let substituted = build_dgcl_standalone_candidate(source, &public,
                    br#"{"schema_version":"substituted.v1","implementation_complete_candidate":true}"#, "substituted.v1").unwrap();
                assert!(!substituted.implementation_complete_candidate);
                assert!(build_dgcl_standalone_candidate(
                    source,
                    &public,
                    br#"{"schema_version":"substituted.v1"}"#,
                    "epistesys-dgcl-completion-output.v1"
                )
                .is_err());
                recovered += 1;
                continue;
            }
            _ => unreachable!(),
        }
        let observed = candidate(source, &cut, &cut_snapshot, &cut_claims, &verifier);
        assert!(
            observed.is_err() || !observed.unwrap().implementation_complete_candidate,
            "A{id:02}"
        );
        let restored = candidate(source, &original, &snapshot, &claims, &verifier).unwrap();
        assert!(restored.implementation_complete_candidate, "A{id:02}");
        assert_eq!(
            restored
                .requirement_realizations
                .iter()
                .map(|item| item.requirement_id.clone())
                .collect::<Vec<_>>(),
            ids
        );
        recovered += 1;
    }
    assert_eq!(recovered, 16);
}
