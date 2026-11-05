//! Bounded, recomputable in-memory cache. Disk persistence and evidence
//! retention deliberately use other owners and are not implemented here.
use crate::resource::{Digest32, ResourceError, ResourceKind, ResourceReservationView};
use lc631_substrate::resource::{ResourceAmount, ResourceClaim, ResourceRequest};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

const MAX_CACHE_ENTRIES: usize = 4_096;
const MAX_CACHE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CACHE_TTL: Duration = Duration::from_secs(60 * 60);
const ENTRY_METADATA_RESERVATION: usize = 512;
const PAYLOAD_HASH_HEADER_BYTES: usize = 96;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CacheMode {
    Off,
    MemoryOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CachePolicy {
    mode: CacheMode,
    max_entries: usize,
    max_bytes: u64,
    ttl: Duration,
}

impl CachePolicy {
    pub(crate) const fn disabled() -> Self {
        Self {
            mode: CacheMode::Off,
            max_entries: 0,
            max_bytes: 0,
            ttl: Duration::ZERO,
        }
    }

    pub(crate) fn memory(
        max_entries: usize,
        max_bytes: u64,
        ttl: Duration,
    ) -> Result<Self, CacheError> {
        if max_entries == 0
            || max_entries > MAX_CACHE_ENTRIES
            || max_bytes == 0
            || max_bytes > MAX_CACHE_BYTES
            || max_bytes > usize::MAX as u64
            || ttl.is_zero()
            || ttl > MAX_CACHE_TTL
        {
            return Err(CacheError::InvalidPolicy);
        }
        Ok(Self {
            mode: CacheMode::MemoryOnly,
            max_entries,
            max_bytes,
            ttl,
        })
    }

    /// Requesting disk caching without an owner-minted manifest is visible
    /// failure. Even with a manifest this alpha slice has no disk writer.
    pub(crate) fn request_disk_cache(
        max_entries: usize,
        max_bytes: u64,
        ttl: Duration,
        owner_manifest: Option<&OwnedCacheRootManifest>,
    ) -> Result<Self, CacheError> {
        if max_entries == 0
            || max_entries > MAX_CACHE_ENTRIES
            || max_bytes == 0
            || max_bytes > MAX_CACHE_BYTES
            || max_bytes > usize::MAX as u64
            || ttl.is_zero()
            || ttl > MAX_CACHE_TTL
        {
            return Err(CacheError::InvalidPolicy);
        }
        match owner_manifest {
            None => Err(CacheError::OwnerManifestRequired),
            Some(manifest)
                if manifest
                    ._root_identity
                    .as_bytes()
                    .iter()
                    .all(|byte| *byte == 0)
                    || manifest
                        ._manifest_digest
                        .as_bytes()
                        .iter()
                        .all(|byte| *byte == 0) =>
            {
                Err(CacheError::InvalidOwnerManifest)
            }
            Some(_) => Err(CacheError::DiskBackendUnsupported),
        }
    }

    pub(crate) const fn mode(self) -> CacheMode {
        self.mode
    }

    pub(crate) const fn disk_cache_enabled(self) -> bool {
        false
    }
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Opaque marker reserved for a future host-minted, root-bound cache manifest.
/// This slice intentionally provides no public constructor for it.
#[derive(Debug)]
pub(crate) struct OwnedCacheRootManifest {
    _root_identity: Digest32,
    _manifest_digest: Digest32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecomputableArtifactKind {
    ParsedStructure,
    ProgramLowering,
    SolverQueryPlan,
    DerivedNumericValue,
}

/// Every revision that can change the interpretation or validation of a
/// recomputable value is required; absent inputs cannot be filled with zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheIdentity {
    source: Digest32,
    context: Digest32,
    goal: Digest32,
    probes: Digest32,
    logic: Digest32,
    solver: Digest32,
    options: Digest32,
    model: Digest32,
    tool: Digest32,
    validator: Digest32,
    codec: Digest32,
    measurement_revisions: Option<[Digest32; 8]>,
}

impl CacheIdentity {
    #[allow(clippy::too_many_arguments)]
    pub fn checked(
        source: Digest32,
        context: Digest32,
        goal: Digest32,
        probes: Digest32,
        logic: Digest32,
        solver: Digest32,
        options: Digest32,
        model: Digest32,
        tool: Digest32,
        validator: Digest32,
        codec: Digest32,
    ) -> Result<Self, CacheError> {
        let identity = Self {
            source,
            context,
            goal,
            probes,
            logic,
            solver,
            options,
            model,
            tool,
            validator,
            codec,
            measurement_revisions: None,
        };
        if identity
            .revisions()
            .iter()
            .any(|revision| revision.as_bytes().iter().all(|byte| *byte == 0))
        {
            return Err(CacheError::InvalidIdentity);
        }
        Ok(identity)
    }

    pub(crate) const fn source_revision(self) -> Digest32 {
        self.source
    }

    pub(crate) fn with_measurement_revisions(
        mut self,
        revisions: [Digest32; 8],
    ) -> Result<Self, CacheError> {
        if revisions
            .iter()
            .any(|revision| revision.as_bytes().iter().all(|b| *b == 0))
        {
            return Err(CacheError::InvalidIdentity);
        }
        self.measurement_revisions = Some(revisions);
        Ok(self)
    }

    fn revisions(self) -> [Digest32; 11] {
        [
            self.source,
            self.context,
            self.goal,
            self.probes,
            self.logic,
            self.solver,
            self.options,
            self.model,
            self.tool,
            self.validator,
            self.codec,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CacheKey(Digest32);

impl CacheKey {
    pub(crate) const fn digest(self) -> Digest32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheError {
    Disabled,
    InvalidPolicy,
    InvalidIdentity,
    SourceMismatch,
    OwnerManifestRequired,
    DiskBackendUnsupported,
    InvalidOwnerManifest,
    EntryTooLarge,
    EntryCapacity,
    ByteCapacity,
    DuplicateKey,
    KeyConflict,
    AllocationFailed,
    ArithmeticOverflow,
    ClockOverflow,
    ClockRewound,
    MissingEntry,
    Resource(ResourceError),
}

#[derive(Debug)]
pub(crate) enum CacheLookup<'store, 'resource> {
    Disabled,
    Miss,
    Expired(CacheKey),
    Corrupt(CacheKey),
    Hit(CacheLease<'store, 'resource>),
}

impl<'store, 'resource> PartialEq for CacheLookup<'store, 'resource> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Disabled, Self::Disabled) | (Self::Miss, Self::Miss) => true,
            (Self::Expired(left), Self::Expired(right))
            | (Self::Corrupt(left), Self::Corrupt(right)) => left == right,
            (Self::Hit(left), Self::Hit(right)) => {
                left.key() == right.key() && left.payload_digest() == right.payload_digest()
            }
            _ => false,
        }
    }
}
impl<'store, 'resource> Eq for CacheLookup<'store, 'resource> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Eviction {
    Removed,
    NotFound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CacheStats {
    pub(crate) entries: usize,
    pub(crate) payload_bytes: u64,
    pub(crate) mode: CacheMode,
    pub(crate) disk_cache_enabled: bool,
}

/// Only runtime-owned producers can create this wrapper. It intentionally has
/// no serde implementation and no public constructor; receipts, authority,
/// replay state, weights and build artifacts are not cacheable payload kinds.
pub(crate) struct RecomputablePayload {
    identity: CacheIdentity,
    kind: RecomputableArtifactKind,
    bytes: Vec<u8>,
}

impl RecomputablePayload {
    pub(crate) fn checked(
        identity: CacheIdentity,
        kind: RecomputableArtifactKind,
        bytes: Vec<u8>,
    ) -> Result<Self, CacheError> {
        if bytes.len() > MAX_ENTRY_BYTES as usize {
            return Err(CacheError::EntryTooLarge);
        }
        Ok(Self {
            identity,
            kind,
            bytes,
        })
    }

    #[cfg(test)]
    pub(crate) fn fixture(
        identity: CacheIdentity,
        kind: RecomputableArtifactKind,
        bytes: &[u8],
    ) -> Result<Self, CacheError> {
        if bytes.len() > MAX_ENTRY_BYTES as usize {
            return Err(CacheError::EntryTooLarge);
        }
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(bytes.len())
            .map_err(|_| CacheError::AllocationFailed)?;
        owned.extend_from_slice(bytes);
        Self::checked(identity, kind, owned)
    }
}

struct CacheEntry<'resource> {
    identity: CacheIdentity,
    key: CacheKey,
    kind: RecomputableArtifactKind,
    payload: Vec<u8>,
    payload_digest: Digest32,
    inserted_at: Instant,
    expires_at: Instant,
    _payload_claim: ResourceClaim<'resource>,
}

pub(crate) struct CacheStore<'store, 'resource> {
    policy: CachePolicy,
    owner: Option<&'store ResourceReservationView<'resource>>,
    entries: Vec<CacheEntry<'store>>,
    payload_bytes: u64,
    _metadata_claim: Option<ResourceClaim<'store>>,
}

impl<'store, 'resource> CacheStore<'store, 'resource> {
    pub(crate) fn disabled() -> CacheStore<'static, 'static> {
        CacheStore {
            policy: CachePolicy::disabled(),
            owner: None,
            entries: Vec::new(),
            payload_bytes: 0,
            _metadata_claim: None,
        }
    }

    pub(crate) fn new_memory(
        policy: CachePolicy,
        owner: &'store ResourceReservationView<'resource>,
    ) -> Result<Self, CacheError> {
        if policy.mode != CacheMode::MemoryOnly {
            return Err(CacheError::Disabled);
        }
        owner.check().map_err(CacheError::Resource)?;
        let maximum_slots = policy
            .max_entries
            .checked_mul(2)
            .ok_or(CacheError::ArithmeticOverflow)?;
        let slot_bytes = std::mem::size_of::<CacheEntry<'store>>()
            .checked_add(ENTRY_METADATA_RESERVATION)
            .ok_or(CacheError::ArithmeticOverflow)?;
        let table_bytes = slot_bytes
            .checked_mul(maximum_slots)
            .ok_or(CacheError::ArithmeticOverflow)?;
        let metadata_claim = claim_ram(owner, table_bytes)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(policy.max_entries)
            .map_err(|_| CacheError::AllocationFailed)?;
        if entries.capacity() > maximum_slots {
            return Err(CacheError::AllocationFailed);
        }
        Ok(Self {
            policy,
            owner: Some(owner),
            entries,
            payload_bytes: 0,
            _metadata_claim: Some(metadata_claim),
        })
    }

    pub(crate) fn insert(
        &mut self,
        payload: &RecomputablePayload,
        now: Instant,
    ) -> Result<CacheKey, CacheError> {
        self.insert_copy(payload.identity, payload.kind, &payload.bytes, now)
    }

    /// Copy only borrowed, explicitly recomputable input. The input's original
    /// owner is not consumed and no filesystem/evidence/replay root is accepted.
    pub(crate) fn insert_copy(
        &mut self,
        identity: CacheIdentity,
        kind: RecomputableArtifactKind,
        bytes: &[u8],
        now: Instant,
    ) -> Result<CacheKey, CacheError> {
        // A new admission is also a reclamation opportunity. Expired entries
        // are removed before capacity is checked; clock regressions remain
        // visible and cannot extend an entry's lifetime.
        self.reclaim_expired(now)?;
        let owner = self.owner.ok_or(CacheError::Disabled)?;
        owner.check().map_err(CacheError::Resource)?;
        if identity.source_revision() != owner.source_revision() {
            return Err(CacheError::SourceMismatch);
        }
        let payload_bytes =
            u64::try_from(bytes.len()).map_err(|_| CacheError::ArithmeticOverflow)?;
        if payload_bytes > MAX_ENTRY_BYTES || payload_bytes > self.policy.max_bytes {
            return Err(CacheError::EntryTooLarge);
        }
        if self.entries.len() >= self.policy.max_entries {
            return Err(CacheError::EntryCapacity);
        }
        let next_total = self
            .payload_bytes
            .checked_add(payload_bytes)
            .ok_or(CacheError::ArithmeticOverflow)?;
        if next_total > self.policy.max_bytes {
            return Err(CacheError::ByteCapacity);
        }
        let expires_at = now
            .checked_add(self.policy.ttl)
            .ok_or(CacheError::ClockOverflow)?;
        let key = cache_key(owner, identity, kind)?;
        if let Some(existing) = self.entries.iter().find(|entry| entry.key == key) {
            return Err(if existing.identity == identity {
                CacheError::DuplicateKey
            } else {
                CacheError::KeyConflict
            });
        }
        let payload_digest = digest_payload(owner, bytes)?;
        let charged_payload_bytes = bytes
            .len()
            .checked_mul(2)
            .and_then(|capacity| capacity.checked_add(ENTRY_METADATA_RESERVATION))
            .ok_or(CacheError::ArithmeticOverflow)?
            .max(1);
        let payload_claim = claim_ram(owner, charged_payload_bytes)?;
        let mut cached_bytes = Vec::new();
        cached_bytes
            .try_reserve_exact(bytes.len())
            .map_err(|_| CacheError::AllocationFailed)?;
        if cached_bytes.capacity() > charged_payload_bytes {
            return Err(CacheError::AllocationFailed);
        }
        cached_bytes.extend_from_slice(bytes);
        owner.check().map_err(CacheError::Resource)?;
        if self.entries.len() == self.entries.capacity() {
            return Err(CacheError::EntryCapacity);
        }
        self.entries.push(CacheEntry {
            identity,
            key,
            kind,
            payload: cached_bytes,
            payload_digest,
            inserted_at: now,
            expires_at,
            _payload_claim: payload_claim,
        });
        self.payload_bytes = next_total;
        Ok(key)
    }

    pub(crate) fn lookup(
        &self,
        identity: &CacheIdentity,
        kind: RecomputableArtifactKind,
        now: Instant,
    ) -> Result<CacheLookup<'_, 'store>, CacheError> {
        if self.policy.mode == CacheMode::Off {
            return Ok(CacheLookup::Disabled);
        }
        let owner = self.owner.ok_or(CacheError::Disabled)?;
        owner.check().map_err(CacheError::Resource)?;
        if identity.source_revision() != owner.source_revision() {
            return Err(CacheError::SourceMismatch);
        }
        let key = cache_key(owner, *identity, kind)?;
        let Some(entry) = self.entries.iter().find(|entry| entry.key == key) else {
            return Ok(CacheLookup::Miss);
        };
        if entry.identity != *identity {
            return Err(CacheError::KeyConflict);
        }
        if now < entry.inserted_at {
            return Err(CacheError::ClockRewound);
        }
        if now >= entry.expires_at {
            return Ok(CacheLookup::Expired(key));
        }
        let observed_digest = digest_payload(owner, &entry.payload)?;
        if observed_digest != entry.payload_digest {
            return Ok(CacheLookup::Corrupt(key));
        }
        owner.check().map_err(CacheError::Resource)?;
        Ok(CacheLookup::Hit(CacheLease { entry }))
    }

    /// Eviction mutates only entries owned by this in-memory cache. A live
    /// `CacheLease` borrows `self`, so Rust prevents eviction while bytes are in use.
    pub(crate) fn evict(&mut self, key: CacheKey) -> Result<Eviction, CacheError> {
        let Some(index) = self.entries.iter().position(|entry| entry.key == key) else {
            return Ok(Eviction::NotFound);
        };
        let payload_bytes = u64::try_from(self.entries[index].payload.len())
            .map_err(|_| CacheError::ArithmeticOverflow)?;
        let remaining = self
            .payload_bytes
            .checked_sub(payload_bytes)
            .ok_or(CacheError::ArithmeticOverflow)?;
        self.entries.remove(index);
        self.payload_bytes = remaining;
        Ok(Eviction::Removed)
    }

    /// Reclaims expired cache-owned memory only. Time is a monotonic `Instant`;
    /// a rewind is a visible error and cannot extend or revive an entry.
    pub(crate) fn reclaim_expired(&mut self, now: Instant) -> Result<usize, CacheError> {
        if self.policy.mode == CacheMode::Off {
            return Ok(0);
        }
        let owner = self.owner.ok_or(CacheError::Disabled)?;
        owner.check().map_err(CacheError::Resource)?;
        if self.entries.iter().any(|entry| now < entry.inserted_at) {
            return Err(CacheError::ClockRewound);
        }
        let removed_bytes = self.entries.iter().try_fold(0_u64, |total, entry| {
            if now >= entry.expires_at {
                total
                    .checked_add(
                        u64::try_from(entry.payload.len())
                            .map_err(|_| CacheError::ArithmeticOverflow)?,
                    )
                    .ok_or(CacheError::ArithmeticOverflow)
            } else {
                Ok(total)
            }
        })?;
        let before = self.entries.len();
        self.entries.retain(|entry| now < entry.expires_at);
        self.payload_bytes = self
            .payload_bytes
            .checked_sub(removed_bytes)
            .ok_or(CacheError::ArithmeticOverflow)?;
        Ok(before - self.entries.len())
    }

    pub(crate) fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.entries.len(),
            payload_bytes: self.payload_bytes,
            mode: self.policy.mode,
            disk_cache_enabled: self.policy.disk_cache_enabled(),
        }
    }

    #[cfg(test)]
    pub(crate) fn corrupt_payload_for_test(&mut self, key: CacheKey) -> Result<(), CacheError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.key == key)
            .ok_or(CacheError::MissingEntry)?;
        if entry.payload.is_empty() {
            return Err(CacheError::EntryTooLarge);
        }
        entry.payload[0] ^= 0x01;
        Ok(())
    }
}

