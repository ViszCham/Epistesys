//! Resource-bound standalone observation profiles; not host authority.
use super::{checked_request, resource_error, CommandIo};
use lc631_core::context::SourceRevision;
use lc631_runtime::resource::{ResourceKind, ResourceReservationView};

impl CommandIo {
    /// The raw config, not an unverified claimed source, pins this loan.
    /// The callback owns all artifact/probe material until it returns a small
    /// summary. This is logical admission, not a process RSS measurement.
    pub(crate) fn with_path_services(
        &self,
        source: &str,
        operation: impl FnOnce(&ResourceReservationView<'_>) -> Result<Vec<u8>, String>,
    ) -> Result<Vec<u8>, String> {
        self.with_observation_view(
            source,
            96 * 1024 * 1024,
            32 * 1024 * 1024,
            "path_summary_limit",
            operation,
        )
    }

    pub(crate) fn with_checked_tl_view(
        &self,
        source: &str,
        operation: impl FnOnce(&ResourceReservationView<'_>) -> Result<Vec<u8>, String>,
    ) -> Result<Vec<u8>, String> {
        self.with_observation_view(
            source,
            64 * 1024 * 1024,
            8 * 1024 * 1024,
            "checked_tl_summary_limit",
            operation,
        )
    }

    fn with_observation_view(
        &self,
        source: &str,
        ram: u64,
        output: u64,
        overflow: &'static str,
        operation: impl FnOnce(&ResourceReservationView<'_>) -> Result<Vec<u8>, String>,
    ) -> Result<Vec<u8>, String> {
        let summary_cap = crate::tlcx_commands::MAX_SUMMARY_BYTES as u64;
        let requests = [
            checked_request(ResourceKind::Ram, ram).map_err(resource_error)?,
            checked_request(ResourceKind::Output, output).map_err(resource_error)?,
        ];
        let mut operation_error = None;
        let result = self.with_reserved_binding(
            SourceRevision::of(source).digest(),
            summary_cap,
            summary_cap,
            &requests,
            |view| {
                let result = operation(view).and_then(|bytes| {
                    if bytes.len() > summary_cap as usize {
                        return Err(overflow.to_owned());
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
