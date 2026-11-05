//! Bounded model enumeration/hash work. Hash equality is not model authenticity.
use crate::{format_digest, MediaError};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub struct ModelScanLimits {
    pub max_entries: usize,
    pub max_depth: usize,
    pub max_metadata_bytes: usize,
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
}

impl Default for ModelScanLimits {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_depth: 64,
            max_metadata_bytes: 8 * 1024 * 1024,
            max_file_bytes: 8 * 1024 * 1024 * 1024,
            max_total_bytes: 32 * 1024 * 1024 * 1024,
        }
    }
}

struct ModelScan<'a> {
    limits: ModelScanLimits,
    files: Vec<(String, u64, String)>,
    seen: usize,
    metadata_bytes: usize,
    read_bytes: u64,
    deadline: Instant,
    resources: Option<&'a lc631_process_supervisor::ResourceReservationView<'a>>,
}

pub fn model_cache_manifest_sha256_bounded<'a>(
    root: &Path,
    limits: ModelScanLimits,
    resources: Option<&'a lc631_process_supervisor::ResourceReservationView<'a>>,
) -> Result<String, MediaError> {
    if limits.max_entries == 0
        || limits.max_entries > 100_000
        || limits.max_depth == 0
        || limits.max_depth > 256
        || limits.max_metadata_bytes == 0
        || limits.max_metadata_bytes > 16 * 1024 * 1024
        || limits.max_file_bytes == 0
        || limits.max_total_bytes == 0
    {
        return Err(MediaError::MediaBudgetExceeded);
    }
    let _claim = resources
        .map(|view| {
            if view.amount(lc631_process_supervisor::ResourceKind::Output)?
                < limits.max_metadata_bytes as u64
            {
                return Err(lc631_process_supervisor::ResourceError::Exhausted(
                    lc631_process_supervisor::ResourceKind::Output,
                ));
            }
            let metadata = limits.max_metadata_bytes as u64;
            let ram = metadata
                .checked_add(65_536)
                .ok_or(lc631_process_supervisor::ResourceError::ArithmeticOverflow)?;
            view.claim_resources(&[
                lc631_process_supervisor::ResourceRequest::new(
                    lc631_process_supervisor::ResourceKind::Ram,
                    lc631_process_supervisor::ResourceAmount::checked(ram)?,
                ),
                lc631_process_supervisor::ResourceRequest::new(
                    lc631_process_supervisor::ResourceKind::Output,
                    lc631_process_supervisor::ResourceAmount::checked(metadata)?,
                ),
            ])
        })
        .transpose()
        .map_err(|error| MediaError::Io(format!("model_scan_resource:{error:?}")))?;
    let mut scan = ModelScan {
        limits,
        files: Vec::new(),
        seen: 0,
        metadata_bytes: 0,
        read_bytes: 0,
        deadline: Instant::now()
            .checked_add(Duration::from_secs(1_800))
            .ok_or(MediaError::InvalidTimeout)?,
        resources,
    };
    scan.collect(root, root, 0)?;
    scan.files.sort();
    let mut hash = Sha256::new();
    hash.update(b"lc631-directory-manifest.v3\0");
    // Preserve exact legacy compact tuple order/escaping without building a
    // second giant JSON String or changing the signed digest domain.
    struct HashWriter<'a>(&'a mut Sha256);
    impl std::io::Write for HashWriter<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(HashWriter(&mut hash), &scan.files)
        .map_err(|_| MediaError::BackendDescriptorMismatch)?;
    Ok(format_digest(hash.finalize()))
}