impl Default for CacheStore<'static, 'static> {
    fn default() -> Self {
        Self::disabled()
    }
}

/// A borrowed cache hit. It exposes recomputable bytes and their integrity
/// digest only; it cannot be promoted into evidence, authority or a commit.
pub(crate) struct CacheLease<'store, 'resource> {
    entry: &'store CacheEntry<'resource>,
}

impl CacheLease<'_, '_> {
    pub(crate) const fn key(&self) -> CacheKey {
        self.entry.key
    }

    pub(crate) const fn kind(&self) -> RecomputableArtifactKind {
        self.entry.kind
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.entry.payload
    }

    pub(crate) const fn payload_digest(&self) -> Digest32 {
        self.entry.payload_digest
    }

    pub(crate) const fn claim_boundary(&self) -> &'static str {
        "recomputable bytes with checked identity/digest only; no evidence, authority, or commit"
    }
}

impl std::fmt::Debug for CacheLease<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheLease")
            .field("key", &self.entry.key)
            .field("kind", &self.entry.kind)
            .field("payload_bytes", &self.entry.payload.len())
            .field("payload_digest", &self.entry.payload_digest)
            .finish()
    }
}

fn cache_key(
    owner: &ResourceReservationView<'_>,
    identity: CacheIdentity,
    kind: RecomputableArtifactKind,
) -> Result<CacheKey, CacheError> {
    let _scratch_claim = claim_ram(owner, PAYLOAD_HASH_HEADER_BYTES)?;
    let mut hasher = Sha256::new();
    match identity.measurement_revisions {
        None => hasher.update(b"epistesys.recomputable-cache-key.v2\0"),
        Some(_) => hasher.update(b"epistesys.recomputable-cache-key.v3\0"),
    }
    hasher.update(kind.stable_tag().as_bytes());
    hasher.update([0]);
    for (name, digest) in identity.fields() {
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(digest.as_bytes());
    }
    if let Some(revisions) = identity.measurement_revisions {
        let names = [
            "target",
            "baseline",
            "corpus",
            "oracle",
            "policy",
            "evidence",
            "definition",
            "encoding",
        ];
        for (name, digest) in names.into_iter().zip(revisions) {
            hasher.update(name.as_bytes());
            hasher.update([0]);
            hasher.update(digest.as_bytes());
        }
    }
    Ok(CacheKey(Digest32::from_bytes(hasher.finalize().into())))
}

