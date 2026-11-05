//! Additive storage profile; byte hashes are not authority or authentication.
use super::*;
mod part_read;
mod reader;
mod scan;
mod session;
mod types;
use part_read::read_snapshot_part;
pub use reader::HostJournalBundleReader;
use scan::{next_complete, require_profile_or_empty, scan, Snapshot};
pub use session::HostJournalBundleSession;
use types::{descriptor, parse_name, MARKER, MARKER_BYTES, META_CLAIM, SESSION_SCRATCH};
pub use types::{
    BundlePart, JournalBundleDescriptor, JournalBundleLimits, JournalBundleOpenError,
    JournalBundlePartDescriptor, JournalBundleWriteError, OwnedJournalBundleInventory,
    OwnedJournalBundlePart,
};
