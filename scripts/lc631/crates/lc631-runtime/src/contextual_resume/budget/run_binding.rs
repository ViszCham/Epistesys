//! 署名されたrun/request予算対応。作用権限を発行しない。
//! Signed run/request budget binding; never an execution permit.
use super::{nonzero, BudgetBinding, BudgetCaps, BudgetError};
use lc631_analysis::{dgcl_repair_request_digest, DgclRepairRequest};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
    UntrustedReceipt, VerifiedReceipt,
};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BudgetRequestSpec {
    pub request_digest: Digest32,
    pub before_snapshot: Digest32,
    pub intended_snapshot: Digest32,
    pub reserved_delta_bytes: u64,
}

/// Untrusted signing material. Verification below is the only checked constructor.
pub struct BudgetRunManifest {
    pub binding: BudgetBinding,
    pub plan: Digest32,
    pub caps: BudgetCaps,
    pub requests: Vec<BudgetRequestSpec>,
}

/// Checked values cannot be assembled from report fields.
/// ```compile_fail
/// use lc631_runtime::contextual_resume::budget::CheckedBudgetRun;
/// let _: CheckedBudgetRun<'_> = CheckedBudgetRun {};
/// ```
pub struct CheckedBudgetRun<'a> {
    manifest: BudgetRunManifest,
    receipt: VerifiedReceipt,
    verifier: &'a ReceiptVerifier,
    digest: Digest32,
}

pub(super) fn parse_digest(value: &str) -> Result<Digest32, BudgetError> {
    let value = value
        .strip_prefix("sha256:")
        .ok_or(BudgetError::BindingMismatch)?;
    if value.len() != 64 {
        return Err(BudgetError::BindingMismatch);
    }
    let mut bytes = [0_u8; 32];
    fn hex(x: u8) -> Result<u8, BudgetError> {
        match x {
            b'0'..=b'9' => Ok(x - b'0'),
            b'a'..=b'f' => Ok(x - b'a' + 10),
            _ => Err(BudgetError::BindingMismatch),
        }
    }
    for (slot, pair) in bytes.iter_mut().zip(value.as_bytes().as_chunks::<2>().0) {
        *slot = (hex(pair[0])? << 4) | hex(pair[1])?;
    }
    Ok(Digest32::from_bytes(bytes))
}
pub(super) fn text_digest(value: Digest32) -> String {
    let mut out = String::from("sha256:");
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for b in value.as_bytes() {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 15) as usize] as char);
    }
    out
}
pub(super) fn digest_bytes(domain: &[u8], bytes: &[u8]) -> Digest32 {
    let mut h = Sha256::new();
    h.update(domain);
    h.update(bytes);
    Digest32::from_bytes(h.finalize().into())
}

