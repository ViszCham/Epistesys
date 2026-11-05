//! Byte-authenticated private inputs for trusted local media backends.
//! Same-owner/privileged code is not an OS sandbox boundary on Unix.
use lc631_substrate::resource::ResourceReservationView;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

mod resources;
pub use resources::ReservedSourceSnapshot;

#[cfg(unix)]
#[path = "media_snapshot/unix.rs"]
mod platform;
#[cfg(windows)]
#[path = "media_snapshot/windows.rs"]
mod platform;

const MAX_SOURCE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ACTIVE_SNAPSHOTS: usize = 8;
const MAX_TOTAL_SNAPSHOT_BYTES: u64 = 8 * 1024 * 1024 * 1024;
static RESERVATIONS: Mutex<(usize, u64)> = Mutex::new((0, 0));
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity(u64, u64);

struct Reservation {
    bytes: u64,
    retained: bool,
}
impl Reservation {
    fn acquire(bytes: u64) -> io::Result<Self> {
        let mut state = RESERVATIONS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let total = state.1.checked_add(bytes).ok_or_else(budget)?;
        if state.0 >= MAX_ACTIVE_SNAPSHOTS || total > MAX_TOTAL_SNAPSHOT_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "media_snapshot_capacity_exceeded",
            ));
        }
        state.0 += 1;
        state.1 = total;
        Ok(Self {
            bytes,
            retained: false,
        })
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        if self.retained {
            return;
        }
        let mut state = RESERVATIONS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.0 -= 1;
        state.1 -= self.bytes;
    }
}

pub struct AuthenticatedSourceSnapshot {
    directory: PathBuf,
    path: PathBuf,
    directory_identity: Identity,
    file_identity: Option<Identity>,
    directory_handle: Option<File>,
    file_handle: Option<File>,
    bytes: u64,
    cleaned: bool,
    reservation: Option<Reservation>,
}

impl AuthenticatedSourceSnapshot {
    /// Streams at most 64KiB per read and never opens the original for writing.
    /// The caller reserves returned bytes from its decoder temporary budget.
    pub fn capture(
        source: &Path,
        expected_sha256: &str,
        max_bytes: u64,
        temporary_budget: u64,
    ) -> io::Result<Self> {
        Self::capture_inner(
            source,
            expected_sha256,
            max_bytes,
            temporary_budget,
            true,
            None,
        )
    }

