//! Positive control: an owned historical report may leave the callback.
//! This clone is metadata and cannot reconstruct a checked/paid/current owner.
#![forbid(unsafe_code)]
#![allow(dead_code)]
use lc631_analysis::creativity_measurement::ComponentReport;
use lc631_runtime::{reorganization_measurement::reframe as f, resource::ResourceReservationView};
fn control<'o, 'p, 'i, 's, 'v>(
    owner: &f::OwnedReframe<'o, 'p, 'i, 's, 'v>,
    view: &ResourceReservationView<'_>,
) -> Result<ComponentReport, f::Error> {
    owner.with_current(view, |current| current.report().clone())
}
fn main() {}
