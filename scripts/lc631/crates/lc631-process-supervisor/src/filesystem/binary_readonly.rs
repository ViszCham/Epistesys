//! Read-only binary content observation, explicitly permitting hardlink aliases.
//! Security-file unique-link policy remains in the separate existing capsule.
use super::{platform, unsafe_path, DirectoryAnchor, FileIdentity};
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read};

/// A named regular binary reader, not an authenticated executable, snapshot,
/// mutation permit or spawn handle. Parent/leaf identity is rechecked around
/// streaming reads; arbitrary OS I/O has no universal wall-clock guarantee.
///
/// ```compile_fail
/// use lc631_process_supervisor::filesystem::BinaryReadOnlyAnchoredFile;
/// fn write(mut file: BinaryReadOnlyAnchoredFile) { file.write_and_sync(b"x").unwrap(); }
/// ```
/// ```compile_fail
/// use lc631_process_supervisor::filesystem::{BinaryReadOnlyAnchoredFile, ReadOnlyAnchoredFile};
/// fn promote(file: BinaryReadOnlyAnchoredFile) -> ReadOnlyAnchoredFile { file }
/// ```
pub struct BinaryReadOnlyAnchoredFile {
    file: File,
    parent: DirectoryAnchor,
    name: OsString,
    identity: FileIdentity,
}

impl BinaryReadOnlyAnchoredFile {
    pub(super) fn from_parent(parent: DirectoryAnchor, name: OsString) -> io::Result<Self> {
        parent.validate_current()?;
        let file = platform::open_binary_file(&parent, &name)?;
        let identity = platform::binary_identity(&file)?;
        let reader = Self {
            file,
            parent,
            name,
            identity,
        };
        reader.validate_current()?;
        Ok(reader)
    }

    pub fn metadata(&self) -> io::Result<std::fs::Metadata> {
        self.validate_current()?;
        self.file.metadata()
    }

    pub fn validate_current(&self) -> io::Result<()> {
        self.parent.validate_current()?;
        if platform::binary_identity(&self.file)? != self.identity {
            return Err(unsafe_path());
        }
        // Reopen through the same no-follow/nonblocking regular-file policy.
        // Aliases are permitted; replacing the named inode is not.
        let current = platform::open_binary_file(&self.parent, &self.name)?;
        if platform::binary_identity(&current)? != self.identity {
            return Err(unsafe_path());
        }
        Ok(())
    }
}

impl Read for BinaryReadOnlyAnchoredFile {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.validate_current()?;
        self.file.read(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::super::DirectoryAnchor;
    use std::fs;
    use std::io::Read;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "epistesys-tlcx10-binary-read-{}-{stamp}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            // Exact fixture-owned entries only; unlink symlinks/FIFOs without
            // traversing them. Never recursively remove the directory.
            for name in [
                "candidate",
                "alias",
                "old",
                "link",
                "fifo",
                "ready",
                "result",
            ] {
                let path = self.0.join(name);
                match fs::symlink_metadata(&path) {
                    Ok(_) => {
                        if let Err(error) = fs::remove_file(path) {
                            eprintln!("binary-read fixture retained: {error}");
                            return;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        eprintln!("binary-read fixture inspection failed: {error}");
                        return;
                    }
                }
            }
            if let Err(error) = fs::remove_dir(&self.0) {
                eprintln!("binary-read fixture directory retained: {error}");
            }
        }
    }

    #[test]
    fn tlcx10_binary_hardlink_read_does_not_relax_unique_security_file_policy() {
        let scratch = Scratch::new();
        fs::write(scratch.0.join("candidate"), b"binary fixture").unwrap();
        fs::hard_link(scratch.0.join("candidate"), scratch.0.join("alias")).unwrap();
        let anchor = DirectoryAnchor::open_existing(&scratch.0).unwrap();
        assert!(anchor.open_read_only(Path::new("candidate")).is_err());
        assert!(anchor.open_read_write(Path::new("candidate")).is_err());
        for name in ["candidate", "alias"] {
            let mut binary = anchor.open_binary_read_only(Path::new(name)).unwrap();
            let mut actual = Vec::new();
            binary.read_to_end(&mut actual).unwrap();
            assert_eq!(actual, b"binary fixture");
            binary.validate_current().unwrap();
        }
        assert_eq!(
            fs::read(scratch.0.join("alias")).unwrap(),
            b"binary fixture"
        );
    }

