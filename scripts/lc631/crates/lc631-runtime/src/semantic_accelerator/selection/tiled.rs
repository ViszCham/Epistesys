//! Actual rational NumericBatch -> additive tiled native profile -> CPU final.
//! This numeric bridge alone establishes neither world provenance nor distinctness.
#![forbid(unsafe_code)]
pub mod world;
use super::*;
use lc631_accelerator::semantic_pareto::{
    tiled::*, CurrentGpuReadback, GridAxis, GridDirection, ScopedSemanticGpuBudget,
    SemanticAdapterMetadata,
};
use lc631_accelerator::{AcceleratorBroker, CubeClWgpuBackend, NumericMask};
use lc631_core::{
    measurement_numeric::{checked_multiply, BigInt},
    EpochId,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TiledUnsupported {
    Shape,
    Direction,
    NonExact,
    Fraction,
    Range,
    Work,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TiledRunError {
    Consumer(ConsumerError),
    Native(TiledError),
}
pub struct OwnedTiledObservation<'r> {
    actual: ActualTiledObservation,
    _claim: ResourceClaim<'r>,
}
impl OwnedTiledObservation<'_> {
    pub fn historical_frontier_words(&self) -> &[u32] {
        self.actual.historical_frontier_words()
    }
    pub fn adapter(&self) -> &SemanticAdapterMetadata {
        self.actual.adapter()
    }
    pub fn check_current<'a, 'b>(
        &'a self,
        broker: &'b AcceleratorBroker<CubeClWgpuBackend>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<CurrentGpuReadback<'a, 'b>, TiledRunError> {
        current(&self._claim, resources).map_err(TiledRunError::Consumer)?;
        broker
            .check_semantic_tiled_current(&self.actual)
            .map_err(|e| TiledRunError::Native(TiledError::Input(e)))
    }
}
#[allow(
    clippy::large_enum_variant,
    reason = "Inline actual observation keeps its retained claim without post-effect boxing"
)]
pub enum TiledOutcome<'a, 'v, 'r> {
    ObservedCurrent {
        selection: CurrentSelection<'a, 'v, 'r>,
        observation: OwnedTiledObservation<'r>,
    },
    ObservedRejected {
        reason: ConsumerError,
        observation: OwnedTiledObservation<'r>,
    },
    FreshCpuUnsupported {
        reason: TiledUnsupported,
        selection: CurrentSelection<'a, 'v, 'r>,
    },
}
fn lower(
    batch: &NumericBatch<'_, '_, '_, '_>,
    axes: &[GridAxis],
) -> Result<CheckedTiledBatch, TiledUnsupported> {
    if batch.rows.len() > MAX_TILED_ROWS || axes.len() != batch.policy.axes.len() {
        return Err(TiledUnsupported::Shape);
    }
    let count = batch
        .rows
        .len()
        .checked_mul(axes.len())
        .ok_or(TiledUnsupported::Work)?;
    let mut values = Vec::new();
    let mut ids = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| TiledUnsupported::Work)?;
    ids.try_reserve_exact(batch.rows.len())
        .map_err(|_| TiledUnsupported::Work)?;
    for row in batch.rows {
        let mut hash = Sha256::new();
        hash.update(row.id.0.to_le_bytes());
        hash.update(row.measurement_binding.as_bytes());
        hash.update(row.context.interpretation_digest().as_bytes());
        hash.update(row.canonical_key);
        ids.push(hash.finalize().into());
        for ((value, axis), kind) in row.axes.iter().zip(axes).zip(batch.policy.axes) {
            if (axis.direction == GridDirection::Maximize) != kind.maximize() {
                return Err(TiledUnsupported::Direction);
            }
            if axis.denominator == 0 {
                return Err(TiledUnsupported::Range);
            }
            let interval = match value {
                AxisValue::Exact(v) if v.is_exact() => v,
                AxisValue::Exact(_)
                | AxisValue::Bounded(_)
                | AxisValue::Estimated(_)
                | AxisValue::Unavailable(_) => return Err(TiledUnsupported::NonExact),
            };
            let scaled = checked_multiply(
                interval.lower(),
                &Rational::from_integer(BigInt::from(axis.denominator)),
            )
            .map_err(|_| TiledUnsupported::Range)?;
            if scaled.denom() != &BigInt::from(1) {
                return Err(TiledUnsupported::Fraction);
            }
            let n = i32::try_from(scaled.numer()).map_err(|_| TiledUnsupported::Range)?;
            if n < axis.minimum || n > axis.maximum {
                return Err(TiledUnsupported::Range);
            }
            values.push(n);
        }
    }
    CheckedTiledBatch::checked_numeric(
        *batch.token.as_bytes(),
        EpochId(batch.stamp.epoch),
        axes.to_vec(),
        ids,
        values,
        vec![NumericMask::Known; batch.rows.len()],
        super::super::MAX_COMPARISONS,
    )
    .map_err(|_| TiledUnsupported::Work)
}
/// Never silently turns a partially executed GPU run into a final observation.
/// Unsupported exact layout explicitly computes a fresh CPU selection.
pub fn run_semantic_tiled<'a, 'v, 'r>(
    batch: &NumericBatch<'a, '_, 'v, '_>,
    axes: &[GridAxis],
    broker: &mut AcceleratorBroker<CubeClWgpuBackend>,
    budget: ScopedSemanticGpuBudget,
    resources: &'r ResourceReservationView<'_>,
) -> Result<TiledOutcome<'a, 'v, 'r>, TiledRunError> {
    current(&batch.claim, resources).map_err(TiledRunError::Consumer)?;
    let _transport = resources
        .claim_resources(&[
            request(ResourceKind::Ram, RETAINED_RAM).map_err(TiledRunError::Consumer)?
        ])
        .map_err(|e| TiledRunError::Consumer(ConsumerError::Resource(e)))?;
    let packet = match lower(batch, axes) {
        Ok(v) => v,
        Err(reason) => {
            return Ok(TiledOutcome::FreshCpuUnsupported {
                reason,
                selection: finish_selection(batch, None, resources)
                    .map_err(TiledRunError::Consumer)?,
            })
        }
    };
    let retained = resources
        .claim_resources(&[
            request(ResourceKind::Ram, RETAINED_RAM).map_err(TiledRunError::Consumer)?,
            request(ResourceKind::Output, RETAINED_RAM).map_err(TiledRunError::Consumer)?,
        ])
        .map_err(|e| TiledRunError::Consumer(ConsumerError::Resource(e)))?;
    // Preallocate the candidate bool vector before native effects.
    let mut full = Vec::new();
    full.try_reserve_exact(batch.rows.len())
        .map_err(|_| TiledRunError::Consumer(ConsumerError::AllocationFailed))?;
    let mut check = || current(&batch.claim, resources).is_ok();
    let actual = broker
        .execute_semantic_tiled(&packet, budget, &mut check)
        .map_err(TiledRunError::Native)?;
    let observation = OwnedTiledObservation {
        actual,
        _claim: retained,
    };
    let verified = (|| {
        let read = broker
            .check_semantic_tiled_current(&observation.actual)
            .map_err(|_| ConsumerError::StaleCandidate)?;
        if observation.actual.packet_digest() != packet.digest()
            || read.binding() != *batch.token.as_bytes()
            || read.epoch() != EpochId(batch.stamp.epoch)
        {
            return Err(ConsumerError::StaleCandidate);
        }
        for value in read.frontier_words() {
            if *value > 1 {
                return Err(ConsumerError::ConsumerMismatch { row: full.len() });
            }
            full.push(*value == 1);
        }
        finish_selection(
            batch,
            Some(FrontierCandidate {
                token: batch.token,
                stamp: batch.stamp,
                retained: &full,
            }),
            resources,
        )
    })();
    Ok(match verified {
        Ok(selection) => TiledOutcome::ObservedCurrent {
            selection,
            observation,
        },
        Err(reason) => TiledOutcome::ObservedRejected {
            reason,
            observation,
        },
    })
}
