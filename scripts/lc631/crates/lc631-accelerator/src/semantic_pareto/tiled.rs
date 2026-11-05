//! Additive bounded tiled numeric profile; never 2048 distinct logical worlds.
#![forbid(unsafe_code)]
use crate::semantic_pareto::*;

pub const MAX_TILED_ROWS: usize = 2048;
pub const TILE_ROWS: usize = 512;
pub const MAX_PAIR_AXES: usize = 8_388_608;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticParetoExactGridTiledV1;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TiledError {
    Input(SemanticGpuError),
    WorkLimit,
    Incomplete {
        completed_tiles: usize,
        reason: SemanticGpuError,
    },
}
/// Owned full ordered numeric payload. Layout checking is not source provenance.
pub struct CheckedTiledBatch {
    binding: [u8; 32],
    epoch: EpochId,
    axes: Vec<GridAxis>,
    rows: Vec<[u8; 32]>,
    values: Vec<i32>,
    masks: Vec<NumericMask>,
    digest: [u8; 32],
    work: usize,
}
impl CheckedTiledBatch {
    pub fn checked_numeric(
        binding: [u8; 32],
        epoch: EpochId,
        axes: Vec<GridAxis>,
        rows: Vec<[u8; 32]>,
        values: Vec<i32>,
        masks: Vec<NumericMask>,
        pair_axis_budget: usize,
    ) -> Result<Self, TiledError> {
        let n = rows.len();
        let work = n
            .checked_mul(n)
            .and_then(|v| v.checked_mul(axes.len()))
            .ok_or(TiledError::WorkLimit)?;
        if pair_axis_budget > MAX_PAIR_AXES || work > pair_axis_budget {
            return Err(TiledError::WorkLimit);
        }
        if n == 0
            || n > MAX_TILED_ROWS
            || axes.is_empty()
            || axes.len() > MAX_SEMANTIC_AXES
            || n.checked_mul(axes.len()) != Some(values.len())
            || masks.len() != n
        {
            return Err(TiledError::Input(SemanticGpuError::UnsupportedShape));
        }
        // Reuse existing complete numeric admission on each <=512-row chunk.
        // Global uniqueness is checked separately, never just within tiles.
        for (i, row) in rows.iter().enumerate() {
            if rows[..i].contains(row) {
                return Err(TiledError::Input(SemanticGpuError::DuplicateRow));
            }
        }
        let mut hash = Sha256::new();
        hash.update(b"epistesys.SemanticParetoExactGridTiledV1\0");
        hash.update(binding);
        hash.update(epoch.0.to_le_bytes());
        hash.update((n as u64).to_le_bytes());
        hash.update((work as u64).to_le_bytes());
        for start in (0..n).step_by(TILE_ROWS) {
            let end = (start + TILE_ROWS).min(n);
            let checked = CheckedSemanticGridBatch::checked_numeric(
                binding,
                epoch,
                axes.clone(),
                rows[start..end].to_vec(),
                values[start * axes.len()..end * axes.len()].to_vec(),
                masks[start..end].to_vec(),
            )
            .map_err(TiledError::Input)?;
            hash.update(checked.digest());
        }
        Ok(Self {
            binding,
            epoch,
            axes,
            rows,
            values,
            masks,
            digest: hash.finalize().into(),
            work,
        })
    }
    pub fn capability(&self) -> SemanticParetoExactGridTiledV1 {
        SemanticParetoExactGridTiledV1
    }
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn binding(&self) -> [u8; 32] {
        self.binding
    }
    pub fn epoch(&self) -> EpochId {
        self.epoch
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
    pub fn pair_axis_work(&self) -> usize {
        self.work
    }
    pub fn tile_count(&self) -> usize {
        self.rows.len().div_ceil(TILE_ROWS).pow(2)
    }
    pub fn requested_buffer_bytes(&self) -> Result<u64, TiledError> {
        self.values
            .len()
            .checked_add(self.axes.len())
            .and_then(|v| v.checked_add(self.rows.len().checked_mul(2)?))
            .and_then(|v| v.checked_mul(4))
            .and_then(|v| u64::try_from(v).ok())
            .ok_or(TiledError::Input(SemanticGpuError::RequestedBudget))
    }
}
/// Independent scalar algorithm traverses all original rows without tile logic.
pub fn cpu_tiled_frontier(batch: &CheckedTiledBatch) -> Vec<u32> {
    (0..batch.row_count())
        .map(|r| {
            let dominated = (0..batch.row_count()).any(|l| {
                if l == r {
                    return false;
                }
                let (mut weak, mut strict) = (true, false);
                for (k, a) in batch.axes.iter().enumerate() {
                    let (x, y) = (
                        batch.values[l * batch.axes.len() + k],
                        batch.values[r * batch.axes.len() + k],
                    );
                    match a.direction {
                        GridDirection::Minimize => {
                            weak &= x <= y;
                            strict |= x < y;
                        }
                        GridDirection::Maximize => {
                            weak &= x >= y;
                            strict |= x > y;
                        }
                    }
                }
                weak && strict
            });
            u32::from(!dominated)
        })
        .collect()
}
#[cfg(feature = "gpu-cubecl-wgpu")]
pub struct ActualTiledObservation {
    native: ActualGpuObservation,
    tiles: usize,
}
#[cfg(feature = "gpu-cubecl-wgpu")]
impl ActualTiledObservation {
    pub fn capability(&self) -> SemanticParetoExactGridTiledV1 {
        SemanticParetoExactGridTiledV1
    }
    pub fn historical_frontier_words(&self) -> &[u32] {
        self.native.historical_frontier_words()
    }
    pub fn packet_digest(&self) -> [u8; 32] {
        self.native.packet_digest()
    }
    pub fn adapter(&self) -> &SemanticAdapterMetadata {
        self.native.adapter()
    }
    pub fn completed_tiles(&self) -> usize {
        self.tiles
    }
    pub fn requested_buffer_bytes(&self) -> u64 {
        self.native.requested_buffer_bytes()
    }
    pub fn buffer_count(&self) -> u32 {
        self.native.buffer_count()
    }
    pub fn current_at_completion(&self) -> bool {
        self.native.current_at_completion()
    }
}
#[cfg(feature = "gpu-cubecl-wgpu")]
impl AcceleratorBroker<crate::CubeClWgpuBackend> {
    pub fn execute_semantic_tiled(
        &mut self,
        batch: &CheckedTiledBatch,
        budget: ScopedSemanticGpuBudget,
        check: &mut dyn FnMut() -> bool,
    ) -> Result<ActualTiledObservation, TiledError> {
        if self.quarantined {
            return Err(TiledError::Input(SemanticGpuError::Quarantined));
        }
        let Some(owner) = self.semantic_owner else {
            self.quarantined = true;
            return Err(TiledError::Input(SemanticGpuError::OwnerExhausted));
        };
        let Some(receipt) = self.next_receipt.checked_add(1) else {
            self.quarantined = true;
            return Err(TiledError::Input(SemanticGpuError::OwnerExhausted));
        };
        let Some(fence) = self.next_fence.checked_add(1) else {
            self.quarantined = true;
            return Err(TiledError::Input(SemanticGpuError::OwnerExhausted));
        };
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::gpu::semantic_pareto::tiled::execute(batch, budget, check)
        }));
        let native = match run {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                if matches!(
                    e,
                    TiledError::Incomplete { .. }
                        | TiledError::Input(SemanticGpuError::Quarantined)
                ) {
                    self.quarantined = true;
                }
                return Err(e);
            }
            Err(_) => {
                self.quarantined = true;
                return Err(TiledError::Incomplete {
                    completed_tiles: 0,
                    reason: SemanticGpuError::MayHaveExecuted,
                });
            }
        };
        self.next_receipt = receipt;
        self.next_fence = fence;
        if !self.minted.insert(ReceiptId(receipt)) {
            self.quarantined = true;
            return Err(TiledError::Incomplete {
                completed_tiles: batch.tile_count(),
                reason: SemanticGpuError::Quarantined,
            });
        }
        Ok(ActualTiledObservation {
            native: ActualGpuObservation {
                owner,
                generation: self.generation,
                fence: FenceId(fence),
                receipt: ReceiptId(receipt),
                packet: batch.digest,
                binding: batch.binding,
                epoch: batch.epoch,
                frontier: native.frontier,
                current_at_completion: native.current,
                requested_buffer_bytes: native.requested_buffer_bytes,
                buffer_count: native.buffer_count,
                adapter: native.adapter,
            },
            tiles: batch.tile_count(),
        })
    }
    pub fn check_semantic_tiled_current<'a, 'b>(
        &'b self,
        obs: &'a ActualTiledObservation,
    ) -> Result<CurrentGpuReadback<'a, 'b>, SemanticGpuError> {
        self.check_semantic_current(&obs.native)
    }
}
