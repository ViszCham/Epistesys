//! Lease-injected cooperative bounded I/O. This does not interrupt blocking Read.
use lc631_substrate::resource::{
    ResourceBufferClaim, ResourceError, ResourceKind, ResourceReservationView,
};
use std::io::{self, Read};

pub struct BoundedBytes<'a> {
    // Field drop order releases the actual Vec BEFORE its live accounting claim.
    bytes: Vec<u8>,
    _claim: ResourceBufferClaim<'a>,
}
impl BoundedBytes<'_> {
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
    pub fn as_utf8(&self) -> Result<&str, ResourceError> {
        std::str::from_utf8(&self.bytes).map_err(|_| ResourceError::Io(io::ErrorKind::InvalidData))
    }
}

pub fn read_bounded_bytes<'a, R: Read + ?Sized>(
    reader: &mut R,
    view: &'a ResourceReservationView<'a>,
    kind: ResourceKind,
) -> Result<BoundedBytes<'a>, ResourceError> {
    let claim = view.claim_buffer(kind)?;
    let limit = usize::try_from(claim.limit()).map_err(|_| ResourceError::ArithmeticOverflow)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(limit)
        .map_err(|_| ResourceError::AllocationFailed)?;
    let mut scratch = [0_u8; 8192];
    loop {
        if let Err(error) = claim.check() {
            claim.record_cleanup_unknown();
            return Err(error);
        }
        let remaining = limit
            .checked_sub(bytes.len())
            .ok_or(ResourceError::ArithmeticOverflow)?;
        // At the exact cap read one stack-only sentinel byte; never allocate a
        // limit+1 payload and never trust a metadata size checked earlier.
        let length = scratch.len().min(remaining.max(1));
        let read = match reader.read(&mut scratch[..length]) {
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                claim.record_cleanup_unknown();
                return Err(ResourceError::Io(error.kind()));
            }
        };
        if let Err(error) = claim.check() {
            claim.record_cleanup_unknown();
            return Err(error);
        }
        if read > length {
            claim.record_cleanup_unknown();
            return Err(ResourceError::Io(io::ErrorKind::InvalidData));
        }
        if read > remaining {
            claim.record_cleanup_unknown();
            return Err(ResourceError::Exhausted(kind));
        }
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&scratch[..read]);
    }
    Ok(BoundedBytes {
        bytes,
        _claim: claim,
    })
}
