//! Full actual source/target finite payloads, with current native policies/reports.
use super::{NativeOwnerRefs, ProductError, ProductPlan, ReportedProduct};
use crate::{
    checked_tl_reduction as tl, product_correspondence::codec,
    spatiotemporal_selection as selection,
};
use lc631_substrate::{resource::ResourceReservationView, semantic_term::Term, Digest32};

pub(super) fn check(
    plan: &ProductPlan<'_>,
    baseline: &ReportedProduct<'_>,
    target: &ReportedProduct<'_>,
    owners: NativeOwnerRefs<'_, '_>,
    source_responses: Term,
    target_responses: Term,
    view: &ResourceReservationView<'_>,
) -> Result<Digest32, ProductError> {
    owners
        .tl_reduction
        .inspect_product_tuple_current(owners.tl_root, owners.tl_candidate, view, |actual| {
            let data = &plan.data;
            let source_time = codec::temporal_term(data, actual.source_temporal)?;
            let target_time = codec::temporal_term(data, actual.target_temporal)?;
            let source_space = codec::spatial_term(data, actual.source_spatial)?;
            let target_space = codec::spatial_term(data, actual.target_spatial)?;
            let time = actual
                .temporal_owner
                .ok_or(ProductError::UnsupportedProjection)?;
            let space = actual
                .spatial_owner
                .ok_or(ProductError::UnsupportedProjection)?;
            time.inspect_product_current(view, |source, target_input, policy, report| {
                if codec::temporal_term(data, source)? != source_time
                    || codec::temporal_term(data, target_input)? != target_time
                    || codec::identity_envelope(data, source, actual.source_spatial)?
                        != baseline.identity_envelope_bytes()
                    || codec::identity_envelope(data, target_input, actual.target_spatial)?
                        != target.identity_envelope_bytes()
                {
                    return Err(ProductError::CorrespondenceMismatch);
                }
                // Reduction.current already checks the exact root's probes and mapping
                // against this actual policy/report, including denominator and binding.
                if policy.digest() != report.policy_digest()
                    || policy.binding_digest() != report.binding_digest()
                    || actual.source_temporal_policy.probes() != policy.probes()
                    || actual.source_temporal_policy.mapping() != policy.mapping()
                {
                    return Err(ProductError::CorrespondenceMismatch);
                }
                Ok(())
            })
            .map_err(producer_error)??;
            space
                .inspect_product_current(view, |source, target, policy, report| {
                    if codec::spatial_term(data, source)? != source_space
                        || codec::spatial_term(data, target)? != target_space
                        || policy.digest() != report.policy_digest()
                        || policy.binding_digest() != report.binding_digest()
                        || actual.source_spatial_policy.probes() != policy.probes()
                        || actual.source_spatial_policy.mapping() != policy.mapping()
                    {
                        return Err(ProductError::CorrespondenceMismatch);
                    }
                    Ok(())
                })
                .map_err(producer_error)??;
            let source = codec::app(
                "tlcx-finite-product-v1",
                vec![source_responses, source_time, source_space],
            );
            let target_term = codec::app(
                "tlcx-finite-product-v1",
                vec![target_responses, target_time, target_space],
            );
            if &source != baseline.term()
                || &target_term != target.term()
                || codec::identity_envelope(data, actual.source_temporal, actual.source_spatial)?
                    != baseline.identity_envelope_bytes()
                || codec::identity_envelope(data, actual.target_temporal, actual.target_spatial)?
                    != target.identity_envelope_bytes()
                || actual.report.denominator() != 4
                || actual.report.profile() != tl::PROFILE
            {
                return Err(ProductError::CorrespondenceMismatch);
            }
            Ok(actual.root_digest)
        })
        .map_err(error)?
}
pub(super) fn error(error: tl::Error) -> ProductError {
    match error {
        tl::Error::Resource(e) => ProductError::Resource(e),
        tl::Error::Producer(e) => producer_error(e),
        tl::Error::Temporal(e) => temporal_error(e),
        tl::Error::Spatial(e) => spatial_error(e),
        _ => ProductError::CorrespondenceMismatch,
    }
}
fn producer_error(error: selection::Error) -> ProductError {
    match error {
        selection::Error::Resource(e) => ProductError::Resource(e),
        selection::Error::Temporal(e) => temporal_error(e),
        selection::Error::Spatial(e) => spatial_error(e),
        selection::Error::FiniteStore(crate::finite_results::FiniteResultError::Resource(e))
        | selection::Error::SpatialStore(
            crate::causal_spatial_observation::ObservationError::Resource(e),
        ) => ProductError::Resource(e),
        selection::Error::FiniteStore(
            crate::finite_results::FiniteResultError::AllocationFailed,
        )
        | selection::Error::SpatialStore(
            crate::causal_spatial_observation::ObservationError::AllocationFailed,
        ) => super::allocation(),
        _ => ProductError::CorrespondenceMismatch,
    }
}
fn temporal_error(error: crate::temporal_preservation::TemporalPreservationError) -> ProductError {
    use crate::{
        finite_results::FiniteResultError as F,
        temporal_preservation::TemporalPreservationError as E,
    };
    match error {
        E::Resource(e) | E::Store(F::Resource(e)) => ProductError::Resource(e),
        E::AllocationFailed | E::Store(F::AllocationFailed) => super::allocation(),
        _ => ProductError::CorrespondenceMismatch,
    }
}
fn spatial_error(error: crate::spatial_preservation::SpatialPreservationError) -> ProductError {
    use crate::{
        causal_spatial_observation::ObservationError as O,
        spatial_preservation::SpatialPreservationError as E,
    };
    match error {
        E::Resource(e) | E::Observation(O::Resource(e)) => ProductError::Resource(e),
        E::AllocationFailed | E::Observation(O::AllocationFailed) => super::allocation(),
        _ => ProductError::CorrespondenceMismatch,
    }
}
