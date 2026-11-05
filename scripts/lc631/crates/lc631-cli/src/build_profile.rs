//! Compiled feature metadata, never a hardware observation or permission.
use crate::{effect_error::CliFailure, io::CommandIo};
use serde::Serialize;

#[derive(Serialize)]
struct BuildProfile {
    schema_version: &'static str,
    version: &'static str,
    accelerator_gpu_compiled: bool,
    runtime_semantic_gpu_compiled: bool,
    gpu_execution_observed: bool,
    authority_created: bool,
}

impl BuildProfile {
    const fn compiled() -> Self {
        Self {
            schema_version: "epistesys-build-feature-view.v1",
            version: lc631_core::LC631_VERSION,
            accelerator_gpu_compiled: lc631_accelerator::is_gpu_compiled(),
            runtime_semantic_gpu_compiled: lc631_runtime::semantic_accelerator::is_gpu_compiled(),
            gpu_execution_observed: false,
            authority_created: false,
        }
    }
}

pub(crate) fn run(resources: &CommandIo, arguments: &[String]) -> Result<(), CliFailure> {
    if !arguments.is_empty() {
        return Err(CliFailure::not_attempted(
            "build_profile_unexpected_arguments",
        ));
    }
    resources
        .write_public("epistesys-build-profile", BuildProfile::compiled())
        .map_err(CliFailure::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_metadata_never_implies_hardware_or_authority() {
        let view = BuildProfile::compiled();
        if cfg!(feature = "gpu-cubecl-wgpu") {
            assert!(view.accelerator_gpu_compiled && view.runtime_semantic_gpu_compiled);
        }
        if view.runtime_semantic_gpu_compiled {
            assert!(view.accelerator_gpu_compiled);
        }
        assert!(!view.gpu_execution_observed && !view.authority_created);
        assert_eq!(view.version, env!("CARGO_PKG_VERSION"));
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["schema_version"], "epistesys-build-feature-view.v1");
        assert_eq!(json["gpu_execution_observed"], false);
    }
}
