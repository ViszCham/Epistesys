//! Real owned-root rev2 tests. Fixtures print paths and never delete files.
use lc631_host::{
    BundlePart, HostError, HostJournalBundleSession, JournalBundleLimits, JournalBundleOpenError,
};
use lc631_runtime::journal::{
    decode_journal_index, decode_segment, BundleLiveError, JournalBundleLiveSession,
    JournalChainLimits, JournalCursor, JournalIndexError, JournalRecordKind, JournalSegmentBuilder,
    JOURNAL_LOCAL_HEAD_SECTION_TAG,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, Reservation, ResourceAmount, ResourceBroker, ResourceKind,
    ResourceLimits, ResourceRequest, RunResources,
};
use lc631_substrate::{
    codec::{decode_envelope_borrowed, encode_frame, SectionInput},
    Digest32,
};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let base = std::env::temp_dir().canonicalize().unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        for n in 0..32_u32 {
            let path = base.join(format!(
                "epistesys-tlcx07-rev2-{}-{}",
                std::process::id(),
                now + u128::from(n)
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    eprintln!("retained test-owned root: {}", path.display());
                    return Self(path);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("test root: {e}"),
            }
        }
        panic!("unique test root unavailable")
    }
    fn path(&self) -> &Path {
        &self.0
    }
    fn part(&self, name: &str, seq: u64) -> PathBuf {
        self.0.join(format!("{name}-{seq:020}.bin"))
    }
}
fn resources(id: u64, disk: Option<u64>) -> (ResourceBroker, RunResources, Reservation) {
    let limits = ResourceLimits::uniform(64 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            id,
            Digest32::from_bytes([9; 32]),
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut requests = vec![
        ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(32 * 1024 * 1024).unwrap(),
        ),
        ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(32 * 1024 * 1024).unwrap(),
        ),
    ];
    if let Some(bytes) = disk {
        requests.push(ResourceRequest::new(
            ResourceKind::Disk,
            ResourceAmount::checked(bytes).unwrap(),
        ));
    }
    let reservation = run.reserve(&requests).unwrap();
    (broker, run, reservation)
}
fn normal(id: u64) -> (ResourceBroker, RunResources, Reservation) {
    resources(id, Some(16 * 1024 * 1024))
}
fn limits() -> JournalBundleLimits {
    JournalBundleLimits::checked(1000, 16 * 1024 * 1024).unwrap()
}

#[test]
fn tlcx07_bundle_limits_reserve_space_for_the_mandatory_marker() {
    for bytes in 0..8 {
        assert!(JournalBundleLimits::checked(1, bytes).is_err());
    }
    assert!(JournalBundleLimits::checked(1, 8).is_ok());
}
fn frame() -> Vec<u8> {
    let mut b = JournalSegmentBuilder::genesis();
    b.append_at(
        1,
        JournalRecordKind::Evidence,
        Digest32::from_bytes([1; 32]),
        b"evidence",
    )
    .unwrap();
    b.finish().unwrap()
}
fn create_part(root: &Root, name: &str, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.part(name, 1))
        .unwrap();
    file.write_all(bytes).unwrap();
}
fn marker(root: &Root, id: u64) {
    let (_b, _r, mut reservation) = normal(id);
    {
        let view = reservation.lease();
        let _host = HostJournalBundleSession::open(root.path(), 0, limits(), &view).unwrap();
    }
    reservation.release_confirmed().unwrap();
    assert_eq!(
        fs::read(root.path().join(".journal-bundle-v1")).unwrap(),
        b"EJBDL001"
    );
}

#[test]
fn tlcx07_rev2_disk_preclaim_failure_leaves_nonexistent_child_absent() {
    for (id, disk, expected) in [(1, None, "Unreserved"), (2, Some(7), "Exhausted")] {
        let parent = Root::new();
        let child = parent.path().join("child-store");
        let (_b, _r, mut reservation) = resources(id, disk);
        {
            let view = reservation.lease();
            let result = HostJournalBundleSession::open(&child, 0, limits(), &view);
            assert!(
                matches!(result, Err(JournalBundleOpenError::BeforeEffect(HostError::Io(s)))
                if s.contains(expected))
            );
        }
        assert!(
            !child.exists(),
            "marker's 8-byte claim must precede root and lock creation"
        );
        assert!(!child.join(".resource-owner-v1.lock").exists());
        reservation.release_confirmed().unwrap();
    }
}

