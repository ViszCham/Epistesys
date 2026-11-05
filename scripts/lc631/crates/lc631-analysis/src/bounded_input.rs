//! Capped analysis input. A resource source revision binds the calling task,
//! not the truth or content hash of the file being inspected.
use lc631_process_supervisor::{ResourceError, ResourceKind, ResourceReservationView};
use lc631_substrate::{
    resource::{ResourceAmount, ResourceClaim, ResourceRequest},
    Digest32,
};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Owns the captured bytes and their borrowed live resource claim together.
pub struct AnalysisText<'a> {
    // Drop owned bytes before their exact local claim, never the whole view quota.
    bytes: Vec<u8>,
    _claim: ResourceClaim<'a>,
}

impl<'a> AnalysisText<'a> {
    pub fn read<R: Read + ?Sized>(
        reader: &mut R,
        resources: &'a ResourceReservationView<'a>,
        task_source: Digest32,
        cap: usize,
    ) -> Result<Self, ResourceError> {
        if resources.source_revision() != task_source {
            return Err(ResourceError::SourceMismatch);
        }
        resources.check()?;
        if cap == 0 {
            return Err(ResourceError::InvalidConfig);
        }
        let amount = u64::try_from(cap).map_err(|_| ResourceError::ArithmeticOverflow)?;
        let claim = resources.claim_resources(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(amount)?,
        )])?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(cap)
            .map_err(|_| ResourceError::AllocationFailed)?;
        if bytes.capacity() > cap {
            return Err(ResourceError::AllocationFailed);
        }
        // Fixed 8KiB stack scratch is not heap payload or an RSS guarantee.
        let mut scratch = [0_u8; 8192];
        loop {
            resources.check()?;
            let remaining = cap
                .checked_sub(bytes.len())
                .ok_or(ResourceError::ArithmeticOverflow)?;
            let length = scratch.len().min(remaining.max(1));
            let count = match reader.read(&mut scratch[..length]) {
                Ok(count) => count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(ResourceError::Io(error.kind())),
            };
            resources.check()?;
            if count > length {
                return Err(ResourceError::Io(io::ErrorKind::InvalidData));
            }
            if count > remaining {
                return Err(ResourceError::Exhausted(ResourceKind::Ram));
            }
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&scratch[..count]);
        }
        // Invalid data is a completed failed read, not an unknown child effect.
        // Drop releases this known buffer; no claim of valid text is returned.
        std::str::from_utf8(&bytes).map_err(|_| ResourceError::Io(io::ErrorKind::InvalidData))?;
        Ok(Self {
            bytes,
            _claim: claim,
        })
    }
    pub fn as_str(&self) -> Result<&str, ResourceError> {
        std::str::from_utf8(&self.bytes).map_err(|_| ResourceError::Io(io::ErrorKind::InvalidData))
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub(crate) enum TextInput<'a> {
    Reserved(AnalysisText<'a>),
    Legacy(String),
}
impl TextInput<'_> {
    pub(crate) fn as_str(&self) -> io::Result<&str> {
        match self {
            Self::Reserved(text) => text.as_str().map_err(resource_io),
            Self::Legacy(text) => Ok(text),
        }
    }
}

/// The compatibility branch still caps actual reads, but reports no common-run
/// accounting. No implicit ResourceBroker/issuer is constructed here.
pub(crate) fn read_text_file<'a>(
    path: &Path,
    cap: usize,
    resources: Option<&'a ResourceReservationView<'a>>,
) -> io::Result<TextInput<'a>> {
    if let Some(view) = resources {
        return read_resource_text_file(path, cap, view)
            .map(TextInput::Reserved)
            .map_err(resource_io);
    }
    legacy_text_file(path, cap).map(TextInput::Legacy)
}

pub(crate) fn read_resource_text_file<'a>(
    path: &Path,
    cap: usize,
    view: &'a ResourceReservationView<'a>,
) -> Result<AnalysisText<'a>, ResourceError> {
    view.check()?;
    let mut file = File::open(path).map_err(|e| ResourceError::Io(e.kind()))?;
    AnalysisText::read(&mut file, view, view.source_revision(), cap)
}

pub(crate) fn legacy_text_file(path: &Path, cap: usize) -> io::Result<String> {
    let file = File::open(path)?;
    if cap == 0 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let sentinel = cap.checked_add(1).ok_or(io::ErrorKind::InvalidInput)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(sentinel)
        .map_err(|_| io::ErrorKind::OutOfMemory)?;
    file.take(u64::try_from(sentinel).map_err(|_| io::ErrorKind::InvalidInput)?)
        .read_to_end(&mut bytes)?;
    if bytes.len() > cap {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "analysis_input_budget_exceeded",
        ));
    }
    String::from_utf8(bytes).map_err(|_| io::ErrorKind::InvalidData.into())
}

/// A fixed stack buffer bounds RAM; the byte cap and live-view checks bound
/// cooperative work. File identity/semantic authenticity remain separate.
pub(crate) fn hash_capped(
    path: &Path,
    cap: u64,
    resources: Option<&ResourceReservationView<'_>>,
) -> io::Result<String> {
    hash_capped_typed(path, cap, resources).map_err(resource_io)
}

pub(crate) fn hash_capped_typed(
    path: &Path,
    cap: u64,
    resources: Option<&ResourceReservationView<'_>>,
) -> Result<String, ResourceError> {
    use sha2::{Digest, Sha256};
    let sentinel = cap
        .checked_add(1)
        .ok_or(ResourceError::ArithmeticOverflow)?;
    let mut file = File::open(path)
        .map_err(|e| ResourceError::Io(e.kind()))?
        .take(sentinel);
    let mut hash = Sha256::new();
    let mut buffer = [0; 8192];
    let mut total = 0_u64;
    loop {
        if let Some(view) = resources {
            view.check()?;
        }
        let count = file
            .read(&mut buffer)
            .map_err(|e| ResourceError::Io(e.kind()))?;
        if let Some(view) = resources {
            view.check()?;
        }
        total = total
            .checked_add(count as u64)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        if total > cap {
            return Err(ResourceError::Exhausted(ResourceKind::Output));
        }
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    let mut encoded = String::from("sha256:");
    for byte in hash.finalize() {
        use std::fmt::Write as _;
        write!(encoded, "{byte:02x}").map_err(|_| ResourceError::AllocationFailed)?;
    }
    Ok(encoded)
}

pub(crate) fn resource_io(error: ResourceError) -> io::Error {
    let kind = match &error {
        ResourceError::Cancelled => io::ErrorKind::Interrupted,
        ResourceError::DeadlineExceeded => io::ErrorKind::TimedOut,
        ResourceError::Io(kind) => *kind,
        ResourceError::AllocationFailed => io::ErrorKind::OutOfMemory,
        _ => io::ErrorKind::InvalidData,
    };
    io::Error::new(kind, format!("analysis_resource:{error:?}"))
}
