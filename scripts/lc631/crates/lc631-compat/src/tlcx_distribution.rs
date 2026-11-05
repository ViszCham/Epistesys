//! 配布treeの限定read-only照合。起動・署名・releaseの証拠ではない。
//! Bounded read-only package-tree comparison, not execution, authenticity or release evidence.
#![forbid(unsafe_code)]
use sha2::{Digest, Sha256};
mod filesystem;
pub use filesystem::{audit_tree, hash_reader};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Source,
    LegacySchema,
    BoundedSchema,
    PluginManifest,
    Launcher,
    Hook,
    Skill,
    Executable,
    Support,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageKind {
    Source,
    PluginWindowsX64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditError {
    EmptyInventory,
    InvalidPath,
    DuplicatePath,
    WrongRole,
    MissingRole(Role),
    ForbiddenMember,
    InvalidLimits,
    MemberLimit,
    TotalLimit,
    VisitLimit,
    DepthLimit,
    ReadLimit,
    ReadFailed,
    RootUnavailable,
    LinkOrSpecialFile,
    AllocationFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    members: usize,
    member_bytes: u64,
    total_bytes: u64,
    visits: usize,
    depth: usize,
    reads: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            members: 4096,
            member_bytes: 32 * 1024 * 1024,
            total_bytes: 256 * 1024 * 1024,
            visits: 16_384,
            depth: 32,
            reads: 8192,
        }
    }
}
impl Limits {
    pub fn new(
        members: usize,
        member_bytes: u64,
        total_bytes: u64,
        visits: usize,
        depth: usize,
        reads: usize,
    ) -> Result<Self, AuditError> {
        if members == 0
            || members > 4096
            || member_bytes == 0
            || member_bytes > 32 * 1024 * 1024
            || total_bytes == 0
            || total_bytes > 256 * 1024 * 1024
            || visits == 0
            || visits > 16_384
            || depth == 0
            || depth > 32
            || reads == 0
            || reads > 8192
        {
            return Err(AuditError::InvalidLimits);
        }
        Ok(Self {
            members,
            member_bytes,
            total_bytes,
            visits,
            depth,
            reads,
        })
    }
}

/// Expected pins are obligations supplied by a separately identified manifest.
/// They do not attest that a package was built, tested or authenticated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedMember<'a> {
    path: &'a str,
    role: Role,
    bytes: u64,
    digest: [u8; 32],
}
impl<'a> ExpectedMember<'a> {
    pub fn new(
        path: &'a str,
        role: Role,
        bytes: u64,
        digest: [u8; 32],
    ) -> Result<Self, AuditError> {
        validate_path(path)?;
        let accepted = match role {
            Role::Source => path.starts_with("scripts/lc631/") && source_suffix(path),
            Role::LegacySchema | Role::BoundedSchema => {
                path.starts_with("schemas/") && path.ends_with(".schema.json")
            }
            Role::PluginManifest => path == ".codex-plugin/plugin.json",
            Role::Launcher => path.starts_with("scripts/run-") && path.ends_with(".ps1"),
            Role::Hook => path.starts_with("hooks/") && source_suffix(path),
            Role::Skill => path.starts_with("skills/") && path.ends_with("/SKILL.md"),
            Role::Executable => path == "scripts/lc631/bin/windows-x86_64/lc631.exe",
            Role::Support => source_suffix(path),
        };
        if !accepted {
            return Err(AuditError::WrongRole);
        }
        Ok(Self {
            path,
            role,
            bytes,
            digest,
        })
    }
    pub const fn path(self) -> &'a str {
        self.path
    }
    pub const fn role(self) -> Role {
        self.role
    }
    pub const fn expected_bytes(self) -> u64 {
        self.bytes
    }
    pub const fn expected_digest(self) -> [u8; 32] {
        self.digest
    }
}

