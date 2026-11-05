//! Exact bounded legacy text boundaries. This module never creates a broker.
use lc631_process_supervisor::{
    read_bounded_bytes, BoundedBytes, ResourceError, ResourceKind, ResourceReservationView,
};
use std::io::{self, Read};
use std::path::Path;

pub(crate) fn require_capacity(
    resources: Option<&ResourceReservationView<'_>>,
    minimum: u64,
) -> Result<(), crate::HostError> {
    if let Some(view) = resources {
        for kind in [ResourceKind::Ram, ResourceKind::Output] {
            if view.amount(kind).map_err(resource_error)? < minimum {
                return Err(resource_error(ResourceError::Exhausted(kind)));
            }
        }
    }
    Ok(())
}

fn resource_error(error: ResourceError) -> crate::HostError {
    crate::HostError::Io(format!("host_resource:{error:?}"))
}

pub(crate) fn require_disk(
    view: Option<&ResourceReservationView<'_>>,
    bytes: u64,
) -> Result<(), crate::HostError> {
    if let Some(view) = view {
        if view.amount(ResourceKind::Disk).map_err(resource_error)? < bytes {
            return Err(resource_error(ResourceError::Exhausted(ResourceKind::Disk)));
        }
    }
    Ok(())
}

fn open_read_only(
    path: &Path,
) -> Result<lc631_process_supervisor::filesystem::ReadOnlyAnchoredFile, crate::HostError> {
    let path =
        std::path::absolute(path).map_err(|error| crate::HostError::Io(error.to_string()))?;
    let parent = path
        .parent()
        .ok_or_else(|| crate::HostError::Io("host_path_parent_missing".into()))?;
    let name = path
        .file_name()
        .ok_or_else(|| crate::HostError::Io("host_path_name_missing".into()))?;
    lc631_process_supervisor::filesystem::DirectoryAnchor::open_existing(parent)
        .and_then(|anchor| anchor.open_read_only(Path::new(name)))
        .map_err(|error| crate::HostError::Io(error.to_string()))
}

pub fn read_text<'a, R: Read + ?Sized>(
    reader: &mut R,
    view: &'a ResourceReservationView<'a>,
    kind: ResourceKind,
) -> Result<BoundedBytes<'a>, ResourceError> {
    let bytes = read_bounded_bytes(reader, view, kind)?;
    if let Err(error) = bytes.as_utf8() {
        view.record_cleanup_unknown();
        return Err(error);
    }
    Ok(bytes)
}

/// Compatibility-only bounded read when old Rust callers supply no runtime
/// owner. It is not evidence of common-run accounting or deadline enforcement.
pub(crate) fn legacy_text(path: &Path, limit: usize) -> io::Result<String> {
    String::from_utf8(legacy_bytes(path, limit)?).map_err(|_| io::ErrorKind::InvalidData.into())
}

fn legacy_bytes(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    let sentinel = limit.checked_add(1).ok_or(io::ErrorKind::InvalidInput)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(sentinel)
        .map_err(|_| io::ErrorKind::OutOfMemory)?;
    (&mut file).take(sentinel as u64).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "host_input_budget_exceeded",
        ));
    }
    Ok(bytes)
}

pub(crate) fn path_matches(
    path: &Path,
    expected: &[u8],
    limit: usize,
    view: Option<&ResourceReservationView<'_>>,
) -> Result<bool, crate::HostError> {
    require_capacity(view, limit as u64)?;
    let Some(view) = view else {
        return legacy_bytes(path, limit)
            .map(|bytes| bytes == expected)
            .map_err(|error| crate::HostError::Io(error.to_string()));
    };
    let file = open_read_only(path)?;
    let sentinel = limit
        .checked_add(1)
        .ok_or_else(|| crate::HostError::Io("host_input_budget_overflow".into()))?;
    let bytes = read_bounded_bytes(&mut file.take(sentinel as u64), view, ResourceKind::Output)
        .map_err(|error| crate::HostError::Io(format!("host_resource:{error:?}")))?;
    if bytes.as_slice().len() > limit {
        view.record_cleanup_unknown();
        return Err(crate::HostError::Io("host_input_budget_exceeded".into()));
    }
    Ok(bytes.as_slice() == expected)
}

pub(crate) fn with_path_text<T>(
    path: &Path,
    limit: usize,
    view: Option<&ResourceReservationView<'_>>,
    consume: impl FnOnce(&str) -> Result<T, crate::HostError>,
) -> Result<T, crate::HostError> {
    require_capacity(view, limit as u64)?;
    let Some(view) = view else {
        let text =
            legacy_text(path, limit).map_err(|error| crate::HostError::Io(error.to_string()))?;
        return consume(&text);
    };
    let file = open_read_only(path)?;
    let mut reader = file.take(
        limit
            .checked_add(1)
            .ok_or_else(|| crate::HostError::Io("host_input_budget_overflow".into()))?
            as u64,
    );
    let bytes = read_text(&mut reader, view, ResourceKind::Output)
        .map_err(|error| crate::HostError::Io(format!("host_resource:{error:?}")))?;
    if bytes.as_slice().len() > limit {
        view.record_cleanup_unknown();
        return Err(crate::HostError::Io("host_input_budget_exceeded".into()));
    }
    let text = bytes
        .as_utf8()
        .map_err(|error| crate::HostError::Io(format!("host_resource:{error:?}")))?;
    // The live buffer/claim stays borrowed through the consumer; no second
    // Vec/String allocation escapes this reader adapter.
    consume(text)
}
