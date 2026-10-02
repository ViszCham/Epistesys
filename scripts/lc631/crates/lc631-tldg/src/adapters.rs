use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard,
    SubjectRevision, UntrustedReceipt,
};
use serde::Serialize;
use std::collections::BTreeMap;

pub const UNIFIED_SYNTAX_SCHEMA: &str = "lc631-unified-syntax-hypergraph.v1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendFamily {
    BuiltinLossless,
    BuiltinOpenDiscourse,
    BuiltinExecutableSymbolic,
    EnglishHpsg,
    JapaneseHpsg,
    EnhancedUd,
    Ucca,
    Amr,
    PropBank,
    TimeMl,
    GfRgl,
    TreeSitter,
    RustSyntaxHir,
    TypeScriptCompiler,
    CppCompiler,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendState {
    Unavailable,
    Declared,
    RevisionPinned,
    ExecutedCandidate,
    ValidatedObservation,
    RejectedExecution,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BackendDescriptor {
    pub family: BackendFamily,
    pub name: String,
    pub revision: String,
    pub state: BackendState,
    pub target_schema: &'static str,
    pub evidence_digest: Option<String>,
    pub execution_receipt_digest: Option<String>,
    pub attestation_verified: bool,
    pub claim_boundary: String,
}

impl BackendDescriptor {
    pub const fn can_authorize(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct BackendRegistry {
    entries: BTreeMap<BackendFamily, BackendDescriptor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BackendExecutionReceipt {
    pub family: BackendFamily,
    pub source_revision: String,
    pub backend_revision: String,
    pub output_digest: String,
    pub candidate_executed: bool,
    pub attestation: Option<UntrustedReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BackendExecutionOutcome {
    pub receipt_index: usize,
    pub family: BackendFamily,
    pub accepted: bool,
    pub reason: &'static str,
    pub receipt_digest: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct BackendExecutionValidationReport {
    pub accepted_count: usize,
    pub rejected_count: usize,
    pub outcomes: Vec<BackendExecutionOutcome>,
}

impl BackendRegistry {
    pub fn insert(&mut self, descriptor: BackendDescriptor) -> Result<(), BackendFamily> {
        if self.entries.contains_key(&descriptor.family) {
            return Err(descriptor.family);
        }
        self.entries.insert(descriptor.family, descriptor);
        Ok(())
    }

    pub fn get(&self, family: BackendFamily) -> Option<&BackendDescriptor> {
        self.entries.get(&family)
    }

    pub fn iter(&self) -> impl Iterator<Item = &BackendDescriptor> {
        self.entries.values()
    }

    pub fn validate_executions(
        &mut self,
        receipts: &[BackendExecutionReceipt],
        source_revision: &str,
        verifier: &ReceiptVerifier,
        replay: &mut ReplayGuard,
        now_epoch: u64,
    ) -> usize {
        self.validate_executions_detailed(receipts, source_revision, verifier, replay, now_epoch)
            .accepted_count
    }

    pub fn validate_executions_detailed(
        &mut self,
        receipts: &[BackendExecutionReceipt],
        source_revision: &str,
        verifier: &ReceiptVerifier,
        replay: &mut ReplayGuard,
        now_epoch: u64,
    ) -> BackendExecutionValidationReport {
        let mut family_counts = BTreeMap::<BackendFamily, usize>::new();
        for receipt in receipts {
            *family_counts.entry(receipt.family).or_default() += 1;
        }
        let mut report = BackendExecutionValidationReport::default();
        for (receipt_index, receipt) in receipts.iter().enumerate() {
            let Some(descriptor) = self.entries.get_mut(&receipt.family) else {
                report.rejected_count += 1;
                report.outcomes.push(BackendExecutionOutcome {
                    receipt_index,
                    family: receipt.family,
                    accepted: false,
                    reason: "backend_not_registered",
                    receipt_digest: None,
                });
                continue;
            };
            clear_execution_observation(descriptor);
            let reason = if family_counts
                .get(&receipt.family)
                .copied()
                .unwrap_or_default()
                > 1
            {
                Some("duplicate_backend_receipts_in_batch")
            } else if receipt.source_revision != source_revision {
                Some("source_revision_mismatch")
            } else if receipt.backend_revision != descriptor.revision {
                Some("backend_revision_mismatch")
            } else if !receipt.candidate_executed {
                Some("execution_not_observed")
            } else if !valid_digest(&receipt.output_digest) {
                Some("output_digest_invalid")
            } else if receipt.attestation.is_none() {
                Some("attestation_missing")
            } else {
                None
            };
            if let Some(reason) = reason {
                descriptor.state = BackendState::RejectedExecution;
                report.rejected_count += 1;
                report.outcomes.push(BackendExecutionOutcome {
                    receipt_index,
                    family: receipt.family,
                    accepted: false,
                    reason,
                    receipt_digest: None,
                });
                continue;
            }
            let Some(attestation) = receipt.attestation.clone() else {
                descriptor.state = BackendState::RejectedExecution;
                report.rejected_count += 1;
                report.outcomes.push(BackendExecutionOutcome {
                    receipt_index,
                    family: receipt.family,
                    accepted: false,
                    reason: "attestation_missing",
                    receipt_digest: None,
                });
                continue;
            };
            let Ok(subject) = SubjectRevision::checked(source_revision) else {
                descriptor.state = BackendState::RejectedExecution;
                report.rejected_count += 1;
                report.outcomes.push(BackendExecutionOutcome {
                    receipt_index,
                    family: receipt.family,
                    accepted: false,
                    reason: "source_subject_invalid",
                    receipt_digest: None,
                });
                continue;
            };
            let Ok(scope) = ReceiptScope::checked(backend_execution_scope(receipt.family)) else {
                descriptor.state = BackendState::RejectedExecution;
                report.rejected_count += 1;
                report.outcomes.push(BackendExecutionOutcome {
                    receipt_index,
                    family: receipt.family,
                    accepted: false,
                    reason: "receipt_scope_invalid",
                    receipt_digest: None,
                });
                continue;
            };
            match verifier.verify(
                attestation,
                &ReceiptPolicy::exact(
                    ReceiptClass::TldgBackend,
                    subject,
                    scope,
                    backend_execution_payload_digest(receipt),
                    now_epoch,
                ),
                replay,
            ) {
                Ok(verified) => {
                    let digest = verified.receipt_digest();
                    descriptor.state = BackendState::ValidatedObservation;
                    descriptor.evidence_digest = Some(receipt.output_digest.clone());
                    descriptor.execution_receipt_digest = Some(digest.clone());
                    descriptor.attestation_verified = true;
                    report.accepted_count += 1;
                    report.outcomes.push(BackendExecutionOutcome {
                        receipt_index,
                        family: receipt.family,
                        accepted: true,
                        reason: "authenticated_execution_observation",
                        receipt_digest: Some(digest),
                    });
                }
                Err(_) => {
                    descriptor.state = BackendState::RejectedExecution;
                    report.rejected_count += 1;
                    report.outcomes.push(BackendExecutionOutcome {
                        receipt_index,
                        family: receipt.family,
                        accepted: false,
                        reason: "attestation_rejected_or_replayed",
                        receipt_digest: None,
                    });
                }
            }
        }
        report
    }
}

fn clear_execution_observation(descriptor: &mut BackendDescriptor) {
    descriptor.state = BackendState::Declared;
    descriptor.evidence_digest = None;
    descriptor.execution_receipt_digest = None;
    descriptor.attestation_verified = false;
}

pub fn default_backend_registry() -> BackendRegistry {
    let mut registry = BackendRegistry::default();
    registry
        .insert(BackendDescriptor {
            family: BackendFamily::BuiltinLossless,
            name: "lc631-lossless-reference".into(),
            revision: "lc631-lossless-reference.v1".into(),
            state: BackendState::RevisionPinned,
            target_schema: UNIFIED_SYNTAX_SCHEMA,
            evidence_digest: None,
            execution_receipt_digest: None,
            attestation_verified: false,
            claim_boundary: "local deterministic source roundtrip only".into(),
        })
        .expect("unique builtin backend");
    for (family, name, boundary) in [
        (
            BackendFamily::BuiltinOpenDiscourse,
            "lc631-open-discourse-reference",
            "local surface discourse constraints only; not HPSG, NLI, or human understanding",
        ),
        (
            BackendFamily::BuiltinExecutableSymbolic,
            "lc631-executable-symbolic-reference",
            "local delimiter, binding-candidate, and effect-candidate validation only; not compiler acceptance",
        ),
    ] {
        registry
            .insert(BackendDescriptor {
                family,
                name: name.into(),
                revision: format!("{name}.v1"),
                state: BackendState::RevisionPinned,
                target_schema: UNIFIED_SYNTAX_SCHEMA,
                evidence_digest: None,
                execution_receipt_digest: None,
                attestation_verified: false,
                claim_boundary: boundary.into(),
            })
            .expect("unique builtin profile backend");
    }
    for (family, name) in [
        (BackendFamily::EnglishHpsg, "ace-erg"),
        (BackendFamily::JapaneseHpsg, "sudachi-jacy-ace"),
        (BackendFamily::EnhancedUd, "enhanced-ud"),
        (BackendFamily::Ucca, "ucca"),
        (BackendFamily::Amr, "amr"),
        (BackendFamily::PropBank, "propbank"),
        (BackendFamily::TimeMl, "timeml"),
        (BackendFamily::GfRgl, "gf-rgl"),
        (BackendFamily::TreeSitter, "tree-sitter"),
        (BackendFamily::RustSyntaxHir, "rust-analyzer-rustc"),
        (BackendFamily::TypeScriptCompiler, "typescript-compiler"),
        (BackendFamily::CppCompiler, "c-cpp-compiler"),
    ] {
        registry
            .insert(BackendDescriptor {
                family,
                name: name.into(),
                revision: format!("{name}.unconfigured"),
                state: BackendState::Unavailable,
                target_schema: UNIFIED_SYNTAX_SCHEMA,
                evidence_digest: None,
                execution_receipt_digest: None,
                attestation_verified: false,
                claim_boundary:
                    "registry declaration only; no external parser execution or validation".into(),
            })
            .expect("unique backend family");
    }
    registry
}

impl BackendRegistry {
    pub fn builtin_profiles_validated(&self) -> bool {
        [
            BackendFamily::BuiltinLossless,
            BackendFamily::BuiltinOpenDiscourse,
            BackendFamily::BuiltinExecutableSymbolic,
        ]
        .into_iter()
        .all(|family| {
            self.get(family).is_some_and(|backend| {
                backend.state == BackendState::ValidatedObservation
                    && backend.evidence_digest.is_some()
                    && backend.execution_receipt_digest.is_some()
                    && backend.attestation_verified
            })
        })
    }
}

pub fn execute_builtin_backend_receipts(
    artifact: &crate::DeepGrammarArtifact,
    issuer: &ReceiptIssuer,
    now_epoch: u64,
) -> Result<Vec<BackendExecutionReceipt>, String> {
    let registry = default_backend_registry();
    let mut receipts = Vec::new();
    for family in [
        BackendFamily::BuiltinLossless,
        BackendFamily::BuiltinOpenDiscourse,
        BackendFamily::BuiltinExecutableSymbolic,
    ] {
        let descriptor = registry
            .get(family)
            .ok_or_else(|| "builtin_descriptor_missing".to_string())?;
        let output_digest = match family {
            BackendFamily::BuiltinLossless => stable_sha256(&artifact.boundary_roundtrip),
            BackendFamily::BuiltinOpenDiscourse => stable_sha256(
                &serde_json::to_string(&(
                    &artifact.source.revision,
                    &artifact.regions,
                    &artifact.syntax,
                    &artifact.constraints,
                ))
                .map_err(|error| format!("builtin_discourse_encode:{error}"))?,
            ),
            BackendFamily::BuiltinExecutableSymbolic => stable_sha256(
                &serde_json::to_string(&(
                    &artifact.source.revision,
                    &artifact.tokens,
                    &artifact.syntax,
                    &artifact.constraints,
                ))
                .map_err(|error| format!("builtin_executable_encode:{error}"))?,
            ),
            _ => return Err("non_builtin_family".into()),
        };
        let mut receipt = BackendExecutionReceipt {
            family,
            source_revision: artifact.source.revision.clone(),
            backend_revision: descriptor.revision.clone(),
            output_digest,
            candidate_executed: true,
            attestation: None,
        };
        receipt.attestation = Some(
            issuer
                .issue(
                    ReceiptClass::TldgBackend,
                    SubjectRevision::checked(&receipt.source_revision)
                        .map_err(|error| format!("subject:{error:?}"))?,
                    ReceiptScope::checked(backend_execution_scope(family))
                        .map_err(|error| format!("scope:{error:?}"))?,
                    backend_execution_payload_digest(&receipt),
                    now_epoch,
                    None,
                    None,
                )
                .map_err(|error| format!("issue:{error:?}"))?,
        );
        receipts.push(receipt);
    }
    Ok(receipts)
}

pub fn backend_execution_payload_digest(receipt: &BackendExecutionReceipt) -> String {
    stable_sha256(&format!(
        "{:?}\0{}\0{}\0{}\0{}",
        receipt.family,
        receipt.source_revision,
        receipt.backend_revision,
        receipt.output_digest,
        receipt.candidate_executed
    ))
}

pub fn backend_execution_scope(family: BackendFamily) -> String {
    format!("tldg/backend/{}", backend_family_name(family))
}

fn backend_family_name(family: BackendFamily) -> &'static str {
    match family {
        BackendFamily::BuiltinLossless => "builtin_lossless",
        BackendFamily::BuiltinOpenDiscourse => "builtin_open_discourse",
        BackendFamily::BuiltinExecutableSymbolic => "builtin_executable_symbolic",
        BackendFamily::EnglishHpsg => "english_hpsg",
        BackendFamily::JapaneseHpsg => "japanese_hpsg",
        BackendFamily::EnhancedUd => "enhanced_ud",
        BackendFamily::Ucca => "ucca",
        BackendFamily::Amr => "amr",
        BackendFamily::PropBank => "prop_bank",
        BackendFamily::TimeMl => "time_ml",
        BackendFamily::GfRgl => "gf_rgl",
        BackendFamily::TreeSitter => "tree_sitter",
        BackendFamily::RustSyntaxHir => "rust_syntax_hir",
        BackendFamily::TypeScriptCompiler => "typescript_compiler",
        BackendFamily::CppCompiler => "cpp_compiler",
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{analyze, RelationKind};

    #[test]
    fn unavailable_backends_are_not_promoted() {
        let registry = default_backend_registry();
        assert_eq!(
            registry.get(BackendFamily::EnglishHpsg).unwrap().state,
            BackendState::Unavailable
        );
        assert!(registry.iter().all(|backend| !backend.can_authorize()));
    }

    #[test]
    fn invalid_execution_after_valid_execution_clears_prior_evidence_atomically() {
        let key = [61_u8; 32];
        let issuer = ReceiptIssuer::from_key_bytes("tldg-test", &key).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("tldg-test", &key).unwrap();
        let artifact = analyze("alpha + beta").unwrap();
        let receipts = execute_builtin_backend_receipts(&artifact, &issuer, 10).unwrap();
        let mut registry = default_backend_registry();
        let mut replay = ReplayGuard::default();
        let accepted = registry.validate_executions_detailed(
            &receipts,
            &artifact.source.revision,
            &verifier,
            &mut replay,
            10,
        );
        assert_eq!(accepted.accepted_count, 3);
        assert!(registry.builtin_profiles_validated());

        let mut tampered = receipts[0].clone();
        tampered.output_digest = stable_sha256("different_output_same_receipt");
        let rejected = registry.validate_executions_detailed(
            &[tampered],
            &artifact.source.revision,
            &verifier,
            &mut replay,
            10,
        );
        assert_eq!(rejected.rejected_count, 1);
        let descriptor = registry.get(BackendFamily::BuiltinLossless).unwrap();
        assert_eq!(descriptor.state, BackendState::RejectedExecution);
        assert!(descriptor.evidence_digest.is_none());
        assert!(descriptor.execution_receipt_digest.is_none());
        assert!(!descriptor.attestation_verified);
        assert!(!registry.builtin_profiles_validated());
    }

    #[test]
    fn duplicate_backend_receipts_are_all_rejected_without_winner_selection() {
        let key = [62_u8; 32];
        let issuer = ReceiptIssuer::from_key_bytes("tldg-test", &key).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("tldg-test", &key).unwrap();
        let artifact = analyze("alpha + beta").unwrap();
        let receipts = execute_builtin_backend_receipts(&artifact, &issuer, 10).unwrap();
        let duplicate = receipts[0].clone();
        let mut registry = default_backend_registry();
        let report = registry.validate_executions_detailed(
            &[duplicate.clone(), duplicate],
            &artifact.source.revision,
            &verifier,
            &mut ReplayGuard::default(),
            10,
        );
        assert_eq!(report.accepted_count, 0);
        assert_eq!(report.rejected_count, 2);
        assert!(report
            .outcomes
            .iter()
            .all(|outcome| outcome.reason == "duplicate_backend_receipts_in_batch"));
        assert_eq!(
            registry.get(BackendFamily::BuiltinLossless).unwrap().state,
            BackendState::RejectedExecution
        );
    }

    #[test]
    fn builtin_payload_digest_changes_when_relation_content_changes_not_only_count() {
        let key = [63_u8; 32];
        let issuer = ReceiptIssuer::from_key_bytes("tldg-test", &key).unwrap();
        let original = analyze("alpha + beta").unwrap();
        let mut changed = original.clone();
        changed.syntax.relations[0].kind = RelationKind::DependencyCandidate;
        assert_eq!(
            original.syntax.relations.len(),
            changed.syntax.relations.len()
        );
        let original_receipts = execute_builtin_backend_receipts(&original, &issuer, 10).unwrap();
        let changed_receipts = execute_builtin_backend_receipts(&changed, &issuer, 10).unwrap();
        let original_digest = original_receipts
            .iter()
            .find(|receipt| receipt.family == BackendFamily::BuiltinOpenDiscourse)
            .unwrap()
            .output_digest
            .as_str();
        let changed_digest = changed_receipts
            .iter()
            .find(|receipt| receipt.family == BackendFamily::BuiltinOpenDiscourse)
            .unwrap()
            .output_digest
            .as_str();
        assert_ne!(original_digest, changed_digest);
    }
}
