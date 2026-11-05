use lc631_host::{HostError, HostPersistentStoreOwner, PersistentStoreKind, PersistentStoreLimits};
use lc631_process_supervisor::test_fixtures::{bounded_test_lease, BoundedTestLease};
use lc631_process_supervisor::{ResourceError, ResourceReservationView};
use lc631_receipt_kernel::digest_bytes;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const TEST_ROOT_PREFIX: &str = "epistesys-tlcx07-inventory-";
const SEGMENT_CAP: u64 = 4 * 1024 * 1024;

struct OwnedTestRoot {
    path: PathBuf,
    canonical_base: PathBuf,
}

impl OwnedTestRoot {
    fn create() -> Self {
        let canonical_base = std::env::temp_dir()
            .canonicalize()
            .expect("system temporary directory must be accessible");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after the epoch")
            .as_nanos();
        for attempt in 0..32_u32 {
            let path = canonical_base.join(format!(
                "{TEST_ROOT_PREFIX}{}-{}",
                std::process::id(),
                nonce.saturating_add(u128::from(attempt))
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    return Self {
                        path,
                        canonical_base,
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("test-owned root creation failed: {error}"),
            }
        }
        panic!("could not allocate a unique test-owned root")
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn segment(&self, sequence: u64) -> PathBuf {
        self.path.join(format!("segment-{sequence:020}.bin"))
    }
}

impl Drop for OwnedTestRoot {
    fn drop(&mut self) {
        let Ok(metadata) = fs::symlink_metadata(&self.path) else {
            return;
        };
        if !metadata.is_dir() || is_reparse(&metadata) {
            return;
        }
        let Ok(canonical) = self.path.canonicalize() else {
            return;
        };
        let owned = canonical
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(TEST_ROOT_PREFIX));
        if !owned || canonical.parent() != Some(self.canonical_base.as_path()) {
            return;
        }
        let Ok(entries) = fs::read_dir(&canonical) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if name != ".resource-owner-v1.lock"
                && name != ".writer.lock"
                && !name
                    .to_str()
                    .is_some_and(|name| name.starts_with("segment-") || name.starts_with("junk-"))
            {
                return;
            }
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                return;
            };
            if !metadata.is_file() || is_reparse(&metadata) {
                return;
            }
            if fs::remove_file(path).is_err() {
                return;
            }
        }
        let _ = fs::remove_dir(canonical);
    }
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

/// A locally-issued view for exercising claim accounting only; it is not a
/// process-global broker or proof of aggregate runtime quota enforcement.
fn test_lease(run_id: u64) -> BoundedTestLease {
    bounded_test_lease(run_id).unwrap()
}

fn write_segment(
    root: &Path,
    sequence: u64,
    bytes: &[u8],
    limits: PersistentStoreLimits,
    run_id: u64,
) {
    let lease = test_lease(run_id);
    {
        let view = lease.view();
        write_segment_with_view(root, sequence, bytes, limits, &view);
    }
    lease.close_confirmed().unwrap();
}

fn write_segment_with_view<'a>(
    root: &Path,
    sequence: u64,
    bytes: &[u8],
    limits: PersistentStoreLimits,
    view: &'a ResourceReservationView<'a>,
) {
    let target = root.join(format!("segment-{sequence:020}.bin"));
    let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
        root,
        &target,
        SEGMENT_CAP,
        limits,
        view,
    )
    .unwrap();
    assert_eq!(
        owner.write_binary_checkpoint_segment(bytes),
        Ok(digest_bytes(bytes))
    );
}

fn default_limits() -> PersistentStoreLimits {
    PersistentStoreLimits::default()
}

