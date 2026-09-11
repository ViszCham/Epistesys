use super::*;
use cubecl_core::{self as cubecl, prelude::*, Runtime};
use cubecl_wgpu::{AutoGraphicsApi, RuntimeOptions, WgpuDevice, WgpuRuntime};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

const WORKGROUP_SIZE: u32 = 64;
const SUMMARY_CHUNK_VALUES: usize = 1_024;
const MAX_RELATION_PAIRS: usize = 262_144;
static GPU_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

#[cube(launch)]
fn projection_loss_kernel(
    axis_0: &Array<u32>,
    axis_1: &Array<u32>,
    axis_2: &Array<u32>,
    masks: &Array<u32>,
    output: &mut Array<u32>,
) {
    if ABSOLUTE_POS < output.len() {
        let mask = masks[ABSOLUTE_POS];
        let mut value = 4_294_967_293u32;
        if mask == 0u32 {
            value = axis_0[ABSOLUTE_POS] + axis_1[ABSOLUTE_POS] + axis_2[ABSOLUTE_POS];
        } else if mask == 1u32 {
            value = 4_294_967_295u32;
        } else if mask == 2u32 {
            value = 4_294_967_294u32;
        }
        output[ABSOLUTE_POS] = value;
    }
}

#[cube(launch)]
fn relation_kernel(
    axis_0: &Array<u32>,
    axis_1: &Array<u32>,
    axis_2: &Array<u32>,
    masks: &Array<u32>,
    relation_left: &Array<u32>,
    relation_right: &Array<u32>,
    output: &mut Array<u32>,
) {
    if ABSOLUTE_POS < output.len() {
        let left = relation_left[ABSOLUTE_POS] as usize;
        let right = relation_right[ABSOLUTE_POS] as usize;
        let left_mask = masks[left];
        let right_mask = masks[right];
        let mut relation = 3u32;
        if left_mask == 2u32 || right_mask == 2u32 {
            relation = 5u32;
        } else if left_mask == 1u32 || right_mask == 1u32 {
            relation = 4u32;
        } else if left_mask == 3u32 || right_mask == 3u32 {
            relation = 3u32;
        } else {
            let equal = axis_0[left] == axis_0[right]
                && axis_1[left] == axis_1[right]
                && axis_2[left] == axis_2[right];
            let lower = axis_0[left] <= axis_0[right]
                && axis_1[left] <= axis_1[right]
                && axis_2[left] <= axis_2[right];
            let higher = axis_0[right] <= axis_0[left]
                && axis_1[right] <= axis_1[left]
                && axis_2[right] <= axis_2[left];
            if equal {
                relation = 2u32;
            } else if lower {
                relation = 0u32;
            } else if higher {
                relation = 1u32;
            }
        }
        output[ABSOLUTE_POS] = relation;
    }
}

#[cube(launch)]
fn world_distance_kernel(
    axis_0: &Array<u32>,
    axis_1: &Array<u32>,
    axis_2: &Array<u32>,
    output: &mut Array<u32>,
) {
    if ABSOLUTE_POS < output.len() {
        let distance_0 = if axis_0[ABSOLUTE_POS] >= axis_0[0] {
            axis_0[ABSOLUTE_POS] - axis_0[0]
        } else {
            axis_0[0] - axis_0[ABSOLUTE_POS]
        };
        let distance_1 = if axis_1[ABSOLUTE_POS] >= axis_1[0] {
            axis_1[ABSOLUTE_POS] - axis_1[0]
        } else {
            axis_1[0] - axis_1[ABSOLUTE_POS]
        };
        let distance_2 = if axis_2[ABSOLUTE_POS] >= axis_2[0] {
            axis_2[ABSOLUTE_POS] - axis_2[0]
        } else {
            axis_2[0] - axis_2[ABSOLUTE_POS]
        };
        output[ABSOLUTE_POS] = distance_0 + distance_1 + distance_2;
    }
}

#[cube(launch)]
fn world_contradiction_kernel(
    relations: &Array<u32>,
    offsets: &Array<u32>,
    output: &mut Array<u32>,
) {
    if ABSOLUTE_POS < output.len() {
        let mut count = 0u32;
        let start = offsets[ABSOLUTE_POS] as usize;
        let end = offsets[ABSOLUTE_POS + 1] as usize;
        for position in start..end {
            if relations[position] == 6u32 {
                count += 1u32;
            }
        }
        output[ABSOLUTE_POS] = count;
    }
}

#[cube(launch)]
fn world_incomparability_kernel(
    relations: &Array<u32>,
    offsets: &Array<u32>,
    output: &mut Array<u32>,
) {
    if ABSOLUTE_POS < output.len() {
        let mut count = 0u32;
        let start = offsets[ABSOLUTE_POS] as usize;
        let end = offsets[ABSOLUTE_POS + 1] as usize;
        for position in start..end {
            if relations[position] == 3u32 {
                count += 1u32;
            }
        }
        output[ABSOLUTE_POS] = count;
    }
}

#[cube(launch)]
fn world_pareto_kernel(relations: &Array<u32>, offsets: &Array<u32>, output: &mut Array<u32>) {
    if ABSOLUTE_POS < output.len() {
        let mut retained = 1u32;
        let start = offsets[ABSOLUTE_POS] as usize;
        let end = offsets[ABSOLUTE_POS + 1] as usize;
        for position in start..end {
            if relations[position] == 1u32 {
                retained = 0u32;
            }
        }
        output[ABSOLUTE_POS] = retained;
    }
}

