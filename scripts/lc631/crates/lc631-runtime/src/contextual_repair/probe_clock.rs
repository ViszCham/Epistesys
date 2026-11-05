//! Thin sampling facade over central's actual inner-owner epoch history.
#![forbid(unsafe_code)]
use super::measurement::{ObservedEpoch, ProbeError};
use std::sync::Arc;
pub(super) type ClockHistory = Arc<ObservedEpoch>;
pub(super) fn shared(clock: fn() -> u64, floor: u64) -> Result<ClockHistory, ProbeError> {
    let birth = clock();
    if birth < floor {
        return Err(ProbeError::ClockRollback);
    }
    Ok(ObservedEpoch::shared(birth))
}
pub(super) fn sample(
    clock: fn() -> u64,
    history: &ClockHistory,
    expires: u64,
) -> Result<u64, ProbeError> {
    let now = clock();
    history.observe(now, expires)?;
    Ok(now)
}
