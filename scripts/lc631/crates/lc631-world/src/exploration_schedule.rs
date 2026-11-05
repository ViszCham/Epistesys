//! TLCX-25 preparatory scheduling only. No lease, process or authority is issued.
#![forbid(unsafe_code)]

#[path = "exploration_schedule/priority.rs"]
pub mod priority;
use priority::{PriorityInput, PriorityInterval, ScheduleError};
use std::sync::Arc;

pub const MAX_GROUPS: usize = 2_048;
pub const MAX_KEY_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerNoveltyPolicyId {
    DistanceToFrontierUnionV1,
    NearestVerifiedCandidateV1,
}

/// Opaque bindings partition state; none of these bytes are ranking keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScheduleBinding {
    pub comparison_scope: [u8; 32],
    pub selection_policy: [u8; 32],
    pub budget_series: [u8; 32],
    pub novelty_policy: SchedulerNoveltyPolicyId,
}

/// Caller supplies canonical typed-content + obligations + policy/evidence
/// lineage bytes, excluding opaque IDs/order/timestamps. This module checks
/// bounds and equality, not semantic canonicalization or evidence authenticity.
pub struct SchedulingProposal<'a> {
    pub binding: ScheduleBinding,
    pub canonical_key: &'a [u8],
    pub priority: &'a PriorityInput,
    pub estimated_query_slots: u64,
    /// Reported local demand: CPU work units, GPU bytes, child slots.
    pub demand: [u64; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionReason {
    ReservedExploration,
    PriorityLowerBound,
    UndefinedInputRoundRobin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopReason {
    Cancelled,
    Deadline,
    BudgetExhausted,
    NoAdmissibleWork,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FairnessAnchor {
    Unanchored,
    LocalCursorOnly,
}

/// A local proposal-accounting ticket; NOT a ResourceLease/ExecutionPermit.
pub struct SchedulingTicket {
    owner: Arc<()>,
    binding: ScheduleBinding,
    serial: u64,
    key: Vec<u8>,
    pub reason: SelectionReason,
    pub priority: Option<PriorityInterval>,
    pub fairness_anchor: FairnessAnchor,
}
impl SchedulingTicket {
    pub fn canonical_key(&self) -> &[u8] {
        &self.key
    }
}

// Return the proposal inline; do not introduce a new fallible allocation after
// the scheduler has accounted its ticket. This is not an execution receipt.
#[allow(clippy::large_enum_variant)]
pub enum ScheduleStep {
    Proposed(SchedulingTicket),
    Stopped(StopReason),
}

struct WorkRecord {
    key: Vec<u8>,
    attempts: u32,
    live_serial: Option<u64>,
    demand: [u64; 3],
}

/// Read-only recovery material. Persistence/authentication and recovery
/// admission are deliberately absent until the runtime adapter is implemented.
pub struct FairnessView<'a> {
    pub binding: ScheduleBinding,
    pub last_served_key: Option<&'a [u8]>,
    pub frontier_revision: [u8; 32],
    pub slots_consumed: u64,
    pub anchor: FairnessAnchor,
}

pub struct ExplorationScheduler {
    owner: Arc<()>,
    binding: ScheduleBinding,
    query_slots: u64,
    consumed: u64,
    max_attempts: u32,
    capacity: [u64; 3],
    used: [u64; 3],
    records: Vec<WorkRecord>,
    last_fair_key: Option<Vec<u8>>,
    frontier_revision: [u8; 32],
    last_tick: u64,
    deadline: u64,
    cancelled: bool,
}

impl ExplorationScheduler {
    pub fn new(
        binding: ScheduleBinding,
        query_slots: u64,
        max_attempts: u32,
        capacity: [u64; 3],
        deadline: u64,
    ) -> Result<Self, ScheduleError> {
        if query_slots == 0 || max_attempts == 0 || deadline == 0 {
            return Err(ScheduleError::InvalidConfig);
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(MAX_GROUPS)
            .map_err(|_| ScheduleError::AllocationFailed)?;
        Ok(Self {
            owner: Arc::new(()),
            binding,
            query_slots,
            consumed: 0,
            max_attempts,
            capacity,
            used: [0; 3],
            records,
            last_fair_key: None,
            frontier_revision: [0; 32],
            last_tick: 0,
            deadline,
            cancelled: false,
        })
    }

    pub fn fairness(&self) -> FairnessView<'_> {
        FairnessView {
            binding: self.binding,
            last_served_key: self.last_fair_key.as_deref(),
            frontier_revision: self.frontier_revision,
            slots_consumed: self.consumed,
            anchor: self.anchor(),
        }
    }

    fn anchor(&self) -> FairnessAnchor {
        if self.last_fair_key.is_some() {
            FairnessAnchor::LocalCursorOnly
        } else {
            FairnessAnchor::Unanchored
        }
    }

    /// Validate the whole batch before scheduling. Duplicate work is one key;
    /// conflicting payloads with the same key are rejected, not arrival-ranked.
    fn validate(&self, proposals: &[SchedulingProposal<'_>]) -> Result<(), ScheduleError> {
        if proposals.len() > MAX_GROUPS {
            return Err(ScheduleError::GroupLimit);
        }
        for (i, proposal) in proposals.iter().enumerate() {
            if proposal.binding != self.binding {
                return Err(ScheduleError::BindingMismatch);
            }
            if proposal.canonical_key.is_empty() || proposal.canonical_key.len() > MAX_KEY_BYTES {
                return Err(ScheduleError::KeyLimit);
            }
            proposal.priority.validate()?;
            for other in &proposals[..i] {
                if other.canonical_key == proposal.canonical_key
                    && (other.priority != proposal.priority
                        || other.estimated_query_slots != proposal.estimated_query_slots
                        || other.demand != proposal.demand)
                {
                    return Err(ScheduleError::ConflictingDuplicate);
                }
            }
        }
        Ok(())
    }

    fn admissible(&self, proposal: &SchedulingProposal<'_>) -> bool {
        let record = self
            .records
            .iter()
            .find(|r| r.key == proposal.canonical_key);
        if record.is_some_and(|r| r.live_serial.is_some() || r.attempts >= self.max_attempts) {
            return false;
        }
        proposal.demand.iter().enumerate().all(|(i, demand)| {
            self.used[i]
                .checked_add(*demand)
                .is_some_and(|sum| sum <= self.capacity[i])
        })
    }

    /// Pure local allocation: no solver/query is executed. Every returned ticket
    /// conservatively consumes a slot, including later timeout or failed broker
    /// admission. Only an external adapter may reserve real resources/execute.
    pub fn propose(
        &mut self,
        proposals: &[SchedulingProposal<'_>],
        frontier_revision: [u8; 32],
        now: u64,
        cancelled: bool,
    ) -> Result<ScheduleStep, ScheduleError> {
        if now < self.last_tick {
            return Err(ScheduleError::ClockRegression);
        }
        self.last_tick = now;
        self.cancelled |= cancelled;
        if self.cancelled {
            return Ok(ScheduleStep::Stopped(StopReason::Cancelled));
        }
        if now >= self.deadline {
            return Ok(ScheduleStep::Stopped(StopReason::Deadline));
        }
        if self.consumed >= self.query_slots {
            return Ok(ScheduleStep::Stopped(StopReason::BudgetExhausted));
        }
        self.validate(proposals)?;
        let missing = proposals.iter().any(|p| p.priority.is_missing());
        // Slots 0,8,... reserve exactly ceil(B/8) exploration opportunities.
        let reason = if missing {
            SelectionReason::UndefinedInputRoundRobin
        } else if self.consumed.is_multiple_of(8) {
            SelectionReason::ReservedExploration
        } else {
            SelectionReason::PriorityLowerBound
        };
        let selected = self.choose(proposals, reason)?;
        let Some(proposal) = selected else {
            return Ok(ScheduleStep::Stopped(StopReason::NoAdmissibleWork));
        };
        let priority = proposal
            .priority
            .priority(proposal.estimated_query_slots, self.query_slots)?;
        self.issue(proposal, reason, priority, frontier_revision)
    }

    fn choose<'a, 'b>(
        &self,
        proposals: &'a [SchedulingProposal<'b>],
        reason: SelectionReason,
    ) -> Result<Option<&'a SchedulingProposal<'b>>, ScheduleError> {
        let mut selected: Option<&SchedulingProposal<'_>> = None;
        for proposal in proposals.iter().filter(|p| self.admissible(p)) {
            let replace = match selected {
                None => true,
                Some(previous) => {
                    if reason == SelectionReason::PriorityLowerBound {
                        let a = proposal
                            .priority
                            .priority(proposal.estimated_query_slots, self.query_slots)?;
                        let b = previous
                            .priority
                            .priority(previous.estimated_query_slots, self.query_slots)?;
                        match (a, b) {
                            (Some(a), Some(b)) => {
                                let order = priority::checked_compare(&a.lower, &b.lower)?;
                                order.is_gt()
                                    || (order.is_eq()
                                        && proposal.canonical_key < previous.canonical_key)
                            }
                            _ => return Err(ScheduleError::MissingPriority),
                        }
                    } else {
                        let cursor = self.last_fair_key.as_deref();
                        let a_after = cursor.is_none_or(|key| proposal.canonical_key > key);
                        let b_after = cursor.is_none_or(|key| previous.canonical_key > key);
                        (a_after && !b_after)
                            || (a_after == b_after
                                && proposal.canonical_key < previous.canonical_key)
                    }
                }
            };
            if replace {
                selected = Some(proposal);
            }
        }
        Ok(selected)
    }

    fn issue(
        &mut self,
        proposal: &SchedulingProposal<'_>,
        reason: SelectionReason,
        priority: Option<PriorityInterval>,
        frontier_revision: [u8; 32],
    ) -> Result<ScheduleStep, ScheduleError> {
        let index = self
            .records
            .iter()
            .position(|r| r.key == proposal.canonical_key);
        if index.is_none() && self.records.len() >= MAX_GROUPS {
            return Err(ScheduleError::GroupLimit);
        }
        // All fallible bounded copies precede publication of counters.
        let ticket_key = copy_key(proposal.canonical_key)?;
        let record_key = if index.is_none() {
            Some(copy_key(proposal.canonical_key)?)
        } else {
            None
        };
        let fair_key = if reason != SelectionReason::PriorityLowerBound {
            Some(copy_key(proposal.canonical_key)?)
        } else {
            None
        };
        let serial = self
            .consumed
            .checked_add(1)
            .ok_or(ScheduleError::ArithmeticOverflow)?;
        let index = match (index, record_key) {
            (Some(index), None) => index,
            (None, Some(key)) => {
                self.records.push(WorkRecord {
                    key,
                    attempts: 0,
                    live_serial: None,
                    demand: [0; 3],
                });
                self.records.len() - 1
            }
            _ => return Err(ScheduleError::ConflictingDuplicate),
        };
        let attempts = self.records[index]
            .attempts
            .checked_add(1)
            .ok_or(ScheduleError::ArithmeticOverflow)?;
        for i in 0..3 {
            self.used[i] = self.used[i]
                .checked_add(proposal.demand[i])
                .ok_or(ScheduleError::ArithmeticOverflow)?;
        }
        let anchor = self.anchor();
        let record = &mut self.records[index];
        record.attempts = attempts;
        record.live_serial = Some(serial);
        record.demand = proposal.demand;
        self.consumed = serial;
        self.frontier_revision = frontier_revision;
        if let Some(key) = fair_key {
            self.last_fair_key = Some(key);
        }
        Ok(ScheduleStep::Proposed(SchedulingTicket {
            owner: Arc::clone(&self.owner),
            binding: self.binding,
            serial,
            key: ticket_key,
            reason,
            priority,
            fairness_anchor: anchor,
        }))
    }

    /// Unknown cleanup retains demand and blocks duplicate dispatch. This
    /// caller assertion is local accounting, never observed OS cleanup evidence.
    pub fn finish(
        &mut self,
        ticket: &SchedulingTicket,
        cleanup_reported: bool,
    ) -> Result<(), ScheduleError> {
        if ticket.binding != self.binding || !Arc::ptr_eq(&self.owner, &ticket.owner) {
            return Err(ScheduleError::BindingMismatch);
        }
        let record = self
            .records
            .iter_mut()
            .find(|r| r.key == ticket.key && r.live_serial == Some(ticket.serial))
            .ok_or(ScheduleError::StaleTicket)?;
        if !cleanup_reported {
            return Err(ScheduleError::CleanupUnobserved);
        }
        for i in 0..3 {
            self.used[i] = self.used[i]
                .checked_sub(record.demand[i])
                .ok_or(ScheduleError::ArithmeticOverflow)?;
        }
        record.live_serial = None;
        record.demand = [0; 3];
        Ok(())
    }
}

fn copy_key(key: &[u8]) -> Result<Vec<u8>, ScheduleError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(key.len())
        .map_err(|_| ScheduleError::AllocationFailed)?;
    result.extend_from_slice(key);
    Ok(result)
}
