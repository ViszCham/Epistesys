//! One actual local inventory check, not full G11 or package/host execution.
#![forbid(unsafe_code)]

use super::{
    assess, Backend, CandidateDisposition, CandidateIdentity, CandidateInspection, GateError,
    GateId, GateKey, ObservedArtifact, Platform, ProfileStatus, ReleaseDigest, ReleasePlan,
    ReportedGateRecord,
};
use crate::tlcx_distribution::{
    audit_tree, AuditError, IdentityState, PackageKind, Plan as DistributionPlan,
};
use sha2::{Digest, Sha256};
use std::path::Path;

/// A closed local obligation. Windows is the package layout, not an assertion
/// that the observer ran on Windows or that an executable was started.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageLocalProfile {
    InventoryWindowsV1,
}

impl PackageLocalProfile {
    pub fn key(self) -> GateKey {
        let domain = match self {
            Self::InventoryWindowsV1 => b"epistesys.tlcx42.local-package-inventory.windows.v1\0",
        };
        GateKey {
            gate: GateId::G11,
            platform: Platform::WindowsX64,
            backend: Backend::Package,
            profile: ReleaseDigest::from_bytes(Sha256::digest(domain).into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalPackageError {
    WrongPackageKind,
    DistributionPlanMismatch,
    CandidateMismatch,
    RunMismatch,
    ProfileNotRequired,
    InventoryMismatch,
    Gate(GateError),
    Audit(AuditError),
}

/// A comparison anchor made from a declared plan BEFORE binding the candidate.
/// It is not an observation, manifest authentication or execution permission.
pub struct ExpectedDistributionPlan {
    digest: [u8; 32],
}

impl ExpectedDistributionPlan {
    pub fn for_plugin(plan: &DistributionPlan<'_>) -> Result<Self, LocalPackageError> {
        if plan.kind() != PackageKind::PluginWindowsX64 {
            return Err(LocalPackageError::WrongPackageKind);
        }
        Ok(Self {
            digest: plan.digest(),
        })
    }

    pub fn bind_candidate(
        self,
        candidate: &CandidateIdentity,
        run: ReleaseDigest,
    ) -> CheckedPackageRequirement {
        CheckedPackageRequirement {
            expected: self,
            candidate: candidate.digest(),
            run,
        }
    }
}

/// Checked plan binding only, not a fact. Private fields prevent relabeling it
/// into another gate, platform, backend, candidate, run or expected inventory.
pub struct CheckedPackageRequirement {
    expected: ExpectedDistributionPlan,
    candidate: ReleaseDigest,
    run: ReleaseDigest,
}

/// Generate and consume the actual local fact in this call. Existing report-only
/// `assess` is unchanged. A Blocked result cannot be repaired by an inventory
/// match. This cannot certify full G11, ingress, command execution or host gates.
pub fn assess_current_package(
    plan: &ReleasePlan<'_>,
    requirement: &CheckedPackageRequirement,
    distribution: &DistributionPlan<'_>,
    root: &Path,
    artifacts: &[ObservedArtifact],
    records: &[ReportedGateRecord],
    now: u64,
) -> Result<CandidateInspection, LocalPackageError> {
    check_anchors(plan, requirement, distribution)?;
    let report = assess(plan, artifacts, records, now).map_err(LocalPackageError::Gate)?;
    if report.disposition == CandidateDisposition::Blocked {
        return Ok(report);
    }
    CurrentLocalPackageFact::observe(requirement, distribution, root)?.consume(report)
}

fn check_anchors(
    plan: &ReleasePlan<'_>,
    requirement: &CheckedPackageRequirement,
    distribution: &DistributionPlan<'_>,
) -> Result<(), LocalPackageError> {
    if distribution.kind() != PackageKind::PluginWindowsX64 {
        return Err(LocalPackageError::WrongPackageKind);
    }
    if requirement.expected.digest != distribution.digest() {
        return Err(LocalPackageError::DistributionPlanMismatch);
    }
    if requirement.candidate != plan.identity.digest() {
        return Err(LocalPackageError::CandidateMismatch);
    }
    if requirement.run != plan.run {
        return Err(LocalPackageError::RunMismatch);
    }
    if !plan
        .required
        .contains(&PackageLocalProfile::InventoryWindowsV1.key())
    {
        return Err(LocalPackageError::ProfileNotRequired);
    }
    Ok(())
}

// Not exported, deserializable, cloneable or reconstructible from a report.
// The private constructor invokes the actual read-only producer itself.
struct CurrentLocalPackageFact {
    candidate: ReleaseDigest,
}

impl CurrentLocalPackageFact {
    fn observe(
        requirement: &CheckedPackageRequirement,
        distribution: &DistributionPlan<'_>,
        root: &Path,
    ) -> Result<Self, LocalPackageError> {
        let observed = audit_tree(root, distribution).map_err(LocalPackageError::Audit)?;
        if observed.plan_digest() != requirement.expected.digest {
            return Err(LocalPackageError::DistributionPlanMismatch);
        }
        if observed.identity() != IdentityState::MatchedInventory {
            return Err(LocalPackageError::InventoryMismatch);
        }
        Ok(Self {
            candidate: requirement.candidate,
        })
    }

    fn consume(
        self,
        mut report: CandidateInspection,
    ) -> Result<CandidateInspection, LocalPackageError> {
        if report.candidate != self.candidate {
            return Err(LocalPackageError::CandidateMismatch);
        }
        let row = report
            .profiles
            .iter_mut()
            .find(|row| row.key == PackageLocalProfile::InventoryWindowsV1.key())
            .ok_or(LocalPackageError::ProfileNotRequired)?;
        row.status = ProfileStatus::CurrentVerified(PackageLocalProfile::InventoryWindowsV1);
        // Keep the existing CandidateNotFullyReleased disposition. This fact
        // cannot remove other required profiles, full G11 or any host gap.
        Ok(report)
    }
}
