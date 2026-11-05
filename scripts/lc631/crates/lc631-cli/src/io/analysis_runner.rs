//! Typed producer ports backed by one parent TaskRun; no legacy fallback.
use super::CommandIo;
use lc631_analysis::{
    BoundedToolRequest, DgclWorkerRunner, LanguageObservation, LanguageWorkerConfig,
    LanguageWorkerError, ResourceToolResult, ToolProcessRunner,
};
use lc631_process_supervisor::{ResourceError, ResourceReservationView};
use std::io::Read;

pub(crate) struct AnalysisRunner<'a> {
    resources: &'a CommandIo,
}
impl<'a> AnalysisRunner<'a> {
    pub(super) fn new(resources: &'a CommandIo) -> Self {
        Self { resources }
    }
}

pub(super) fn baseline_guard(
    resources: &CommandIo,
    root: &std::path::Path,
) -> Result<lc631_compat::BaselineGuardReport, String> {
    lc631_compat::audit_v630_root_with_reader(root, &mut |path: &std::path::Path, cap| {
        let mut file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                ) =>
            {
                return Ok(None)
            }
            Err(error) => return Err(format!("compat_file_open:{:?}", error.kind())),
        };
        let source = format!("frozen-compat-file:{}", path.display());
        let observation = resources
            .with_source_input_resources(&source, 8192, |view| {
                let claim = view.claim_resources(&[
                    lc631_process_supervisor::ResourceRequest::new(
                        lc631_process_supervisor::ResourceKind::Ram,
                        lc631_process_supervisor::ResourceAmount::checked(8192)?,
                    ),
                    lc631_process_supervisor::ResourceRequest::new(
                        lc631_process_supervisor::ResourceKind::Output,
                        lc631_process_supervisor::ResourceAmount::checked(8192)?,
                    ),
                ])?;
                let mut reader = CheckedRead {
                    inner: &mut file,
                    view,
                    failure: None,
                };
                let result = lc631_compat::hash_capped_reader(&mut reader, cap);
                if let Some(error) = reader.failure {
                    return Err(error);
                }
                drop(claim);
                classify_baseline_read(result)
            })
            .map_err(|error| format!("compat_resource:{error:?}"))?;
        match observation {
            BaselineRead::Digest(digest) => Ok(Some(digest)),
            BaselineRead::Unavailable => Ok(None),
            BaselineRead::ReadCallLimit => Err("compat_reader:ReadCallLimit".to_owned()),
        }
    })
}

/// Read-call work is not an output-byte quantity. Keep its cause separate from
/// both missing optional metadata and the existing byte-budget resource error.
enum BaselineRead {
    Digest(String),
    Unavailable,
    ReadCallLimit,
}

fn classify_baseline_read(
    result: Result<String, lc631_compat::CompatError>,
) -> Result<BaselineRead, ResourceError> {
    use lc631_compat::CompatError;
    match result {
        Ok(digest) => Ok(BaselineRead::Digest(digest)),
        Err(CompatError::ReadCallLimit) => Ok(BaselineRead::ReadCallLimit),
        Err(CompatError::BudgetExceeded) => Err(ResourceError::Exhausted(
            lc631_process_supervisor::ResourceKind::Output,
        )),
        Err(CompatError::InvalidBudget) => Err(ResourceError::InvalidConfig),
        Err(CompatError::ReaderFailed | CompatError::ReadFailed(_)) => {
            Ok(BaselineRead::Unavailable)
        }
    }
}

#[cfg(test)]
mod baseline_read_tests {
    use super::*;
    use lc631_compat::CompatError;

    #[test]
    fn tlcx95_call_limit_is_not_missing_metadata_or_output_bytes() {
        assert!(matches!(
            classify_baseline_read(Err(CompatError::ReadCallLimit)),
            Ok(BaselineRead::ReadCallLimit)
        ));
        assert!(matches!(
            classify_baseline_read(Err(CompatError::ReaderFailed)),
            Ok(BaselineRead::Unavailable)
        ));
        assert!(matches!(
            classify_baseline_read(Err(CompatError::BudgetExceeded)),
            Err(ResourceError::Exhausted(
                lc631_process_supervisor::ResourceKind::Output
            ))
        ));
        assert!(matches!(
            classify_baseline_read(Err(CompatError::InvalidBudget)),
            Err(ResourceError::InvalidConfig)
        ));
        assert!(matches!(
            classify_baseline_read(Ok("observed".to_owned())),
            Ok(BaselineRead::Digest(digest)) if digest == "observed"
        ));
    }
}

struct CheckedRead<'a, R> {
    inner: &'a mut R,
    view: &'a ResourceReservationView<'a>,
    failure: Option<ResourceError>,
}
impl<R: Read> Read for CheckedRead<'_, R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if let Err(error) = self.view.check() {
            self.failure = Some(error);
            return Err(std::io::Error::other("resource input unavailable"));
        }
        let result = self.inner.read(bytes);
        if let Err(error) = self.view.check() {
            self.failure = Some(error);
            return Err(std::io::Error::other("resource input unavailable"));
        }
        result
    }
}

impl DgclWorkerRunner for AnalysisRunner<'_> {
    fn run(
        &mut self,
        source: &str,
        language: &str,
        config: &LanguageWorkerConfig,
    ) -> Result<LanguageObservation, LanguageWorkerError> {
        // Keep the worker's domain error intact. Only outer admission/cleanup
        // failures become Resource; invalid language/framing is not a quota.
        self.resources
            .with_source_process_resources(source, 2_097_156 + 4_096, |view| {
                Ok(lc631_analysis::run_language_worker_with_resources(
                    source, language, config, view,
                ))
            })
            .map_err(|error| LanguageWorkerError::Resource(format!("{error:?}")))?
    }
}

