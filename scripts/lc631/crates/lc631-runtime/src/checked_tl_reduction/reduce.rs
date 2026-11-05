//! Canonical K1 over real live owners. No report/interval input constructor.
use super::*;
use crate::joint_selection::AxisValue;
use crate::spatiotemporal_selection::{SpatialOwner, TemporalOwner};
use crate::{
    spatial_preservation::SpatialPreservationPolicy,
    temporal_preservation::TemporalPreservationPolicy,
};
use crate::spatial_preservation::SpatialModelInput;
use crate::temporal_preservation::TemporalModelInput;
use lc631_substrate::Digest32;
use lc631_analysis::creativity_measurement::{weighted_assessment, Rational, WeightedProbe};

/// Callback-scoped actual tuple inputs and original live producer references.
pub(crate) struct ProductTupleView<'p, 'i, 's, 'v> {
    pub(crate) source_temporal: TemporalModelInput<'p>,
    pub(crate) source_spatial: SpatialModelInput,
    pub(crate) target_temporal: TemporalModelInput<'p>,
    pub(crate) target_spatial: SpatialModelInput,
    pub(crate) source_temporal_policy: &'p TemporalPreservationPolicy,
    pub(crate) source_spatial_policy: &'p SpatialPreservationPolicy,
    pub(crate) temporal_owner: Option<&'p TemporalOwner<'i, 'v>>,
    pub(crate) spatial_owner: Option<&'p SpatialOwner<'s, 'v>>,
    pub(crate) report: &'p Report,
    pub(crate) root_digest: Digest32,
}

