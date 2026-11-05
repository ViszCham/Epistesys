//! Exact bounded BigRational reference priority, not S_Creative or semantic TL.
#![forbid(unsafe_code)]
use lc631_core::measurement_numeric::{check, BigInt, NumericError, Rational as BigRational};
use num_traits::{One, Zero};

const MAX_BITS: u64 = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduleError {
    InvalidConfig,
    InvalidInterval,
    NumericLimit,
    NumericInput(NumericError),
    AllocationFailed,
    GroupLimit,
    KeyLimit,
    BindingMismatch,
    ConflictingDuplicate,
    ClockRegression,
    MissingPriority,
    ArithmeticOverflow,
    StaleTicket,
    CleanupUnobserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingPriorityReason {
    Unknown,
    Unsupported,
    Unavailable,
    Incomparable,
    Undefined,
    EmptyActiveSet,
    NeedsEvidence,
    ResourceExhausted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitInterval {
    lower: BigRational,
    upper: BigRational,
}
impl UnitInterval {
    pub fn checked(lower: BigRational, upper: BigRational) -> Result<Self, ScheduleError> {
        check_size(&lower)?;
        check_size(&upper)?;
        if lower < BigRational::zero()
            || upper > BigRational::one()
            || checked_compare(&lower, &upper)?.is_gt()
        {
            return Err(ScheduleError::InvalidInterval);
        }
        Ok(Self { lower, upper })
    }
    pub fn lower(&self) -> &BigRational {
        &self.lower
    }
    pub fn upper(&self) -> &BigRational {
        &self.upper
    }
}

/// u is a previously checked maximum active scalar width/range-width, not
/// REC-U or U_gain. D/G observations must be normalized upstream under the
/// fixed profile. This module cannot manufacture missing measurements.
#[derive(Clone, Debug, Eq, PartialEq)]
// Keep the two caller-owned rational bounds inline. Boxing would add an
// unchecked allocation to input construction solely to reduce enum stack size.
#[allow(clippy::large_enum_variant)]
pub enum PriorityInput {
    Discrete {
        uncertainty: UnitInterval,
        novelty: UnitInterval,
    },
    Missing(MissingPriorityReason),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriorityInterval {
    pub lower: BigRational,
    pub upper: BigRational,
}

impl PriorityInput {
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing(_))
    }

    pub fn validate(&self) -> Result<(), ScheduleError> {
        match self {
            Self::Discrete {
                uncertainty,
                novelty,
            } => {
                for value in [
                    &uncertainty.lower,
                    &uncertainty.upper,
                    &novelty.lower,
                    &novelty.upper,
                ] {
                    check_size(value)?;
                }
                Ok(())
            }
            Self::Missing(_) => Ok(()),
        }
    }

    /// The only saturation here is the expressly registered query cost/B map.
    pub fn priority(
        &self,
        estimated_cost: u64,
        budget: u64,
    ) -> Result<Option<PriorityInterval>, ScheduleError> {
        if budget == 0 {
            return Err(ScheduleError::InvalidConfig);
        }
        self.validate()?;
        match self {
            Self::Missing(_) => Ok(None),
            Self::Discrete {
                uncertainty,
                novelty,
            } => {
                let cost = BigRational::new(
                    BigInt::from(estimated_cost.min(budget)),
                    BigInt::from(budget),
                );
                let denominator = BigRational::one() + cost;
                let endpoint =
                    |u: &BigRational, n: &BigRational| -> Result<BigRational, ScheduleError> {
                        // Preflight conservative integer-size bounds before products.
                        if u.numer().bits()
                            + u.denom().bits()
                            + n.numer().bits()
                            + n.denom().bits()
                            + 132
                            > MAX_BITS
                        {
                            return Err(ScheduleError::NumericLimit);
                        }
                        let left = BigRational::one() + u;
                        let right = BigRational::one() + n;
                        check_size(&left)?;
                        check_size(&right)?;
                        let product = left * right;
                        check_size(&product)?;
                        let value = product / &denominator;
                        check_size(&value)?;
                        Ok(value)
                    };
                Ok(Some(PriorityInterval {
                    lower: endpoint(&uncertainty.lower, &novelty.lower)?,
                    upper: endpoint(&uncertainty.upper, &novelty.upper)?,
                }))
            }
        }
    }
}

fn check_size(value: &BigRational) -> Result<(), ScheduleError> {
    // Rational::new_raw is safe and public. Check canonical denominator and
    // reduction before num-rational comparisons can divide by that denominator.
    check(value).map_err(|error| match error {
        NumericError::NumericBudget | NumericError::ResourceExhausted => {
            ScheduleError::NumericLimit
        }
        NumericError::DivisionByZero
        | NumericError::ReversedInterval
        | NumericError::InvalidInput
        | NumericError::InvalidPrecision
        | NumericError::NoApplicable => ScheduleError::NumericInput(error),
    })
}

pub fn checked_compare(
    a: &BigRational,
    b: &BigRational,
) -> Result<std::cmp::Ordering, ScheduleError> {
    check_size(a)?;
    check_size(b)?;
    if a.numer().bits() + b.denom().bits() > MAX_BITS
        || b.numer().bits() + a.denom().bits() > MAX_BITS
    {
        return Err(ScheduleError::NumericLimit);
    }
    Ok(a.cmp(b))
}
