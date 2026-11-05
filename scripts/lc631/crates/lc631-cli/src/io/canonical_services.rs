//! Source-bound service lending under the existing command owner.
use super::{checked_request, resource_error, CommandIo};
use lc631_core::context::SourceRevision;
use lc631_runtime::resource::{
    ResourceBroker, ResourceKind, ResourceReservationView, RunResources,
};

impl CommandIo {
    /// Explicit local archive quota is not execution or storage authority.
    /// Ordinary ephemeral services keep their original reservations unchanged.
    pub(crate) fn with_canonical_services_for_archive(
        &self,
        source: &str,
        archive: bool,
        operation: impl FnOnce(
            &ResourceBroker,
            &RunResources,
            &ResourceReservationView<'_>,
        ) -> Result<Vec<u8>, String>,
    ) -> Result<Vec<u8>, String> {
        if archive {
            self.with_canonical_artifact_services(
                SourceRevision::of(source).digest(),
                true,
                operation,
            )
        } else {
            self.with_canonical_services(source, operation)
        }
    }

    /// Borrows this SAME command owner/run, bound before opening a stored file.
    /// The caller must independently authorize the exact named local root.
    pub(crate) fn with_canonical_artifact_services(
        &self,
        source: lc631_runtime::resource::Digest32,
        write: bool,
        operation: impl FnOnce(
            &ResourceBroker,
            &RunResources,
            &ResourceReservationView<'_>,
        ) -> Result<Vec<u8>, String>,
    ) -> Result<Vec<u8>, String> {
        let cap = crate::tlcx_commands::MAX_SUMMARY_BYTES as u64;
        let mut requests = vec![
            checked_request(ResourceKind::Ram, 64 * 1024 * 1024).map_err(resource_error)?,
            checked_request(ResourceKind::Output, 8 * 1024 * 1024).map_err(resource_error)?,
            checked_request(ResourceKind::Child, 2).map_err(resource_error)?,
            checked_request(ResourceKind::Cleanup, 1024 * 1024).map_err(resource_error)?,
        ];
        if write {
            requests.push(
                checked_request(
                    ResourceKind::Disk,
                    (lc631_runtime::canonical_solve::replay::MAX_REPLAY_BYTES * 2) as u64,
                )
                .map_err(resource_error)?,
            );
        }
        let mut failure = None;
        let result = self.with_reserved_binding(source, cap, cap, &requests, |view| {
            let result = operation(&self.owner, &self.run, view).and_then(|bytes| {
                if bytes.len() > cap as usize {
                    return Err("canonical_artifact:SummaryLimit".to_owned());
                }
                view.check().map_err(resource_error)?;
                Ok(bytes)
            });
            if let Err(error) = &result {
                failure = Some(error.clone());
            }
            Ok(result)
        });
        match result {
            Ok(result) => result,
            Err(cleanup) => match failure {
                Some(operation) => Err(format!("{operation}; io_resource_cleanup:{cleanup:?}")),
                None => Err(resource_error(cleanup)),
            },
        }
    }

    /// Only a bounded owned summary may escape. Native/model owners die in the
    /// callback before its source-bound reservation is released. The existing
    /// run retains the separate summary allowance through the final write.
    pub(crate) fn with_canonical_services(
        &self,
        source: &str,
        operation: impl FnOnce(
            &ResourceBroker,
            &RunResources,
            &ResourceReservationView<'_>,
        ) -> Result<Vec<u8>, String>,
    ) -> Result<Vec<u8>, String> {
        let summary_cap = crate::tlcx_commands::MAX_SUMMARY_BYTES as u64;
        let requests = [
            checked_request(ResourceKind::Ram, 64 * 1024 * 1024).map_err(resource_error)?,
            checked_request(ResourceKind::Output, 8 * 1024 * 1024).map_err(resource_error)?,
            checked_request(ResourceKind::Child, 2).map_err(resource_error)?,
            checked_request(ResourceKind::Cleanup, 1024 * 1024).map_err(resource_error)?,
        ];
        let mut operation_error = None;
        let result = self.with_reserved_binding(
            SourceRevision::of(source).digest(),
            summary_cap,
            summary_cap,
            &requests,
            |view| {
                let result = operation(&self.owner, &self.run, view).and_then(|bytes| {
                    if bytes.len() > summary_cap as usize {
                        return Err("canonical_summary_limit".to_owned());
                    }
                    view.check().map_err(resource_error)?;
                    Ok(bytes)
                });
                if let Err(error) = &result {
                    operation_error = Some(error.clone());
                }
                Ok(result)
            },
        );
        match result {
            Ok(result) => result,
            Err(cleanup) => match operation_error {
                Some(operation) => Err(format!("{operation}; io_resource_cleanup:{cleanup:?}")),
                None => Err(resource_error(cleanup)),
            },
        }
    }
}
