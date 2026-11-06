//! Positive type control: the real consuming constructor and read-only getters.
#![forbid(unsafe_code)]
#![allow(dead_code)]
use lc631_runtime::reorganization_measurement::reframe as f;
use lc631_runtime::resource::ResourceReservationView;
fn control<'p, 'i, 's, 'v>(
    evaluated: f::EvaluatedFrameProgram<'p, 'i, 's, 'v>,
    view: &'v ResourceReservationView<'_>,
) -> Result<u64, f::Error> {
    let paid = f::PaidFrameMap::execute_current(evaluated, view)?;
    std::hint::black_box(paid.profile());
    std::hint::black_box(paid.trace_for(f::ProbeId(0)));
    Ok(paid.mapping_work_spent())
}
fn main() {}
