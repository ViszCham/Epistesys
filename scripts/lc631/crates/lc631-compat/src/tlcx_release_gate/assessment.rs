use super::*;

pub fn assess(
    plan: &ReleasePlan<'_>,
    artifacts: &[ObservedArtifact],
    records: &[ReportedGateRecord],
    now: u64,
) -> Result<CandidateInspection, GateError> {
    validate_inputs(plan, artifacts, records)?;
    let artifacts = inspect_artifacts(plan, artifacts);
    let mut profiles = Vec::new();
    profiles
        .try_reserve_exact(plan.required.len())
        .map_err(|_| GateError::AllocationFailed)?;
    for key in plan.required {
        let status = match records.iter().find(|record| record.key == *key) {
            None if key.gate.requires_host() => ProfileStatus::HostPending,
            None => ProfileStatus::Missing,
            Some(record) => profile_status(plan, record, now),
        };
        profiles.push(ProfileCheck { key: *key, status });
    }
    let invalid_artifact = artifacts
        .iter()
        .any(|a| a.status != ArtifactStatus::BytesMatch);
    let invalid_record = profiles.iter().any(|r| invalid_record(r.status));
    let disposition = if invalid_artifact || invalid_record {
        CandidateDisposition::Blocked
    } else {
        CandidateDisposition::CandidateNotFullyReleased
    };
    Ok(CandidateInspection {
        candidate: plan.identity.digest(),
        artifacts,
        profiles,
        disposition,
    })
}

fn validate_inputs(
    plan: &ReleasePlan<'_>,
    artifacts: &[ObservedArtifact],
    records: &[ReportedGateRecord],
) -> Result<(), GateError> {
    if artifacts.len() > 5 {
        return Err(GateError::DuplicateArtifact);
    }
    if records.len() > 256 {
        return Err(GateError::RecordLimit);
    }
    for (index, artifact) in artifacts.iter().enumerate() {
        if artifacts[..index]
            .iter()
            .any(|a| a.kind() == artifact.kind())
        {
            return Err(GateError::DuplicateArtifact);
        }
    }
    for (index, record) in records.iter().enumerate() {
        if !plan.required.contains(&record.key) {
            return Err(GateError::UnregisteredProfile);
        }
        if records[..index].iter().any(|r| r.key == record.key) {
            return Err(GateError::DuplicateRecord);
        }
    }
    Ok(())
}

fn inspect_artifacts(plan: &ReleasePlan<'_>, artifacts: &[ObservedArtifact]) -> [ArtifactCheck; 5] {
    ArtifactKind::ALL.map(|kind| {
        let expected = plan.identity.pins()[kind as usize].digest;
        let status = match artifacts.iter().find(|a| a.kind() == kind) {
            None => ArtifactStatus::Missing,
            Some(actual) if actual.digest() == expected => ArtifactStatus::BytesMatch,
            Some(_) => ArtifactStatus::DigestMismatch,
        };
        ArtifactCheck { kind, status }
    })
}

fn invalid_record(status: ProfileStatus) -> bool {
    matches!(
        status,
        ProfileStatus::Failed
            | ProfileStatus::Skipped
            | ProfileStatus::Stale
            | ProfileStatus::FutureObservation
            | ProfileStatus::InvalidTimeWindow
            | ProfileStatus::SnapshotMismatch
            | ProfileStatus::CandidateMismatch
            | ProfileStatus::RunMismatch
    )
}

fn profile_status(plan: &ReleasePlan<'_>, record: &ReportedGateRecord, now: u64) -> ProfileStatus {
    if record.candidate != plan.identity.digest() {
        return ProfileStatus::CandidateMismatch;
    }
    if record.source_snapshot != plan.identity.source_snapshot() {
        return ProfileStatus::SnapshotMismatch;
    }
    if record.run != plan.run {
        return ProfileStatus::RunMismatch;
    }
    if record.observed_at >= record.expires_at {
        return ProfileStatus::InvalidTimeWindow;
    }
    let Some(age) = now.checked_sub(record.observed_at) else {
        return ProfileStatus::FutureObservation;
    };
    if now >= record.expires_at || age > plan.max_age {
        return ProfileStatus::Stale;
    }
    match record.outcome {
        ReportedOutcome::Failed => ProfileStatus::Failed,
        ReportedOutcome::Skipped => ProfileStatus::Skipped,
        ReportedOutcome::Unsupported => ProfileStatus::Unsupported,
        ReportedOutcome::Unavailable if record.key.gate.requires_host() => {
            ProfileStatus::HostPending
        }
        ReportedOutcome::Unavailable => ProfileStatus::Unavailable,
        ReportedOutcome::Passed if record.receipt.is_none() => ProfileStatus::MissingReceipt,
        ReportedOutcome::Passed => ProfileStatus::ReportedPassNeedsVerification,
    }
}
