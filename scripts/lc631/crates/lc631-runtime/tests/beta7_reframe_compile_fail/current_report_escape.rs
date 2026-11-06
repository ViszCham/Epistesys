//! After API construction, require E0515 for a borrowed scoped report escaping.
#![forbid(unsafe_code)]
#![allow(dead_code)]
use lc631_analysis::creativity_measurement::ComponentReport;
use lc631_runtime::{reorganization_measurement::reframe as f, resource::ResourceReservationView};
fn reject<'o, 'p, 'i, 's, 'v>(
    owner: &f::OwnedReframe<'o, 'p, 'i, 's, 'v>,
    view: &ResourceReservationView<'_>,
) -> Result<&'static ComponentReport, f::Error> {
    owner.with_current(view, |current| current.report())
}
fn main() {}