#[test]
fn tlcx07_rev2_zero_byte_parts_never_form_a_complete_bundle() {
    let root = Root::new();
    marker(&root, 3);
    for name in ["segment", "index", "local-head"] {
        create_part(&root, name, b"");
    }
    let (_b, _r, mut reservation) = normal(4);
    {
        let view = reservation.lease();
        let result = JournalBundleLiveSession::open(
            root.path(),
            1024 * 1024,
            limits(),
            &view,
            JournalChainLimits::default(),
        );
        assert!(matches!(result, Err(BundleLiveError::HostOpen(
            JournalBundleOpenError::BeforeEffect(HostError::Io(message))))
            if message.contains("bundle_part_empty")));
    }
    for name in ["segment", "index", "local-head"] {
        assert_eq!(fs::read(root.part(name, 1)).unwrap(), b"");
    }
    reservation.release_confirmed().unwrap();
}

fn produced() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let root = Root::new();
    let (_b, _r, mut reservation) = normal(10);
    let bytes = frame();
    {
        let view = reservation.lease();
        let mut live = JournalBundleLiveSession::open(
            root.path(),
            1024 * 1024,
            limits(),
            &view,
            JournalChainLimits::default(),
        )
        .unwrap();
        live.append(JournalCursor::genesis(), &bytes).unwrap();
    }
    reservation.release_confirmed().unwrap();
    (
        fs::read(root.part("segment", 1)).unwrap(),
        fs::read(root.part("index", 1)).unwrap(),
        fs::read(root.part("local-head", 1)).unwrap(),
    )
}

#[test]
fn tlcx07_rev2_selected_read_ignores_unrelated_new_file_but_final_snapshot_rejects() {
    let (segment, index, head) = produced();
    let root = Root::new();
    marker(&root, 11);
    create_part(&root, "segment", &segment);
    create_part(&root, "index", &index);
    create_part(&root, "local-head", &head);
    let (_b, _r, mut reservation) = normal(12);
    {
        let view = reservation.lease();
        let mut host = HostJournalBundleSession::open(root.path(), 0, limits(), &view).unwrap();
        create_part(&root, "unrelated", b"late entry");
        let selected = host.read_part(1, BundlePart::Segment).unwrap();
        assert_eq!(selected.as_bytes(), segment);
        drop(selected);
        assert!(host.validate_snapshot().is_err());
        assert!(host.next_sequence().is_err());
    }
    assert_eq!(fs::read(root.part("unrelated", 1)).unwrap(), b"late entry");
    reservation.release_confirmed().unwrap();
}