#[test]
fn tlcx07_inventory_returns_sorted_fixed_identities_without_paths() {
    let root = OwnedTestRoot::create();
    let first = b"first opaque segment";
    let second = b"second opaque segment";
    write_segment(root.path(), 2, second, default_limits(), 1);
    write_segment(root.path(), 1, first, default_limits(), 2);
    let lease = test_lease(3);
    let view = lease.view();

    let inventory = {
        let target = root.segment(3);
        let owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &target,
            SEGMENT_CAP,
            default_limits(),
            &view,
        )
        .unwrap();
        let inventory = owner.binary_checkpoint_segment_inventory().unwrap();
        assert!(!owner.target_exists());
        inventory
    };
    assert!(matches!(
        lease.close_confirmed(),
        Err(ResourceError::LiveBuffer)
    ));

    assert_eq!(inventory.len(), 2);
    assert_eq!(inventory[0].sequence(), 1);
    assert_eq!(inventory[0].byte_len(), first.len() as u64);
    assert_eq!(inventory[0].digest(), digest_bytes(first));
    assert_eq!(inventory[1].sequence(), 2);
    assert_eq!(inventory[1].byte_len(), second.len() as u64);
    assert_eq!(inventory[1].digest(), digest_bytes(second));
    drop(inventory);
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx07_inventory_rejects_legacy_json_profile_and_mixed_root_entries() {
    {
        let root = OwnedTestRoot::create();
        let lease = test_lease(1);
        let view = lease.view();
        let legacy = root.path().join("checkpoint-00000000000000000001.json");
        let owner = HostPersistentStoreOwner::begin(
            root.path(),
            &legacy,
            PersistentStoreKind::Checkpoint,
            SEGMENT_CAP,
            default_limits(),
            &view,
        )
        .unwrap();
        assert!(matches!(
            owner.binary_checkpoint_segment_inventory(),
            Err(HostError::Io(message))
                if message.contains("binary_checkpoint_segment_profile_required")
        ));
        drop(owner);
        lease.close_confirmed().unwrap();
    }

    {
        let root = OwnedTestRoot::create();
        write_segment(root.path(), 1, b"valid-name", default_limits(), 2);
        let junk = root.path().join("junk-unknown.dat");
        let lease = test_lease(3);
        let view = lease.view();
        let target = root.segment(2);
        let owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &target,
            SEGMENT_CAP,
            default_limits(),
            &view,
        )
        .unwrap();
        fs::write(&junk, b"unknown entry").unwrap();
        assert!(matches!(
            owner.binary_checkpoint_segment_inventory(),
            Err(HostError::Io(message)) if message.contains("unknown_checkpoint_entry")
        ));
        drop(owner);
        fs::remove_file(junk).unwrap();
        lease.close_confirmed().unwrap();
    }
}

