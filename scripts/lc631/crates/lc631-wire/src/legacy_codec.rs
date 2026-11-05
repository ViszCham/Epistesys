//! Explicit legacy codecs. Parsing and digest agreement never authenticate data.
//!
//! Receipt MACs retain their custom NUL-delimited text, not JSON. Replay V1/V2
//! retain their distinct compact-JSON preimages. Neither uses FrameDigest.

use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptError, ReceiptPolicy, ReceiptVerifier, ReplayGuard, UntrustedReceipt, VerifiedReceipt,
    RECEIPT_SCHEMA,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::{self, Write};

/// Admission ceiling, not permission to enlarge a caller's existing I/O limit.
pub const MAX_LEGACY_JSON_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyJsonKind {
    ReceiptV1,
    ReplayV1,
    ReplayV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LegacyCodecError {
    InvalidLimit,
    LimitExceeded,
    AllocationFailed,
    InvalidJson,
    SchemaMismatch,
    EncodingFailed,
    WrongKind,
    DigestMismatch,
    Receipt(ReceiptError),
}

/// Borrow original transport bytes while owning only the typed, untrusted DTO.
/// This is not a verified receipt, replay chain, execution permit or host receipt.
pub struct LegacyJsonDocument<'a> {
    original: &'a [u8],
    payload: LegacyPayload,
    limit: usize,
}

enum LegacyPayload {
    Receipt(UntrustedReceipt),
    ReplayV1(ReplayEntryV1),
    ReplayV2(ReplayEntryV2),
}

impl<'a> LegacyJsonDocument<'a> {
    pub fn decode(
        kind: LegacyJsonKind,
        input: &'a [u8],
        max_bytes: usize,
    ) -> Result<Self, LegacyCodecError> {
        if max_bytes == 0 || max_bytes > MAX_LEGACY_JSON_BYTES {
            return Err(LegacyCodecError::InvalidLimit);
        }
        if input.len() > max_bytes {
            return Err(LegacyCodecError::LimitExceeded);
        }
        let payload = match kind {
            LegacyJsonKind::ReceiptV1 => {
                // The existing strict DTO checks duplicate/unknown fields first.
                let receipt = parse(input)?;
                let version: ReceiptVersion = parse(input)?;
                require_schema(&version.claims.schema_version, RECEIPT_SCHEMA)?;
                LegacyPayload::Receipt(receipt)
            }
            LegacyJsonKind::ReplayV1 => {
                let entry: ReplayEntryV1 = parse(input)?;
                require_schema(&entry.schema_version, "lc631-host-replay.v1")?;
                LegacyPayload::ReplayV1(entry)
            }
            LegacyJsonKind::ReplayV2 => {
                let entry: ReplayEntryV2 = parse(input)?;
                require_schema(&entry.schema_version, "lc631-host-replay.v2")?;
                LegacyPayload::ReplayV2(entry)
            }
        };
        Ok(Self {
            original: input,
            payload,
            limit: max_bytes,
        })
    }

    pub fn original_bytes(&self) -> &'a [u8] {
        self.original
    }

    /// An explicit normalized legacy DTO view; not the original transport bytes.
    pub fn compact_legacy_json(&self) -> Result<Vec<u8>, LegacyCodecError> {
        match &self.payload {
            LegacyPayload::Receipt(value) => encode_bounded(value, self.limit),
            LegacyPayload::ReplayV1(value) => encode_bounded(value, self.limit),
            LegacyPayload::ReplayV2(value) => encode_bounded(value, self.limit),
        }
    }

    pub fn receipt(&self) -> Option<&UntrustedReceipt> {
        match &self.payload {
            LegacyPayload::Receipt(receipt) => Some(receipt),
            LegacyPayload::ReplayV1(_) | LegacyPayload::ReplayV2(_) => None,
        }
    }

    /// Only the existing verifier may promote the untrusted receipt, using all
    /// current-policy checks and the caller-owned nonce/replay state.
    pub fn verify_receipt(
        self,
        verifier: &ReceiptVerifier,
        policy: &ReceiptPolicy,
        replay: &mut ReplayGuard,
    ) -> Result<VerifiedReceipt, LegacyCodecError> {
        match self.payload {
            LegacyPayload::Receipt(receipt) => verifier
                .verify(receipt, policy, replay)
                .map_err(LegacyCodecError::Receipt),
            LegacyPayload::ReplayV1(_) | LegacyPayload::ReplayV2(_) => {
                Err(LegacyCodecError::WrongKind)
            }
        }
    }

    /// Compact bytes matching the corresponding existing host basis struct.
    /// The entry's own hash is deliberately excluded. This is not a MAC.
    pub fn replay_preimage(&self) -> Result<Vec<u8>, LegacyCodecError> {
        match &self.payload {
            LegacyPayload::ReplayV1(entry) => encode_bounded(&entry.basis(), self.limit),
            LegacyPayload::ReplayV2(entry) => encode_bounded(&entry.basis(), self.limit),
            LegacyPayload::Receipt(_) => Err(LegacyCodecError::WrongKind),
        }
    }

    /// Checks ONE unauthenticated entry only: no chain order, trusted head,
    /// rollback resistance, current authority or host delivery is established.
    pub fn check_replay_integrity(&self) -> Result<(), LegacyCodecError> {
        let expected = match &self.payload {
            LegacyPayload::ReplayV1(entry) => &entry.entry_sha256,
            LegacyPayload::ReplayV2(entry) => &entry.entry_sha256,
            LegacyPayload::Receipt(_) => return Err(LegacyCodecError::WrongKind),
        };
        let bytes = self.replay_preimage()?;
        let text = std::str::from_utf8(&bytes).map_err(|_| LegacyCodecError::EncodingFailed)?;
        if stable_sha256(text) != *expected {
            return Err(LegacyCodecError::DigestMismatch);
        }
        Ok(())
    }
}

