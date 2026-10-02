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

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct PrincipalBinding(String);

impl PrincipalBinding {
    pub fn checked(digest: impl Into<String>) -> Result<Self, PermitError> {
        let digest = digest.into();
        if !is_sha256_digest(&digest) {
            return Err(PermitError::PrincipalBindingInvalid);
        }
        Ok(Self(digest))
    }

    /// Create an opaque, host-session-bound principal label from verified host
    /// receipts. This is a binding token, not a claim about a real-world name.
    pub fn from_attested_seed(
        seed_receipt_digest: &str,
        host_key_fingerprint: &str,
    ) -> Result<Self, PermitError> {
        if !is_sha256_digest(seed_receipt_digest) || !is_sha256_digest(host_key_fingerprint) {
            return Err(PermitError::PrincipalBindingInvalid);
        }
        Self::checked(stable_sha256(&format!(
            "host-principal-binding.v1\0{seed_receipt_digest}\0{host_key_fingerprint}"
        )))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExecutionPermit {
    principal_binding: PrincipalBinding,
    action: Action,
    scope: String,
    source_revision: String,
    source_span: SourceSpan,
    authority_revision: AuthorityRevision,
    polarity: DeonticPolarity,
    issued_at_epoch: u64,
    expires_at_epoch: u64,
    revocation_revision: u64,
    authority_receipt_digest: String,
    seed_receipt_digest: String,
    issuer_key_fingerprint: String,
    permit_digest: String,
}

impl ExecutionPermit {
    #[allow(clippy::too_many_arguments)]
    pub fn from_verified_user_event(
        event: VerifiedAuthorityEvent,
        source_text: &str,
        authority_revision: AuthorityRevision,
        issued_at_epoch: u64,
        expires_at_epoch: u64,
        revocation_revision: u64,
    ) -> Result<Self, PermitError> {
        let seed_receipt_digest = event
            .parent_seed_receipt_digest()
            .ok_or(PermitError::MissingSeedReceipt)?
            .to_string();
        let principal_binding = PrincipalBinding::from_attested_seed(
            &seed_receipt_digest,
            &event.verifier_key_fingerprint,
        )?;
        let source_event = event.event();
        if source_event.source != AuthoritySourceKind::VerifiedHostUserSpan {
            return Err(PermitError::UnverifiedOrigin);
        }
        if !matches!(
            source_event.polarity,
            DeonticPolarity::Grant | DeonticPolarity::Deny
        ) {
            return Err(PermitError::PolarityNotActionable);
        }
        if source_event.scope.trim().is_empty()
            || source_event.span.slice(source_text).is_err()
            || !is_sha256_digest(&event.verifier_key_fingerprint)
            || !is_sha256_digest(&event.receipt_digest)
            || !is_sha256_digest(&seed_receipt_digest)
            || !is_sha256_digest(&stable_sha256(source_text))
            || issued_at_epoch < event.receipt_issued_at_epoch
            || expires_at_epoch <= issued_at_epoch
            || event
                .receipt_expires_at_epoch
                .is_some_and(|receipt_expiry| expires_at_epoch > receipt_expiry)
            || expires_at_epoch - issued_at_epoch > 86_400
        {
            return Err(PermitError::PermitBindingInvalid);
        }
        let source_revision = stable_sha256(source_text);
        let issuer_key_fingerprint = event.verifier_key_fingerprint.clone();
        let authority_receipt_digest = event.receipt_digest.clone();
        let permit_digest = execution_permit_digest(
            &principal_binding,
            source_event.action,
            &source_event.scope,
            &source_revision,
            authority_revision,
            source_event.polarity,
            source_event.span,
            issued_at_epoch,
            expires_at_epoch,
            revocation_revision,
            &authority_receipt_digest,
            &seed_receipt_digest,
            &issuer_key_fingerprint,
        );
        Ok(Self {
            principal_binding,
            action: source_event.action,
            scope: source_event.scope.clone(),
            source_revision,
            source_span: source_event.span,
            authority_revision,
            polarity: source_event.polarity,
            issued_at_epoch,
            expires_at_epoch,
            revocation_revision,
            authority_receipt_digest,
            seed_receipt_digest,
            issuer_key_fingerprint,
            permit_digest,
        })
    }

    pub fn digest(&self) -> &str {
        &self.permit_digest
    }

    pub fn action(&self) -> Action {
        self.action
    }

    pub fn scope(&self) -> &str {
        &self.scope
    }

    pub fn principal_binding(&self) -> &PrincipalBinding {
        &self.principal_binding
    }

    pub fn polarity(&self) -> DeonticPolarity {
        self.polarity
    }

    pub fn source_span(&self) -> SourceSpan {
        self.source_span
    }

    pub fn expires_at_epoch(&self) -> u64 {
        self.expires_at_epoch
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CallerOrigin {
    HostVerifiedUser,
    StandaloneCli,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermitRejection {
    PrincipalBindingInvalid,
    PermitBindingInvalid,
    MissingSeedReceipt,
    UnverifiedOrigin,
    PolarityNotActionable,
    PermitDigestMismatch,
    PrincipalMismatch,
    ActionMismatch,
    ScopeMismatch,
    SourceRevisionMismatch,
    AuthorityRevisionMismatch,
    TrustedIssuerMismatch,
    NotYetValid,
    Expired,
    Revoked,
    RevocationRevisionChanged,
    UntrustedCallerOrigin,
}

pub type PermitError = PermitRejection;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermitValidation {
    ValidGrant,
    ValidDeny,
    Rejected(PermitRejection),
}

pub struct ExecutionPermitContext<'a> {
    pub principal: &'a PrincipalBinding,
    pub action: Action,
    pub scope: &'a str,
    pub source_revision: &'a str,
    pub authority_revision: AuthorityRevision,
    pub now_epoch: u64,
    pub revocation_revision: u64,
    pub revoked_permit_digests: &'a [String],
    pub caller_origin: CallerOrigin,
    pub trusted_host_fingerprint: &'a str,
}

impl ExecutionPermit {
    pub fn validate(&self, context: &ExecutionPermitContext<'_>) -> PermitValidation {
        validate_execution_permit(self, context)
    }
}

pub fn validate_execution_permit(
    permit: &ExecutionPermit,
    context: &ExecutionPermitContext<'_>,
) -> PermitValidation {
    if permit.permit_digest
        != execution_permit_digest(
            &permit.principal_binding,
            permit.action,
            &permit.scope,
            &permit.source_revision,
            permit.authority_revision,
            permit.polarity,
            permit.source_span,
            permit.issued_at_epoch,
            permit.expires_at_epoch,
            permit.revocation_revision,
            &permit.authority_receipt_digest,
            &permit.seed_receipt_digest,
            &permit.issuer_key_fingerprint,
        )
    {
        return PermitValidation::Rejected(PermitRejection::PermitDigestMismatch);
    }
    if context.caller_origin != CallerOrigin::HostVerifiedUser {
        return PermitValidation::Rejected(PermitRejection::UntrustedCallerOrigin);
    }
    if &permit.principal_binding != context.principal {
        return PermitValidation::Rejected(PermitRejection::PrincipalMismatch);
    }
    if permit.action != context.action {
        return PermitValidation::Rejected(PermitRejection::ActionMismatch);
    }
    if permit.scope != context.scope {
        return PermitValidation::Rejected(PermitRejection::ScopeMismatch);
    }
    if permit.source_revision != context.source_revision {
        return PermitValidation::Rejected(PermitRejection::SourceRevisionMismatch);
    }
    if permit.authority_revision != context.authority_revision {
        return PermitValidation::Rejected(PermitRejection::AuthorityRevisionMismatch);
    }
    if permit.issuer_key_fingerprint != context.trusted_host_fingerprint {
        return PermitValidation::Rejected(PermitRejection::TrustedIssuerMismatch);
    }
    if context.now_epoch < permit.issued_at_epoch {
        return PermitValidation::Rejected(PermitRejection::NotYetValid);
    }
    if context.now_epoch >= permit.expires_at_epoch {
        return PermitValidation::Rejected(PermitRejection::Expired);
    }
    if permit.revocation_revision != context.revocation_revision {
        return PermitValidation::Rejected(PermitRejection::RevocationRevisionChanged);
    }
    if context
        .revoked_permit_digests
        .iter()
        .any(|digest| digest == &permit.permit_digest)
    {
        return PermitValidation::Rejected(PermitRejection::Revoked);
    }
    match permit.polarity {
        DeonticPolarity::Grant => PermitValidation::ValidGrant,
        DeonticPolarity::Deny => PermitValidation::ValidDeny,
        _ => PermitValidation::Rejected(PermitRejection::PolarityNotActionable),
    }
}

pub fn decide_execution_permits(
    permits: &[ExecutionPermit],
    context: &ExecutionPermitContext<'_>,
) -> AuthorityDecision {
    let mut matching_permits = 0usize;
    let mut grants = 0usize;
    let mut denies = 0usize;
    let mut rejected = false;
    let mut residuals = Vec::new();
    for permit in permits
        .iter()
        .filter(|permit| permit.action == context.action && permit.scope == context.scope)
    {
        matching_permits += 1;
        match validate_execution_permit(permit, context) {
            PermitValidation::ValidGrant => grants += 1,
            PermitValidation::ValidDeny => denies += 1,
            PermitValidation::Rejected(reason) => {
                rejected = true;
                residuals.push(format!("execution_permit_rejected:{reason:?}"));
            }
        }
    }
    let (authorized, polarity) = if denies > 0 {
        (false, DeonticPolarity::Deny)
    } else if grants > 1 || (grants == 1 && rejected) {
        (false, DeonticPolarity::Conflict)
    } else if grants == 1 {
        (true, DeonticPolarity::Grant)
    } else if matching_permits == 0 || rejected {
        (false, DeonticPolarity::Unknown)
    } else {
        (false, DeonticPolarity::Mention)
    };
    AuthorityDecision {
        action: context.action,
        authorized,
        polarity,
        scope: context.scope.to_string(),
        residuals,
    }
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[allow(clippy::too_many_arguments)]
fn execution_permit_digest(
    principal: &PrincipalBinding,
    action: Action,
    scope: &str,
    source_revision: &str,
    authority_revision: AuthorityRevision,
    polarity: DeonticPolarity,
    source_span: SourceSpan,
    issued_at: u64,
    expires_at: u64,
    revocation_revision: u64,
    authority_receipt_digest: &str,
    seed_receipt_digest: &str,
    issuer_fingerprint: &str,
) -> String {
    stable_sha256(&format!(
        "execution-permit.v1\0{}\0{action:?}\0{}\0{}\0{}\0{polarity:?}\0{}\0{}\0{}\0{}\0{}\0{}\0{seed_receipt_digest}\0{issuer_fingerprint}",
        principal.as_str(),
        scope,
        source_revision,
        authority_revision.0,
        source_span.start,
        source_span.end,
        issued_at,
        expires_at,
        revocation_revision,
        authority_receipt_digest,
    ))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerifiedAuthorityEvent {
    event: AuthorityEvent,
    receipt_digest: String,
    verifier_key_fingerprint: String,
    parent_seed_receipt_digest: Option<String>,
    receipt_issued_at_epoch: u64,
    receipt_expires_at_epoch: Option<u64>,
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
            verifier_key_fingerprint: receipt.verifier_key_fingerprint().to_string(),
            parent_seed_receipt_digest: receipt.parent_receipt_digest().map(str::to_string),
            receipt_issued_at_epoch: receipt.issued_at_epoch(),
            receipt_expires_at_epoch: receipt.expires_at_epoch(),
        })
    }

    pub fn event(&self) -> &AuthorityEvent {
        &self.event
    }

    pub fn receipt_digest(&self) -> &str {
        &self.receipt_digest
    }

    pub fn verifier_key_fingerprint(&self) -> &str {
        &self.verifier_key_fingerprint
    }

    pub fn parent_seed_receipt_digest(&self) -> Option<&str> {
        self.parent_seed_receipt_digest.as_deref()
    }

    pub fn receipt_issued_at_epoch(&self) -> u64 {
        self.receipt_issued_at_epoch
    }

    pub fn receipt_expires_at_epoch(&self) -> Option<u64> {
        self.receipt_expires_at_epoch
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
