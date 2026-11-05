//! Registered file classes and per-root engineering limits.
use super::store_error;
use crate::HostError;
use std::path::Path;
pub(super) const LOCK: &str = ".resource-owner-v1.lock";
const KEY_CAP: usize = 256;
const LEDGER_CAP: usize = 8 * 1024 * 1024;
const OBSERVATION_CAP: usize = 1024 * 1024;
const CHECKPOINT_SEGMENT_CAP: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistentStoreKind {
    ReceiptKey,
    ReplayLedger,
    StopObservation,
    Checkpoint,
    CheckpointHead,
    RepairBackup,
}
impl PersistentStoreKind {
    pub(super) fn is_checkpoint(self) -> bool {
        matches!(self, Self::Checkpoint | Self::CheckpointHead)
    }
    pub(super) fn is_flat(self) -> bool {
        self.is_checkpoint() || self == Self::RepairBackup
    }
    pub(super) fn constrain(self, mut limits: PersistentStoreLimits) -> PersistentStoreLimits {
        if self.is_checkpoint() {
            limits.max_files = limits.max_files.min(1002);
            limits.max_total_bytes = limits.max_total_bytes.min(16 * 1024 * 1024);
            limits.max_depth = limits.max_depth.min(1);
        }
        if self == Self::RepairBackup {
            limits.max_files = limits.max_files.min(1001);
            limits.max_depth = limits.max_depth.min(1);
        }
        limits
    }
}

/// Private storage-selection profile. Binary checkpoint segments deliberately
/// do not extend the public exhaustive `PersistentStoreKind` enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PersistentStoreProfile {
    Registered(PersistentStoreKind),
    BinaryCheckpointSegment,
}

impl PersistentStoreProfile {
    pub(super) const fn binary_checkpoint_segment() -> Self {
        Self::BinaryCheckpointSegment
    }

    pub(super) const fn kind(self) -> PersistentStoreKind {
        match self {
            Self::Registered(kind) => kind,
            Self::BinaryCheckpointSegment => PersistentStoreKind::Checkpoint,
        }
    }

    pub(super) fn constrain(self, limits: PersistentStoreLimits) -> PersistentStoreLimits {
        self.kind().constrain(limits)
    }

    pub(super) fn is_checkpoint(self) -> bool {
        matches!(self, Self::BinaryCheckpointSegment) || self.kind().is_checkpoint()
    }

    pub(super) fn is_flat(self) -> bool {
        matches!(self, Self::BinaryCheckpointSegment) || self.kind().is_flat()
    }

    pub(super) fn target_class(self) -> FileClass {
        match self {
            Self::Registered(kind) => target_class(kind),
            Self::BinaryCheckpointSegment => FileClass::CheckpointSegment,
        }
    }

