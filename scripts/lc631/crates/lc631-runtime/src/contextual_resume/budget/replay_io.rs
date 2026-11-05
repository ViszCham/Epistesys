//! 実readerのbudget replay。writer lockとhead比較はlive ownerが保持する。
//! Exact reader replay; the live owner retains the writer lock and head comparisons.
use super::codec::{decode, payload_digest, PAYLOAD_BYTES};
use super::state::{genesis_head, replay, State};
use super::verified_codec::{decode_verified, verified_digest, VERIFIED_BYTES};
use super::BudgetPhase;
use super::{BudgetError, BudgetHead};
use crate::journal::{decode_segment, JournalCursor, JournalRecordKind, RECORD_HEADER_BYTES};
use lc631_host::{BundlePart, HostJournalBundleReader, JournalBundleLimits};
use lc631_substrate::codec::decode_envelope_borrowed;
use lc631_substrate::resource::{
    ResourceAmount, ResourceKind, ResourceRequest, ResourceReservationView,
};
use std::path::Path;

pub(super) fn replay_root<'a>(
    root: &Path,
    limits: JournalBundleLimits,
    resources: &'a ResourceReservationView<'a>,
) -> Result<(State, BudgetHead), BudgetError> {
    let amount = ResourceAmount::checked(65_536).map_err(BudgetError::Resource)?;
    let _work = resources
        .claim_resources(&[
            ResourceRequest::new(ResourceKind::Ram, amount),
            ResourceRequest::new(ResourceKind::Output, amount),
        ])
        .map_err(BudgetError::Resource)?;
    let mut reader = HostJournalBundleReader::open_existing(root, limits, resources)
        .map_err(BudgetError::Host)?;
    let inventory = reader.inventory().map_err(BudgetError::Host)?;
    if inventory.items().is_empty() || inventory.items().len() > 9 {
        return Err(BudgetError::MissingHistory);
    }
    let mut state = None;
    let mut head = genesis_head();
    let mut cursor = JournalCursor::genesis();
    for item in inventory.items() {
        if item.sequence()
            != head
                .sequence
                .checked_add(1)
                .ok_or(BudgetError::SequenceMismatch)?
        {
            return Err(BudgetError::SequenceMismatch);
        }
        let owned = reader
            .read_part(item.sequence(), BundlePart::Segment)
            .map_err(BudgetError::Host)?;
        // This profile has one fixed event per segment. Reject before decoding
        // record metadata, not after materializing arbitrary journal payloads.
        if ![PAYLOAD_BYTES, VERIFIED_BYTES]
            .into_iter()
            .any(|n| owned.as_bytes().len() == 64 + 32 + RECORD_HEADER_BYTES + n)
        {
            return Err(BudgetError::UnsupportedPayload);
        }
        let segment = decode_segment(owned.as_bytes()).map_err(BudgetError::Journal)?;
        segment
            .verify_predecessor(cursor)
            .map_err(BudgetError::Journal)?;
        if segment.records().len() != 1 {
            return Err(BudgetError::UnsupportedPayload);
        }
        let record = segment
            .records()
            .first()
            .ok_or(BudgetError::MissingHistory)?;
        if record.kind() != JournalRecordKind::Evidence {
            return Err(BudgetError::UnsupportedPayload);
        }
        if record.payload().len() == VERIFIED_BYTES {
            if record.object_digest() != verified_digest(record.payload()) {
                return Err(BudgetError::InvalidPayload);
            }
            let event = decode_verified(record.payload())?;
            let mut next = replay(state, event.base, head, record.digest())?;
            // Structural traversal of subsequent quota records only. This is
            // downgraded at the public recovery boundary below, never a token.
            next.summary.phase = BudgetPhase::QuotaReadyRequiresFreshAuthority;
            state = Some(next);
        } else {
            if record.object_digest() != payload_digest(record.payload()) {
                return Err(BudgetError::UnsupportedPayload);
            }
            let event = decode(record.payload())?;
            state = Some(replay(state, event, head, record.digest())?);
        }
        cursor = segment.cursor();
        let raw_head = reader
            .read_part(item.sequence(), BundlePart::LocalHead)
            .map_err(BudgetError::Host)?;
        let frame = decode_envelope_borrowed(raw_head.as_bytes()).map_err(BudgetError::Codec)?;
        head = BudgetHead {
            sequence: cursor.sequence(),
            record_digest: cursor.record_digest(),
            local_head_frame_digest: frame.digest(),
        };
    }
    reader.validate_snapshot().map_err(BudgetError::Host)?;
    let mut state = state.ok_or(BudgetError::MissingHistory)?;
    if state.summary.phase == BudgetPhase::QuotaReadyRequiresFreshAuthority {
        state.summary.phase = BudgetPhase::SettledNeedsFreshValidation;
    }
    Ok((state, head))
}
