//! 日本語: 既存GridMap/compare_spatial/K1のみで数値比較する。
//! English: Numeric comparison reuses GridMap/compare_spatial/K1.
use super::*;
use lc631_analysis::creativity_measurement::{weighted_assessment, Rational, WeightedProbe};
use lc631_analysis::spatiotemporal_loss as old;
use lc631_substrate::measurement_numeric::BigInt;

pub(super) fn compare(
    policy: &SpatialPreservationPolicy,
    source: SpatialModelInput,
    target: SpatialModelInput,
    a: [Option<bool>; 4],
    b: [Option<bool>; 4],
) -> Result<SpatialPreservationState, SpatialPreservationError> {
    let mut uncertainty = SpatialUncertainty::default();
    let Some(map) = policy.mapping() else {
        uncertainty.missing_mapping = policy.probes().len();
        uncertainty.unsupported_quantity = policy
            .probes()
            .iter()
            .filter(|probe| probe.relation == ProtectedSpatialRelation::ContinuousDistance)
            .count();
        return Ok(SpatialPreservationState::NoScalar {
            reason: Gap::MissingCalibration,
            uncertainty,
        });
    };
    if map.source_frame != source.frame || map.target_frame != target.frame {
        return Ok(SpatialPreservationState::Incomparable(
            IncomparableReason::FrameMismatch,
        ));
    }
    if source.unit != map.source_unit
        || target.unit != map.target_unit
        || source.unit != target.unit
    {
        return Ok(SpatialPreservationState::Incomparable(
            IncomparableReason::UnitMismatch,
        ));
    }
    let mut values = Vec::new();
    let mut weights = Vec::new();
    let mut views = Vec::new();
    values
        .try_reserve_exact(6)
        .map_err(|_| SpatialPreservationError::AllocationFailed)?;
    weights
        .try_reserve_exact(6)
        .map_err(|_| SpatialPreservationError::AllocationFailed)?;
    views
        .try_reserve_exact(6)
        .map_err(|_| SpatialPreservationError::AllocationFailed)?;
    for probe in policy.probes() {
        let result = match probe.relation {
            ProtectedSpatialRelation::ContinuousDistance => {
                uncertainty.unsupported_quantity += 1;
                continue;
            }
            ProtectedSpatialRelation::Kernel(relation) => {
                let index = RELATIONS
                    .iter()
                    .position(|value| *value == relation)
                    .ok_or(SpatialPreservationError::InvalidProbePlan)?;
                match (a[index], b[index]) {
                    (Some(left), Some(right)) if left == right => {
                        old::RelationComparison::MatchesDeclaredRelation
                    }
                    (Some(_), Some(_)) => old::RelationComparison::DiffersFromDeclaredRelation,
                    (None, _) | (_, None) => {
                        old::RelationComparison::NeedsEvidence(Gap::MissingObservation)
                    }
                }
            }
            ProtectedSpatialRelation::MappedPlacement => placement(policy, source, target, map),
        };
        let (lo, hi) = match result {
            old::RelationComparison::MatchesDeclaredRelation => (0, 0),
            old::RelationComparison::DiffersFromDeclaredRelation => (1, 1),
            old::RelationComparison::NeedsEvidence(Gap::MissingObservation) => {
                uncertainty.missing_objects += 1;
                (0, 1)
            }
            old::RelationComparison::NeedsEvidence(reason) => {
                return Ok(SpatialPreservationState::NoScalar {
                    reason,
                    uncertainty,
                })
            }
            old::RelationComparison::Incomparable(reason) => {
                return Ok(SpatialPreservationState::Incomparable(reason))
            }
        };
        values.push(
            RationalInterval::from_ratio(lo, 1, hi, 1)
                .map_err(|e| SpatialPreservationError::Measurement(e.into()))?,
        );
        weights.push(Rational::from_integer(BigInt::from(probe.weight)));
    }
    if uncertainty.unsupported_quantity != 0 {
        return Ok(SpatialPreservationState::NoScalar {
            reason: Gap::UnsupportedQuantity,
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
    let loss = weighted_assessment(&views).map_err(SpatialPreservationError::Measurement)?;
    Ok(SpatialPreservationState::GivenFiniteModel { loss, uncertainty })
}

fn placement(
    policy: &SpatialPreservationPolicy,
    source: SpatialModelInput,
    target: SpatialModelInput,
    map: DeclaredSpatialMap,
) -> old::RelationComparison {
    let binding = policy.legacy_binding();
    let sf = old::Frame2::declared(Digest32::from_bytes(map.source_frame.0), map.source_unit);
    let tf = old::Frame2::declared(Digest32::from_bytes(map.target_frame.0), map.target_unit);
    let mapping = old::GridMap2::declared(binding, sf, tf, map.signs, map.translation);
    let mut missing = false;
    let mut changed = false;
    for (a, b) in [(source.left, target.left), (source.right, target.right)] {
        let (Some(a), Some(b)) = (a, b) else {
            missing = true;
            continue;
        };
        let ap = (
            old::Point2::declared(sf, a.min()[0], a.min()[1]),
            old::Point2::declared(sf, a.max()[0], a.max()[1]),
        );
        let first = [
            if map.signs.0 == AxisSign::Negative {
                b.max()[0]
            } else {
                b.min()[0]
            },
            if map.signs.1 == AxisSign::Negative {
                b.max()[1]
            } else {
                b.min()[1]
            },
        ];
        let last = [
            if map.signs.0 == AxisSign::Negative {
                b.min()[0]
            } else {
                b.max()[0]
            },
            if map.signs.1 == AxisSign::Negative {
                b.min()[1]
            } else {
                b.max()[1]
            },
        ];
        let bp = (
            old::Point2::declared(tf, first[0], first[1]),
            old::Point2::declared(tf, last[0], last[1]),
        );
        match old::compare_spatial(
            binding,
            old::SpatialProbe::MappedPlacement,
            Some(ap),
            Some(bp),
            Some(mapping),
        )
        .comparison()
        {
            old::RelationComparison::MatchesDeclaredRelation => {}
            old::RelationComparison::DiffersFromDeclaredRelation => changed = true,
            old::RelationComparison::NeedsEvidence(Gap::MissingObservation) => missing = true,
            other @ (old::RelationComparison::NeedsEvidence(_)
            | old::RelationComparison::Incomparable(_)) => return other,
        }
    }
    if changed {
        old::RelationComparison::DiffersFromDeclaredRelation
    } else if missing {
        old::RelationComparison::NeedsEvidence(Gap::MissingObservation)
    } else {
        old::RelationComparison::MatchesDeclaredRelation
    }
}
