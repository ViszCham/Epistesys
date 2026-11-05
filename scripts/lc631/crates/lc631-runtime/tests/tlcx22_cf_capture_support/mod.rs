//! 日本語: 新testsだけのactual supervisor orchestration。新std runnerを持たない。
//! English: Actual-supervisor orchestration for new tests only; no new std process runner.
pub use lc631_process_supervisor as sup;
pub use lc631_runtime::resource::*;
use std::{ffi::OsString,path::PathBuf,time::Duration};
#[path="../fixtures/tlcx22_cf_capture_child.rs"]
pub mod fixture;
pub const CHILD_TEST:&str="cf_capture_fixture_entry";
pub const SMALL_RUST:&[u8]=b"pub fn cf_metadata_fixture() -> u8 { 7 }\n";
pub fn fixture_entry() {
    let Some(mode)=std::env::var_os(fixture::MODE_ENV) else {
        assert!(std::env::var_os(fixture::MODE_ENV).is_none());
        return;
    };
    let arguments:Vec<_>=std::env::args_os().skip(1).collect();
    let expected:Vec<OsString>=["--exact",CHILD_TEST,"--nocapture","--quiet"]
        .into_iter().map(OsString::from).collect();
    assert_eq!(arguments,expected,"fixture mode cannot short-circuit the parent test suite");
    fixture::dispatch(mode.to_str().expect("fixed ASCII fixture mode"));
}

pub struct Budget {
    pub broker:ResourceBroker,
    pub run:RunResources,
    pub reservation:Reservation,
}
pub fn budget(id:u64)->Budget {
    let limits=ResourceLimits::uniform(64*1024*1024).unwrap();
    let broker=ResourceBroker::new(limits).unwrap();
    let run=broker.begin_run(id,Digest32::from_bytes([0xc7;32]),limits,
        Deadline::after(Duration::from_secs(90)).unwrap(),Cancellation::default()).unwrap();
    let amount=|kind,n|ResourceRequest::new(kind,ResourceAmount::checked(n).unwrap());
    let reservation=run.reserve(&[amount(ResourceKind::Ram,32*1024*1024),
        amount(ResourceKind::Output,4*1024*1024),amount(ResourceKind::Child,1),
        amount(ResourceKind::Cleanup,1)]).unwrap();
    Budget {broker,run,reservation}
}
pub fn finish(mut budget:Budget) {
    budget.reservation.release_confirmed().expect("actual captures/cleanup must release");
    budget.run.close_confirmed().unwrap();
    for kind in [ResourceKind::Ram,ResourceKind::Output,ResourceKind::Child,ResourceKind::Cleanup] {
        assert_eq!(budget.broker.used(kind).unwrap(),0);
    }
}
pub struct ChildRequest {
    executable:PathBuf,arguments:Vec<OsString>,directory:PathBuf,
    environment:Vec<(OsString,OsString)>,input:Vec<u8>,
    timeout:Duration,stdout_limit:usize,stderr_limit:usize,
}
fn environment()->Vec<(OsString,OsString)> {
    let mut result=Vec::new();
    for key in ["PATH","SystemRoot","WINDIR","HOME","USERPROFILE","RUSTUP_HOME","CARGO_HOME",
        "LD_LIBRARY_PATH","DYLD_LIBRARY_PATH","TMP","TEMP","TMPDIR"] {
        if let Some(value)=std::env::var_os(key) {result.push((OsString::from(key),value));}
    }
    result
}
impl ChildRequest {
    pub fn fixture(mode:&str,input:&[u8],timeout:Duration,stdout:usize,stderr:usize)->Self {
        let mut env=environment();
        env.push((OsString::from(fixture::MODE_ENV),OsString::from(mode)));
        Self {executable:std::env::current_exe().unwrap(),
            arguments:vec!["--exact".into(),CHILD_TEST.into(),"--nocapture".into(),"--quiet".into()],
            directory:std::env::current_dir().unwrap(),environment:env,input:input.to_vec(),
            timeout,stdout_limit:stdout,stderr_limit:stderr}
    }
    pub fn compiler()->Self {
        let executable=compiler_path().expect("actual rustc required; no skipped positive");
        Self {executable,arguments:["--edition=2021","--crate-type=lib","--emit=metadata",
            "--error-format=json","-o","-","-"].into_iter().map(OsString::from).collect(),
            directory:std::env::current_dir().unwrap(),environment:environment(),
            input:SMALL_RUST.to_vec(),timeout:Duration::from_secs(60),
            stdout_limit:1024*1024,stderr_limit:1024*1024}
    }
    pub fn request(&self)->sup::RunRequest<'_> {
        sup::RunRequest {executable:self.executable.as_os_str(),arguments:&self.arguments,
            current_dir:&self.directory,environment:&self.environment,stdin:Some(&self.input),
            timeout:self.timeout,stdout_limit:self.stdout_limit,stderr_limit:self.stderr_limit}
    }
}
fn compiler_path()->Option<PathBuf> {
    if let Some(path)=std::env::var_os("EPISTESYS_CF_CAPTURE_RUSTC") {
        let path=PathBuf::from(path);
        return (path.is_absolute() && path.is_file()).then_some(path);
    }
    let paths=std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&paths).take(256) {
        let name=if cfg!(windows) {"rustc.exe"} else {"rustc"};
        let path=directory.join(name);
        if path.is_absolute() && path.is_file() {return Some(path);}
    }
    None
}
pub fn captured<'v>(request:sup::RunRequest<'_>,view:&'v ResourceReservationView<'v>)
 ->sup::ResourceRunOutcome<'v> {
    let actual=sup::observe_native_dispatches(||sup::run_bounded_with_resources_observed(request,view)).unwrap();
    assert!(actual.is_complete(),"root launch observation incomplete");
    assert_eq!(actual.dispatches().len(),1,"one actual supervisor root required");
    assert!(actual.dispatches()[0].process_id()>0);
    match actual.into_result() {
        Ok(owner)=>owner,
        Err(error)=>panic!("actual child capture failed: {error:?}"),
    }
}
pub fn complete_fixture_payload(raw:&[u8])->(&[u8],&[u8]) {
    let positions:Vec<_>=raw.windows(fixture::MAGIC.len()).enumerate()
        .filter_map(|(i,x)|(x==fixture::MAGIC).then_some(i)).collect();
    assert_eq!(positions.len(),1,"fixture frame must be unique");
    let start=positions[0];let header=start+fixture::MAGIC.len();
    let length=u32::from_le_bytes(raw[header..header+4].try_into().unwrap()) as usize;
    let end=header+4+length;
    assert_eq!(raw.len(),end,"process::exit fixture emits no libtest postlude");
    assert!(start<=1024,"harness prelude must stay separate and bounded");
    (&raw[..start],&raw[header+4..end])
}
pub fn has_fixture_entry(raw:&[u8])->bool {
    raw.windows(fixture::MAGIC.len()).any(|x|x==fixture::MAGIC)
}
pub fn assert_finished(out:&sup::RunOutcome) {
    assert!(out.process_tree_kill_verified,"tree outcome: {out:?}");
    assert!(out.io_cleanup_complete,"IO outcome: {out:?}");
}
