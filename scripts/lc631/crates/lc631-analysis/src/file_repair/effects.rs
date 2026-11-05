//! A borrowed effect plan is not authority or a persistence receipt.
use super::*;
use lc631_process_supervisor::filesystem::AnchoredFile;
use lc631_process_supervisor::{ResourceKind, ResourceReservationView};

pub struct DgclFileEffectPlan<'a> {
    pub source: &'a str,
    pub repository_root: &'a Path,
    pub target_relative: &'a Path,
    pub backup_relative: &'a Path,
    pub before: &'a [u8],
    pub replacement: &'a [u8],
}
/// Admit common-run Disk, own the durable backup, and verify raw persisted
/// bytes after the exactly-once callback. No evidence or authority is issued.
pub trait DgclFileEffectOwner {
    fn apply(
        &mut self,
        plan: DgclFileEffectPlan<'_>,
        operation: &mut dyn FnMut(
            &ResourceReservationView<'_>,
        ) -> Result<(), DgclRepairDriverError>,
    ) -> Result<(), DgclRepairDriverError>;
}
pub(super) struct WriteCommit<'a> {
    pub root: &'a DirectoryAnchor,
    pub target: &'a mut Option<AnchoredFile>,
    pub before: &'a [u8],
    pub replacement: &'a [u8],
    pub backup_name: &'a Path,
    pub authorized: &'a dyn Fn() -> bool,
}
impl WriteCommit<'_> {
    pub(super) fn write(
        &mut self,
        source: &str,
        view: Option<&ResourceReservationView<'_>>,
    ) -> Result<(), DgclRepairDriverError> {
        let fail = DgclRepairDriverError::Failed;
        let _disk = if let Some(view) = view {
            use lc631_substrate::resource::{ResourceAmount, ResourceRequest};
            view.check().map_err(|_| fail)?;
            if view.source_revision() != lc631_core::context::SourceRevision::of(source).digest() {
                return Err(fail);
            }
            let bytes = self
                .before
                .len()
                .checked_add(self.replacement.len())
                .and_then(|n| u64::try_from(n).ok())
                .ok_or(fail)?;
            Some(
                view.claim_resources(&[ResourceRequest::new(
                    ResourceKind::Disk,
                    ResourceAmount::checked(bytes).map_err(|_| fail)?,
                )])
                .map_err(|_| fail)?,
            )
        } else {
            None
        };
        if !(self.authorized)() {
            return Err(fail);
        }
        // No backup directory/target effect precedes source, permit and Disk admission.
        let result = (|| {
            let backups = self
                .root
                .directory(Path::new(".dgcl-repair-backups"), true)
                .map_err(|_| fail)?;
            let mut backup = backups
                .create_private_file(self.backup_name)
                .map_err(|_| fail)?;
            backup.write_and_sync(self.before).map_err(|_| fail)?;
            if let Some(view) = view {
                view.check().map_err(|_| fail)?;
            }
            if !(self.authorized)() {
                return Err(fail);
            }
            self.target
                .as_mut()
                .ok_or(fail)?
                .compare_and_write(self.before, self.replacement)
                .map_err(|_| fail)?;
            // Release the real RW handle before the owner re-opens this file
            // no-follow to verify exact persisted replacement bytes on Windows.
            drop(self.target.take());
            if let Some(view) = view {
                view.check().map_err(|_| fail)?;
            }
            Ok(())
        })();
        if result.is_err() {
            if let Some(view) = view {
                view.record_cleanup_unknown();
            }
        }
        result
    }
}
pub(super) fn current_authority<'a>(
    config: &'a DgclFileRepairConfig<'_>,
    started: Instant,
    permit: &'a ExecutionPermit,
    scope: &'a str,
) -> ExecutionPermitContext<'a> {
    let logical = config
        .authorization
        .now_epoch
        .saturating_add(started.elapsed().as_secs());
    let now = config
        .epoch_clock
        .map_or(logical, |clock| clock().max(logical));
    ExecutionPermitContext {
        principal: permit.principal_binding(),
        action: Action::Edit,
        scope,
        source_revision: &config.pipeline.source_revision,
        authority_revision: config.authorization.authority_revision,
        now_epoch: now,
        revocation_revision: config.authorization.revocation_revision,
        revoked_permit_digests: config.authorization.revoked_permit_digests,
        caller_origin: CallerOrigin::HostVerifiedUser,
        trusted_host_fingerprint: config.authorization.trusted_host_fingerprint,
    }
}
