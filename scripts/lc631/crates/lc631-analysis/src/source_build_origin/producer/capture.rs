//! 日本語: 元の宣言source producerが成功・部分captureの所有権を持つ。
//! The original declared-source producer owns every successful or partial capture.
use super::super::{BuildPlan, BuildServices};
use crate::declared_build_capture::{capture_declared_build, SnapshotCaptureAttempt};
use lc631_substrate::resource::ResourceReservationView;

pub(super) fn capture<'r>(
    plan: &BuildPlan<'_, '_>, services: &mut BuildServices<'_, '_>,
    view: &'r ResourceReservationView<'_>,
) -> SnapshotCaptureAttempt<'r> {
    // Callers check the exact plan first; keep snapshot failures in their original vocabulary.
    if let Err(error) = plan.input.binding.check_goal_profile_inputs(view) {
        return SnapshotCaptureAttempt::failure(error);
    }
    capture_declared_build(plan.input.binding, services.source_reader,
        services.capture_authority, view)
}