fn digest_payload(
    owner: &ResourceReservationView<'_>,
    bytes: &[u8],
) -> Result<Digest32, CacheError> {
    let _scratch_claim = claim_ram(owner, PAYLOAD_HASH_HEADER_BYTES)?;
    let payload_len = u64::try_from(bytes.len()).map_err(|_| CacheError::ArithmeticOverflow)?;
    let mut hasher = Sha256::new();
    hasher.update(b"epistesys.recomputable-cache-payload.v2\0");
    hasher.update(payload_len.to_le_bytes());
    hasher.update(bytes);
    Ok(Digest32::from_bytes(hasher.finalize().into()))
}

fn claim_ram<'a>(
    owner: &'a ResourceReservationView<'_>,
    bytes: usize,
) -> Result<ResourceClaim<'a>, CacheError> {
    let bytes = u64::try_from(bytes).map_err(|_| CacheError::ArithmeticOverflow)?;
    owner
        .claim_resources(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(bytes.max(1)).map_err(CacheError::Resource)?,
        )])
        .map_err(CacheError::Resource)
}

impl CacheIdentity {
    fn fields(self) -> [(&'static str, Digest32); 11] {
        [
            ("source", self.source),
            ("context", self.context),
            ("goal", self.goal),
            ("probes", self.probes),
            ("logic", self.logic),
            ("solver", self.solver),
            ("options", self.options),
            ("model", self.model),
            ("tool", self.tool),
            ("validator", self.validator),
            ("codec", self.codec),
        ]
    }
}

impl RecomputableArtifactKind {
    /// Stable domain tags are part of the cache-key protocol. Do not derive
    /// these from Rust enum layout or debug formatting.
    const fn stable_tag(self) -> &'static str {
        match self {
            Self::ParsedStructure => "parsed-structure.v1",
            Self::ProgramLowering => "program-lowering.v1",
            Self::SolverQueryPlan => "solver-query-plan.v1",
            Self::DerivedNumericValue => "derived-numeric-value.v1",
        }
    }
}