#[allow(clippy::manual_clamp)]
#[cube(launch)]
fn jacobi_kernel(
    offsets: &Array<u32>,
    indices: &Array<u32>,
    weights: &Array<i32>,
    old: &Array<i32>,
    masks: &Array<u32>,
    next: &mut Array<i32>,
) {
    if ABSOLUTE_POS < old.len() {
        if masks[ABSOLUTE_POS] != 0u32 {
            next[ABSOLUTE_POS] = old[ABSOLUTE_POS];
        } else {
            let start = offsets[ABSOLUTE_POS] as usize;
            let end = offsets[ABSOLUTE_POS + 1] as usize;
            let mut contribution = 0i32;
            for position in start..end {
                let neighbor = indices[position] as usize;
                if masks[neighbor] == 0u32 {
                    contribution += old[neighbor] * weights[position] / 100i32;
                }
            }
            let mut updated = old[ABSOLUTE_POS] + contribution;
            if updated < -100i32 {
                updated = -100i32;
            }
            if updated > 100i32 {
                updated = 100i32;
            }
            next[ABSOLUTE_POS] = updated;
        }
    }
}

#[cube(launch)]
fn qubo_delta_kernel(
    linear: &Array<i32>,
    incident_offsets: &Array<u32>,
    incident_terms: &Array<u32>,
    term_left: &Array<u32>,
    term_right: &Array<u32>,
    coefficients: &Array<i32>,
    selected: &Array<u32>,
    deltas: &mut Array<i32>,
) {
    if ABSOLUTE_POS < linear.len() {
        let start = incident_offsets[ABSOLUTE_POS] as usize;
        let end = incident_offsets[ABSOLUTE_POS + 1] as usize;
        let mut local = linear[ABSOLUTE_POS];
        for position in start..end {
            let term = incident_terms[position] as usize;
            let left = term_left[term] as usize;
            let right = term_right[term] as usize;
            let other = if left == ABSOLUTE_POS { right } else { left };
            if selected[other] != 0u32 {
                local += coefficients[term];
            }
        }
        deltas[ABSOLUTE_POS] = if selected[ABSOLUTE_POS] != 0u32 {
            -local
        } else {
            local
        };
    }
}

#[cube(launch)]
fn summarize_u32_kernel(input: &Array<u32>, output: &mut Array<u32>) {
    if ABSOLUTE_POS == 0 {
        let mut minimum = input[0];
        let mut maximum = input[0];
        let mut hash_1 = 2_166_136_261u32;
        let mut hash_2 = 2_654_435_769u32;
        let mut hash_3 = 2_246_822_507u32;
        let mut zeroes = 0u32;
        for index in 0..input.len() {
            let value = input[index];
            if value < minimum {
                minimum = value;
            }
            if value > maximum {
                maximum = value;
            }
            hash_1 = (hash_1 ^ value) * 16_777_619u32;
            hash_2 =
                (hash_2 ^ (value + (index as u32 + 1u32))) * 2_246_822_507u32 + 3_266_489_907u32;
            hash_3 = (hash_3 + value * ((index as u32 + 1u32) * 668_265_261u32)) * 374_761_393u32;
            if value == 0u32 {
                zeroes += 1u32;
            }
        }
        output[0] = 3u32;
        output[1] = input.len() as u32;
        output[2] = minimum;
        output[3] = maximum;
        output[4] = hash_1;
        output[5] = hash_2;
        output[6] = hash_3;
        output[7] = zeroes;
    }
}

#[cube(launch)]
fn summarize_i32_kernel(input: &Array<i32>, output: &mut Array<u32>) {
    if ABSOLUTE_POS == 0 {
        let mut minimum = input[0];
        let mut maximum = input[0];
        let mut hash_1 = 2_166_136_261u32;
        let mut hash_2 = 2_654_435_769u32;
        let mut hash_3 = 2_246_822_507u32;
        let mut zeroes = 0u32;
        for index in 0..input.len() {
            let value = input[index];
            if value < minimum {
                minimum = value;
            }
            if value > maximum {
                maximum = value;
            }
            let bits = value as u32;
            hash_1 = (hash_1 ^ bits) * 16_777_619u32;
            hash_2 =
                (hash_2 ^ (bits + (index as u32 + 1u32))) * 2_246_822_507u32 + 3_266_489_907u32;
            hash_3 = (hash_3 + bits * ((index as u32 + 1u32) * 668_265_261u32)) * 374_761_393u32;
            if value == 0i32 {
                zeroes += 1u32;
            }
        }
        output[0] = 4u32;
        output[1] = input.len() as u32;
        output[2] = minimum as u32;
        output[3] = maximum as u32;
        output[4] = hash_1;
        output[5] = hash_2;
        output[6] = hash_3;
        output[7] = zeroes;
    }
}

