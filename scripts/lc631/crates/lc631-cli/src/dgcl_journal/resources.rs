//! CLI orchestration owns persistence; analysis receives only a borrowed view.
use super::*;
use crate::io::CommandIo;
use lc631_host::{HostPersistentStoreOwner, PersistentStoreKind, PersistentStoreLimits};
use lc631_runtime::resource::ResourceReservationView;

const RAM: u64 = 1024 * 1024 + 32768;

pub(crate) fn read<T>(
    binding: Option<(&CommandIo, &str)>,
    operation: impl FnOnce(Option<&ResourceReservationView<'_>>) -> Result<T, String>,
) -> Result<T, String> {
    match binding {
        None => operation(None),
        Some((io, source)) => io.with_journal_view(source, RAM, 0, |view| operation(Some(view))),
    }
}

pub(crate) fn write<T>(
    binding: (&CommandIo, &str),
    root: &Path,
    target: &Path,
    kind: PersistentStoreKind,
    operation: impl FnOnce(&ResourceReservationView<'_>) -> Result<T, String>,
) -> Result<T, String> {
    let (io, source) = binding;
    io.with_journal_view(source, RAM, 16384, |view| {
        let mut owner = HostPersistentStoreOwner::begin(
            root,
            target,
            kind,
            16384,
            PersistentStoreLimits {
                max_files: 1002,
                max_total_bytes: 16 * 1024 * 1024,
                max_depth: 1,
            },
            view,
        )
        .map_err(|e| format!("repair_journal_owner:{e:?}"))?;
        if !owner.target_exists() {
            owner
                .before_write(16384)
                .map_err(|e| format!("repair_journal_admission:{e:?}"))?;
        }
        let value = operation(view)?;
        // Re-open and hash the actual synced bytes. A caller success boolean
        // or body digest is not a durable raw-file handoff witness.
        let mut file =
            std::fs::File::open(target).map_err(|e| format!("repair_journal_handoff_read:{e}"))?;
        let raw = read_checkpoint_record_with_resources(&mut file, view.source_revision(), view)
            .map_err(|e| format!("repair_journal_handoff_bytes:{e:?}"))?;
        let digest = stable_sha256(
            raw.as_str()
                .map_err(|e| format!("repair_journal_handoff_utf8:{e:?}"))?,
        );
        drop(raw);
        // Windows validates the reopened target with an exclusive handle. Do
        // not retain this read handle across the owner-bound handoff check.
        drop(file);
        owner
            .confirm_handoff(&digest)
            .map_err(|e| format!("repair_journal_handoff:{e:?}"))?;
        Ok(value)
    })
}
