//! Exact bounded legacy text boundaries. This module never creates a broker.
use lc631_process_supervisor::{
    filesystem::{DirectoryAnchor, ReadOnlyAnchoredFile},
    read_bounded_bytes, BoundedBytes, ResourceError, ResourceKind, ResourceReservationView,
};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

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

pub(crate) fn checked_absolute(path: &Path) -> io::Result<PathBuf> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "host_path_invalid",
        ));
    }
    std::path::absolute(path)
}

pub(crate) fn open_or_create_root(path: &Path) -> io::Result<DirectoryAnchor> {
    let absolute = checked_absolute(path)?;
    let mut last_missing = None;
    // The root itself plus at most four missing parent components.
    for candidate in absolute.ancestors().take(5) {
        match DirectoryAnchor::open_existing(candidate) {
            Ok(anchor) => {
                anchor.validate_current()?;
                let suffix = absolute.strip_prefix(candidate).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "host_root_scope_invalid")
                })?;
                if suffix.as_os_str().is_empty() {
                    return Ok(anchor);
                }
                let root = anchor.directory(suffix, true)?;
                root.validate_current()?;
                return Ok(root);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                last_missing = Some(error);
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_missing
        .unwrap_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "host_root_parent_missing")))
}

fn open_read_only(path: &Path) -> Result<ReadOnlyAnchoredFile, crate::HostError> {
    let path = checked_absolute(path).map_err(|error| crate::HostError::Io(error.to_string()))?;
    let parent = path
        .parent()
        .ok_or_else(|| crate::HostError::Io("host_path_parent_missing".into()))?;
    let name = path
        .file_name()
        .ok_or_else(|| crate::HostError::Io("host_path_name_missing".into()))?;
    DirectoryAnchor::open_existing(parent)
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
fn legacy_bytes(file: &mut ReadOnlyAnchoredFile, limit: usize) -> io::Result<Vec<u8>> {
    let sentinel = limit.checked_add(1).ok_or(io::ErrorKind::InvalidInput)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(sentinel)
        .map_err(|_| io::ErrorKind::OutOfMemory)?;
    file.take(sentinel as u64).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "host_input_budget_exceeded",
        ));
    }
    Ok(bytes)
}

pub(crate) fn with_file_text<T>(
    mut file: ReadOnlyAnchoredFile,
    limit: usize,
    view: Option<&ResourceReservationView<'_>>,
    consume: impl FnOnce(&str) -> Result<T, crate::HostError>,
) -> Result<T, crate::HostError> {
    require_capacity(view, limit as u64)?;
    file.validate_current()
        .map_err(|error| crate::HostError::Io(error.to_string()))?;
    let Some(view) = view else {
        let bytes = legacy_bytes(&mut file, limit)
            .map_err(|error| crate::HostError::Io(error.to_string()))?;
        let text = std::str::from_utf8(&bytes).map_err(|_| {
            crate::HostError::Io(io::Error::from(io::ErrorKind::InvalidData).to_string())
        })?;
        file.validate_current()
            .map_err(|error| crate::HostError::Io(error.to_string()))?;
        let result = consume(text);
        file.validate_current()
            .map_err(|error| crate::HostError::Io(error.to_string()))?;
        return result;
    };
    view.check().map_err(resource_error)?;
    let sentinel = limit
        .checked_add(1)
        .ok_or_else(|| crate::HostError::Io("host_input_budget_overflow".into()))?;
    let bytes = {
        let mut reader = (&mut file).take(sentinel as u64);
        read_text(&mut reader, view, ResourceKind::Output).map_err(resource_error)?
    };
    if bytes.as_slice().len() > limit {
        view.record_cleanup_unknown();
        return Err(crate::HostError::Io("host_input_budget_exceeded".into()));
    }
    let text = bytes.as_utf8().map_err(resource_error)?;
    file.validate_current()
        .map_err(|error| crate::HostError::Io(error.to_string()))?;
    view.check().map_err(resource_error)?;
    // The same handle and live buffer claim remain held through consumption
    // and the final identity/reservation checks.
    let result = consume(text);
    file.validate_current()
        .map_err(|error| crate::HostError::Io(error.to_string()))?;
    view.check().map_err(resource_error)?;
    result
}

pub(crate) fn with_path_text<T>(
    path: &Path,
    limit: usize,
    view: Option<&ResourceReservationView<'_>>,
    consume: impl FnOnce(&str) -> Result<T, crate::HostError>,
) -> Result<T, crate::HostError> {
    require_capacity(view, limit as u64)?;
    with_file_text(open_read_only(path)?, limit, view, consume)
}
