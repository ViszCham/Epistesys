//! V2の検証済みapplication観測用記録。永続bytesはlive tokenでない。
//! V2 application-observation record; persisted bytes never reconstruct a live token.
use super::codec::{decode, encode, Event, Kind, PAYLOAD_BYTES};
use super::run_binding::digest_bytes;
use super::{nonzero, BudgetError};
use lc631_substrate::Digest32;

pub(super) const VERIFIED_BYTES: usize = 928;
#[derive(Clone, Copy)]
pub(super) struct VerifiedEvent {
    pub base: Event,
    /// exact request, plan, preimage, actual application identity, actual run,
    /// accepted receipt, actual after-lease, signed run binding receipt.
    pub pins: [Digest32; 8],
    pub checked_epoch: u64,
}
pub(super) fn encode_verified(e: VerifiedEvent) -> [u8; VERIFIED_BYTES] {
    let mut out = [0_u8; VERIFIED_BYTES];
    out[..8].copy_from_slice(b"ETBUD002");
    out[8..10].copy_from_slice(&2_u16.to_le_bytes());
    out[10..12].copy_from_slice(&4_u16.to_le_bytes());
    out[12..16].copy_from_slice(&(VERIFIED_BYTES as u32).to_le_bytes());
    out[16..16 + PAYLOAD_BYTES].copy_from_slice(&encode(e.base));
    let mut at = 16 + PAYLOAD_BYTES;
    for d in e.pins {
        out[at..at + 32].copy_from_slice(d.as_bytes());
        at += 32;
    }
    out[at..at + 8].copy_from_slice(&e.checked_epoch.to_le_bytes());
    out
}
pub(super) fn decode_verified(bytes: &[u8]) -> Result<VerifiedEvent, BudgetError> {
    if bytes.len() != VERIFIED_BYTES
        || &bytes[..8] != b"ETBUD002"
        || bytes[8..10] != 2_u16.to_le_bytes()
        || bytes[10..12] != 4_u16.to_le_bytes()
        || bytes[12..16] != (VERIFIED_BYTES as u32).to_le_bytes()
    {
        return Err(BudgetError::UnsupportedPayload);
    }
    let base = decode(&bytes[16..16 + PAYLOAD_BYTES])?;
    if base.kind != Kind::SettledNeedsFreshValidation {
        return Err(BudgetError::InvalidPayload);
    }
    let mut pins = [Digest32::from_bytes([0; 32]); 8];
    let mut at = 16 + PAYLOAD_BYTES;
    for p in &mut pins {
        *p = Digest32::from_bytes(
            bytes[at..at + 32]
                .try_into()
                .map_err(|_| BudgetError::InvalidPayload)?,
        );
        at += 32;
    }
    let checked_epoch = u64::from_le_bytes(
        bytes[at..at + 8]
            .try_into()
            .map_err(|_| BudgetError::InvalidPayload)?,
    );
    if !pins.into_iter().all(nonzero)
        || pins[0] != base.proposal.reservation
        || pins[5] != base.report
        || pins[7] != base.proposal.intent_binding
    {
        return Err(BudgetError::BindingMismatch);
    }
    Ok(VerifiedEvent {
        base,
        pins,
        checked_epoch,
    })
}
pub(super) fn verified_digest(bytes: &[u8]) -> Digest32 {
    digest_bytes(
        b"Epistesys/TLCX/RepairBudget/VerifiedTransition/v2\0",
        bytes,
    )
}
