//! Initial RED is missing planned API, not an observed old-behavior failure.
//! Fake executable bytes below prove inventory only, never command execution.
use lc631_compat::tlcx_distribution::{
    AuditError, ExpectedMember, Limits, PackageKind, Plan, Role,
};
use lc631_compat::tlcx_release_gate::*;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const FILES: [(&str, Role, &[u8]); 8] = [
    ("scripts/lc631/Cargo.toml", Role::Source, b"source"),
    ("schemas/legacy.schema.json", Role::LegacySchema, b"legacy"),
    (
        "schemas/bounded.schema.json",
        Role::BoundedSchema,
        b"bounded",
    ),
    (".codex-plugin/plugin.json", Role::PluginManifest, b"plugin"),
    ("scripts/run-lc631.ps1", Role::Launcher, b"launcher"),
    ("hooks/hooks.json", Role::Hook, b"hooks"),
    ("skills/fixture/SKILL.md", Role::Skill, b"skill"),
    (
        "scripts/lc631/bin/windows-x86_64/lc631.exe",
        Role::Executable,
        b"exe",
    ),
];

struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "tlcx95-package-owned-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap(); // Exclusive ownership, never reuse a root.
        let fixture = Self { root };
        for (path, _, bytes) in FILES {
            let target = fixture.root.join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, bytes).unwrap();
        }
        fixture
    }

    fn change(&self, path: &str, bytes: &[u8]) {
        assert!(FILES.iter().any(|member| member.0 == path));
        fs::write(self.root.join(path), bytes).unwrap();
    }

    fn cut(&self, path: &str) {
        assert!(FILES.iter().any(|member| member.0 == path));
        let target = self.root.join(path);
        assert!(ordinary_single_link(&target));
        fs::remove_file(target).unwrap();
    }
}

fn ordinary_single_link(path: &Path) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.nlink() != 1 {
            return false;
        }
    }
    true
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Exact generated members only; never recursive deletion or traversal.
        // Unexpected types/failures retain the remainder, with no reclaim claim.
        let paths = FILES.iter().map(|entry| entry.0).chain(["extra.txt"]);
        for relative in paths {
            let target = self.root.join(relative);
            match fs::symlink_metadata(&target) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => return,
                Ok(_) if !ordinary_single_link(&target) => return,
                Ok(_) => {
                    if fs::remove_file(target).is_err() {
                        return;
                    }
                }
            }
        }
        for directory in [
            "scripts/lc631/bin/windows-x86_64",
            "scripts/lc631/bin",
            "scripts/lc631",
            "scripts",
            "schemas",
            ".codex-plugin",
            "hooks",
            "skills/fixture",
            "skills",
            "",
        ] {
            let target = self.root.join(directory);
            let Ok(meta) = fs::symlink_metadata(&target) else {
                return;
            };
            if !meta.is_dir() || meta.file_type().is_symlink() {
                return;
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    return;
                }
            }
            if fs::remove_dir(target).is_err() {
                return;
            }
        }
    }
}

fn entries() -> Vec<ExpectedMember<'static>> {
    FILES
        .into_iter()
        .map(|(path, role, bytes)| {
            ExpectedMember::new(path, role, bytes.len() as u64, Sha256::digest(bytes).into())
                .unwrap()
        })
        .collect()
}

fn digest(n: u8) -> ReleaseDigest {
    ReleaseDigest::from_bytes([n; 32])
}
fn observations() -> [ObservedArtifact; 5] {
    ArtifactKind::ALL
        .map(|kind| ObservedArtifact::read(kind, &mut Cursor::new([kind as u8, 42]), 2).unwrap())
}
fn candidate(observed: &[ObservedArtifact; 5]) -> CandidateIdentity {
    CandidateIdentity::checked(std::array::from_fn(|i| ArtifactPin {
        kind: observed[i].kind(),
        digest: observed[i].digest(),
    }))
    .unwrap()
}
fn required() -> Vec<GateKey> {
    let mut rows: Vec<_> = GateId::ALL
        .into_iter()
        .map(|gate| GateKey {
            gate,
            platform: Platform::WindowsX64,
            backend: Backend::Core,
            profile: digest(gate as u8),
        })
        .collect();
    rows.push(PackageLocalProfile::InventoryWindowsV1.key());
    rows
}
fn local_status(report: &CandidateInspection) -> ProfileStatus {
    report
        .profiles
        .iter()
        .find(|row| row.key == PackageLocalProfile::InventoryWindowsV1.key())
        .unwrap()
        .status
}