#[cube(launch)]
fn summarize_u32_chunks_kernel(input: &Array<u32>, chunk_size: usize, output: &mut Array<u32>) {
    let chunk = ABSOLUTE_POS;
    let start = chunk * chunk_size;
    if start < input.len() {
        let mut minimum = input[start];
        let mut maximum = input[start];
        let mut hash_1 = 0u32;
        let mut hash_2 = 0u32;
        let mut hash_3 = 0u32;
        let mut zeroes = 0u32;
        let mut count = 0u32;
        for offset in 0..chunk_size {
            let index = start + offset;
            if index < input.len() {
                let value = input[index];
                let position = index as u32 + 1u32;
                if value < minimum {
                    minimum = value;
                }
                if value > maximum {
                    maximum = value;
                }
                hash_1 += (value ^ (position * 2_654_435_769u32)) * 2_246_822_507u32;
                hash_2 ^= (value + position * 3_266_489_909u32) * 668_265_261u32;
                hash_3 += (value + 374_761_393u32) * (position * 2_146_121_005u32);
                if value == 0u32 {
                    zeroes += 1u32;
                }
                count += 1u32;
            }
        }
        let base = chunk * 8;
        output[base] = count;
        output[base + 1] = minimum;
        output[base + 2] = maximum;
        output[base + 3] = hash_1;
        output[base + 4] = hash_2;
        output[base + 5] = hash_3;
        output[base + 6] = zeroes;
        output[base + 7] = 0u32;
    }
}

#[cube(launch)]
fn summarize_i32_chunks_kernel(input: &Array<i32>, chunk_size: usize, output: &mut Array<u32>) {
    let chunk = ABSOLUTE_POS;
    let start = chunk * chunk_size;
    if start < input.len() {
        let mut minimum = input[start];
        let mut maximum = input[start];
        let mut hash_1 = 0u32;
        let mut hash_2 = 0u32;
        let mut hash_3 = 0u32;
        let mut zeroes = 0u32;
        let mut count = 0u32;
        for offset in 0..chunk_size {
            let index = start + offset;
            if index < input.len() {
                let value = input[index];
                let bits = value as u32;
                let position = index as u32 + 1u32;
                if value < minimum {
                    minimum = value;
                }
                if value > maximum {
                    maximum = value;
                }
                hash_1 += (bits ^ (position * 2_654_435_769u32)) * 2_246_822_507u32;
                hash_2 ^= (bits + position * 3_266_489_909u32) * 668_265_261u32;
                hash_3 += (bits + 374_761_393u32) * (position * 2_146_121_005u32);
                if value == 0i32 {
                    zeroes += 1u32;
                }
                count += 1u32;
            }
        }
        let base = chunk * 8;
        output[base] = count;
        output[base + 1] = minimum as u32;
        output[base + 2] = maximum as u32;
        output[base + 3] = hash_1;
        output[base + 4] = hash_2;
        output[base + 5] = hash_3;
        output[base + 6] = zeroes;
        output[base + 7] = 0u32;
    }
}

#[cube(launch)]
fn reduce_u32_chunks_kernel(chunks: &Array<u32>, chunk_count: usize, output: &mut Array<u32>) {
    if ABSOLUTE_POS == 0 {
        let mut count = 0u32;
        let mut minimum = chunks[1];
        let mut maximum = chunks[2];
        let mut hash_1 = 0u32;
        let mut hash_2 = 0u32;
        let mut hash_3 = 0u32;
        let mut zeroes = 0u32;
        for chunk in 0..chunk_count {
            let base = chunk * 8;
            count += chunks[base];
            if chunks[base + 1] < minimum {
                minimum = chunks[base + 1];
            }
            if chunks[base + 2] > maximum {
                maximum = chunks[base + 2];
            }
            hash_1 += chunks[base + 3];
            hash_2 ^= chunks[base + 4];
            hash_3 += chunks[base + 5];
            zeroes += chunks[base + 6];
        }
        output[0] = 5u32;
        output[1] = count;
        output[2] = minimum;
        output[3] = maximum;
        output[4] = hash_1;
        output[5] = hash_2;
        output[6] = hash_3;
        output[7] = zeroes;
    }
}

#[cube(launch)]
fn reduce_i32_chunks_kernel(chunks: &Array<u32>, chunk_count: usize, output: &mut Array<u32>) {
    if ABSOLUTE_POS == 0 {
        let mut count = 0u32;
        let mut minimum = chunks[1] as i32;
        let mut maximum = chunks[2] as i32;
        let mut hash_1 = 0u32;
        let mut hash_2 = 0u32;
        let mut hash_3 = 0u32;
        let mut zeroes = 0u32;
        for chunk in 0..chunk_count {
            let base = chunk * 8;
            let chunk_minimum = chunks[base + 1] as i32;
            let chunk_maximum = chunks[base + 2] as i32;
            count += chunks[base];
            if chunk_minimum < minimum {
                minimum = chunk_minimum;
            }
            if chunk_maximum > maximum {
                maximum = chunk_maximum;
            }
            hash_1 += chunks[base + 3];
            hash_2 ^= chunks[base + 4];
            hash_3 += chunks[base + 5];
            zeroes += chunks[base + 6];
        }
        output[0] = 6u32;
        output[1] = count;
        output[2] = minimum as u32;
        output[3] = maximum as u32;
        output[4] = hash_1;
        output[5] = hash_2;
        output[6] = hash_3;
        output[7] = zeroes;
    }
}

#[cube(launch)]
fn projection_finalists_kernel(input: &Array<u32>, finalist_count: usize, output: &mut Array<u32>) {
    if ABSOLUTE_POS == 0 {
        for rank in 0..finalist_count {
            let mut best_value = 4_294_967_295u32;
            let mut best_index = input.len() as u32;
            for index in 0..input.len() {
                let mut used = false;
                for previous in 0..rank {
                    if output[previous] == index as u32 {
                        used = true;
                    }
                }
                let value = input[index];
                if !used
                    && (value < best_value || (value == best_value && (index as u32) < best_index))
                {
                    best_value = value;
                    best_index = index as u32;
                }
            }
            output[rank] = best_index;
            output[finalist_count + rank] = best_value;
        }
        let cutoff = output[finalist_count + finalist_count - 1];
        let mut tie_count = 0u32;
        for index in 0..input.len() {
            if input[index] == cutoff {
                tie_count += 1u32;
            }
        }
        output[finalist_count * 2] = cutoff;
        output[finalist_count * 2 + 1] = tie_count;
    }
}

