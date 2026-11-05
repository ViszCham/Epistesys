//! Exact finite Reframe v2. Given interpretations stay Unassessed/SharedDependency.
//! No v1 root inference adapter, authority, global quota or host closure.
#![forbid(unsafe_code)]
mod current;
mod evaluate;
mod limits;
mod lower;
mod measure;
mod model;
mod paid;
mod parse;
mod pipeline;
mod root;
mod selection;
mod syntax;
mod transition;
pub use current::{CurrentReframe, ReframeRow};
pub use evaluate::EvaluatedFrameProgram;
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceKind, ResourceRequest, ResourceReservationView,
};
pub use limits::ExactLimits;
pub use lower::LoweredFrameProgram;
pub use measure::{OwnedReframe, PairInput};
pub use model::*;
pub use paid::PaidFrameMap;
pub use parse::ParsedFrameProgram;
pub use root::{FrameFamily, ReframePlan};
pub use selection::{compare_current, select_current, CurrentDecision};
pub use transition::CheckedFrameMap;
fn claim<'v>(r: &'v ResourceReservationView<'_>, bytes: u64) -> Result<ResourceClaim<'v>, Error> {
    r.check()?;
    Ok(r.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(bytes)?,
    )])?)
}
fn vec_cap<T>(n: usize) -> Result<Vec<T>, Error> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| Error::AllocationFailed)?;
    Ok(v)
}
