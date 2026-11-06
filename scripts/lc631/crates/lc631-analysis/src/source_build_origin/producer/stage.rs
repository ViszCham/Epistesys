//! 日本語: 実childの検査済captureだけを私有witnessへ移し、不確かなownerを保持する。
//! One actual admitted compiler child. Completed, checked captures move into
//! private bounded witnesses; uncertain outcomes keep their original owners.
use super::super::{
    compiler_image::BuildImages, BuildError, BuildPlan, BuildServices, BuildStage,
    CompilerChildWitness,
};
use super::{check_authority, ensure_time};
use lc631_process_supervisor::{
    run_bounded_with_resources_attempt, NativeProcessEffect, ResourceRunAttempt, RunRequest,
};
use lc631_substrate::resource::{ResourceError, ResourceReservationView};
use std::{
    ffi::OsString,
    path::Path,
    time::{Duration, Instant},
};

pub(super) struct StageFailure<'v> {
    pub(super) cause: BuildError,
    pub(super) observed: usize,
    pub(super) witness: Option<CompilerChildWitness>,
    pub(super) partial: Option<ResourceRunAttempt<'v, BuildError>>,
}
impl StageFailure<'_> {
    fn before(cause: BuildError) -> Self {
        Self {
            cause,
            observed: 0,
            witness: None,
            partial: None,
        }
    }
}

#[expect(
    clippy::result_large_err,
    reason = "failure must retain its original partial native/IO owners"
)]
pub(super) fn dispatch<'v>(
    plan: &BuildPlan<'_, 'v>,
    services: &mut BuildServices<'_, '_>,
    images: &BuildImages,
    stage: BuildStage,
    deadline: Instant,
    view: &'v ResourceReservationView<'_>,
) -> Result<CompilerChildWitness, StageFailure<'v>> {
    let prepared = prepare(plan, stage, deadline, view).map_err(StageFailure::before)?;
    let Prepared {
        arguments,
        environment,
        scope,
        mut stdout,
        mut stderr,
    } = prepared;
    let timeout = remaining(plan, deadline, view).map_err(StageFailure::before)?;
    let mut stamp = None;
    let attempt = {
        let mut admit = || -> Result<(), BuildError> {
            ensure_time(deadline, view)?;
            images.check_inputs(plan, view)?;
            if stage == BuildStage::Link {
                images.check_output_absent(plan, view)?;
            }
            ensure_time(deadline, view)?;
            let observed = check_authority(plan, services, stage, view)?;
            ensure_time(deadline, view)?;
            stamp = Some(observed);
            Ok(())
        };
        run_bounded_with_resources_attempt(
            RunRequest {
                executable: plan.input.compiler.executable.as_os_str(),
                arguments: &arguments,
                current_dir: plan.input.frame.cwd,
                environment: &environment,
                stdin: None,
                timeout,
                stdout_limit: plan.input.limits.max_metadata_bytes,
                stderr_limit: plan.input.limits.max_metadata_bytes,
            },
            view,
            &mut admit,
        )
    };
    // This count comes from the lower actual native boundary, not Option absence.
    let observed = match attempt.effect() {
        NativeProcessEffect::NotAttempted => 0,
        NativeProcessEffect::MayHaveStarted | NativeProcessEffect::Completed => 1,
    };
    let mut cause = attempt
        .admission_error()
        .cloned()
        .or_else(|| attempt.resource_error().cloned().map(BuildError::Resource));
    let mut witness = None;
    if let (Some(raw), Some(stamp)) = (attempt.outcome(), stamp) {
        let cap = plan.input.limits.max_metadata_bytes;
        if raw.stdout.len() <= cap && raw.stderr.len() <= cap {
            stdout.extend_from_slice(&raw.stdout);
            stderr.extend_from_slice(&raw.stderr);
            witness = Some(CompilerChildWitness {
                stage,
                exit_code: raw.exit_code,
                cleanup_verified: raw.io_cleanup_complete && raw.process_tree_kill_verified,
                frame_digest: plan.frame_digest(),
                invocation_digest: plan.invocation_digest(stage),
                options_digest: plan.options_digest(stage),
                source_revision: view.source_revision(),
                compiler_digest: plan.input.compiler.expected_image,
                cwd_digest: plan.cwd_digest(),
                environment_digest: plan.environment_digest(),
                test_scope: scope,
                test_checked_epoch: stamp.epoch,
                stdout,
                stderr,
            });
        } else if cause.is_none() {
            cause = Some(
                ResourceError::Exhausted(lc631_substrate::resource::ResourceKind::Output).into(),
            );
        }
        if cause.is_none() {
            cause = validate_output(plan, stage, raw).err();
        }
    } else if cause.is_none() {
        cause = Some(BuildError::BuildFailed);
    }
    if cause.is_none() {
        cause = (|| {
            ensure_time(deadline, view)?;
            images.check_inputs(plan, view)?;
            ensure_time(deadline, view)?;
            check_authority(plan, services, stage, view)?;
            ensure_time(deadline, view)
        })()
        .err();
    }
    if let Some(cause) = cause {
        // Even an absent outcome may retain a real native/pipe partial owner.
        return Err(StageFailure {
            cause,
            observed,
            witness,
            partial: Some(attempt),
        });
    }
    let Some(witness) = witness else {
        return Err(StageFailure {
            cause: BuildError::BuildFailed,
            observed,
            witness: None,
            partial: Some(attempt),
        });
    };
    // Only this private, validated transfer releases Child/Cleanup before stage+1.
    drop(attempt);
    Ok(witness)
}

