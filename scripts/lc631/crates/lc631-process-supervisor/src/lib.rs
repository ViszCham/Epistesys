#![deny(unsafe_op_in_unsafe_fn)]

pub mod filesystem;
mod native_dispatch;
pub use native_dispatch::{
    observe_native_dispatches, DispatchObserveError, NativeDispatch, NativeDispatchTrace,
};
mod spawn_failure;
#[cfg(feature = "test-fixtures")]
pub mod test_fixtures;

mod media_snapshot;
pub use media_snapshot::{AuthenticatedSourceSnapshot, ReservedSourceSnapshot};

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub(crate) use windows::spawn_observed as spawn_with_state;
#[cfg(windows)]
pub use windows::{spawn, ManagedChild, SpawnRequest, SpawnedProcess};

#[cfg(unix)]
mod unix;

#[cfg(unix)]
pub(crate) use unix::spawn_observed as spawn_with_state;
#[cfg(unix)]
pub use unix::{spawn, ManagedChild, SpawnRequest, SpawnedProcess};

mod bounded;

pub use bounded::{
    read_bounded_bytes, required_ram_bytes, run_bounded, run_bounded_with_resources,
    run_bounded_with_resources_observed, BoundedBytes, ResourceRunError, ResourceRunOutcome,
    RunOutcome, RunRequest,
};
pub use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
pub use lc631_substrate::Digest32;
