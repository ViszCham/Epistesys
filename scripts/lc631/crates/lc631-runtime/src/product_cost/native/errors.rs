//! Preserve actual resource failures through closed native producer errors.
use super::*;
pub(super) fn product(e: crate::product_correspondence::ProductError) -> CostError {
    match e {
        crate::product_correspondence::ProductError::Resource(e) => CostError::Resource(e),
        _ => CostError::CorrespondenceMismatch,
    }
}
pub(super) fn risk(e: crate::adversarial_measurement::Error) -> CostError {
    match e {
        crate::adversarial_measurement::Error::Resource(e) => CostError::Resource(e),
        crate::adversarial_measurement::Error::AllocationFailed => {
            CostError::Resource(ResourceError::AllocationFailed)
        }
        _ => CostError::CorrespondenceMismatch,
    }
}
pub(super) fn description(e: crate::description_measurement::Error) -> CostError {
    match e {
        crate::description_measurement::Error::Resource(e) => CostError::Resource(e),
        crate::description_measurement::Error::Reconstruction(
            crate::reconstruction_measurement::Error::Resource(e),
        ) => CostError::Resource(e),
        _ => CostError::CorrespondenceMismatch,
    }
}
pub(super) fn tl(e: crate::checked_tl_reduction::Error) -> CostError {
    match e {
        crate::checked_tl_reduction::Error::Resource(e) => CostError::Resource(e),
        crate::checked_tl_reduction::Error::Producer(e) => selection(e),
        crate::checked_tl_reduction::Error::Temporal(e) => temporal(e),
        crate::checked_tl_reduction::Error::Spatial(e) => spatial(e),
        _ => CostError::CorrespondenceMismatch,
    }
}
pub(super) fn selection(e: crate::spatiotemporal_selection::Error) -> CostError {
    use crate::spatiotemporal_selection::Error as E;
    match e {
        E::Resource(e) => CostError::Resource(e),
        E::Temporal(e) => temporal(e),
        E::Spatial(e) => spatial(e),
        E::FiniteStore(crate::finite_results::FiniteResultError::Resource(e))
        | E::SpatialStore(crate::causal_spatial_observation::ObservationError::Resource(e)) => {
            CostError::Resource(e)
        }
        E::FiniteStore(crate::finite_results::FiniteResultError::AllocationFailed)
        | E::SpatialStore(crate::causal_spatial_observation::ObservationError::AllocationFailed) => {
            CostError::Resource(ResourceError::AllocationFailed)
        }
        _ => CostError::CorrespondenceMismatch,
    }
}
fn temporal(e: crate::temporal_preservation::TemporalPreservationError) -> CostError {
    use crate::{
        finite_results::FiniteResultError as F,
        temporal_preservation::TemporalPreservationError as E,
    };
    match e {
        E::Resource(e) | E::Store(F::Resource(e)) => CostError::Resource(e),
        E::AllocationFailed | E::Store(F::AllocationFailed) => {
            CostError::Resource(ResourceError::AllocationFailed)
        }
        _ => CostError::CorrespondenceMismatch,
    }
}
fn spatial(e: crate::spatial_preservation::SpatialPreservationError) -> CostError {
    use crate::{
        causal_spatial_observation::ObservationError as O,
        spatial_preservation::SpatialPreservationError as E,
    };
    match e {
        E::Resource(e) | E::Observation(O::Resource(e)) => CostError::Resource(e),
        E::AllocationFailed | E::Observation(O::AllocationFailed) => {
            CostError::Resource(ResourceError::AllocationFailed)
        }
        _ => CostError::CorrespondenceMismatch,
    }
}
