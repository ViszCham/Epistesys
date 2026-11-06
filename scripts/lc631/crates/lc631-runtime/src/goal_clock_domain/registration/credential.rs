//! 署名は actual actor の occurrence と固定 binding を対象に検査する。
//! Verification binds one actor occurrence, its typed inputs and exact receipt class.
use super::{
    profile::ProfileState, registry, ClockFault, ClockGap, EndpointBinding, Error, Profile,
    Purpose, RegistryAuthority, Role,
};
use lc631_receipt_kernel::{
    digest_bytes, generate_key, ReceiptClass, ReceiptError, ReceiptPolicy, ReceiptScope,
    ReplayGuard, SubjectRevision, UntrustedReceipt, VerifiedReceipt,
};
use lc631_substrate::resource::{ResourceError, ResourceReservationView};
use sha2::{Digest, Sha256};

pub(super) struct Identity {
    subject: String,
    scope: &'static str,
    payload: String,
}
impl Identity {
    pub(super) fn new(
        profile: &Profile<'_>,
        binding: EndpointBinding,
        view: &ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let scope = match (binding.role, binding.purpose) {
            (Role::Baseline, Purpose::Execution) => "clock/epoch-seconds-v1/baseline/execution",
            (Role::Baseline, Purpose::Capture) => "clock/epoch-seconds-v1/baseline/capture",
            (Role::Candidate, Purpose::Execution) => "clock/epoch-seconds-v1/candidate/execution",
            (Role::Candidate, Purpose::Capture) => "clock/epoch-seconds-v1/candidate/capture",
        };
        // Two canonical digest strings are 71 bytes each. Check before minting
        // the occurrence and allocating any string representation.
        if scope.len() + 2 * 71 > profile.metadata_limit() {
            return Err(Error::InputLimit);
        }
        // Reuse the receipt kernel's OS random-byte source for an occurrence,
        // not an issuer or a key handed to the candidate. Never fall back to a
        // restartable counter, a function pointer or a caller-provided label.
        let nonce = generate_key().map_err(|_| Error::Clock(registry::unavailable()))?;
        let subject = digest_bytes(&nonce);
        let mut hash = Sha256::new();
        hash.update(b"epistesys.registered-clock.binding.v1\0");
        hash.update(nonce);
        hash.update(scope.as_bytes());
        hash.update([0]);
        hash.update(profile.state.fingerprint.as_bytes());
        hash.update([0]);
        for pin in [
            profile.domain(),
            binding.root,
            binding.source,
            binding.program,
            binding.completion_plan,
            binding.probe_plan,
            binding.endpoint_revision,
        ] {
            hash.update(pin.as_bytes());
        }
        // IDs are additional local context, not a substitute for exact live
        // ResourceClaim::same_reservation checks.
        for n in [view.owner_id(), view.run_id(), view.reservation_id()] {
            hash.update(n.to_le_bytes());
        }
        let mut payload = String::with_capacity(71);
        payload.push_str("sha256:");
        for byte in hash.finalize().iter() {
            use std::fmt::Write as _;
            let _ = write!(payload, "{byte:02x}");
        }
        Ok(Self {
            subject,
            scope,
            payload,
        })
    }
    pub(super) fn subject(&self) -> &str {
        &self.subject
    }
    pub(super) fn scope(&self) -> &str {
        self.scope
    }
    pub(super) fn payload(&self) -> &str {
        &self.payload
    }

    fn policy(&self, now: u64) -> Result<ReceiptPolicy, ClockFault> {
        let subject = SubjectRevision::checked(self.subject.clone()).map_err(classify)?;
        let scope = ReceiptScope::checked(self.scope).map_err(classify)?;
        Ok(ReceiptPolicy::exact(
            ReceiptClass::ToolExecution,
            subject,
            scope,
            &self.payload,
            now,
        ))
    }
}

