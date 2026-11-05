//! Finite solver/backend registry; registrations are descriptors, not observed proof.
use crate::protocol::{
    query_protocol_compatibility, QueryProtocolCompatibility, QueryResultKind, SolverProtocol,
};

pub const MAX_REGISTERED_SOLVERS: usize = 16;
pub const MAX_SOLVER_ARGUMENTS: usize = 128;
pub const MAX_SOLVER_ARGUMENT_BYTES: usize = 1024 * 1024;
pub const MAX_SOLVER_STDIN_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_SOLVER_OUTPUT_BYTES: usize = 2 * 1024 * 1024 + 4;
pub const MAX_SOLVER_TIMEOUT_MS: u64 = 120_000;
pub const MAX_SOLVER_RAM_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_SOLVER_DISK_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_SOLVER_CLEANUP_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BackendId {
    Z3,
    Clingo,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LogicProfile {
    SmtBooleanQfLia,
    AspFiniteGroundNormal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolverRegistryError {
    EmptyRegistry,
    TooManyRegistrations,
    DuplicateRegistration,
    ProtocolProfileMismatch,
    MissingVersion,
    InvalidExecutable,
    InvalidBinaryDigest,
    InvalidLimits,
    ArgumentLimitExceeded,
    UnsupportedProfile,
    UnsupportedMapping,
    AmbiguousRegistration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryCompleteness {
    pub z3_qf_lia_registered: bool,
    pub clingo_ground_asp_registered: bool,
}

impl RegistryCompleteness {
    pub const fn is_complete(self) -> bool {
        self.z3_qf_lia_registered && self.clingo_ground_asp_registered
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncompleteRegistry {
    pub missing_z3_qf_lia: bool,
    pub missing_clingo_ground_asp: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SolverResourceCaps {
    pub max_argv_count: usize,
    pub max_argv_bytes: usize,
    pub max_stdin_bytes: usize,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_timeout_ms: u64,
    pub max_ram_bytes: u64,
    pub max_disk_bytes: u64,
    pub max_cleanup_bytes: u64,
    pub max_child_count: u32,
}

impl SolverResourceCaps {
    fn validate(self) -> Result<(), SolverRegistryError> {
        if self.max_argv_count == 0
            || self.max_argv_count > MAX_SOLVER_ARGUMENTS
            || self.max_argv_bytes == 0
            || self.max_argv_bytes > MAX_SOLVER_ARGUMENT_BYTES
            || self.max_stdin_bytes == 0
            || self.max_stdin_bytes > MAX_SOLVER_STDIN_BYTES
            || self.max_stdout_bytes == 0
            || self.max_stdout_bytes > MAX_SOLVER_OUTPUT_BYTES
            || self.max_stderr_bytes == 0
            || self.max_stderr_bytes > MAX_SOLVER_OUTPUT_BYTES
            || self.max_timeout_ms == 0
            || self.max_timeout_ms > MAX_SOLVER_TIMEOUT_MS
            || self.max_ram_bytes == 0
            || self.max_ram_bytes > MAX_SOLVER_RAM_BYTES
            || self.max_disk_bytes > MAX_SOLVER_DISK_BYTES
            || self.max_cleanup_bytes == 0
            || self.max_cleanup_bytes > MAX_SOLVER_CLEANUP_BYTES
            || self.max_child_count == 0
            || self.max_child_count > 1
        {
            Err(SolverRegistryError::InvalidLimits)
        } else {
            Ok(())
        }
    }
}

/// A pinned declaration consumed by a resource-owning process adapter.
///
/// `binary_digest` and `version` are expected values only. A runner must bind
/// its process observation to these fields before returning a result. This
/// descriptor is not a tool registration receipt or an authority capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolverRegistration {
    id: BackendId,
    profile: LogicProfile,
    protocol: SolverProtocol,
    executable: String,
    version: String,
    binary_digest: [u8; 32],
    fixed_arguments: Vec<String>,
    caps: SolverResourceCaps,
}

impl SolverRegistration {
    #[allow(clippy::too_many_arguments)]
    pub fn checked(
        id: BackendId,
        profile: LogicProfile,
        protocol: SolverProtocol,
        executable: &str,
        version: &str,
        binary_digest: [u8; 32],
        fixed_arguments: Vec<String>,
        caps: SolverResourceCaps,
    ) -> Result<Self, SolverRegistryError> {
        let expected = match id {
            BackendId::Z3 => {
                profile == LogicProfile::SmtBooleanQfLia
                    && matches!(
                        protocol,
                        SolverProtocol::SmtLibStatusV1
                            | SolverProtocol::SmtLibPayloadV1
                            | SolverProtocol::SmtLibPayloadWireV2
                    )
            }
            BackendId::Clingo => {
                profile == LogicProfile::AspFiniteGroundNormal
                    && protocol == SolverProtocol::ClingoStatusV1
            }
        };
        if !expected {
            return Err(SolverRegistryError::ProtocolProfileMismatch);
        }
        if executable.is_empty()
            || executable.len() > 4096
            || executable.contains('\0')
            || !std::path::Path::new(executable).is_absolute()
        {
            return Err(SolverRegistryError::InvalidExecutable);
        }
        if version.is_empty() || version.len() > 128 || version.contains('\0') {
            return Err(SolverRegistryError::MissingVersion);
        }
        if binary_digest.iter().all(|byte| *byte == 0) {
            return Err(SolverRegistryError::InvalidBinaryDigest);
        }
        caps.validate()?;
        if fixed_arguments.len() > caps.max_argv_count {
            return Err(SolverRegistryError::ArgumentLimitExceeded);
        }
        let mut total = executable.len();
        for argument in &fixed_arguments {
            if argument.contains('\0') {
                return Err(SolverRegistryError::InvalidExecutable);
            }
            total = total
                .checked_add(argument.len())
                .ok_or(SolverRegistryError::ArgumentLimitExceeded)?;
        }
        if total > caps.max_argv_bytes {
            return Err(SolverRegistryError::ArgumentLimitExceeded);
        }
        Ok(Self {
            id,
            profile,
            protocol,
            executable: executable.to_owned(),
            version: version.to_owned(),
            binary_digest,
            fixed_arguments,
            caps,
        })
    }

    pub const fn id(&self) -> BackendId {
        self.id
    }
    pub const fn profile(&self) -> LogicProfile {
        self.profile
    }
    pub const fn protocol(&self) -> SolverProtocol {
        self.protocol
    }
    pub fn executable(&self) -> &str {
        &self.executable
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub const fn binary_digest(&self) -> &[u8; 32] {
        &self.binary_digest
    }
    pub fn fixed_arguments(&self) -> &[String] {
        &self.fixed_arguments
    }
    pub const fn caps(&self) -> SolverResourceCaps {
        self.caps
    }
}

/// A bounded registry with explicit resolution; it never chooses a backend by
/// majority vote or treats a declared entry as a live process observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolverRegistry {
    entries: Vec<SolverRegistration>,
}

impl SolverRegistry {
    pub fn checked(entries: Vec<SolverRegistration>) -> Result<Self, SolverRegistryError> {
        if entries.is_empty() {
            return Err(SolverRegistryError::EmptyRegistry);
        }
        if entries.len() > MAX_REGISTERED_SOLVERS {
            return Err(SolverRegistryError::TooManyRegistrations);
        }
        for (index, left) in entries.iter().enumerate() {
            if entries[index + 1..].iter().any(|right| {
                left.id == right.id
                    && left.profile == right.profile
                    && left.protocol == right.protocol
            }) {
                return Err(SolverRegistryError::DuplicateRegistration);
            }
        }
        let mut entries = entries;
        entries.sort_by_key(|entry| {
            let protocol = match entry.protocol {
                SolverProtocol::SmtLibStatusV1 => 1,
                SolverProtocol::ClingoStatusV1 => 2,
                SolverProtocol::SmtLibPayloadV1 => 3,
                SolverProtocol::SmtLibPayloadWireV2 => 4,
            };
            (entry.id, entry.profile, protocol)
        });
        Ok(Self { entries })
    }

    pub fn resolve(
        &self,
        id: BackendId,
        profile: LogicProfile,
    ) -> Result<&SolverRegistration, SolverRegistryError> {
        let mut matching = self
            .entries
            .iter()
            .filter(|entry| entry.id == id && entry.profile == profile);
        let first = matching
            .next()
            .ok_or(SolverRegistryError::UnsupportedProfile)?;
        if matching.next().is_some() {
            return Err(SolverRegistryError::AmbiguousRegistration);
        }
        Ok(first)
    }

    /// Resolve a backend profile by its declared protocol compatibility with
    /// this query kind. Multiple compatible descriptors remain ambiguous;
    /// the registry never selects between them by insertion or sort order.
    ///
    /// For a single registered descriptor with an unsupported query shape, it
    /// returns that descriptor so the caller can preserve the historical
    /// invocation binding before recording `UnsupportedMapping`.
    pub fn resolve_for_query(
        &self,
        id: BackendId,
        profile: LogicProfile,
        result_kind: QueryResultKind,
    ) -> Result<&SolverRegistration, SolverRegistryError> {
        let mut matching = self
            .entries
            .iter()
            .filter(|entry| entry.id == id && entry.profile == profile);
        let first = matching
            .next()
            .ok_or(SolverRegistryError::UnsupportedProfile)?;
        let mut compatible = self.entries.iter().filter(|entry| {
            entry.id == id
                && entry.profile == profile
                && query_protocol_compatibility(entry.protocol, result_kind)
                    == QueryProtocolCompatibility::Supported
        });
        if let Some(selected) = compatible.next() {
            if compatible.next().is_some() {
                return Err(SolverRegistryError::AmbiguousRegistration);
            }
            return Ok(selected);
        }
        if matching.next().is_none() {
            return Ok(first);
        }
        Err(SolverRegistryError::UnsupportedMapping)
    }

    pub fn entries(&self) -> &[SolverRegistration] {
        &self.entries
    }

    /// Completeness is explicit against the two mandatory finite profiles;
    /// accepting one backend never marks the registry complete.
    pub fn completeness(&self) -> RegistryCompleteness {
        RegistryCompleteness {
            z3_qf_lia_registered: self.entries.iter().any(|entry| {
                entry.id == BackendId::Z3
                    && entry.profile == LogicProfile::SmtBooleanQfLia
                    && entry.protocol == SolverProtocol::SmtLibStatusV1
            }),
            clingo_ground_asp_registered: self.entries.iter().any(|entry| {
                entry.id == BackendId::Clingo
                    && entry.profile == LogicProfile::AspFiniteGroundNormal
            }),
        }
    }

    pub fn require_complete_profiles(&self) -> Result<(), IncompleteRegistry> {
        let status = self.completeness();
        if status.is_complete() {
            Ok(())
        } else {
            Err(IncompleteRegistry {
                missing_z3_qf_lia: !status.z3_qf_lia_registered,
                missing_clingo_ground_asp: !status.clingo_ground_asp_registered,
            })
        }
    }
}
