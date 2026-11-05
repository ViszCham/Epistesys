//! Central-only metadata probes: an exact positive API control precedes each rejection.
const _: Option<lc631_runtime::composite_original::OriginalFormula> = None;
#[path = "tlcx22_composite_compile_support/mod.rs"]
mod exact;
use exact::LinkedArtifacts;
use std::{
    io::Write,
    process::{Command, Stdio},
};
fn probe(source: &str, linked: &LinkedArtifacts) -> std::process::Output {
    linked
        .verify_current()
        .expect("exact compiled artifacts/source changed before rustc");
    let mut command = Command::new("rustc");
    command
        .args([
            "--edition=2021",
            "--crate-type=lib",
            "--emit=metadata",
            "-o",
            "-",
            "-",
        ])
        .arg("-L")
        .arg(format!("dependency={}", linked.deps().display()));
    for (name, path) in linked.externs() {
        command
            .arg("--extern")
            .arg(format!("{name}={}", path.display()));
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let source=format!("const _:Option<lc631_analysis::creativity_measurement::ComponentMetric>=None;\nconst _:Option<lc631_substrate::Digest32>=None;\n{source}");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    linked
        .verify_current()
        .expect("exact compiled artifacts/source changed during rustc");
    output
}
const HEADER: &str = r#"
use lc631_runtime::{composite_original::*,resource::ResourceReservationView};
fn smoke<'c,'r,'i,'v>(p:&PendingComposite<'c,'r,'i,'v>,
 e:ExpectedPending<'c,'r,'i,'v>,v:&ResourceReservationView<'_>) {
 let _:Result<(),Error>=p.with_current(OriginalFormula::F2V1,e,v,|x|{
  let _=x.missing_leaves();let _=x.source();assert!(x.scalar().is_none());
 });
 let _:bool=serde_json::from_str("true").unwrap();
}
"#;
fn exact_libraries() -> LinkedArtifacts {
    let linked = LinkedArtifacts::current("tlcx22_composite_original_compile_contract")
        .expect("unsupported Cargo fingerprint format or no unique exact linked artifact set");
    println!("{}", linked.receipt().expect("bounded artifact receipt"));
    let control = probe(HEADER, &linked);
    assert!(
        control.status.success(),
        "exact linked API positive control failed: {}",
        String::from_utf8_lossy(&control.stderr)
    );
    linked
}
fn rejects(code: &str, marker: &str) {
    let linked = exact_libraries();
    let out = probe(&format!("{HEADER}\n{code}"), &linked);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "forbidden program compiled");
    assert!(
        err.contains(marker),
        "wrong rejection, expected {marker}: {err}"
    );
    assert!(
        !err.contains("E0432") && !err.contains("E0463"),
        "missing APIs are not negative evidence: {err}"
    );
}
#[test]
fn n01_detached_or_retagged_reports_cannot_construct_pending_owner() {
    rejects(
        "fn inject(){let _=PendingComposite::from_reports;}",
        "from_reports",
    );
    rejects(
        "fn inject(){let _=PendingComposite::from_scalar;}",
        "from_scalar",
    );
}
#[test]
fn n02_given_conditional_root_has_no_source_validated_conversion() {
    rejects(
        "fn promote(p:&ConditionalRootPlan<'_,'_>){let _=p.into_source_validated();}",
        "into_source_validated",
    );
}
#[test]
fn n03_current_source_borrow_cannot_escape_scoped_consumer() {
    rejects(
        r#"
fn escape<'c,'r,'i,'v>(p:&PendingComposite<'c,'r,'i,'v>,
 e:ExpectedPending<'c,'r,'i,'v>,v:&ResourceReservationView<'_>) {
 let source=p.with_current(OriginalFormula::F2V1,e,v,|x|x.source()).unwrap();
 let _=source.text();
}
"#,
        "lifetime",
    );
}
#[test]
fn n04_private_pending_owner_cannot_deserialize_or_expose_backing() {
    rejects(
        "fn inject(){let _:PendingComposite<'_,'_,'_,'_>=serde_json::from_str(\"{}\").unwrap();}",
        "Deserialize",
    );
    rejects(
        "fn inspect(p:&PendingComposite<'_,'_,'_,'_>){let _=&p.root;}",
        "private",
    );
}
