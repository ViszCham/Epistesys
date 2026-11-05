//! Lossless actual rational batch -> same-broker native profile -> exact CPU final.
//! Native observation is over submitted bytes, not semantic/provenance validation.
#![forbid(unsafe_code)]
use super::*;
use lc631_accelerator::semantic_pareto::{
    ActualGpuObservation, CheckedSemanticGridBatch, CurrentGpuReadback, GridAxis, GridDirection,
    ScopedSemanticGpuBudget, SemanticGpuError,
};
use lc631_accelerator::{AcceleratorBroker, CubeClWgpuBackend, NumericMask};
use lc631_core::measurement_numeric::{checked_multiply, BigInt};
use lc631_core::EpochId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridUnsupported {
    Shape,
    Direction,
    NonExact,
    NonPoint,
    Fraction,
    Range,
    NumericBudget,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreshCpuReason {
    Unsupported(GridUnsupported),
    Backend(SemanticGpuError),
}
/// Keep the actual readback and its RAM/output claim inline on both native
/// branches, including CPU rejection or cancellation after execution. Boxing an
/// outcome after dispatch would add an allocation before this mandatory evidence
/// can be returned. A fallible pre-dispatch boxed owner would require another
/// allocation/reservation contract solely to reduce this bounded enum's size.
#[allow(
    clippy::large_enum_variant,
    reason = "Inline native outcomes preserve the observation and retained claim without post-effect boxing"
)]
pub enum SemanticGridOutcome<'a, 'v, 'r> {
    ObservedCurrent {
        selection: CurrentSelection<'a, 'v, 'r>,
        observation: OwnedGpuObservation<'r>,
    },
    ObservedRejected {
        observation: OwnedGpuObservation<'r>,
        reason: ConsumerError,
    },
    FreshCpu {
        selection: CurrentSelection<'a, 'v, 'r>,
        reason: FreshCpuReason,
    },
}
/// The actual host vector cannot detach from its RAM/output claim. Cancellation
/// removes currentness, never its retained accounting or historical observation.
pub struct OwnedGpuObservation<'r> {
    observation: ActualGpuObservation,
    _retained: ResourceClaim<'r>,
}
impl OwnedGpuObservation<'_> {
    pub fn historical(&self) -> &ActualGpuObservation {
        &self.observation
    }
    pub fn check_current<'a, 'b>(
        &'a self,
        broker: &'b AcceleratorBroker<CubeClWgpuBackend>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<CurrentGpuReadback<'a, 'b>, GridRunError> {
        current(&self._retained, resources).map_err(GridRunError::Consumer)?;
        broker
            .check_semantic_current(&self.observation)
            .map_err(|_| GridRunError::Consumer(ConsumerError::StaleCandidate))
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GridRunError {
    Consumer(ConsumerError),
    BackendAndCpu {
        backend: SemanticGpuError,
        cpu: ConsumerError,
    },
}

/// The only exact-selection lowering is private to this current-owner consumer.
/// Pure layout constructors in accelerator do not establish semantic provenance.
fn lower_exact(
    batch: &NumericBatch<'_, '_, '_, '_>,
    axes: &[GridAxis],
) -> Result<CheckedSemanticGridBatch, GridUnsupported> {
    if batch.rows.len() > 512 || axes.len() != batch.policy.axes.len() {
        return Err(GridUnsupported::Shape);
    }
    let mut values = Vec::new();
    let mut identities = Vec::new();
    values
        .try_reserve_exact(batch.rows.len() * axes.len())
        .map_err(|_| GridUnsupported::Shape)?;
    identities
        .try_reserve_exact(batch.rows.len())
        .map_err(|_| GridUnsupported::Shape)?;
    for row in batch.rows {
        let mut hash = Sha256::new();
        hash.update(row.id.0.to_le_bytes());
        hash.update(row.measurement_binding.as_bytes());
        hash.update(row.context.interpretation_digest().as_bytes());
        hash.update(row.canonical_key);
        identities.push(hash.finalize().into());
        for ((value, definition), kind) in row.axes.iter().zip(axes).zip(batch.policy.axes) {
            if (definition.direction == GridDirection::Maximize) != kind.maximize() {
                return Err(GridUnsupported::Direction);
            }
            if definition.denominator == 0 {
                return Err(GridUnsupported::Range);
            }
            let interval = match value {
                AxisValue::Exact(v) => *v,
                AxisValue::Bounded(_) => return Err(GridUnsupported::NonExact),
                AxisValue::Estimated(_) => return Err(GridUnsupported::NonExact),
                AxisValue::Unavailable(_) => return Err(GridUnsupported::NonExact),
            };
            if !interval.is_exact() {
                return Err(GridUnsupported::NonPoint);
            }
            let scale = Rational::from_integer(BigInt::from(definition.denominator));
            let scaled = checked_multiply(interval.lower(), &scale)
                .map_err(|_| GridUnsupported::NumericBudget)?;
            if scaled.denom() != &BigInt::from(1) {
                return Err(GridUnsupported::Fraction);
            }
            let n = i32::try_from(scaled.numer()).map_err(|_| GridUnsupported::Range)?;
            if n < definition.minimum || n > definition.maximum {
                return Err(GridUnsupported::Range);
            }
            values.push(n);
        }
    }
    CheckedSemanticGridBatch::checked_numeric(
        *batch.token.as_bytes(),
        EpochId(batch.stamp.epoch),
        axes.to_vec(),
        identities,
        values,
        vec![NumericMask::Known; batch.rows.len()],
    )
    .map_err(|_| GridUnsupported::Shape)
}

/// Same broker reference is needed both to dispatch and to recheck its actual seal.
/// Unsupported input performs explicitly fresh exact CPU work, never cached work.
pub fn run_semantic_grid<'a, 'v, 'r>(
    batch: &NumericBatch<'a, '_, 'v, '_>,
    axes: &[GridAxis],
    broker: &mut AcceleratorBroker<CubeClWgpuBackend>,
    budget: ScopedSemanticGpuBudget,
    resources: &'r ResourceReservationView<'_>,
) -> Result<SemanticGridOutcome<'a, 'v, 'r>, GridRunError> {
    current(&batch.claim, resources).map_err(GridRunError::Consumer)?;
    let _transport = resources
        .claim_resources(&[
            request(ResourceKind::Ram, RETAINED_RAM).map_err(GridRunError::Consumer)?
        ])
        .map_err(|e| GridRunError::Consumer(ConsumerError::Resource(e)))?;
    let packet = match lower_exact(batch, axes) {
        Ok(v) => v,
        Err(reason) => {
            return Ok(SemanticGridOutcome::FreshCpu {
                selection: finish_selection(batch, None, resources)
                    .map_err(GridRunError::Consumer)?,
                reason: FreshCpuReason::Unsupported(reason),
            })
        }
    };
    let retained = resources
        .claim_resources(&[
            request(ResourceKind::Ram, RETAINED_RAM).map_err(GridRunError::Consumer)?,
            request(ResourceKind::Output, RETAINED_RAM).map_err(GridRunError::Consumer)?,
        ])
        .map_err(|e| GridRunError::Consumer(ConsumerError::Resource(e)))?;
    let mut check = || current(&batch.claim, resources).is_ok();
    let observation = match broker.execute_semantic_pareto(&packet, budget, &mut check) {
        Ok(v) => v,
        Err(backend) => {
            let selection = finish_selection(batch, None, resources)
                .map_err(|cpu| GridRunError::BackendAndCpu { backend, cpu })?;
            return Ok(SemanticGridOutcome::FreshCpu {
                selection,
                reason: FreshCpuReason::Backend(backend),
            });
        }
    };
    let observation = OwnedGpuObservation {
        observation,
        _retained: retained,
    };
    if !observation.observation.current_at_completion() {
        let reason = current(&batch.claim, resources)
            .err()
            .unwrap_or(ConsumerError::StaleCandidate);
        return Ok(SemanticGridOutcome::ObservedRejected {
            observation,
            reason,
        });
    }
    let verified = (|| {
        let readback = broker
            .check_semantic_current(&observation.observation)
            .map_err(|_| ConsumerError::StaleCandidate)?;
        if observation.observation.packet_digest() != packet.digest()
            || readback.binding() != *batch.token.as_bytes()
            || readback.epoch() != EpochId(batch.stamp.epoch)
        {
            return Err(ConsumerError::StaleCandidate);
        }
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(readback.frontier_words().len())
            .map_err(|_| ConsumerError::AllocationFailed)?;
        for v in readback.frontier_words() {
            if *v > 1 {
                return Err(ConsumerError::ConsumerMismatch {
                    row: retained.len(),
                });
            }
            retained.push(*v == 1);
        }
        finish_selection(
            batch,
            Some(FrontierCandidate {
                token: batch.token,
                stamp: batch.stamp,
                retained: &retained,
            }),
            resources,
        )
    })();
    Ok(match verified {
        Ok(selection) => SemanticGridOutcome::ObservedCurrent {
            selection,
            observation,
        },
        Err(reason) => SemanticGridOutcome::ObservedRejected {
            observation,
            reason,
        },
    })
}