struct GpuState {
    client: ComputeClient<WgpuRuntime>,
    device_identity: String,
    device_lost: Arc<AtomicBool>,
}

fn gpu_state() -> &'static GpuState {
    static STATE: OnceLock<GpuState> = OnceLock::new();
    STATE.get_or_init(|| {
        let setup = cubecl_wgpu::init_setup::<AutoGraphicsApi>(
            &WgpuDevice::DefaultDevice,
            RuntimeOptions::default(),
        );
        let adapter = setup.adapter.get_info();
        let device_identity = format!(
            "cubecl-wgpu:{}:{:?}:{:?}",
            adapter.name, adapter.backend, adapter.device_type
        );
        let device_lost = Arc::new(AtomicBool::new(false));
        let callback_state = Arc::clone(&device_lost);
        setup
            .device
            .set_device_lost_callback(move |_reason, _message| {
                callback_state.store(true, Ordering::Release);
            });
        let uncaptured_state = Arc::clone(&device_lost);
        setup.device.on_uncaptured_error(Arc::new(move |_error| {
            uncaptured_state.store(true, Ordering::Release);
        }));
        GpuState {
            client: <WgpuRuntime as Runtime>::client(&WgpuDevice::DefaultDevice),
            device_identity,
            device_lost,
        }
    })
}

struct InFlightGuard {
    overlap_observed: bool,
}

impl InFlightGuard {
    fn enter() -> Self {
        let previous = GPU_IN_FLIGHT.fetch_add(1, Ordering::AcqRel);
        Self {
            overlap_observed: previous > 0,
        }
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        GPU_IN_FLIGHT.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Default)]
pub struct CubeClWgpuBackend {
    profile_device: bool,
}

impl sealed::Sealed for CubeClWgpuBackend {}

impl CubeClWgpuBackend {
    pub fn profiled() -> Self {
        Self {
            profile_device: true,
        }
    }
}

