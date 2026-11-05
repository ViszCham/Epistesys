//! 明示local fileを既存read-only anchorで読む。作成・更新・実行なし。
//! Explicit local anchored read only; no creation, writes or solver execution.
#![forbid(unsafe_code)]
use super::{claim_config, read_registry_claimed, LoadedRegistry, RegistryConfigError};
use lc631_process_supervisor::filesystem::DirectoryAnchor;
use lc631_runtime::resource::ResourceReservationView;
use std::{
    io,
    path::{Component, Path},
};

const MAX_PATH_BYTES: usize = 4096;
const MAX_COMPONENTS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigFileError {
    InvalidPath,
    File(io::ErrorKind),
}

/// Borrowed explicit local path: validation itself allocates no owned path.
#[derive(Clone, Copy)]
pub struct ConfigFile<'path> {
    path: &'path Path,
}
impl<'path> ConfigFile<'path> {
    pub fn checked(path: &'path Path) -> Result<Self, ConfigFileError> {
        let text = path.to_str().ok_or(ConfigFileError::InvalidPath)?;
        if !path.is_absolute()
            || text.len() > MAX_PATH_BYTES
            || text.contains('\0')
            || text.starts_with("//")
            || text.starts_with(r"\\")
            || path.file_name().is_none()
            || path.components().count() > MAX_COMPONENTS
        {
            return Err(ConfigFileError::InvalidPath);
        }
        for component in path.components() {
            match component {
                Component::RootDir => {}
                Component::Normal(name) => {
                    #[cfg(windows)]
                    if !windows_file_component(name.to_str().ok_or(ConfigFileError::InvalidPath)?) {
                        return Err(ConfigFileError::InvalidPath);
                    }
                    #[cfg(not(windows))]
                    let _ = name;
                }
                #[cfg(windows)]
                Component::Prefix(prefix)
                    if matches!(prefix.kind(), std::path::Prefix::Disk(_)) => {}
                _ => return Err(ConfigFileError::InvalidPath),
            }
        }
        Ok(Self { path })
    }
    pub fn path(self) -> &'path Path {
        self.path
    }
}

// Windows reserved devices and alternate data streams are rejected lexically
// BEFORE opening. A regular-file metadata check after open is insufficient.
#[cfg(windows)]
fn windows_file_component(name: &str) -> bool {
    if name.chars().any(|c| c < ' ' || "<>:\"|?*".contains(c)) || name.ends_with(['.', ' ']) {
        return false;
    }
    let stem = name.split('.').next().unwrap_or("").trim_end_matches(' ');
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"]
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return false;
    }
    if let (Some(prefix), Some(number)) = (stem.get(..3), stem.get(3..)) {
        if (prefix.eq_ignore_ascii_case("COM") || prefix.eq_ignore_ascii_case("LPT"))
            && matches!(
                number,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        {
            return false;
        }
    }
    true
}

/// Claims backing BEFORE opening the existing anchored regular file. The
/// existing platform capsule owns no-follow identity/hardlink checks. Neither
/// O_NONBLOCK nor cooperative checks establish a hard filesystem wall timeout.
pub fn read_registry_file<'view>(
    request: ConfigFile<'_>,
    view: &'view ResourceReservationView<'_>,
) -> Result<LoadedRegistry<'view>, RegistryFileError> {
    let claim = claim_config(view).map_err(RegistryFileError::Registry)?;
    view.check()
        .map_err(|e| RegistryFileError::Registry(RegistryConfigError::Resource(e)))?;
    let parent = request
        .path
        .parent()
        .ok_or(RegistryFileError::Path(ConfigFileError::InvalidPath))?;
    let name = request
        .path
        .file_name()
        .ok_or(RegistryFileError::Path(ConfigFileError::InvalidPath))?;
    let anchor = DirectoryAnchor::open_existing(parent).map_err(file_error)?;
    let mut file = anchor.open_read_only(Path::new(name)).map_err(file_error)?;
    if !file.metadata().map_err(file_error)?.is_file() {
        return Err(RegistryFileError::Path(ConfigFileError::InvalidPath));
    }
    let loaded =
        read_registry_claimed(&mut file, view, claim).map_err(RegistryFileError::Registry)?;
    file.validate_current().map_err(file_error)?;
    anchor.validate_current().map_err(file_error)?;
    view.check()
        .map_err(|e| RegistryFileError::Registry(RegistryConfigError::Resource(e)))?;
    Ok(loaded)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryFileError {
    Path(ConfigFileError),
    Registry(RegistryConfigError),
}
fn file_error(error: io::Error) -> RegistryFileError {
    RegistryFileError::Path(ConfigFileError::File(error.kind()))
}
