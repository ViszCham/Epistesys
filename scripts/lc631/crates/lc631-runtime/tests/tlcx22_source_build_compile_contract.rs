//! Exact linked positive control before negative metadata probes. No Cargo child.
use lc631_analysis::source_build_origin as _;
use std::{io::Write,path::{Path,PathBuf},process::{Command,Stdio}};
fn probe(source:&str,a:&Path,r:&Path,j:&Path)->std::process::Output{
    let exe=std::env::current_exe().unwrap();let deps=exe.parent().unwrap();
    let mut child=Command::new("rustc").args(["--edition=2021","--crate-type=lib","--emit=metadata","-o","-","-"])
        .arg("-L").arg(format!("dependency={}",deps.display()))
        .arg("--extern").arg(format!("lc631_analysis={}",a.display()))
        .arg("--extern").arg(format!("lc631_runtime={}",r.display()))
        .arg("--extern").arg(format!("serde_json={}",j.display()))
        .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(source.as_bytes()).unwrap();child.wait_with_output().unwrap()
}
const CONTROL:&str=r#"
use lc631_analysis::{source_build_origin::*,ResourceToolResult,DgclVerifiedRepairValidation};
use lc631_runtime::resource::ResourceReservationView;
fn valid<'p,'s,'v>(o:&OwnedSourceBuildOrigin<'p,'s,'v>,p:&BuildPlan<'s,'v>,
 s:&mut BuildServices<'_,'_>,v:&ResourceReservationView<'_>){
 let _:Result<(),BuildError>=o.with_current(p,s,v,|view|{let _=view.artifact_digest();});
 let _:bool=serde_json::from_str("true").unwrap();
}
"#;
#[derive(Debug)]
enum ArtifactSelectionError { MissingEvidence }
fn linked()->Result<(PathBuf,PathBuf,PathBuf),ArtifactSelectionError>{
    // Fail-closed until central supplies C's exact current-test Cargo fingerprint
    // -> dependency fingerprint -> exact library fp/hash capability adapter.
    // SourceOwnFile dependency: tlcx22_composite_compile_support/mod.rs.
    // No timestamp/API-compatible fallback; no compiler process starts here.
    Err(ArtifactSelectionError::MissingEvidence)
}
fn rejected(code:&str,marker:&str){
    let(a,r,j)=linked().expect("MissingEvidence: C exact current-artifact helper not connected; no metadata probe is evidence");
    let control=probe(CONTROL,&a,&r,&j);
    assert!(control.status.success(),"exact current API positive control required: {}",String::from_utf8_lossy(&control.stderr));
    let out=probe(&format!("{CONTROL}\n{code}"),&a,&r,&j);
    let e=String::from_utf8_lossy(&out.stderr);assert!(!out.status.success());
    assert!(e.contains(marker),"wrong rejection wanted {marker}: {e}");
    assert!(!e.contains("E0432")&&!e.contains("E0463"),"unresolved APIs not proof: {e}");
}
#[test]
fn tlcx22_sbo_no_public_process_capture_report_or_pending_token_constructor(){
    rejected("fn inject(p:ResourceToolResult<'_>){let _=OwnedSourceBuildOrigin::from_process(p);}","from_process");
    rejected("fn inject(p:lc631_analysis::BoundedToolResult){let _=OwnedSourceBuildOrigin::from_report(p);}","from_report");
    rejected("fn inject(c:lc631_analysis::declared_build_capture::OwnedDeclaredBuildCapture<'_>){let _=OwnedSourceBuildOrigin::from_capture(c);}","from_capture");
    rejected("fn inject(t:DgclVerifiedRepairValidation<'_>){let _=OwnedSourceBuildOrigin::from_verified_validation(t);}","from_verified_validation");
    rejected("fn inject(){let _:OwnedSourceBuildOrigin<'_,'_,'_>=serde_json::from_str(\"{}\").unwrap();}","Deserialize");
}
#[test]
fn tlcx22_sbo_current_frame_view_cannot_escape_or_become_current_source_goal(){
    rejected(r#"
fn escape<'p,'s,'v>(o:&OwnedSourceBuildOrigin<'p,'s,'v>,p:&BuildPlan<'s,'v>,
 s:&mut BuildServices<'_,'_>,v:&ResourceReservationView<'_>){
 let leaked=o.with_current(p,s,v,|x|x.children()).unwrap();let _=leaked.len();}
"#,"lifetime");
    rejected("fn retag(o:OwnedSourceBuildOrigin<'_,'_,'_>){let _=o.current_source_goal();}","current_source_goal");
    rejected("fn retag(o:OwnedSourceBuildOrigin<'_,'_,'_>){let _:lc631_runtime::contextual_repair::snapshot_gate::ExpectedCurrentBuild=o.into();}","From");
}
#[test]
fn tlcx22_sbo_pure_plan_cannot_drop_before_the_observed_owner(){
    rejected(r#"
fn early<'s,'v>(p:BuildPlan<'s,'v>,s:&mut BuildServices<'_,'_>,v:&'v ResourceReservationView<'_>){
 let owner=observe_registered_source_build(&p,s,v).unwrap_or_else(|_|panic!("actual"));
 drop(p);drop(owner);}
"#,"borrow");
}