    #[test]
    fn tlcx10_binary_read_rejects_namespace_swap_even_with_identical_bytes() {
        let scratch = Scratch::new();
        fs::write(scratch.0.join("candidate"), b"same bytes").unwrap();
        let anchor = DirectoryAnchor::open_existing(&scratch.0).unwrap();
        let mut binary = anchor
            .open_binary_read_only(Path::new("candidate"))
            .unwrap();
        fs::rename(scratch.0.join("candidate"), scratch.0.join("old")).unwrap();
        fs::write(scratch.0.join("candidate"), b"same bytes").unwrap();
        assert!(binary.validate_current().is_err());
        assert!(binary.read(&mut [0; 1]).is_err());
        assert_eq!(fs::read(scratch.0.join("old")).unwrap(), b"same bytes");
    }

    #[test]
    fn tlcx10_binary_read_accepts_one_leaf_not_parent_or_nested_paths() {
        let scratch = Scratch::new();
        let anchor = DirectoryAnchor::open_existing(&scratch.0).unwrap();
        for path in ["", ".", "..", "../candidate", "nested/candidate"] {
            assert!(anchor.open_binary_read_only(Path::new(path)).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn tlcx10_binary_read_rejects_symlink_without_following_target() {
        let scratch = Scratch::new();
        fs::write(scratch.0.join("candidate"), b"target preserved").unwrap();
        std::os::unix::fs::symlink("candidate", scratch.0.join("link")).unwrap();
        let anchor = DirectoryAnchor::open_existing(&scratch.0).unwrap();
        assert!(anchor.open_binary_read_only(Path::new("link")).is_err());
        assert_eq!(
            fs::read(scratch.0.join("candidate")).unwrap(),
            b"target preserved"
        );
    }

    #[cfg(target_os = "linux")]
    const FIFO_ENV: &str = "EPISTESYS_TLCX10_BINARY_FIFO_WORKER_ROOT";
    #[cfg(target_os = "linux")]
    const FIFO_WORKER: &str = "filesystem::binary_readonly::tests::tlcx10_binary_fifo_worker";

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "subprocess worker only; exercised by bounded FIFO admission test"]
    fn tlcx10_binary_fifo_worker() {
        let root = PathBuf::from(std::env::var_os(FIFO_ENV).expect("FIFO worker root"));
        assert!(root.is_absolute());
        let status = std::process::Command::new("/usr/bin/mkfifo")
            .arg("-m")
            .arg("600")
            .arg(root.join("fifo"))
            .status()
            .unwrap();
        assert!(status.success());
        fs::write(root.join("ready"), b"initial-fifo").unwrap();
        let anchor = DirectoryAnchor::open_existing(&root).unwrap();
        assert!(anchor.open_binary_read_only(Path::new("fifo")).is_err());
        fs::write(root.join("result"), b"rejected").unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn tlcx10_binary_fifo_admission_has_a_test_only_watchdog() {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};
        let scratch = Scratch::new();
        let mut worker = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", FIFO_WORKER, "--ignored", "--nocapture"])
            .env(FIFO_ENV, &scratch.0)
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
        assert_eq!(fs::read(scratch.0.join("ready")).unwrap(), b"initial-fifo");
        assert!(
            !timed_out && status.success(),
            "FIFO worker failed: {status}, timeout={timed_out}"
        );
        assert_eq!(fs::read(scratch.0.join("result")).unwrap(), b"rejected");
    }
}
