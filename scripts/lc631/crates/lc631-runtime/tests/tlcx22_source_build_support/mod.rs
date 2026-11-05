//! New owned roots only. No existing files are overwritten/deleted by setup.
#![allow(dead_code)]
pub mod authority;
pub mod profiles;
use profiles::*;
use lc631_analysis::{source_build_origin as b, declared_build_capture::*,
    build_dgcl_pipeline, DgclBackendSet, DgclPipelineReport, ToolProcessRunner,
    BoundedToolRequest, ResourceToolResult};
use lc631_core::context::SourceRevision;
use lc631_receipt_kernel::*;
use lc631_runtime::resource::*;
use lc631_substrate::{Digest32, source::SourceStore};
use sha2::{Digest, Sha256};
use std::{cell::Cell, rc::Rc, path::{Path, PathBuf}, io::{Read,Write},
    time::{Duration,SystemTime,UNIX_EPOCH}};

pub fn q(kind: ResourceKind, n: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap())
}
fn new_file(path: &Path, bytes: &[u8]) {
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap();
    f.write_all(bytes).unwrap(); f.sync_all().unwrap();
}
pub fn append_owned(path: &Path, bytes: &[u8], root: &Path) {
    assert!(path.starts_with(root)); assert!(path.is_file());
    let mut f = std::fs::OpenOptions::new().append(true).open(path).unwrap();
    f.write_all(bytes).unwrap(); f.sync_all().unwrap();
}
pub fn file_hash(path: &Path, cap: usize) -> Digest32 {
    let mut file = std::fs::File::open(path).unwrap(); let mut h=Sha256::new();
    let mut buffer=[0u8;8192]; let mut total=0;
    loop { let n=file.read(&mut buffer).unwrap(); if n==0 {break;} total+=n; assert!(total<=cap); h.update(&buffer[..n]); }
    Digest32::from_bytes(h.finalize().into())
}
pub struct Fixture {
    pub root: PathBuf, pub output: PathBuf, pub subject_path: PathBuf, pub frame_path: PathBuf,
    pub subject: SourceStore, pub pipeline: DgclPipelineReport, pub compiler: CompilerConfig,
    pub environment: Vec<(String,String)>, pub issuer: ReceiptIssuer, pub verifier: ReceiptVerifier,
    registration: UntrustedReceipt, broker: ResourceBroker, run: RunResources, reservation: Reservation,
    pub cancel: Cancellation, pub authority_cut: Rc<Cell<authority::Cut>>,
}
impl Fixture {
    pub fn new(subject_text: &str) -> Self {
        let nonce=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("epistesys-sbo-{}-{nonce:x}",std::process::id()));
        assert!(!root.exists()); std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("src")).unwrap(); std::fs::create_dir(root.join("target")).unwrap();
        new_file(&root.join(".epistesys-source-build-owned"),b"new-test-owned-root-v1\n");
        new_file(&root.join("Cargo.toml"),b"[package]\nname='sbo_fixture'\nversion='0.0.0'\nedition='2021'\n");
        let subject_path=root.join("src/subject.rs"); let frame_path=root.join("src/frame.rs");
        new_file(&subject_path,subject_text.as_bytes());
        new_file(&root.join("src/driver.rs"),DRIVER.as_bytes());
        new_file(&frame_path,format!("{subject_text}\n{DRIVER}").as_bytes());
        let output=root.join(format!("target/origin{}",std::env::consts::EXE_SUFFIX)); assert!(!output.exists());
        let pipeline=build_dgcl_pipeline(TASK,DgclBackendSet{english:None,japanese:None}).unwrap();
        let compiler=CompilerConfig::selected();
        let issuer=ReceiptIssuer::from_key_bytes("source-build-local-tool",&[131;32]).unwrap();
        let verifier=ReceiptVerifier::from_key_bytes("source-build-local-tool",&[131;32]).unwrap();
        let registration=issuer.issue(ReceiptClass::ToolExecution,
            SubjectRevision::checked(format!("sha256:{}",hex(compiler.image))).unwrap(),
            ReceiptScope::checked(b::compiler_registration_scope(compiler.spec())).unwrap(),
            b::compiler_registration_payload(compiler.spec()),100,Some(400),None).unwrap();
        // Byte budgets and execution-unit concurrency are distinct dimensions.
        let mut amounts=[0u64;lc631_substrate::resource::RESOURCE_KINDS];
        for (kind,amount) in [(ResourceKind::Ram,1024*1024*1024),
            (ResourceKind::Disk,1024*1024*1024),(ResourceKind::Output,1024*1024*1024),
            (ResourceKind::Child,1),(ResourceKind::Cleanup,1)]{amounts[kind.index()]=amount;}
        let caps=ResourceLimits::checked(amounts).unwrap(); let broker=ResourceBroker::new(caps).unwrap();
        let cancel=Cancellation::default();
        let run=broker.begin_run(2301,SourceRevision::of(TASK).digest(),caps,
            Deadline::after(Duration::from_secs(180)).unwrap(),cancel.clone()).unwrap();
        let reservation=run.reserve(&[q(ResourceKind::Ram,512*1024*1024),q(ResourceKind::Output,2*1024*1024),
            q(ResourceKind::Disk,256*1024*1024),q(ResourceKind::Child,1),q(ResourceKind::Cleanup,1)]).unwrap();
        eprintln!("retained new source-build fixture: {}",root.display());
        Self{root,output,subject_path,frame_path,subject:subject(subject_text),pipeline,compiler,
            environment:environment(),issuer,verifier,registration,broker,run,reservation,cancel,
            authority_cut:Rc::new(Cell::new(authority::Cut::None))}
    }
    pub fn with_input(&self, observer: Option<&mut dyn b::BuildEventObserver>,
        body: impl for<'s,'v,'lease> FnOnce(b::BuildPlanInput<'s>, &mut b::BuildServices<'_, '_>,
            &'v ResourceReservationView<'lease>)) {
        let view=self.reservation.lease();
        let parse=|s:&str|declared_capture_digest(s).unwrap();
        let binding=SourceReadBinding::checked(&self.root,TASK,&self.pipeline,
            parse(&lc631_analysis::dgcl_repository_root_digest(&self.root).unwrap()),
            parse(&self.pipeline.program_ir.program_digest),parse(&self.pipeline.completion_plan.plan_digest)).unwrap();
        let env:Vec<_>=self.environment.iter().map(|(name,value)|b::EnvironmentEntry{name,value}).collect();
        let relative=PathBuf::from(format!("target/origin{}",std::env::consts::EXE_SUFFIX));
        let input=b::BuildPlanInput{binding:&binding,root:&self.root,subject:&self.subject,
            subject_relative:Path::new("src/subject.rs"),driver_relative:Path::new("src/driver.rs"),
            frame_relative:Path::new("src/frame.rs"),output_relative:&relative,
            entrypoint:"checked_add",profile:b::StandaloneProfile::CheckedU32AddFourV1,
            emission:b::Emission::LinkExecutable,compiler:self.compiler.spec(),compiler_registration:&self.registration,
            frame:b::ProcessFrame{cwd:&self.root,environment:&env},limits:limits()};
        let mut reader=Reader(&self.run);
        let mut capture=authority::CaptureAuthority::new(&binding,&self.issuer,&self.verifier);
        let mut authority=authority::Authority::new(&self.issuer,&self.verifier,self.authority_cut.clone());
        let mut services=b::BuildServices{source_reader:&mut reader,capture_authority:&mut capture,
            authority:&mut authority,observer};
        body(input,&mut services,&view)
    }
}
fn hex(d:Digest32)->String { d.as_bytes().iter().map(|b|format!("{b:02x}")).collect() }
struct Reader<'a>(&'a RunResources);
impl ToolProcessRunner for Reader<'_> {
    fn run(&mut self,_:&str,_:BoundedToolRequest<'_>,
        _:&mut dyn FnMut(&ResourceToolResult<'_>)->Result<(),ResourceError>)->Result<(),ResourceError>{
        Err(ResourceError::InvalidConfig) // source reader is NOT the compiler dispatcher
    }
    fn with_input(&mut self,source:&str,cap:usize,
        consume:&mut dyn FnMut(&ResourceReservationView<'_>)->Result<(),ResourceError>)->Result<(),ResourceError>{
        let mut reservation=self.0.reserve_for_source(SourceRevision::of(source).digest(),
            &[q(ResourceKind::Ram,u64::try_from(cap).map_err(|_|ResourceError::ArithmeticOverflow)?)])?;
        let result=consume(&reservation.lease());reservation.release_confirmed()?;result
    }
}
