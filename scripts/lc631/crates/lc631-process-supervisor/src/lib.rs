#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{spawn, ManagedChild, SpawnRequest, SpawnedProcess};
