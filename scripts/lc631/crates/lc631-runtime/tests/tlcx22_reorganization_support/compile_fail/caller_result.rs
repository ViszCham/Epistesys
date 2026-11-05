//! Expected after exports: missing constructors, not successful proof injection.
use lc631_runtime::reorganization_measurement::{OwnedRebind, PreparedRebindProgram};
use lc631_solvers::typed_query::BoolExpr;
fn inject(composed: &BoolExpr) {
    let _ = PreparedRebindProgram::from_composed(composed);
    let _ = OwnedRebind::from_success(true);
    let _ = OwnedRebind::from_scalar(3, 4);
}
fn main() {}
