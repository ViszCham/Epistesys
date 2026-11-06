//! Central-only metadata probes with a positive API control, never E0432-as-safety.
//! Missing goal_suite_measurement is the planned target-level API RED first.
use lc631_runtime::goal_suite_measurement as _;
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn libraries(deps: &Path, name: &str) -> Vec<PathBuf> {
    let prefix = format!("lib{name}-");
    let mut files: Vec<_> = std::fs::read_dir(deps)
        .unwrap()
        .map(Result::unwrap)
        .filter(|e| {
            e.file_name().to_string_lossy().starts_with(&prefix)
                && e.path().extension().is_some_and(|s| s == "rlib")
        })
        .collect();
    files.sort_by_key(|e| e.metadata().unwrap().modified().unwrap());
    files.into_iter().rev().take(8).map(|e| e.path()).collect()
}
fn probe(source: &str, runtime: &Path, json: &Path) -> std::process::Output {
    let executable = std::env::current_exe().unwrap();
    let deps = executable.parent().unwrap();
    let mut child = Command::new("rustc")
        .args([
            "--edition=2021",
            "--crate-type=lib",
            "--emit=metadata",
            "-o",
            "-",
            "-",
        ])
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .arg("--extern")
        .arg(format!("lc631_runtime={}", runtime.display()))
        .arg("--extern")
        .arg(format!("serde_json={}", json.display()))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
const HEADER: &str = r#"
use lc631_runtime::{goal_suite_measurement::*, resource::ResourceReservationView};
fn smoke<'r,'o,'origin,'v>(owner: &OwnedArtifactPair<'r,'o,'origin,'v>,
 root: &ArtifactPairRoot<'origin,'v>, pair: CodingPair<'o,'v>,
 gate: &mut ArtifactCurrent<'_,'_,'_,'_,'_>, view: &ResourceReservationView<'_>) {
 let _: Result<(), Error> = owner.with_current(root,pair,gate,view,|v| {let _=v.solve().metric;});
 let _: bool=serde_json::from_str("true").unwrap();
}
"#;
fn exact_libraries() -> (PathBuf, PathBuf) {
    let executable = std::env::current_exe().unwrap();
    let deps = executable.parent().unwrap();
    let mut last = String::new();
    for runtime in libraries(deps, "lc631_runtime") {
        for json in libraries(deps, "serde_json") {
            let out = probe(HEADER, &runtime, &json);
            if out.status.success() {
                return (runtime, json);
            }
            last = String::from_utf8_lossy(&out.stderr).into_owned();
        }
    }
    panic!("no exact linked API positive control; missing APIs are NOT successful negative evidence: {last}")
}
fn rejects(code: &str, marker: &str) {
    let (runtime, json) = exact_libraries();
    let out = probe(&format!("{HEADER}\n{code}"), &runtime, &json);
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "forbidden program compiled");
    let expected = error.contains(marker) || (marker == "private" && error.contains("no field"));
    assert!(expected, "wrong failure, wanted {marker}: {error}");
    assert!(
        !error.contains("E0432") && !error.contains("E0463"),
        "unresolved crates are not ownership evidence: {error}"
    );
}

#[test]
fn tlcx22_goal_private_origin_and_report_injection_are_not_public_constructors() {
    rejects(
        "fn steal(r:&ArtifactPairRoot<'_,'_>){let _=&r.baseline_before;}",
        "private",
    );
    rejects(
        "fn inject(){let _=OwnedArtifactPair::from_report;}",
        "from_report",
    );
}
#[test]
fn tlcx22_goal_actual_owner_cannot_deserialize_from_report_bytes() {
    rejects(
        "fn inject(){let _:OwnedArtifactPair<'_,'_,'_,'_>=serde_json::from_str(\"{}\").unwrap();}",
        "Deserialize",
    );
}
#[test]
fn tlcx22_goal_current_view_cannot_escape_higher_ranked_consumer() {
    rejects(
        r#"
fn escape<'a,'r,'o,'origin,'v>(owner: &OwnedArtifactPair<'r,'o,'origin,'v>,
 root: &ArtifactPairRoot<'origin,'v>,pair:CodingPair<'o,'v>,
 gate:&mut ArtifactCurrent<'_,'_,'_,'_,'_>,view:&ResourceReservationView<'_>) {
 let escaped=owner.with_current(root,pair,gate,view,|v|v.solve()).unwrap();
 let _=escaped.metric;
}
"#,
        "lifetime",
    );
}
#[test]
fn tlcx22_goal_borrowed_baseline_cannot_drop_before_metric_owner_use() {
    rejects(
        r#"
fn early<'origin,'v>(root:&ArtifactPairRoot<'origin,'v>,
 base:lc631_runtime::contextual_repair::measurement::OwnedCodingProbeObservation<'v>,
 cand:&lc631_runtime::contextual_repair::measurement::OwnedCodingProbeObservation<'v>,
 gate:&mut ArtifactCurrent<'_,'_,'_,'_,'_>,view:&'v ResourceReservationView<'_>) {
 let pair=CodingPair{baseline:&base,candidate:cand};
 let owner=measure_artifact_pair(root,pair,gate,view).unwrap();
 drop(base);
 let _=owner.with_current(root,pair,gate,view,|_|());
}
"#,
        "borrow",
    );
}
