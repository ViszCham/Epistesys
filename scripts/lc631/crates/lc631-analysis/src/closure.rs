use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
    UntrustedReceipt,
};
use lc631_tldg::{Dg1Report, Dg1Status, DG1_SCHEMA};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const CLOSURE_SCHEMA: &str = "epistesys-coding-closure.v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingEvidenceKind {
    ImplementationBinding,
    ProductionConnection,
    StaticValidation,
    RuntimeValidation,
    AcceptanceTest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingEvidenceProducer {
    StructuralObservation,
    DeclaredConnectionObserver,
    CompilerObserved,
    RuntimeObserved,
    AcceptanceOracleObserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingEvidenceState {
    Unavailable,
    Rejected,
    VerifiedReceipt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingGapKind {
    ImplementationBinding,
    ProductionConnection,
    StaticValidation,
    RuntimeValidation,
    AcceptanceTest,
    UnsupportedInput,
    EmptyRequirementSet,
    ParserSyntaxErrors,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingClosureStatus {
    Held,
    EvidenceCompleteForDeclaredRequirements,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CodingEvidenceClaim {
    pub requirement_id: u32,
    #[serde(default)]
    pub coding_requirement_id: Option<String>,
    #[serde(default)]
    pub coding_task_id: Option<String>,
    pub kind: CodingEvidenceKind,
    pub source_revision: String,
    pub target: String,
    pub target_digest: String,
    #[serde(default)]
    pub producer: Option<CodingEvidenceProducer>,
    #[serde(default)]
    pub producer_run_digest: Option<String>,
    #[serde(default)]
    pub observation_digest: Option<String>,
    #[serde(default)]
    pub lifecycle_lease: Option<crate::DgclEvidenceLease>,
    pub attestation: Option<UntrustedReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CodingEvidenceRejection {
    pub claim_index: usize,
    pub requirement_id: u32,
    pub kind: CodingEvidenceKind,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CodingGap {
    pub gap_id: String,
    pub requirement_id: Option<u32>,
    pub kind: CodingGapKind,
    pub state: CodingEvidenceState,
    pub target_binding: Option<String>,
    pub evidence_digest: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CodingClosureReport {
    pub schema_version: &'static str,
    pub source_revision: String,
    pub requirement_count: usize,
    pub residual_span_count: usize,
    pub gaps: Vec<CodingGap>,
    pub rejected_claim_count: usize,
    pub rejected_claims: Vec<CodingEvidenceRejection>,
    pub duplicate_claim_count: usize,
    pub status: CodingClosureStatus,
    pub authority_created: bool,
    pub output_commit_allowed: bool,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum CodingClosureError {
    SourceRevisionMismatch,
    InvalidRequirementSpan,
    DuplicateRequirementId,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EvidenceKey {
    requirement_id: u32,
    kind: CodingEvidenceKind,
}

pub fn coding_evidence_scope(claim: &CodingEvidenceClaim) -> String {
    format!(
        "dgcl/evidence/{}/{}/{}/{}/{}/{}",
        claim.requirement_id,
        evidence_kind_name(claim.kind),
        claim
            .producer
            .map(evidence_producer_name)
            .unwrap_or("unbound"),
        claim.coding_requirement_id.as_deref().unwrap_or("legacy"),
        claim.coding_task_id.as_deref().unwrap_or("legacy"),
        claim
            .lifecycle_lease
            .as_ref()
            .map(|lease| lease.lease_digest.as_str())
            .unwrap_or("unbound-lifecycle")
    )
}

pub fn coding_evidence_payload_digest(claim: &CodingEvidenceClaim) -> String {
    stable_sha256(&format!(
        "{}\0{}\0{:?}\0{}\0{}\0{:?}\0{}\0{}\0{}\0{}\0{}",
        claim.requirement_id,
        claim.source_revision,
        claim.kind,
        claim.target,
        claim.target_digest,
        claim.producer,
        claim
            .producer_run_digest
            .as_deref()
            .unwrap_or("missing-run"),
        claim
            .observation_digest
            .as_deref()
            .unwrap_or("missing-observation"),
        claim
            .coding_requirement_id
            .as_deref()
            .unwrap_or("legacy-requirement"),
        claim.coding_task_id.as_deref().unwrap_or("legacy-task"),
        claim
            .lifecycle_lease
            .as_ref()
            .map(|lease| lease.lease_digest.as_str())
            .unwrap_or("unbound-lifecycle")
    ))
}

pub fn build_coding_closure(
    source: &str,
    dg1: &Dg1Report,
    claims: &[CodingEvidenceClaim],
    verifier: Option<&ReceiptVerifier>,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> Result<CodingClosureReport, CodingClosureError> {
    let source_revision = stable_sha256(source);
    if dg1.source_revision != source_revision
        || dg1.schema_version != DG1_SCHEMA
        || !dg1.exact_source_roundtrip
    {
        return Err(CodingClosureError::SourceRevisionMismatch);
    }
    let mut requirement_ids = BTreeSet::new();
    for requirement in &dg1.requirement_candidates {
        if !requirement_ids.insert(requirement.id) {
            return Err(CodingClosureError::DuplicateRequirementId);
        }
        if source.get(requirement.source_span.clone()) != Some(requirement.source_text.as_str()) {
            return Err(CodingClosureError::InvalidRequirementSpan);
        }
    }

    let mut claim_counts = BTreeMap::<EvidenceKey, usize>::new();
    for claim in claims {
        *claim_counts
            .entry(EvidenceKey {
                requirement_id: claim.requirement_id,
                kind: claim.kind,
            })
            .or_default() += 1;
    }
    let duplicate_claim_count = claim_counts.values().filter(|count| **count > 1).count();
    let mut accepted = BTreeMap::<EvidenceKey, (String, String)>::new();
    let mut rejected_keys = BTreeSet::<EvidenceKey>::new();
    let mut rejected_claims = Vec::new();
    let mut rejected_claim_count = 0usize;
    for (claim_index, claim) in claims.iter().enumerate() {
        let key = EvidenceKey {
            requirement_id: claim.requirement_id,
            kind: claim.kind,
        };
        let rejection = if claim_counts.get(&key) != Some(&1) {
            Some("duplicate_claim_for_requirement_evidence_kind")
        } else if !requirement_ids.contains(&claim.requirement_id) {
            Some("unknown_requirement_id")
        } else if claim.coding_requirement_id.is_some() || claim.coding_task_id.is_some() {
            Some("action_scoped_claim_requires_dgcl_closure")
        } else if claim.producer != Some(expected_producer(claim.kind)) {
            Some("evidence_producer_policy_mismatch")
        } else if !claim
            .producer_run_digest
            .as_deref()
            .is_some_and(valid_digest)
            || !claim
                .observation_digest
                .as_deref()
                .is_some_and(valid_digest)
        {
            Some("producer_observation_binding_missing_or_invalid")
        } else if claim.source_revision != source_revision {
            Some("source_revision_mismatch")
        } else if !valid_target(&claim.target) {
            Some("target_invalid_or_outside_relative_scope")
        } else if !valid_digest(&claim.target_digest) {
            Some("target_digest_invalid")
        } else {
            None
        };
        if let Some(reason) = rejection {
            rejected_claim_count += 1;
            rejected_claims.push(CodingEvidenceRejection {
                claim_index,
                requirement_id: claim.requirement_id,
                kind: claim.kind,
                reason,
            });
            if requirement_ids.contains(&claim.requirement_id) {
                rejected_keys.insert(key);
            }
            continue;
        }
        let (Some(verifier), Some(attestation)) = (verifier, claim.attestation.clone()) else {
            rejected_claim_count += 1;
            rejected_keys.insert(key);
            rejected_claims.push(CodingEvidenceRejection {
                claim_index,
                requirement_id: claim.requirement_id,
                kind: claim.kind,
                reason: "receipt_or_verifier_unavailable",
            });
            continue;
        };
        let Ok(subject) = SubjectRevision::checked(source_revision.clone()) else {
            rejected_claim_count += 1;
            rejected_keys.insert(key);
            rejected_claims.push(CodingEvidenceRejection {
                claim_index,
                requirement_id: claim.requirement_id,
                kind: claim.kind,
                reason: "source_subject_invalid",
            });
            continue;
        };
        let Ok(scope) = ReceiptScope::checked(coding_evidence_scope(claim)) else {
            rejected_claim_count += 1;
            rejected_keys.insert(key);
            rejected_claims.push(CodingEvidenceRejection {
                claim_index,
                requirement_id: claim.requirement_id,
                kind: claim.kind,
                reason: "receipt_scope_invalid",
            });
            continue;
        };
        let verified = verifier.verify(
            attestation,
            &ReceiptPolicy::exact(
                ReceiptClass::Closure,
                subject,
                scope,
                coding_evidence_payload_digest(claim),
                now_epoch,
            ),
            replay,
        );
        if let Ok(verified) = verified {
            accepted.insert(key, (verified.receipt_digest(), claim.target.clone()));
        } else {
            rejected_claim_count += 1;
            rejected_keys.insert(key);
            rejected_claims.push(CodingEvidenceRejection {
                claim_index,
                requirement_id: claim.requirement_id,
                kind: claim.kind,
                reason: "receipt_rejected_or_replayed",
            });
        }
    }

    let mut gaps = Vec::new();
    if dg1.requirement_candidates.is_empty() {
        gaps.push(CodingGap {
            gap_id: "dgcl:empty-requirement-set".into(),
            requirement_id: None,
            kind: CodingGapKind::EmptyRequirementSet,
            state: CodingEvidenceState::Unavailable,
            target_binding: None,
            evidence_digest: None,
            reason: "no supported requirement candidate; explicit no-op contract is not supplied"
                .into(),
        });
    }
    if dg1.status == Dg1Status::ParsedWithSyntaxErrors {
        gaps.push(CodingGap {
            gap_id: "dgcl:parser-syntax-errors".into(),
            requirement_id: None,
            kind: CodingGapKind::ParserSyntaxErrors,
            state: CodingEvidenceState::Unavailable,
            target_binding: None,
            evidence_digest: None,
            reason: "one or more pinned Rust CST regions contain error or missing nodes".into(),
        });
    }
    for residual in &dg1.instruction_residuals {
        if source.get(residual.source_span.clone()) != Some(residual.source_text.as_str()) {
            return Err(CodingClosureError::InvalidRequirementSpan);
        }
        gaps.push(CodingGap {
            gap_id: format!(
                "dgcl:unsupported-input:{}-{}",
                residual.source_span.start, residual.source_span.end
            ),
            requirement_id: None,
            kind: CodingGapKind::UnsupportedInput,
            state: CodingEvidenceState::Unavailable,
            target_binding: None,
            evidence_digest: None,
            reason: format!("unresolved_input:{:?}", residual.reason),
        });
    }
    for requirement in &dg1.requirement_candidates {
        for kind in [
            CodingEvidenceKind::ImplementationBinding,
            CodingEvidenceKind::ProductionConnection,
            CodingEvidenceKind::StaticValidation,
            CodingEvidenceKind::RuntimeValidation,
            CodingEvidenceKind::AcceptanceTest,
        ] {
            let key = EvidenceKey {
                requirement_id: requirement.id,
                kind,
            };
            let evidence_digest = accepted.get(&key).map(|(digest, _)| digest.clone());
            let target_binding = accepted.get(&key).map(|(_, target)| target.clone());
            gaps.push(CodingGap {
                gap_id: format!("dgcl:req:{}:{}", requirement.id, evidence_kind_name(kind)),
                requirement_id: Some(requirement.id),
                kind: gap_kind(kind),
                state: if evidence_digest.is_some() {
                    CodingEvidenceState::VerifiedReceipt
                } else if rejected_keys.contains(&key) {
                    CodingEvidenceState::Rejected
                } else {
                    CodingEvidenceState::Unavailable
                },
                target_binding,
                evidence_digest,
                reason: if accepted.contains_key(&key) {
                    "receipt verified for exact source revision, scope, and target digest; semantic correctness remains unproven".into()
                } else {
                    "missing or rejected per-requirement evidence".into()
                },
            });
        }
    }
    gaps.sort_by(|left, right| left.gap_id.cmp(&right.gap_id));
    let complete = gaps
        .iter()
        .all(|gap| gap.state == CodingEvidenceState::VerifiedReceipt);
    Ok(CodingClosureReport {
        schema_version: CLOSURE_SCHEMA,
        source_revision,
        requirement_count: dg1.requirement_candidates.len(),
        residual_span_count: dg1.instruction_residuals.len(),
        gaps,
        rejected_claim_count,
        rejected_claims,
        duplicate_claim_count,
        status: if complete {
            CodingClosureStatus::EvidenceCompleteForDeclaredRequirements
        } else {
            CodingClosureStatus::Held
        },
        authority_created: false,
        output_commit_allowed: false,
        claim_boundary: "per-requirement producer/run-bound evidence receipts checked against a caller-supplied verifier; verifier-root provenance remains external; not semantic correctness, repository-wide closure, authority, or output permission",
    })
}

fn gap_kind(kind: CodingEvidenceKind) -> CodingGapKind {
    match kind {
        CodingEvidenceKind::ImplementationBinding => CodingGapKind::ImplementationBinding,
        CodingEvidenceKind::ProductionConnection => CodingGapKind::ProductionConnection,
        CodingEvidenceKind::StaticValidation => CodingGapKind::StaticValidation,
        CodingEvidenceKind::RuntimeValidation => CodingGapKind::RuntimeValidation,
        CodingEvidenceKind::AcceptanceTest => CodingGapKind::AcceptanceTest,
    }
}

fn evidence_kind_name(kind: CodingEvidenceKind) -> &'static str {
    match kind {
        CodingEvidenceKind::ImplementationBinding => "implementation_binding",
        CodingEvidenceKind::ProductionConnection => "production_connection",
        CodingEvidenceKind::StaticValidation => "static_validation",
        CodingEvidenceKind::RuntimeValidation => "runtime_validation",
        CodingEvidenceKind::AcceptanceTest => "acceptance_test",
    }
}

pub(crate) fn expected_producer(kind: CodingEvidenceKind) -> CodingEvidenceProducer {
    match kind {
        CodingEvidenceKind::ImplementationBinding => CodingEvidenceProducer::StructuralObservation,
        CodingEvidenceKind::ProductionConnection => {
            CodingEvidenceProducer::DeclaredConnectionObserver
        }
        CodingEvidenceKind::StaticValidation => CodingEvidenceProducer::CompilerObserved,
        CodingEvidenceKind::RuntimeValidation => CodingEvidenceProducer::RuntimeObserved,
        CodingEvidenceKind::AcceptanceTest => CodingEvidenceProducer::AcceptanceOracleObserved,
    }
}

fn evidence_producer_name(producer: CodingEvidenceProducer) -> &'static str {
    match producer {
        CodingEvidenceProducer::StructuralObservation => "structural_observation",
        CodingEvidenceProducer::DeclaredConnectionObserver => "declared_connection_observer",
        CodingEvidenceProducer::CompilerObserved => "compiler_observed",
        CodingEvidenceProducer::RuntimeObserved => "runtime_observed",
        CodingEvidenceProducer::AcceptanceOracleObserved => "acceptance_oracle_observed",
    }
}

fn valid_target(target: &str) -> bool {
    if target.is_empty()
        || target.len() > 512
        || target.chars().any(char::is_control)
        || target.starts_with('/')
        || target.starts_with('\\')
        || target.contains(':')
    {
        return false;
    }
    !target
        .split(['/', '\\'])
        .any(|segment| segment.is_empty() || segment == ".." || segment == ".")
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lc631_tldg::{analyze_dg1, Dg1Budget};

    fn evidence(
        issuer: &lc631_receipt_kernel::ReceiptIssuer,
        source: &str,
        requirement_id: u32,
        kind: CodingEvidenceKind,
        now: u64,
    ) -> CodingEvidenceClaim {
        let mut claim = CodingEvidenceClaim {
            requirement_id,
            coding_requirement_id: None,
            coding_task_id: None,
            kind,
            source_revision: stable_sha256(source),
            target: match kind {
                CodingEvidenceKind::ImplementationBinding => "src/lib.rs",
                CodingEvidenceKind::ProductionConnection => "src/main.rs",
                CodingEvidenceKind::StaticValidation => "tests/static.rs",
                CodingEvidenceKind::RuntimeValidation => "tests/runtime.rs",
                CodingEvidenceKind::AcceptanceTest => "tests/acceptance.rs",
            }
            .into(),
            target_digest: stable_sha256(&format!("{kind:?}:{requirement_id}")),
            producer: Some(expected_producer(kind)),
            producer_run_digest: Some(stable_sha256(&format!(
                "test-run.v1\0{kind:?}\0{requirement_id}"
            ))),
            observation_digest: Some(stable_sha256(&format!(
                "test-observation.v1\0{kind:?}\0{requirement_id}"
            ))),
            lifecycle_lease: None,
            attestation: None,
        };
        let attestation = issuer
            .issue(
                ReceiptClass::Closure,
                SubjectRevision::checked(&claim.source_revision).unwrap(),
                ReceiptScope::checked(coding_evidence_scope(&claim)).unwrap(),
                coding_evidence_payload_digest(&claim),
                now,
                None,
                None,
            )
            .unwrap();
        claim.attestation = Some(attestation);
        claim
    }

    #[test]
    fn one_missing_acceptance_receipt_keeps_only_that_requirement_gap_open() {
        let key = [91_u8; 32];
        let issuer =
            lc631_receipt_kernel::ReceiptIssuer::from_key_bytes("dgcl-test", &key).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("dgcl-test", &key).unwrap();
        let source = "Please update the implementation.";
        let dg1 = analyze_dg1(source, Dg1Budget::default()).unwrap();
        let claims = [
            CodingEvidenceKind::ImplementationBinding,
            CodingEvidenceKind::ProductionConnection,
            CodingEvidenceKind::StaticValidation,
            CodingEvidenceKind::RuntimeValidation,
        ]
        .into_iter()
        .map(|kind| evidence(&issuer, source, 1, kind, 10))
        .collect::<Vec<_>>();
        let report = build_coding_closure(
            source,
            &dg1,
            &claims,
            Some(&verifier),
            &mut ReplayGuard::default(),
            10,
        )
        .unwrap();
        assert_eq!(report.status, CodingClosureStatus::Held);
        assert_eq!(
            report
                .gaps
                .iter()
                .filter(|gap| gap.state == CodingEvidenceState::Unavailable)
                .count(),
            1
        );
        assert_eq!(
            report
                .gaps
                .iter()
                .find(|gap| gap.state == CodingEvidenceState::Unavailable)
                .unwrap()
                .kind,
            CodingGapKind::AcceptanceTest
        );
        assert!(!report.authority_created);
        assert!(!report.output_commit_allowed);
    }

    #[test]
    fn explicitly_trusted_verifier_accepts_exact_receipts_only_for_declared_gaps() {
        let key = [94_u8; 32];
        let issuer =
            lc631_receipt_kernel::ReceiptIssuer::from_key_bytes("dgcl-test", &key).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("dgcl-test", &key).unwrap();
        let source = "Please update the implementation.";
        let dg1 = analyze_dg1(source, Dg1Budget::default()).unwrap();
        let claims = [
            CodingEvidenceKind::ImplementationBinding,
            CodingEvidenceKind::ProductionConnection,
            CodingEvidenceKind::StaticValidation,
            CodingEvidenceKind::RuntimeValidation,
            CodingEvidenceKind::AcceptanceTest,
        ]
        .into_iter()
        .map(|kind| evidence(&issuer, source, 1, kind, 10))
        .collect::<Vec<_>>();
        let report = build_coding_closure(
            source,
            &dg1,
            &claims,
            Some(&verifier),
            &mut ReplayGuard::default(),
            10,
        )
        .unwrap();
        assert_eq!(
            report.status,
            CodingClosureStatus::EvidenceCompleteForDeclaredRequirements
        );
        assert!(report
            .gaps
            .iter()
            .all(|gap| gap.state == CodingEvidenceState::VerifiedReceipt));
        assert!(report.gaps.iter().all(|gap| gap.target_binding.is_some()));
        assert!(!report.authority_created);
        assert!(!report.output_commit_allowed);
    }

    #[test]
    fn source_edit_invalidates_all_prior_requirement_evidence() {
        let key = [92_u8; 32];
        let issuer =
            lc631_receipt_kernel::ReceiptIssuer::from_key_bytes("dgcl-test", &key).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("dgcl-test", &key).unwrap();
        let source = "Please update the implementation.";
        let dg1 = analyze_dg1(source, Dg1Budget::default()).unwrap();
        let claims = [CodingEvidenceKind::ImplementationBinding]
            .into_iter()
            .map(|kind| evidence(&issuer, source, 1, kind, 10))
            .collect::<Vec<_>>();
        let changed = "Please carefully update the implementation.";
        assert_eq!(
            build_coding_closure(
                changed,
                &dg1,
                &claims,
                Some(&verifier),
                &mut ReplayGuard::default(),
                10
            ),
            Err(CodingClosureError::SourceRevisionMismatch)
        );
    }

    #[test]
    fn target_or_digest_tamper_after_signing_leaves_the_individual_gap_rejected() {
        let key = [95_u8; 32];
        let issuer =
            lc631_receipt_kernel::ReceiptIssuer::from_key_bytes("dgcl-test", &key).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("dgcl-test", &key).unwrap();
        let source = "Please update the implementation.";
        let dg1 = analyze_dg1(source, Dg1Budget::default()).unwrap();
        let mut claim = evidence(
            &issuer,
            source,
            1,
            CodingEvidenceKind::ImplementationBinding,
            10,
        );
        claim.target_digest = stable_sha256("tampered target content");
        let report = build_coding_closure(
            source,
            &dg1,
            &[claim],
            Some(&verifier),
            &mut ReplayGuard::default(),
            10,
        )
        .unwrap();
        assert!(report.gaps.iter().any(|gap| {
            gap.kind == CodingGapKind::ImplementationBinding
                && gap.state == CodingEvidenceState::Rejected
                && gap.target_binding.is_none()
        }));
        assert_eq!(report.rejected_claim_count, 1);
        assert_eq!(
            report.rejected_claims[0].reason,
            "receipt_rejected_or_replayed"
        );
    }

    #[test]
    fn wrong_receipt_class_cannot_substitute_for_closure_evidence() {
        let key = [102_u8; 32];
        let issuer =
            lc631_receipt_kernel::ReceiptIssuer::from_key_bytes("dgcl-test", &key).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("dgcl-test", &key).unwrap();
        let source = "Please update the implementation.";
        let dg1 = analyze_dg1(source, Dg1Budget::default()).unwrap();
        let mut claim = evidence(
            &issuer,
            source,
            1,
            CodingEvidenceKind::ImplementationBinding,
            10,
        );
        claim.attestation = Some(
            issuer
                .issue(
                    ReceiptClass::ToolExecution,
                    SubjectRevision::checked(&claim.source_revision).unwrap(),
                    ReceiptScope::checked(coding_evidence_scope(&claim)).unwrap(),
                    coding_evidence_payload_digest(&claim),
                    10,
                    None,
                    None,
                )
                .unwrap(),
        );
        let report = build_coding_closure(
            source,
            &dg1,
            &[claim],
            Some(&verifier),
            &mut ReplayGuard::default(),
            10,
        )
        .unwrap();
        assert!(report
            .rejected_claims
            .iter()
            .any(|claim| { claim.reason == "receipt_rejected_or_replayed" }));

        let mut wrong_scope = evidence(
            &issuer,
            source,
            1,
            CodingEvidenceKind::ImplementationBinding,
            10,
        );
        wrong_scope.attestation = Some(
            issuer
                .issue(
                    ReceiptClass::Closure,
                    SubjectRevision::checked(&wrong_scope.source_revision).unwrap(),
                    ReceiptScope::checked("dgcl/evidence/wrong-scope").unwrap(),
                    coding_evidence_payload_digest(&wrong_scope),
                    10,
                    None,
                    None,
                )
                .unwrap(),
        );
        let report = build_coding_closure(
            source,
            &dg1,
            &[wrong_scope],
            Some(&verifier),
            &mut ReplayGuard::default(),
            10,
        )
        .unwrap();
        assert!(report
            .rejected_claims
            .iter()
            .any(|claim| { claim.reason == "receipt_rejected_or_replayed" }));
    }

    #[test]
    fn producer_kind_run_and_observation_are_bound_to_the_receipt_payload() {
        let key = [96_u8; 32];
        let issuer =
            lc631_receipt_kernel::ReceiptIssuer::from_key_bytes("dgcl-test", &key).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("dgcl-test", &key).unwrap();
        let source = "Please update the implementation.";
        let dg1 = analyze_dg1(source, Dg1Budget::default()).unwrap();

        let mut wrong_producer = evidence(
            &issuer,
            source,
            1,
            CodingEvidenceKind::ImplementationBinding,
            10,
        );
        wrong_producer.producer = Some(CodingEvidenceProducer::AcceptanceOracleObserved);
        let report = build_coding_closure(
            source,
            &dg1,
            &[wrong_producer],
            Some(&verifier),
            &mut ReplayGuard::default(),
            10,
        )
        .unwrap();
        assert!(report
            .rejected_claims
            .iter()
            .any(|claim| { claim.reason == "evidence_producer_policy_mismatch" }));

        let mut wrong_signed_kind = evidence(
            &issuer,
            source,
            1,
            CodingEvidenceKind::ImplementationBinding,
            10,
        );
        wrong_signed_kind.kind = CodingEvidenceKind::ProductionConnection;
        wrong_signed_kind.producer = Some(CodingEvidenceProducer::DeclaredConnectionObserver);
        let report = build_coding_closure(
            source,
            &dg1,
            &[wrong_signed_kind],
            Some(&verifier),
            &mut ReplayGuard::default(),
            10,
        )
        .unwrap();
        assert!(report
            .rejected_claims
            .iter()
            .any(|claim| { claim.reason == "receipt_rejected_or_replayed" }));

        let mut wrong_run = evidence(
            &issuer,
            source,
            1,
            CodingEvidenceKind::ImplementationBinding,
            10,
        );
        wrong_run.producer_run_digest = Some(stable_sha256("another-run"));
        let report = build_coding_closure(
            source,
            &dg1,
            &[wrong_run],
            Some(&verifier),
            &mut ReplayGuard::default(),
            10,
        )
        .unwrap();
        assert!(report
            .rejected_claims
            .iter()
            .any(|claim| { claim.reason == "receipt_rejected_or_replayed" }));
    }

    #[test]
    fn self_issued_receipt_without_the_callers_trusted_verifier_is_not_evidence() {
        let key = [97_u8; 32];
        let issuer = lc631_receipt_kernel::ReceiptIssuer::from_key_bytes("self", &key).unwrap();
        let source = "Please update the implementation.";
        let dg1 = analyze_dg1(source, Dg1Budget::default()).unwrap();
        let claim = evidence(
            &issuer,
            source,
            1,
            CodingEvidenceKind::ImplementationBinding,
            10,
        );
        let report = build_coding_closure(
            source,
            &dg1,
            &[claim],
            None,
            &mut ReplayGuard::default(),
            10,
        )
        .unwrap();
        assert_eq!(report.status, CodingClosureStatus::Held);
        assert!(report.gaps.iter().any(|gap| {
            gap.kind == CodingGapKind::ImplementationBinding
                && gap.state == CodingEvidenceState::Rejected
        }));
    }

    #[test]
    fn trusted_verifier_rejects_a_receipt_signed_by_another_issuer() {
        let untrusted_issuer =
            lc631_receipt_kernel::ReceiptIssuer::from_key_bytes("untrusted", &[98_u8; 32]).unwrap();
        let trusted_verifier = ReceiptVerifier::from_key_bytes("trusted", &[99_u8; 32]).unwrap();
        let source = "Please update the implementation.";
        let dg1 = analyze_dg1(source, Dg1Budget::default()).unwrap();
        let claim = evidence(
            &untrusted_issuer,
            source,
            1,
            CodingEvidenceKind::ImplementationBinding,
            10,
        );
        let report = build_coding_closure(
            source,
            &dg1,
            &[claim],
            Some(&trusted_verifier),
            &mut ReplayGuard::default(),
            10,
        )
        .unwrap();
        assert!(report
            .rejected_claims
            .iter()
            .any(|claim| { claim.reason == "receipt_rejected_or_replayed" }));
    }

    #[test]
    fn replayed_producer_receipt_cannot_close_a_second_run() {
        let key = [100_u8; 32];
        let issuer =
            lc631_receipt_kernel::ReceiptIssuer::from_key_bytes("dgcl-test", &key).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("dgcl-test", &key).unwrap();
        let source = "Please update the implementation.";
        let dg1 = analyze_dg1(source, Dg1Budget::default()).unwrap();
        let claim = evidence(
            &issuer,
            source,
            1,
            CodingEvidenceKind::ImplementationBinding,
            10,
        );
        let mut replay = ReplayGuard::default();
        let first = build_coding_closure(
            source,
            &dg1,
            std::slice::from_ref(&claim),
            Some(&verifier),
            &mut replay,
            10,
        )
        .unwrap();
        assert!(first.gaps.iter().any(|gap| {
            gap.kind == CodingGapKind::ImplementationBinding
                && gap.state == CodingEvidenceState::VerifiedReceipt
        }));
        let second =
            build_coding_closure(source, &dg1, &[claim], Some(&verifier), &mut replay, 10).unwrap();
        assert!(second.gaps.iter().any(|gap| {
            gap.kind == CodingGapKind::ImplementationBinding
                && gap.state == CodingEvidenceState::Rejected
        }));
    }

    #[test]
    fn expired_producer_receipt_remains_a_rejected_gap() {
        let key = [101_u8; 32];
        let issuer =
            lc631_receipt_kernel::ReceiptIssuer::from_key_bytes("dgcl-test", &key).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("dgcl-test", &key).unwrap();
        let source = "Please update the implementation.";
        let dg1 = analyze_dg1(source, Dg1Budget::default()).unwrap();
        let mut claim = evidence(
            &issuer,
            source,
            1,
            CodingEvidenceKind::ImplementationBinding,
            10,
        );
        claim.attestation = Some(
            issuer
                .issue(
                    ReceiptClass::Closure,
                    SubjectRevision::checked(&claim.source_revision).unwrap(),
                    ReceiptScope::checked(coding_evidence_scope(&claim)).unwrap(),
                    coding_evidence_payload_digest(&claim),
                    10,
                    Some(11),
                    None,
                )
                .unwrap(),
        );
        let report = build_coding_closure(
            source,
            &dg1,
            &[claim],
            Some(&verifier),
            &mut ReplayGuard::default(),
            12,
        )
        .unwrap();
        assert!(report.gaps.iter().any(|gap| {
            gap.kind == CodingGapKind::ImplementationBinding
                && gap.state == CodingEvidenceState::Rejected
        }));
    }

    #[test]
    fn duplicate_claims_cannot_close_a_gap_by_first_match_wins() {
        let key = [93_u8; 32];
        let issuer =
            lc631_receipt_kernel::ReceiptIssuer::from_key_bytes("dgcl-test", &key).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("dgcl-test", &key).unwrap();
        let source = "Please update the implementation.";
        let dg1 = analyze_dg1(source, Dg1Budget::default()).unwrap();
        let claim = evidence(
            &issuer,
            source,
            1,
            CodingEvidenceKind::ImplementationBinding,
            10,
        );
        let report = build_coding_closure(
            source,
            &dg1,
            &[claim.clone(), claim],
            Some(&verifier),
            &mut ReplayGuard::default(),
            10,
        )
        .unwrap();
        assert_eq!(report.duplicate_claim_count, 1);
        assert_eq!(report.rejected_claim_count, 2);
        assert!(report
            .gaps
            .iter()
            .any(|gap| gap.kind == CodingGapKind::ImplementationBinding
                && gap.state == CodingEvidenceState::Rejected));
        assert!(report
            .rejected_claims
            .iter()
            .all(|claim| claim.reason == "duplicate_claim_for_requirement_evidence_kind"));
    }

    #[test]
    fn unsupported_prose_and_empty_requirements_are_blocking_gaps() {
        let source = "A description with no supported directive.";
        let dg1 = analyze_dg1(source, Dg1Budget::default()).unwrap();
        let report =
            build_coding_closure(source, &dg1, &[], None, &mut ReplayGuard::default(), 10).unwrap();
        assert_eq!(report.status, CodingClosureStatus::Held);
        assert!(report
            .gaps
            .iter()
            .any(|gap| gap.kind == CodingGapKind::EmptyRequirementSet));
        assert!(report
            .gaps
            .iter()
            .any(|gap| gap.kind == CodingGapKind::UnsupportedInput));
    }

    #[test]
    fn rust_cst_errors_are_a_separate_closure_gap() {
        let source = "Please repair the function.\n```rust\nfn f( {\n```";
        let dg1 = analyze_dg1(source, Dg1Budget::default()).unwrap();
        assert_eq!(dg1.status, Dg1Status::ParsedWithSyntaxErrors);
        let report =
            build_coding_closure(source, &dg1, &[], None, &mut ReplayGuard::default(), 10).unwrap();
        assert!(report
            .gaps
            .iter()
            .any(|gap| gap.kind == CodingGapKind::ParserSyntaxErrors));
        assert_eq!(report.status, CodingClosureStatus::Held);
    }
}
