//! Test-only live clock inputs. A trace entry is written only when an actual
//! runtime owner/gate/provider invokes that lane, not when a script is set.
use std::cell::RefCell;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lane {
    DirectBase,
    HiddenBase,
    DirectNext,
    HiddenNext,
    CaptureBase,
    CaptureNext,
}
impl Lane {
    fn index(self) -> usize {
        self as usize
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Event {
    pub lane: Lane,
    pub epoch: u64,
}
struct Script {
    values: Vec<u64>,
    cursor: usize,
    tail: u64,
}
impl Script {
    fn constant(n: u64) -> Self {
        Self {
            values: Vec::new(),
            cursor: 0,
            tail: n,
        }
    }
}
struct State {
    lanes: [Script; 6],
    trace: Vec<Event>,
}
impl State {
    fn fresh() -> Self {
        Self {
            lanes: std::array::from_fn(|_| Script::constant(100)),
            trace: Vec::new(),
        }
    }
}
thread_local! { static STATE:RefCell<State>=RefCell::new(State::fresh()); }
pub fn reset() {
    STATE.with(|s| *s.borrow_mut() = State::fresh());
}
pub fn clear_trace() {
    STATE.with(|s| s.borrow_mut().trace.clear());
}
pub fn script(lane: Lane, values: &[u64], tail: u64) {
    assert!(values.len() <= 64);
    STATE.with(|s| {
        s.borrow_mut().lanes[lane.index()] = Script {
            values: values.to_vec(),
            cursor: 0,
            tail,
        }
    });
}
pub fn constant(lane: Lane, n: u64) {
    script(lane, &[], n);
}
pub fn all(n: u64) {
    for lane in [
        Lane::DirectBase,
        Lane::HiddenBase,
        Lane::DirectNext,
        Lane::HiddenNext,
        Lane::CaptureBase,
        Lane::CaptureNext,
    ] {
        constant(lane, n);
    }
}
pub fn sample(lane: Lane) -> u64 {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        let active = &mut state.lanes[lane.index()];
        let epoch = active
            .values
            .get(active.cursor)
            .copied()
            .unwrap_or(active.tail);
        active.cursor += 1;
        assert!(state.trace.len() < 1024, "finite trace budget");
        state.trace.push(Event { lane, epoch });
        epoch
    })
}
pub fn trace() -> Vec<Event> {
    STATE.with(|s| s.borrow().trace.clone())
}
pub fn values(lane: Lane) -> Vec<u64> {
    trace()
        .into_iter()
        .filter(|e| e.lane == lane)
        .map(|e| e.epoch)
        .collect()
}
pub fn direct_base() -> u64 {
    sample(Lane::DirectBase)
}
pub fn hidden_base() -> u64 {
    sample(Lane::HiddenBase)
}
pub fn direct_next() -> u64 {
    sample(Lane::DirectNext)
}
pub fn hidden_next() -> u64 {
    sample(Lane::HiddenNext)
}
