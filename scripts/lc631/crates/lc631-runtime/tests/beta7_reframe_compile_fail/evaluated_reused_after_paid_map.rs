//! After API construction, require E0382 at the second evaluator-owner use.
#![forbid(unsafe_code)]
#![allow(dead_code)]
use lc631_runtime::reorganization_measurement::reframe as f;
use lc631_runtime::resource::ResourceReservationView;
fn reject<'p, 'i, 's, 'v>(
    evaluated: f::EvaluatedFrameProgram<'p, 'i, 's, 'v>,
    view: &'v ResourceReservationView<'_>,
) {
    let _first = f::PaidFrameMap::execute_current(evaluated, view);
    let _second = f::PaidFrameMap::execute_current(evaluated, view);
}
fn main() {}
