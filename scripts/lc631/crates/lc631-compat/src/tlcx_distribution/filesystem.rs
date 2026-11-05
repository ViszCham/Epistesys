//! 実treeとbounded streamの観測。 / Actual tree and bounded stream observations.
#![forbid(unsafe_code)]
use super::{
    validate_path, AuditError, AuditReport, FileObservation, IdentityState, Limits,
    MemberObservation, MemberState, Plan,
};
use sha2::{Digest, Sha256};
use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::path::Path;

/// Read an existing tree only. No process execution, creation or cleanup.
/// Metadata/link checks are portable observations, NOT an anchored race-proof capability.
/// A hostile concurrent filesystem owner still requires an OS-specific locked reader.
pub fn audit_tree<'a>(root: &Path, plan: &Plan<'a>) -> Result<AuditReport<'a>, AuditError> {
    let root_meta = fs::symlink_metadata(root).map_err(|_| AuditError::RootUnavailable)?;
    if !root_meta.is_dir() || linked(&root_meta) {
        return Err(AuditError::LinkOrSpecialFile);
    }
    let root = fs::canonicalize(root).map_err(|_| AuditError::RootUnavailable)?;
    let unlisted = inventory(&root, plan)?;
    let mut members = Vec::new();
    members
        .try_reserve_exact(plan.entries.len())
        .map_err(|_| AuditError::AllocationFailure)?;
    let mut total = 0u64;
    for entry in plan.entries {
        let observed = read_member(&root, entry.path, plan.limits, total)?;
        let state = match observed {
            None => MemberState::Missing,
            Some(file) => {
                total = total
                    .checked_add(file.bytes)
                    .ok_or(AuditError::TotalLimit)?;
                if file.bytes != entry.bytes {
                    MemberState::LengthMismatch
                } else if file.digest != entry.digest {
                    MemberState::DigestMismatch
                } else {
                    MemberState::Matched
                }
            }
        };
        members.push(MemberObservation {
            expected: *entry,
            observed,
            state,
        });
    }
    let identity = if unlisted.is_empty()
        && members
            .iter()
            .all(|entry| entry.state == MemberState::Matched)
    {
        IdentityState::MatchedInventory
    } else {
        IdentityState::Mismatch
    };
    Ok(AuditReport {
        plan: plan.digest,
        members,
        unlisted,
        identity,
        bytes: total,
    })
}

fn inventory(root: &Path, plan: &Plan<'_>) -> Result<Vec<String>, AuditError> {
    let mut pending = Vec::new();
    pending
        .try_reserve(1)
        .map_err(|_| AuditError::AllocationFailure)?;
    pending.push((root.to_path_buf(), 0usize));
    let mut unlisted = Vec::new();
    let mut visited = 0usize;
    while let Some((directory, depth)) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(|_| AuditError::ReadFailed)? {
            if visited >= plan.limits.visits {
                return Err(AuditError::VisitLimit);
            }
            visited += 1;
            let entry = entry.map_err(|_| AuditError::ReadFailed)?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|_| AuditError::ReadFailed)?;
            if linked(&meta) || (!meta.is_dir() && !meta.is_file()) {
                return Err(AuditError::LinkOrSpecialFile);
            }
            let relative = relative(root, &path)?;
            validate_path(&relative)?;
            if meta.is_dir() {
                if depth + 1 > plan.limits.depth {
                    return Err(AuditError::DepthLimit);
                }
                pending
                    .try_reserve(1)
                    .map_err(|_| AuditError::AllocationFailure)?;
                pending.push((path, depth + 1));
            } else if !plan.entries.iter().any(|member| member.path == relative) {
                if unlisted.len() >= plan.limits.members {
                    return Err(AuditError::VisitLimit);
                }
                unlisted
                    .try_reserve(1)
                    .map_err(|_| AuditError::AllocationFailure)?;
                unlisted.push(relative);
            }
        }
    }
    unlisted.sort();
    Ok(unlisted)
}

fn read_member(
    root: &Path,
    relative: &str,
    limits: Limits,
    total: u64,
) -> Result<Option<FileObservation>, AuditError> {
    let mut path = root.to_path_buf();
    for part in relative.split('/') {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(AuditError::ReadFailed),
            Ok(meta) if linked(&meta) => return Err(AuditError::LinkOrSpecialFile),
            Ok(_) => {}
        }
    }
    let resolved = fs::canonicalize(&path).map_err(|_| AuditError::ReadFailed)?;
    if !resolved.starts_with(root) {
        return Err(AuditError::InvalidPath);
    }
    let mut file = File::open(&path).map_err(|_| AuditError::ReadFailed)?;
    let meta = file.metadata().map_err(|_| AuditError::ReadFailed)?;
    if !meta.is_file() || linked(&meta) {
        return Err(AuditError::LinkOrSpecialFile);
    }
    let remaining = limits
        .total_bytes
        .checked_sub(total)
        .ok_or(AuditError::TotalLimit)?;
    let cap = remaining.min(limits.member_bytes);
    let observed = hash_reader(&mut file, cap, limits.reads).map_err(|error| {
        if error == AuditError::MemberLimit && remaining < limits.member_bytes {
            AuditError::TotalLimit
        } else {
            error
        }
    })?;
    Ok(Some(observed))
}

/// Fixed stack buffer plus a one-byte sentinel. Interrupted calls consume the
/// read-call allowance; adversarial endless interruption cannot loop forever.
pub fn hash_reader(
    reader: &mut (impl Read + ?Sized),
    cap: u64,
    calls: usize,
) -> Result<FileObservation, AuditError> {
    if cap > 32 * 1024 * 1024 || calls == 0 || calls > 8192 {
        return Err(AuditError::InvalidLimits);
    }
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 8192];
    for _ in 0..calls {
        let remaining = cap.checked_sub(total).ok_or(AuditError::MemberLimit)?;
        let request = if remaining == 0 {
            1
        } else {
            remaining.min(buffer.len() as u64) as usize
        };
        let count = match reader.read(&mut buffer[..request]) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(AuditError::ReadFailed),
            Ok(count) => count,
        };
        if count > request || count as u64 > remaining {
            return Err(AuditError::MemberLimit);
        }
        if count == 0 {
            return Ok(FileObservation {
                bytes: total,
                digest: hash.finalize().into(),
            });
        }
        total += count as u64;
        hash.update(&buffer[..count]);
    }
    Err(AuditError::ReadLimit)
}

fn relative(root: &Path, path: &Path) -> Result<String, AuditError> {
    let path = path
        .strip_prefix(root)
        .map_err(|_| AuditError::InvalidPath)?;
    let text = path.to_str().ok_or(AuditError::InvalidPath)?;
    Ok(text.replace('\\', "/"))
}
fn linked(meta: &Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}