#[test]
fn tlcx07_inventory_holds_without_a_partial_result_when_limits_are_exceeded() {
    let root = OwnedTestRoot::create();
    let mut limits = default_limits();
    limits.max_files = 5;
    write_segment(root.path(), 1, b"one", limits, 1);
    let lease = test_lease(2);

    {
        let view = lease.view();
        let owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &root.segment(1),
            SEGMENT_CAP,
            limits,
            &view,
        )
        .unwrap();
        for sequence in 2..=9 {
            fs::write(root.segment(sequence), [sequence as u8]).unwrap();
        }
        assert!(matches!(
            owner.binary_checkpoint_segment_inventory(),
            Err(HostError::Io(message)) if message.contains("file_count_exhausted")
        ));
    }
    lease.close_confirmed().unwrap();

    let root = OwnedTestRoot::create();
    let mut byte_limits = default_limits();
    byte_limits.max_total_bytes = 8;
    write_segment(root.path(), 1, b"12345", byte_limits, 3);
    let lease = test_lease(4);
    {
        let view = lease.view();
        let owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &root.segment(1),
            SEGMENT_CAP,
            byte_limits,
            &view,
        )
        .unwrap();
        fs::write(root.segment(2), b"67890").unwrap();
        assert!(matches!(
            owner.binary_checkpoint_segment_inventory(),
            Err(HostError::Io(message)) if message.contains("root_byte_budget_exhausted")
        ));
    }
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx07_inventory_bound_read_by_sequence_returns_only_the_selected_fixed_segment() {
    let root = OwnedTestRoot::create();
    let first = b"first raw segment";
    let second = b"second raw segment";
    write_segment(root.path(), 1, first, default_limits(), 1);
    write_segment(root.path(), 2, second, default_limits(), 2);
    let lease = test_lease(3);

    {
        let view = lease.view();
        let owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &root.segment(3),
            SEGMENT_CAP,
            default_limits(),
            &view,
        )
        .unwrap();
        let inventory = owner.binary_checkpoint_segment_inventory().unwrap();
        assert_eq!(
            inventory
                .iter()
                .map(|item| item.sequence())
                .collect::<Vec<_>>(),
            [1, 2]
        );

        let selected = owner
            .read_binary_checkpoint_segment_at(inventory[1].sequence())
            .unwrap();
        assert_eq!(selected.as_bytes(), second);
        assert_eq!(selected.len() as u64, inventory[1].byte_len());
        assert_eq!(digest_bytes(selected.as_bytes()), inventory[1].digest());
    }

    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx07_inventory_bound_read_rejects_a_sequence_absent_from_a_fresh_inventory() {
    let root = OwnedTestRoot::create();
    write_segment(root.path(), 1, b"existing segment", default_limits(), 1);
    let lease = test_lease(2);

    {
        let view = lease.view();
        let owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &root.segment(2),
            SEGMENT_CAP,
            default_limits(),
            &view,
        )
        .unwrap();
        assert!(matches!(
            owner.read_binary_checkpoint_segment_at(999),
            Err(HostError::Io(message)) if message.contains("binary_checkpoint_sequence_missing")
        ));
        assert!(!root.segment(999).exists());
    }

    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx07_descriptor_read_is_bound_to_inventory_size_and_digest() {
    let root = OwnedTestRoot::create();
    let original = b"original segment bytes";
    write_segment(root.path(), 1, original, default_limits(), 1);
    let lease = test_lease(2);

    {
        let view = lease.view();
        let owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &root.segment(2),
            SEGMENT_CAP,
            default_limits(),
            &view,
        )
        .unwrap();
        let inventory = owner.binary_checkpoint_segment_inventory().unwrap();
        let descriptor = inventory.first().unwrap();

        let selected = owner
            .read_binary_checkpoint_segment_descriptor(descriptor)
            .unwrap();
        assert_eq!(selected.as_bytes(), original);
        drop(selected);

        // Preserve the fixed sequence/name while changing only the bytes.
        let replacement = b"modified segment bytes";
        assert_eq!(replacement.len(), original.len());
        fs::write(root.segment(1), replacement).unwrap();
        assert!(matches!(
            owner.read_binary_checkpoint_segment_descriptor(descriptor),
            Err(HostError::Io(message)) if message.contains("binary_checkpoint_segment_digest_changed")
        ));

        // A stale size descriptor is also rejected without returning a partial
        // buffer. This shorter payload remains within the fixed segment cap.
        fs::write(root.segment(1), b"short").unwrap();
        assert!(matches!(
            owner.read_binary_checkpoint_segment_descriptor(descriptor),
            Err(HostError::Io(_))
        ));
    }

    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx07_owned_inventory_retains_ram_claim_after_owner_drop() {
    let root = OwnedTestRoot::create();
    write_segment(root.path(), 1, b"one", default_limits(), 1);
    let lease = test_lease(2);
    let view = lease.view();
    let (first_inventory, second_inventory) = {
        let owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root.path(),
            &root.segment(2),
            SEGMENT_CAP,
            default_limits(),
            &view,
        )
        .unwrap();
        let first = owner.binary_checkpoint_segment_inventory().unwrap();
        let second = owner.binary_checkpoint_segment_inventory().unwrap();
        (first, second)
    };

    assert_eq!(first_inventory.len(), 1);
    assert_eq!(second_inventory.len(), 1);
    assert!(matches!(
        lease.close_confirmed(),
        Err(ResourceError::LiveBuffer)
    ));
    drop(first_inventory);
    assert!(matches!(
        lease.close_confirmed(),
        Err(ResourceError::LiveBuffer)
    ));
    drop(second_inventory);
    lease.close_confirmed().unwrap();
}
