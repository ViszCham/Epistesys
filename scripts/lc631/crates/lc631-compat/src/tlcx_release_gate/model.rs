use super::CandidateIdentity;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ReleaseDigest([u8; 32]);
impl ReleaseDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ArtifactKind {
    Source,
    Binary,
    Plugin,
    Schema,
    Documentation,
}
impl ArtifactKind {
    pub const ALL: [Self; 5] = [
        Self::Source,
        Self::Binary,
        Self::Plugin,
        Self::Schema,
        Self::Documentation,
    ];
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactPin {
    pub kind: ArtifactKind,
    pub digest: ReleaseDigest,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum GateId {
    G00,
    G01,
    G02,
    G03,
    G04,
    G05,
    G06,
    G07,
    G08,
    G09,
    G10,
    G11,
    G12,
    G13,
    G14,
    G15,
}
impl GateId {
    pub const ALL: [Self; 16] = [
        Self::G00,
        Self::G01,
        Self::G02,
        Self::G03,
        Self::G04,
        Self::G05,
        Self::G06,
        Self::G07,
        Self::G08,
        Self::G09,
        Self::G10,
        Self::G11,
        Self::G12,
        Self::G13,
        Self::G14,
        Self::G15,
    ];
    pub const fn requires_host(self) -> bool {
        matches!(self, Self::G13 | Self::G15)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Platform {
    WindowsX64,
    UbuntuX64,
    Portable,
    Host,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Backend {
    Core,
    Smt,
    Asp,
    ConfiguredNlp,
    Gpu,
    Package,
    Host,
    Audit,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct GateKey {
    pub gate: GateId,
    pub platform: Platform,
    pub backend: Backend,
    pub profile: ReleaseDigest,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportedOutcome {
    Passed,
    Failed,
    Skipped,
    Unsupported,
    Unavailable,
}
/// A report, not an authenticated observation. Presence of a receipt digest is
/// required for inspection but cannot establish trusted execution provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportedGateRecord {
    pub key: GateKey,
    pub candidate: ReleaseDigest,
    pub source_snapshot: ReleaseDigest,
    pub run: ReleaseDigest,
    pub observed_at: u64,
    pub expires_at: u64,
    pub outcome: ReportedOutcome,
    pub receipt: Option<ReleaseDigest>,
}

pub struct ReleasePlan<'a> {
    pub(super) identity: CandidateIdentity,
    pub(super) required: &'a [GateKey],
    pub(super) run: ReleaseDigest,
    pub(super) max_age: u64,
}
impl<'a> ReleasePlan<'a> {
    pub fn checked(
        identity: CandidateIdentity,
        required: &'a [GateKey],
        run: ReleaseDigest,
        max_age: u64,
    ) -> Result<Self, GateError> {
        if required.len() > 128 || max_age == 0 {
            return Err(GateError::InvalidPlan);
        }
        for (index, key) in required.iter().enumerate() {
            if required[..index].contains(key) {
                return Err(GateError::DuplicateRequirement);
            }
        }
        // Required hard gates cannot be silently dropped by an empty profile matrix.
        for gate in GateId::ALL {
            if !required.iter().any(|key| key.gate == gate) {
                return Err(GateError::MissingGate(gate));
            }
        }
        Ok(Self {
            identity,
            required,
            run,
            max_age,
        })
    }
    pub const fn identity(&self) -> &CandidateIdentity {
        &self.identity
    }
    pub fn requirements(&self) -> &[GateKey] {
        self.required
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactStatus {
    BytesMatch,
    Missing,
    DigestMismatch,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactCheck {
    pub kind: ArtifactKind,
    pub status: ArtifactStatus,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileStatus {
    Missing,
    HostPending,
    Failed,
    Skipped,
    Unsupported,
    Unavailable,
    Stale,
    FutureObservation,
    InvalidTimeWindow,
    SnapshotMismatch,
    CandidateMismatch,
    RunMismatch,
    MissingReceipt,
    ReportedPassNeedsVerification,
    /// Actual local observation for this closed obligation, not the whole gate.
    CurrentVerified(super::PackageLocalProfile),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileCheck {
    pub key: GateKey,
    pub status: ProfileStatus,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateDisposition {
    Blocked,
    CandidateNotFullyReleased,
}
/// There is intentionally no Ready/Released variant or authority conversion.
pub struct CandidateInspection {
    pub candidate: ReleaseDigest,
    pub artifacts: [ArtifactCheck; 5],
    pub profiles: Vec<ProfileCheck>,
    pub disposition: CandidateDisposition,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateError {
    InvalidPlan,
    DuplicateRequirement,
    MissingGate(GateId),
    DuplicateArtifact,
    DuplicateRecord,
    UnregisteredProfile,
    RecordLimit,
    AllocationFailed,
    InvalidReadLimit,
    ReadLimit,
    ReadCallLimit,
    ReaderFailed(std::io::ErrorKind),
    InterruptedLimit,
}