impl ToolProcessRunner for AnalysisRunner<'_> {
    fn run(
        &mut self,
        source: &str,
        request: BoundedToolRequest<'_>,
        consume: &mut dyn FnMut(&ResourceToolResult<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        let output = request
            .stdout_limit
            .checked_add(request.stderr_limit)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        let output = u64::try_from(output.max(1)).map_err(|_| ResourceError::ArithmeticOverflow)?;
        self.resources
            .with_source_process_resources(source, output, |view| {
                let capture = lc631_analysis::run_bounded_tool_with_resources(request, view)?;
                consume(&capture)
            })
    }

    fn with_input(
        &mut self,
        source: &str,
        cap: usize,
        consume: &mut dyn FnMut(&ResourceReservationView<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        self.resources
            .with_source_input_resources(source, cap, |view| consume(view))
    }
}

pub(crate) struct FileEffectOwner<'a> {
    resources: &'a CommandIo,
}
impl<'a> FileEffectOwner<'a> {
    pub(super) fn new(resources: &'a CommandIo) -> Self {
        Self { resources }
    }
}
impl lc631_analysis::DgclFileEffectOwner for FileEffectOwner<'_> {
    fn apply(
        &mut self,
        plan: lc631_analysis::DgclFileEffectPlan<'_>,
        operation: &mut dyn FnMut(
            &ResourceReservationView<'_>,
        ) -> Result<(), lc631_analysis::DgclRepairDriverError>,
    ) -> Result<(), lc631_analysis::DgclRepairDriverError> {
        use lc631_analysis::DgclRepairDriverError as Failure;
        use lc631_host::persistent_owner::{
            HostPersistentStoreOwner, PersistentStoreKind, PersistentStoreLimits,
        };
        use lc631_process_supervisor::filesystem::DirectoryAnchor;
        if plan.before.len() > 1024 * 1024
            || plan.replacement.len() > 1024 * 1024
            || !plan
                .target_relative
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
        {
            return Err(Failure::Unavailable);
        }
        let disk = plan
            .before
            .len()
            .max(1)
            .checked_add(plan.before.len())
            .and_then(|bytes| bytes.checked_add(plan.replacement.len()))
            .and_then(|bytes| u64::try_from(bytes.max(1)).ok())
            .ok_or(Failure::Unavailable)?;
        let mut attempted = false;
        let result =
            self.resources
                .with_effect_resources(plan.source, 4 * 1024 * 1024, disk, |view| {
                    let backup_root = plan.repository_root.join(".dgcl-repair-backups");
                    let backup = backup_root.join(plan.backup_relative);
                    let mut owner = match HostPersistentStoreOwner::begin(
                        &backup_root,
                        &backup,
                        PersistentStoreKind::RepairBackup,
                        (plan.before.len() as u64).max(1),
                        PersistentStoreLimits::default(),
                        view,
                    ) {
                        Ok(owner) => owner,
                        Err(_) => return Ok(Err(Failure::Unavailable)),
                    };
                    owner
                        .before_write(plan.before.len() as u64)
                        .map_err(|_| ResourceError::InvalidConfig)?;
                    attempted = true;
                    if let Err(error) = operation(view) {
                        view.record_cleanup_unknown();
                        return Ok(Err(error));
                    }
                    let root = DirectoryAnchor::open_existing(plan.repository_root)
                        .map_err(|error| ResourceError::Io(error.kind()))?;
                    let mut target = root
                        .open_read_only(plan.target_relative)
                        .map_err(|error| ResourceError::Io(error.kind()))?;
                    let read_claim = view.claim_resources(&[
                        lc631_process_supervisor::ResourceRequest::new(
                            lc631_process_supervisor::ResourceKind::Ram,
                            lc631_process_supervisor::ResourceAmount::checked(1024 * 1024)?,
                        ),
                        lc631_process_supervisor::ResourceRequest::new(
                            lc631_process_supervisor::ResourceKind::Output,
                            lc631_process_supervisor::ResourceAmount::checked(1024 * 1024)?,
                        ),
                    ])?;
                    let actual = target
                        .read_bounded(1024 * 1024)
                        .map_err(|error| ResourceError::Io(error.kind()))?;
                    if actual != plan.replacement {
                        view.record_cleanup_unknown();
                        return Ok(Err(Failure::Failed));
                    }
                    target
                        .validate_current()
                        .map_err(|error| ResourceError::Io(error.kind()))?;
                    drop(actual);
                    drop(target);
                    drop(read_claim);
                    let before = std::str::from_utf8(plan.before)
                        .map_err(|_| ResourceError::InvalidConfig)?;
                    owner
                        .confirm_handoff(&lc631_core::stable_sha256(before))
                        .map_err(|_| ResourceError::CleanupUnverified)?;
                    Ok(Ok(()))
                });
        match result {
            Ok(domain) => domain,
            Err(ResourceError::Cancelled | ResourceError::DeadlineExceeded) => {
                Err(Failure::TimedOut)
            }
            Err(ResourceError::CleanupUnverified) => Err(Failure::Failed),
            Err(_) if attempted => Err(Failure::Failed),
            Err(_) => Err(Failure::Unavailable),
        }
    }
}
