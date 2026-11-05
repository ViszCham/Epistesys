use super::{ArtifactKind, ArtifactPin, GateError, ReleaseDigest};
use sha2::{Digest, Sha256};
use std::io::{ErrorKind, Read};

pub const MAX_ARTIFACT_BYTES: u64 = 256 * 1024 * 1024;
// Counts every returning read, including interruption and the EOF sentinel.
// A full-cap stream using the 8 KiB scratch needs only 32,769 calls.
const MAX_ARTIFACT_READ_CALLS: usize = 65_536;

/// Content pins are not authentication, package coverage or version promotion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateIdentity {
    pins: [ArtifactPin; 5],
    digest: ReleaseDigest,
}
impl CandidateIdentity {
    pub fn checked(mut pins: [ArtifactPin; 5]) -> Result<Self, GateError> {
        pins.sort_unstable_by_key(|pin| pin.kind);
        for (index, kind) in ArtifactKind::ALL.into_iter().enumerate() {
            if pins[index].kind != kind {
                return Err(GateError::DuplicateArtifact);
            }
        }
        let mut hash = Sha256::new();
        hash.update(b"epistesys.tlcx42.five-artifact-candidate.v1\0");
        for pin in pins {
            hash.update([pin.kind as u8]);
            hash.update(pin.digest.bytes());
        }
        Ok(Self {
            pins,
            digest: ReleaseDigest::from_bytes(hash.finalize().into()),
        })
    }
    pub const fn digest(&self) -> ReleaseDigest {
        self.digest
    }
    pub const fn source_snapshot(&self) -> ReleaseDigest {
        self.pins[0].digest
    }
    pub fn pins(&self) -> &[ArtifactPin; 5] {
        &self.pins
    }
}

/// Only this bounded reader creates observations. A caller's Read supplies the
/// actual bytes, not path-root authenticity, filesystem atomicity or provenance.
/// No Deserialize/from-reported-digest constructor exists.
/// ```compile_fail
/// use lc631_compat::tlcx_release_gate::ObservedArtifact;
/// fn require_deserialize<T: serde::de::DeserializeOwned>() {}
/// require_deserialize::<ObservedArtifact>();
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedArtifact {
    kind: ArtifactKind,
    digest: ReleaseDigest,
    length: u64,
}
impl ObservedArtifact {
    pub fn read(
        kind: ArtifactKind,
        reader: &mut (impl Read + ?Sized),
        cap: u64,
    ) -> Result<Self, GateError> {
        if cap == 0 || cap > MAX_ARTIFACT_BYTES {
            return Err(GateError::InvalidReadLimit);
        }
        let mut hash = Sha256::new();
        let mut length = 0_u64;
        let mut interrupted = 0_u16;
        let mut buffer = [0_u8; 8192];
        for _ in 0..MAX_ARTIFACT_READ_CALLS {
            let remaining = cap.checked_sub(length).ok_or(GateError::ReadLimit)?;
            let requested = usize::try_from(remaining.min(buffer.len() as u64).max(1))
                .map_err(|_| GateError::ReadLimit)?;
            let count = match reader.read(&mut buffer[..requested]) {
                Ok(n) => n,
                Err(e) if e.kind() == ErrorKind::Interrupted => {
                    interrupted = interrupted
                        .checked_add(1)
                        .ok_or(GateError::InterruptedLimit)?;
                    if interrupted > 1024 {
                        return Err(GateError::InterruptedLimit);
                    }
                    continue;
                }
                Err(e) => return Err(GateError::ReaderFailed(e.kind())),
            };
            if count > requested || count as u64 > remaining {
                return Err(GateError::ReadLimit);
            }
            if count == 0 {
                return Ok(Self {
                    kind,
                    digest: ReleaseDigest::from_bytes(hash.finalize().into()),
                    length,
                });
            }
            hash.update(&buffer[..count]);
            length = length
                .checked_add(count as u64)
                .ok_or(GateError::ReadLimit)?;
        }
        Err(GateError::ReadCallLimit)
    }
    pub const fn kind(&self) -> ArtifactKind {
        self.kind
    }
    pub const fn digest(&self) -> ReleaseDigest {
        self.digest
    }
    pub const fn length(&self) -> u64 {
        self.length
    }
}
