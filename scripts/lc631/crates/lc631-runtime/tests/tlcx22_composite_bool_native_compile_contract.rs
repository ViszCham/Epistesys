//! 日本語: R2型境界。実linked APIのpositive-control後だけ拒否を数える。
//! English: Count R2 rejections only after an exact linked API positive control.
const _: Option<lc631_runtime::composite_original::bool_program::Provenance> = None;
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
use lc631_runtime::{bridge_gain_measurement as b,composite_original::bool_program::*,resource::ResourceReservationView};
fn smoke<'g,'a,'b,'o,'p,'i,'v>(token:&OwnedBoolCorrespondence<'g,'a,'b,'o,'p,'i,'v>,
 coordinate:&BoolCoordinateSpec<'i>,target:BoolTarget<'_>,
 owners:Owners<'g,'a,'b,'o,'p,'i,'v>,view:&ResourceReservationView<'_>){
 let _:Result<(),Error>=token.with_current(coordinate,target,owners,view,|c|{
  let _=c.bridge_report();let _=c.native_program();assert!(c.original_scalar().is_none());
 });
 let _:bool=serde_json::from_str("true").unwrap();
}
"#;
fn exact_libraries() -> LinkedArtifacts {
    let linked = LinkedArtifacts::current("tlcx22_composite_bool_native_compile_contract")
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
fn rejects(text: &str, marker: &str) {
    let linked = exact_libraries();
    let out = probe(&format!("{HEADER}\n{text}"), &linked);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "forbidden program compiled");
    assert!(
        err.contains(marker),
        "wrong rejection wanted {marker}: {err}"
    );
    assert!(
        !err.contains("E0432") && !err.contains("E0463"),
        "unresolved API/crate is not evidence: {err}"
    );
}
#[test]
fn n01_report_scalar_or_deserialization_cannot_create_actual_correspondence() {
    rejects(
        "fn inject(){let _=OwnedBoolCorrespondence::from_report;}",
        "from_report",
    );
    rejects(
        "fn inject(){let _=OwnedBoolCorrespondence::from_scalar;}",
        "from_scalar",
    );
    rejects("fn inject(){let _:OwnedBoolCorrespondence<'_,'_,'_,'_,'_,'_,'_>=serde_json::from_str(\"{}\").unwrap();}","Deserialize");
}
#[test]
fn n02_current_native_report_cannot_escape_callback() {
    rejects(
        r#"
fn escape<'g,'a,'b,'o,'p,'i,'v>(token:&OwnedBoolCorrespondence<'g,'a,'b,'o,'p,'i,'v>,
 coordinate:&BoolCoordinateSpec<'i>,target:BoolTarget<'_>,
 owners:Owners<'g,'a,'b,'o,'p,'i,'v>,view:&ResourceReservationView<'_>){
 let leaked=token.with_current(coordinate,target,owners,view,|c|c.bridge_report()).unwrap();
 let _=leaked.metric;
}
"#,
        "lifetime",
    );
}
#[test]
fn n03_actual_gain_cannot_drop_before_the_correspondence_owner() {
    rejects(
        r#"
fn early<'a,'b,'o,'p,'i,'v>(gain:b::OwnedBridgeGain<'a,'b,'o,'p,'i,'v>,
 root:&'p b::BridgeRoot<'i,'v>,program:&'o b::BridgeProgram<'p,'i,'v>,
 coordinate:&BoolCoordinateSpec<'i>,target:BoolTarget<'_>,
 native:NativeExpected<'_,'p,'i,'v>,view:&'v ResourceReservationView<'_>){
 let owners=Owners{root,program,gain:&gain,native};
 let token=capture_current(coordinate,target,owners,view).unwrap();drop(gain);
 let _=token.with_current(coordinate,target,owners,view,|_|());
}
"#,
        "borrow",
    );
}
