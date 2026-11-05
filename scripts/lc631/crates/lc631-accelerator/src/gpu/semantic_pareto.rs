//! Same-client exact-grid kernel and requested-buffer accounting only.
//! Driver/pool resident memory is opaque; the legacy path is not byte-accounted.
pub(crate) mod tiled;
use super::*;
use crate::semantic_pareto::{
    CheckedSemanticGridBatch, GridDirection, ScopedSemanticGpuBudget, SemanticGpuError,
};
use std::sync::{Mutex, MutexGuard};

#[derive(Default)]
pub(super) struct RequestedLedger {
    active: u64,
    retired: u64,
    buffers: u32,
    quarantined: bool,
    // Handles are moved here on unknown completion, not silently dropped.
    retained: Vec<cubecl_core::server::Handle>,
}
pub(super) struct LegacyGate<'a> {
    state: &'a GpuState,
    _gate: MutexGuard<'a, ()>,
    complete: bool,
}
impl LegacyGate<'_> {
    pub(super) fn completed(&mut self) {
        self.complete = true;
    }
}
impl Drop for LegacyGate<'_> {
    fn drop(&mut self) {
        if !self.complete {
            if let Ok(mut ledger) = self.state.requested.lock() {
                ledger.quarantined = true;
            }
        }
    }
}
pub(super) fn legacy_gate(state: &GpuState) -> Result<LegacyGate<'_>, AcceleratorError> {
    let gate = state.operation.try_lock().map_err(|_| {
        AcceleratorError::BackendUnavailable("shared_gpu_operation_busy_or_poisoned".into())
    })?;
    if state
        .requested
        .lock()
        .map_err(|_| AcceleratorError::ArenaQuarantined)?
        .quarantined
    {
        return Err(AcceleratorError::ArenaQuarantined);
    }
    Ok(LegacyGate {
        state,
        _gate: gate,
        complete: false,
    })
}
struct NativeLease<'a> {
    state: &'a GpuState,
    requested: u64,
    handles: Vec<cubecl_core::server::Handle>,
    complete: bool,
}
impl Drop for NativeLease<'_> {
    fn drop(&mut self) {
        if let Ok(mut ledger) = self.state.requested.lock() {
            if self.complete {
                self.handles.clear(); // Native completion already observed, then handles drop.
                ledger.active -= self.requested;
                ledger.buffers -= 4;
            } else {
                // Charge remains nonreusable; unknown allocator effects are not zero.
                ledger.active -= self.requested;
                ledger.retired += self.requested;
                ledger.quarantined = true;
                ledger.retained.append(&mut self.handles);
            }
        }
        // Poison means accounting is inaccessible and admission stays closed.
        // In that exceptional case handles drop; no handle-retention claim is made.
    }
}
pub(crate) struct NativeReadback {
    pub frontier: Vec<u32>,
    pub current: bool,
    pub requested_buffer_bytes: u64,
    pub buffer_count: u32,
    pub adapter: crate::semantic_pareto::SemanticAdapterMetadata,
}

#[cube(launch)]
fn semantic_pareto_kernel(
    values: &Array<i32>,
    directions: &Array<u32>,
    masks: &Array<u32>,
    output: &mut Array<u32>,
    rows: usize,
    axes: usize,
) {
    if ABSOLUTE_POS < rows {
        let right = ABSOLUTE_POS;
        let mut retained = 1u32;
        for left in 0..rows {
            let mut weak = true;
            let mut strict = false;
            for axis in 0..axes {
                let a = values[left * axes + axis];
                let b = values[right * axes + axis];
                if directions[axis] == 0u32 {
                    weak = weak && a <= b;
                    strict = strict || a < b;
                } else {
                    weak = weak && a >= b;
                    strict = strict || a > b;
                }
            }
            if left != right && masks[left] == 0u32 && masks[right] == 0u32 && weak && strict {
                retained = 0u32;
            }
        }
        output[right] = retained;
    }
}

