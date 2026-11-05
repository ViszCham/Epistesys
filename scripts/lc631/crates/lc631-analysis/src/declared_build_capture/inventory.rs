//! 日本語: 走査前fundingと走査中metadata上限を持つ実Rust/Cargo inventory。
//! English: Actual Rust/Cargo inventory with pre-funded bounded metadata.
//! Canonical bytes match the inherited declared-input profile. Neither atomic
//! filesystem isolation nor a hermetic build world is claimed.
#![forbid(unsafe_code)]
use crate::{
    build_dgcl_execution_snapshot, dgcl_repository_root_digest, dgcl_validator_revision,
    DgclExecutionSnapshot, DgclPipelineReport, DgclTargetIdentity, ToolProcessRunner,
};
use lc631_core::ExecutionPermitContext;
use lc631_substrate::resource::{
    ResourceAmount, ResourceError, ResourceKind, ResourceRequest, ResourceReservationView,
};
use std::{collections::{btree_map::Entry, BTreeMap}, fs, path::{Path, PathBuf}};

pub(super) const INVENTORY_SCRATCH_RAM: u64 = 8 * 1024 * 1024;
const MAX_VISITS: usize = 16_384;
const MAX_INPUTS: usize = 4_096;
const MAX_PATH_BYTES: usize = 4_096;
const MAX_KEYS_BYTES: usize = 256 * 1024;
const MAX_PENDING_BYTES: usize = 256 * 1024;
const MAX_VIRTUAL_TASKS: usize = 64;
const FILE_CAP: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum InventoryError {
    Resource(ResourceError),
    UnsafePath,
    Limit,
    Snapshot,
}
impl From<ResourceError> for InventoryError {
    fn from(error: ResourceError) -> Self { Self::Resource(error) }
}
fn charge(total: &mut usize, amount: usize, limit: usize) -> Result<(), InventoryError> {
    let next = total.checked_add(amount).ok_or(InventoryError::Limit)?;
    if next > limit { return Err(InventoryError::Limit); }
    *total = next;
    Ok(())
}
fn text(path: &Path) -> Result<&str, InventoryError> {
    let value = path.to_str().ok_or(InventoryError::UnsafePath)?;
    if value.len() > MAX_PATH_BYTES { return Err(InventoryError::Limit); }
    #[cfg(unix)]
    if value.contains('\\') { return Err(InventoryError::UnsafePath); }
    Ok(value)
}
fn path_key(path: &Path) -> Result<String, InventoryError> {
    Ok(text(path)?.replace('\\', "/"))
}
fn check_metadata(metadata: &fs::Metadata) -> Result<(), InventoryError> {
    if metadata.file_type().is_symlink() { return Err(InventoryError::UnsafePath); }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 { return Err(InventoryError::UnsafePath); }
    }
    Ok(())
}
fn read_digest(path: &Path, source: &str, runner: &mut dyn ToolProcessRunner,
    resources: &ResourceReservationView<'_>) -> Result<String, InventoryError> {
    resources.check()?;
    let value = crate::process::with_input_result(runner, source, 8192, |view| {
        // Nested IO uses the SAME accounting owner/run. A new issuer with equal
        // source labels is not the caller's admitted resource owner.
        if view.owner_id() != resources.owner_id() || view.run_id() != resources.run_id() {
            return Err(ResourceError::ForeignOwner);
        }
        let _buffer = view.claim_resources(&[ResourceRequest::new(ResourceKind::Ram,
            ResourceAmount::checked(8192)?)])?;
        crate::bounded_input::hash_capped_typed(path, FILE_CAP, Some(view))
    })?;
    resources.check()?;
    Ok(value)
}