#[test]
fn tlcx95_current_inventory_is_actual_local_positive_not_g11_or_host_closure() {
    let tree = Fixture::new();
    let entries = entries();
    let distribution =
        Plan::new(PackageKind::PluginWindowsX64, &entries, Limits::default()).unwrap();
    let anchor = ExpectedDistributionPlan::for_plugin(&distribution).unwrap();
    let observed = observations();
    let id = candidate(&observed);
    let requirement = anchor.bind_candidate(&id, digest(90));
    let required = required();
    let plan = ReleasePlan::checked(id, &required, digest(90), 20).unwrap();
    let report = assess_current_package(
        &plan,
        &requirement,
        &distribution,
        &tree.root,
        &observed,
        &[],
        20,
    )
    .unwrap();
    assert_eq!(
        local_status(&report),
        ProfileStatus::CurrentVerified(PackageLocalProfile::InventoryWindowsV1)
    );
    assert_eq!(
        report.profiles[11].status,
        ProfileStatus::Missing,
        "full G11 is still unobserved"
    );
    assert_eq!(report.profiles[13].status, ProfileStatus::HostPending);
    assert_eq!(report.profiles[15].status, ProfileStatus::HostPending);
    assert_eq!(
        report.disposition,
        CandidateDisposition::CandidateNotFullyReleased
    );
}

#[test]
fn tlcx95_reported_pass_cannot_create_current_inventory_or_restore_cut_file() {
    let tree = Fixture::new();
    let entries = entries();
    let distribution =
        Plan::new(PackageKind::PluginWindowsX64, &entries, Limits::default()).unwrap();
    let anchor = ExpectedDistributionPlan::for_plugin(&distribution).unwrap();
    let observed = observations();
    let id = candidate(&observed);
    let requirement = anchor.bind_candidate(&id, digest(90));
    let required = required();
    let plan = ReleasePlan::checked(id, &required, digest(90), 20).unwrap();
    let record = ReportedGateRecord {
        key: PackageLocalProfile::InventoryWindowsV1.key(),
        candidate: id.digest(),
        source_snapshot: id.source_snapshot(),
        run: digest(90),
        observed_at: 10,
        expires_at: 30,
        outcome: ReportedOutcome::Passed,
        receipt: Some(digest(91)),
    };
    assert_eq!(
        local_status(&assess(&plan, &observed, &[record], 20).unwrap()),
        ProfileStatus::ReportedPassNeedsVerification
    );
    assess_current_package(
        &plan,
        &requirement,
        &distribution,
        &tree.root,
        &observed,
        &[record],
        20,
    )
    .unwrap();
    tree.cut("scripts/lc631/bin/windows-x86_64/lc631.exe");
    assert!(matches!(
        assess_current_package(
            &plan,
            &requirement,
            &distribution,
            &tree.root,
            &observed,
            &[record],
            20
        ),
        Err(LocalPackageError::InventoryMismatch)
    ));
}

#[test]
fn tlcx95_expected_distribution_is_anchored_before_candidate_and_cannot_be_swapped() {
    let tree = Fixture::new();
    let entries = entries();
    let distribution =
        Plan::new(PackageKind::PluginWindowsX64, &entries, Limits::default()).unwrap();
    let anchor = ExpectedDistributionPlan::for_plugin(&distribution).unwrap();
    let observed = observations();
    let id = candidate(&observed);
    let requirement = anchor.bind_candidate(&id, digest(90));
    let required = required();
    let plan = ReleasePlan::checked(id, &required, digest(90), 20).unwrap();
    let tighter = Plan::new(
        PackageKind::PluginWindowsX64,
        &entries,
        Limits::new(20, 1024, 8192, 100, 16, 100).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        assess_current_package(
            &plan,
            &requirement,
            &tighter,
            &tree.root,
            &observed,
            &[],
            20
        ),
        Err(LocalPackageError::DistributionPlanMismatch)
    ));
    let different_run = ReleasePlan::checked(id, &required, digest(89), 20).unwrap();
    assert!(matches!(
        assess_current_package(
            &different_run,
            &requirement,
            &distribution,
            &tree.root,
            &observed,
            &[],
            20
        ),
        Err(LocalPackageError::RunMismatch)
    ));
    let mut changed = observations();
    changed[0] =
        ObservedArtifact::read(ArtifactKind::Source, &mut Cursor::new(b"other"), 5).unwrap();
    let other_plan = ReleasePlan::checked(candidate(&changed), &required, digest(90), 20).unwrap();
    assert!(matches!(
        assess_current_package(
            &other_plan,
            &requirement,
            &distribution,
            &tree.root,
            &changed,
            &[],
            20
        ),
        Err(LocalPackageError::CandidateMismatch)
    ));
}

