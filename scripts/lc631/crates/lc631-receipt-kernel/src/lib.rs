#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const RECEIPT_SCHEMA: &str = "lc631-authenticated-receipt.v1";
const MIN_KEY_BYTES: usize = 32;
const HMAC_BLOCK_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptClass {
    Accelerator,
    Authority,
    Baseline,
    Closure,
    HostOutput,
    HostSeed,
    MediaBackend,
    MediaSource,
    Package,
    RemoteCi,
    ReleaseEvaluation,
    StageCompletion,
    TldgBackend,
    ToolExecution,
    Validation,
    VerifierCapability,
}

impl ReceiptClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accelerator => "accelerator",
            Self::Authority => "authority",
            Self::Baseline => "baseline",
            Self::Closure => "closure",
            Self::HostOutput => "host_output",
            Self::HostSeed => "host_seed",
            Self::MediaBackend => "media_backend",
            Self::MediaSource => "media_source",
            Self::Package => "package",
            Self::RemoteCi => "remote_ci",
            Self::ReleaseEvaluation => "release_evaluation",
            Self::StageCompletion => "stage_completion",
            Self::TldgBackend => "tldg_backend",
            Self::ToolExecution => "tool_execution",
            Self::Validation => "validation",
            Self::VerifierCapability => "verifier_capability",
        }
    }
}

