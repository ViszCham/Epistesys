//! ローカルsolver宣言のbounded decode。登録は認証・観測・権限ではない。
//! Bounded caller declarations only; loading never executes a solver.
#![forbid(unsafe_code)]

use lc631_runtime::canonical_solve::{
    BackendId, LogicProfile, SolverProtocol, SolverRegistration, SolverRegistry,
    SolverRegistryError, SolverResourceCaps,
};
use lc631_runtime::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use serde::de::{Error as _, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::io::Read;

pub mod file;
pub(crate) mod run;

pub const CONFIG_MAX_BYTES: usize = 64 * 1024;
/// Conservative logical backing for input, bounded DTOs and checked registry.
/// This is not allocator/RSS accounting or a guarantee against OOM.
pub const CONFIG_RAM_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryConfigError {
    Resource(ResourceError),
    ReadFailed,
    TooLarge,
    AllocationFailed,
    InvalidJson,
    InvalidDigest,
    UnsupportedArguments,
    UnsupportedDiskBudget,
    Registry(SolverRegistryError),
}

/// The getter borrows rather than transfers the charged registry allocation.
/// Drop registry allocations before releasing the final field's RAM claim.
pub struct LoadedRegistry<'view> {
    registry: SolverRegistry,
    _claim: ResourceClaim<'view>,
}

impl LoadedRegistry<'_> {
    pub fn registry(&self) -> &SolverRegistry {
        &self.registry
    }
}

/// Decode at most 64KiB after admission, consuming at most cap+1 bytes.
/// Generic Read has no path/type/deadline policy: the command's explicit local
/// regular-file loader must supply those separately. No executable is opened.
pub fn read_registry<'view>(
    reader: &mut impl Read,
    view: &'view ResourceReservationView<'_>,
) -> Result<LoadedRegistry<'view>, RegistryConfigError> {
    let claim = claim_config(view)?;
    read_registry_claimed(reader, view, claim)
}

fn claim_config<'view>(
    view: &'view ResourceReservationView<'_>,
) -> Result<ResourceClaim<'view>, RegistryConfigError> {
    let amount =
        ResourceAmount::checked(CONFIG_RAM_BYTES).map_err(RegistryConfigError::Resource)?;
    view.claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
        .map_err(RegistryConfigError::Resource)
}

fn read_registry_claimed<'view>(
    reader: &mut impl Read,
    view: &'view ResourceReservationView<'_>,
    claim: ResourceClaim<'view>,
) -> Result<LoadedRegistry<'view>, RegistryConfigError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(CONFIG_MAX_BYTES + 1)
        .map_err(|_| RegistryConfigError::AllocationFailed)?;
    let mut buffer = [0u8; 4096];
    while bytes.len() < CONFIG_MAX_BYTES + 1 {
        view.check().map_err(RegistryConfigError::Resource)?;
        let remaining = CONFIG_MAX_BYTES + 1 - bytes.len();
        let capacity = remaining.min(buffer.len());
        let count = reader
            .read(&mut buffer[..capacity])
            .map_err(|_| RegistryConfigError::ReadFailed)?;
        view.check().map_err(RegistryConfigError::Resource)?;
        if count == 0 {
            break;
        }
        // Read's safe contract requires count<=the supplied buffer length.
        let part = buffer[..capacity]
            .get(..count)
            .ok_or(RegistryConfigError::ReadFailed)?;
        bytes.extend_from_slice(part);
    }
    view.check().map_err(RegistryConfigError::Resource)?;
    if bytes.len() > CONFIG_MAX_BYTES {
        return Err(RegistryConfigError::TooLarge);
    }
    let config: RegistryConfig =
        serde_json::from_slice(&bytes).map_err(|_| RegistryConfigError::InvalidJson)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(config.entries.0.len())
        .map_err(|_| RegistryConfigError::AllocationFailed)?;
    for entry in config.entries.0 {
        entries.push(entry.into_registration()?);
    }
    let registry = SolverRegistry::checked(entries).map_err(RegistryConfigError::Registry)?;
    view.check().map_err(RegistryConfigError::Resource)?;
    Ok(LoadedRegistry {
        registry,
        _claim: claim,
    })
}

