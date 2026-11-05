use lc631_host::persistent_owner::{
    HostPersistentStoreOwner, HostPersistentStoreReader, PersistentStoreLimits,
};
use lc631_host::HostError;
use lc631_process_supervisor::{
    test_fixtures::{bounded_test_lease, BoundedTestLease},
    ResourceAmount, ResourceError, ResourceKind, ResourceRequest, ResourceReservationView,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const TEST_ROOT_PREFIX: &str = "epistesys-tlcx07-readonly-root-";
const SEGMENT_LIMIT: usize = 4 * 1024 * 1024;

struct ScratchRoot {
    path: PathBuf,
    parent: PathBuf,
}

impl ScratchRoot {
    fn create() -> Self {
        let parent = std::env::temp_dir()
            .canonicalize()
            .expect("system temporary directory is accessible");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock follows the Unix epoch")
            .as_nanos();
        for attempt in 0..32_u32 {
            let path = parent.join(format!(
                "{TEST_ROOT_PREFIX}{}-{}",
                std::process::id(),
                nonce.saturating_add(u128::from(attempt))
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self { path, parent },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("test-owned root create failed: {error}"),
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

impl Drop for ScratchRoot {
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
        if canonical.parent() != Some(self.parent.as_path())
            || !canonical
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(TEST_ROOT_PREFIX))
        {
            return;
        }
        let Ok(entries) = fs::read_dir(&canonical) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name != ".resource-owner-v1.lock"
                && name != ".writer.lock"
                && name != "not-a-directory"
                && !name.starts_with("segment-")
                && !name.starts_with("unknown-")
            {
                return;
            }
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                return;
            };
            if !metadata.is_file() || is_reparse(&metadata) || fs::remove_file(&path).is_err() {
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

fn test_view(lease: &BoundedTestLease) -> ResourceReservationView<'_> {
    lease.view()
}

fn writer_limits() -> PersistentStoreLimits {
    PersistentStoreLimits::default()
}

fn write_segment(root: &Path, sequence: u64, bytes: &[u8], run_id: u64) {
    let lease = test_lease(run_id);
    {
        let view = test_view(&lease);
        let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &root.join(format!("segment-{sequence:020}.bin")),
            SEGMENT_LIMIT as u64,
            writer_limits(),
            &view,
        )
        .unwrap();
        owner.write_binary_checkpoint_segment(bytes).unwrap();
    }
    lease.close_confirmed().unwrap();
}

fn directory_snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut entries = fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().into_owned();
            (name, fs::read(entry.path()).unwrap())
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

#[test]
fn tlcx07_read_only_reader_missing_root_is_not_created() {
    let parent = ScratchRoot::create();
    let missing_root = parent.path().join("missing-root");
    assert!(!missing_root.exists());
    let lease = test_lease(1);
    {
        let view = test_view(&lease);
        assert!(matches!(
            HostPersistentStoreReader::open_binary_checkpoint_root(
                &missing_root,
                writer_limits(),
                &view,
            ),
            Err(HostError::Io(_))
        ));
        assert!(!missing_root.exists());

        let empty_before = directory_snapshot(parent.path());
        let reader = HostPersistentStoreReader::open_binary_checkpoint_root(
            parent.path(),
            writer_limits(),
            &view,
        )
        .unwrap();
        assert!(reader.inventory().unwrap().is_empty());
        drop(reader);
        assert_eq!(directory_snapshot(parent.path()), empty_before);
    }
    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx07_read_only_reader_rejects_a_regular_file_as_root_without_mutating_it() {
    let scratch = ScratchRoot::create();
    let file_root = scratch.path().join("not-a-directory");
    fs::write(&file_root, b"ordinary file, not a root").unwrap();
    let before = fs::read(&file_root).unwrap();
    let lease = test_lease(1);

    {
        let view = test_view(&lease);
        assert!(matches!(
            HostPersistentStoreReader::open_binary_checkpoint_root(
                &file_root,
                writer_limits(),
                &view,
            ),
            Err(HostError::Io(_))
        ));
        assert_eq!(fs::read(&file_root).unwrap(), before);
    }

    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx07_read_only_reader_existing_root_is_unchanged_by_inventory_and_read() {
    let root = ScratchRoot::create();
    let bytes = b"bounded checkpoint bytes";
    write_segment(root.path(), 1, bytes, 1);
    let before = directory_snapshot(root.path());
    let lease = test_lease(2);

    {
        let view = test_view(&lease);
        let reader = HostPersistentStoreReader::open_binary_checkpoint_root(
            root.path(),
            writer_limits(),
            &view,
        )
        .unwrap();
        let descriptors = reader.inventory().unwrap();
        assert_eq!(descriptors.len(), 1);
        assert_eq!(descriptors[0].sequence(), 1);
        let selected = reader
            .read_binary_checkpoint_segment_descriptor(&descriptors[0])
            .unwrap();
        assert_eq!(selected.as_bytes(), bytes);
        drop(selected);
        drop(reader);
    }

    lease.close_confirmed().unwrap();
    assert_eq!(directory_snapshot(root.path()), before);
}

#[test]
fn tlcx07_read_only_reader_rejects_missing_sequences_and_foreign_descriptors() {
    let root = ScratchRoot::create();
    let foreign_root = ScratchRoot::create();
    write_segment(root.path(), 1, b"local segment", 1);
    write_segment(foreign_root.path(), 1, b"foreign segment", 2);
    let lease = test_lease(3);

    {
        let view = test_view(&lease);
        let reader = HostPersistentStoreReader::open_binary_checkpoint_root(
            root.path(),
            writer_limits(),
            &view,
        )
        .unwrap();
        assert!(matches!(
            reader.read_binary_checkpoint_segment_at(999),
            Err(HostError::Io(message)) if message.contains("binary_checkpoint_sequence_missing")
        ));
        assert!(!root.segment(999).exists());

        let foreign = HostPersistentStoreReader::open_binary_checkpoint_root(
            foreign_root.path(),
            writer_limits(),
            &view,
        )
        .unwrap();
        let descriptor = foreign.inventory().unwrap()[0].clone();
        assert!(matches!(
            reader.read_binary_checkpoint_segment_descriptor(&descriptor),
            Err(HostError::Io(message)) if message.contains("binary_checkpoint_descriptor_unknown")
        ));
    }

    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx07_read_only_reader_holds_unknown_and_mixed_entries_without_writing() {
    let root = ScratchRoot::create();
    write_segment(root.path(), 1, b"known segment", 1);
    let unknown = root.path().join("unknown-entry.bin");
    fs::write(&unknown, b"unknown").unwrap();
    let before = directory_snapshot(root.path());
    let lease = test_lease(2);

    {
        let view = test_view(&lease);
        assert!(matches!(
            HostPersistentStoreReader::open_binary_checkpoint_root(
                root.path(),
                writer_limits(),
                &view,
            ),
            Err(HostError::Io(_))
        ));
        assert_eq!(directory_snapshot(root.path()), before);
    }

    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx07_read_only_reader_keeps_buffer_claim_until_returned_bytes_drop() {
    let root = ScratchRoot::create();
    let bytes = vec![0x51; SEGMENT_LIMIT - 1];
    write_segment(root.path(), 1, &bytes, 1);
    let lease = test_lease(2);

    {
        let view = test_view(&lease);
        let reader = HostPersistentStoreReader::open_binary_checkpoint_root(
            root.path(),
            writer_limits(),
            &view,
        )
        .unwrap();
        let descriptors = reader.inventory().unwrap();
        assert_eq!(descriptors.len(), 1);
        let owned = reader
            .read_binary_checkpoint_segment_descriptor(&descriptors[0])
            .unwrap();

        let pressure = view.claim_resources(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(12 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(12 * 1024 * 1024).unwrap(),
            ),
        ]);
        assert!(matches!(
            pressure,
            Err(ResourceError::Exhausted(
                ResourceKind::Ram | ResourceKind::Output
            ))
        ));
        drop(pressure);
        drop(owned);
        let available = view
            .claim_resources(&[
                ResourceRequest::new(
                    ResourceKind::Ram,
                    ResourceAmount::checked(12 * 1024 * 1024).unwrap(),
                ),
                ResourceRequest::new(
                    ResourceKind::Output,
                    ResourceAmount::checked(12 * 1024 * 1024).unwrap(),
                ),
            ])
            .unwrap();
        drop(available);
        drop(reader);
    }

    lease.close_confirmed().unwrap();
}

#[test]
fn tlcx07_read_only_inventory_retains_its_output_claim_until_reader_drop() {
    let root = ScratchRoot::create();
    let lease = test_lease(3);

    {
        let view = test_view(&lease);
        let reader = HostPersistentStoreReader::open_binary_checkpoint_root(
            root.path(),
            writer_limits(),
            &view,
        )
        .unwrap();
        let request = [ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
        )];
        let while_retained = view.claim_resources(&request);
        assert!(matches!(
            while_retained,
            Err(ResourceError::Exhausted(ResourceKind::Output))
        ));
        drop(while_retained);
        drop(reader);
        let after_drop = view.claim_resources(&request).unwrap();
        drop(after_drop);
    }
    lease.close_confirmed().unwrap();
}