fn parse<T: DeserializeOwned>(input: &[u8]) -> Result<T, LegacyCodecError> {
    // Do not expose serde's full input-bearing diagnostics for signed payloads.
    serde_json::from_slice(input).map_err(|_| LegacyCodecError::InvalidJson)
}

fn require_schema(actual: &str, expected: &str) -> Result<(), LegacyCodecError> {
    if actual == expected {
        Ok(())
    } else {
        Err(LegacyCodecError::SchemaMismatch)
    }
}

// Version inspection is separate because ReceiptClaims intentionally keeps its
// private wire fields. Full strict decoding always precedes this limited view.
#[derive(Deserialize)]
struct ReceiptVersion {
    claims: SchemaVersion,
}

#[derive(Deserialize)]
struct SchemaVersion {
    schema_version: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReplayEntryV1 {
    schema_version: String,
    sequence: u64,
    previous_entry_sha256: String,
    artifact_id: u64,
    turn_id: u64,
    candidate_digest: String,
    output_digest: String,
    host_callback_digest: String,
    host_seed_receipt_digest: String,
    entry_sha256: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReplayEntryV2 {
    schema_version: String,
    sequence: u64,
    previous_entry_sha256: String,
    artifact_id: u64,
    turn_id: u64,
    candidate_digest: String,
    output_digest: String,
    host_callback_digest: String,
    host_seed_receipt_digest: String,
    authenticity_receipt_digest: String,
    entry_sha256: String,
}

// Declaration order exactly matches host::ReplayEntryBasisV1/V2. Do not convert
// to Value/BTreeMap, flatten versions, or sort these fields before hashing.
#[derive(Serialize)]
struct ReplayBasisV1<'a> {
    schema_version: &'a str,
    sequence: u64,
    previous_entry_sha256: &'a str,
    artifact_id: u64,
    turn_id: u64,
    candidate_digest: &'a str,
    output_digest: &'a str,
    host_callback_digest: &'a str,
    host_seed_receipt_digest: &'a str,
}

#[derive(Serialize)]
struct ReplayBasisV2<'a> {
    schema_version: &'a str,
    sequence: u64,
    previous_entry_sha256: &'a str,
    artifact_id: u64,
    turn_id: u64,
    candidate_digest: &'a str,
    output_digest: &'a str,
    host_callback_digest: &'a str,
    host_seed_receipt_digest: &'a str,
    authenticity_receipt_digest: &'a str,
}

impl ReplayEntryV1 {
    fn basis(&self) -> ReplayBasisV1<'_> {
        ReplayBasisV1 {
            schema_version: &self.schema_version,
            sequence: self.sequence,
            previous_entry_sha256: &self.previous_entry_sha256,
            artifact_id: self.artifact_id,
            turn_id: self.turn_id,
            candidate_digest: &self.candidate_digest,
            output_digest: &self.output_digest,
            host_callback_digest: &self.host_callback_digest,
            host_seed_receipt_digest: &self.host_seed_receipt_digest,
        }
    }
}

impl ReplayEntryV2 {
    fn basis(&self) -> ReplayBasisV2<'_> {
        ReplayBasisV2 {
            schema_version: &self.schema_version,
            sequence: self.sequence,
            previous_entry_sha256: &self.previous_entry_sha256,
            artifact_id: self.artifact_id,
            turn_id: self.turn_id,
            candidate_digest: &self.candidate_digest,
            output_digest: &self.output_digest,
            host_callback_digest: &self.host_callback_digest,
            host_seed_receipt_digest: &self.host_seed_receipt_digest,
            authenticity_receipt_digest: &self.authenticity_receipt_digest,
        }
    }
}

struct BoundedWriter {
    bytes: Vec<u8>,
    limit: usize,
    failure: Option<LegacyCodecError>,
}

impl Write for BoundedWriter {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if let Err(error) = self.reserve_for(input.len()) {
            self.failure = Some(error);
            return Err(io::Error::other("bounded legacy encoding failed"));
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl BoundedWriter {
    fn reserve_for(&mut self, additional: usize) -> Result<(), LegacyCodecError> {
        let required = self
            .bytes
            .len()
            .checked_add(additional)
            .filter(|required| *required <= self.limit)
            .ok_or(LegacyCodecError::LimitExceeded)?;
        if required > self.bytes.capacity() {
            // Geometric growth avoids one reallocation per escaped byte. The
            // requested backing capacity never exceeds the caller's byte cap.
            let doubled = self
                .bytes
                .capacity()
                .checked_mul(2)
                .ok_or(LegacyCodecError::LimitExceeded)?;
            let capacity = doubled.max(8).max(required).min(self.limit);
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(|_| LegacyCodecError::AllocationFailed)?;
        }
        Ok(())
    }
}

fn encode_bounded<T: Serialize>(value: &T, limit: usize) -> Result<Vec<u8>, LegacyCodecError> {
    let mut writer = BoundedWriter {
        bytes: Vec::new(),
        limit,
        failure: None,
    };
    if serde_json::to_writer(&mut writer, value).is_err() {
        return Err(writer.failure.unwrap_or(LegacyCodecError::EncodingFailed));
    }
    Ok(writer.bytes)
}
