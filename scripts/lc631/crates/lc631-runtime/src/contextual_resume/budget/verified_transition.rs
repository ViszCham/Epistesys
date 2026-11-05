//! concrete wrapperのprivate pendingと31 tokenを同時消費する狭い境界。
//! Consumes concrete-wrapper private pending state together with the real31 token.
use super::super::file_driver::{actual_identity, PendingFileApplication};
use super::super::run_binding::{parse_digest, CheckedBudgetRun};
use super::super::state::{replay, State};
use super::super::verified_codec::{encode_verified, verified_digest, VerifiedEvent};
use super::super::{BudgetError, BudgetHead, BudgetPhase, ReportedSettlementProposal};
use super::{BudgetSession, PersistedBudgetReservation};
use crate::journal::{decode_segment, JournalRecordKind, JournalSegmentBuilder};
use lc631_analysis::{
    validate_dgcl_evidence_lease, DgclLifecycleState, DgclVerifiedRepairApplication,
};
use lc631_substrate::resource::{ResourceAmount, ResourceKind, ResourceRequest};

impl BudgetSession<'_> {
    /// Only budget's concrete wrapper can construct PendingFileApplication.
    /// No public raw report/receipt/hash admission method exists.
    pub(in super::super) fn finish_concrete_application(
        &mut self,
        pending: PendingFileApplication,
        token: DgclVerifiedRepairApplication<'_>,
        run: &CheckedBudgetRun<'_>,
        now: u64,
    ) -> Result<(), BudgetError> {
        self.resources.check().map_err(BudgetError::Resource)?;
        run.check_at(now)?;
        let (ticket, spec, identity, preimage) = pending.into_parts();
        let state = self.state.ok_or(BudgetError::Poisoned)?;
        check_ticket(&ticket, self.owner, state)?;
        if state.summary.binding != run.binding() || state.summary.caps != run.caps() {
            return Err(BudgetError::BindingMismatch);
        }
        let r = token.request();
        let a = token.application();
        if now < token.checked_epoch()
            || run.request(r)? != spec
            || actual_identity(a)? != identity
            || parse_digest(&r.target_digest_before)? != preimage
            || parse_digest(&a.target_digest_before)? != preimage
            || parse_digest(&a.source_revision)? != run.binding().source
            || parse_digest(&a.plan_digest)? != run.plan()
            || parse_digest(&a.request_digest)? != spec.request_digest
            || parse_digest(&a.candidate_digest)? != ticket.proposal.candidate
            || parse_digest(&a.target_digest_after)? != ticket.proposal.candidate
            || parse_digest(&a.permit_digest)? != ticket.proposal.authority_binding
            || parse_digest(&a.after_snapshot.snapshot_digest)? != spec.intended_snapshot
            || a.delta_bytes > spec.reserved_delta_bytes
            || a.attestation
                .as_ref()
                .ok_or(BudgetError::BindingMismatch)?
                .receipt_digest()
                != token.validated_receipt_digest()
            || validate_dgcl_evidence_lease(&a.after_lease, &a.after_snapshot, now).state
                != DgclLifecycleState::Current
        {
            return Err(BudgetError::BindingMismatch);
        }
        let accepted = parse_digest(token.validated_receipt_digest())?;
        let base = state.settle(
            self.head,
            ReportedSettlementProposal {
                observed_snapshot: spec.intended_snapshot,
                report_digest: accepted,
            },
        )?;
        let event = VerifiedEvent {
            base,
            pins: [
                spec.request_digest,
                run.plan(),
                preimage,
                identity,
                parse_digest(&a.run_digest)?,
                accepted,
                parse_digest(&a.after_lease.lease_digest)?,
                run.digest(),
            ],
            checked_epoch: token.checked_epoch(),
        };
        self.persist_verified(event)
    }

    fn persist_verified(&mut self, event: VerifiedEvent) -> Result<(), BudgetError> {
        let previous = self.state.take();
        let result = (|| {
            self.resources.check().map_err(BudgetError::Resource)?;
            let amount = ResourceAmount::checked(65_536).map_err(BudgetError::Resource)?;
            let _claim = self
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
            let bytes = encode_verified(event);
            let mut builder = JournalSegmentBuilder::after(cursor);
            builder
                .append_at(
                    cursor
                        .sequence()
                        .checked_add(1)
                        .ok_or(BudgetError::SequenceMismatch)?,
                    JournalRecordKind::Evidence,
                    verified_digest(&bytes),
                    &bytes,
                )
                .map_err(BudgetError::Journal)?;
            let frame = builder.finish().map_err(BudgetError::Journal)?;
            let record = decode_segment(&frame)
                .map_err(BudgetError::Journal)?
                .cursor()
                .record_digest();
            let mut next = replay(previous, event.base, self.head, record)?;
            let receipt = self
                .writer
                .append(cursor, &frame)
                .map_err(BudgetError::Bundle)?;
            if receipt.head().record_digest() != record {
                return Err(BudgetError::HeadMismatch);
            }
            // Live observation advances quota bookkeeping only. Replay does not
            // recreate this live observation or any permission/application ticket.
            next.summary.phase = BudgetPhase::QuotaReadyRequiresFreshAuthority;
            self.state = Some(next);
            self.head = BudgetHead {
                sequence: receipt.head().sequence(),
                record_digest: record,
                local_head_frame_digest: receipt.head_frame_digest(),
            };
            Ok(())
        })();
        if result.is_err() {
            self.state = None;
        }
        result
    }
}
fn check_ticket(t: &PersistedBudgetReservation, owner: u64, s: State) -> Result<(), BudgetError> {
    if t.owner != owner {
        return Err(BudgetError::ForeignTicket);
    }
    if t.binding != s.summary.binding
        || t.proposal != s.pending
        || t.reservation_record != s.reservation_record
    {
        return Err(BudgetError::TicketMismatch);
    }
    Ok(())
}
