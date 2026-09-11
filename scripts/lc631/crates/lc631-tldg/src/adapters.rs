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
        let mut accepted = 0;
        for receipt in receipts {
            let Some(descriptor) = self.entries.get_mut(&receipt.family) else {
                continue;
            };
            descriptor.state = BackendState::ExecutedCandidate;
            if receipt.source_revision != source_revision
                || receipt.backend_revision != descriptor.revision
                || !receipt.candidate_executed
                || !valid_digest(&receipt.output_digest)
            {
                continue;
            }
            let Some(attestation) = receipt.attestation.clone() else {
                continue;
            };
            let Ok(subject) = SubjectRevision::checked(source_revision) else {
                continue;
            };
            let Ok(scope) = ReceiptScope::checked(backend_execution_scope(receipt.family)) else {
                continue;
            };
            let verified = verifier.verify(
                attestation,
                &ReceiptPolicy::exact(
                    ReceiptClass::TldgBackend,
                    subject,
                    scope,
                    backend_execution_payload_digest(receipt),
                    now_epoch,
                ),
                replay,
            );
            if let Ok(verified) = verified {
                descriptor.state = BackendState::ValidatedObservation;
                descriptor.evidence_digest = Some(receipt.output_digest.clone());
                descriptor.execution_receipt_digest = Some(verified.receipt_digest());
                descriptor.attestation_verified = true;
                accepted += 1;
            }
        }
        accepted
    }
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
            BackendFamily::BuiltinOpenDiscourse => stable_sha256(&format!(
                "{}:{}",
                artifact.source.revision,
                artifact.syntax.relations.len()
            )),
            BackendFamily::BuiltinExecutableSymbolic => stable_sha256(&format!(
                "{}:{}",
                artifact.source.revision,
                artifact.constraints.edges.len()
            )),
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

    #[test]
    fn unavailable_backends_are_not_promoted() {
        let registry = default_backend_registry();
        assert_eq!(
            registry.get(BackendFamily::EnglishHpsg).unwrap().state,
            BackendState::Unavailable
        );
        assert!(registry.iter().all(|backend| !backend.can_authorize()));
    }
}
