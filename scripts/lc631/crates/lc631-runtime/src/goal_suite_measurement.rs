//! Bounded actual-artifact and finite-native goal suites; not source coding closure.
//! Conditional artifact quality, requested-source goals and native feasibility are
//! distinct profiles. Given/Unassessed/SharedDependency never creates authority.
#![forbid(unsafe_code)]
mod artifact;
mod numeric;
mod objective;
use crate::contextual_repair::{measurement::ProbeError, snapshot_gate::SnapshotGateError};
pub use artifact::{
    measure_artifact_pair, ArtifactCurrent, ArtifactPairRoot, ArtifactRootInput, ArtifactView,
    CodingPair, OwnedArtifactPair, SourceCodingRoot, SourceCodingRootInput,
};
use lc631_analysis::{
    creativity_measurement::MeasurementError, declared_build_capture::SnapshotCaptureError,
};
use lc631_core::context::{ContextRevision, ContextScope};
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    Digest32,
};
pub use objective::{
    measure_native_pair, NativePair, ObjectiveProfile, ObjectiveRoot, ObjectiveRootInput,
    ObjectiveView, OwnedObjectivePair,
};
use sha2::{Digest, Sha256};
use std::cell::Cell;

pub struct Limits {
    pub max_queries: u8,
    pub max_pairs: u8,
    pub max_current_checks: u8,
    pub max_added_ram_bytes: u64,
    pub max_summary_bytes: u64,
}
pub struct ArtifactPins {
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub definition: Digest32,
    pub oracle: Digest32,
    pub hard: Digest32,
    pub evidence: Digest32,
}
pub struct SourcePins {
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub definition: Digest32,
    pub oracle: Digest32,
    pub hard: Digest32,
    pub evidence: Digest32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceCorrespondence {
    UnverifiedNeedsEvidence,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceEligibility {
    Pending,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredOverlap {
    Absent,
    SameArtifactPrimitive,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GoalOptimality {
    BestAmongEvaluated,
    BudgetLimited,
    Incomparable,
}
#[derive(Debug)]
pub enum Error {
    MissingBaselineOrigin,
    MissingVerifiedOrigin,
    OracleMismatch,
    InvalidLimits,
    GoalOrProbeMismatch,
    BindingMismatch,
    BaselineMismatch,
    RoleOrOriginMismatch,
    UndeclaredOverlap,
    HardInvariantFailed,
    BuildDrift,
    Unauthorized,
    Expired,
    ClockRollback,
    Resource(ResourceError),
    BuildCapture(SnapshotCaptureError),
    SnapshotGate(SnapshotGateError),
    Probe(ProbeError),
    NativeFingerprint(lc631_solvers::runner::ProcessFailure),
    Measurement(MeasurementError),
    Numeric(lc631_substrate::measurement_numeric::NumericError),
    Unsupported,
    NoScalar,
    ReadBudget,
    AllocationFailed,
    InputLimit,
}
impl From<ResourceError> for Error {
    fn from(e: ResourceError) -> Self {
        Self::Resource(e)
    }
}
impl From<MeasurementError> for Error {
    fn from(e: MeasurementError) -> Self {
        Self::Measurement(e)
    }
}
impl From<lc631_substrate::measurement_numeric::NumericError> for Error {
    fn from(error: lc631_substrate::measurement_numeric::NumericError) -> Self {
        Self::Numeric(error)
    }
}

const ROOT_RAM: u64 = 16 * 1024;
const PAIR_RAM: u64 = 32 * 1024;
const VIEW_RAM: u64 = 16 * 1024;
const SOURCE_CAP: usize = 256 * 1024;
struct Budget {
    limits: Limits,
    pairs: Cell<u8>,
    reads: Cell<u8>,
}
impl Budget {
    fn checked(limits: Limits) -> Result<Self, Error> {
        if limits.max_queries != 4
            || limits.max_pairs != 1
            || !(1..=8).contains(&limits.max_current_checks)
            || limits.max_added_ram_bytes == 0
            || limits.max_added_ram_bytes > 64 * 1024
            || limits.max_summary_bytes == 0
            || limits.max_summary_bytes > 64 * 1024
        {
            return Err(Error::InvalidLimits);
        }
        Ok(Self {
            limits,
            pairs: Cell::new(0),
            reads: Cell::new(0),
        })
    }
    fn root_funding(&self) -> Result<(), Error> {
        if self.limits.max_added_ram_bytes < ROOT_RAM {
            return Err(Error::Resource(ResourceError::Exhausted(ResourceKind::Ram)));
        }
        Ok(())
    }
    fn pair_funding(&self) -> Result<(), Error> {
        if self.limits.max_added_ram_bytes < ROOT_RAM + PAIR_RAM + VIEW_RAM {
            return Err(Error::Resource(ResourceError::Exhausted(ResourceKind::Ram)));
        }
        // This slice reserves the fixed conservative64KiB report-view allowance;
        // a smaller caller output profile cannot silently inherit that allowance.
        if self.limits.max_summary_bytes < 64 * 1024 {
            return Err(Error::Resource(ResourceError::Exhausted(
                ResourceKind::Output,
            )));
        }
        Ok(())
    }
    fn pair(&self) -> Result<(), Error> {
        let n = self.pairs.get();
        if n >= self.limits.max_pairs {
            return Err(Error::ReadBudget);
        }
        self.pairs.set(n + 1);
        Ok(())
    }
    fn read(&self) -> Result<(), Error> {
        let n = self.reads.get();
        if n >= self.limits.max_current_checks {
            return Err(Error::ReadBudget);
        }
        self.reads.set(n + 1);
        Ok(())
    }
}
fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    amount: u64,
) -> Result<ResourceClaim<'v>, Error> {
    Ok(view.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(amount)?,
    )])?)
}
fn check_claim(claim: &ResourceClaim<'_>, view: &ResourceReservationView<'_>) -> Result<(), Error> {
    view.check()?;
    claim.check()?;
    if !claim.same_reservation(view) {
        return Err(Error::Resource(ResourceError::ForeignOwner));
    }
    Ok(())
}
fn hash(parts: &[&[u8]]) -> Digest32 {
    let mut h = Sha256::new();
    for part in parts {
        h.update((part.len() as u64).to_le_bytes());
        h.update(part);
    }
    Digest32::from_bytes(h.finalize().into())
}
fn nonzero(d: Digest32) -> bool {
    d != Digest32::from_bytes([0; 32])
}
fn same_owner<T, U>(a: &T, b: &U) -> bool {
    // Sized live occurrences only: never dereferenced, hashed, persisted or ranked.
    std::ptr::eq((a as *const T).cast::<()>(), (b as *const U).cast::<()>())
}
fn overlap(value: DeclaredOverlap) -> Result<(), Error> {
    match value {
        DeclaredOverlap::Absent => Err(Error::UndeclaredOverlap),
        DeclaredOverlap::SameArtifactPrimitive => Ok(()),
    }
}
fn probe_error(error: ProbeError) -> Error {
    match error {
        ProbeError::Unauthorized => Error::Unauthorized,
        ProbeError::ClockRollback => Error::ClockRollback,
        ProbeError::Resource(e) => Error::Resource(e),
        other => Error::Probe(other),
    }
}