pub(crate) fn execute(
    batch: &CheckedSemanticGridBatch,
    budget: ScopedSemanticGpuBudget,
    check_current: &mut dyn FnMut() -> bool,
) -> Result<NativeReadback, SemanticGpuError> {
    if !check_current() {
        return Err(SemanticGpuError::NotStartedNotCurrent);
    }
    let state = gpu_state(); // The existing OnceLock and existing CubeCL device/client.
    let _gate = state
        .operation
        .try_lock()
        .map_err(|_| SemanticGpuError::Busy)?;
    if state.device_lost.load(Ordering::Acquire) {
        return Err(SemanticGpuError::Quarantined);
    }
    let requested = batch.requested_buffer_bytes()?;
    let largest = (batch.values().len() * 4) as u64;
    budget.validate_requested(requested, largest)?;
    if budget.max_buffer_bytes > state.max_buffer_bytes
        || largest > state.max_storage_binding_bytes
        || state.max_storage_buffers < 4
    {
        return Err(SemanticGpuError::DeviceLimits);
    }
    // Reserve exactly the four caller-requested buffers before any GPU create.
    let mut handles = Vec::new();
    handles
        .try_reserve_exact(4)
        .map_err(|_| SemanticGpuError::RequestedBudget)?;
    {
        let mut ledger = state
            .requested
            .lock()
            .map_err(|_| SemanticGpuError::Quarantined)?;
        if ledger.quarantined {
            return Err(SemanticGpuError::Quarantined);
        }
        let total = ledger
            .active
            .checked_add(ledger.retired)
            .and_then(|n| n.checked_add(requested))
            .ok_or(SemanticGpuError::RequestedBudget)?;
        let inflight = ledger
            .active
            .checked_add(requested)
            .ok_or(SemanticGpuError::RequestedBudget)?;
        if total > budget.max_requested_bytes
            || inflight > budget.max_inflight_requested_bytes
            || ledger
                .buffers
                .checked_add(4)
                .is_none_or(|n| n > budget.max_buffers)
        {
            return Err(SemanticGpuError::RequestedBudget);
        }
        // Make the fault-retention append allocation-free for this serialized job.
        ledger
            .retained
            .try_reserve_exact(4)
            .map_err(|_| SemanticGpuError::RequestedBudget)?;
        ledger.active = inflight;
        ledger.buffers += 4;
    }
    let mut lease = NativeLease {
        state,
        requested,
        handles,
        complete: false,
    };
    let _inflight = InFlightGuard::enter();
    let client = &state.client;
    lease
        .handles
        .push(client.create_from_slice(i32::as_bytes(batch.values())));
    let directions: Vec<u32> = batch
        .axes()
        .iter()
        .map(|a| u32::from(a.direction == GridDirection::Maximize))
        .collect();
    lease
        .handles
        .push(client.create_from_slice(u32::as_bytes(&directions)));
    let masks: Vec<u32> = batch.masks().iter().map(|m| m.code()).collect();
    lease
        .handles
        .push(client.create_from_slice(u32::as_bytes(&masks)));
    lease.handles.push(client.empty(batch.row_count() * 4));
    let rows = batch.row_count();
    let axes = batch.axes().len();
    // SAFETY: checked 1..512 rows / 1..32 axes; exact buffer lengths and checked
    // integer layout; kernel guards each row and loops only admitted dimensions.
    unsafe {
        semantic_pareto_kernel::launch::<WgpuRuntime>(
            client,
            CubeCount::Static((rows as u32).div_ceil(WORKGROUP_SIZE), 1, 1),
            CubeDim::new_1d(WORKGROUP_SIZE),
            ArrayArg::from_raw_parts(lease.handles[0].clone(), batch.values().len()),
            ArrayArg::from_raw_parts(lease.handles[1].clone(), axes),
            ArrayArg::from_raw_parts(lease.handles[2].clone(), rows),
            ArrayArg::from_raw_parts(lease.handles[3].clone(), rows),
            rows,
            axes,
        );
    }
    // No error after dispatch is relabelled NotStarted. Keep charged handles on failure.
    cubecl_core::future::block_on(client.sync()).map_err(|_| SemanticGpuError::MayHaveExecuted)?;
    let bytes = client
        .read_one(lease.handles[3].clone())
        .map_err(|_| SemanticGpuError::MayHaveExecuted)?;
    cubecl_core::future::block_on(client.sync()).map_err(|_| SemanticGpuError::MayHaveExecuted)?;
    if state.device_lost.load(Ordering::Acquire) {
        return Err(SemanticGpuError::MayHaveExecuted);
    }
    if bytes.len() != rows * 4 {
        return Err(SemanticGpuError::InvalidReadback);
    }
    let frontier = u32::from_bytes(&bytes).to_vec();
    if frontier.iter().any(|v| *v > 1) {
        return Err(SemanticGpuError::InvalidReadback);
    }
    lease.complete = true;
    // Preserve completed native bytes even if the external currentness callback
    // fails/panics after dispatch. The resulting observation is NotCurrent.
    let current =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(check_current)).unwrap_or(false);
    Ok(NativeReadback {
        frontier,
        current,
        requested_buffer_bytes: requested,
        buffer_count: lease.handles.len() as u32,
        adapter: state.adapter.clone(),
    })
}

// Mutex fields live in the existing GpuState. No second device/client owner.
pub(super) fn operation_mutex() -> Mutex<()> {
    Mutex::new(())
}

pub(crate) fn device_current() -> bool {
    let state = gpu_state();
    !state.device_lost.load(Ordering::Acquire)
        && state
            .requested
            .lock()
            .is_ok_and(|ledger| !ledger.quarantined)
}
