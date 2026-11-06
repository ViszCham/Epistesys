//! 日本語: 固定1:3時間/空間有限MODEL tupleの実owner付きTL縮約。
//! English: Fixed 1:3 temporal/spatial Given finite-MODEL tuple reduction.
//! Exact arithmetic is not semantic validation, eligibility, map-back or Authority.
//! Cross-domain association and source interpretation remain Given/unverified.
#![forbid(unsafe_code)]
/// Actual bounded model constructors for explicit application-boundary adapters.
/// Reexports only; no authority, observation or checked-report constructor.
pub mod input;
mod plan;
mod reduce;
use crate::joint_selection::MissingAxis;
use lc631_core::measurement_numeric::RationalInterval;
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
pub use plan::{RootPlan, TupleCandidate};
pub use reduce::selection::{
    select_current_reduced, CurrentReducedSelection, ReducedSelectionError, ReductionRow,
};
pub use reduce::{reduce_current, OwnedReduction, Owners};

pub const PROFILE: &str = "finite-formal-v1.4/TL-TOTAL/given-temporal-spatial-tuple-1-3.v1";
pub const DENOMINATOR: u64 = 4;
const SCRATCH: u64 = 8 * 1024 * 1024;
const RETAINED: u64 = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Axis {
    Temporal,
    Spatial,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoScalar {
    MissingOwner,
    Unavailable(MissingAxis),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gate {
    Pass,
    Fail,
    NeedsEvidence,
    NoScalar,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Resource(ResourceError),
    BindingMismatch,
    InvalidRoot,
    Producer(crate::spatiotemporal_selection::Error),
    Temporal(crate::temporal_preservation::TemporalPreservationError),
    Spatial(crate::spatial_preservation::SpatialPreservationError),
    Arithmetic(lc631_analysis::creativity_measurement::MeasurementError),
}
/// Private computed export. No constructor takes a scalar, gate, weight or mask.
/// Axis applicability is permanently [Temporal,Spatial], weights [1,3], W=4.
/// Missing/unsupported/incomparable remains NoScalar, never candidate reweighting.
pub struct Report {
    total: Option<RationalInterval>,
    gaps: [Option<NoScalar>; 2],
    gates: [Gate; 2],
}
impl Report {
    pub fn total(&self) -> Option<&RationalInterval> {
        self.total.as_ref()
    }
    pub const fn gaps(&self) -> [Option<NoScalar>; 2] {
        self.gaps
    }
    /// Mandatory threshold tau=0 per axis, separate from aggregate arithmetic.
    pub const fn hard_gates(&self) -> [Gate; 2] {
        self.gates
    }
    pub const fn denominator(&self) -> u64 {
        DENOMINATOR
    }
    pub const fn profile(&self) -> &'static str {
        PROFILE
    }
    pub const fn source_semantics_verified(&self) -> bool {
        false
    }
    pub const fn authority_created(&self) -> bool {
        false
    }
}
fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    bytes: u64,
) -> Result<ResourceClaim<'v>, Error> {
    view.check().map_err(Error::Resource)?;
    view.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(bytes).map_err(Error::Resource)?,
    )])
    .map_err(Error::Resource)
}