impl NumericBackend for CubeClWgpuBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::CubeClWgpu
    }

    fn execute(
        &mut self,
        input: &NumericEpochInput,
    ) -> Result<RawBackendObservation, AcceleratorError> {
        input.validate()?;
        let state = gpu_state();
        if state.device_lost.load(Ordering::Acquire) {
            return Err(AcceleratorError::BackendFault(FaultKind::DeviceLost));
        }
        let in_flight = InFlightGuard::enter();
        let pair_count = input.relation_left.len();
        if pair_count > MAX_RELATION_PAIRS {
            return Err(AcceleratorError::BudgetExceeded("relation_pairs"));
        }

        let client = state.client.clone();
        let axes = std::array::from_fn::<_, 3, _>(|axis| {
            input
                .axes_x100
                .iter()
                .map(|values| u32::from(values[axis]))
                .collect::<Vec<_>>()
        });
        let mask_codes = input
            .masks
            .iter()
            .map(|mask| mask.code())
            .collect::<Vec<_>>();
        let axis_handles = axes.map(|lane| client.create_from_slice(u32::as_bytes(&lane)));
        let mask_handle = client.create_from_slice(u32::as_bytes(&mask_codes));
        let relation_offset_handle =
            client.create_from_slice(u32::as_bytes(&input.relation_offsets));
        let relation_left_handle = client.create_from_slice(u32::as_bytes(&input.relation_left));
        let relation_right_handle = client.create_from_slice(u32::as_bytes(&input.relation_right));
        let projection_handle = client.empty(input.rows() * std::mem::size_of::<u32>());
        let relation_handle = client.empty(pair_count * std::mem::size_of::<u32>());
        let world_distance_handle = client.empty(input.rows() * std::mem::size_of::<u32>());
        let world_contradiction_handle = client.empty(input.rows() * std::mem::size_of::<u32>());
        let world_incomparability_handle = client.empty(input.rows() * std::mem::size_of::<u32>());
        let world_pareto_handle = client.empty(input.rows() * std::mem::size_of::<u32>());
        let summary_bytes = NUMERIC_SUMMARY_WORDS * std::mem::size_of::<u32>();
        let projection_summary_handle = client.empty(summary_bytes);
        let relation_summary_handle = client.empty(summary_bytes);
        let jacobi_summary_handle = client.empty(summary_bytes);
        let qubo_summary_handle = client.empty(summary_bytes);
        let world_distance_summary_handle = client.empty(summary_bytes);
        let world_contradiction_summary_handle = client.empty(summary_bytes);
        let world_incomparability_summary_handle = client.empty(summary_bytes);
        let world_pareto_summary_handle = client.empty(summary_bytes);
        let finalist_count = MAX_FINALIST_ROWS.min(input.rows());
        let projection_finalist_words = finalist_count * 2 + 2;
        let projection_finalists_handle =
            client.empty(projection_finalist_words * std::mem::size_of::<u32>());
        let rows = u32::try_from(input.rows())
            .map_err(|_| AcceleratorError::BudgetExceeded("projection_rows"))?;
        let pairs = u32::try_from(pair_count)
            .map_err(|_| AcceleratorError::BudgetExceeded("relation_pairs"))?;

        let launch = || -> Result<(usize, usize), AcceleratorError> {
            // SAFETY: every raw handle below is paired with the validated logical
            // element count used to allocate it. The relation allocation uses the
            // checked rows-squared budget, and downstream kernels stay on-device.
            unsafe {
                projection_loss_kernel::launch::<WgpuRuntime>(
                    &client,
                    CubeCount::Static(rows.div_ceil(WORKGROUP_SIZE), 1, 1),
                    CubeDim::new_1d(WORKGROUP_SIZE),
                    ArrayArg::from_raw_parts(axis_handles[0].clone(), input.rows()),
                    ArrayArg::from_raw_parts(axis_handles[1].clone(), input.rows()),
                    ArrayArg::from_raw_parts(axis_handles[2].clone(), input.rows()),
                    ArrayArg::from_raw_parts(mask_handle.clone(), input.rows()),
                    ArrayArg::from_raw_parts(projection_handle.clone(), input.rows()),
                );
                relation_kernel::launch::<WgpuRuntime>(
                    &client,
                    CubeCount::Static(pairs.div_ceil(WORKGROUP_SIZE), 1, 1),
                    CubeDim::new_1d(WORKGROUP_SIZE),
                    ArrayArg::from_raw_parts(axis_handles[0].clone(), input.rows()),
                    ArrayArg::from_raw_parts(axis_handles[1].clone(), input.rows()),
                    ArrayArg::from_raw_parts(axis_handles[2].clone(), input.rows()),
                    ArrayArg::from_raw_parts(mask_handle.clone(), input.rows()),
                    ArrayArg::from_raw_parts(relation_left_handle.clone(), pair_count),
                    ArrayArg::from_raw_parts(relation_right_handle.clone(), pair_count),
                    ArrayArg::from_raw_parts(relation_handle.clone(), pair_count),
                );
                world_distance_kernel::launch::<WgpuRuntime>(
                    &client,
                    CubeCount::Static(rows.div_ceil(WORKGROUP_SIZE), 1, 1),
                    CubeDim::new_1d(WORKGROUP_SIZE),
                    ArrayArg::from_raw_parts(axis_handles[0].clone(), input.rows()),
                    ArrayArg::from_raw_parts(axis_handles[1].clone(), input.rows()),
                    ArrayArg::from_raw_parts(axis_handles[2].clone(), input.rows()),
                    ArrayArg::from_raw_parts(world_distance_handle.clone(), input.rows()),
                );
                world_contradiction_kernel::launch::<WgpuRuntime>(
                    &client,
                    CubeCount::Static(rows.div_ceil(WORKGROUP_SIZE), 1, 1),
                    CubeDim::new_1d(WORKGROUP_SIZE),
                    ArrayArg::from_raw_parts(relation_handle.clone(), pair_count),
                    ArrayArg::from_raw_parts(relation_offset_handle.clone(), input.rows() + 1),
                    ArrayArg::from_raw_parts(world_contradiction_handle.clone(), input.rows()),
                );
                world_incomparability_kernel::launch::<WgpuRuntime>(
                    &client,
                    CubeCount::Static(rows.div_ceil(WORKGROUP_SIZE), 1, 1),
                    CubeDim::new_1d(WORKGROUP_SIZE),
                    ArrayArg::from_raw_parts(relation_handle.clone(), pair_count),
                    ArrayArg::from_raw_parts(relation_offset_handle.clone(), input.rows() + 1),
                    ArrayArg::from_raw_parts(world_incomparability_handle.clone(), input.rows()),
                );
                world_pareto_kernel::launch::<WgpuRuntime>(
                    &client,
                    CubeCount::Static(rows.div_ceil(WORKGROUP_SIZE), 1, 1),
                    CubeDim::new_1d(WORKGROUP_SIZE),
                    ArrayArg::from_raw_parts(relation_handle.clone(), pair_count),
                    ArrayArg::from_raw_parts(relation_offset_handle.clone(), input.rows() + 1),
                    ArrayArg::from_raw_parts(world_pareto_handle.clone(), input.rows()),
                );
            }

            let (jacobi_handle, jacobi_workgroups) = launch_jacobi(&client, &input.jacobi)?;
            let (qubo_handle, qubo_workgroups) = launch_qubo(&client, &input.qubo)?;
            launch_u32_summary(
                &client,
                projection_handle.clone(),
                input.rows(),
                projection_summary_handle.clone(),
            );
            launch_u32_summary(
                &client,
                relation_handle.clone(),
                pair_count,
                relation_summary_handle.clone(),
            );
            launch_i32_summary(
                &client,
                jacobi_handle,
                input.jacobi.initial_x100.len(),
                jacobi_summary_handle.clone(),
            );
            launch_i32_summary(
                &client,
                qubo_handle,
                input.qubo.linear.len(),
                qubo_summary_handle.clone(),
            );
            launch_u32_summary(
                &client,
                world_distance_handle.clone(),
                input.rows(),
                world_distance_summary_handle.clone(),
            );
            launch_u32_summary(
                &client,
                world_contradiction_handle.clone(),
                input.rows(),
                world_contradiction_summary_handle.clone(),
            );
            launch_u32_summary(
                &client,
                world_incomparability_handle.clone(),
                input.rows(),
                world_incomparability_summary_handle.clone(),
            );
            launch_u32_summary(
                &client,
                world_pareto_handle.clone(),
                input.rows(),
                world_pareto_summary_handle.clone(),
            );
            launch_projection_finalists(
                &client,
                projection_handle.clone(),
                input.rows(),
                finalist_count,
                projection_finalists_handle.clone(),
                projection_finalist_words,
            );
            cubecl_core::future::block_on(client.sync()).map_err(|error| {
                AcceleratorError::BackendUnavailable(format!("gpu_sync:{error}"))
            })?;
            Ok((jacobi_workgroups, qubo_workgroups))
        };

        let (jacobi_workgroups, qubo_workgroups, device_timing, device_elapsed_ns) =
            if self.profile_device {
                let (result, profile) = client
                    .profile(launch, "lc631-bounded-summary-epoch")
                    .map_err(|error| {
                        AcceleratorError::BackendUnavailable(format!("gpu_profile:{error}"))
                    })?;
                let (jacobi_workgroups, qubo_workgroups) = result?;
                let timing = if profile.timing_method().to_string() == "device" {
                    MeasurementState::Observed
                } else {
                    MeasurementState::Unavailable
                };
                let ticks = cubecl_core::future::block_on(profile.resolve());
                let nanos = u64::try_from(ticks.duration().as_nanos()).unwrap_or(u64::MAX);
                (jacobi_workgroups, qubo_workgroups, timing, Some(nanos))
            } else {
                let (jacobi_workgroups, qubo_workgroups) = launch()?;
                (
                    jacobi_workgroups,
                    qubo_workgroups,
                    MeasurementState::NotRequested,
                    None,
                )
            };

        if state.device_lost.load(Ordering::Acquire) {
            return Err(AcceleratorError::BackendFault(FaultKind::DeviceLost));
        }
        let projection_summary =
            read_u32(&client, projection_summary_handle, "projection_summary")?;
        let relation_summary = read_u32(&client, relation_summary_handle, "relation_summary")?;
        let jacobi_summary = read_u32(&client, jacobi_summary_handle, "jacobi_summary")?;
        let qubo_summary = read_u32(&client, qubo_summary_handle, "qubo_summary")?;
        let world_distance_summary = read_u32(
            &client,
            world_distance_summary_handle,
            "world_distance_summary",
        )?;
        let world_contradiction_summary = read_u32(
            &client,
            world_contradiction_summary_handle,
            "world_contradiction_summary",
        )?;
        let world_incomparability_summary = read_u32(
            &client,
            world_incomparability_summary_handle,
            "world_incomparability_summary",
        )?;
        let world_pareto_summary =
            read_u32(&client, world_pareto_summary_handle, "world_pareto_summary")?;
        let projection_finalist_words =
            read_u32(&client, projection_finalists_handle, "projection_finalists")?;
        if projection_finalist_words.len() != finalist_count * 2 + 2 {
            return Err(AcceleratorError::ShapeMismatch("projection_finalists"));
        }
        let projection_finalists = projection_finalist_words[..finalist_count]
            .iter()
            .copied()
            .filter_map(|index| usize::try_from(index).ok())
            .filter(|index| *index < input.rows())
            .collect::<Vec<_>>();
        let projection_finalist_values =
            projection_finalist_words[finalist_count..finalist_count * 2].to_vec();
        let projection_cutoff = Some(projection_finalist_words[finalist_count * 2]);
        let projection_tie_count =
            usize::try_from(projection_finalist_words[finalist_count * 2 + 1])
                .unwrap_or(usize::MAX);
        let readback_values =
            NUMERIC_SUMMARY_WORDS * REQUIRED_LOGICAL_LANES.len() + projection_finalist_words.len();
        let input_mask_digest = numeric_mask_digest(&input.masks);
        let jacobi_mask_digest = numeric_mask_digest(&input.jacobi.masks);
        let empty_mask_digest = numeric_mask_digest(&[]);
        let lanes = vec![
            projection_lane_from_summary(
                input.rows(),
                projection_summary,
                projection_finalists,
                projection_finalist_values,
                projection_cutoff,
                projection_tie_count,
                input_mask_digest.clone(),
                NUMERIC_SUMMARY_WORDS + projection_finalist_words.len(),
            ),
            lane_from_summary(
                LaneKind::DominanceConflict,
                input.rows(),
                pair_count,
                relation_summary,
                input_mask_digest.clone(),
                NUMERIC_SUMMARY_WORDS,
            ),
            lane_from_summary(
                LaneKind::SynchronousJacobi,
                input.jacobi.initial_x100.len(),
                input.jacobi.initial_x100.len(),
                jacobi_summary,
                jacobi_mask_digest,
                NUMERIC_SUMMARY_WORDS,
            ),
            lane_from_summary(
                LaneKind::QuboDelta,
                input.qubo.linear.len(),
                input.qubo.linear.len(),
                qubo_summary,
                empty_mask_digest,
                NUMERIC_SUMMARY_WORDS,
            ),
            lane_from_summary(
                LaneKind::WorldDistance,
                input.rows(),
                input.rows(),
                world_distance_summary,
                input_mask_digest.clone(),
                NUMERIC_SUMMARY_WORDS,
            ),
            lane_from_summary(
                LaneKind::WorldContradiction,
                input.rows(),
                input.rows(),
                world_contradiction_summary,
                input_mask_digest.clone(),
                NUMERIC_SUMMARY_WORDS,
            ),
            lane_from_summary(
                LaneKind::WorldIncomparability,
                input.rows(),
                input.rows(),
                world_incomparability_summary,
                input_mask_digest.clone(),
                NUMERIC_SUMMARY_WORDS,
            ),
            lane_from_summary(
                LaneKind::WorldPareto,
                input.rows(),
                input.rows(),
                world_pareto_summary,
                input_mask_digest,
                NUMERIC_SUMMARY_WORDS,
            ),
        ];
        Ok(RawBackendObservation {
            backend: BackendKind::CubeClWgpu,
            device_identity: state.device_identity.clone(),
            lanes,
            native_completion_observed: true,
            full_readback: false,
            copy_operations: 9,
            workgroups: rows.div_ceil(WORKGROUP_SIZE) as usize
                + pairs.div_ceil(WORKGROUP_SIZE) as usize
                + jacobi_workgroups
                + qubo_workgroups
                + rows.div_ceil(WORKGROUP_SIZE) as usize * 4
                + 9,
            overlap_observed: in_flight.overlap_observed,
            readback_values,
            telemetry: BackendTelemetry {
                device_timing,
                device_elapsed_ns,
                host_inflight_overlap_observed: in_flight.overlap_observed,
                shader_occupancy: MeasurementState::Unavailable,
                device_loss_callback_registered: true,
                device_loss_callback_observed: state.device_lost.load(Ordering::Acquire),
            },
        })
    }
}