impl BudgetRunManifest {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, BudgetError> {
        self.binding.check()?;
        self.caps.check()?;
        if !nonzero(self.plan)
            || self.requests.is_empty()
            || self.requests.len() > 4
            || self.requests.len() as u64 > self.caps.attempts
        {
            return Err(BudgetError::InvalidBinding);
        }
        let mut out = Vec::new();
        out.try_reserve_exact(1024)
            .map_err(|_| BudgetError::InvalidBinding)?;
        out.extend_from_slice(b"ETBRUN02");
        out.extend_from_slice(&2_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        for d in self.binding.values() {
            out.extend_from_slice(d.as_bytes());
        }
        out.extend_from_slice(self.plan.as_bytes());
        for n in [
            self.caps.attempts,
            self.caps.per_attempt_delta_bytes,
            self.caps.total_delta_bytes,
            self.requests.len() as u64,
        ] {
            out.extend_from_slice(&n.to_le_bytes());
        }
        let mut expected = self.binding.initial_snapshot;
        let mut total = 0_u64;
        for (i, r) in self.requests.iter().enumerate() {
            if ![r.request_digest, r.before_snapshot, r.intended_snapshot]
                .into_iter()
                .all(nonzero)
                || r.before_snapshot != expected
                || r.reserved_delta_bytes == 0
                || r.reserved_delta_bytes > self.caps.per_attempt_delta_bytes
                || self.requests[..i]
                    .iter()
                    .any(|other| other.request_digest == r.request_digest)
            {
                return Err(BudgetError::InvalidBinding);
            }
            total = total
                .checked_add(r.reserved_delta_bytes)
                .ok_or(BudgetError::BudgetExhausted)?;
            if total > self.caps.total_delta_bytes {
                return Err(BudgetError::BudgetExhausted);
            }
            for d in [r.request_digest, r.before_snapshot, r.intended_snapshot] {
                out.extend_from_slice(d.as_bytes());
            }
            out.extend_from_slice(&r.reserved_delta_bytes.to_le_bytes());
            expected = r.intended_snapshot;
        }
        Ok(out)
    }
    pub fn payload_digest(&self) -> Result<String, BudgetError> {
        Ok(text_digest(digest_bytes(
            b"Epistesys/TLCX/RepairBudget/RunBinding/v2\0",
            &self.canonical_bytes()?,
        )))
    }
    pub fn receipt_scope(&self) -> String {
        format!(
            "tlcx/repair-budget/run/v2/{}",
            text_digest(self.binding.run_family)
        )
    }
}

impl<'a> CheckedBudgetRun<'a> {
    pub fn verify(
        manifest: BudgetRunManifest,
        receipt: UntrustedReceipt,
        verifier: &'a ReceiptVerifier,
        replay: &mut ReplayGuard,
        now_epoch: u64,
    ) -> Result<Self, BudgetError> {
        let payload = manifest.payload_digest()?;
        let policy = ReceiptPolicy::exact(
            ReceiptClass::Validation,
            SubjectRevision::checked(text_digest(manifest.binding.source))
                .map_err(|_| BudgetError::BindingMismatch)?,
            ReceiptScope::checked(manifest.receipt_scope())
                .map_err(|_| BudgetError::BindingMismatch)?,
            &payload,
            now_epoch,
        );
        let receipt = verifier
            .verify(receipt, &policy, replay)
            .map_err(|_| BudgetError::BindingMismatch)?;
        if receipt.expires_at_epoch().is_none() {
            return Err(BudgetError::BindingMismatch);
        }
        let digest = parse_digest(&receipt.receipt_digest())?;
        Ok(Self {
            manifest,
            receipt,
            verifier,
            digest,
        })
    }
    pub(super) fn check_at(&self, now: u64) -> Result<(), BudgetError> {
        if self.receipt.verifier_key_fingerprint() != self.verifier.key_fingerprint()
            || now < self.receipt.issued_at_epoch()
            || self.receipt.expires_at_epoch().is_none_or(|end| now >= end)
        {
            return Err(BudgetError::BindingMismatch);
        }
        Ok(())
    }
    pub(super) fn binding(&self) -> BudgetBinding {
        self.manifest.binding
    }
    pub(super) fn caps(&self) -> BudgetCaps {
        self.manifest.caps
    }
    pub(super) fn plan(&self) -> Digest32 {
        self.manifest.plan
    }
    pub(super) fn digest(&self) -> Digest32 {
        self.digest
    }
    pub(super) fn request(&self, r: &DgclRepairRequest) -> Result<BudgetRequestSpec, BudgetError> {
        if r.attempt == 0
            || r.attempt as usize > self.manifest.requests.len()
            || [
                r.gap_id.as_str(),
                r.source_revision.as_str(),
                r.plan_digest.as_str(),
                r.target_ref.as_str(),
                r.target_digest_before.as_str(),
                r.candidate_digest.as_str(),
                r.authorization_scope.as_str(),
            ]
            .into_iter()
            .any(|s| s.len() > 1024 || s.is_empty() || s.chars().any(char::is_control))
            || parse_digest(&r.source_revision)? != self.manifest.binding.source
            || parse_digest(&r.plan_digest)? != self.manifest.plan
            || r.repository_root_digest
                .as_deref()
                .map(parse_digest)
                .transpose()?
                != Some(self.manifest.binding.root_target)
            || r.authorization_span.start > r.authorization_span.end
        {
            return Err(BudgetError::BindingMismatch);
        }
        let spec = self.manifest.requests[r.attempt as usize - 1];
        if spec.request_digest != parse_digest(&dgcl_repair_request_digest(r))? {
            return Err(BudgetError::BindingMismatch);
        }
        Ok(spec)
    }
}
