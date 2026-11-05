//! Expected after exports: Evaluation borrows Requests, not just model ASTs.
use lc631_runtime::{reorganization_measurement as r, resource::ResourceReservationView};
fn drop_requests<'g, 'p, 'i, 'v>(
    program: &'g r::PreparedRebindProgram<'p, 'i, 'v>,
    view: &'v ResourceReservationView<'_>,
) -> r::OwnedRebind<'g, 'g, 'p, 'i, 'v> {
    let owner;
    {
        let queries = r::RebindQueries::capture(program, view).unwrap();
        owner = r::OwnedRebind::evaluate(&queries, view).unwrap();
    }
    owner
}
fn main() {}
