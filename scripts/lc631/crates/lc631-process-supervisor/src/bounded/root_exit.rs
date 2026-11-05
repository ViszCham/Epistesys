//! Bounded root-status observation, independent of process-tree accounting.
use std::io;
use std::time::{Duration, Instant};

// An empty tree can precede the root handle becoming signalled. Observe that
// status within the existing cleanup deadline; never infer an exit code from
// process counts. Injectable clock/sleep keep boundary tests deterministic.
pub(super) fn await_root_exit(
    observed: Option<i32>,
    deadline: Instant,
    mut poll: impl FnMut() -> io::Result<Option<i32>>,
    mut now: impl FnMut() -> Instant,
    mut sleep: impl FnMut(Duration),
) -> io::Result<Option<i32>> {
    if observed.is_some() {
        return Ok(observed);
    }
    loop {
        if let Some(code) = poll()? {
            return Ok(Some(code));
        }
        let remaining = deadline.saturating_duration_since(now());
        if remaining.is_zero() {
            return Ok(None);
        }
        sleep(remaining.min(Duration::from_millis(2)));
    }
}
