//! One same-state four-buffer lease, ordered <=512x512 all-pairs dispatches.
//! Only a complete full readback constructs an observation. No resident VRAM claim.
use super::*;
use crate::semantic_pareto::tiled::{CheckedTiledBatch, TiledError, TILE_ROWS};

#[cube(launch)]
fn tiled_kernel(
    values: &Array<i32>,
    directions: &Array<u32>,
    masks: &Array<u32>,
    output: &mut Array<u32>,
    target_start: usize,
    target_count: usize,
    compare_start: usize,
    compare_count: usize,
    axes: usize,
) {
    if ABSOLUTE_POS < target_count {
        let right = target_start + ABSOLUTE_POS;
        let mut retained = output[right];
        for index in 0..compare_count {
            let left = compare_start + index;
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
fn incomplete(done: usize, reason: SemanticGpuError) -> TiledError {
    TiledError::Incomplete {
        completed_tiles: done,
        reason,
    }
}
/// Same parent's NativeLease and ledger; no second allocation/client owner.
pub(crate) fn execute(
    batch: &CheckedTiledBatch,
    budget: ScopedSemanticGpuBudget,
    check: &mut dyn FnMut() -> bool,
) -> Result<NativeReadback, TiledError> {
    if !check() {
        return Err(TiledError::Input(SemanticGpuError::NotStartedNotCurrent));
    }
    let state = gpu_state();
    let _gate = state
        .operation
        .try_lock()
        .map_err(|_| TiledError::Input(SemanticGpuError::Busy))?;
    if state.device_lost.load(Ordering::Acquire) {
        return Err(TiledError::Input(SemanticGpuError::Quarantined));
    }
    let requested = batch.requested_buffer_bytes()?;
    let largest = u64::try_from(
        batch
            .values()
            .len()
            .checked_mul(4)
            .ok_or(TiledError::Input(SemanticGpuError::RequestedBudget))?,
    )
    .map_err(|_| TiledError::Input(SemanticGpuError::RequestedBudget))?;
    budget
        .validate_requested(requested, largest)
        .map_err(TiledError::Input)?;
    if budget.max_buffer_bytes > state.max_buffer_bytes
        || largest > state.max_storage_binding_bytes
        || state.max_storage_buffers < 4
    {
        return Err(TiledError::Input(SemanticGpuError::DeviceLimits));
    }
    // All owned host allocations/capacity and all four requested native buffers
    // are admitted before the first native create or kernel effect.
    let (mut directions, mut masks, mut frontier, mut handles) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    directions
        .try_reserve_exact(batch.axes().len())
        .map_err(|_| TiledError::Input(SemanticGpuError::RequestedBudget))?;
    masks
        .try_reserve_exact(batch.row_count())
        .map_err(|_| TiledError::Input(SemanticGpuError::RequestedBudget))?;
    frontier
        .try_reserve_exact(batch.row_count())
        .map_err(|_| TiledError::Input(SemanticGpuError::RequestedBudget))?;
    handles
        .try_reserve_exact(4)
        .map_err(|_| TiledError::Input(SemanticGpuError::RequestedBudget))?;
    directions.extend(
        batch
            .axes()
            .iter()
            .map(|a| u32::from(a.direction == GridDirection::Maximize)),
    );
    masks.extend(batch.masks().iter().map(|m| m.code()));
    frontier.resize(batch.row_count(), 1u32);
    {
        let mut ledger = state
            .requested
            .lock()
            .map_err(|_| TiledError::Input(SemanticGpuError::Quarantined))?;
        if ledger.quarantined {
            return Err(TiledError::Input(SemanticGpuError::Quarantined));
        }
        let total = ledger
            .active
            .checked_add(ledger.retired)
            .and_then(|n| n.checked_add(requested))
            .ok_or(TiledError::Input(SemanticGpuError::RequestedBudget))?;
        let active = ledger
            .active
            .checked_add(requested)
            .ok_or(TiledError::Input(SemanticGpuError::RequestedBudget))?;
        let count = ledger
            .buffers
            .checked_add(4)
            .ok_or(TiledError::Input(SemanticGpuError::RequestedBudget))?;
        if total > budget.max_requested_bytes
            || active > budget.max_inflight_requested_bytes
            || count > budget.max_buffers
        {
            return Err(TiledError::Input(SemanticGpuError::RequestedBudget));
        }
        ledger
            .retained
            .try_reserve_exact(4)
            .map_err(|_| TiledError::Input(SemanticGpuError::RequestedBudget))?;
        ledger.active = active;
        ledger.buffers = count;
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
    lease
        .handles
        .push(client.create_from_slice(u32::as_bytes(&directions)));
    lease
        .handles
        .push(client.create_from_slice(u32::as_bytes(&masks)));
    lease
        .handles
        .push(client.create_from_slice(u32::as_bytes(&frontier)));
    let rows = batch.row_count();
    let axes = batch.axes().len();
    let mut done = 0;
    for target_start in (0..rows).step_by(TILE_ROWS) {
        let target_count = (rows - target_start).min(TILE_ROWS);
        for compare_start in (0..rows).step_by(TILE_ROWS) {
            // Keep the known synchronized-tile count if a caller callback
            // panics. Leave the lease incomplete so its charge and handles
            // remain quarantined as unknown rather than claiming cleanup.
            let current = std::panic::catch_unwind(std::panic::AssertUnwindSafe(&mut *check))
                .map_err(|_| incomplete(done, SemanticGpuError::MayHaveExecuted))?;
            if !current {
                // Previous dispatches have synchronized; these handles can drop.
                cubecl_core::future::block_on(client.sync())
                    .map_err(|_| incomplete(done, SemanticGpuError::MayHaveExecuted))?;
                lease.complete = true;
                return Err(incomplete(done, SemanticGpuError::StaleObservation));
            }
            let compare_count = (rows - compare_start).min(TILE_ROWS);
            // SAFETY: whole arrays have admitted exact lengths. Global starts
            // and counts are bounded by rows<=2048, both tile dimensions<=512.
            // A single thread owns each target word; sync orders every launch.
            unsafe {
                tiled_kernel::launch::<WgpuRuntime>(
                    client,
                    CubeCount::Static((target_count as u32).div_ceil(WORKGROUP_SIZE), 1, 1),
                    CubeDim::new_1d(WORKGROUP_SIZE),
                    ArrayArg::from_raw_parts(lease.handles[0].clone(), batch.values().len()),
                    ArrayArg::from_raw_parts(lease.handles[1].clone(), axes),
                    ArrayArg::from_raw_parts(lease.handles[2].clone(), rows),
                    ArrayArg::from_raw_parts(lease.handles[3].clone(), rows),
                    target_start,
                    target_count,
                    compare_start,
                    compare_count,
                    axes,
                );
            }
            cubecl_core::future::block_on(client.sync())
                .map_err(|_| incomplete(done, SemanticGpuError::MayHaveExecuted))?;
            if state.device_lost.load(Ordering::Acquire) {
                return Err(incomplete(done, SemanticGpuError::MayHaveExecuted));
            }
            done += 1;
        }
    }
    let bytes = client
        .read_one(lease.handles[3].clone())
        .map_err(|_| incomplete(done, SemanticGpuError::MayHaveExecuted))?;
    cubecl_core::future::block_on(client.sync())
        .map_err(|_| incomplete(done, SemanticGpuError::MayHaveExecuted))?;
    if state.device_lost.load(Ordering::Acquire) {
        return Err(incomplete(done, SemanticGpuError::MayHaveExecuted));
    }
    if bytes.len() != rows * 4 {
        return Err(incomplete(done, SemanticGpuError::InvalidReadback));
    }
    let words = u32::from_bytes(&bytes);
    if words.iter().any(|v| *v > 1) {
        return Err(incomplete(done, SemanticGpuError::InvalidReadback));
    }
    frontier.copy_from_slice(words);
    lease.complete = true;
    let current = std::panic::catch_unwind(std::panic::AssertUnwindSafe(check)).unwrap_or(false);
    Ok(NativeReadback {
        frontier,
        current,
        requested_buffer_bytes: requested,
        buffer_count: 4,
        adapter: state.adapter.clone(),
    })
}
