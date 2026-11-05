//! 実bundle writerのみが予約ticketを発行する。復旧はticketを再発行しない。
//! Only successful real bundle handoff issues tickets; replay never reissues them.
use super::codec::{encode, payload_digest, Event};
use super::replay_io::replay_root;
use super::state::{genesis_head, initialization, replay, BudgetSummary, State};
use super::{
    nonzero, BudgetBinding, BudgetCaps, BudgetError, BudgetHead, ReportedSettlementProposal,
    ReservationProposal,
};
use crate::journal::{
    decode_segment, inspect_bundle_root, JournalBundleLiveSession, JournalBundleReceipt,
    JournalChainLimits, JournalCursor, JournalRecordKind, JournalSegmentBuilder,
};
use lc631_host::{BundlePart, HostError, HostJournalBundleReader, JournalBundleLimits};
use lc631_substrate::resource::{
    ResourceAmount, ResourceKind, ResourceRequest, ResourceReservationView,
};
use lc631_substrate::Digest32;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);
#[path = "verified_transition.rs"]
mod verified_transition;
fn owner_id() -> Result<u64, BudgetError> {
    // Uniqueness only: Relaxed is sufficient; no data is published by this counter.
    NEXT_OWNER
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| BudgetError::OwnerIdExhausted)
}

/// Non-Clone/non-Deserialize local budget ticket; never MutationAuthority.
/// Private owner rejects transplantation even between byte-identical cloned roots.
#[derive(Debug)]
pub struct PersistedBudgetReservation {
    owner: u64,
    binding: BudgetBinding,
    reservation_record: Digest32,
    proposal: ReservationProposal,
}
impl PersistedBudgetReservation {
    pub const fn reservation_record_digest(&self) -> Digest32 {
        self.reservation_record
    }
    pub const fn proposal(&self) -> ReservationProposal {
        self.proposal
    }
    pub const fn claim_boundary(&self) -> &'static str {
        "successful local budget handoff only; not application permission, verified application, proof, completion or delivery"
    }
}

pub struct BudgetSession<'a> {
    writer: JournalBundleLiveSession<'a>,
    resources: &'a ResourceReservationView<'a>,
    state: Option<State>,
    head: BudgetHead,
    owner: u64,
}

fn check_source(
    binding: BudgetBinding,
    caps: BudgetCaps,
    resources: &ResourceReservationView<'_>,
) -> Result<(), BudgetError> {
    binding.check()?;
    caps.check()?;
    resources.check().map_err(BudgetError::Resource)?;
    if resources.source_revision() != binding.source {
        return Err(BudgetError::BindingMismatch);
    }
    Ok(())
}

impl<'a> BudgetSession<'a> {
    /// Explicit new-run declaration only. Never called as a missing-history fallback.
    pub fn initialize_new(
        root: &Path,
        binding: BudgetBinding,
        caps: BudgetCaps,
        planned_bytes: u64,
        store_limits: JournalBundleLimits,
        chain_limits: JournalChainLimits,
        resources: &'a ResourceReservationView<'a>,
    ) -> Result<Self, BudgetError> {
        check_source(binding, caps, resources)?;
        let event = initialization(binding, caps)?;
        let owner = owner_id()?;
        let mut writer = JournalBundleLiveSession::open(
            root,
            planned_bytes,
            store_limits,
            resources,
            chain_limits,
        )
        .map_err(BudgetError::Bundle)?;
        if writer.cursor().map_err(BudgetError::Bundle)? != JournalCursor::genesis() {
            return Err(BudgetError::ExistingHistory);
        }
        let mut session = Self {
            writer,
            resources,
            state: None,
            head: genesis_head(),
            owner,
        };
        session.persist(event, |_| Ok(()))?;
        Ok(session)
    }

    /// Existing marked, nonempty profile only. The writer lock remains owned
    /// throughout structural inspection and exact budget replay.
    #[allow(clippy::too_many_arguments)]
    pub fn resume_existing(
        root: &Path,
        binding: BudgetBinding,
        caps: BudgetCaps,
        expected: BudgetHead,
        planned_bytes: u64,
        store_limits: JournalBundleLimits,
        chain_limits: JournalChainLimits,
        resources: &'a ResourceReservationView<'a>,
    ) -> Result<Self, BudgetError> {
        check_source(binding, caps, resources)?;
        if expected.sequence == 0
            || !nonzero(expected.record_digest)
            || !nonzero(expected.local_head_frame_digest)
        {
            return Err(BudgetError::MissingHistory);
        }
        let owner = owner_id()?;
        // Before opening a writer, missing/legacy/empty roots cannot become a fresh run.
        {
            let mut reader = HostJournalBundleReader::open_existing(root, store_limits, resources)
                .map_err(BudgetError::Host)?;
            if reader.next_sequence().map_err(BudgetError::Host)? == 1 {
                return Err(BudgetError::MissingHistory);
            }
            reader.validate_snapshot().map_err(BudgetError::Host)?;
        }
        let mut writer = JournalBundleLiveSession::open(
            root,
            planned_bytes,
            store_limits,
            resources,
            chain_limits,
        )
        .map_err(BudgetError::Bundle)?;
        let before = inspect_bundle_root(root, store_limits, resources, chain_limits)
            .map_err(BudgetError::Inspection)?;
        let (state, head) = replay_root(root, store_limits, resources)?;
        let after = inspect_bundle_root(root, store_limits, resources, chain_limits)
            .map_err(BudgetError::Inspection)?;
        let cursor = writer.cursor().map_err(BudgetError::Bundle)?;
        if before != after
            || head != expected
            || head.sequence != cursor.sequence()
            || head.record_digest != cursor.record_digest()
            || head.local_head_frame_digest != after.head_frame_digest()
            || after.cursor() != cursor
        {
            return Err(BudgetError::HeadMismatch);
        }
        if state.summary.binding != binding {
            return Err(BudgetError::BindingMismatch);
        }
        if state.summary.caps != caps {
            return Err(BudgetError::CapsMismatch);
        }
        Ok(Self {
            writer,
            resources,
            state: Some(state),
            head,
            owner,
        })
    }