/// Core additionally funds seal/root/authority work. This inventory keeps its
/// own 8MiB scratch claim through destruction of the map/JSON/target temporaries,
/// so private callers cannot accidentally perform an unfunded collection.
/// Logical payload/node/index slack is bounded; allocator headers/OS/RSS and
/// allocation aborts remain separate. Limits refuse without truncation.
pub(super) fn collect(
    root: &Path,
    source: &str,
    pipeline: &DgclPipelineReport,
    authority: &ExecutionPermitContext<'_>,
    runner: &mut dyn ToolProcessRunner,
    resources: &ResourceReservationView<'_>,
) -> Result<DgclExecutionSnapshot, InventoryError> {
    resources.check()?;
    if resources.source_revision() != lc631_core::context::SourceRevision::of(source).digest() {
        return Err(InventoryError::Resource(ResourceError::SourceMismatch));
    }
    let scratch = resources.claim_resources(&[ResourceRequest::new(ResourceKind::Ram,
        ResourceAmount::checked(INVENTORY_SCRATCH_RAM)?)])?;
    if !root.is_absolute() || source.len() > 262_144
        || pipeline.completion_plan.tasks.len() > MAX_VIRTUAL_TASKS {
        return Err(InventoryError::Limit);
    }
    text(root)?;
    let metadata = fs::symlink_metadata(root).map_err(|_| InventoryError::UnsafePath)?;
    check_metadata(&metadata)?;
    if !metadata.is_dir() { return Err(InventoryError::UnsafePath); }

    // The maximum slot reserve is bounded before allocation. Path heap capacity
    // is charged before each push, not after a complete preflight walk.
    let mut pending = Vec::<PathBuf>::new();
    pending.try_reserve_exact(MAX_VISITS).map_err(|_| ResourceError::AllocationFailed)?;
    if pending.capacity() > MAX_VISITS { return Err(ResourceError::AllocationFailed.into()); }
    let mut pending_bytes = 0;
    let mut first = PathBuf::new();
    first.try_reserve_exact(root.as_os_str().len()).map_err(|_| ResourceError::AllocationFailed)?;
    first.push(root);
    if first.capacity() > MAX_PATH_BYTES * 2 { return Err(InventoryError::Limit); }
    charge(&mut pending_bytes, first.capacity(), MAX_PENDING_BYTES)?;
    pending.push(first);
    let mut inputs = BTreeMap::<String, String>::new();
    let (mut visited, mut key_bytes) = (0_usize, 0_usize);
    while let Some(directory) = pending.pop() {
        pending_bytes = pending_bytes.checked_sub(directory.capacity()).ok_or(InventoryError::Limit)?;
        resources.check()?;
        let entries = fs::read_dir(&directory).map_err(|_| InventoryError::UnsafePath)?;
        for entry in entries {
            resources.check()?;
            charge(&mut visited, 1, MAX_VISITS)?;
            let entry = entry.map_err(|_| InventoryError::UnsafePath)?;
            let name = entry.file_name();
            if matches!(name.to_str(), Some("target" | ".git" | ".cache")) { continue; }
            let path = entry.path();
            text(&path)?;
            if path.capacity() > MAX_PATH_BYTES * 2 { return Err(InventoryError::Limit); }
            let metadata = fs::symlink_metadata(&path).map_err(|_| InventoryError::UnsafePath)?;
            check_metadata(&metadata)?;
            if metadata.is_dir() {
                if pending.len() >= MAX_VISITS { return Err(InventoryError::Limit); }
                charge(&mut pending_bytes, path.capacity(), MAX_PENDING_BYTES)?;
                pending.push(path);
            } else if metadata.is_file() && (name == "Cargo.toml" || name == "Cargo.lock"
                || path.extension().and_then(|extension| extension.to_str()) == Some("rs")) {
                if inputs.len() >= MAX_INPUTS { return Err(InventoryError::Limit); }
                let relative = path.strip_prefix(root).map_err(|_| InventoryError::UnsafePath)?;
                charge(&mut key_bytes, text(relative)?.len(), MAX_KEYS_BYTES)?;
                let key = path_key(relative)?;
                match inputs.entry(key) {
                    Entry::Vacant(slot) => { slot.insert(read_digest(&path, source, runner, resources)?); }
                    Entry::Occupied(_) => return Err(InventoryError::UnsafePath),
                }
            }
        }
    }
    resources.check()?;
    // At most 256KiB keys and 4096 canonical digest strings: worst-case JSON
    // escaping, map slots, targets and paths fit the reserved logical slack.
    let build = serde_json::to_vec(&inputs).map_err(|_| InventoryError::Snapshot)?;
    if build.len() > 2 * 1024 * 1024 { return Err(InventoryError::Limit); }
    let mut targets = inputs;
    for task in &pipeline.completion_plan.tasks {
        resources.check()?;
        if task.target_ref.len() > MAX_PATH_BYTES || task.target_digest.len() > 128 {
            return Err(InventoryError::Limit);
        }
        // Overlay virtual targets only AFTER the physical build preimage, as
        // required by the inherited canonical snapshot. Do not rewrite build.
        let added = !targets.contains_key(&task.target_ref);
        if added {
            if targets.len() >= MAX_INPUTS { return Err(InventoryError::Limit); }
            charge(&mut key_bytes, task.target_ref.len(), MAX_KEYS_BYTES)?;
        }
        match targets.entry(task.target_ref.clone()) {
            Entry::Vacant(slot) => {
                slot.insert(task.target_digest.clone());
            }
            Entry::Occupied(mut slot) => { slot.insert(task.target_digest.clone()); }
        }
    }
    let targets: Vec<DgclTargetIdentity> = targets.into_iter()
        .map(|(target_ref, target_digest)| DgclTargetIdentity { target_ref, target_digest }).collect();
    let root_digest = dgcl_repository_root_digest(root).map_err(|_| InventoryError::UnsafePath)?;
    let snapshot = build_dgcl_execution_snapshot(source, &build,
        &format!("dgcl-cargo-offline-check-test.v1/{root_digest}"),
        &dgcl_validator_revision(), &format!("authority-revision:{}", authority.authority_revision.0),
        targets).map_err(|_| InventoryError::Snapshot)?;
    scratch.check()?;
    resources.check()?;
    Ok(snapshot)
}

#[cfg(test)]
mod tests;