const INDEX_DOMAIN: &[u8] = b"Epistesys/TLCX/JournalIndex/sha256/v1\0";
fn coherent_forgery(index: &[u8], head: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let frame = decode_envelope_borrowed(index).unwrap();
    let section = frame.section(0).unwrap().unwrap();
    let mut payload = section.payload().to_vec();
    payload[104 + 16] ^= 1; // record digest, not lengths/sequence
    let mut h = Sha256::new();
    h.update(INDEX_DOMAIN);
    h.update(&payload[..72]);
    h.update([0_u8; 32]);
    h.update(&payload[104..]);
    payload[72..104].copy_from_slice(&h.finalize());
    let forged_index = encode_frame(&[SectionInput {
        tag: section.tag(),
        version: section.version(),
        flags: section.flags(),
        count: section.count(),
        payload: &payload,
    }])
    .unwrap();
    let index_digest = decode_envelope_borrowed(&forged_index).unwrap().digest();
    let outer = decode_envelope_borrowed(head).unwrap();
    let head_section = outer.section(0).unwrap().unwrap();
    let mut head_payload = head_section.payload().to_vec();
    head_payload[80..112].copy_from_slice(index_digest.as_bytes());
    let forged_head = encode_frame(&[SectionInput {
        tag: JOURNAL_LOCAL_HEAD_SECTION_TAG,
        version: 1,
        flags: 0,
        count: 1,
        payload: &head_payload,
    }])
    .unwrap();
    (forged_index, forged_head)
}
#[test]
fn tlcx07_rev2_resealed_forged_index_fails_segment_correspondence() {
    let (segment, index, head) = produced();
    let (forged_index, forged_head) = coherent_forgery(&index, &head);
    let parsed = decode_journal_index(&forged_index).unwrap();
    let view = decode_segment(&segment).unwrap();
    assert_eq!(
        parsed.verify_segment(&view),
        Err(JournalIndexError::StaleSegment)
    );
    let head_frame = decode_envelope_borrowed(&forged_head).unwrap();
    let head_section = head_frame.section(0).unwrap().unwrap();
    let index_frame = decode_envelope_borrowed(&forged_index).unwrap();
    assert_eq!(
        &head_section.payload()[80..112],
        index_frame.digest().as_bytes()
    );
    let root = Root::new();
    marker(&root, 20);
    create_part(&root, "segment", &segment);
    create_part(&root, "index", &forged_index);
    create_part(&root, "local-head", &forged_head);
    let (_b, _r, mut reservation) = normal(21);
    {
        let view = reservation.lease();
        let result = JournalBundleLiveSession::open(
            root.path(),
            1024 * 1024,
            limits(),
            &view,
            JournalChainLimits::default(),
        );
        assert!(matches!(result, Err(BundleLiveError::RebuildRequired)));
    }
    assert_eq!(fs::read(root.part("index", 1)).unwrap(), forged_index);
    assert_eq!(fs::read(root.part("local-head", 1)).unwrap(), forged_head);
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_rev2_after_real_part_write_failure_is_may_have_applied_and_poisoned() {
    for (id, stop) in [(30, BundlePart::Segment), (31, BundlePart::Index)] {
        let root = Root::new();
        let (_b, _r, mut reservation) = normal(id);
        let segment = frame();
        let retained_index;
        {
            let view = reservation.lease();
            let mut live = JournalBundleLiveSession::open(
                root.path(),
                1024 * 1024,
                limits(),
                &view,
                JournalChainLimits::default(),
            )
            .unwrap();
            let failure = live.append_observed(JournalCursor::genesis(), &segment, |stage| {
                if stage == stop {
                    Err(HostError::Io("test injected after real write".into()))
                } else {
                    Ok(())
                }
            });
            assert!(matches!(
                failure,
                Err(BundleLiveError::StorageMayHaveApplied(_))
            ));
            assert!(matches!(live.cursor(), Err(BundleLiveError::Poisoned)));
            assert!(matches!(
                live.append(JournalCursor::genesis(), &segment),
                Err(BundleLiveError::Poisoned)
            ));
            assert_eq!(fs::read(root.part("segment", 1)).unwrap(), segment);
            if stop == BundlePart::Index {
                let idx = fs::read(root.part("index", 1)).unwrap();
                let parsed = decode_journal_index(&idx).unwrap();
                parsed
                    .verify_segment(&decode_segment(&segment).unwrap())
                    .unwrap();
                retained_index = Some(idx);
            } else {
                assert!(!root.part("index", 1).exists());
                retained_index = None;
            }
            assert!(!root.part("local-head", 1).exists());
        }
        // A separate fresh lease replays; uncertain original lease is not
        // reported as clean or explicitly released as confirmed.
        let (_fresh_b, _fresh_r, mut fresh_reservation) = normal(id + 20);
        {
            let view = fresh_reservation.lease();
            let result = JournalBundleLiveSession::open(
                root.path(),
                1024 * 1024,
                limits(),
                &view,
                JournalChainLimits::default(),
            );
            assert!(matches!(result, Err(BundleLiveError::RebuildRequired)));
        }
        assert_eq!(fs::read(root.part("segment", 1)).unwrap(), segment);
        if let Some(idx) = retained_index {
            assert_eq!(fs::read(root.part("index", 1)).unwrap(), idx);
        } else {
            assert!(!root.part("index", 1).exists());
        }
        assert_eq!(
            fs::read(root.path().join(".journal-bundle-v1")).unwrap(),
            b"EJBDL001"
        );
        assert!(!root.part("local-head", 1).exists());
        assert!(matches!(
            reservation.release_confirmed(),
            Err(lc631_runtime::resource::ResourceError::CleanupUnverified)
        ));
        fresh_reservation.release_confirmed().unwrap();
    }
}

fn next_frame(cursor: JournalCursor) -> Vec<u8> {
    let mut builder = JournalSegmentBuilder::after(cursor);
    builder
        .append_at(
            cursor.sequence() + 1,
            JournalRecordKind::Evidence,
            Digest32::from_bytes([2; 32]),
            b"next",
        )
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn tlcx07_bundle_two_appends_reopen_and_keep_legacy_reader_out() {
    let root = Root::new();
    let first = frame();
    let first_cursor = decode_segment(&first).unwrap().cursor();
    let second = next_frame(first_cursor);
    let second_cursor = decode_segment(&second).unwrap().cursor();
    let (_b, _r, mut reservation) = normal(60);
    {
        let view = reservation.lease();
        let mut live = JournalBundleLiveSession::open(
            root.path(),
            1024 * 1024,
            limits(),
            &view,
            JournalChainLimits::default(),
        )
        .unwrap();
        let one = live.append(JournalCursor::genesis(), &first).unwrap();
        let two = live.append(first_cursor, &second).unwrap();
        assert_eq!(one.file_sequence(), 1);
        assert_eq!(two.file_sequence(), 2);
        assert_ne!(one.head_frame_digest(), two.head_frame_digest());
        assert_eq!(live.cursor().unwrap(), second_cursor);
        assert!(
            lc631_host::persistent_owner::HostPersistentStoreReader::open_binary_checkpoint_root(
                root.path(),
                lc631_host::PersistentStoreLimits::default(),
                &view
            )
            .is_err()
        );
    }
    reservation.release_confirmed().unwrap();
    let (_b, _r, mut fresh) = normal(61);
    {
        let view = fresh.lease();
        let mut reopened = JournalBundleLiveSession::open(
            root.path(),
            0,
            limits(),
            &view,
            JournalChainLimits::default(),
        )
        .unwrap();
        assert_eq!(reopened.cursor().unwrap(), second_cursor);
    }
    fresh.release_confirmed().unwrap();
    assert_eq!(fs::read(root.part("segment", 1)).unwrap(), first);
    assert_eq!(fs::read(root.part("segment", 2)).unwrap(), second);
}

#[test]
fn tlcx07_bundle_each_rehashed_head_binding_is_checked() {
    let (segment, index, head) = produced();
    for (case, (offset, mask)) in [
        (0, 2),
        (8, 2),
        (16, 1),
        (48, 1),
        (80, 1),
        (112, 1),
        (144, 2),
        (148, 1),
    ]
    .into_iter()
    .enumerate()
    {
        let envelope = decode_envelope_borrowed(&head).unwrap();
        let section = envelope.section(0).unwrap().unwrap();
        let mut payload = section.payload().to_vec();
        payload[offset] ^= mask;
        let changed = encode_frame(&[SectionInput {
            tag: JOURNAL_LOCAL_HEAD_SECTION_TAG,
            version: 1,
            flags: 0,
            count: 1,
            payload: &payload,
        }])
        .unwrap();
        decode_envelope_borrowed(&changed).unwrap();
        let root = Root::new();
        marker(&root, 70 + u64::try_from(case).unwrap());
        create_part(&root, "segment", &segment);
        create_part(&root, "index", &index);
        create_part(&root, "local-head", &changed);
        let (_b, _r, mut reservation) = normal(90 + u64::try_from(case).unwrap());
        {
            let view = reservation.lease();
            assert!(matches!(
                JournalBundleLiveSession::open(
                    root.path(),
                    0,
                    limits(),
                    &view,
                    JournalChainLimits::default()
                ),
                Err(BundleLiveError::RebuildRequired)
            ));
        }
        reservation.release_confirmed().unwrap();
        assert_eq!(fs::read(root.part("local-head", 1)).unwrap(), changed);
    }
}

#[test]
fn tlcx07_bundle_root_byte_limit_counts_marker_and_both_sidecars() {
    let (segment, index, head) = produced();
    let physical = u64::try_from(8 + segment.len() + index.len() + head.len()).unwrap();
    let root = Root::new();
    marker(&root, 110);
    create_part(&root, "segment", &segment);
    create_part(&root, "index", &index);
    create_part(&root, "local-head", &head);
    let (_b, _r, reservation) = normal(111);
    {
        let view = reservation.lease();
        let mut live = JournalBundleLiveSession::open(
            root.path(),
            physical,
            JournalBundleLimits::checked(2, physical).unwrap(),
            &view,
            JournalChainLimits::default(),
        )
        .unwrap();
        let cursor = live.cursor().unwrap();
        assert!(matches!(live.append(cursor, &next_frame(cursor)),
            Err(BundleLiveError::StorageRejectedBeforeWrite(HostError::Io(message)))
            if message.contains("bundle_root_bytes_exhausted")));
        assert!(matches!(live.cursor(), Err(BundleLiveError::Poisoned)));
    }
    for name in ["segment", "index", "local-head"] {
        assert!(!root.part(name, 2).exists());
    }
}

fn capped_resources(id: u64, ram: u64, output: u64) -> (ResourceBroker, RunResources, Reservation) {
    let limits = ResourceLimits::uniform(64 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            id,
            Digest32::from_bytes([9; 32]),
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(ram).unwrap()),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(output).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Disk,
                ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
            ),
        ])
        .unwrap();
    (broker, run, reservation)
}

