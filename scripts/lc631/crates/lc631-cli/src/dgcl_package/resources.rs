//! Explicit compatibility selection, never resource-error fallback.
use super::*;
use crate::io::CommandIo;
use lc631_analysis::{DgclSourceObserver, LanguageWorkerError, ToolProcessRunner};

#[derive(Clone, Copy)]
pub(crate) struct PackageIo<'a>(pub(crate) Option<&'a CommandIo>);
impl PackageIo<'_> {
    pub(crate) fn pipeline(self, source: &str) -> Result<DgclPipelineReport, String> {
        match self.0 {
            None => build_dgcl_pipeline(
                source,
                DgclBackendSet {
                    english: None,
                    japanese: None,
                },
            )
            .map_err(|e| format!("dgcl_package_pipeline:{e:?}")),
            Some(io) => build_package_pipeline_with_resources(source, io),
        }
    }
    pub(crate) fn preflight(
        self,
        root: &Path,
        source: &str,
        pipeline: &DgclPipelineReport,
        plan: &DgclCargoValidationPlan,
    ) -> Result<(), String> {
        match self.0 {
            None => lc631_analysis::preflight_dgcl_cargo_plan(root, pipeline, plan),
            Some(io) => lc631_analysis::preflight_dgcl_cargo_plan_with_runner(
                root,
                source,
                pipeline,
                plan,
                &mut io.analysis_runner(),
            ),
        }
        .map_err(|e| format!("dgcl_package_plan_preflight:{e:?}"))
    }
    pub(crate) fn snapshot(
        self,
        root: &Path,
        source: &str,
        pipeline: &DgclPipelineReport,
        authority: &ExecutionPermitContext<'_>,
    ) -> Result<lc631_analysis::DgclExecutionSnapshot, String> {
        match self.0 {
            None => capture_dgcl_cargo_snapshot(root, source, pipeline, authority),
            Some(io) => lc631_analysis::capture_dgcl_cargo_snapshot_with_runner(
                root,
                source,
                pipeline,
                authority,
                &mut io.analysis_runner(),
            ),
        }
        .map_err(|e| format!("dgcl_package_snapshot:{e:?}"))
    }
    pub(crate) fn observe(
        self,
        root: &Path,
        source: &str,
        pipeline: &DgclPipelineReport,
        plan: &DgclCargoValidationPlan,
        context: lc631_analysis::DgclCargoExecutionContext<'_>,
    ) -> Result<lc631_analysis::CheckedDgclCargoObservation, String> {
        match self.0 {
            None => observe_dgcl_cargo_validation(root, source, pipeline, plan, context),
            Some(io) => lc631_analysis::observe_dgcl_cargo_validation_with_runner(
                root,
                source,
                pipeline,
                plan,
                context,
                &mut io.analysis_runner(),
            ),
        }
        .map_err(|e| format!("dgcl_package_observation:{e:?}"))
    }
}

pub(crate) fn build_package_pipeline_with_resources(
    source: &str,
    resources: &CommandIo,
) -> Result<DgclPipelineReport, String> {
    struct Observer<'a>(&'a CommandIo);
    impl DgclSourceObserver for Observer<'_> {
        fn inspect(&mut self, source: &str, _language: &str) -> Result<(), LanguageWorkerError> {
            self.0
                .analysis_runner()
                .with_input(source, source.len().max(1), &mut |view| {
                    view.check()?;
                    if view.source_revision()
                        != lc631_core::context::SourceRevision::of(source).digest()
                    {
                        return Err(lc631_runtime::resource::ResourceError::SourceMismatch);
                    }
                    Ok(())
                })
                .map_err(|e| LanguageWorkerError::Resource(format!("{e:?}")))
        }
    }
    lc631_analysis::build_dgcl_pipeline_with_source_observer(source, &mut Observer(resources))
        .map_err(|e| format!("dgcl_package_pipeline:{e:?}"))
}
