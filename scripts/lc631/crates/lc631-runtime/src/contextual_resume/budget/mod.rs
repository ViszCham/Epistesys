//! 永続予算のみ。予約は作用権限・検証・完了ではない。
//! Durable budget accounting only; reservations are not action authority or validation.
#![forbid(unsafe_code)]
mod codec;
mod file_driver;
mod live;
mod replay_io;
mod run_binding;
mod state;
mod verified_codec;

pub use file_driver::BudgetedFileRepairDriver;
use lc631_host::HostError;
use lc631_substrate::Digest32;
pub use live::{BudgetSession, PersistedBudgetReservation};
pub use run_binding::{BudgetRequestSpec, BudgetRunManifest, CheckedBudgetRun};
pub use state::{BudgetPhase, BudgetSummary};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BudgetBinding {
    pub run_family: Digest32,
    pub source: Digest32,
    pub context: Digest32,
    pub scope: Digest32,
    pub goal: Digest32,
    pub policy: Digest32,
    /// Declared logical target scope, not authenticated filesystem identity.
    pub root_target: Digest32,
    pub initial_snapshot: Digest32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BudgetCaps {
    pub attempts: u64,
    pub per_attempt_delta_bytes: u64,
    pub total_delta_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReservationProposal {
    pub reservation: Digest32,
    pub candidate: Digest32,
    pub before_snapshot: Digest32,
    pub intended_after_snapshot: Digest32,
    /// Receipt/binding identities only. They are not validated by this budget.
    pub authority_binding: Digest32,
    pub intent_binding: Digest32,
    pub delta_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportedSettlementProposal {
    pub observed_snapshot: Digest32,
    pub report_digest: Digest32,
}

/// A local head expectation/report; not an external monotonic anchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BudgetHead {
    pub sequence: u64,
    pub record_digest: Digest32,
    pub local_head_frame_digest: Digest32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BudgetError {
    InvalidBinding,
    InvalidCaps,
    InvalidProposal,
    UnsupportedPayload,
    InvalidPayload,
    BindingMismatch,
    CapsMismatch,
    HeadMismatch,
    SequenceMismatch,
    CountersMismatch,
    BudgetExhausted,
    ReconciliationRequired,
    DuplicateSettlement,
    MissingHistory,
    ExistingHistory,
    ForeignTicket,
    TicketMismatch,
    OwnerIdExhausted,
    Poisoned,
    Resource(lc631_substrate::resource::ResourceError),
    Host(HostError),
    Bundle(crate::journal::BundleLiveError),
    Inspection(crate::journal::BundleInspectionError),
    Journal(crate::journal::JournalError),
    Codec(lc631_substrate::codec::CodecError),
}

pub(super) fn zero() -> Digest32 {
    Digest32::from_bytes([0; 32])
}
pub(super) fn nonzero(digest: Digest32) -> bool {
    digest != zero()
}

impl BudgetBinding {
    pub(super) fn values(self) -> [Digest32; 8] {
        [
            self.run_family,
            self.source,
            self.context,
            self.scope,
            self.goal,
            self.policy,
            self.root_target,
            self.initial_snapshot,
        ]
    }
    pub(super) fn check(self) -> Result<(), BudgetError> {
        if self.values().into_iter().all(nonzero) {
            Ok(())
        } else {
            Err(BudgetError::InvalidBinding)
        }
    }
}
impl BudgetCaps {
    pub(super) fn check(self) -> Result<(), BudgetError> {
        if self.attempts == 0
            || self.attempts > 4
            || self.per_attempt_delta_bytes == 0
            || self.total_delta_bytes == 0
        {
            Err(BudgetError::InvalidCaps)
        } else {
            Ok(())
        }
    }
}
