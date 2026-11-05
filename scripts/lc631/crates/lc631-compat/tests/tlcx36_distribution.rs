//! 配布監査の負例を先行登録。 / Negative-first distribution audit contract.
use lc631_compat::tlcx_distribution::*;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "tlcx36-owned-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}

fn write(root: &Path, path: &str, bytes: &[u8]) {
    let destination = root.join(path);
    fs::create_dir_all(destination.parent().unwrap()).unwrap();
    fs::write(destination, bytes).unwrap();
}

fn member(path: &'static str, role: Role, bytes: &[u8]) -> ExpectedMember<'static> {
    ExpectedMember::new(path, role, bytes.len() as u64, Sha256::digest(bytes).into()).unwrap()
}

fn source_members() -> [ExpectedMember<'static>; 3] {
    [
        member("scripts/lc631/Cargo.toml", Role::Source, b"source"),
        member("schemas/legacy.schema.json", Role::LegacySchema, b"legacy"),
        member(
            "schemas/bounded.schema.json",
            Role::BoundedSchema,
            b"bounded",
        ),
    ]
}

#[test]
fn tlcx36_rejects_path_traversal_portable_collisions_and_wrong_roles() {
    for path in [
        "../escape",
        "/root",
        "a\\b",
        "a:b",
        "a/./b",
        "a//b",
        "a/../b",
        "a/NUL.txt",
        "a/x.",
        "a/x ",
        ".git/config",
        "models/weights.gguf",
    ] {
        assert!(
            ExpectedMember::new(path, Role::Source, 1, [0; 32]).is_err(),
            "{path}"
        );
    }
    assert!(ExpectedMember::new("schemas/x.schema.json", Role::Executable, 1, [0; 32]).is_err());
    assert!(ExpectedMember::new("scripts/lc631/other.exe", Role::Source, 1, [0; 32]).is_err());
    let duplicates = [
        member("scripts/lc631/Cargo.toml", Role::Source, b"a"),
        member("scripts/lc631/cargo.toml", Role::Source, b"b"),
    ];
    assert!(matches!(
        Plan::new(PackageKind::Source, &duplicates, Limits::default()),
        Err(AuditError::DuplicatePath)
    ));
}

#[test]
fn tlcx36_source_profile_never_satisfies_plugin_profile() {
    let entries = source_members();
    assert!(Plan::new(PackageKind::Source, &entries, Limits::default()).is_ok());
    assert!(matches!(
        Plan::new(PackageKind::PluginWindowsX64, &entries, Limits::default()),
        Err(AuditError::MissingRole(_))
    ));
    assert!(matches!(
        Plan::new(PackageKind::Source, &[], Limits::default()),
        Err(AuditError::EmptyInventory)
    ));
}

#[test]
fn tlcx36_plugin_roles_are_required_and_binary_digest_is_not_execution() {
    let mut entries = source_members().to_vec();
    entries.extend([
        member(
            ".codex-plugin/plugin.json",
            Role::PluginManifest,
            b"fixture",
        ),
        member("scripts/run-lc631.ps1", Role::Launcher, b"fixture"),
        member("hooks/hooks.json", Role::Hook, b"fixture"),
        member("skills/fixture/SKILL.md", Role::Skill, b"fixture"),
        member(
            "scripts/lc631/bin/windows-x86_64/lc631.exe",
            Role::Executable,
            b"fixture",
        ),
    ]);
    let root = root();
    let plan = Plan::new(PackageKind::PluginWindowsX64, &entries, Limits::default()).unwrap();
    // Synthetic bytes establish identity only; this fixture is never executable evidence.
    write(&root, entries[0].path(), b"source");
    write(&root, entries[1].path(), b"legacy");
    write(&root, entries[2].path(), b"bounded");
    for entry in &entries[3..] {
        write(&root, entry.path(), b"fixture");
    }
    let report = audit_tree(&root, &plan).unwrap();
    assert_eq!(report.identity(), IdentityState::MatchedInventory);
    assert_eq!(
        report.execution(),
        ExecutionEvidence::NeedsFreshPackageExecution
    );
    write(&root, entries[7].path(), b"mutated");
    let changed = audit_tree(&root, &plan).unwrap();
    assert_eq!(changed.members()[7].state(), MemberState::DigestMismatch);
    let without_hook: Vec<_> = entries
        .iter()
        .copied()
        .filter(|entry| entry.role() != Role::Hook)
        .collect();
    assert!(matches!(
        Plan::new(
            PackageKind::PluginWindowsX64,
            &without_hook,
            Limits::default()
        ),
        Err(AuditError::MissingRole(Role::Hook))
    ));
}

#[test]
fn tlcx36_unlisted_regular_file_is_retained_as_a_mismatch() {
    let root = root();
    let entries = source_members();
    write(&root, entries[0].path(), b"source");
    write(&root, entries[1].path(), b"legacy");
    write(&root, entries[2].path(), b"bounded");
    write(&root, "extra.txt", b"extra");
    let plan = Plan::new(PackageKind::Source, &entries, Limits::default()).unwrap();
    let report = audit_tree(&root, &plan).unwrap();
    assert_eq!(report.unlisted(), &["extra.txt".to_string()]);
    assert_eq!(report.identity(), IdentityState::Mismatch);
}

#[test]
fn tlcx36_actual_tree_bytes_are_hashed_and_changed_or_missing_members_are_visible() {
    let root = root();
    let entries = source_members();
    write(&root, entries[0].path(), b"source");
    write(&root, entries[1].path(), b"wrong!");
    let plan = Plan::new(PackageKind::Source, &entries, Limits::default()).unwrap();
    let report = audit_tree(&root, &plan).unwrap();
    assert_eq!(report.members()[0].state(), MemberState::Matched);
    assert_eq!(report.members()[1].state(), MemberState::DigestMismatch);
    assert_eq!(report.members()[2].state(), MemberState::Missing);
    assert_eq!(report.identity(), IdentityState::Mismatch);
    assert_eq!(
        report.execution(),
        ExecutionEvidence::NeedsFreshPackageExecution
    );
    assert!(root.exists()); // Retained owned fixture; no deletion routine.
}

#[test]
fn tlcx36_nonempty_matching_tree_is_only_inventory_evidence() {
    let root = root();
    let entries = source_members();
    for (path, bytes) in [
        (entries[0].path(), b"source".as_slice()),
        (entries[1].path(), b"legacy".as_slice()),
        (entries[2].path(), b"bounded".as_slice()),
    ] {
        write(&root, path, bytes);
    }
    let plan = Plan::new(PackageKind::Source, &entries, Limits::default()).unwrap();
    let report = audit_tree(&root, &plan).unwrap();
    assert_eq!(report.identity(), IdentityState::MatchedInventory);
    assert_eq!(report.members().len(), 3);
    assert!(report.unlisted().is_empty());
    assert_eq!(
        report.execution(),
        ExecutionEvidence::NeedsFreshPackageExecution
    );
    assert_eq!(report.plan_digest(), plan.digest());
}

#[test]
fn tlcx36_unlisted_file_and_secret_named_member_block_inventory_match() {
    let root = root();
    let entries = source_members();
    for entry in &entries {
        write(&root, entry.path(), b"source");
    }
    write(&root, "private.key", b"dummy-not-a-secret");
    let plan = Plan::new(PackageKind::Source, &entries, Limits::default()).unwrap();
    assert!(matches!(
        audit_tree(&root, &plan),
        Err(AuditError::ForbiddenMember)
    ));
}

struct GrowingReader {
    delivered: usize,
}
impl Read for GrowingReader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        bytes.fill(7);
        self.delivered += bytes.len();
        Ok(bytes.len())
    }
}

