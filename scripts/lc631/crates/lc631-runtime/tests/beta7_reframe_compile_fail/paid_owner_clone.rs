//! After API construction, require E0277 for PaidFrameMap: Clone.
#![forbid(unsafe_code)]
#![allow(dead_code)]
use lc631_runtime::reorganization_measurement::reframe as f;
fn require_clone<T: Clone>() {}
fn reject<'p, 'i, 's, 'v>() {
    require_clone::<f::PaidFrameMap<'p, 'i, 's, 'v>>();
}
fn main() {}
