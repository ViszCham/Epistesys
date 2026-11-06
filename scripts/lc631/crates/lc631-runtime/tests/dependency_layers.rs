use serde_json::Value;
use std::process::Command;

// New internal edges require an explicit policy change and review.
const LAYERS: &[(&str, &[&str])] = &[
    ("lc631-substrate", &[]),
    (
        "lc631-solvers",
        &["lc631-process-supervisor", "lc631-substrate"],
    ),
    ("lc631-receipt-kernel", &[]),
    ("lc631-process-supervisor", &["lc631-substrate"]),
    ("lc631-compat", &[]),
    ("lc631-core", &["lc631-receipt-kernel", "lc631-substrate"]),
    (
        "lc631-runtime",
        &[
            // Explicit forward-edge review: TLCX33 semantic-gpu policy, 2026-11-02.
            "lc631-accelerator",
            "lc631-analysis",
            "lc631-core",
            "lc631-host",
            // TLCX30: runtime consumes source-bound Media candidates; no reverse edge.
            "lc631-media",
            "lc631-process-supervisor",
            "lc631-receipt-kernel",
            "lc631-solvers",
            "lc631-substrate",
            "lc631-world",
        ],
    ),
    ("lc631-tl", &["lc631-core", "lc631-receipt-kernel"]),
    ("lc631-wire", &["lc631-core", "lc631-receipt-kernel"]),
    ("lc631-world", &["lc631-core"]),
    (
        "lc631-analysis",
        &[
            "lc631-core",
            "lc631-process-supervisor",
            "lc631-receipt-kernel",
            "lc631-substrate",
            "lc631-tl",
            "lc631-tldg",
        ],
    ),
    (
        "lc631-tldg",
        &[
            "lc631-accelerator",
            "lc631-core",
            "lc631-receipt-kernel",
            "lc631-substrate",
            "lc631-tl",
        ],
    ),
    (
        "lc631-accelerator",
        &["lc631-core", "lc631-receipt-kernel", "lc631-world"],
    ),
    (
        "lc631-media",
        &[
            "lc631-core",
            "lc631-process-supervisor",
            "lc631-receipt-kernel",
            "lc631-tl",
        ],
    ),
    (
        "lc631-host",
        &[
            "lc631-core",
            "lc631-process-supervisor",
            "lc631-receipt-kernel",
        ],
    ),
    (
        "lc631-cli",
        &[
            "lc631-accelerator",
            "lc631-analysis",
            "lc631-compat",
            "lc631-core",
            "lc631-host",
            "lc631-media",
            "lc631-process-supervisor",
            "lc631-receipt-kernel",
            "lc631-runtime",
            "lc631-tl",
            "lc631-tldg",
            "lc631-wire",
            "lc631-world",
        ],
    ),
];

fn allowed(package: &str, dependency: &str) -> bool {
    LAYERS
        .iter()
        .find(|(name, _)| *name == package)
        .is_some_and(|(_, edges)| !dependency.starts_with("lc631-") || edges.contains(&dependency))
}

fn workspace_package_names_match(actual: &std::collections::BTreeSet<String>) -> bool {
    let expected = LAYERS
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    actual == &expected
}

#[test]
fn tlcx_01_forbidden_reverse_edges_are_rejected() {
    assert!(!allowed("lc631-solvers", "lc631-host"));
    assert!(!allowed("lc631-solvers", "lc631-core"));
    assert!(!allowed("lc631-analysis", "lc631-solvers"));
    assert!(!allowed("lc631-world", "lc631-substrate"));
    assert!(!allowed("lc631-solvers", "lc631-receipt-kernel"));
    assert!(!allowed("lc631-host", "lc631-runtime"));
    assert!(!allowed("lc631-world", "lc631-accelerator"));
    assert!(!allowed("lc631-world", "lc631-tldg"));
    assert!(!allowed("lc631-substrate", "lc631-core"));
    assert!(!allowed("lc631-substrate", "lc631-process-supervisor"));
    assert!(!allowed("lc631-runtime", "lc631-cli"));
    assert!(!allowed("lc631-accelerator", "lc631-runtime"));
    assert!(allowed("lc631-runtime", "lc631-accelerator"));
    assert!(allowed("lc631-cli", "lc631-runtime"));
    assert!(allowed("lc631-process-supervisor", "lc631-substrate"));
    assert!(allowed("lc631-runtime", "lc631-core"));
    assert!(allowed("lc631-runtime", "lc631-host"));
    assert!(allowed("lc631-runtime", "lc631-media"));
    assert!(!allowed("lc631-media", "lc631-runtime"));
    assert!(allowed("lc631-runtime", "lc631-analysis"));
    assert!(allowed("lc631-runtime", "lc631-world"));
    assert!(allowed("lc631-solvers", "lc631-process-supervisor"));
    assert!(allowed("lc631-solvers", "lc631-substrate"));
    assert!(!allowed("lc631-solvers", "lc631-runtime"));
    assert!(!allowed("unknown-workspace-crate", "serde"));
}

#[test]
fn tlcx_01_dependency_audit_rejects_a_substituted_empty_crate() {
    let mut package_names = LAYERS
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(workspace_package_names_match(&package_names));
    assert!(package_names.remove("lc631-solvers"));
    package_names.insert("lc631-empty-replacement".to_owned());
    assert!(!workspace_package_names_match(&package_names));
}

#[test]
fn tlcx_01_actual_workspace_respects_layering() {
    // Fixed, repository-owned invocation: no caller command or build script.
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--locked",
            "--offline",
            "--format-version=1",
            "--manifest-path",
            manifest,
        ])
        .output()
        .expect("repository-owned cargo metadata must start");
    assert!(output.status.success());
    assert!(output.stdout.len() <= 2 * 1024 * 1024);
    let metadata: Value = serde_json::from_slice(&output.stdout).unwrap();
    let packages = metadata["packages"].as_array().unwrap();
    let package_names = packages
        .iter()
        .map(|package| package["name"].as_str().unwrap().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        package_names.len(),
        packages.len(),
        "package names must be unique"
    );
    assert!(
        workspace_package_names_match(&package_names),
        "workspace package set must match the reviewed layer allowlist"
    );
    for package in packages {
        let name = package["name"].as_str().unwrap();
        for dependency in package["dependencies"].as_array().unwrap() {
            let dependency = dependency["name"].as_str().unwrap();
            assert!(
                allowed(name, dependency),
                "forbidden dependency {name} -> {dependency}"
            );
        }
    }
}