pub struct Owners<'a, 'input, 'policy, 'view> {
    pub temporal: Option<&'a TemporalOwner<'input, 'view>>,
    pub spatial: Option<&'a SpatialOwner<'policy, 'view>>,
}
/// Borrowed actual owners and source/candidate/policy outlive all current uses.
/// Report backing drops before the retained claim; historical export is not renewed.
pub struct OwnedReduction<'a, 'root, 'input, 'policy, 'view> {
    report: Report,
    root: &'root RootPlan<'input, 'policy, 'view>,
    candidate: &'a TupleCandidate<'root, 'input, 'policy, 'view>,
    owners: Owners<'a, 'input, 'policy, 'view>,
    retained: ResourceClaim<'view>,
}
impl<'a, 'root, 'input, 'policy, 'view> OwnedReduction<'a, 'root, 'input, 'policy, 'view> {
    pub(crate) fn inspect_product_tuple_current<R>(
        &self,
        root: &RootPlan<'input, 'policy, 'view>,
        candidate: &TupleCandidate<'root, 'input, 'policy, 'view>,
        resources: &ResourceReservationView<'_>,
        consume: impl for<'p> FnOnce(ProductTupleView<'p, 'input, 'policy, 'view>) -> R,
    ) -> Result<R, Error> {
        self.current(root, candidate, resources)?;
        let result = consume(ProductTupleView {
            source_temporal: root.temporal,
            source_spatial: root.spatial,
            target_temporal: candidate.temporal,
            target_spatial: candidate.spatial,
            source_temporal_policy: root.temporal_policy,
            source_spatial_policy: root.spatial_policy,
            temporal_owner: self.owners.temporal,
            spatial_owner: self.owners.spatial,
            report: &self.report,
            root_digest: root.digest,
        });
        self.current(root, candidate, resources)?;
        Ok(result)
    }
    pub fn historical(&self) -> &Report {
        &self.report
    }
    pub fn current(
        &self,
        root: &RootPlan<'input, 'policy, 'view>,
        candidate: &TupleCandidate<'root, 'input, 'policy, 'view>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&Report, Error> {
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(resources)
            || !std::ptr::eq(root, self.root)
            || !std::ptr::eq(candidate, self.candidate)
        {
            return Err(Error::BindingMismatch);
        }
        // Recheck source, exact transform policy and both private live owners.
        // Scratch is charged before reconstructing bounded policies at every read.
        let scratch = claim(resources, SCRATCH)?;
        let _ = axes(root, candidate, &self.owners, resources)?;
        scratch.check().map_err(Error::Resource)?;
        self.retained.check().map_err(Error::Resource)?;
        Ok(&self.report)
    }
}
pub fn reduce_current<'a, 'root, 'input, 'policy, 'view>(
    root: &'root RootPlan<'input, 'policy, 'view>,
    candidate: &'a TupleCandidate<'root, 'input, 'policy, 'view>,
    owners: Owners<'a, 'input, 'policy, 'view>,
    resources: &'view ResourceReservationView<'_>,
) -> Result<OwnedReduction<'a, 'root, 'input, 'policy, 'view>, Error> {
    let scratch = claim(resources, SCRATCH)?;
    let retained = claim(resources, RETAINED)?;
    let values = axes(root, candidate, &owners, resources)?;
    let gaps = [gap(&values[0]), gap(&values[1])];
    let gates = [gate(&values[0]), gate(&values[1])];
    let total = match (values[0].interval(), values[1].interval()) {
        (Some(t), Some(s)) => {
            let one = Rational::from_integer(1.into());
            let three = Rational::from_integer(3.into());
            Some(
                weighted_assessment(&[
                    WeightedProbe {
                        canonical_payload: b"TemporalLoss",
                        weight: &one,
                        value: t,
                    },
                    WeightedProbe {
                        canonical_payload: b"SpatialLoss",
                        weight: &three,
                        value: s,
                    },
                ])
                .map_err(Error::Arithmetic)?,
            )
        }
        _ => None,
    };
    // Neither unsupported/missing axis is dropped; W remains4 in every report.
    scratch.check().map_err(Error::Resource)?;
    retained.check().map_err(Error::Resource)?;
    Ok(OwnedReduction {
        report: Report { total, gaps, gates },
        root,
        candidate,
        owners,
        retained,
    })
}
fn axes<'a>(
    root: &RootPlan<'_, '_, '_>,
    candidate: &TupleCandidate<'_, '_, '_, '_>,
    owners: &Owners<'a, '_, '_, '_>,
    resources: &ResourceReservationView<'_>,
) -> Result<[AxisValue<'a>; 2], Error> {
    root.check(resources)?;
    if !std::ptr::eq(root, candidate.root) {
        return Err(Error::BindingMismatch);
    }
    let t = match owners.temporal {
        None => AxisValue::Unavailable(MissingAxis::MissingEvaluator),
        Some(owner) => {
            let expected = TemporalPreservationPolicy::declared(
                root.temporal,
                candidate.temporal,
                root.temporal_policy.probes().to_vec(),
                root.temporal_policy.mapping(),
            )
            .map_err(Error::Temporal)?;
            let report = owner.historical_report();
            if expected.binding_digest() != report.binding_digest()
                || expected.digest() != report.policy_digest()
                || expected.denominator() != report.denominator()
            {
                return Err(Error::BindingMismatch);
            }
            owner.axis_current(resources).map_err(Error::Producer)?
        }
    };
    let s = match owners.spatial {
        None => AxisValue::Unavailable(MissingAxis::MissingEvaluator),
        Some(owner) => {
            let expected = SpatialPreservationPolicy::declared(
                root.spatial,
                candidate.spatial,
                root.spatial_policy.probes().to_vec(),
                root.spatial_policy.mapping(),
            )
            .map_err(Error::Spatial)?;
            let report = owner.historical_report();
            if expected.binding_digest() != report.binding_digest()
                || expected.digest() != report.policy_digest()
                || expected.denominator() != report.denominator()
            {
                return Err(Error::BindingMismatch);
            }
            owner.axis_current(resources).map_err(Error::Producer)?
        }
    };
    Ok([t, s])
}
fn gap(value: &AxisValue<'_>) -> Option<NoScalar> {
    match value {
        AxisValue::Unavailable(MissingAxis::MissingEvaluator) => Some(NoScalar::MissingOwner),
        AxisValue::Unavailable(reason) => Some(NoScalar::Unavailable(*reason)),
        AxisValue::Exact(_) | AxisValue::Bounded(_) | AxisValue::Estimated(_) => None,
    }
}
fn gate(value: &AxisValue<'_>) -> Gate {
    match value.interval() {
        None => Gate::NoScalar,
        Some(v) => {
            let zero = Rational::from_integer(0.into());
            if v.upper() <= &zero {
                Gate::Pass
            } else if v.lower() > &zero {
                Gate::Fail
            } else {
                Gate::NeedsEvidence
            }
        }
    }
}
pub mod selection;
