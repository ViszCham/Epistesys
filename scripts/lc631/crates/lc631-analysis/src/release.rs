use std::collections::BTreeSet;

use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
    UntrustedReceipt,
};

use crate::{
    ClosureState, EvidenceState, ReceiptBinding, ReleaseEvidenceBundle, ReleaseSummary,
    StageReceipt,
};

const V630_BASELINE_COMMIT: &str = "0d933982fa392043c2fe825644a3a560963b6615";

pub fn expected_v630_baseline_digest() -> String {
    stable_sha256(&format!("v6.3.0:{V630_BASELINE_COMMIT}"))
}

pub(crate) fn apply_release_evidence(
    stages: &mut [StageReceipt],
    bundle: &ReleaseEvidenceBundle,
    source_revision: Option<&str>,
    verifier: Option<&ReceiptVerifier>,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> ReleaseSummary {
    let mut seen = BTreeSet::new();
    let mut accepted = 0_usize;
    let mut rejected = 0_usize;
    let mut receipts = bundle.stage_evidence.iter().collect::<Vec<_>>();
    receipts.sort_by_key(|receipt| stage_number(&receipt.stage_id).unwrap_or(u8::MAX));
    for receipt in receipts {
        let Some(number) = stage_number(&receipt.stage_id) else {
            rejected += 1;
            continue;
        };
        if !(0..=38).contains(&number)
            || !seen.insert(receipt.stage_id.clone())
            || receipt.binding != ReceiptBinding::HostBound
            || source_revision != Some(receipt.source_revision.as_str())
            || !digest(&receipt.evidence_digest)
            || receipt.scope.trim().is_empty()
            || !receipt.unsupported.is_empty()
            || !stage_receipt_is_authentic(receipt, source_revision, verifier, replay, now_epoch)
        {
            rejected += 1;
            continue;
        }
        let dependencies_observed = stages
            .iter()
            .find(|stage| stage.stage_id == receipt.stage_id)
            .is_some_and(|stage| {
                stage.dependencies.iter().all(|dependency| {
                    stages
                        .iter()
                        .find(|candidate| candidate.stage_id == *dependency)
                        .is_some_and(|candidate| {
                            matches!(
                                candidate.closure_state,
                                ClosureState::Satisfied | ClosureState::NotApplicable
                            )
                        })
                })
            });
        let stage = stages
            .iter_mut()
            .find(|stage| stage.stage_id == receipt.stage_id);
        if dependencies_observed
            && stage.as_ref().is_some_and(|stage| {
                !matches!(
                    stage.state,
                    EvidenceState::Refuted | EvidenceState::NotApplicable
                )
            })
        {
            let stage = stage.unwrap();
            stage.state = EvidenceState::Observed;
            stage.evidence_digests = vec![receipt.evidence_digest.clone()];
            stage.blockers.clear();
            stage.closure_state = ClosureState::Satisfied;
            accepted += 1;
        } else {
            rejected += 1;
        }
    }

    let evaluation_authentic = verify_component(
        verifier,
        replay,
        bundle.evaluation_attestation.clone(),
        ReceiptClass::ReleaseEvaluation,
        source_revision,
        "release/evaluation",
        &evaluation_payload_digest(bundle),
        now_epoch,
    );
    let evaluation_complete = evaluation_authentic
        && bundle.semantic_reversal_observed
        && bundle.directional_loss_observed
        && bundle.dead_fixture_count == 0
        && digest(&bundle.evaluation_corpus_digest)
        && digest(&bundle.evaluation_result_digest)
        && stages
            .iter()
            .find(|stage| stage.stage_id == "RPA-38")
            .is_some_and(|stage| stage.state == EvidenceState::Observed);
    let v630_baseline_bound = bundle.v630_baseline_digest == expected_v630_baseline_digest()
        && verify_component(
            verifier,
            replay,
            bundle.v630_baseline_attestation.clone(),
            ReceiptClass::Baseline,
            Some("v6.3.0@0d933982fa392043c2fe825644a3a560963b6615"),
            "release/baseline/v6.3.0",
            &bundle.v630_baseline_digest,
            now_epoch,
        );
    let host_output_bound = digest(&bundle.exact_host_output_digest)
        && verify_component(
            verifier,
            replay,
            bundle.exact_host_output_attestation.clone(),
            ReceiptClass::HostOutput,
            source_revision,
            "release/host-output",
            &bundle.exact_host_output_digest,
            now_epoch,
        );
    let package_bound = digest(&bundle.package_manifest_digest)
        && verify_component(
            verifier,
            replay,
            bundle.package_manifest_attestation.clone(),
            ReceiptClass::Package,
            source_revision,
            "release/package",
            &bundle.package_manifest_digest,
            now_epoch,
        );
    let remote_ci_bound = digest(&bundle.remote_ci_digest)
        && verify_component(
            verifier,
            replay,
            bundle.remote_ci_attestation.clone(),
            ReceiptClass::RemoteCi,
            source_revision,
            "release/remote-ci",
            &bundle.remote_ci_digest,
            now_epoch,
        );
    let authenticity_complete = evaluation_authentic
        && v630_baseline_bound
        && host_output_bound
        && package_bound
        && remote_ci_bound;
    let prior_complete = (0..=38).all(|number| {
        stages
            .iter()
            .find(|stage| stage.stage_id == format!("RPA-{number:02}"))
            .is_some_and(|stage| {
                matches!(
                    stage.closure_state,
                    ClosureState::Satisfied | ClosureState::NotApplicable
                )
            })
    });
    let release_complete = evaluation_complete
        && v630_baseline_bound
        && host_output_bound
        && package_bound
        && remote_ci_bound
        && authenticity_complete
        && prior_complete;
    if let Some(stage) = stages.iter_mut().find(|stage| stage.stage_id == "RPA-39") {
        stage.state = if release_complete {
            EvidenceState::Observed
        } else {
            EvidenceState::NeedsEvidence
        };
        stage.evidence_digests = if release_complete {
            vec![
                bundle.evaluation_result_digest.clone(),
                bundle.v630_baseline_digest.clone(),
                bundle.exact_host_output_digest.clone(),
                bundle.package_manifest_digest.clone(),
                bundle.remote_ci_digest.clone(),
            ]
        } else {
            Vec::new()
        };
    }
    let mut blockers = Vec::new();
    if !evaluation_complete {
        blockers.push("RPA-38 evaluation is incomplete".to_string());
    }
    if !v630_baseline_bound {
        blockers.push("v6.3.0 baseline digest is unbound or mismatched".to_string());
    }
    if !host_output_bound {
        blockers.push("exact host output digest is unbound".to_string());
    }
    if !package_bound {
        blockers.push("package manifest digest is unbound".to_string());
    }
    if !remote_ci_bound {
        blockers.push("remote CI digest is unbound".to_string());
    }
    if !prior_complete {
        blockers.push("one or more RPA-00..38 stages are not Observed".to_string());
    }

    ReleaseSummary {
        accepted_stage_receipts: accepted,
        rejected_stage_receipts: rejected,
        evaluation_complete,
        v630_baseline_bound,
        host_output_bound,
        package_bound,
        remote_ci_bound,
        release_complete,
        authenticity_complete,
        blockers,
    }
}

fn stage_receipt_is_authentic(
    receipt: &crate::StageCompletionReceipt,
    source_revision: Option<&str>,
    verifier: Option<&ReceiptVerifier>,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> bool {
    verify_component(
        verifier,
        replay,
        receipt.attestation.clone(),
        ReceiptClass::StageCompletion,
        source_revision,
        &format!("release/stage/{}/{}", receipt.stage_id, receipt.scope),
        &stage_completion_payload_digest(receipt),
        now_epoch,
    )
}

#[allow(clippy::too_many_arguments)]
fn verify_component(
    verifier: Option<&ReceiptVerifier>,
    replay: &mut ReplayGuard,
    attestation: Option<UntrustedReceipt>,
    class: ReceiptClass,
    subject: Option<&str>,
    scope: &str,
    payload: &str,
    now_epoch: u64,
) -> bool {
    let Some((verifier, attestation, subject)) = verifier
        .zip(attestation)
        .zip(subject)
        .map(|((verifier, attestation), subject)| (verifier, attestation, subject))
    else {
        return false;
    };
    let Ok(subject) = SubjectRevision::checked(subject) else {
        return false;
    };
    let Ok(scope) = ReceiptScope::checked(scope) else {
        return false;
    };
    verifier
        .verify(
            attestation,
            &ReceiptPolicy::exact(class, subject, scope, payload, now_epoch),
            replay,
        )
        .is_ok()
}

pub fn stage_completion_payload_digest(receipt: &crate::StageCompletionReceipt) -> String {
    stable_sha256(&format!(
        "{}\0{:?}\0{}\0{}\0{}\0{}",
        receipt.stage_id,
        receipt.binding,
        receipt.source_revision,
        receipt.evidence_digest,
        receipt.scope,
        receipt.unsupported.join("\u{1f}")
    ))
}

pub fn evaluation_payload_digest(bundle: &ReleaseEvidenceBundle) -> String {
    stable_sha256(&format!(
        "{}\0{}\0{}\0{}\0{}\0{}",
        bundle.evaluation_corpus_digest,
        bundle.evaluation_result_digest,
        bundle.semantic_reversal_observed,
        bundle.directional_loss_observed,
        bundle.dead_fixture_count,
        bundle.bundle_version
    ))
}

pub fn release_component_scope(component: &str) -> String {
    format!("release/{component}")
}

fn stage_number(value: &str) -> Option<u8> {
    value.strip_prefix("RPA-")?.parse().ok()
}

fn digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}