#[cfg(test)]
mod corruption_tests {
    #![allow(clippy::drop_non_drop)] // End the borrowed view scope before releasing its owner.

    use super::*;
    use crate::resource::{Cancellation, Deadline, ResourceBroker, ResourceLimits};

    #[test]
    fn tlcx06_corrupt_payload_never_becomes_a_cache_hit_or_evidence() {
        let source = Digest32::from_bytes([1; 32]);
        let owner = ResourceBroker::new(ResourceLimits::uniform(8 * 1024 * 1024).unwrap()).unwrap();
        let run = owner
            .begin_run(
                1,
                source,
                ResourceLimits::uniform(8 * 1024 * 1024).unwrap(),
                Deadline::after(Duration::from_secs(60)).unwrap(),
                Cancellation::default(),
            )
            .unwrap();
        let reservation = run
            .reserve(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(8 * 1024 * 1024).unwrap(),
            )])
            .unwrap();
        let view = reservation.lease();
        let policy = CachePolicy::memory(2, 4096, Duration::from_secs(30)).unwrap();
        let mut cache = CacheStore::new_memory(policy, &view).unwrap();
        let identity = CacheIdentity::checked(
            source,
            Digest32::from_bytes([2; 32]),
            Digest32::from_bytes([3; 32]),
            Digest32::from_bytes([4; 32]),
            Digest32::from_bytes([5; 32]),
            Digest32::from_bytes([6; 32]),
            Digest32::from_bytes([7; 32]),
            Digest32::from_bytes([8; 32]),
            Digest32::from_bytes([9; 32]),
            Digest32::from_bytes([10; 32]),
            Digest32::from_bytes([11; 32]),
        )
        .unwrap();
        let now = Instant::now();
        let payload = RecomputablePayload::fixture(
            identity,
            RecomputableArtifactKind::ParsedStructure,
            b"source-bound",
        )
        .unwrap();
        let key = cache.insert(&payload, now).unwrap();
        // Independently calculated SHA-256 over the frozen v2 field grammar.
        assert_eq!(
            key.digest().as_bytes(),
            &[
                0xa6, 0xc1, 0x26, 0x08, 0x47, 0xa3, 0xb1, 0x6c, 0xf7, 0xa5, 0xe6, 0x71, 0x59, 0x85,
                0x0e, 0xfa, 0xcc, 0x16, 0x98, 0xa1, 0xc1, 0xae, 0xff, 0xd3, 0xec, 0xe9, 0xd0, 0x12,
                0xf0, 0x3f, 0xbf, 0x21,
            ]
        );
        let extended = identity
            .with_measurement_revisions([Digest32::from_bytes([12; 32]); 8])
            .unwrap();
        assert_ne!(
            key,
            cache_key(&view, extended, RecomputableArtifactKind::ParsedStructure).unwrap()
        );
        cache.corrupt_payload_for_test(key).unwrap();
        assert!(matches!(
            cache
                .lookup(&identity, RecomputableArtifactKind::ParsedStructure, now)
                .unwrap(),
            CacheLookup::Corrupt(found) if found == key
        ));
        drop(cache);
        drop(view);
        let mut reservation = reservation;
        reservation.release_confirmed().unwrap();
    }
}
