//! Bounded path-content hashing for local solver registration setup.
use super::resources::map_prestart_resource_error;
use crate::runner::ProcessFailure;
use lc631_process_supervisor::filesystem::DirectoryAnchor;
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceKind, ResourceRequest, ResourceReservationView,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;

const HASH_BUFFER_BYTES: usize = 64 * 1024;
const MAX_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
#[cfg(windows)]
const WINDOWS_REPARSE_POINT: u32 = 0x0400;

/// Hash the registered executable as bounded input.
///
/// The result is a path-content observation, not an executable handle or a
/// trusted attestation. The supervisor rechecks immediately before launch,
/// but path replacement between the hash and spawn remains possible. The
/// binary-only read capsule permits hardlinks without relaxing security files.
pub fn hash_registered_executable(
    reservation: &ResourceReservationView<'_>,
    path: &Path,
) -> Result<[u8; 32], ProcessFailure> {
    let _hash_buffer_claim = reserve_hash_buffer(reservation)?;
    hash_file_contents(reservation, path)
}

pub(super) fn reserve_hash_buffer<'view>(
    reservation: &'view ResourceReservationView<'_>,
) -> Result<ResourceClaim<'view>, ProcessFailure> {
    let bytes = u64::try_from(HASH_BUFFER_BYTES)
        .map_err(|_| ProcessFailure::ResourceExhaustedBeforeStart)?;
    reservation
        .claim_resources(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(bytes).map_err(map_prestart_resource_error)?,
        )])
        .map_err(map_prestart_resource_error)
}