    pub fn summary(&self) -> Result<BudgetSummary, BudgetError> {
        self.state
            .map(|state| state.summary)
            .ok_or(BudgetError::Poisoned)
    }
    pub fn head(&self) -> Result<BudgetHead, BudgetError> {
        self.state.ok_or(BudgetError::Poisoned)?;
        Ok(self.head)
    }

    pub fn reserve_before_apply(
        &mut self,
        proposal: ReservationProposal,
    ) -> Result<PersistedBudgetReservation, BudgetError> {
        self.reserve_before_apply_observed(proposal, |_| Ok(()))
    }

    /// Hook observes actual write/sync/close boundaries, never application proof.
    pub fn reserve_before_apply_observed(
        &mut self,
        proposal: ReservationProposal,
        after_part: impl FnMut(BundlePart) -> Result<(), HostError>,
    ) -> Result<PersistedBudgetReservation, BudgetError> {
        let state = self.state.ok_or(BudgetError::Poisoned)?;
        let event = state.reserve(self.head, proposal)?;
        let receipt = self.persist(event, after_part)?;
        Ok(PersistedBudgetReservation {
            owner: self.owner,
            binding: state.summary.binding,
            reservation_record: receipt.head().record_digest(),
            proposal,
        })
    }

    /// Closes the local budget ticket once, but retains pending application
    /// reconciliation and ALL charged allowance. No verified-settlement port exists.
    pub fn record_reported_settlement(
        &mut self,
        ticket: PersistedBudgetReservation,
        proposal: ReportedSettlementProposal,
    ) -> Result<BudgetSummary, BudgetError> {
        let state = self.state.ok_or(BudgetError::Poisoned)?;
        if ticket.owner != self.owner {
            return Err(BudgetError::ForeignTicket);
        }
        if ticket.binding != state.summary.binding
            || ticket.proposal != state.pending
            || ticket.reservation_record != state.reservation_record
        {
            return Err(BudgetError::TicketMismatch);
        }
        let event = state.settle(self.head, proposal)?;
        self.persist(event, |_| Ok(()))?;
        self.summary()
    }

    fn persist(
        &mut self,
        event: Event,
        after_part: impl FnMut(BundlePart) -> Result<(), HostError>,
    ) -> Result<JournalBundleReceipt, BudgetError> {
        // Invalidate before any fallible work/callback; unwind cannot expose
        // an old Ready state after a possibly persisted reservation.
        let previous = self.state.take();
        let result = (|| {
            self.resources.check().map_err(BudgetError::Resource)?;
            let amount = ResourceAmount::checked(65_536).map_err(BudgetError::Resource)?;
            let _work = self
                .resources
                .claim_resources(&[
                    ResourceRequest::new(ResourceKind::Ram, amount),
                    ResourceRequest::new(ResourceKind::Output, amount),
                ])
                .map_err(BudgetError::Resource)?;
            let cursor = self.writer.cursor().map_err(BudgetError::Bundle)?;
            if cursor.sequence() != self.head.sequence
                || cursor.record_digest() != self.head.record_digest
            {
                return Err(BudgetError::HeadMismatch);
            }
            let payload = encode(event);
            let mut builder = JournalSegmentBuilder::after(cursor);
            builder
                .append_at(
                    cursor
                        .sequence()
                        .checked_add(1)
                        .ok_or(BudgetError::SequenceMismatch)?,
                    JournalRecordKind::Evidence,
                    payload_digest(&payload),
                    &payload,
                )
                .map_err(BudgetError::Journal)?;
            let bytes = builder.finish().map_err(BudgetError::Journal)?;
            let record_digest = decode_segment(&bytes)
                .map_err(BudgetError::Journal)?
                .cursor()
                .record_digest();
            let next = replay(previous, event, self.head, record_digest)?;
            let receipt = self
                .writer
                .append_observed(cursor, &bytes, after_part)
                .map_err(BudgetError::Bundle)?;
            if receipt.head().record_digest() != record_digest {
                return Err(BudgetError::HeadMismatch);
            }
            self.head = BudgetHead {
                sequence: receipt.head().sequence(),
                record_digest,
                local_head_frame_digest: receipt.head_frame_digest(),
            };
            self.state = Some(next);
            Ok(receipt)
        })();
        if result.is_err() {
            self.state = None;
        }
        result
    }
}