#[test]
fn tlcx07_bundle_parser_work_denial_preserves_open_effects_and_writes_no_parts() {
    use lc631_runtime::resource::ResourceError;
    for (id, ram, output, denied) in [
        (120, 10 * 1024 * 1024, 32 * 1024 * 1024, ResourceKind::Ram),
        (
            121,
            32 * 1024 * 1024,
            10 * 1024 * 1024,
            ResourceKind::Output,
        ),
    ] {
        let root = Root::new();
        let (_b, _r, mut reservation) = capped_resources(id, ram, output);
        {
            let view = reservation.lease();
            assert!(
                matches!(JournalBundleLiveSession::open(root.path(), 1024 * 1024,
                limits(), &view, JournalChainLimits::default()),
                Err(BundleLiveError::Resource(ResourceError::Exhausted(kind))) if kind == denied)
            );
        }
        // Resource refusal after host open is NOT evidence of zero effects.
        assert_eq!(
            fs::read(root.path().join(".journal-bundle-v1")).unwrap(),
            b"EJBDL001"
        );
        for name in ["segment", "index", "local-head"] {
            assert!(!root.part(name, 1).exists());
        }
        reservation.release_confirmed().unwrap();
    }
}

#[test]
fn tlcx07_bundle_unwind_after_real_write_keeps_poison_and_uncertain_cleanup() {
    let root = Root::new();
    let (_b, _r, mut reservation) = normal(130);
    let bytes = frame();
    {
        let view = reservation.lease();
        let mut live = JournalBundleLiveSession::open(
            root.path(),
            1024 * 1024,
            limits(),
            &view,
            JournalChainLimits::default(),
        )
        .unwrap();
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = live.append_observed(JournalCursor::genesis(), &bytes, |part| {
                assert_ne!(
                    part,
                    BundlePart::Segment,
                    "owned test fault after segment sync"
                );
                Ok(())
            });
        }));
        assert!(failure.is_err());
        assert!(matches!(live.cursor(), Err(BundleLiveError::Poisoned)));
    }
    assert!(matches!(
        reservation.release_confirmed(),
        Err(lc631_runtime::resource::ResourceError::CleanupUnverified)
    ));
    assert_eq!(fs::read(root.part("segment", 1)).unwrap(), bytes);
    assert!(!root.part("index", 1).exists());
    assert!(!root.part("local-head", 1).exists());
}
