//! Pure epoch arithmetic; trusted time and clock resolution remain caller contracts.
use super::MediaAttestationV3Error;
use std::time::Duration;

pub(super) fn epoch_after_elapsed(
    base_epoch: u64,
    elapsed: Duration,
) -> Result<u64, MediaAttestationV3Error> {
    base_epoch
        .checked_add(elapsed.as_secs())
        .ok_or(MediaAttestationV3Error::InvalidBinding)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tlcx30_single_anchor_retains_partial_seconds_across_execution_and_verification() {
        let execution = Duration::from_millis(600);
        let verification = Duration::from_millis(600);
        assert_eq!(epoch_after_elapsed(10, execution + verification), Ok(11));
        // This is the old split-floor arithmetic, not the adopted clock path.
        assert_eq!(
            epoch_after_elapsed(10, execution).unwrap() + verification.as_secs(),
            10
        );
    }

    #[test]
    fn tlcx30_single_anchor_reaches_exact_expiry_boundary_once_total_elapsed_reaches_one_second() {
        assert_eq!(epoch_after_elapsed(10, Duration::from_millis(999)), Ok(10));
        assert_eq!(
            epoch_after_elapsed(10, Duration::from_millis(1_000)),
            Ok(11)
        );
        // Receipt currentness is [issued, expires); equality must reach expiry.
        let expiry = 11;
        assert!(epoch_after_elapsed(10, Duration::from_millis(600 + 600)).unwrap() >= expiry);
    }

    #[test]
    fn tlcx30_single_anchor_overflow_is_not_saturated_into_a_current_epoch() {
        assert_eq!(epoch_after_elapsed(u64::MAX, Duration::ZERO), Ok(u64::MAX));
        assert_eq!(
            epoch_after_elapsed(u64::MAX, Duration::from_secs(1)),
            Err(MediaAttestationV3Error::InvalidBinding)
        );
    }
}
