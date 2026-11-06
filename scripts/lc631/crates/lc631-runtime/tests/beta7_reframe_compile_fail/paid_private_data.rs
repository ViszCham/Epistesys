//! After API construction, require E0616 at private capability data access.
#![forbid(unsafe_code)]
#![allow(dead_code)]
use lc631_runtime::reorganization_measurement::reframe as f;
fn reject<'p, 'i, 's, 'v>(paid: &f::PaidFrameMap<'p, 'i, 's, 'v>) {
    let _ = &paid.data;
}
fn main() {}
