use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub struct SpawnRequest<'a> {
    pub executable: &'a OsStr,
    pub arguments: &'a [OsString],
    pub current_dir: &'a Path,
    pub environment: &'a [(OsString, OsString)],
}

pub struct SpawnedProcess {
    pub child: ManagedChild,
    pub stdin: File,
    pub stdout: File,
    pub stderr: File,
}

/// Owns one freshly created process group, not a filesystem/network sandbox.
/// Descendants that deliberately leave the group are outside this containment.
pub struct ManagedChild {
    child: Child,
    group_id: libc::pid_t,
    cleaned: bool,
}

impl ManagedChild {
    pub fn process_id(&self) -> u32 {
        self.child.id()
    }

    pub fn memory_limit_bytes(&self) -> io::Result<Option<usize>> {
        // A process group does not impose aggregate memory limits. A trusted
        // worker may separately set its own per-process RLIMIT_AS.
        Ok(None)
    }

    pub fn try_wait(&mut self) -> io::Result<Option<i32>> {
        self.child
            .try_wait()
            .map(|status| status.map(|status| status.code().unwrap_or(-1)))
    }

    fn group_empty(&self) -> io::Result<bool> {
        // SAFETY: group_id is the positive PID of the child whose process_group(0)
        // was established before exec. Signal zero observes only that owned group.
        if unsafe { libc::kill(-self.group_id, 0) } == 0 {
            return Ok(false);
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(true)
        } else {
            Err(error)
        }
    }

    fn signal_group(&self, signal: libc::c_int) -> io::Result<()> {
        // SAFETY: the negative identifier addresses only this spawn's dedicated
        // group; the request cannot supply a PID or a group chosen by the caller.
        if unsafe { libc::kill(-self.group_id, signal) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(error)
        }
    }

    pub fn wait_for_tree_empty(&mut self, timeout: Duration) -> io::Result<bool> {
        let deadline = Instant::now() + timeout;
        loop {
            let root_reaped = self.try_wait()?.is_some();
            if root_reaped && self.group_empty()? {
                self.cleaned = true;
                return Ok(true);
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            thread::sleep(Duration::from_millis(2));
        }
    }

    pub fn terminate_tree(&mut self, timeout: Duration) -> io::Result<bool> {
        if self.cleaned {
            return Ok(true);
        }
        let deadline = Instant::now() + timeout;
        self.signal_group(libc::SIGTERM)?;
        let grace = Duration::from_millis(25).min(timeout / 4);
        if self.wait_for_tree_empty(grace)? {
            return Ok(true);
        }
        self.signal_group(libc::SIGKILL)?;
        // Child::kill targets the owned root as well; group signaling alone must
        // never leave a lone root alive. try_wait below reaps its exit status.
        if self.try_wait()?.is_none() {
            let _ = self.child.kill();
        }
        self.wait_for_tree_empty(deadline.saturating_duration_since(Instant::now()))
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        if !self.cleaned {
            let _ = self.terminate_tree(Duration::from_millis(100));
        }
    }
}

pub fn spawn(request: SpawnRequest<'_>) -> io::Result<SpawnedProcess> {
    if request.executable.is_empty() || !request.current_dir.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid process input",
        ));
    }
    let mut command = Command::new(request.executable);
    command
        .args(request.arguments)
        .current_dir(request.current_dir)
        .env_clear()
        .envs(request.environment.iter().cloned())
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut root = command.spawn()?;
    let group_id = match libc::pid_t::try_from(root.id()) {
        Ok(id) => id,
        Err(_) => {
            let _ = root.kill();
            let _ = root.wait();
            return Err(io::Error::other("process id outside pid_t"));
        }
    };
    let mut child = ManagedChild {
        child: root,
        group_id,
        cleaned: false,
    };
    let stdin = child
        .child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("missing stdin"))?;
    let stdout = child
        .child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("missing stdout"))?;
    let stderr = child
        .child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("missing stderr"))?;
    let stdin = File::from(OwnedFd::from(stdin));
    let stdout = File::from(OwnedFd::from(stdout));
    let stderr = File::from(OwnedFd::from(stderr));
    for pipe in [&stdin, &stdout, &stderr] {
        set_nonblocking(pipe)?;
    }
    Ok(SpawnedProcess {
        child,
        stdin,
        stdout,
        stderr,
    })
}

fn set_nonblocking(file: &File) -> io::Result<()> {
    let descriptor = file.as_raw_fd();
    // SAFETY: descriptor is borrowed from an open, uniquely owned File. fcntl
    // changes only its open-file status flags, never its ownership or lifetime.
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(descriptor, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