pub struct Plan<'a> {
    kind: PackageKind,
    entries: &'a [ExpectedMember<'a>],
    limits: Limits,
    digest: [u8; 32],
}
impl<'a> Plan<'a> {
    pub fn new(
        kind: PackageKind,
        entries: &'a [ExpectedMember<'a>],
        limits: Limits,
    ) -> Result<Self, AuditError> {
        if entries.is_empty() {
            return Err(AuditError::EmptyInventory);
        }
        if entries.len() > limits.members {
            return Err(AuditError::VisitLimit);
        }
        let mut total = 0u64;
        for (index, entry) in entries.iter().enumerate() {
            if entries[..index]
                .iter()
                .any(|other| entry.path.eq_ignore_ascii_case(other.path))
            {
                return Err(AuditError::DuplicatePath);
            }
            if entry.bytes > limits.member_bytes {
                return Err(AuditError::MemberLimit);
            }
            total = total
                .checked_add(entry.bytes)
                .ok_or(AuditError::TotalLimit)?;
            if total > limits.total_bytes {
                return Err(AuditError::TotalLimit);
            }
            if kind == PackageKind::Source && entry.role == Role::Executable {
                return Err(AuditError::WrongRole);
            }
        }
        let required: &[Role] = match kind {
            PackageKind::Source => &[Role::Source, Role::LegacySchema, Role::BoundedSchema],
            PackageKind::PluginWindowsX64 => &[
                Role::Source,
                Role::LegacySchema,
                Role::BoundedSchema,
                Role::PluginManifest,
                Role::Launcher,
                Role::Hook,
                Role::Skill,
                Role::Executable,
            ],
        };
        for role in required {
            if !entries.iter().any(|entry| entry.role == *role) {
                return Err(AuditError::MissingRole(*role));
            }
        }
        let mut hash = Sha256::new();
        hash.update(b"epistesys.tlcx36.distribution-plan.v1\0");
        hash.update([kind_code(kind)]);
        // Exact manifest order is retained; order-independent inventory identity is a future codec.
        for entry in entries {
            hash.update((entry.path.len() as u64).to_le_bytes());
            hash.update(entry.path.as_bytes());
            hash.update([role_code(entry.role)]);
            hash.update(entry.bytes.to_le_bytes());
            hash.update(entry.digest);
        }
        for value in [
            limits.members as u64,
            limits.member_bytes,
            limits.total_bytes,
            limits.visits as u64,
            limits.depth as u64,
            limits.reads as u64,
        ] {
            hash.update(value.to_le_bytes());
        }
        Ok(Self {
            kind,
            entries,
            limits,
            digest: hash.finalize().into(),
        })
    }
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
    pub const fn kind(&self) -> PackageKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberState {
    Matched,
    Missing,
    LengthMismatch,
    DigestMismatch,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileObservation {
    bytes: u64,
    digest: [u8; 32],
}
impl FileObservation {
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemberObservation<'a> {
    expected: ExpectedMember<'a>,
    observed: Option<FileObservation>,
    state: MemberState,
}
impl<'a> MemberObservation<'a> {
    pub const fn expected(&self) -> ExpectedMember<'a> {
        self.expected
    }
    pub const fn observed(&self) -> Option<FileObservation> {
        self.observed
    }
    pub const fn state(&self) -> MemberState {
        self.state
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityState {
    MatchedInventory,
    Mismatch,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionEvidence {
    NeedsFreshPackageExecution,
}
pub struct AuditReport<'a> {
    plan: [u8; 32],
    members: Vec<MemberObservation<'a>>,
    unlisted: Vec<String>,
    identity: IdentityState,
    bytes: u64,
}
impl<'a> AuditReport<'a> {
    pub const fn plan_digest(&self) -> [u8; 32] {
        self.plan
    }
    pub fn members(&self) -> &[MemberObservation<'a>] {
        &self.members
    }
    pub fn unlisted(&self) -> &[String] {
        &self.unlisted
    }
    pub const fn identity(&self) -> IdentityState {
        self.identity
    }
    pub const fn observed_bytes(&self) -> u64 {
        self.bytes
    }
    pub const fn execution(&self) -> ExecutionEvidence {
        ExecutionEvidence::NeedsFreshPackageExecution
    }
}

fn validate_path(path: &str) -> Result<(), AuditError> {
    if path.is_empty()
        || path.len() > 1024
        || !path.is_ascii()
        || path.starts_with('/')
        || path
            .bytes()
            .any(|byte| byte < 32 || byte == 127 || b"\\:*?\"<>|".contains(&byte))
    {
        return Err(AuditError::InvalidPath);
    }
    for part in path.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.ends_with(['.', ' ']) {
            return Err(AuditError::InvalidPath);
        }
        let lower = part.to_ascii_lowercase();
        let stem = lower.split('.').next().ok_or(AuditError::InvalidPath)?;
        if matches!(stem, "con" | "prn" | "aux" | "nul")
            || (stem.len() == 4
                && (stem.starts_with("com") || stem.starts_with("lpt"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            return Err(AuditError::InvalidPath);
        }
        if matches!(
            lower.as_str(),
            ".git"
                | ".cache"
                | "target"
                | "credentials"
                | "secrets"
                | "receipt-root"
                | "replay-ledger"
        ) || lower.starts_with(".env")
            || lower.starts_with("id_rsa")
            || lower.starts_with("id_ed25519")
            || [
                ".key",
                ".pem",
                ".pfx",
                ".p12",
                ".token",
                ".gguf",
                ".safetensors",
                ".onnx",
                ".pt",
                ".pth",
            ]
            .iter()
            .any(|suffix| lower.ends_with(suffix))
        {
            return Err(AuditError::ForbiddenMember);
        }
    }
    Ok(())
}
fn kind_code(kind: PackageKind) -> u8 {
    match kind {
        PackageKind::Source => 0,
        PackageKind::PluginWindowsX64 => 1,
    }
}
fn source_suffix(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [
        ".rs",
        ".toml",
        ".lock",
        ".py",
        ".json",
        ".md",
        ".ps1",
        ".sh",
        ".txt",
        ".yml",
        ".yaml",
        ".gitignore",
        ".gitattributes",
    ]
    .iter()
    .any(|suffix| lower.ends_with(suffix))
}
fn role_code(role: Role) -> u8 {
    match role {
        Role::Source => 0,
        Role::LegacySchema => 1,
        Role::BoundedSchema => 2,
        Role::PluginManifest => 3,
        Role::Launcher => 4,
        Role::Hook => 5,
        Role::Skill => 6,
        Role::Executable => 7,
        Role::Support => 8,
    }
}
