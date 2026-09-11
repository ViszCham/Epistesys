use std::collections::{BTreeMap, BTreeSet};

use crate::{
    EvidenceState, ReceiptBinding, StageReceipt, ValidationEvidenceBundle, ValidationKind,
    ValidationReceipt, ValidationSummary,
};
use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};

pub(crate) fn apply_validation_stages(
    stages: &mut [StageReceipt],
    bundle: &ValidationEvidenceBundle,
    expected_source_revision: Option<&str>,
    verifier: Option<&ReceiptVerifier>,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> ValidationSummary {
    let duplicates = duplicate_kinds(&bundle.receipts);
    let mut accepted = BTreeMap::new();
    for receipt in &bundle.receipts {
        if !duplicates.contains(&receipt.kind)
            && receipt_is_bound(
                receipt,
                expected_source_revision,
                verifier,
                replay,
                now_epoch,
            )
        {
            accepted.insert(receipt.kind, receipt);
        }
    }
    let candidate = bundle
        .receipts
        .iter()
        .filter(|receipt| {
            receipt.binding == ReceiptBinding::LocalCommand
                && receipt.state == EvidenceState::Observed
                && digest(&receipt.invocation_digest)
                && digest(&receipt.output_digest)
                && expected_source_revision == Some(receipt.source_revision.as_str())
        })
        .map(|receipt| (receipt.kind, receipt))
        .collect::<BTreeMap<_, _>>();

    apply_required(
        stages,
        "RPA-19",
        &[
            ValidationKind::Rustfmt,
            ValidationKind::RustcCheck,
            ValidationKind::Clippy,
        ],
        &accepted,
        &candidate,
    );
    apply_required(
        stages,
        "RPA-20",
        &[
            ValidationKind::Tests,
            ValidationKind::Coverage,
            ValidationKind::Mutation,
        ],
        &accepted,
        &candidate,
    );
    apply_required(
        stages,
        "RPA-21",
        &[ValidationKind::Miri],
        &accepted,
        &candidate,
    );
    apply_required(
        stages,
        "RPA-22",
        &[
            ValidationKind::Property,
            ValidationKind::Fuzz,
            ValidationKind::Differential,
        ],
        &accepted,
        &candidate,
    );
    apply_required(
        stages,
        "RPA-23",
        &[ValidationKind::ConcurrencyModel],
        &accepted,
        &candidate,
    );
    apply_required(
        stages,
        "RPA-24",
        &[ValidationKind::FormalVerifier],
        &accepted,
        &candidate,
    );
    apply_required(
        stages,
        "RPA-25",
        &[ValidationKind::SupplyChain, ValidationKind::SemverMsrv],
        &accepted,
        &candidate,
    );
    apply_required(
        stages,
        "RPA-26",
        &[ValidationKind::Performance],
        &accepted,
        &candidate,
    );
    let all_prior_observed = (19..=26).all(|number| {
        stages
            .iter()
            .find(|stage| stage.stage_id == format!("RPA-{number:02}"))
            .is_some_and(|stage| stage.state == EvidenceState::Observed)
    });
    if let Some(stage) = stages.iter_mut().find(|stage| stage.stage_id == "RPA-27") {
        let human = accepted.get(&ValidationKind::HumanReview);
        stage.state = if all_prior_observed && human.is_some() {
            EvidenceState::Observed
        } else if candidate.contains_key(&ValidationKind::HumanReview) {
            EvidenceState::Candidate
        } else {
            EvidenceState::NeedsEvidence
        };
        stage.evidence_digests = human
            .into_iter()
            .map(|receipt| receipt.output_digest.clone())
            .collect();
        stage.blockers = if stage.state == EvidenceState::Observed {
            Vec::new()
        } else {
            vec![
                "all validation stages plus one host-bound human-review receipt are required"
                    .to_string(),
            ]
        };
    }

    ValidationSummary {
        accepted_receipts: accepted.len(),
        rejected_receipts: bundle.receipts.len().saturating_sub(accepted.len()),
        duplicate_kinds: duplicates.into_iter().collect(),
        all_receipts_non_authorizing: true,
        authenticated_receipts: accepted.len(),
        legacy_host_bound_rejected: bundle
            .receipts
            .iter()
            .filter(|receipt| {
                receipt.binding == ReceiptBinding::HostBound
                    && !accepted.contains_key(&receipt.kind)
            })
            .count(),
    }
}

fn apply_required(
    stages: &mut [StageReceipt],
    stage_id: &str,
    required: &[ValidationKind],
    accepted: &BTreeMap<ValidationKind, &ValidationReceipt>,
    candidate: &BTreeMap<ValidationKind, &ValidationReceipt>,
) {
    let complete = required.iter().all(|kind| accepted.contains_key(kind));
    let partial = required.iter().any(|kind| candidate.contains_key(kind));
    if let Some(stage) = stages.iter_mut().find(|stage| stage.stage_id == stage_id) {
        stage.state = if complete {
            EvidenceState::Observed
        } else if partial {
            EvidenceState::Candidate
        } else {
            EvidenceState::NeedsEvidence
        };
        stage.evidence_digests = required
            .iter()
            .filter_map(|kind| accepted.get(kind))
            .map(|receipt| receipt.output_digest.clone())
            .collect();
        stage.blockers = required
            .iter()
            .filter(|kind| !accepted.contains_key(kind))
            .map(|kind| format!("missing host-bound {kind:?} receipt"))
            .collect();
    }
}

fn receipt_is_bound(
    receipt: &ValidationReceipt,
    expected: Option<&str>,
    verifier: Option<&ReceiptVerifier>,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> bool {
    let structurally_bound = receipt.binding == ReceiptBinding::HostBound
        && receipt.state == EvidenceState::Observed
        && receipt.unsupported.is_empty()
        && expected == Some(receipt.source_revision.as_str())
        && !receipt.tool_revision.trim().is_empty()
        && !receipt.scope.trim().is_empty()
        && digest(&receipt.invocation_digest)
        && digest(&receipt.output_digest);
    if !structurally_bound {
        return false;
    }
    let Some((verifier, attestation, source_revision)) = verifier
        .zip(receipt.attestation.clone())
        .zip(expected)
        .map(|((verifier, attestation), source)| (verifier, attestation, source))
    else {
        return false;
    };
    let Ok(subject) = SubjectRevision::checked(source_revision) else {
        return false;
    };
    let Ok(scope) = ReceiptScope::checked(validation_receipt_scope(receipt)) else {
        return false;
    };
    verifier
        .verify(
            attestation,
            &ReceiptPolicy::exact(
                ReceiptClass::Validation,
                subject,
                scope,
                validation_receipt_payload_digest(receipt),
                now_epoch,
            ),
            replay,
        )
        .is_ok()
}

pub fn validation_receipt_scope(receipt: &ValidationReceipt) -> String {
    format!("validation/{:?}/{}", receipt.kind, receipt.scope).to_ascii_lowercase()
}

pub fn validation_receipt_payload_digest(receipt: &ValidationReceipt) -> String {
    stable_sha256(&format!(
        "{:?}\0{:?}\0{:?}\0{}\0{}\0{}\0{}\0{}\0{}",
        receipt.kind,
        receipt.binding,
        receipt.state,
        receipt.source_revision,
        receipt.tool_revision,
        receipt.invocation_digest,
        receipt.output_digest,
        receipt.scope,
        receipt.unsupported.join("\u{1f}")
    ))
}

fn digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn duplicate_kinds(receipts: &[ValidationReceipt]) -> BTreeSet<ValidationKind> {
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for receipt in receipts {
        if !seen.insert(receipt.kind) {
            duplicates.insert(receipt.kind);
        }
    }
    duplicates
}
