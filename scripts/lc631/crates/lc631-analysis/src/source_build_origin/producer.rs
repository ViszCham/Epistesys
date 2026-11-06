//! 日本語: 実3stageと前後source captureからのみoriginを構築する。
//! English: Only three actual stages and before/after captures can mint this origin.
#![forbid(unsafe_code)]
mod authority;
mod capture;
mod stage;

use super::{
    check_claim, compiler_image::BuildImages, AuthorityStamp, BuildError, BuildEvent, BuildPlan,
    BuildServices, BuildStage, OwnedBuildFailure, OwnedSourceBuildOrigin,
};
use crate::declared_build_capture::{
    OwnedDeclaredBuildCapture, SnapshotCaptureAttempt, SnapshotCaptureError,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceError, ResourceKind, ResourceRequest, ResourceReservationView,
};
use std::time::{Duration, Instant};

pub fn observe<'p, 's, 'v>(
    plan: &'p BuildPlan<'s, 'v>,
    services: &mut BuildServices<'_, '_>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedSourceBuildOrigin<'p, 's, 'v>, OwnedBuildFailure<'p, 's, 'v>> {
    let mut failure = OwnedBuildFailure {
        plan,
        cause: BuildError::BuildFailed,
        observed_children: 0,
        children: Vec::new(),
        before: None,
        after: None,
        images: None,
        partial: None,
        retained: None,
    };
    if let Err(cause) = build(plan, services, view, &mut failure) {
        failure.cause = cause;
        return Err(failure);
    }
    let parts = (
        failure.before.take(),
        failure.after.take(),
        failure.images.take(),
        failure.retained.take(),
    );
    match parts {
        (Some(before), Some(after), Some(images), Some(retained)) => Ok(OwnedSourceBuildOrigin {
            plan,
            before,
            after,
            images,
            retained,
            children: std::mem::take(&mut failure.children),
        }),
        (before, after, images, retained) => {
            failure.before = before;
            failure.after = after;
            failure.images = images;
            failure.retained = retained;
            Err(failure)
        }
    }
}

