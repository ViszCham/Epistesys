//! TLCX-42 candidate-input inspection, not release authorization or execution proof.
#![forbid(unsafe_code)]
mod artifacts;
mod assessment;
mod local_package;
mod model;
pub use artifacts::{CandidateIdentity, ObservedArtifact};
pub use assessment::assess;
pub use local_package::{
    assess_current_package, CheckedPackageRequirement, ExpectedDistributionPlan, LocalPackageError,
    PackageLocalProfile,
};
pub use model::*;