    fn capture_inner(
        source: &Path,
        expected_sha256: &str,
        max_bytes: u64,
        temporary_budget: u64,
        legacy: bool,
        control: Option<&ResourceReservationView<'_>>,
    ) -> io::Result<Self> {
        if expected_sha256.len() != 71
            || !expected_sha256.starts_with("sha256:")
            || !expected_sha256[7..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || max_bytes == 0
            || temporary_budget == 0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid_media_snapshot_contract",
            ));
        }
        let mut input = platform::open_source(source)?;
        let metadata = input.metadata()?;
        let limit = max_bytes.min(temporary_budget).min(MAX_SOURCE_BYTES);
        if !metadata.is_file() || metadata.len() > limit {
            return Err(budget());
        }
        let expected_length = metadata.len();
        let mut reservation = if legacy {
            Some(Reservation::acquire(expected_length)?)
        } else {
            None
        };
        let directory = create_directory()?;
        let setup = platform::open_directory(&directory)
            .and_then(|handle| platform::identity(&handle).map(|identity| (handle, identity)));
        let (directory_handle, directory_identity) = setup.map_err(|cause| {
            if let Some(reservation) = &mut reservation {
                reservation.retained = true;
            }
            io::Error::other(CaptureFailure {
                cause,
                cleanup: Some(io::Error::other("created_directory_cleanup_unobserved")),
            })
        })?;
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= 16
                    && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
            })
            .unwrap_or("bin");
        let path = directory.join(format!("source.{extension}"));
        let mut snapshot = Self {
            directory,
            path,
            directory_identity,
            file_identity: None,
            directory_handle: Some(directory_handle),
            file_handle: None,
            bytes: 0,
            cleaned: false,
            reservation,
        };
        let result = snapshot.capture_bytes(&mut input, expected_sha256, expected_length, control);
        if let Err(error) = result {
            if let Err(cleanup) = snapshot.cleanup() {
                return Err(io::Error::other(CaptureFailure {
                    cause: error,
                    cleanup: Some(cleanup),
                }));
            }
            return Err(io::Error::new(
                error.kind(),
                CaptureFailure {
                    cause: error,
                    cleanup: None,
                },
            ));
        }
        Ok(snapshot)
    }

    fn capture_bytes(
        &mut self,
        input: &mut File,
        expected: &str,
        limit: u64,
        control: Option<&ResourceReservationView<'_>>,
    ) -> io::Result<()> {
        let mut output = platform::create_file(&self.path)?;
        self.file_identity = Some(platform::identity(&output)?);
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            check_control(control)?;
            let count = input.read(&mut buffer)?;
            check_control(control)?;
            if count == 0 {
                break;
            }
            self.bytes = self.bytes.checked_add(count as u64).ok_or_else(budget)?;
            if self.bytes > limit {
                return Err(budget());
            }
            output.write_all(&buffer[..count])?;
            hasher.update(&buffer[..count]);
        }
        if self.bytes != limit {
            return Err(budget());
        }
        let digest = format!(
            "sha256:{}",
            hasher
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        if digest != expected.to_ascii_lowercase() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "media_snapshot_digest_mismatch",
            ));
        }
        output.sync_all()?;
        platform::seal(&output)?;
        drop(output);
        let reader = platform::open_snapshot(&self.path)?;
        if Some(platform::identity(&reader)?) != self.file_identity {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "media_snapshot_identity_changed",
            ));
        }
        self.file_handle = Some(reader);
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Success requires observed cleanup. A cleanup failure retains its quota
    /// until process restart rather than allowing unbounded abandoned snapshots.
    pub fn close(mut self) -> io::Result<()> {
        self.cleanup()
    }

    fn cleanup(&mut self) -> io::Result<()> {
        if self.cleaned {
            return Ok(());
        }
        self.file_handle.take();
        self.directory_handle.take();
        if let Some(identity) = self.file_identity {
            platform::remove_owned(&self.path, identity, false)?;
            self.file_identity = None;
        }
        platform::remove_owned(&self.directory, self.directory_identity, true)?;
        self.cleaned = true;
        Ok(())
    }
}

impl Drop for AuthenticatedSourceSnapshot {
    fn drop(&mut self) {
        if self.cleanup().is_err() {
            if let Some(reservation) = &mut self.reservation {
                reservation.retained = true;
            }
        }
    }
}

#[derive(Debug)]
struct CaptureFailure {
    cause: io::Error,
    cleanup: Option<io::Error>,
}
impl std::fmt::Display for CaptureFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.cleanup {
            Some(cleanup) => write!(
                formatter,
                "media_snapshot_capture:{}; cleanup:{cleanup}",
                self.cause
            ),
            None => write!(formatter, "{}", self.cause),
        }
    }
}
impl std::error::Error for CaptureFailure {}

fn check_control(control: Option<&ResourceReservationView<'_>>) -> io::Result<()> {
    if let Some(control) = control {
        control
            .check()
            .map_err(|error| io::Error::other(format!("resource_scope:{error:?}")))?;
    }
    Ok(())
}

fn create_directory() -> io::Result<PathBuf> {
    let parent = std::env::temp_dir().canonicalize()?;
    for _ in 0..16 {
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let path = parent.join(format!(
            "epistesys-media-snapshot-{}-{time}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match platform::create_directory(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "media_snapshot_name_exhausted",
    ))
}

fn budget() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "media_snapshot_byte_budget_exceeded",
    )
}
