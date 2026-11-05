//! The legacy route is explicit; injected resource paths never fall back to it.
use super::*;
use crate::process::{with_input_result, with_tool_result, ToolProcessRunner};
use lc631_process_supervisor::ResourceError;

pub(super) enum ProducerIo<'a> {
    Legacy,
    Injected {
        source: &'a str,
        runner: &'a mut dyn ToolProcessRunner,
    },
}
impl ProducerIo<'_> {
    pub(super) fn check(&mut self) -> Result<(), DgclValidationProducerError> {
        match self {
            Self::Legacy => Ok(()),
            Self::Injected { source, runner } => {
                with_input_result(*runner, source, 1, |view| view.check()).map_err(resource_error)
            }
        }
    }
    pub(super) fn digest(&mut self, path: &Path) -> Result<String, DgclValidationProducerError> {
        match self {
            Self::Legacy => digest_validation_file(path),
            Self::Injected { source, runner } => with_input_result(*runner, source, 8192, |view| {
                crate::bounded_input::hash_capped_typed(path, 128 * 1024 * 1024, Some(view))
            })
            .map_err(resource_error),
        }
    }
    pub(super) fn connection(
        &mut self,
        root: &Path,
        spec: &crate::DgclConnectionSpec,
    ) -> Result<(), DgclValidationProducerError> {
        match self {
            Self::Legacy => crate::production_connection::validate_connection_source(root, spec),
            Self::Injected { source, runner } => {
                crate::production_connection::validate_connection_source_with_reader(
                    root,
                    spec,
                    &mut |path| {
                        with_input_result(*runner, source, 16 * 1024, |view| {
                            let text = crate::bounded_input::read_resource_text_file(
                                path,
                                16 * 1024,
                                view,
                            )?;
                            Ok(text.as_str()?.to_owned())
                        })
                        .map_err(resource_error)
                    },
                )
            }
        }
    }
    pub(super) fn tool<T>(
        &mut self,
        request: BoundedToolRequest<'_>,
        consume: impl FnOnce(&BoundedToolResult, &[u8], &[u8]) -> T,
    ) -> Result<T, DgclValidationProducerError> {
        match self {
            Self::Legacy => {
                let result = run_bounded_tool(request);
                Ok(consume(&result, &result.stdout, &result.stderr))
            }
            Self::Injected { source, runner } => {
                with_tool_result(*runner, source, request, consume).map_err(resource_error)
            }
        }
    }
}

fn resource_error(error: ResourceError) -> DgclValidationProducerError {
    DgclValidationProducerError::Resource(format!("{error:?}"))
}
