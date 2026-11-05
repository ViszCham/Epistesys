//! Expected after exports: HRTB current report cannot escape as a static receipt.
use lc631_analysis::creativity_measurement::ComponentReport;
use lc631_runtime::{reorganization_measurement as r, resource::ResourceReservationView};
fn escape<'q, 'g, 'p, 'i, 'v>(
    owner: &r::OwnedRebind<'q, 'g, 'p, 'i, 'v>,
    view: &ResourceReservationView<'_>,
) -> Result<&'static ComponentReport, r::Error> {
    owner.with_current(view, |current| current.report())
}
fn main() {}
