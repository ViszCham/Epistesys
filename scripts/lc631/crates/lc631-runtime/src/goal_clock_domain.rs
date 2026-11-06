//! Registered local clock actors, bound to a live resource lease and a concrete role.
//! This local profile cannot certify host time, source semantics or a Test grant.
#![forbid(unsafe_code)]
mod comparison;
mod native;
mod registration;
mod session;

pub use comparison::{
    measure_clock_bound, ClockBasis, CurrentPorts, HostAuthenticity, OwnedRegisteredPair,
    RegisteredArtifactRoot, RegisteredPair, RegisteredRootInput, RegisteredView,
};
pub use native::{check_observation_for_registered_profile, ProbeInputs, RegisteredNativeFailure, RegisteredObservation};
pub use registration::{
    register, ClockEpochSource, ClockWork, EndpointBinding, EndpointCandidate, Limits, Profile,
    ProfileInput, Protocol, Purpose, RegisteredEndpoint, RegistryAuthority, RegistryView, Role,
    TrustRequirement,
};
pub(crate) use registration::EndpointReader;
pub use session::{
    CaptureAttempt, RegisteredCapture, RoleSession, SnapshotGrantSource, SnapshotGrantView,
};

use crate::{contextual_repair::measurement::ProbeError, goal_suite_measurement as goal};
use lc631_analysis::declared_build_capture::SnapshotCaptureError;
use lc631_substrate::resource::ResourceError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderUnavailable;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockGap {
    RegistrationMissing,
    UntrustedIssuer,
    UnknownDomain,
    IncomparableDomains,
    UnknownProtocol,
    LegacyClockProvenanceUnassessed,
    HostAnchorUnavailable,
    CurrentRegistryUnavailable,
    ProviderUnavailable,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClockFault {
    NeedsEvidence(ClockGap),
    RegistrationExpired,
    RegistrationRevoked,
    BindingMismatch,
    ClockRollback,
    ObservedExpiry,
    BudgetExhausted,
    Resource(ResourceError),
}

/// Goal-only failures that have no corresponding clock, probe or capture cause.
/// Keep them distinct when using the existing four-output artifact evaluator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtifactFailure {
    MissingBaselineOrigin, MissingVerifiedOrigin, OracleMismatch, InvalidLimits,
    GoalOrProbeMismatch, BaselineMismatch, RoleOrOriginMismatch, UndeclaredOverlap,
    HardInvariantFailed, BuildDrift, ReadBudget, AllocationFailed, Unsupported, NoScalar,
    Gate(crate::contextual_repair::snapshot_gate::SnapshotGateError),
    Native(lc631_solvers::runner::ProcessFailure),
    Measurement(lc631_analysis::creativity_measurement::MeasurementError),
    Numeric(lc631_substrate::measurement_numeric::NumericError),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Clock(ClockFault),
    Probe(ProbeError),
    Snapshot(SnapshotCaptureError),
    InputLimit,
    Artifact(ArtifactFailure),
}
impl From<ResourceError> for Error {
    fn from(value: ResourceError) -> Self { Self::Clock(ClockFault::Resource(value)) }
}
impl From<ClockFault> for Error {
    fn from(value: ClockFault) -> Self { Self::Clock(value) }
}
impl From<ProbeError> for Error {
    fn from(value: ProbeError) -> Self {
        match value {
            ProbeError::ClockDomain(error) => Self::Clock(error),
            ProbeError::Resource(error) => Self::Clock(ClockFault::Resource(error)),
            other => Self::Probe(other),
        }
    }
}
impl From<SnapshotCaptureError> for Error {
    fn from(value: SnapshotCaptureError) -> Self { Self::Snapshot(value) }
}
impl From<goal::Error> for Error {
    fn from(value: goal::Error) -> Self {
        use goal::Error as G;
        let cause = match value {
            G::Resource(e) => return e.into(),
            G::Probe(e) => return e.into(),
            G::BuildCapture(e) => return Self::Snapshot(e),
            G::InputLimit => return Self::InputLimit,
            G::ClockRollback => return Self::Clock(ClockFault::ClockRollback),
            G::Expired => return Self::Clock(ClockFault::ObservedExpiry),
            G::Unauthorized => return Self::Probe(ProbeError::Unauthorized),
            G::BindingMismatch => return Self::Clock(ClockFault::BindingMismatch),
            G::MissingBaselineOrigin => ArtifactFailure::MissingBaselineOrigin,
            G::MissingVerifiedOrigin => ArtifactFailure::MissingVerifiedOrigin,
            G::OracleMismatch => ArtifactFailure::OracleMismatch,
            G::InvalidLimits => ArtifactFailure::InvalidLimits,
            G::GoalOrProbeMismatch => ArtifactFailure::GoalOrProbeMismatch,
            G::BaselineMismatch => ArtifactFailure::BaselineMismatch,
            G::RoleOrOriginMismatch => ArtifactFailure::RoleOrOriginMismatch,
            G::UndeclaredOverlap => ArtifactFailure::UndeclaredOverlap,
            G::HardInvariantFailed => ArtifactFailure::HardInvariantFailed,
            G::BuildDrift => ArtifactFailure::BuildDrift,
            G::SnapshotGate(e) => ArtifactFailure::Gate(e),
            G::NativeFingerprint(e) => ArtifactFailure::Native(e),
            G::Measurement(e) => ArtifactFailure::Measurement(e),
            G::Numeric(e) => ArtifactFailure::Numeric(e),
            G::Unsupported => ArtifactFailure::Unsupported,
            G::NoScalar => ArtifactFailure::NoScalar,
            G::ReadBudget => ArtifactFailure::ReadBudget,
            G::AllocationFailed => ArtifactFailure::AllocationFailed,
        };
        Self::Artifact(cause)
    }
}