enum SchemaVersion {
    V1,
}
impl<'de> Deserialize<'de> for SchemaVersion {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        exact_tag(d, "epistesys-solver-registry.v1", Self::V1)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryConfig {
    #[serde(rename = "schema_version")]
    _schema_version: SchemaVersion,
    entries: BoundedSequence<Entry, 16>,
}
enum Backend {
    Z3,
}
impl<'de> Deserialize<'de> for Backend {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        exact_tag(d, "z3", Self::Z3)
    }
}
enum Profile {
    QfLia,
}
impl<'de> Deserialize<'de> for Profile {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        exact_tag(d, "smt_boolean_qf_lia", Self::QfLia)
    }
}
enum Protocol {
    StatusV1,
    WireV2,
}
impl<'de> Deserialize<'de> for Protocol {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ProtocolTag;
        impl<'de> Visitor<'de> for ProtocolTag {
            type Value = Protocol;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("smt_lib_status_v1 or smt_lib_payload_wire_v2 as a string")
            }
            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Protocol, E> {
                match text {
                    "smt_lib_status_v1" => Ok(Protocol::StatusV1),
                    "smt_lib_payload_wire_v2" => Ok(Protocol::WireV2),
                    _ => Err(E::custom("unsupported protocol tag")),
                }
            }
        }
        d.deserialize_str(ProtocolTag)
    }
}
// deserialize_str deliberately rejects Serde's alternate unit-enum map form.
fn exact_tag<'de, D: Deserializer<'de>, T>(
    d: D,
    expected: &'static str,
    value: T,
) -> Result<T, D::Error> {
    struct Tag<T> {
        expected: &'static str,
        value: T,
    }
    impl<'de, T> Visitor<'de> for Tag<T> {
        type Value = T;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "the string {}", self.expected)
        }
        fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<T, E> {
            if text == self.expected {
                Ok(self.value)
            } else {
                Err(E::custom("unsupported tag"))
            }
        }
    }
    d.deserialize_str(Tag { expected, value })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: Backend,
    profile: Profile,
    protocol: Protocol,
    executable: String,
    version: String,
    binary_sha256: String,
    fixed_arguments: BoundedSequence<String, 128>,
    caps: Caps,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Caps {
    max_argv_count: usize,
    max_argv_bytes: usize,
    max_stdin_bytes: usize,
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
    max_timeout_ms: u64,
    max_ram_bytes: u64,
    max_disk_bytes: u64,
    max_cleanup_bytes: u64,
    max_child_count: u32,
}
impl Entry {
    fn into_registration(self) -> Result<SolverRegistration, RegistryConfigError> {
        let Backend::Z3 = self.id;
        let Profile::QfLia = self.profile;
        let protocol = match self.protocol {
            Protocol::StatusV1 => SolverProtocol::SmtLibStatusV1,
            Protocol::WireV2 => SolverProtocol::SmtLibPayloadWireV2,
        };
        if self.fixed_arguments.0.as_slice() != ["-in", "-smt2"] {
            return Err(RegistryConfigError::UnsupportedArguments);
        }
        if self.caps.max_disk_bytes != 0 {
            return Err(RegistryConfigError::UnsupportedDiskBudget);
        }
        let digest = decode_digest(&self.binary_sha256)?;
        let c = self.caps;
        SolverRegistration::checked(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            protocol,
            &self.executable,
            &self.version,
            digest,
            self.fixed_arguments.0,
            SolverResourceCaps {
                max_argv_count: c.max_argv_count,
                max_argv_bytes: c.max_argv_bytes,
                max_stdin_bytes: c.max_stdin_bytes,
                max_stdout_bytes: c.max_stdout_bytes,
                max_stderr_bytes: c.max_stderr_bytes,
                max_timeout_ms: c.max_timeout_ms,
                max_ram_bytes: c.max_ram_bytes,
                max_disk_bytes: c.max_disk_bytes,
                max_cleanup_bytes: c.max_cleanup_bytes,
                max_child_count: c.max_child_count,
            },
        )
        .map_err(RegistryConfigError::Registry)
    }
}
fn decode_digest(text: &str) -> Result<[u8; 32], RegistryConfigError> {
    if text.len() != 64 {
        return Err(RegistryConfigError::InvalidDigest);
    }
    fn digit(byte: u8) -> Result<u8, RegistryConfigError> {
        match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            b'A'..=b'F' => Ok(byte - b'A' + 10),
            _ => Err(RegistryConfigError::InvalidDigest),
        }
    }
    let mut digest = [0; 32];
    for (out, pair) in digest.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
        *out = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    Ok(digest)
}

/// Bound sequence storage before growth; the source byte cap also bounds all
/// strings. The rejected extra item can be decoded, but is never appended.
struct BoundedSequence<T, const MAX: usize>(Vec<T>);
impl<'de, T: Deserialize<'de>, const MAX: usize> Deserialize<'de> for BoundedSequence<T, MAX> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SequenceVisitor<T, const MAX: usize>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>, const MAX: usize> Visitor<'de> for SequenceVisitor<T, MAX> {
            type Value = BoundedSequence<T, MAX>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "an array with at most {MAX} entries")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                values
                    .try_reserve_exact(MAX)
                    .map_err(|_| A::Error::custom("allocation failed"))?;
                while let Some(value) = seq.next_element()? {
                    if values.len() == MAX {
                        return Err(A::Error::custom("sequence limit"));
                    }
                    values.push(value);
                }
                Ok(BoundedSequence(values))
            }
        }
        deserializer.deserialize_seq(SequenceVisitor::<T, MAX>(std::marker::PhantomData))
    }
}