macro_rules! checked_text {
    ($name:ident, $limit:expr) => {
        #[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn checked(value: impl Into<String>) -> Result<Self, ReceiptError> {
                let value = value.into();
                if value.trim().is_empty()
                    || value.len() > $limit
                    || value.chars().any(char::is_control)
                {
                    return Err(ReceiptError::InvalidField(stringify!($name)));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

checked_text!(PrincipalId, 128);
checked_text!(SubjectRevision, 256);
checked_text!(ReceiptScope, 512);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptClaims {
    schema_version: String,
    issuer: PrincipalId,
    class: ReceiptClass,
    subject_revision: SubjectRevision,
    scope: ReceiptScope,
    nonce_hex: String,
    payload_digest: String,
    issued_at_epoch: u64,
    expires_at_epoch: Option<u64>,
    parent_receipt_digest: Option<String>,
}

impl ReceiptClaims {
    pub fn issuer(&self) -> &PrincipalId {
        &self.issuer
    }

    pub fn class(&self) -> ReceiptClass {
        self.class
    }

    pub fn subject_revision(&self) -> &SubjectRevision {
        &self.subject_revision
    }

    pub fn scope(&self) -> &ReceiptScope {
        &self.scope
    }

    pub fn payload_digest(&self) -> &str {
        &self.payload_digest
    }

    pub fn parent_receipt_digest(&self) -> Option<&str> {
        self.parent_receipt_digest.as_deref()
    }

    pub fn issued_at_epoch(&self) -> u64 {
        self.issued_at_epoch
    }

    pub fn expires_at_epoch(&self) -> Option<u64> {
        self.expires_at_epoch
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UntrustedReceipt {
    claims: ReceiptClaims,
    mac_sha256: String,
}

impl UntrustedReceipt {
    pub fn claims(&self) -> &ReceiptClaims {
        &self.claims
    }

    pub fn receipt_digest(&self) -> String {
        digest_text(&format!(
            "{}\0{}",
            canonical_claims(&self.claims),
            self.mac_sha256
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerifiedReceipt {
    wire: UntrustedReceipt,
    verifier_key_fingerprint: String,
}

impl VerifiedReceipt {
    pub fn class(&self) -> ReceiptClass {
        self.wire.claims.class
    }

    pub fn subject_revision(&self) -> &SubjectRevision {
        &self.wire.claims.subject_revision
    }

    pub fn scope(&self) -> &ReceiptScope {
        &self.wire.claims.scope
    }

    pub fn payload_digest(&self) -> &str {
        &self.wire.claims.payload_digest
    }

    pub fn receipt_digest(&self) -> String {
        self.wire.receipt_digest()
    }

    pub fn verifier_key_fingerprint(&self) -> &str {
        &self.verifier_key_fingerprint
    }

    pub fn into_untrusted(self) -> UntrustedReceipt {
        self.wire
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptPolicy {
    expected_class: ReceiptClass,
    expected_subject: SubjectRevision,
    expected_scope: ReceiptScope,
    expected_payload_digest: String,
    expected_parent_receipt_digest: Option<String>,
    now_epoch: u64,
}

impl ReceiptPolicy {
    pub fn exact(
        expected_class: ReceiptClass,
        expected_subject: SubjectRevision,
        expected_scope: ReceiptScope,
        expected_payload_digest: impl Into<String>,
        now_epoch: u64,
    ) -> Self {
        Self {
            expected_class,
            expected_subject,
            expected_scope,
            expected_payload_digest: expected_payload_digest.into(),
            expected_parent_receipt_digest: None,
            now_epoch,
        }
    }

    pub fn with_parent(mut self, parent_receipt_digest: impl Into<String>) -> Self {
        self.expected_parent_receipt_digest = Some(parent_receipt_digest.into());
        self
    }
}

#[derive(Debug)]
struct SecretKey(Vec<u8>);

impl SecretKey {
    fn checked(bytes: &[u8]) -> Result<Self, ReceiptError> {
        if bytes.len() < MIN_KEY_BYTES {
            return Err(ReceiptError::WeakKey);
        }
        Ok(Self(bytes.to_vec()))
    }
}

impl Drop for SecretKey {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

#[derive(Debug)]
pub struct ReceiptIssuer {
    principal: PrincipalId,
    key: SecretKey,
}

impl ReceiptIssuer {
    pub fn from_key_bytes(principal: impl Into<String>, key: &[u8]) -> Result<Self, ReceiptError> {
        Ok(Self {
            principal: PrincipalId::checked(principal)?,
            key: SecretKey::checked(key)?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        &self,
        class: ReceiptClass,
        subject_revision: SubjectRevision,
        scope: ReceiptScope,
        payload_digest: impl Into<String>,
        issued_at_epoch: u64,
        expires_at_epoch: Option<u64>,
        parent_receipt_digest: Option<String>,
    ) -> Result<UntrustedReceipt, ReceiptError> {
        let payload_digest = payload_digest.into();
        require_digest(&payload_digest)?;
        if expires_at_epoch.is_some_and(|expires| expires < issued_at_epoch) {
            return Err(ReceiptError::InvalidExpiry);
        }
        if let Some(parent) = &parent_receipt_digest {
            require_digest(parent)?;
        }
        let mut nonce = [0_u8; 16];
        getrandom::fill(&mut nonce).map_err(|_| ReceiptError::RandomUnavailable)?;
        let claims = ReceiptClaims {
            schema_version: RECEIPT_SCHEMA.into(),
            issuer: self.principal.clone(),
            class,
            subject_revision,
            scope,
            nonce_hex: hex(&nonce),
            payload_digest,
            issued_at_epoch,
            expires_at_epoch,
            parent_receipt_digest,
        };
        let mac_sha256 = hmac_digest(&self.key.0, canonical_claims(&claims).as_bytes());
        Ok(UntrustedReceipt { claims, mac_sha256 })
    }

    pub fn principal(&self) -> &PrincipalId {
        &self.principal
    }

    pub fn key_fingerprint(&self) -> String {
        digest_bytes(&self.key.0)
    }
}

#[derive(Debug)]
pub struct ReceiptVerifier {
    principal: PrincipalId,
    key: SecretKey,
    key_fingerprint: String,
}

impl ReceiptVerifier {
    pub fn from_key_bytes(principal: impl Into<String>, key: &[u8]) -> Result<Self, ReceiptError> {
        let key = SecretKey::checked(key)?;
        let key_fingerprint = digest_bytes(&key.0);
        Ok(Self {
            principal: PrincipalId::checked(principal)?,
            key,
            key_fingerprint,
        })
    }

    pub fn verify(
        &self,
        receipt: UntrustedReceipt,
        policy: &ReceiptPolicy,
        replay: &mut ReplayGuard,
    ) -> Result<VerifiedReceipt, ReceiptError> {
        validate_wire(&receipt)?;
        let expected_mac = hmac_digest(&self.key.0, canonical_claims(&receipt.claims).as_bytes());
        if !constant_time_eq(expected_mac.as_bytes(), receipt.mac_sha256.as_bytes()) {
            return Err(ReceiptError::MacMismatch);
        }
        let claims = &receipt.claims;
        if claims.issuer != self.principal {
            return Err(ReceiptError::IssuerMismatch);
        }
        if claims.class != policy.expected_class {
            return Err(ReceiptError::ClassMismatch);
        }
        if claims.subject_revision != policy.expected_subject {
            return Err(ReceiptError::SubjectMismatch);
        }
        if claims.scope != policy.expected_scope {
            return Err(ReceiptError::ScopeMismatch);
        }
        if claims.payload_digest != policy.expected_payload_digest {
            return Err(ReceiptError::PayloadMismatch);
        }
        if claims.parent_receipt_digest != policy.expected_parent_receipt_digest {
            return Err(ReceiptError::ParentMismatch);
        }
        if policy.now_epoch < claims.issued_at_epoch {
            return Err(ReceiptError::NotYetValid);
        }
        if claims
            .expires_at_epoch
            .is_some_and(|expires| policy.now_epoch > expires)
        {
            return Err(ReceiptError::Expired);
        }
        let digest = receipt.receipt_digest();
        replay.check_and_record(&claims.nonce_hex, &digest)?;
        Ok(VerifiedReceipt {
            wire: receipt,
            verifier_key_fingerprint: self.key_fingerprint.clone(),
        })
    }

    pub fn key_fingerprint(&self) -> &str {
        &self.key_fingerprint
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ReplayGuard {
    seen_nonces: BTreeSet<String>,
    seen_receipts: BTreeSet<String>,
}

impl ReplayGuard {
    fn check_and_record(&mut self, nonce: &str, receipt: &str) -> Result<(), ReceiptError> {
        if self.seen_nonces.contains(nonce) || self.seen_receipts.contains(receipt) {
            return Err(ReceiptError::ReplayDetected);
        }
        self.seen_nonces.insert(nonce.to_string());
        self.seen_receipts.insert(receipt.to_string());
        Ok(())
    }

    pub fn observed_count(&self) -> usize {
        self.seen_receipts.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ReceiptError {
    ClassMismatch,
    Expired,
    InvalidDigest,
    InvalidExpiry,
    InvalidField(&'static str),
    InvalidWire,
    IssuerMismatch,
    MacMismatch,
    NotYetValid,
    PayloadMismatch,
    ParentMismatch,
    RandomUnavailable,
    ReplayDetected,
    ScopeMismatch,
    SubjectMismatch,
    WeakKey,
}

pub fn generate_key() -> Result<[u8; 32], ReceiptError> {
    let mut key = [0_u8; 32];
    getrandom::fill(&mut key).map_err(|_| ReceiptError::RandomUnavailable)?;
    Ok(key)
}

pub fn digest_text(value: &str) -> String {
    digest_bytes(value.as_bytes())
}

pub fn digest_bytes(value: &[u8]) -> String {
    format_digest(Sha256::digest(value))
}

fn validate_wire(receipt: &UntrustedReceipt) -> Result<(), ReceiptError> {
    let claims = &receipt.claims;
    if claims.schema_version != RECEIPT_SCHEMA
        || claims.nonce_hex.len() != 32
        || !claims
            .nonce_hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || claims
            .expires_at_epoch
            .is_some_and(|expires| expires < claims.issued_at_epoch)
    {
        return Err(ReceiptError::InvalidWire);
    }
    require_digest(&claims.payload_digest)?;
    require_digest(&receipt.mac_sha256)?;
    if let Some(parent) = &claims.parent_receipt_digest {
        require_digest(parent)?;
    }
    Ok(())
}

fn require_digest(value: &str) -> Result<(), ReceiptError> {
    if value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        Ok(())
    } else {
        Err(ReceiptError::InvalidDigest)
    }
}

fn canonical_claims(claims: &ReceiptClaims) -> String {
    format!(
        "{}\0{}\0{:?}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
        claims.schema_version,
        claims.issuer.as_str(),
        claims.class,
        claims.subject_revision.as_str(),
        claims.scope.as_str(),
        claims.nonce_hex,
        claims.payload_digest,
        claims.issued_at_epoch,
        claims
            .expires_at_epoch
            .map_or_else(|| "none".into(), |value| value.to_string()),
        claims.parent_receipt_digest.as_deref().unwrap_or("none")
    )
}

fn hmac_digest(key: &[u8], message: &[u8]) -> String {
    let mut block = [0_u8; HMAC_BLOCK_BYTES];
    if key.len() > HMAC_BLOCK_BYTES {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner_key = [0x36_u8; HMAC_BLOCK_BYTES];
    let mut outer_key = [0x5c_u8; HMAC_BLOCK_BYTES];
    for index in 0..HMAC_BLOCK_BYTES {
        inner_key[index] ^= block[index];
        outer_key[index] ^= block[index];
    }
    let mut inner = Sha256::new();
    inner.update(inner_key);
    inner.update(message);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_key);
    outer.update(inner_digest);
    format_digest(outer.finalize())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn format_digest(bytes: impl AsRef<[u8]>) -> String {
    format!("sha256:{}", hex(bytes.as_ref()))
}