#[test]
fn tlcx95_inventory_profile_cannot_be_relabelled_g13_or_omit_required_profile() {
    let tree = Fixture::new();
    let entries = entries();
    let distribution =
        Plan::new(PackageKind::PluginWindowsX64, &entries, Limits::default()).unwrap();
    let anchor = ExpectedDistributionPlan::for_plugin(&distribution).unwrap();
    let observed = observations();
    let id = candidate(&observed);
    let requirement = anchor.bind_candidate(&id, digest(90));
    let mut required = required();
    required.last_mut().unwrap().gate = GateId::G13;
    let plan = ReleasePlan::checked(id, &required, digest(90), 20).unwrap();
    assert!(matches!(
        assess_current_package(
            &plan,
            &requirement,
            &distribution,
            &tree.root,
            &observed,
            &[],
            20
        ),
        Err(LocalPackageError::ProfileNotRequired)
    ));
}

#[test]
fn tlcx95_current_inventory_rechecks_changes_unlisted_and_exact_role_requirements() {
    let tree = Fixture::new();
    let entries = entries();
    let distribution =
        Plan::new(PackageKind::PluginWindowsX64, &entries, Limits::default()).unwrap();
    let anchor = ExpectedDistributionPlan::for_plugin(&distribution).unwrap();
    let observed = observations();
    let id = candidate(&observed);
    let requirement = anchor.bind_candidate(&id, digest(90));
    let required = required();
    let plan = ReleasePlan::checked(id, &required, digest(90), 20).unwrap();
    for (path, _, original) in FILES {
        tree.change(path, b"changed");
        assert!(
            matches!(
                assess_current_package(
                    &plan,
                    &requirement,
                    &distribution,
                    &tree.root,
                    &observed,
                    &[],
                    20
                ),
                Err(LocalPackageError::InventoryMismatch)
            ),
            "{path}"
        );
        tree.change(path, original);
        assert!(assess_current_package(
            &plan,
            &requirement,
            &distribution,
            &tree.root,
            &observed,
            &[],
            20
        )
        .is_ok());
    }
    fs::write(tree.root.join("extra.txt"), b"unlisted").unwrap();
    assert!(matches!(
        assess_current_package(
            &plan,
            &requirement,
            &distribution,
            &tree.root,
            &observed,
            &[],
            20
        ),
        Err(LocalPackageError::InventoryMismatch)
    ));
    for role in [
        Role::Source,
        Role::LegacySchema,
        Role::BoundedSchema,
        Role::PluginManifest,
        Role::Launcher,
        Role::Hook,
        Role::Skill,
        Role::Executable,
    ] {
        let missing: Vec<_> = entries
            .iter()
            .copied()
            .filter(|entry| entry.role() != role)
            .collect();
        assert!(
            matches!(Plan::new(PackageKind::PluginWindowsX64, &missing, Limits::default()), Err(AuditError::MissingRole(actual)) if actual == role)
        );
    }
}

#[test]
fn tlcx95_source_only_and_empty_plans_cannot_supply_package_observation() {
    let source = entries();
    let source = &source[..3];
    let source_plan = Plan::new(PackageKind::Source, source, Limits::default()).unwrap();
    assert!(matches!(
        ExpectedDistributionPlan::for_plugin(&source_plan),
        Err(LocalPackageError::WrongPackageKind)
    ));
    assert!(matches!(
        Plan::new(PackageKind::PluginWindowsX64, &[], Limits::default()),
        Err(AuditError::EmptyInventory)
    ));
}
