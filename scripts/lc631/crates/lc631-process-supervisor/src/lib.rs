#![deny(unsafe_op_in_unsafe_fn)]

pub mod filesystem;

mod media_snapshot;
pub use media_snapshot::AuthenticatedSourceSnapshot;

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{spawn, ManagedChild, SpawnRequest, SpawnedProcess};

#[cfg(unix)]
mod unix;

#[cfg(unix)]
pub use unix::{spawn, ManagedChild, SpawnRequest, SpawnedProcess};

mod bounded;

pub use bounded::{run_bounded, RunOutcome, RunRequest};