impl ModelScan<'_> {
    fn check(&self) -> Result<(), MediaError> {
        if Instant::now() >= self.deadline {
            return Err(MediaError::BackendTimeout);
        }
        if let Some(view) = self.resources {
            view.check()
                .map_err(|error| MediaError::Io(format!("model_scan_resource:{error:?}")))?;
        }
        Ok(())
    }

    fn collect(&mut self, root: &Path, current: &Path, depth: usize) -> Result<(), MediaError> {
        self.check()?;
        if depth > self.limits.max_depth {
            return Err(MediaError::MediaBudgetExceeded);
        }
        let children =
            std::fs::read_dir(current).map_err(|error| MediaError::Io(error.to_string()))?;
        for child in children {
            self.check()?;
            if self.seen >= self.limits.max_entries {
                return Err(MediaError::MediaBudgetExceeded);
            }
            self.seen += 1;
            let child = child.map_err(|error| MediaError::Io(error.to_string()))?;
            let path = child.path();
            let metadata = std::fs::symlink_metadata(&path)
                .map_err(|error| MediaError::Io(error.to_string()))?;
            if metadata.file_type().is_symlink() || model_reparse(&metadata) {
                return Err(MediaError::BackendDescriptorMismatch);
            }
            if metadata.is_dir() {
                self.collect(root, &path, depth + 1)?;
            } else if metadata.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| MediaError::BackendDescriptorMismatch)?
                    .to_str()
                    .ok_or(MediaError::BackendDescriptorMismatch)?;
                let extra = relative
                    .len()
                    .checked_add(71)
                    .ok_or(MediaError::MediaBudgetExceeded)?;
                self.metadata_bytes = self
                    .metadata_bytes
                    .checked_add(extra)
                    .ok_or(MediaError::MediaBudgetExceeded)?;
                let backing = self
                    .files
                    .len()
                    .checked_add(1)
                    .and_then(|count| {
                        count.checked_mul(std::mem::size_of::<(String, u64, String)>())
                    })
                    .ok_or(MediaError::MediaBudgetExceeded)?;
                if self
                    .metadata_bytes
                    .checked_add(backing)
                    .is_none_or(|bytes| bytes > self.limits.max_metadata_bytes)
                {
                    return Err(MediaError::MediaBudgetExceeded);
                }
                self.files
                    .try_reserve_exact(1)
                    .map_err(|_| MediaError::MediaBudgetExceeded)?;
                let hash = self.hash_file(&path)?;
                self.files
                    .push((relative.replace('\\', "/"), metadata.len(), hash));
            }
        }
        Ok(())
    }

    fn hash_file(&mut self, path: &Path) -> Result<String, MediaError> {
        let mut file = File::open(path).map_err(|error| MediaError::Io(error.to_string()))?;
        let expected = file
            .metadata()
            .map_err(|error| MediaError::Io(error.to_string()))?
            .len();
        if expected > self.limits.max_file_bytes
            || self
                .read_bytes
                .checked_add(expected)
                .is_none_or(|bytes| bytes > self.limits.max_total_bytes)
        {
            return Err(MediaError::MediaBudgetExceeded);
        }
        let mut hash = Sha256::new();
        let mut buffer = [0; 65_536];
        let mut observed = 0u64;
        loop {
            self.check()?;
            let remaining = self
                .limits
                .max_file_bytes
                .checked_sub(observed)
                .ok_or(MediaError::MediaBudgetExceeded)?
                .min(
                    self.limits
                        .max_total_bytes
                        .checked_sub(self.read_bytes)
                        .ok_or(MediaError::MediaBudgetExceeded)?,
                );
            let length = usize::try_from(remaining.min(buffer.len() as u64 - 1) + 1)
                .map_err(|_| MediaError::MediaBudgetExceeded)?;
            let read = file
                .read(&mut buffer[..length])
                .map_err(|error| MediaError::Io(error.to_string()))?;
            if read == 0 {
                break;
            }
            observed = observed
                .checked_add(read as u64)
                .ok_or(MediaError::MediaBudgetExceeded)?;
            self.read_bytes = self
                .read_bytes
                .checked_add(read as u64)
                .ok_or(MediaError::MediaBudgetExceeded)?;
            if observed > expected
                || observed > self.limits.max_file_bytes
                || self.read_bytes > self.limits.max_total_bytes
            {
                return Err(MediaError::MediaBudgetExceeded);
            }
            hash.update(&buffer[..read]);
        }
        if observed != expected {
            return Err(MediaError::SourceSnapshotChanged);
        }
        Ok(format_digest(hash.finalize()))
    }
}

fn model_reparse(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

pub(crate) fn sha256_file_with_resources<'a>(
    path: &Path,
    resources: Option<&'a lc631_process_supervisor::ResourceReservationView<'a>>,
) -> Result<String, MediaError> {
    ModelScan {
        limits: ModelScanLimits::default(),
        files: Vec::new(),
        seen: 0,
        metadata_bytes: 0,
        read_bytes: 0,
        deadline: Instant::now()
            .checked_add(Duration::from_secs(1_800))
            .ok_or(MediaError::InvalidTimeout)?,
        resources,
    }
    .hash_file(path)
}
