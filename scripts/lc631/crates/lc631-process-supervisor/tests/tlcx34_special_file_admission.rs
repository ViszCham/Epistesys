#![cfg(unix)]

use lc631_process_supervisor::filesystem::DirectoryAnchor;
use std::ffi::CString;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const FIFO_CHILD: &str = "EPISTESYS_FIFO_ADMISSION_CHILD";

#[test]
fn special_file_child() {
    let Some(directory) = std::env::var_os(FIFO_CHILD) else {
        return;
    };
    let anchor = DirectoryAnchor::open_existing(Path::new(&directory)).unwrap();
    for result in [
        anchor.open_read_only(Path::new("source.fifo")).map(|_| ()),
        anchor.open_read_write(Path::new("source.fifo")).map(|_| ()),
    ] {
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }
}

#[test]
fn tlcx34_unconnected_fifo_is_refused_before_blocking_open() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("tlcx34-fifo-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    use std::os::unix::ffi::OsStrExt;
    let fifo = CString::new(root.join("source.fifo").as_os_str().as_bytes()).unwrap();
    // SAFETY: terminated unique path in this test's new directory; no existing target is changed.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "special_file_child", "--nocapture"])
        .env(FIFO_CHILD, &root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "special-file child failed: {status}");
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("anchored open blocked before rejecting an unconnected FIFO");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // Retain this unique fixture rather than recursively deleting a computed path.
}
