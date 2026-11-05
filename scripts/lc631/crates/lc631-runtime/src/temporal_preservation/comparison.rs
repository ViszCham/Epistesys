//! 日本語: 既存relation比較とK1正本を利用し、unknownを固定分母に残す。
//! English: Reuse canonical relation comparison/K1; keep unknowns in the fixed denominator.
use super::*;
use lc631_analysis::creativity_measurement::{weighted_assessment, Rational, WeightedProbe};
use lc631_analysis::spatiotemporal_loss as legacy;
use lc631_solvers::temporal::{FiniteTrace, Truth};
use lc631_substrate::measurement_numeric::BigInt;

pub(super) fn compare(
    policy: &TemporalPreservationPolicy,
    source: &FiniteTrace,
    target: &FiniteTrace,
    source_truth: Truth,
    target_truth: Truth,
) -> Result<TemporalPreservationState, TemporalPreservationError> {
    if let Some(map) = policy.mapping() {
        // FiniteTrace checks nonempty and a single exact clock per trace.
        if source.steps()[0].instant().clock() != map.source_clock
            || target.steps()[0].instant().clock() != map.target_clock
        {
            return Ok(TemporalPreservationState::Incomparable(
                IncomparableReason::ClockMismatch,
            ));
        }
    }
    let mut values = Vec::new();
    let mut weights = Vec::new();
    let mut views = Vec::new();
    let count = policy.probes().len();
    values
        .try_reserve_exact(count)
        .map_err(|_| TemporalPreservationError::AllocationFailed)?;
    weights
        .try_reserve_exact(count)
        .map_err(|_| TemporalPreservationError::AllocationFailed)?;
    views
        .try_reserve_exact(count)
        .map_err(|_| TemporalPreservationError::AllocationFailed)?;
    let mut unresolved_probes = 0;
    let mut uncertainty = TemporalUncertainty::default();
    let mut no_scalar = None;
    for probe in policy.probes() {
        let comparison = compare_probe(
            policy,
            probe.relation,
            source,
            target,
            source_truth,
            target_truth,
        )?;
        let (lo, hi) = match comparison {
            legacy::RelationComparison::MatchesDeclaredRelation => (0, 0),
            legacy::RelationComparison::DiffersFromDeclaredRelation => (1, 1),
            legacy::RelationComparison::NeedsEvidence(reason) => {
                unresolved_probes += 1;
                match reason {
                    Gap::MissingObservation => uncertainty.missing_observation += 1,
                    Gap::MissingCalibration => {
                        uncertainty.missing_calibration += 1;
                        no_scalar.get_or_insert(reason);
                        continue;
                    }
                    Gap::MissingEvidence => {
                        uncertainty.missing_evidence += 1;
                        no_scalar.get_or_insert(reason);
                        continue;
                    }
                    Gap::UnsupportedQuantity => {
                        uncertainty.unsupported_quantity += 1;
                        no_scalar.get_or_insert(reason);
                        continue;
                    }
                }
                (0, 1)
            }
            legacy::RelationComparison::Incomparable(reason) => {
                return Ok(TemporalPreservationState::Incomparable(reason))
            }
        };
        values.push(
            RationalInterval::from_ratio(lo, 1, hi, 1)
                .map_err(|e| TemporalPreservationError::Measurement(e.into()))?,
        );
        weights.push(Rational::from_integer(BigInt::from(probe.weight)));
    }
    // Do not drop unsupported probes and renormalize the remaining values.
    // The report retains the original policy denominator and actual pair pins.
    if let Some(reason) = no_scalar {
        return Ok(TemporalPreservationState::NoScalar {
            reason,
            uncertainty,
        });
    }
    for ((probe, value), weight) in policy.probes().iter().zip(&values).zip(&weights) {
        views.push(WeightedProbe {
            canonical_payload: probe.id.as_bytes(),
            weight,
            value,
        });
    }
    let loss = weighted_assessment(&views).map_err(TemporalPreservationError::Measurement)?;
    Ok(TemporalPreservationState::GivenFiniteModel {
        loss,
        unresolved_probes,
        uncertainty,
    })
}

fn compare_probe(
    policy: &TemporalPreservationPolicy,
    relation: ProtectedTemporalRelation,
    source: &FiniteTrace,
    target: &FiniteTrace,
    a: Truth,
    b: Truth,
) -> Result<legacy::RelationComparison, TemporalPreservationError> {
    let Some(mapping) = policy.mapping() else {
        return Ok(legacy::RelationComparison::NeedsEvidence(
            Gap::MissingCalibration,
        ));
    };
    match relation {
        ProtectedTemporalRelation::MappedPredicateTruth => Ok(match (a, b) {
            (Truth::Unknown, _) | (_, Truth::Unknown) => {
                legacy::RelationComparison::NeedsEvidence(Gap::MissingObservation)
            }
            (Truth::True, Truth::True) | (Truth::False, Truth::False) => {
                legacy::RelationComparison::MatchesDeclaredRelation
            }
            (Truth::True, Truth::False) | (Truth::False, Truth::True) => {
                legacy::RelationComparison::DiffersFromDeclaredRelation
            }
        }),
        ProtectedTemporalRelation::Intervals {
            relation,
            source: si,
            target: ti,
        } => {
            let Some(source_intervals) = intervals(source, si, mapping.source_clock)? else {
                return Ok(legacy::RelationComparison::NeedsEvidence(
                    Gap::UnsupportedQuantity,
                ));
            };
            let Some(target_intervals) = intervals(target, ti, mapping.target_clock)? else {
                return Ok(legacy::RelationComparison::NeedsEvidence(
                    Gap::UnsupportedQuantity,
                ));
            };
            let binding = policy.legacy_binding();
            let map = legacy::TimeTranslation::declared(
                binding,
                policy::clock_digest(mapping.source_clock),
                policy::clock_digest(mapping.target_clock),
                mapping.offset_nanoseconds,
                legacy::TimeUnit::Nanosecond,
            );
            legacy::compare_temporal(
                binding,
                relation,
                Some(source_intervals),
                Some(target_intervals),
                Some(map),
            )
            .map(|report| report.comparison())
            .map_err(TemporalPreservationError::TemporalInput)
        }
    }
}

fn intervals(
    trace: &FiniteTrace,
    indices: [u16; 4],
    clock: lc631_solvers::temporal::ClockSpec,
) -> Result<Option<(legacy::TimeInterval, legacy::TimeInterval)>, TemporalPreservationError> {
    let mut endpoints = [0_i64; 4];
    for (slot, index) in endpoints.iter_mut().zip(indices) {
        let Some(step) = trace.steps().get(usize::from(index)) else {
            return Err(TemporalPreservationError::InvalidProbePlan);
        };
        let Ok(value) = i64::try_from(step.instant().nanoseconds()) else {
            return Ok(None);
        };
        *slot = value;
    }
    let instant = |value| {
        legacy::Instant::declared(
            policy::clock_digest(clock),
            value,
            legacy::TimeUnit::Nanosecond,
        )
    };
    let first = legacy::TimeInterval::checked(instant(endpoints[0]), instant(endpoints[1]))
        .map_err(TemporalPreservationError::TemporalInput)?;
    let second = legacy::TimeInterval::checked(instant(endpoints[2]), instant(endpoints[3]))
        .map_err(TemporalPreservationError::TemporalInput)?;
    Ok(Some((first, second)))
}
