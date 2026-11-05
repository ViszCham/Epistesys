//! In-process logical write charges; no persistence, permission or resume.
//! Reservations survive ambiguous effects until the run owner stops.
use super::{DgclRepairConfigError, DgclRepairLoopReport};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DgclRepairDeltaUnit {
    /// max(before UTF-8 byte length, replacement UTF-8 byte length).
    /// This is neither edit distance nor an OS disk/RAM quota.
    ConservativeWriteSizeBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct DgclRepairTotalBudgetObservation {
    pub limit_bytes: u64,
    /// Settled cost of receipt-verified applications, including later failures.
    pub charged_bytes: u64,
    /// Unsettled allowance retained when application effects are uncertain.
    pub reserved_bytes: u64,
    pub unit: DgclRepairDeltaUnit,
    pub ephemeral: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct DgclRepairTotalBudgetReport {
    pub schema_version: &'static str,
    pub source_revision: String,
    pub plan_digest: String,
    pub repair: DgclRepairLoopReport,
    pub budget: DgclRepairTotalBudgetObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepairTotalBudgetError {
    Exhausted,
    ActiveReservation,
    InvalidAmount,
}

/// One owner and at most one outstanding apply. There is deliberately no
/// Deserialize or constructor from a prior public observation/report.
pub(crate) struct RepairTotalBudget {
    limit: u64,
    charged: u64,
    reserved: u64,
}

impl RepairTotalBudget {
    pub(crate) fn new(limit: u64) -> Result<Self, DgclRepairConfigError> {
        if limit == 0 {
            return Err(DgclRepairConfigError::InvalidBudget);
        }
        Ok(Self {
            limit,
            charged: 0,
            reserved: 0,
        })
    }

    pub(crate) fn reserve(&mut self, per_attempt: u64) -> Result<u64, RepairTotalBudgetError> {
        if per_attempt == 0 {
            return Err(RepairTotalBudgetError::InvalidAmount);
        }
        if self.reserved != 0 {
            return Err(RepairTotalBudgetError::ActiveReservation);
        }
        let remaining = self
            .limit
            .checked_sub(self.charged)
            .ok_or(RepairTotalBudgetError::InvalidAmount)?;
        if remaining == 0 {
            return Err(RepairTotalBudgetError::Exhausted);
        }
        self.reserved = per_attempt.min(remaining);
        Ok(self.reserved)
    }

    /// Call only after binding, fresh lease and application receipt checks.
    /// Failed checks must leave the reservation unchanged, not call this method.
    pub(crate) fn settle_verified(&mut self, actual: u64) -> Result<(), RepairTotalBudgetError> {
        if self.reserved == 0 || actual > self.reserved {
            return Err(RepairTotalBudgetError::InvalidAmount);
        }
        let charged = self
            .charged
            .checked_add(actual)
            .filter(|value| *value <= self.limit)
            .ok_or(RepairTotalBudgetError::InvalidAmount)?;
        self.charged = charged;
        self.reserved = 0;
        Ok(())
    }

    pub(crate) const fn observation(&self) -> DgclRepairTotalBudgetObservation {
        DgclRepairTotalBudgetObservation {
            limit_bytes: self.limit,
            charged_bytes: self.charged,
            reserved_bytes: self.reserved,
            unit: DgclRepairDeltaUnit::ConservativeWriteSizeBytes,
            ephemeral: true,
        }
    }

    pub(crate) fn finish(self, repair: DgclRepairLoopReport) -> DgclRepairTotalBudgetReport {
        DgclRepairTotalBudgetReport {
            schema_version: "epistesys-dgcl-repair-total-budget.v1",
            source_revision: repair.source_revision.clone(),
            plan_digest: repair.plan_digest.clone(),
            budget: self.observation(),
            repair,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tlcx31_unsettled_allowance_is_retained_and_cannot_be_reserved_twice() {
        let mut budget = RepairTotalBudget::new(70).unwrap();
        assert_eq!(budget.reserve(40), Ok(40));
        assert_eq!(
            budget.reserve(40),
            Err(RepairTotalBudgetError::ActiveReservation)
        );
        assert_eq!(
            budget.settle_verified(41),
            Err(RepairTotalBudgetError::InvalidAmount)
        );
        assert_eq!(budget.observation().reserved_bytes, 40);
        assert_eq!(budget.observation().charged_bytes, 0);
        assert_eq!(budget.settle_verified(30), Ok(()));
        assert_eq!(budget.reserve(50), Ok(40));
        assert_eq!(budget.settle_verified(40), Ok(()));
        assert_eq!(budget.reserve(1), Err(RepairTotalBudgetError::Exhausted));
    }

    #[test]
    fn tlcx31_independent_small_model_enumerates_allowances_and_settlements() {
        for total in 1..=30_u64 {
            for per in 1..=20_u64 {
                let mut budget = RepairTotalBudget::new(total).unwrap();
                let mut used = 0;
                for attempt in 0..4_u64 {
                    if used == total {
                        assert_eq!(budget.reserve(per), Err(RepairTotalBudgetError::Exhausted));
                        break;
                    }
                    let expected = if per < total - used {
                        per
                    } else {
                        total - used
                    };
                    assert_eq!(budget.reserve(per), Ok(expected));
                    let actual = expected - attempt.min(expected);
                    budget.settle_verified(actual).unwrap();
                    used += actual;
                    assert_eq!(budget.observation().charged_bytes, used);
                    assert_eq!(budget.observation().reserved_bytes, 0);
                }
            }
        }
    }

    #[test]
    fn tlcx31_u64_boundaries_are_checked_without_overflow_or_implicit_zero() {
        assert!(RepairTotalBudget::new(0).is_err());
        let mut budget = RepairTotalBudget::new(u64::MAX).unwrap();
        assert_eq!(
            budget.reserve(0),
            Err(RepairTotalBudgetError::InvalidAmount)
        );
        assert_eq!(budget.reserve(u64::MAX), Ok(u64::MAX));
        budget.settle_verified(u64::MAX - 1).unwrap();
        assert_eq!(budget.reserve(u64::MAX), Ok(1));
        budget.settle_verified(1).unwrap();
        assert_eq!(budget.observation().charged_bytes, u64::MAX);
        assert_eq!(budget.reserve(1), Err(RepairTotalBudgetError::Exhausted));
    }
}