fn launch_u32_summary(
    client: &ComputeClient<WgpuRuntime>,
    input: cubecl_core::server::Handle,
    input_len: usize,
    output: cubecl_core::server::Handle,
) {
    let chunk_count = input_len.div_ceil(SUMMARY_CHUNK_VALUES);
    let chunk_words = chunk_count * NUMERIC_SUMMARY_WORDS;
    let chunks = client.empty(chunk_words * std::mem::size_of::<u32>());
    let chunk_count_u32 = u32::try_from(chunk_count).unwrap_or(u32::MAX);
    // SAFETY: the input, chunk, and final output handles use their exact checked
    // logical lengths. Each chunk owns one disjoint eight-word summary.
    unsafe {
        summarize_u32_chunks_kernel::launch::<WgpuRuntime>(
            client,
            CubeCount::Static(chunk_count_u32.div_ceil(WORKGROUP_SIZE), 1, 1),
            CubeDim::new_1d(WORKGROUP_SIZE),
            ArrayArg::from_raw_parts(input, input_len),
            SUMMARY_CHUNK_VALUES,
            ArrayArg::from_raw_parts(chunks.clone(), chunk_words),
        );
        reduce_u32_chunks_kernel::launch::<WgpuRuntime>(
            client,
            CubeCount::Static(1, 1, 1),
            CubeDim::new_1d(1),
            ArrayArg::from_raw_parts(chunks, chunk_words),
            chunk_count,
            ArrayArg::from_raw_parts(output, NUMERIC_SUMMARY_WORDS),
        );
    }
}

