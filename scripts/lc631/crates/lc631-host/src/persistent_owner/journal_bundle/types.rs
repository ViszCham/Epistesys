use super::*;

pub(super) const BUNDLE_MAX: usize = 1000;
pub(super) const ROOT_MAX: u64 = 16 * 1024 * 1024;
pub(super) const META_CLAIM: u64 = 1024 * 1024;
pub(super) const SESSION_SCRATCH: u64 = 8 * 1024 * 1024 + 1;
pub(super) const SEGMENT_CAP: usize = 4 * 1024 * 1024;
pub(super) const INDEX_CAP: usize = 786_632;
pub(super) const HEAD_CAP: usize = 16 * 1024;
pub(super) const MARKER: &str = ".journal-bundle-v1";
pub(super) const MARKER_BYTES: &[u8] = b"EJBDL001";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalBundleLimits {
    pub(super) max_bundles: usize,
    pub(super) max_total_bytes: u64,
}
impl JournalBundleLimits {
    pub fn checked(max_bundles: usize, max_total_bytes: u64) -> Result<Self, HostError> {
        if max_bundles == 0
            || max_bundles > BUNDLE_MAX
            || max_total_bytes < MARKER_BYTES.len() as u64
            || max_total_bytes > ROOT_MAX
        {
            return Err(store_error("bundle_limits_invalid"));
        }
        Ok(Self {
            max_bundles,
            max_total_bytes,
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BundlePart {
    Segment,
    Index,
    LocalHead,
}
impl BundlePart {
    pub(super) fn slot(self) -> usize {
        match self {
            Self::Segment => 0,
            Self::Index => 1,
            Self::LocalHead => 2,
        }
    }
    pub(super) fn cap(self) -> usize {
        match self {
            Self::Segment => SEGMENT_CAP,
            Self::Index => INDEX_CAP,
            Self::LocalHead => HEAD_CAP,
        }
    }
    pub(super) fn prefix(self) -> &'static str {
        match self {
            Self::Segment => "segment-",
            Self::Index => "index-",
            Self::LocalHead => "local-head-",
        }
    }
    pub(super) fn name(self, sequence: u64) -> String {
        format!("{}{:020}.bin", self.prefix(), sequence)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalBundlePartDescriptor {
    pub(super) byte_len: u64,
    pub(super) digest: [u8; 71],
}
impl JournalBundlePartDescriptor {
    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }
    /// Constructor checks ASCII; the array is private and thereafter immutable.
    pub fn digest(&self) -> &str {
        std::str::from_utf8(&self.digest).expect("private digest is checked ASCII")
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalBundleDescriptor {
    pub(super) sequence: u64,
    pub(super) parts: [Option<JournalBundlePartDescriptor>; 3],
}
impl JournalBundleDescriptor {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn part(&self, part: BundlePart) -> Option<&JournalBundlePartDescriptor> {
        self.parts[part.slot()].as_ref()
    }
    pub(super) fn complete(&self) -> bool {
        self.parts.iter().all(Option::is_some)
    }
}
pub struct OwnedJournalBundleInventory<'a> {
    pub(super) entries: Vec<JournalBundleDescriptor>,
    pub(super) _claim: ResourceClaim<'a>,
}
impl OwnedJournalBundleInventory<'_> {
    pub fn items(&self) -> &[JournalBundleDescriptor] {
        &self.entries
    }
}
pub struct OwnedJournalBundlePart<'a> {
    pub(super) bytes: Vec<u8>,
    pub(super) _claim: ResourceClaim<'a>,
}
impl OwnedJournalBundlePart<'_> {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JournalBundleWriteError {
    RejectedBeforeWrite(HostError),
    MayHaveApplied(HostError),
}

pub(super) fn descriptor(bytes: &[u8]) -> Result<JournalBundlePartDescriptor, HostError> {
    let byte_len = u64::try_from(bytes.len()).map_err(|_| store_error("bundle_size_overflow"))?;
    let digest = lc631_receipt_kernel::digest_bytes(bytes);
    let raw = digest.as_bytes();
    if raw.len() != 71 || !raw.is_ascii() {
        return Err(store_error("bundle_digest_invalid"));
    }
    let mut fixed = [0_u8; 71];
    fixed.copy_from_slice(raw);
    Ok(JournalBundlePartDescriptor {
        byte_len,
        digest: fixed,
    })
}
pub(super) fn parse_name(name: &str) -> Result<(u64, BundlePart), HostError> {
    for part in [
        BundlePart::Segment,
        BundlePart::Index,
        BundlePart::LocalHead,
    ] {
        if let Some(digits) = name
            .strip_prefix(part.prefix())
            .and_then(|s| s.strip_suffix(".bin"))
        {
            if digits.len() != 20 || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(store_error("bundle_name_invalid"));
            }
            let sequence = digits
                .parse::<u64>()
                .map_err(|_| store_error("bundle_sequence_invalid"))?;
            if sequence == 0 {
                return Err(store_error("bundle_sequence_invalid"));
            }
            return Ok((sequence, part));
        }
    }
    Err(store_error("bundle_unknown_entry"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JournalBundleOpenError {
    BeforeEffect(HostError),
    MayHaveApplied(HostError),
}
