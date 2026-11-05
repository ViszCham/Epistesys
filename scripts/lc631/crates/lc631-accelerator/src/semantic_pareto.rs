//! SemanticParetoExactGridV1: exact numeric layout, not semantic provenance.
//! Separate from legacy mandatory four/eight lanes; no fixture lane padding.
#![forbid(unsafe_code)]
pub mod tiled;
#[cfg(feature = "gpu-cubecl-wgpu")]
use crate::AcceleratorBroker;
use crate::NumericMask;
use lc631_core::EpochId;
#[cfg(feature = "gpu-cubecl-wgpu")]
use lc631_core::{FenceId, Generation, ReceiptId};
use sha2::{Digest, Sha256};
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

pub const MAX_SEMANTIC_ROWS: usize = 512;
pub const MAX_SEMANTIC_AXES: usize = 32;
/// Distinct numeric capability; never legacy four/eight-lane coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticParetoExactGridV1;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridDirection {
    Minimize,
    Maximize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GridAxis {
    pub denominator: u32,
    pub minimum: i32,
    pub maximum: i32,
    pub direction: GridDirection,
}
/// These are requested logical buffers, NOT pool-resident or global VRAM.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScopedSemanticGpuBudget {
    pub max_buffers: u32,
    pub max_requested_bytes: u64,
    pub max_inflight_requested_bytes: u64,
    pub max_buffer_bytes: u64,
}
impl ScopedSemanticGpuBudget {
    pub fn validate_requested(self, total: u64, largest: u64) -> Result<(), SemanticGpuError> {
        if self.max_buffers < 4
            || self.max_buffers > 64
            || self.max_requested_bytes == 0
            || self.max_inflight_requested_bytes == 0
            || self.max_buffer_bytes == 0
            || self.max_inflight_requested_bytes > self.max_requested_bytes
            || total > self.max_requested_bytes
            || total > self.max_inflight_requested_bytes
            || largest > self.max_buffer_bytes
        {
            return Err(SemanticGpuError::RequestedBudget);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticGpuError {
    UnsupportedShape,
    UnsupportedMask,
    UnsupportedGrid,
    DuplicateRow,
    RequestedBudget,
    DeviceLimits,
    Busy,
    NotStartedNotCurrent,
    Unavailable,
    Quarantined,
    OwnerExhausted,
    ForeignObservation,
    StaleObservation,
    MayHaveExecuted,
    InvalidReadback,
}
/// Immutable owned numeric packet. Its binding is supplied by the real runtime
/// lowering; numeric admission alone does not authenticate its source or mapping.
pub struct CheckedSemanticGridBatch {
    binding: [u8; 32],
    epoch: EpochId,
    axes: Vec<GridAxis>,
    rows: Vec<[u8; 32]>,
    values: Vec<i32>,
    masks: Vec<NumericMask>,
    digest: [u8; 32],
}
impl CheckedSemanticGridBatch {
    pub fn capability(&self) -> SemanticParetoExactGridV1 {
        SemanticParetoExactGridV1
    }
    pub fn checked_numeric(
        binding: [u8; 32],
        epoch: EpochId,
        axes: Vec<GridAxis>,
        rows: Vec<[u8; 32]>,
        values: Vec<i32>,
        masks: Vec<NumericMask>,
    ) -> Result<Self, SemanticGpuError> {
        if rows.is_empty()
            || rows.len() > MAX_SEMANTIC_ROWS
            || axes.is_empty()
            || axes.len() > MAX_SEMANTIC_AXES
            || rows.len().checked_mul(axes.len()) != Some(values.len())
            || masks.len() != rows.len()
        {
            return Err(SemanticGpuError::UnsupportedShape);
        }
        if masks.iter().any(|m| *m != NumericMask::Known) {
            return Err(SemanticGpuError::UnsupportedMask);
        }
        if axes
            .iter()
            .any(|a| a.denominator == 0 || a.minimum > a.maximum)
        {
            return Err(SemanticGpuError::UnsupportedGrid);
        }
        for (index, row) in rows.iter().enumerate() {
            if rows[..index].contains(row) {
                return Err(SemanticGpuError::DuplicateRow);
            }
        }
        for (index, value) in values.iter().enumerate() {
            let a = axes[index % axes.len()];
            if *value < a.minimum || *value > a.maximum {
                return Err(SemanticGpuError::UnsupportedGrid);
            }
        }
        let mut hash = Sha256::new();
        hash.update(b"epistesys.SemanticParetoExactGridV1\0");
        hash.update(binding);
        hash.update(epoch.0.to_le_bytes());
        hash.update((rows.len() as u64).to_le_bytes());
        hash.update((axes.len() as u64).to_le_bytes());
        for axis in &axes {
            hash.update(axis.denominator.to_le_bytes());
            hash.update(axis.minimum.to_le_bytes());
            hash.update(axis.maximum.to_le_bytes());
            hash.update([u8::from(axis.direction == GridDirection::Maximize)]);
        }
        for row in &rows {
            hash.update(row);
        }
        for value in &values {
            hash.update(value.to_le_bytes());
        }
        for mask in &masks {
            hash.update(mask.code().to_le_bytes());
        }
        let digest = hash.finalize().into();
        Ok(Self {
            binding,
            epoch,
            axes,
            rows,
            values,
            masks,
            digest,
        })
    }
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn epoch(&self) -> EpochId {
        self.epoch
    }
    pub fn binding(&self) -> [u8; 32] {
        self.binding
    }
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
    pub fn axes(&self) -> &[GridAxis] {
        &self.axes
    }
    pub fn values(&self) -> &[i32] {
        &self.values
    }
    pub fn masks(&self) -> &[NumericMask] {
        &self.masks
    }
    pub fn requested_buffer_bytes(&self) -> Result<u64, SemanticGpuError> {
        self.values
            .len()
            .checked_add(self.axes.len())
            .and_then(|n| n.checked_add(self.rows.len() * 2))
            .and_then(|n| n.checked_mul(4))
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(SemanticGpuError::RequestedBudget)
    }
}
pub fn cpu_grid_frontier(batch: &CheckedSemanticGridBatch) -> Vec<u32> {
    (0..batch.rows.len())
        .map(|right| {
            let dominated = (0..batch.rows.len()).any(|left| {
                let (mut weak, mut strict) = (true, false);
                for (axis, kind) in batch.axes.iter().enumerate() {
                    let a = batch.values[left * batch.axes.len() + axis];
                    let b = batch.values[right * batch.axes.len() + axis];
                    match kind.direction {
                        GridDirection::Minimize => {
                            weak &= a <= b;
                            strict |= a < b;
                        }
                        GridDirection::Maximize => {
                            weak &= a >= b;
                            strict |= a > b;
                        }
                    }
                }
                left != right && weak && strict
            });
            u32::from(!dominated)
        })
        .collect()
}
pub(super) fn new_owner() -> Option<NonZeroU64> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
        .ok()
        .and_then(NonZeroU64::new)
}
/// Classification reported by the adapter used to initialize the same native client.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticDeviceClass {
    DiscreteGpu,
    IntegratedGpu,
    VirtualGpu,
    Cpu,
    Other,
}
/// Bounded historical adapter metadata, not semantic truth or occupancy evidence.
#[derive(Clone, Debug)]
pub struct SemanticAdapterMetadata {
    pub(crate) name: Box<str>,
    pub(crate) backend: &'static str,
    pub(crate) device_class: SemanticDeviceClass,
    pub(crate) name_truncated: bool,
}
impl SemanticAdapterMetadata {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn backend(&self) -> &str {
        self.backend
    }
    pub fn device_class(&self) -> SemanticDeviceClass {
        self.device_class
    }
    pub fn name_truncated(&self) -> bool {
        self.name_truncated
    }
}
/// Constructed ONLY by this same broker's actual native execution path.
/// No Deserialize, Serialize or public reported-success constructor.
#[cfg(feature = "gpu-cubecl-wgpu")]
pub struct ActualGpuObservation {
    owner: NonZeroU64,
    generation: Generation,
    fence: FenceId,
    receipt: ReceiptId,
    packet: [u8; 32],
    binding: [u8; 32],
    epoch: EpochId,
    frontier: Vec<u32>,
    current_at_completion: bool,
    requested_buffer_bytes: u64,
    buffer_count: u32,
    adapter: SemanticAdapterMetadata,
}
#[cfg(feature = "gpu-cubecl-wgpu")]
impl ActualGpuObservation {
    pub fn adapter(&self) -> &SemanticAdapterMetadata {
        &self.adapter
    }
    pub fn capability(&self) -> SemanticParetoExactGridV1 {
        SemanticParetoExactGridV1
    }
    pub fn packet_digest(&self) -> [u8; 32] {
        self.packet
    }
    /// Historical actual readback only; use same-broker current validation for publication.
    pub fn historical_frontier_words(&self) -> &[u32] {
        &self.frontier
    }
    pub fn current_at_completion(&self) -> bool {
        self.current_at_completion
    }
    pub fn receipt_id(&self) -> ReceiptId {
        self.receipt
    }
    /// Same-state ledger charge for buffers actually created by this run.
    /// Excludes opaque pool/driver/staging allocation; never resident VRAM.
    pub fn requested_buffer_bytes(&self) -> u64 {
        self.requested_buffer_bytes
    }
    pub fn buffer_count(&self) -> u32 {
        self.buffer_count
    }
}
#[cfg(feature = "gpu-cubecl-wgpu")]
pub struct CurrentGpuReadback<'a, 'b> {
    observation: &'a ActualGpuObservation,
    _broker: &'b AcceleratorBroker<crate::CubeClWgpuBackend>,
}
#[cfg(feature = "gpu-cubecl-wgpu")]
impl CurrentGpuReadback<'_, '_> {
    pub fn frontier_words(&self) -> &[u32] {
        &self.observation.frontier
    }
    pub fn binding(&self) -> [u8; 32] {
        self.observation.binding
    }
    pub fn epoch(&self) -> EpochId {
        self.observation.epoch
    }
}
#[cfg(feature = "gpu-cubecl-wgpu")]
impl AcceleratorBroker<crate::CubeClWgpuBackend> {
    pub fn execute_semantic_pareto(
        &mut self,
        batch: &CheckedSemanticGridBatch,
        budget: ScopedSemanticGpuBudget,
        check_current: &mut dyn FnMut() -> bool,
    ) -> Result<ActualGpuObservation, SemanticGpuError> {
        if self.quarantined {
            return Err(SemanticGpuError::Quarantined);
        }
        let Some(owner) = self.semantic_owner else {
            self.quarantined = true;
            return Err(SemanticGpuError::OwnerExhausted);
        };
        let Some(next_receipt) = self.next_receipt.checked_add(1) else {
            self.quarantined = true;
            return Err(SemanticGpuError::OwnerExhausted);
        };
        let Some(next_fence) = self.next_fence.checked_add(1) else {
            self.quarantined = true;
            return Err(SemanticGpuError::OwnerExhausted);
        };
        let native = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::gpu::semantic_pareto::execute(batch, budget, check_current)
        }));
        let native = match native {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                if matches!(
                    e,
                    SemanticGpuError::MayHaveExecuted
                        | SemanticGpuError::InvalidReadback
                        | SemanticGpuError::Quarantined
                ) {
                    self.quarantined = true;
                }
                return Err(e);
            }
            Err(_) => {
                self.quarantined = true;
                return Err(SemanticGpuError::MayHaveExecuted);
            }
        };
        self.next_receipt = next_receipt;
        self.next_fence = next_fence;
        let receipt = ReceiptId(next_receipt);
        if !self.minted.insert(receipt) {
            self.quarantined = true;
            return Err(SemanticGpuError::Quarantined);
        }
        Ok(ActualGpuObservation {
            owner,
            generation: self.generation,
            fence: FenceId(next_fence),
            receipt,
            packet: batch.digest,
            binding: batch.binding,
            epoch: batch.epoch,
            frontier: native.frontier,
            current_at_completion: native.current,
            requested_buffer_bytes: native.requested_buffer_bytes,
            buffer_count: native.buffer_count,
            adapter: native.adapter,
        })
    }
    pub fn check_semantic_current<'a, 'b>(
        &'b self,
        observation: &'a ActualGpuObservation,
    ) -> Result<CurrentGpuReadback<'a, 'b>, SemanticGpuError> {
        if self.semantic_owner != Some(observation.owner) {
            return Err(SemanticGpuError::ForeignObservation);
        }
        if self.quarantined
            || !crate::gpu::semantic_pareto::device_current()
            || self.generation != observation.generation
            || self.next_fence != observation.fence.0
            || !self.minted.contains(&observation.receipt)
            || !observation.current_at_completion
        {
            return Err(SemanticGpuError::StaleObservation);
        }
        Ok(CurrentGpuReadback {
            observation,
            _broker: self,
        })
    }
}