    pub(super) fn classify(self, path: &Path, target: &Path) -> Result<FileClass, HostError> {
        match self {
            Self::Registered(kind) => classify(path, target, kind),
            Self::BinaryCheckpointSegment => classify_binary_checkpoint_segment(path),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PersistentStoreLimits {
    pub max_files: usize,
    pub max_total_bytes: u64,
    pub max_depth: usize,
}
impl Default for PersistentStoreLimits {
    fn default() -> Self {
        Self {
            max_files: 1024,
            max_total_bytes: 256 * 1024 * 1024,
            max_depth: 4,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum FileClass {
    Lock,
    Key,
    Ledger,
    Observation,
    Checkpoint,
    CheckpointSegment,
    CheckpointHead,
    RepairBackup,
}
pub(super) fn target_class(kind: PersistentStoreKind) -> FileClass {
    match kind {
        PersistentStoreKind::ReceiptKey => FileClass::Key,
        PersistentStoreKind::ReplayLedger => FileClass::Ledger,
        PersistentStoreKind::StopObservation => FileClass::Observation,
        PersistentStoreKind::Checkpoint => FileClass::Checkpoint,
        PersistentStoreKind::CheckpointHead => FileClass::CheckpointHead,
        PersistentStoreKind::RepairBackup => FileClass::RepairBackup,
    }
}
pub(super) fn class_cap(class: FileClass) -> usize {
    match class {
        FileClass::Lock => 0,
        FileClass::Key => KEY_CAP,
        FileClass::Ledger => LEDGER_CAP,
        FileClass::Observation => OBSERVATION_CAP,
        FileClass::Checkpoint | FileClass::CheckpointHead => 16_384,
        FileClass::CheckpointSegment => CHECKPOINT_SEGMENT_CAP,
        FileClass::RepairBackup => 1024 * 1024,
    }
}
pub(super) fn classify(
    path: &Path,
    target: &Path,
    kind: PersistentStoreKind,
) -> Result<FileClass, HostError> {
    if kind == PersistentStoreKind::RepairBackup {
        if path == Path::new(LOCK) {
            return Ok(FileClass::Lock);
        }
        let valid = path.components().count() == 1
            && path.to_str().is_some_and(|name| {
                name.strip_suffix(".backup")
                    .and_then(|name| name.split_once('-'))
                    .is_some_and(|(attempt, unique)| {
                        matches!(attempt, "1" | "2" | "3" | "4")
                            && !unique.is_empty()
                            && unique.len() <= 39
                            && unique.bytes().all(|byte| byte.is_ascii_digit())
                            && unique.parse::<u128>().is_ok()
                    })
            });
        return if valid {
            Ok(FileClass::RepairBackup)
        } else {
            Err(store_error("unknown_backup_entry"))
        };
    }
    if kind.is_checkpoint() {
        if path == Path::new(LOCK) || path == Path::new(".writer.lock") {
            return Ok(FileClass::Lock);
        }
        if path.components().count() != 1 {
            return Err(store_error("unknown_checkpoint_entry"));
        }
        let name = path
            .to_str()
            .ok_or_else(|| store_error("checkpoint_name_not_utf8"))?;
        let valid = match kind {
            PersistentStoreKind::Checkpoint => numbered_name(name, "checkpoint-"),
            PersistentStoreKind::CheckpointHead => head_name(name),
            PersistentStoreKind::ReceiptKey
            | PersistentStoreKind::ReplayLedger
            | PersistentStoreKind::StopObservation
            | PersistentStoreKind::RepairBackup => false,
        };
        return if valid {
            Ok(target_class(kind))
        } else {
            Err(store_error("unknown_checkpoint_entry"))
        };
    }
    if path == Path::new(LOCK) || path == Path::new("receipt-root").join(LOCK) {
        return Ok(FileClass::Lock);
    }
    if kind == PersistentStoreKind::ReceiptKey {
        return if path == Path::new("receipt-root-v1.key") {
            Ok(FileClass::Key)
        } else {
            Err(store_error("unknown_entry"))
        };
    }
    if path == target {
        return Ok(target_class(kind));
    }
    if path == Path::new("receipt-root/receipt-root-v1.key") {
        return Ok(FileClass::Key);
    }
    if path == Path::new("host-output-v2.jsonl") {
        return Ok(FileClass::Ledger);
    }
    if path.parent() == Some(Path::new("observations"))
        && observation_name(
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(""),
        )
    {
        return Ok(FileClass::Observation);
    }
    Err(store_error("unknown_entry"))
}

fn classify_binary_checkpoint_segment(path: &Path) -> Result<FileClass, HostError> {
    if path == Path::new(LOCK) || path == Path::new(".writer.lock") {
        return Ok(FileClass::Lock);
    }
    if path.components().count() != 1 {
        return Err(store_error("unknown_checkpoint_entry"));
    }
    let name = path
        .to_str()
        .ok_or_else(|| store_error("checkpoint_name_not_utf8"))?;
    if numbered_binary_name(name, "segment-") {
        Ok(FileClass::CheckpointSegment)
    } else {
        Err(store_error("unknown_checkpoint_entry"))
    }
}
fn numbered_name(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .and_then(|name| name.strip_suffix(".json"))
        .is_some_and(decimal20)
}
fn numbered_binary_name(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .and_then(|name| name.strip_suffix(".bin"))
        .is_some_and(decimal20)
}

pub(super) fn binary_segment_sequence(path: &Path) -> Result<u64, HostError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| store_error("checkpoint_name_not_utf8"))?;
    let digits = name
        .strip_prefix("segment-")
        .and_then(|name| name.strip_suffix(".bin"))
        .filter(|digits| decimal20(digits))
        .ok_or_else(|| store_error("unknown_checkpoint_entry"))?;
    digits
        .parse::<u64>()
        .map_err(|_| store_error("checkpoint_sequence_invalid"))
}

fn decimal20(value: &str) -> bool {
    value.len() == 20
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u64>().is_ok()
}
fn head_name(name: &str) -> bool {
    if numbered_name(name, "head-") {
        return true;
    }
    name.strip_prefix("head-")
        .and_then(|name| name.strip_suffix(".json"))
        .and_then(|body| body.split_once(".renew-"))
        .is_some_and(|(sequence, epoch)| decimal20(sequence) && decimal20(epoch))
}
fn observation_name(name: &str) -> bool {
    name.strip_suffix(".json").is_some_and(|stem| {
        stem.len() == 33
            && stem.as_bytes()[16] == b'-'
            && stem.bytes().enumerate().all(|(index, byte)| {
                index == 16 || byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            })
    })
}
pub(super) fn validate_limits(limits: PersistentStoreLimits) -> Result<(), HostError> {
    if limits.max_files == 0
        || limits.max_files > 1024
        || limits.max_total_bytes == 0
        || limits.max_total_bytes > 256 * 1024 * 1024
        || limits.max_depth == 0
        || limits.max_depth > 4
    {
        Err(store_error("invalid_root_limits"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tlcx07_binary_segment_policy_tests {
    use super::*;

    #[test]
    fn binary_segment_owner_accepts_only_fixed_numbered_segment_names() {
        let valid = Path::new("segment-00000000000000000001.bin");
        assert!(matches!(
            PersistentStoreProfile::binary_checkpoint_segment().classify(valid, valid),
            Ok(FileClass::CheckpointSegment)
        ));

        for invalid in [
            Path::new("segment-1.bin"),
            Path::new("segment-00000000000000000001.json"),
            Path::new("checkpoint-00000000000000000001.bin"),
            Path::new("nested/segment-00000000000000000001.bin"),
        ] {
            assert!(
                PersistentStoreProfile::binary_checkpoint_segment()
                    .classify(invalid, invalid)
                    .is_err(),
                "unregistered binary journal path {invalid:?} must be rejected"
            );
        }
    }

    #[test]
    fn binary_segments_have_a_fixed_four_mib_cap_and_legacy_json_is_unchanged() {
        assert_eq!(
            class_cap(FileClass::CheckpointSegment),
            CHECKPOINT_SEGMENT_CAP
        );
        let legacy = Path::new("checkpoint-00000000000000000001.json");
        assert!(matches!(
            classify(legacy, legacy, PersistentStoreKind::Checkpoint),
            Ok(FileClass::Checkpoint)
        ));
        assert!(matches!(
            PersistentStoreProfile::binary_checkpoint_segment().classify(legacy, legacy),
            Err(HostError::Io(message)) if message.contains("unknown_checkpoint_entry")
        ));
    }
}