#[test]
fn tlcx36_stream_limit_uses_a_sentinel_and_interrupted_reads_are_bounded() {
    let mut stream = GrowingReader { delivered: 0 };
    assert!(matches!(
        hash_reader(&mut stream, 4, 8),
        Err(AuditError::MemberLimit)
    ));
    assert_eq!(stream.delivered, 5);
    struct Interrupted;
    impl Read for Interrupted {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::Interrupted.into())
        }
    }
    assert!(matches!(
        hash_reader(&mut Interrupted, 4, 3),
        Err(AuditError::ReadLimit)
    ));
}

#[test]
fn tlcx36_total_and_directory_visit_budgets_do_not_become_match() {
    let entries = source_members();
    let limits = Limits::new(3, 7, 10, 100, 8, 20).unwrap();
    assert!(matches!(
        Plan::new(PackageKind::Source, &entries, limits),
        Err(AuditError::TotalLimit)
    ));
    let root = root();
    fs::create_dir_all(root.join("a/b/c")).unwrap();
    let limits = Limits::new(3, 32, 96, 1, 8, 20).unwrap();
    let plan = Plan::new(PackageKind::Source, &entries, limits).unwrap();
    assert!(matches!(
        audit_tree(&root, &plan),
        Err(AuditError::VisitLimit)
    ));
}

#[cfg(unix)]
#[test]
fn tlcx36_symbolic_link_member_is_rejected_without_following() {
    let root = root();
    let entries = source_members();
    fs::create_dir_all(root.join("scripts/lc631")).unwrap();
    std::os::unix::fs::symlink("/etc/passwd", root.join(entries[0].path())).unwrap();
    let plan = Plan::new(PackageKind::Source, &entries, Limits::default()).unwrap();
    assert!(matches!(
        audit_tree(&root, &plan),
        Err(AuditError::LinkOrSpecialFile)
    ));
}
