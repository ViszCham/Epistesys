//! 登録済み credential の失効・期限・registry 高水位を profile 内で共有する。
//! A previous expiry or revocation remains recorded across endpoint checks.
use super::{ClockFault, ClockGap, Error, RegistryAuthority, RegistryView};
use lc631_receipt_kernel::ReplayGuard;
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

pub(super) fn unavailable() -> ClockFault {
    ClockFault::NeedsEvidence(ClockGap::CurrentRegistryUnavailable)
}
pub(super) fn canonical_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
}

/// A configured actor may omit/repeat the callback, suppress its error, or fail
/// after invoking it. Only one invocation AND successful outer return publishes.
pub(super) fn with_registry<T>(
    authority: &dyn RegistryAuthority,
    verify: impl FnOnce(RegistryView<'_>) -> Result<T, ClockFault>,
) -> Result<T, ClockFault> {
    let mut verify = Some(verify);
    let mut first = None;
    let mut repeated = false;
    let outer = authority.with_current(&mut |view| {
        let Some(verify) = verify.take() else {
            repeated = true;
            return Err(Error::Clock(unavailable()));
        };
        let checked = verify(view);
        let result = checked
            .as_ref()
            .map(|_| ())
            .map_err(|fault| Error::Clock(fault.clone()));
        first = Some(checked);
        result
    });
    // Keep the actual first verification error even if the actor swallowed it.
    let value = first.ok_or_else(unavailable)??;
    if repeated || outer.is_err() {
        return Err(unavailable());
    }
    Ok(value)
}

struct Watermark {
    now: u64,
    revision: u64,
    revocation: u64,
    set_digest: Digest32,
}
struct RegisteredValidity {
    digest: String,
    expiry: u64,
    revoked: bool,
    expired: bool,
}

pub(super) struct RegistryHistory {
    pub(super) replay: ReplayGuard,
    floor: Option<Watermark>,
    known: Vec<RegisteredValidity>,
    max_known: usize,
}
impl RegistryHistory {
    pub(super) fn new(attempts: u32, endpoints: u32) -> Result<Self, Error> {
        let replay_limit = usize::try_from(attempts).map_err(|_| Error::InputLimit)?;
        let max_known = usize::try_from(endpoints.min(attempts)).map_err(|_| Error::InputLimit)?;
        let replay = ReplayGuard::with_capacity(replay_limit).map_err(|_| Error::InputLimit)?;
        let mut known = Vec::new();
        known.try_reserve_exact(max_known).map_err(|_| {
            Error::Clock(ClockFault::Resource(
                lc631_substrate::resource::ResourceError::AllocationFailed,
            ))
        })?;
        Ok(Self {
            replay,
            floor: None,
            known,
            max_known,
        })
    }

    pub(super) fn observe(
        &mut self,
        view: &RegistryView<'_>,
        expected_key: &str,
        max_metadata: usize,
    ) -> Result<(), ClockFault> {
        if view.verifier.key_fingerprint() != expected_key {
            return Err(ClockFault::NeedsEvidence(ClockGap::UntrustedIssuer));
        }
        let set_digest = revocation_set(view.revoked_receipt_digests, max_metadata)?;
        if let Some(floor) = &self.floor {
            if view.now_epoch < floor.now
                || view.revision < floor.revision
                || view.revocation_revision < floor.revocation
                || (view.revocation_revision == floor.revocation && set_digest != floor.set_digest)
            {
                // Registry-control time is not a sample from a role's raw clock.
                return Err(unavailable());
            }
        }
        self.floor = Some(Watermark {
            now: view.now_epoch,
            revision: view.revision,
            revocation: view.revocation_revision,
            set_digest,
        });
        for known in &mut self.known {
            known.expired |= view.now_epoch >= known.expiry;
            known.revoked |= view.revoked_receipt_digests.contains(&known.digest);
        }
        Ok(())
    }

    pub(super) fn add(&mut self, digest: String, expiry: u64) -> Result<usize, ClockFault> {
        if self.known.len() >= self.max_known {
            return Err(ClockFault::BudgetExhausted);
        }
        let index = self.known.len();
        self.known.push(RegisteredValidity {
            digest,
            expiry,
            revoked: false,
            expired: false,
        });
        Ok(index)
    }

    pub(super) fn check_known(&self, index: usize) -> Result<(), ClockFault> {
        let known = self.known.get(index).ok_or(ClockFault::BindingMismatch)?;
        if known.revoked {
            return Err(ClockFault::RegistrationRevoked);
        }
        if known.expired {
            return Err(ClockFault::RegistrationExpired);
        }
        Ok(())
    }
}

/// Maximum cap admits fewer than 58 canonical 71-byte digests. Sort only this
/// bounded borrowed set so transport ordering cannot change revocation meaning.
fn revocation_set(values: &[String], max_metadata: usize) -> Result<Digest32, ClockFault> {
    const MAX_DIGESTS: usize = 4096 / 71;
    if values.len() > MAX_DIGESTS || values.len() > max_metadata / 71 {
        return Err(ClockFault::BudgetExhausted);
    }
    let mut sorted = [""; MAX_DIGESTS];
    for (slot, value) in sorted.iter_mut().zip(values) {
        if !canonical_digest(value) {
            return Err(unavailable());
        }
        *slot = value.as_str();
    }
    let sorted = &mut sorted[..values.len()];
    sorted.sort_unstable();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(unavailable());
    }
    let mut hash = Sha256::new();
    hash.update(b"epistesys.registered-clock.revocations.v1\0");
    for value in sorted.iter() {
        hash.update(value.as_bytes());
    }
    Ok(Digest32::from_bytes(hash.finalize().into()))
}
