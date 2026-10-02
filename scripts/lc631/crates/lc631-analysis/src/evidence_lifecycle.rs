use lc631_core::stable_sha256;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const DGCL_EVIDENCE_LIFECYCLE_SCHEMA: &str = "epistesys-dgcl-evidence-lifecycle.v1";
const MAX_BUILD_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_TARGETS: usize = 4_096;
const MAX_LEASE_SECONDS: u64 = 300;
const MAX_SOURCE_BYTES: usize = 262_144;
const LIFECYCLE_OBSERVED_BOUNDARY: &str = "hashes caller-supplied source/build/profile/validator/authority/target bytes or identities; does not isolate filesystem writes, authenticate the caller, or prove host authority";

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DgclTargetIdentity {
    pub target_ref: String,
    pub target_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DgclExecutionSnapshot {
    pub schema_version: String,
    pub source_revision: String,
    pub build_input_digest: String,
    pub profile_digest: String,
    pub validator_digest: String,
    pub authority_revision_digest: String,
    pub targets_digest: String,
    pub targets: Vec<DgclTargetIdentity>,
    pub snapshot_digest: String,
    pub observed_boundary: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DgclEvidenceLease {
    pub schema_version: String,
    pub snapshot_digest: String,
    pub source_revision: String,
    pub build_input_digest: String,
    pub profile_digest: String,
    pub validator_digest: String,
    pub authority_revision_digest: String,
    pub targets_digest: String,
    pub issued_at_epoch: u64,
    pub expires_at_epoch: u64,
    pub lease_digest: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DgclLifecycleState {
    Current,
    Expired,
    InvalidLease,
    StaleSource,
    StaleBuild,
    StaleProfile,
    StaleValidator,
    StaleAuthority,
    StaleTargets,
    TargetNotBound,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclLifecycleObservation {
    pub state: DgclLifecycleState,
    pub expected_snapshot_digest: String,
    pub observed_snapshot_digest: String,
    pub lease_digest: String,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum DgclLifecycleError {
    BuildInputTooLarge,
    SourceTooLarge,
    ProfileRevisionInvalid,
    ValidatorRevisionInvalid,
    AuthorityRevisionInvalid,
    TargetBudgetExceeded,
    TargetInvalid,
    DuplicateTarget,
    LeaseDurationInvalid,
    LeaseTimeOverflow,
    InvalidSnapshot,
    TargetTransitionInvalid,
}

pub fn build_dgcl_execution_snapshot(
    source: &str,
    build_inputs: &[u8],
    profile_revision: &str,
    validator_revision: &str,
    authority_revision: &str,
    mut targets: Vec<DgclTargetIdentity>,
) -> Result<DgclExecutionSnapshot, DgclLifecycleError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(DgclLifecycleError::SourceTooLarge);
    }
    if build_inputs.len() > MAX_BUILD_INPUT_BYTES {
        return Err(DgclLifecycleError::BuildInputTooLarge);
    }
    validate_revision(profile_revision).map_err(|_| DgclLifecycleError::ProfileRevisionInvalid)?;
    validate_revision(validator_revision)
        .map_err(|_| DgclLifecycleError::ValidatorRevisionInvalid)?;
    validate_revision(authority_revision)
        .map_err(|_| DgclLifecycleError::AuthorityRevisionInvalid)?;
    if targets.len() > MAX_TARGETS {
        return Err(DgclLifecycleError::TargetBudgetExceeded);
    }
    let mut unique = BTreeMap::new();
    for target in &targets {
        if target.target_ref.trim().is_empty()
            || target.target_ref.len() > 1_024
            || target.target_ref.chars().any(char::is_control)
            || !valid_digest(&target.target_digest)
        {
            return Err(DgclLifecycleError::TargetInvalid);
        }
        if unique
            .insert(target.target_ref.clone(), target.target_digest.clone())
            .is_some()
        {
            return Err(DgclLifecycleError::DuplicateTarget);
        }
    }
    targets.sort();
    let source_revision = stable_sha256(source);
    let build_input_digest = digest_bytes(build_inputs);
    let profile_digest = stable_sha256(profile_revision);
    let validator_digest = stable_sha256(validator_revision);
    let authority_revision_digest = stable_sha256(authority_revision);
    let targets_digest =
        stable_sha256(&serde_json::to_string(&targets).expect("validated targets serialize"));
    let snapshot_digest = lifecycle_snapshot_digest(
        &source_revision,
        &build_input_digest,
        &profile_digest,
        &validator_digest,
        &authority_revision_digest,
        &targets_digest,
    );
    Ok(DgclExecutionSnapshot {
        schema_version: DGCL_EVIDENCE_LIFECYCLE_SCHEMA.into(),
        source_revision,
        build_input_digest,
        profile_digest,
        validator_digest,
        authority_revision_digest,
        targets_digest,
        targets,
        snapshot_digest,
        observed_boundary: LIFECYCLE_OBSERVED_BOUNDARY.into(),
    })
}

pub fn issue_dgcl_evidence_lease(
    snapshot: &DgclExecutionSnapshot,
    now_epoch: u64,
    lease_seconds: u64,
) -> Result<DgclEvidenceLease, DgclLifecycleError> {
    if !snapshot_is_well_formed(snapshot) {
        return Err(DgclLifecycleError::InvalidSnapshot);
    }
    if !(1..=MAX_LEASE_SECONDS).contains(&lease_seconds) {
        return Err(DgclLifecycleError::LeaseDurationInvalid);
    }
    let expires_at_epoch = now_epoch
        .checked_add(lease_seconds)
        .ok_or(DgclLifecycleError::LeaseTimeOverflow)?;
    let mut lease = DgclEvidenceLease {
        schema_version: DGCL_EVIDENCE_LIFECYCLE_SCHEMA.into(),
        snapshot_digest: snapshot.snapshot_digest.clone(),
        source_revision: snapshot.source_revision.clone(),
        build_input_digest: snapshot.build_input_digest.clone(),
        profile_digest: snapshot.profile_digest.clone(),
        validator_digest: snapshot.validator_digest.clone(),
        authority_revision_digest: snapshot.authority_revision_digest.clone(),
        targets_digest: snapshot.targets_digest.clone(),
        issued_at_epoch: now_epoch,
        expires_at_epoch,
        lease_digest: String::new(),
    };
    lease.lease_digest = evidence_lease_digest(&lease);
    Ok(lease)
}

pub fn validate_dgcl_evidence_lease(
    lease: &DgclEvidenceLease,
    current: &DgclExecutionSnapshot,
    now_epoch: u64,
) -> DgclLifecycleObservation {
    let state = if !lease_is_well_formed(lease)
        || lease.lease_digest != evidence_lease_digest(lease)
        || !snapshot_is_well_formed(current)
        || current.snapshot_digest
            != lifecycle_snapshot_digest(
                &current.source_revision,
                &current.build_input_digest,
                &current.profile_digest,
                &current.validator_digest,
                &current.authority_revision_digest,
                &current.targets_digest,
            ) {
        DgclLifecycleState::InvalidLease
    } else if now_epoch < lease.issued_at_epoch || now_epoch >= lease.expires_at_epoch {
        DgclLifecycleState::Expired
    } else if lease.source_revision != current.source_revision {
        DgclLifecycleState::StaleSource
    } else if lease.build_input_digest != current.build_input_digest {
        DgclLifecycleState::StaleBuild
    } else if lease.profile_digest != current.profile_digest {
        DgclLifecycleState::StaleProfile
    } else if lease.validator_digest != current.validator_digest {
        DgclLifecycleState::StaleValidator
    } else if lease.authority_revision_digest != current.authority_revision_digest {
        DgclLifecycleState::StaleAuthority
    } else if lease.targets_digest != current.targets_digest {
        DgclLifecycleState::StaleTargets
    } else {
        DgclLifecycleState::Current
    };
    DgclLifecycleObservation {
        state,
        expected_snapshot_digest: lease.snapshot_digest.clone(),
        observed_snapshot_digest: current.snapshot_digest.clone(),
        lease_digest: lease.lease_digest.clone(),
        claim_boundary: "content identity and lease freshness only; caller-supplied snapshot is not host-authenticated and hash comparison is not execution isolation",
    }
}

pub fn validate_dgcl_target_lease(
    lease: &DgclEvidenceLease,
    current: &DgclExecutionSnapshot,
    target_ref: &str,
    target_digest: &str,
    now_epoch: u64,
) -> DgclLifecycleObservation {
    let mut observation = validate_dgcl_evidence_lease(lease, current, now_epoch);
    if observation.state == DgclLifecycleState::Current
        && !lifecycle_binds_target(current, target_ref, target_digest)
    {
        observation.state = DgclLifecycleState::TargetNotBound;
    }
    observation
}

pub fn lifecycle_binds_target(
    snapshot: &DgclExecutionSnapshot,
    target_ref: &str,
    target_digest: &str,
) -> bool {
    snapshot
        .targets
        .iter()
        .any(|target| target.target_ref == target_ref && target.target_digest == target_digest)
}

pub fn advance_dgcl_target_snapshot(
    current: &DgclExecutionSnapshot,
    target_ref: &str,
    expected_before_digest: &str,
    after_digest: &str,
) -> Result<DgclExecutionSnapshot, DgclLifecycleError> {
    if !snapshot_is_well_formed(current)
        || !valid_digest(expected_before_digest)
        || !valid_digest(after_digest)
        || expected_before_digest == after_digest
    {
        return Err(DgclLifecycleError::TargetTransitionInvalid);
    }
    let mut next = current.clone();
    let Some(target) = next
        .targets
        .iter_mut()
        .find(|target| target.target_ref == target_ref)
    else {
        return Err(DgclLifecycleError::TargetTransitionInvalid);
    };
    if target.target_digest != expected_before_digest {
        return Err(DgclLifecycleError::TargetTransitionInvalid);
    }
    target.target_digest = after_digest.to_string();
    next.targets_digest = stable_sha256(
        &serde_json::to_string(&next.targets).map_err(|_| DgclLifecycleError::InvalidSnapshot)?,
    );
    next.snapshot_digest = lifecycle_snapshot_digest(
        &next.source_revision,
        &next.build_input_digest,
        &next.profile_digest,
        &next.validator_digest,
        &next.authority_revision_digest,
        &next.targets_digest,
    );
    Ok(next)
}

fn lifecycle_snapshot_digest(
    source_revision: &str,
    build_input_digest: &str,
    profile_digest: &str,
    validator_digest: &str,
    authority_revision_digest: &str,
    targets_digest: &str,
) -> String {
    stable_sha256(&format!(
        "{DGCL_EVIDENCE_LIFECYCLE_SCHEMA}\0{source_revision}\0{build_input_digest}\0{profile_digest}\0{validator_digest}\0{authority_revision_digest}\0{targets_digest}"
    ))
}

fn evidence_lease_digest(lease: &DgclEvidenceLease) -> String {
    stable_sha256(&format!(
        "{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
        lease.schema_version,
        lease.snapshot_digest,
        lease.source_revision,
        lease.build_input_digest,
        lease.profile_digest,
        lease.validator_digest,
        lease.authority_revision_digest,
        lease.targets_digest,
        lease.issued_at_epoch,
        lease.expires_at_epoch
    ))
}

fn validate_revision(revision: &str) -> Result<(), ()> {
    if revision.trim().is_empty() || revision.len() > 512 || revision.chars().any(char::is_control)
    {
        Err(())
    } else {
        Ok(())
    }
}

fn lease_is_well_formed(lease: &DgclEvidenceLease) -> bool {
    lease.schema_version == DGCL_EVIDENCE_LIFECYCLE_SCHEMA
        && valid_digest(&lease.snapshot_digest)
        && valid_digest(&lease.source_revision)
        && valid_digest(&lease.build_input_digest)
        && valid_digest(&lease.profile_digest)
        && valid_digest(&lease.validator_digest)
        && valid_digest(&lease.authority_revision_digest)
        && valid_digest(&lease.targets_digest)
        && valid_digest(&lease.lease_digest)
        && lease.expires_at_epoch > lease.issued_at_epoch
        && lease.expires_at_epoch - lease.issued_at_epoch <= MAX_LEASE_SECONDS
        && lease.snapshot_digest
            == lifecycle_snapshot_digest(
                &lease.source_revision,
                &lease.build_input_digest,
                &lease.profile_digest,
                &lease.validator_digest,
                &lease.authority_revision_digest,
                &lease.targets_digest,
            )
}

fn snapshot_is_well_formed(snapshot: &DgclExecutionSnapshot) -> bool {
    if snapshot.schema_version != DGCL_EVIDENCE_LIFECYCLE_SCHEMA
        || snapshot.observed_boundary != LIFECYCLE_OBSERVED_BOUNDARY
        || !valid_digest(&snapshot.source_revision)
        || !valid_digest(&snapshot.build_input_digest)
        || !valid_digest(&snapshot.profile_digest)
        || !valid_digest(&snapshot.validator_digest)
        || !valid_digest(&snapshot.authority_revision_digest)
        || !valid_digest(&snapshot.targets_digest)
        || !valid_digest(&snapshot.snapshot_digest)
        || snapshot.targets.len() > MAX_TARGETS
        || snapshot.targets.windows(2).any(|pair| pair[0] >= pair[1])
        || snapshot.targets.iter().any(|target| {
            target.target_ref.trim().is_empty()
                || target.target_ref.len() > 1_024
                || target.target_ref.chars().any(char::is_control)
                || !valid_digest(&target.target_digest)
        })
    {
        return false;
    }
    let targets_digest = stable_sha256(
        &serde_json::to_string(&snapshot.targets).expect("validated targets serialize"),
    );
    targets_digest == snapshot.targets_digest
        && snapshot.snapshot_digest
            == lifecycle_snapshot_digest(
                &snapshot.source_revision,
                &snapshot.build_input_digest,
                &snapshot.profile_digest,
                &snapshot.validator_digest,
                &snapshot.authority_revision_digest,
                &snapshot.targets_digest,
            )
}

fn digest_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hexadecimal = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hexadecimal}")
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(
        source: &str,
        build: &[u8],
        profile: &str,
        validator: &str,
        authority: &str,
        targets: Vec<DgclTargetIdentity>,
    ) -> DgclExecutionSnapshot {
        build_dgcl_execution_snapshot(source, build, profile, validator, authority, targets)
            .unwrap()
    }

    fn target() -> DgclTargetIdentity {
        DgclTargetIdentity {
            target_ref: "dgcl/action/test".into(),
            target_digest: stable_sha256("target-v1"),
        }
    }

    #[test]
    fn any_identity_change_invalidates_an_unexpired_lease() {
        let original = snapshot(
            "Please test.",
            b"Cargo.lock v1",
            "profile-1",
            "validator-1",
            "authority-1",
            vec![target()],
        );
        let lease = issue_dgcl_evidence_lease(&original, 10, 300).unwrap();
        assert_eq!(
            validate_dgcl_evidence_lease(&lease, &original, 11).state,
            DgclLifecycleState::Current
        );
        let mutations = [
            (
                "Please test!",
                b"Cargo.lock v1".as_slice(),
                "profile-1",
                "validator-1",
                "authority-1",
                vec![target()],
                DgclLifecycleState::StaleSource,
            ),
            (
                "Please test.",
                b"Cargo.lock v2".as_slice(),
                "profile-1",
                "validator-1",
                "authority-1",
                vec![target()],
                DgclLifecycleState::StaleBuild,
            ),
            (
                "Please test.",
                b"Cargo.lock v1".as_slice(),
                "profile-2",
                "validator-1",
                "authority-1",
                vec![target()],
                DgclLifecycleState::StaleProfile,
            ),
            (
                "Please test.",
                b"Cargo.lock v1".as_slice(),
                "profile-1",
                "validator-2",
                "authority-1",
                vec![target()],
                DgclLifecycleState::StaleValidator,
            ),
            (
                "Please test.",
                b"Cargo.lock v1".as_slice(),
                "profile-1",
                "validator-1",
                "authority-2",
                vec![target()],
                DgclLifecycleState::StaleAuthority,
            ),
            (
                "Please test.",
                b"Cargo.lock v1".as_slice(),
                "profile-1",
                "validator-1",
                "authority-1",
                vec![DgclTargetIdentity {
                    target_ref: "dgcl/action/test".into(),
                    target_digest: stable_sha256("target-v2"),
                }],
                DgclLifecycleState::StaleTargets,
            ),
        ];
        for (source, build, profile, validator, authority, targets, expected) in mutations {
            let changed = snapshot(source, build, profile, validator, authority, targets);
            assert_eq!(
                validate_dgcl_evidence_lease(&lease, &changed, 11).state,
                expected
            );
        }
    }

    #[test]
    fn expiry_and_identity_tampering_fail_closed() {
        let current = snapshot(
            "Please test.",
            b"Cargo.lock",
            "profile",
            "validator",
            "authority",
            vec![target()],
        );
        let lease = issue_dgcl_evidence_lease(&current, 100, 30).unwrap();
        assert_eq!(
            validate_dgcl_evidence_lease(&lease, &current, 130).state,
            DgclLifecycleState::Expired
        );
        let mut forged = lease.clone();
        forged.validator_digest = stable_sha256("forged-validator");
        assert_eq!(
            validate_dgcl_evidence_lease(&forged, &current, 110).state,
            DgclLifecycleState::InvalidLease
        );
        let mut forged_snapshot = current.clone();
        forged_snapshot.targets[0].target_digest = stable_sha256("substituted-target");
        assert_eq!(
            validate_dgcl_evidence_lease(&lease, &forged_snapshot, 110).state,
            DgclLifecycleState::InvalidLease
        );
        assert_eq!(
            issue_dgcl_evidence_lease(&forged_snapshot, 100, 30),
            Err(DgclLifecycleError::InvalidSnapshot)
        );
        assert_eq!(
            issue_dgcl_evidence_lease(&current, 100, MAX_LEASE_SECONDS + 1),
            Err(DgclLifecycleError::LeaseDurationInvalid)
        );
        assert_eq!(
            issue_dgcl_evidence_lease(&current, u64::MAX, 1),
            Err(DgclLifecycleError::LeaseTimeOverflow)
        );
    }

    #[test]
    fn target_membership_and_duplicate_identity_are_explicit() {
        let current = snapshot(
            "Please test.",
            b"Cargo.lock",
            "profile",
            "validator",
            "authority",
            vec![target()],
        );
        assert!(lifecycle_binds_target(
            &current,
            "dgcl/action/test",
            &stable_sha256("target-v1")
        ));
        assert!(!lifecycle_binds_target(
            &current,
            "dgcl/action/test",
            &stable_sha256("target-v2")
        ));
        let lease = issue_dgcl_evidence_lease(&current, 10, 30).unwrap();
        assert_eq!(
            validate_dgcl_target_lease(
                &lease,
                &current,
                "dgcl/action/test",
                &stable_sha256("target-v2"),
                11,
            )
            .state,
            DgclLifecycleState::TargetNotBound
        );
        assert_eq!(
            build_dgcl_execution_snapshot(
                "source",
                b"build",
                "profile",
                "validator",
                "authority",
                vec![target(), target()],
            ),
            Err(DgclLifecycleError::DuplicateTarget)
        );
    }

    #[test]
    fn snapshot_and_lease_round_trip_without_granting_authority() {
        let snapshot = snapshot(
            "Please test.",
            b"Cargo.toml + Cargo.lock",
            "profile",
            "validator",
            "authority",
            vec![target()],
        );
        let lease = issue_dgcl_evidence_lease(&snapshot, 40, 60).unwrap();
        let snapshot_json = serde_json::to_value(&snapshot).unwrap();
        let lease_json = serde_json::to_value(&lease).unwrap();
        assert_eq!(
            serde_json::from_value::<DgclExecutionSnapshot>(snapshot_json).unwrap(),
            snapshot
        );
        assert_eq!(
            serde_json::from_value::<DgclEvidenceLease>(lease_json).unwrap(),
            lease
        );
        assert!(snapshot.observed_boundary.contains("does not isolate"));
    }
}