fn build<'p, 's, 'v>(
    plan: &'p BuildPlan<'s, 'v>,
    services: &mut BuildServices<'_, '_>,
    view: &'v ResourceReservationView<'_>,
    failure: &mut OwnedBuildFailure<'p, 's, 'v>,
) -> Result<(), BuildError> {
    plan.begin_build(view)?;
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(plan.input.limits.total_wall_ms))
        .ok_or(ResourceError::ArithmeticOverflow)?;
    let output = plan
        .input
        .limits
        .max_metadata_bytes
        .checked_mul(6)
        .ok_or(BuildError::InputLimit)?;
    let captures = usize::try_from(crate::declared_build_capture::DECLARED_CAPTURE_RETAINED_RAM)
        .map_err(|_| BuildError::InputLimit)?
        .checked_mul(2)
        .ok_or(BuildError::InputLimit)?;
    let fixed = usize::try_from(super::admission::PLAN_RAM)
        .map_err(|_| BuildError::InputLimit)?
        .checked_add(captures)
        .ok_or(BuildError::InputLimit)?;
    let retained = plan
        .input
        .limits
        .max_retained_bytes
        .checked_sub(fixed)
        .ok_or(BuildError::InputLimit)?;
    let minimum = output
        .checked_add(plan.input.limits.max_environment_bytes)
        .and_then(|bytes| bytes.checked_add(16 * 1024))
        .ok_or(BuildError::InputLimit)?;
    if retained < minimum {
        return Err(BuildError::InputLimit);
    }
    failure.retained = Some(view.claim_resources(&[
        request(ResourceKind::Ram, retained)?,
        request(ResourceKind::Output, output)?,
        request(ResourceKind::Disk, plan.input.limits.max_artifact_bytes)?,
    ])?);
    failure
        .children
        .try_reserve_exact(3)
        .map_err(|_| ResourceError::AllocationFailed)?;
    ensure_time(deadline, view)?;
    check_authority(plan, services, BuildStage::CompilerVersion, view)?;
    let images = BuildImages::capture_inputs(plan, view)?;
    images.check_output_absent(plan, view)?;
    failure.images = Some(images);
    ensure_time(deadline, view)?;
    let (before, error) = capture_source(plan, services, view).into_parts();
    failure.before = before;
    if let Some(error) = error {
        return Err(error.into());
    }
    check_capture(
        plan,
        failure
            .before
            .as_ref()
            .ok_or(SnapshotCaptureError::InvalidSnapshot)?,
        view,
    )?;
    ensure_time(deadline, view)?;

    for stage in [
        BuildStage::CompilerVersion,
        BuildStage::CompilerSysroot,
        BuildStage::Link,
    ] {
        ensure_time(deadline, view)?;
        if failure.observed_children >= plan.input.limits.max_children as usize {
            return Err(BuildError::BuildFailed);
        }
        let images = failure.images.as_ref().ok_or(BuildError::BuildFailed)?;
        match stage::dispatch(plan, services, images, stage, deadline, view) {
            Ok(witness) => {
                failure.observed_children += 1;
                failure.children.push(witness);
            }
            Err(attempt) => {
                failure.observed_children += attempt.observed;
                if let Some(witness) = attempt.witness {
                    failure.children.push(witness);
                }
                failure.partial = attempt.partial;
                return Err(attempt.cause);
            }
        }
        let fingerprint = if stage == BuildStage::Link {
            Some(
                failure
                    .images
                    .as_mut()
                    .ok_or(BuildError::BuildFailed)?
                    .capture_artifact(plan, view)?,
            )
        } else {
            None
        };
        if let Some(observer) = services.observer.as_deref_mut() {
            observer.observed(BuildEvent {
                stage,
                artifact_fingerprint: fingerprint,
                children: &failure.children,
            })?;
        }
        ensure_time(deadline, view)?;
    }

    failure
        .images
        .as_ref()
        .ok_or(BuildError::BuildFailed)?
        .check_current(plan, view)?;
    check_authority(plan, services, BuildStage::CurrentOrigin, view)?;
    ensure_time(deadline, view)?;
    let (after, error) = capture_source(plan, services, view).into_parts();
    failure.after = after;
    if let Some(error) = error {
        return Err(error.into());
    }
    let before = failure
        .before
        .as_ref()
        .ok_or(SnapshotCaptureError::InvalidSnapshot)?;
    let after = failure
        .after
        .as_ref()
        .ok_or(SnapshotCaptureError::InvalidSnapshot)?;
    check_capture(plan, before, view)?;
    check_capture(plan, after, view)?;
    if before.content_pins() != after.content_pins() {
        return Err(BuildError::SourceDrift);
    }
    failure
        .images
        .as_ref()
        .ok_or(BuildError::BuildFailed)?
        .check_current(plan, view)?;
    check_authority(plan, services, BuildStage::CurrentOrigin, view)?;
    check_claim(
        failure.retained.as_ref().ok_or(BuildError::BuildFailed)?,
        view,
    )?;
    ensure_time(deadline, view)
}

pub(super) fn capture_source<'r>(
    plan: &BuildPlan<'_, '_>,
    services: &mut BuildServices<'_, '_>,
    view: &'r ResourceReservationView<'_>,
) -> SnapshotCaptureAttempt<'r> {
    capture::capture(plan, services, view)
}
pub(super) fn check_authority(
    plan: &BuildPlan<'_, '_>,
    services: &mut BuildServices<'_, '_>,
    stage: BuildStage,
    view: &ResourceReservationView<'_>,
) -> Result<AuthorityStamp, BuildError> {
    authority::check(plan, services, stage, view)
}
pub(super) fn ensure_time(
    deadline: Instant,
    view: &ResourceReservationView<'_>,
) -> Result<(), BuildError> {
    view.check()?;
    if Instant::now() >= deadline {
        return Err(ResourceError::DeadlineExceeded.into());
    }
    Ok(())
}
fn request(kind: ResourceKind, bytes: usize) -> Result<ResourceRequest, BuildError> {
    let bytes = u64::try_from(bytes).map_err(|_| ResourceError::ArithmeticOverflow)?;
    Ok(ResourceRequest::new(kind, ResourceAmount::checked(bytes)?))
}
fn check_capture(
    plan: &BuildPlan<'_, '_>,
    capture: &OwnedDeclaredBuildCapture<'_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), BuildError> {
    capture.check_current(view)?;
    if capture.root() != plan.root_digest()
        || capture.program() != plan.program_digest()
        || capture.plan() != plan.plan_digest()
        || capture.content_pins().source != plan.input.binding.source()
    {
        return Err(BuildError::FrameMismatch);
    }
    Ok(())
}
