//! 保存された未確認作用は報告settle後も残す。予算返却なし。
//! Reported settlement never resolves application uncertainty or refunds budget.
use super::codec::{empty_proposal, Event, Kind};
use super::{
    nonzero, zero, BudgetBinding, BudgetCaps, BudgetError, BudgetHead, ReportedSettlementProposal,
    ReservationProposal,
};
use lc631_substrate::Digest32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetPhase {
    ReadyForReservation,
    PendingMayHaveApplied,
    SettledNeedsFreshValidation,
    QuotaReadyRequiresFreshAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BudgetSummary {
    pub binding: BudgetBinding,
    pub caps: BudgetCaps,
    pub charged_attempts: u64,
    pub charged_delta_bytes: u64,
    pub phase: BudgetPhase,
}
impl BudgetSummary {
    pub const fn claim_boundary(self) -> &'static str {
        "Unanchored; RollbackResistanceUnavailable; charged budget only; fresh source/authority/application validation required; no mutation permission, proof, completion or delivery"
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct State {
    pub summary: BudgetSummary,
    pub pending: ReservationProposal,
    pub reservation_record: Digest32,
}

pub(super) fn genesis_head() -> BudgetHead {
    BudgetHead {
        sequence: 0,
        record_digest: zero(),
        local_head_frame_digest: zero(),
    }
}

pub(super) fn initialization(
    binding: BudgetBinding,
    caps: BudgetCaps,
) -> Result<Event, BudgetError> {
    binding.check()?;
    caps.check()?;
    Ok(Event {
        kind: Kind::Initialize,
        binding,
        caps,
        prior_sequence: 0,
        prior_record: zero(),
        prior_head_frame: zero(),
        proposal: empty_proposal(),
        report: zero(),
        observed_snapshot: zero(),
        charged_attempts: 0,
        charged_delta: 0,
    })
}

impl State {
    fn event(
        self,
        kind: Kind,
        head: BudgetHead,
        proposal: ReservationProposal,
        report: Digest32,
        observed_snapshot: Digest32,
    ) -> Event {
        Event {
            kind,
            binding: self.summary.binding,
            caps: self.summary.caps,
            prior_sequence: head.sequence,
            prior_record: head.record_digest,
            prior_head_frame: head.local_head_frame_digest,
            proposal,
            report,
            observed_snapshot,
            charged_attempts: self.summary.charged_attempts,
            charged_delta: self.summary.charged_delta_bytes,
        }
    }

    pub fn reserve(self, head: BudgetHead, p: ReservationProposal) -> Result<Event, BudgetError> {
        if !matches!(
            self.summary.phase,
            BudgetPhase::ReadyForReservation | BudgetPhase::QuotaReadyRequiresFreshAuthority
        ) {
            return Err(BudgetError::ReconciliationRequired);
        }
        if ![
            p.reservation,
            p.candidate,
            p.before_snapshot,
            p.intended_after_snapshot,
            p.authority_binding,
            p.intent_binding,
        ]
        .into_iter()
        .all(nonzero)
            || p.delta_bytes == 0
            || p.before_snapshot
                != if self.summary.phase == BudgetPhase::ReadyForReservation {
                    self.summary.binding.initial_snapshot
                } else {
                    self.pending.intended_after_snapshot
                }
        {
            return Err(BudgetError::InvalidProposal);
        }
        let attempts = self
            .summary
            .charged_attempts
            .checked_add(1)
            .ok_or(BudgetError::BudgetExhausted)?;
        let delta = self
            .summary
            .charged_delta_bytes
            .checked_add(p.delta_bytes)
            .ok_or(BudgetError::BudgetExhausted)?;
        if attempts > self.summary.caps.attempts
            || p.delta_bytes > self.summary.caps.per_attempt_delta_bytes
            || delta > self.summary.caps.total_delta_bytes
        {
            return Err(BudgetError::BudgetExhausted);
        }
        Ok(Event {
            charged_attempts: attempts,
            charged_delta: delta,
            ..self.event(Kind::Reserve, head, p, zero(), zero())
        })
    }

    pub fn settle(
        self,
        head: BudgetHead,
        report: ReportedSettlementProposal,
    ) -> Result<Event, BudgetError> {
        match self.summary.phase {
            BudgetPhase::ReadyForReservation => return Err(BudgetError::InvalidProposal),
            BudgetPhase::SettledNeedsFreshValidation
            | BudgetPhase::QuotaReadyRequiresFreshAuthority => {
                return Err(BudgetError::DuplicateSettlement)
            }
            BudgetPhase::PendingMayHaveApplied => {}
        }
        if !nonzero(report.observed_snapshot) || !nonzero(report.report_digest) {
            return Err(BudgetError::InvalidProposal);
        }
        Ok(self.event(
            Kind::SettledNeedsFreshValidation,
            head,
            self.pending,
            report.report_digest,
            report.observed_snapshot,
        ))
    }
}

/// Replay validates counters and transitions independently of decoded field presence.
/// The record digest comes from the actual checked journal record, not payload text.
pub(super) fn replay(
    previous: Option<State>,
    event: Event,
    head: BudgetHead,
    record_digest: Digest32,
) -> Result<State, BudgetError> {
    if event.prior_sequence != head.sequence
        || event.prior_record != head.record_digest
        || event.prior_head_frame != head.local_head_frame_digest
    {
        return Err(BudgetError::HeadMismatch);
    }
    event.binding.check()?;
    event.caps.check()?;
    let Some(state) = previous else {
        if event != initialization(event.binding, event.caps)? || head != genesis_head() {
            return Err(BudgetError::MissingHistory);
        }
        return Ok(State {
            summary: BudgetSummary {
                binding: event.binding,
                caps: event.caps,
                charged_attempts: 0,
                charged_delta_bytes: 0,
                phase: BudgetPhase::ReadyForReservation,
            },
            pending: empty_proposal(),
            reservation_record: zero(),
        });
    };
    if event.binding != state.summary.binding {
        return Err(BudgetError::BindingMismatch);
    }
    if event.caps != state.summary.caps {
        return Err(BudgetError::CapsMismatch);
    }
    match event.kind {
        Kind::Initialize => Err(BudgetError::ExistingHistory),
        Kind::Reserve => {
            if state.reserve(head, event.proposal)? != event {
                return Err(BudgetError::CountersMismatch);
            }
            Ok(State {
                summary: BudgetSummary {
                    charged_attempts: event.charged_attempts,
                    charged_delta_bytes: event.charged_delta,
                    phase: BudgetPhase::PendingMayHaveApplied,
                    ..state.summary
                },
                pending: event.proposal,
                reservation_record: record_digest,
            })
        }
        Kind::SettledNeedsFreshValidation => {
            let expected = state.settle(
                head,
                ReportedSettlementProposal {
                    report_digest: event.report,
                    observed_snapshot: event.observed_snapshot,
                },
            )?;
            if expected != event {
                return Err(BudgetError::CountersMismatch);
            }
            Ok(State {
                summary: BudgetSummary {
                    phase: BudgetPhase::SettledNeedsFreshValidation,
                    ..state.summary
                },
                ..state
            })
        }
    }
}