struct Prepared {
    arguments: Vec<OsString>,
    environment: Vec<(OsString, OsString)>,
    scope: String,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}
fn prepare(
    plan: &BuildPlan<'_, '_>,
    stage: BuildStage,
    deadline: Instant,
    view: &ResourceReservationView<'_>,
) -> Result<Prepared, BuildError> {
    ensure_time(deadline, view)?;
    plan.check(view)?;
    if stage == BuildStage::CurrentOrigin {
        return Err(BuildError::BuildFailed);
    }
    let source_args = plan.args(stage);
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(source_args.len())
        .map_err(|_| ResourceError::AllocationFailed)?;
    for argument in source_args {
        arguments.push(OsString::from(argument));
    }
    let mut environment = Vec::new();
    environment
        .try_reserve_exact(plan.input.frame.environment.len())
        .map_err(|_| ResourceError::AllocationFailed)?;
    for entry in plan.input.frame.environment {
        environment.push((OsString::from(entry.name), OsString::from(entry.value)));
    }
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    stdout
        .try_reserve_exact(plan.input.limits.max_metadata_bytes)
        .map_err(|_| ResourceError::AllocationFailed)?;
    stderr
        .try_reserve_exact(plan.input.limits.max_metadata_bytes)
        .map_err(|_| ResourceError::AllocationFailed)?;
    Ok(Prepared {
        arguments,
        environment,
        scope: plan.execution_scope(stage),
        stdout,
        stderr,
    })
}
fn remaining(
    plan: &BuildPlan<'_, '_>,
    deadline: Instant,
    view: &ResourceReservationView<'_>,
) -> Result<Duration, BuildError> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or(ResourceError::DeadlineExceeded)?;
    Ok(remaining
        .min(view.remaining()?)
        .min(Duration::from_millis(plan.input.limits.child_timeout_ms)))
}
fn validate_output(
    plan: &BuildPlan<'_, '_>,
    stage: BuildStage,
    raw: &lc631_process_supervisor::RunOutcome,
) -> Result<(), BuildError> {
    if raw.timed_out {
        return Err(ResourceError::DeadlineExceeded.into());
    }
    if raw.stdout_truncated || raw.stderr_truncated {
        return Err(
            ResourceError::Exhausted(lc631_substrate::resource::ResourceKind::Output).into(),
        );
    }
    if !raw.io_cleanup_complete || !raw.process_tree_kill_verified {
        return Err(ResourceError::CleanupUnverified.into());
    }
    if raw.exit_code != Some(0) || raw.diagnostic.is_some() {
        return Err(BuildError::BuildFailed);
    }
    match stage {
        BuildStage::CompilerVersion => {
            if super::super::plan::digest(&raw.stdout) != plan.input.compiler.expected_verbose {
                return Err(BuildError::CompilerRegistrationMismatch);
            }
        }
        BuildStage::CompilerSysroot => {
            let text = std::str::from_utf8(&raw.stdout)
                .map_err(|_| BuildError::CompilerRegistrationMismatch)?;
            let path = text
                .strip_suffix("\r\n")
                .or_else(|| text.strip_suffix('\n'))
                .unwrap_or(text);
            if path.is_empty()
                || path.contains(['\r', '\n'])
                || Path::new(path) != plan.input.compiler.declared_sysroot
            {
                return Err(BuildError::CompilerRegistrationMismatch);
            }
        }
        BuildStage::Link => {}
        BuildStage::CurrentOrigin => return Err(BuildError::BuildFailed),
    }
    Ok(())
}