fn launch_i32_summary(
    client: &ComputeClient<WgpuRuntime>,
    input: cubecl_core::server::Handle,
    input_len: usize,
    output: cubecl_core::server::Handle,
) {
    let chunk_count = input_len.div_ceil(SUMMARY_CHUNK_VALUES);
    let chunk_words = chunk_count * NUMERIC_SUMMARY_WORDS;
    let chunks = client.empty(chunk_words * std::mem::size_of::<u32>());
    let chunk_count_u32 = u32::try_from(chunk_count).unwrap_or(u32::MAX);
    // SAFETY: the input, chunk, and final output handles use their exact checked
    // logical lengths. Each chunk owns one disjoint eight-word summary.
    unsafe {
        summarize_i32_chunks_kernel::launch::<WgpuRuntime>(
            client,
            CubeCount::Static(chunk_count_u32.div_ceil(WORKGROUP_SIZE), 1, 1),
            CubeDim::new_1d(WORKGROUP_SIZE),
            ArrayArg::from_raw_parts(input, input_len),
            SUMMARY_CHUNK_VALUES,
            ArrayArg::from_raw_parts(chunks.clone(), chunk_words),
        );
        reduce_i32_chunks_kernel::launch::<WgpuRuntime>(
            client,
            CubeCount::Static(1, 1, 1),
            CubeDim::new_1d(1),
            ArrayArg::from_raw_parts(chunks, chunk_words),
            chunk_count,
            ArrayArg::from_raw_parts(output, NUMERIC_SUMMARY_WORDS),
        );
    }
}

fn launch_projection_finalists(
    client: &ComputeClient<WgpuRuntime>,
    input: cubecl_core::server::Handle,
    input_len: usize,
    finalist_count: usize,
    output: cubecl_core::server::Handle,
    output_words: usize,
) {
    // SAFETY: finalist_count is bounded by input_len and MAX_FINALIST_ROWS;
    // the output allocation contains indices, values, cutoff, and tie count.
    unsafe {
        projection_finalists_kernel::launch::<WgpuRuntime>(
            client,
            CubeCount::Static(1, 1, 1),
            CubeDim::new_1d(1),
            ArrayArg::from_raw_parts(input, input_len),
            finalist_count,
            ArrayArg::from_raw_parts(output, output_words),
        );
    }
}

