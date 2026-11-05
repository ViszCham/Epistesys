//! Injected same-parent execution. No runtime reverse dependency or fallback.
use super::{BoundedToolRequest, BoundedToolResult, ResourceToolResult};
use lc631_process_supervisor::{ResourceError, ResourceReservationView};

pub trait ToolProcessRunner {
    fn run(
        &mut self,
        source: &str,
        request: BoundedToolRequest<'_>,
        consume: &mut dyn FnMut(&ResourceToolResult<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError>;
    fn with_input(
        &mut self,
        source: &str,
        cap: usize,
        consume: &mut dyn FnMut(&ResourceReservationView<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError>;
}

pub(crate) fn with_input_result<T>(
    runner: &mut dyn ToolProcessRunner,
    source: &str,
    cap: usize,
    consume: impl FnOnce(&ResourceReservationView<'_>) -> Result<T, ResourceError>,
) -> Result<T, ResourceError> {
    let mut consume = Some(consume);
    let mut value = None;
    let mut calls = 0_usize;
    runner.with_input(source, cap, &mut |view| {
        calls = calls
            .checked_add(1)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        view.check()?;
        if view.source_revision() != lc631_core::context::SourceRevision::of(source).digest() {
            return Err(ResourceError::SourceMismatch);
        }
        let consume = consume.take().ok_or(ResourceError::InvalidConfig)?;
        value = Some(consume(view)?);
        Ok(())
    })?;
    if calls != 1 {
        return Err(ResourceError::InvalidConfig);
    }
    value.ok_or(ResourceError::InvalidConfig)
}

/// A callback cannot leak the borrowed capture or silently be skipped/repeated.
/// Owned parsed results require the caller's retained payload reservation.
pub(crate) fn with_tool_result<T>(
    runner: &mut dyn ToolProcessRunner,
    source: &str,
    request: BoundedToolRequest<'_>,
    consume: impl FnOnce(&BoundedToolResult, &[u8], &[u8]) -> T,
) -> Result<T, ResourceError> {
    let mut callback = Some(consume);
    let mut value = None;
    let mut calls = 0_usize;
    let expected = super::resources::checked_request_binding(&request)?;
    let source_digest = lc631_core::context::SourceRevision::of(source).digest();
    runner.run(source, request, &mut |capture| {
        calls = calls
            .checked_add(1)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        if capture.source_revision() != source_digest || capture.invocation_digest() != expected {
            return Err(ResourceError::SourceMismatch);
        }
        let consume = callback.take().ok_or(ResourceError::InvalidConfig)?;
        value = Some(consume(
            capture.reported(),
            capture.stdout(),
            capture.stderr(),
        ));
        Ok(())
    })?;
    if calls != 1 {
        return Err(ResourceError::InvalidConfig);
    }
    value.ok_or(ResourceError::InvalidConfig)
}
