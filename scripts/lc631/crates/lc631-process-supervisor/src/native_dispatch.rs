//! Bounded diagnostics for actual supervisor root starts on this thread only.
//!
//! Emitters are crate-private OS-launch seams, not caller-reported evidence.
//! This does not observe descendants, other threads or host-global execution,
//! and creates no execution, filesystem or semantic authority.
#![forbid(unsafe_code)]
use std::cell::Cell;

const CAPACITY: usize = 16;

/// A root PID obtained at the actual OS launch seam. PID reuse is possible;
/// this is not a durable identity, executable authentication or process handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeDispatch {
    process_id: u32,
}
impl NativeDispatch {
    pub fn process_id(&self) -> u32 {
        self.process_id
    }
}

#[derive(Clone, Copy)]
struct TraceState {
    events: [NativeDispatch; CAPACITY],
    len: usize,
    complete: bool,
}
impl TraceState {
    const fn empty() -> Self {
        Self {
            events: [NativeDispatch { process_id: 0 }; CAPACITY],
            len: 0,
            complete: true,
        }
    }
}

thread_local! {
    static ACTIVE: Cell<Option<TraceState>> = const { Cell::new(None) };
}

/// Registration failures are not complete empty observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchObserveError {
    AlreadyObserving,
    ThreadLocalUnavailable,
    ObservationLost,
}

/// Fixed-size observation, with no public event/count constructor or mutation.
/// `complete` only excludes known uncertainty/overflow in these private seams;
/// it does not extend the current-thread, supervisor-root observation scope.
pub struct NativeDispatchTrace<R> {
    result: R,
    state: TraceState,
}
impl<R> NativeDispatchTrace<R> {
    pub fn result(&self) -> &R {
        &self.result
    }
    pub fn into_result(self) -> R {
        self.result
    }
    pub fn is_complete(&self) -> bool {
        self.state.complete
    }
    pub fn dispatches(&self) -> &[NativeDispatch] {
        &self.state.events[..self.state.len]
    }
}

struct Registration {
    active: bool,
}
impl Registration {
    fn begin() -> Result<Self, DispatchObserveError> {
        ACTIVE
            .try_with(|slot| {
                if slot.get().is_some() {
                    return Err(DispatchObserveError::AlreadyObserving);
                }
                slot.set(Some(TraceState::empty()));
                Ok(Self { active: true })
            })
            .map_err(|_| DispatchObserveError::ThreadLocalUnavailable)?
    }
    fn finish(mut self) -> Result<TraceState, DispatchObserveError> {
        let state = ACTIVE
            .try_with(Cell::take)
            .map_err(|_| DispatchObserveError::ThreadLocalUnavailable)?
            .ok_or(DispatchObserveError::ObservationLost)?;
        self.active = false;
        Ok(state)
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        if self.active {
            // In particular, a callback unwind must not leave a stale observer.
            let _ = ACTIVE.try_with(|slot| slot.set(None));
        }
    }
}

/// Observe synchronous supervisor OS-root launch seams during `operation`.
/// Nested registration rejects without running its operation or clearing the
/// outer trace. No heap allocation is required by registration or emitters.
/// A panic propagates normally after the registration is removed by RAII.
pub fn observe_native_dispatches<R>(
    operation: impl FnOnce() -> R,
) -> Result<NativeDispatchTrace<R>, DispatchObserveError> {
    let registration = Registration::begin()?;
    let result = operation();
    let state = registration.finish()?;
    Ok(NativeDispatchTrace { result, state })
}

/// Call only after Unix Command::spawn or Windows ResumeThread succeeds, with
/// the PID returned by that OS operation. Never derive it from a request/report.
pub(crate) fn record_started(process_id: u32) {
    let _ = ACTIVE.try_with(|slot| {
        if let Some(mut state) = slot.get() {
            if process_id == 0 || state.len == CAPACITY {
                state.complete = false;
            } else {
                state.events[state.len] = NativeDispatch { process_id };
                state.len += 1;
            }
            slot.set(Some(state));
        }
    });
}

/// Created/may-have-started failure is never accepted as complete zero. A
/// prior recorded start stays visible but no longer supports an exact count.
pub(crate) fn record_may_have_started() {
    let _ = ACTIVE.try_with(|slot| {
        if let Some(mut state) = slot.get() {
            state.complete = false;
            slot.set(Some(state));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic private-emitter tests exercise bookkeeping, not actual OS/Z3
    // execution. The separate explicit replay integration tests are required.
    #[test]
    fn private_emitter_retains_pid_and_callback_result() {
        let trace = observe_native_dispatches(|| {
            record_started(7);
            41
        })
        .unwrap();
        assert_eq!(trace.result(), &41);
        assert!(trace.is_complete());
        assert_eq!(trace.dispatches().len(), 1);
        assert_eq!(trace.dispatches()[0].process_id(), 7);
        assert_eq!(trace.into_result(), 41);
    }

    #[test]
    fn nested_registration_neither_runs_nor_clears_outer_observation() {
        let trace = observe_native_dispatches(|| {
            record_started(8);
            let nested = observe_native_dispatches(|| panic!("nested operation ran"));
            assert!(matches!(
                nested,
                Err(DispatchObserveError::AlreadyObserving)
            ));
            record_started(9);
        })
        .unwrap();
        assert!(trace.is_complete());
        assert_eq!(
            trace
                .dispatches()
                .iter()
                .map(NativeDispatch::process_id)
                .collect::<Vec<_>>(),
            [8, 9]
        );
    }

    #[test]
    fn callback_unwind_removes_registration() {
        let result = std::panic::catch_unwind(|| {
            let _ = observe_native_dispatches(|| -> () {
                record_started(10);
                panic!("owned unwind test");
            });
        });
        assert!(result.is_err());
        let next = observe_native_dispatches(|| ()).unwrap();
        assert!(next.is_complete());
        assert!(next.dispatches().is_empty());
    }

    #[test]
    fn capacity_overflow_retains_prefix_and_marks_incomplete() {
        let trace = observe_native_dispatches(|| {
            for process_id in 1..=17 {
                record_started(process_id);
            }
        })
        .unwrap();
        assert!(!trace.is_complete());
        assert_eq!(trace.dispatches().len(), CAPACITY);
        assert_eq!(trace.dispatches()[0].process_id(), 1);
        assert_eq!(trace.dispatches()[15].process_id(), 16);
    }

    #[test]
    fn may_have_started_and_invalid_pid_cannot_be_complete_zero() {
        let unknown = observe_native_dispatches(record_may_have_started).unwrap();
        assert!(!unknown.is_complete());
        assert!(unknown.dispatches().is_empty());
        let invalid = observe_native_dispatches(|| record_started(0)).unwrap();
        assert!(!invalid.is_complete());
        assert!(invalid.dispatches().is_empty());
        let later_failure = observe_native_dispatches(|| {
            record_started(11);
            record_may_have_started();
        })
        .unwrap();
        assert!(!later_failure.is_complete());
        assert_eq!(later_failure.dispatches()[0].process_id(), 11);
    }

    #[test]
    fn another_thread_cannot_append_to_current_trace() {
        let trace = observe_native_dispatches(|| {
            std::thread::spawn(|| record_started(12)).join().unwrap();
        })
        .unwrap();
        assert!(trace.is_complete());
        assert!(trace.dispatches().is_empty());
    }
}
