#![forbid(unsafe_code)]

use lc631_receipt_kernel::{ReceiptClass, VerifiedReceipt};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const LC631_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const LC631_NAMESPACE: &str = "lc631";

macro_rules! id_type {
    ($name:ident) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        pub struct $name(pub u64);
    };
}

id_type!(ArtifactId);
id_type!(AuthorityRevision);
id_type!(CandidateId);
id_type!(EpochId);
id_type!(FenceId);
id_type!(Generation);
id_type!(ObligationId);
id_type!(ReceiptId);
id_type!(TurnId);
id_type!(WorldId);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

impl SourceSpan {
    pub fn checked(source: &str, start: usize, end: usize) -> Result<Self, SpanError> {
        if start > end || end > source.len() {
            return Err(SpanError::OutOfBounds);
        }
        if !source.is_char_boundary(start) || !source.is_char_boundary(end) {
            return Err(SpanError::NotUtf8Boundary);
        }
        Ok(Self { start, end })
    }

    pub fn slice<'a>(&self, source: &'a str) -> Result<&'a str, SpanError> {
        source
            .get(self.start..self.end)
            .ok_or(SpanError::OutOfBounds)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum SpanError {
    OutOfBounds,
    NotUtf8Boundary,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosureLevel {
    Declared,
    Typed,
    Reachable,
    Executed,
    Consumed,
    Contained,
    HostBound,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptOwner {
    AcceleratorBroker,
    AuthorityKernel,
    CompatibilityGuard,
    HostBridge,
    MediaSource,
    ProjectionWitness,
    WorldArena,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ClosureWitness {
    pub component: String,
    pub level: ClosureLevel,
    pub owner: ReceiptOwner,
    pub revision_digest: String,
    pub blockers: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ClosureLedger {
    witnesses: BTreeMap<String, ClosureWitness>,
    authenticated_receipts: BTreeMap<String, String>,
}

impl ClosureLedger {
    pub fn insert(&mut self, witness: ClosureWitness) -> Result<(), ClosureError> {
        if witness.level == ClosureLevel::HostBound {
            return Err(ClosureError::UnauthenticatedStrongState(witness.component));
        }
        self.insert_inner(witness, None)
    }

    pub fn insert_verified(&mut self, witness: VerifiedClosureWitness) -> Result<(), ClosureError> {
        let (witness, receipt_digest) = witness.into_parts();
        self.insert_inner(witness, Some(receipt_digest))
    }

    fn insert_inner(
        &mut self,
        witness: ClosureWitness,
        receipt_digest: Option<String>,
    ) -> Result<(), ClosureError> {
        if let Some(existing) = self.witnesses.get(&witness.component) {
            if existing.owner != witness.owner {
                return Err(ClosureError::DuplicateOwner {
                    component: witness.component,
                    first: existing.owner,
                    second: witness.owner,
                });
            }
            if witness.level < existing.level {
                return Err(ClosureError::NonMonotoneRegression(witness.component));
            }
        }
        if let Some(receipt_digest) = receipt_digest {
            self.authenticated_receipts
                .insert(witness.component.clone(), receipt_digest);
        } else {
            self.authenticated_receipts.remove(&witness.component);
        }
        self.witnesses.insert(witness.component.clone(), witness);
        Ok(())
    }

    pub fn witnesses(&self) -> impl Iterator<Item = &ClosureWitness> {
        self.witnesses.values()
    }

    pub fn host_bound(&self) -> bool {
        !self.witnesses.is_empty()
            && self.witnesses.values().all(|witness| {
                witness.level == ClosureLevel::HostBound
                    && witness.blockers.is_empty()
                    && self.authenticated_receipts.contains_key(&witness.component)
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ClosureError {
    DuplicateOwner {
        component: String,
        first: ReceiptOwner,
        second: ReceiptOwner,
    },
    NonMonotoneRegression(String),
    ReceiptClassMismatch,
    ReceiptPayloadMismatch,
    ReceiptScopeMismatch,
    UnauthenticatedStrongState(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerifiedClosureWitness {
    witness: ClosureWitness,
    receipt_digest: String,
}

impl VerifiedClosureWitness {
    pub fn checked(
        witness: ClosureWitness,
        receipt: VerifiedReceipt,
    ) -> Result<Self, ClosureError> {
        if receipt.class() != ReceiptClass::Closure {
            return Err(ClosureError::ReceiptClassMismatch);
        }
        if receipt.payload_digest() != closure_witness_payload_digest(&witness) {
            return Err(ClosureError::ReceiptPayloadMismatch);
        }
        if receipt.scope().as_str() != format!("closure/{}", witness.component) {
            return Err(ClosureError::ReceiptScopeMismatch);
        }
        Ok(Self {
            witness,
            receipt_digest: receipt.receipt_digest(),
        })
    }

    fn into_parts(self) -> (ClosureWitness, String) {
        (self.witness, self.receipt_digest)
    }
}

pub fn closure_witness_payload_digest(witness: &ClosureWitness) -> String {
    stable_sha256(&format!(
        "{}\0{:?}\0{:?}\0{}\0{}",
        witness.component,
        witness.level,
        witness.owner,
        witness.revision_digest,
        witness.blockers.join("\u{1f}")
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Edit,
    Test,
    Commit,
    Push,
    Merge,
    Restart,
    PluginReinstall,
    Delete,
    NetworkAcquireMedia,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoritySourceKind {
    VerifiedHostUserSpan,
    SelfAttestedUser,
    AssistantText,
    RepositoryText,
    ToolOutput,
    QuoteOrCode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeonticPolarity {
    Grant,
    Deny,
    Mention,
    Unknown,
    Conflict,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthorityEvent {
    pub action: Action,
    pub source: AuthoritySourceKind,
    pub polarity: DeonticPolarity,
    pub span: SourceSpan,
    pub scope: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthorityDecision {
    pub action: Action,
    pub authorized: bool,
    pub polarity: DeonticPolarity,
    pub scope: String,
    pub residuals: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerifiedAuthorityEvent {
    event: AuthorityEvent,
    receipt_digest: String,
}

impl VerifiedAuthorityEvent {
    pub fn checked(
        event: AuthorityEvent,
        receipt: VerifiedReceipt,
    ) -> Result<Self, AuthorityError> {
        if event.source != AuthoritySourceKind::VerifiedHostUserSpan {
            return Err(AuthorityError::IneligibleSource);
        }
        if receipt.class() != ReceiptClass::Authority {
            return Err(AuthorityError::ReceiptClassMismatch);
        }
        if receipt.payload_digest() != authority_event_payload_digest(&event) {
            return Err(AuthorityError::ReceiptPayloadMismatch);
        }
        let expected_scope = format!("authority/{}/{}", action_name(event.action), event.scope);
        if receipt.scope().as_str() != expected_scope {
            return Err(AuthorityError::ReceiptScopeMismatch);
        }
        Ok(Self {
            event,
            receipt_digest: receipt.receipt_digest(),
        })
    }

    pub fn event(&self) -> &AuthorityEvent {
        &self.event
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum AuthorityError {
    IneligibleSource,
    ReceiptClassMismatch,
    ReceiptPayloadMismatch,
    ReceiptScopeMismatch,
}

pub fn decide_authority(
    action: Action,
    requested_scope: &str,
    events: &[AuthorityEvent],
) -> AuthorityDecision {
    let applicable = events
        .iter()
        .filter(|event| event.action == action && event.scope == requested_scope)
        .collect::<Vec<_>>();
    let has_deny = applicable
        .iter()
        .any(|event| event.polarity == DeonticPolarity::Deny);
    let grants = 0_usize;
    let authorized = false;
    let polarity = if has_deny {
        DeonticPolarity::Deny
    } else if grants == 1 {
        DeonticPolarity::Grant
    } else if grants > 1 {
        DeonticPolarity::Conflict
    } else if applicable.is_empty() {
        DeonticPolarity::Unknown
    } else {
        DeonticPolarity::Mention
    };
    let mut residuals = Vec::new();
    for event in applicable {
        residuals.push(format!(
            "unauthenticated_authority_event:{:?}",
            event.source
        ));
    }
    AuthorityDecision {
        action,
        authorized,
        polarity,
        scope: requested_scope.to_string(),
        residuals,
    }
}

pub fn decide_verified_authority(
    action: Action,
    requested_scope: &str,
    events: &[VerifiedAuthorityEvent],
) -> AuthorityDecision {
    let applicable = events
        .iter()
        .filter(|verified| {
            verified.event.action == action && verified.event.scope == requested_scope
        })
        .collect::<Vec<_>>();
    let has_deny = applicable
        .iter()
        .any(|verified| verified.event.polarity == DeonticPolarity::Deny);
    let grants = applicable
        .iter()
        .filter(|verified| verified.event.polarity == DeonticPolarity::Grant)
        .count();
    let authorized = !has_deny && grants == 1;
    let polarity = if has_deny {
        DeonticPolarity::Deny
    } else if grants == 1 {
        DeonticPolarity::Grant
    } else if grants > 1 {
        DeonticPolarity::Conflict
    } else if applicable.is_empty() {
        DeonticPolarity::Unknown
    } else {
        DeonticPolarity::Mention
    };
    let mut residuals = Vec::new();
    if grants > 1 {
        residuals.push("duplicate_grant_events".to_string());
    }
    AuthorityDecision {
        action,
        authorized,
        polarity,
        scope: requested_scope.to_string(),
        residuals,
    }
}

pub fn authority_event_payload_digest(event: &AuthorityEvent) -> String {
    stable_sha256(&format!(
        "{:?}\0{:?}\0{:?}\0{}\0{}\0{}",
        event.action, event.source, event.polarity, event.span.start, event.span.end, event.scope
    ))
}

fn action_name(action: Action) -> &'static str {
    match action {
        Action::Edit => "edit",
        Action::Test => "test",
        Action::Commit => "commit",
        Action::Push => "push",
        Action::Merge => "merge",
        Action::Restart => "restart",
        Action::PluginReinstall => "plugin_reinstall",
        Action::Delete => "delete",
        Action::NetworkAcquireMedia => "network_acquire_media",
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RevisionBinding {
    pub artifact: ArtifactId,
    pub source_sha256: String,
    pub program_sha256: String,
    pub authority_revision: AuthorityRevision,
    pub engine_version: String,
    pub kernel_revision: String,
    pub device_identity: String,
}

impl RevisionBinding {
    pub fn digest(&self) -> String {
        stable_sha256(&format!(
            "{}:{}:{}:{}:{}:{}:{}",
            self.artifact.0,
            self.source_sha256,
            self.program_sha256,
            self.authority_revision.0,
            self.engine_version,
            self.kernel_revision,
            self.device_identity
        ))
    }

    pub fn semantic_digest(&self) -> String {
        stable_sha256(&format!(
            "{}:{}:{}:{}:{}",
            self.artifact.0,
            self.source_sha256,
            self.program_sha256,
            self.authority_revision.0,
            self.engine_version
        ))
    }
}

pub fn stable_sha256(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut encoded = String::with_capacity(7 + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

pub fn duplicate_values(values: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for value in values {
        if !seen.insert(value.clone()) {
            duplicates.insert(value);
        }
    }
    duplicates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_span_rejects_mid_codepoint() {
        assert_eq!(
            SourceSpan::checked("日本", 1, 3),
            Err(SpanError::NotUtf8Boundary)
        );
    }

    #[test]
    fn duplicate_receipt_owner_is_rejected() {
        let mut ledger = ClosureLedger::default();
        ledger
            .insert(ClosureWitness {
                component: "gpu".into(),
                level: ClosureLevel::Typed,
                owner: ReceiptOwner::AcceleratorBroker,
                revision_digest: "a".into(),
                blockers: vec![],
            })
            .unwrap();
        let error = ledger
            .insert(ClosureWitness {
                component: "gpu".into(),
                level: ClosureLevel::Executed,
                owner: ReceiptOwner::WorldArena,
                revision_digest: "b".into(),
                blockers: vec![],
            })
            .unwrap_err();
        assert!(matches!(error, ClosureError::DuplicateOwner { .. }));
    }

    #[test]
    fn only_verified_host_user_span_can_grant() {
        let span = SourceSpan { start: 0, end: 4 };
        let rejected = decide_authority(
            Action::Edit,
            "repo",
            &[AuthorityEvent {
                action: Action::Edit,
                source: AuthoritySourceKind::RepositoryText,
                polarity: DeonticPolarity::Grant,
                span,
                scope: "repo".into(),
            }],
        );
        assert!(!rejected.authorized);
        let public_verified_label_is_still_untrusted = decide_authority(
            Action::Edit,
            "repo",
            &[AuthorityEvent {
                action: Action::Edit,
                source: AuthoritySourceKind::VerifiedHostUserSpan,
                polarity: DeonticPolarity::Grant,
                span,
                scope: "repo".into(),
            }],
        );
        assert!(!public_verified_label_is_still_untrusted.authorized);
    }
}
