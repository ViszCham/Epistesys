//! Private native-producer failure ownership, never reconstructed from a report.
#![forbid(unsafe_code)]
use crate::ManagedChild;
use lc631_substrate::resource::ResourceError;
use std::{
    fs::File,
    io,
    time::{Duration, Instant},
};

pub(crate) struct NativePartialOwner {
    pub(crate) child: Option<ManagedChild>,
    #[cfg(unix)]
    pub(crate) ungrouped_root: Option<std::process::Child>,
    pub(crate) pipes: [Option<File>; 3],
}
impl NativePartialOwner {
    pub(crate) fn cleanup(&mut self, timeout: Duration) -> io::Result<bool> {
        // Closing only the owned parent endpoints does not certify child exit.
        self.pipes = std::array::from_fn(|_| None);
        if let Some(child) = self.child.as_mut() {
            return child.terminate_tree(timeout);
        }
        #[cfg(unix)]
        if let Some(child) = self.ungrouped_root.as_mut() {
            let deadline = Instant::now() + timeout;
            if child.try_wait()?.is_none() {
                child.kill()?;
            }
            // Root exit alone cannot establish group cleanup when its group ID
            // could not be represented. Keep this containment gap explicit.
            loop {
                if child.try_wait()?.is_some() {
                    return Ok(false);
                }
                if Instant::now() >= deadline {
                    return Ok(false);
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        Ok(false)
    }
    fn into_spawned(mut self) -> Result<crate::SpawnedProcess, (io::Error, Self)> {
        if self.child.is_none() || self.pipes.iter().any(Option::is_none) {
            return Err((io::Error::other("incomplete owned native launch"), self));
        }
        let child = self.child.take().expect("private child presence checked");
        let stdin = self.pipes[0].take().expect("private pipe presence checked");
        let stdout = self.pipes[1].take().expect("private pipe presence checked");
        let stderr = self.pipes[2].take().expect("private pipe presence checked");
        Ok(crate::SpawnedProcess {
            child,
            stdin,
            stdout,
            stderr,
        })
    }
}
impl Drop for NativePartialOwner {
    fn drop(&mut self) {
        let _ = self.cleanup(Duration::from_millis(250));
    }
}
pub(crate) enum NativeSpawnFailure<E> {
    BeforeChild(io::Error),
    AfterChild {
        error: io::Error,
        owner: NativePartialOwner,
    },
    Admission(E),
    Resource(ResourceError),
}
pub(crate) fn take_spawned(
    owner: &mut Option<NativePartialOwner>,
) -> io::Result<crate::SpawnedProcess> {
    let actual = owner
        .take()
        .ok_or_else(|| io::Error::other("owned native launch absent"))?;
    match actual.into_spawned() {
        Ok(launch) => Ok(launch),
        Err((error, actual)) => {
            *owner = Some(actual);
            Err(error)
        }
    }
}
pub(crate) fn admit<E>(
    view: Option<&lc631_substrate::resource::ResourceReservationView<'_>>,
    deadline: Option<Instant>,
    callback: &mut dyn FnMut() -> Result<(), E>,
) -> Result<(), NativeSpawnFailure<E>> {
    let check = || {
        if let Some(view) = view {
            view.check().map_err(NativeSpawnFailure::Resource)?;
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            return Err(NativeSpawnFailure::Resource(
                ResourceError::DeadlineExceeded,
            ));
        }
        Ok(())
    };
    check()?;
    callback().map_err(NativeSpawnFailure::Admission)?;
    check()
}
impl NativeSpawnFailure<std::convert::Infallible> {
    pub(crate) fn into_legacy(self) -> crate::spawn_failure::SpawnFailure {
        match self {
            Self::BeforeChild(error) => crate::spawn_failure::SpawnFailure::BeforeChild(error),
            Self::AfterChild { error, owner } => {
                drop(owner);
                crate::spawn_failure::SpawnFailure::MayHaveStarted(error)
            }
            Self::Resource(error) => crate::spawn_failure::SpawnFailure::BeforeChild(
                io::Error::other(format!("{error:?}")),
            ),
            Self::Admission(never) => match never {},
        }
    }
}