fn launch_jacobi(
    client: &ComputeClient<WgpuRuntime>,
    input: &JacobiInput,
) -> Result<(cubecl_core::server::Handle, usize), AcceleratorError> {
    if input.rounds > 1_024 {
        return Err(AcceleratorError::BudgetExceeded("jacobi_rounds"));
    }
    let weights = input
        .weights_x100
        .iter()
        .map(|value| i32::from(*value))
        .collect::<Vec<_>>();
    let initial = input
        .initial_x100
        .iter()
        .map(|value| i32::from(*value))
        .collect::<Vec<_>>();
    let masks = input
        .masks
        .iter()
        .map(|mask| mask.code())
        .collect::<Vec<_>>();
    let offsets = client.create_from_slice(u32::as_bytes(&input.offsets));
    let indices = client.create_from_slice(u32::as_bytes(&input.indices));
    let weights = client.create_from_slice(i32::as_bytes(&weights));
    let masks = client.create_from_slice(u32::as_bytes(&masks));
    let mut old = client.create_from_slice(i32::as_bytes(&initial));
    let mut next = client.empty(initial.len() * std::mem::size_of::<i32>());
    let rows = u32::try_from(initial.len())
        .map_err(|_| AcceleratorError::BudgetExceeded("jacobi_rows"))?;
    for _ in 0..input.rounds {
        // SAFETY: the caller validated CSR terminal, indices, weights, masks,
        // and old/new row counts. Ping-pong handles are distinct each round.
        unsafe {
            jacobi_kernel::launch::<WgpuRuntime>(
                client,
                CubeCount::Static(rows.div_ceil(WORKGROUP_SIZE), 1, 1),
                CubeDim::new_1d(WORKGROUP_SIZE),
                ArrayArg::from_raw_parts(offsets.clone(), input.offsets.len()),
                ArrayArg::from_raw_parts(indices.clone(), input.indices.len()),
                ArrayArg::from_raw_parts(weights.clone(), input.weights_x100.len()),
                ArrayArg::from_raw_parts(old.clone(), initial.len()),
                ArrayArg::from_raw_parts(masks.clone(), initial.len()),
                ArrayArg::from_raw_parts(next.clone(), initial.len()),
            );
        }
        std::mem::swap(&mut old, &mut next);
    }
    Ok((old, rows.div_ceil(WORKGROUP_SIZE) as usize * input.rounds))
}

fn launch_qubo(
    client: &ComputeClient<WgpuRuntime>,
    input: &QuboInput,
) -> Result<(cubecl_core::server::Handle, usize), AcceleratorError> {
    let nodes = input.linear.len();
    let mut incident = vec![Vec::<u32>::new(); nodes];
    for (index, term) in input.interactions.iter().enumerate() {
        let index =
            u32::try_from(index).map_err(|_| AcceleratorError::BudgetExceeded("qubo_terms"))?;
        incident[term.left].push(index);
        if term.right != term.left {
            incident[term.right].push(index);
        }
    }
    let mut offsets = Vec::with_capacity(nodes + 1);
    let mut terms = Vec::new();
    offsets.push(0_u32);
    for row in incident {
        terms.extend(row);
        offsets.push(
            u32::try_from(terms.len())
                .map_err(|_| AcceleratorError::BudgetExceeded("qubo_incidence"))?,
        );
    }
    let left = input
        .interactions
        .iter()
        .map(|term| term.left as u32)
        .collect::<Vec<_>>();
    let right = input
        .interactions
        .iter()
        .map(|term| term.right as u32)
        .collect::<Vec<_>>();
    let coefficients = input
        .interactions
        .iter()
        .map(|term| term.coefficient)
        .collect::<Vec<_>>();
    let selected = input
        .selected
        .iter()
        .map(|value| u32::from(*value))
        .collect::<Vec<_>>();
    let handles = (
        client.create_from_slice(i32::as_bytes(&input.linear)),
        client.create_from_slice(u32::as_bytes(&offsets)),
        client.create_from_slice(u32::as_bytes(&terms)),
        client.create_from_slice(u32::as_bytes(&left)),
        client.create_from_slice(u32::as_bytes(&right)),
        client.create_from_slice(i32::as_bytes(&coefficients)),
        client.create_from_slice(u32::as_bytes(&selected)),
        client.empty(nodes * std::mem::size_of::<i32>()),
    );
    let rows = u32::try_from(nodes).map_err(|_| AcceleratorError::BudgetExceeded("qubo_rows"))?;
    // SAFETY: the input validator bounds term endpoints; the host-built CSR
    // references only canonical interaction indices and every node lane matches.
    unsafe {
        qubo_delta_kernel::launch::<WgpuRuntime>(
            client,
            CubeCount::Static(rows.div_ceil(WORKGROUP_SIZE), 1, 1),
            CubeDim::new_1d(WORKGROUP_SIZE),
            ArrayArg::from_raw_parts(handles.0, nodes),
            ArrayArg::from_raw_parts(handles.1, offsets.len()),
            ArrayArg::from_raw_parts(handles.2, terms.len()),
            ArrayArg::from_raw_parts(handles.3, left.len()),
            ArrayArg::from_raw_parts(handles.4, right.len()),
            ArrayArg::from_raw_parts(handles.5, coefficients.len()),
            ArrayArg::from_raw_parts(handles.6, selected.len()),
            ArrayArg::from_raw_parts(handles.7.clone(), nodes),
        );
    }
    Ok((handles.7, rows.div_ceil(WORKGROUP_SIZE) as usize))
}

fn read_u32(
    client: &ComputeClient<WgpuRuntime>,
    handle: cubecl_core::server::Handle,
    lane: &str,
) -> Result<Vec<u32>, AcceleratorError> {
    let bytes = client
        .read_one(handle)
        .map_err(|error| AcceleratorError::BackendUnavailable(format!("{lane}_read:{error}")))?;
    Ok(u32::from_bytes(&bytes).to_vec())
}