pub(super) struct Credential {
    proof: VerifiedReceipt,
    slot: usize,
    registry_revision: u64,
    revocation_bytes: usize,
}
impl Credential {
    pub(super) fn admit(
        profile: &ProfileState<'_>,
        identity: &Identity,
        receipt: UntrustedReceipt,
        registry: &dyn RegistryAuthority,
    ) -> Result<Self, Error> {
        receipt.check_size_bounds().map_err(|_| Error::InputLimit)?;
        let bytes = wire_metadata(&receipt)
            .checked_add(identity.subject.len() + identity.scope.len() + identity.payload.len())
            .ok_or(Error::InputLimit)?;
        let revocation_bytes = profile
            .limits
            .max_metadata_bytes
            .checked_sub(bytes)
            .ok_or(Error::InputLimit)?;
        registry::with_registry(registry, |view| {
            let mut history = profile.registry.lock().map_err(|_| poisoned())?;
            history.observe(&view, &profile.fingerprint, revocation_bytes)?;
            let policy = identity.policy(view.now_epoch)?;
            let proof = view
                .verifier
                .verify(receipt, &policy, &mut history.replay)
                .map_err(classify)?;
            let expiry = proof
                .expires_at_epoch()
                .ok_or(ClockFault::NeedsEvidence(ClockGap::RegistrationMissing))?;
            let digest = proof.receipt_digest();
            if view.revoked_receipt_digests.contains(&digest) {
                return Err(ClockFault::RegistrationRevoked);
            }
            let slot = history.add(digest, expiry)?;
            Ok(Self {
                proof,
                slot,
                registry_revision: view.revision,
                revocation_bytes,
            })
        })
        .map_err(Error::Clock)
    }

    pub(super) fn check(
        &self,
        profile: &ProfileState<'_>,
        registry: &dyn RegistryAuthority,
    ) -> Result<(), ClockFault> {
        // Once observed, expiry/revocation is terminal for this occurrence even
        // if the registry later presents an older or incomplete view.
        profile
            .registry
            .lock()
            .map_err(|_| poisoned())?
            .check_known(self.slot)?;
        registry::with_registry(registry, |view| {
            let mut history = profile.registry.lock().map_err(|_| poisoned())?;
            history.observe(&view, &profile.fingerprint, self.revocation_bytes)?;
            history.check_known(self.slot)?;
            if self.registry_revision != view.revision {
                return Err(registry::unavailable());
            }
            let policy = ReceiptPolicy::exact(
                ReceiptClass::ToolExecution,
                self.proof.subject_revision().clone(),
                self.proof.scope().clone(),
                self.proof.payload_digest(),
                view.now_epoch,
            );
            // Confirm this existing immutable registration against the current
            // verifier, including its principal. Its nonce was spent in the
            // profile's persistent-in-memory admission replay guard. This
            // one-entry verification context issues no new endpoint or slot.
            let mut check = ReplayGuard::with_capacity(1).map_err(classify)?;
            view.verifier
                .verify(self.proof.clone().into_untrusted(), &policy, &mut check)
                .map_err(classify)?;
            Ok(())
        })
    }
}

fn wire_metadata(receipt: &UntrustedReceipt) -> usize {
    // check_size_bounds ran first: at most 2,048 bytes of bounded string fields.
    // Account for schema, nonce, MAC, two epoch values and the optional parent.
    let c = receipt.claims();
    lc631_receipt_kernel::RECEIPT_SCHEMA.len()
        + 32
        + 71
        + 16
        + c.class().as_str().len()
        + c.issuer().as_str().len()
        + c.subject_revision().as_str().len()
        + c.scope().as_str().len()
        + c.payload_digest().len()
        + c.parent_receipt_digest().map_or(0, str::len)
}
fn poisoned() -> ClockFault {
    ClockFault::Resource(ResourceError::Poisoned)
}
fn classify(error: ReceiptError) -> ClockFault {
    match error {
        ReceiptError::MacMismatch | ReceiptError::IssuerMismatch | ReceiptError::WeakKey => {
            ClockFault::NeedsEvidence(ClockGap::UntrustedIssuer)
        }
        ReceiptError::Expired => ClockFault::RegistrationExpired,
        ReceiptError::NotYetValid | ReceiptError::RandomUnavailable => registry::unavailable(),
        ReceiptError::ReplayCapacityExceeded | ReceiptError::InvalidReplayCapacity => {
            ClockFault::BudgetExhausted
        }
        ReceiptError::ClassMismatch
        | ReceiptError::InvalidDigest
        | ReceiptError::InvalidExpiry
        | ReceiptError::InvalidField(_)
        | ReceiptError::InvalidWire
        | ReceiptError::PayloadMismatch
        | ReceiptError::ParentMismatch
        | ReceiptError::ReplayDetected
        | ReceiptError::ScopeMismatch
        | ReceiptError::SubjectMismatch => ClockFault::BindingMismatch,
    }
}
