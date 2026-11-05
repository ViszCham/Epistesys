//! Finite paid tree programs. Description length is not truth or authority.
#![forbid(unsafe_code)]
mod codec;
pub(super) mod execution;
mod inverse;
mod machine;
mod native;
mod profile;
mod programs;
mod report;
pub(super) mod term;
mod types;
pub use execution::{execute_reported, OwnedExecutableBundleCost};
pub use inverse::{
    execute_inverse, ExecutionControls, FixedBootstrap, IsolatedReconstruction, TargetBundleView,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceKind, ResourceRequest, ResourceReservationView,
};
pub(super) use lc631_substrate::semantic_term::Term;
pub use native::{
    capture_baseline_current, measure_candidate_current, CurrentCostObservation,
    MeaningCorrespondence, NativeIdentityAdmissionCost, NativeProductInput,
    OwnedSemanticProductBaseline, OwnedSemanticProductCost,
};
pub use profile::{declare_cost_profile, DeclaredCostProfile};
pub use programs::stage_b_copy_pack_package_v1;
pub use report::*;
use sha2::{Digest, Sha256};
pub use types::*;
pub(super) fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    bytes: u64,
) -> Result<ResourceClaim<'v>, CostError> {
    view.check()?;
    Ok(view.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(bytes)?,
    )])?)
}