pub(super) fn hash_file_contents(
    reservation: &ResourceReservationView<'_>,
    path: &Path,
) -> Result<[u8; 32], ProcessFailure> {
    reservation.check().map_err(map_prestart_resource_error)?;
    if !path.is_absolute() {
        return Err(ProcessFailure::NotStarted);
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| ProcessFailure::NotStarted)?;
    if !metadata.file_type().is_file() || is_reparse_point(&metadata) {
        return Err(ProcessFailure::NotStarted);
    }
    if metadata.len() > MAX_EXECUTABLE_BYTES {
        return Err(ProcessFailure::ResourceExhaustedBeforeStart);
    }

    #[cfg(all(test, target_os = "linux"))]
    tests::after_regular_metadata(path);

    let parent = path.parent().ok_or(ProcessFailure::NotStarted)?;
    let name = path.file_name().ok_or(ProcessFailure::NotStarted)?;
    let directory =
        DirectoryAnchor::open_existing(parent).map_err(|_| ProcessFailure::NotStarted)?;
    let mut file = directory
        .open_binary_read_only(Path::new(name))
        .map_err(|_| ProcessFailure::NotStarted)?;
    let opened_len = file
        .metadata()
        .map_err(|_| ProcessFailure::NotStarted)?
        .len();
    if opened_len != metadata.len() || opened_len > MAX_EXECUTABLE_BYTES {
        return Err(ProcessFailure::NotStarted);
    }

    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    loop {
        reservation.check().map_err(map_prestart_resource_error)?;
        let read = file
            .read(&mut buffer)
            .map_err(|_| ProcessFailure::NotStarted)?;
        reservation.check().map_err(map_prestart_resource_error)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).map_err(|_| ProcessFailure::NotStarted)?)
            .ok_or(ProcessFailure::ResourceExhaustedBeforeStart)?;
        if total > MAX_EXECUTABLE_BYTES {
            return Err(ProcessFailure::ResourceExhaustedBeforeStart);
        }
        hasher.update(&buffer[..read]);
    }
    if total != opened_len {
        return Err(ProcessFailure::NotStarted);
    }
    file.validate_current()
        .map_err(|_| ProcessFailure::NotStarted)?;
    if file
        .metadata()
        .map_err(|_| ProcessFailure::NotStarted)?
        .len()
        != opened_len
    {
        return Err(ProcessFailure::NotStarted);
    }
    Ok(hasher.finalize().into())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & WINDOWS_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use lc631_substrate::resource::{
        Cancellation, Deadline, ReportedResourceLease, ResourceLease, ResourceLeaseIssuer,
    };
    use std::cell::Cell;
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    const RACE_ENV: &str = "EPISTESYS_TLCX10_HASH_FIFO_WORKER_ROOT";
    const WORKER: &str = "supervisor_adapter::executable::tests::tlcx10_hash_fifo_worker";
    const LEAF: &str = "candidate";
    const HOOK_HIT: &str = "hook-hit.txt";
    const RESULT: &str = "result.txt";

    std::thread_local! {
        static SWAP_AFTER_METADATA: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn after_regular_metadata(path: &Path) {
        if !SWAP_AFTER_METADATA.with(|flag| flag.replace(false)) {
            return;
        }
        let before = fs::symlink_metadata(path).unwrap();
        assert!(before.file_type().is_file());
        assert_eq!(before.nlink(), 1);
        fs::remove_file(path).unwrap();
        let status = Command::new("/usr/bin/mkfifo")
            .arg("-m")
            .arg("600")
            .arg(path)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(fs::symlink_metadata(path).unwrap().file_type().is_fifo());
        // This marker proves metadata accepted a regular file and replacement
        // completed. Initial FIFO rejection is not the tested negative.
        fs::write(path.parent().unwrap().join(HOOK_HIT), b"regular-to-fifo").unwrap();
    }

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "epistesys-tlcx10-hash-fifo-{}-{stamp}",
                std::process::id()
            ));
            fs::create_dir(&root).unwrap();
            Self(fs::canonicalize(root).unwrap())
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            for name in [LEAF, HOOK_HIT, RESULT] {
                let path = self.0.join(name);
                match fs::symlink_metadata(&path) {
                    Ok(metadata)
                        if (metadata.file_type().is_file() || metadata.file_type().is_fifo())
                            && metadata.nlink() == 1 =>
                    {
                        if let Err(error) = fs::remove_file(path) {
                            eprintln!("owned hash fixture retained: {error}");
                            return;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    _ => {
                        eprintln!("unexpected hash fixture object retained");
                        return;
                    }
                }
            }
            if let Err(error) = fs::remove_dir(&self.0) {
                eprintln!("hash fixture directory retained: {error}");
            }
        }
    }

    fn owner() -> (ResourceLeaseIssuer, ResourceLease) {
        let issuer = ResourceLeaseIssuer::new().unwrap();
        let report = ReportedResourceLease::checked(
            1,
            lc631_substrate::Digest32::from_bytes([17; 32]),
            &[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(1024 * 1024).unwrap(),
            )],
            Deadline::after(Duration::from_secs(2)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
        let lease = issuer.issue_admitted(1, report).unwrap();
        (issuer, lease)
    }

    #[test]
    #[ignore = "worker subprocess selected only by the bounded race test"]
    fn tlcx10_hash_fifo_worker() {
        let root = PathBuf::from(std::env::var_os(RACE_ENV).expect("race worker root"));
        assert!(root.is_absolute());
        let path = root.join(LEAF);
        fs::write(&path, b"regular fixture").unwrap();
        let (_issuer, lease) = owner();
        {
            let view = lease.view();
            SWAP_AFTER_METADATA.with(|flag| flag.set(true));
            let result = hash_registered_executable(&view, &path);
            assert_eq!(result, Err(ProcessFailure::NotStarted));
            assert_eq!(fs::read(root.join(HOOK_HIT)).unwrap(), b"regular-to-fifo");
            fs::write(root.join(RESULT), b"rejected-before-read").unwrap();
        }
        lease.close_confirmed().unwrap();
    }

    #[test]
    fn tlcx10_executable_hash_rejects_regular_to_fifo_swap_without_blocking() {
        let scratch = Scratch::new();
        let mut worker = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", WORKER, "--ignored", "--nocapture"])
            .env(RACE_ENV, &scratch.0)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let (status, timed_out) = loop {
            match worker.try_wait().unwrap() {
                Some(status) => break (status, false),
                None if Instant::now() >= deadline => {
                    worker.kill().unwrap();
                    break (worker.wait().unwrap(), true);
                }
                None => std::thread::sleep(Duration::from_millis(5)),
            }
        };
        let hit = fs::read(scratch.0.join(HOOK_HIT)).ok();
        assert_eq!(
            hit.as_deref(),
            Some(b"regular-to-fifo".as_slice()),
            "race hook not reached"
        );
        let result = fs::read(scratch.0.join(RESULT)).ok();
        assert!(
            !timed_out && status.success()
                && result.as_deref() == Some(b"rejected-before-read".as_slice()),
            "regular-to-FIFO race reached; watchdog_timed_out={timed_out}, status={status}, result={result:?}"
        );
    }
}
