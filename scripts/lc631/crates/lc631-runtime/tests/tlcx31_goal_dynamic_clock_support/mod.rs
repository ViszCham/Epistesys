//! New owned roots and TWO actual native observations with live TLS owner clocks.
//! No fake owners, provider-success stubs, old-helper edits or host-clock proof.
#![allow(dead_code)]
pub mod clocks;
#[path="../tlcx22_goal_artifact_support/mod.rs"] mod original;
pub use original::{Harness,BASE,HAND};
use original::native;
use clocks::Lane;
use lc631_analysis::{build_dgcl_pipeline,capture_dgcl_cargo_snapshot_with_runner,
    declared_build_capture::*,BoundedToolRequest,DgclBackendSet,ResourceToolResult,ToolProcessRunner};
use lc631_core::{context::SourceRevision,AuthorityRevision};
use lc631_receipt_kernel::{ReceiptIssuer,ReceiptVerifier};
use lc631_runtime::{contextual_repair::{execution::observe_current,measurement::CodingProbePlan,
    snapshot_gate::CodingBuildPort},goal_suite_measurement as g,resource::*};
use std::{io::Write,path::PathBuf,time::{Duration,SystemTime,UNIX_EPOCH}};

struct Reset;
impl Drop for Reset{fn drop(&mut self){clocks::reset();}}
struct CaptureAuthority{inner:native::FixtureSnapshotAuthority,lane:Lane}
impl CurrentSnapshotAuthority for CaptureAuthority{
    fn current(&mut self)->Result<SnapshotAuthorityView<'_>,SnapshotCaptureError>{
        // Actual producer receives this newly sampled context. Its authentic
        // local Test permit is unchanged, not reissued from the scalar epoch.
        self.inner.epoch=clocks::sample(self.lane);self.inner.current()
    }
}
struct Reader<'a>(&'a RunResources);
impl ToolProcessRunner for Reader<'_>{
    fn run(&mut self,_:&str,_:BoundedToolRequest<'_>,
        _:&mut dyn FnMut(&ResourceToolResult<'_>)->Result<(),ResourceError>)->Result<(),ResourceError>{
        Err(ResourceError::InvalidConfig)
    }
    fn with_input(&mut self,source:&str,cap:usize,
        consume:&mut dyn FnMut(&ResourceReservationView<'_>)->Result<(),ResourceError>)->Result<(),ResourceError>{
        let mut r=self.0.reserve_for_source(SourceRevision::of(source).digest(),
            &[q(ResourceKind::Ram,u64::try_from(cap).map_err(|_|ResourceError::ArithmeticOverflow)?)])?;
        let result=consume(&r.lease());r.release_confirmed()?;result
    }
}
fn q(kind:ResourceKind,n:u64)->ResourceRequest{ResourceRequest::new(kind,ResourceAmount::checked(n).unwrap())}
fn create(path:&std::path::Path,bytes:&[u8]){
    let mut f=std::fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap();
    f.write_all(bytes).unwrap();f.sync_all().unwrap();
}
fn root(role:&str,source:&str)->PathBuf{
    let n=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let path=std::env::temp_dir().join(format!("tlcx31-goal-dynamic-{}-{role}-{n:x}",std::process::id()));
    assert!(!path.exists());std::fs::create_dir(&path).unwrap();std::fs::create_dir(path.join("src")).unwrap();
    create(&path.join("Cargo.toml"),b"[package]\nname='goal_dynamic_clock_fixture'\nversion='0.0.0'\nedition='2021'\n");
    create(&path.join("src/lib.rs"),source.as_bytes());
    eprintln!("retained new dynamic clock fixture: {}",path.display());path
}
pub fn fixture(body:impl for<'a,'v,'lease,'g,'bi,'br,'ci,'cr> FnOnce(Harness<'a,'v>,
    &mut g::ArtifactCurrent<'g,'bi,'br,'ci,'cr>,&'v ResourceReservationView<'lease>)){
    clocks::reset();let _reset=Reset;
    let entry=|s:&str|s.replace("fn add(","fn checked_add(").replace("match add(","match checked_add(");
    let wrapping=native::BEFORE.replace("{None}","{Some(a.wrapping_add(b))}");
    let br=root("baseline",&entry(&wrapping));let cr=root("candidate",&entry(native::AFTER));
    let bb=br.join(format!("baseline{}",std::env::consts::EXE_SUFFIX));
    let cb=cr.join(format!("candidate{}",std::env::consts::EXE_SUFFIX));
    assert!(!bb.exists()&&!cb.exists());
    native::compile_native(&br.join("src/lib.rs"),&bb);native::compile_native(&cr.join("src/lib.rs"),&cb);
    let pipeline=build_dgcl_pipeline(native::SOURCE,DgclBackendSet{english:None,japanese:None}).unwrap();
    let plan=CodingProbePlan::checked_add(original::scope().goal(),original::scope().semantics()).unwrap();
    let issuer=ReceiptIssuer::from_key_bytes("goal-dynamic-clock",&[132;32]).unwrap();
    let verifier=ReceiptVerifier::from_key_bytes("goal-dynamic-clock",&[132;32]).unwrap();
    let mut amounts=[0;lc631_substrate::resource::RESOURCE_KINDS];
    for(kind,n)in[(ResourceKind::Ram,256*1024*1024),(ResourceKind::Disk,64*1024*1024),
        (ResourceKind::Output,2*1024*1024),(ResourceKind::Child,2),(ResourceKind::Cleanup,2)]{amounts[kind.index()]=n;}
    let limits=ResourceLimits::checked(amounts).unwrap();let broker=ResourceBroker::new(limits).unwrap();
    let cancel=Cancellation::default();let mut run=broker.begin_run(3132,SourceRevision::of(native::SOURCE).digest(),limits,
        Deadline::after(Duration::from_secs(120)).unwrap(),cancel.clone()).unwrap();
    let mut reservation=run.reserve(&[q(ResourceKind::Ram,96*1024*1024),q(ResourceKind::Output,2*1024*1024),
        q(ResourceKind::Child,2),q(ResourceKind::Disk,64*1024*1024),q(ResourceKind::Cleanup,2)]).unwrap();
    {
        let view=reservation.lease();let mut b_reader=Reader(&run);let mut c_reader=Reader(&run);
        let mut ba=CaptureAuthority{inner:native::snapshot_authority(&br,&issuer,&verifier,100,AuthorityRevision(1)),lane:Lane::CaptureBase};
        let mut ca=CaptureAuthority{inner:native::snapshot_authority(&cr,&issuer,&verifier,100,AuthorityRevision(1)),lane:Lane::CaptureNext};
        let bs={let a=ba.current().unwrap();capture_dgcl_cargo_snapshot_with_runner(&br,native::SOURCE,&pipeline,&a.context,&mut b_reader).unwrap()};
        let cs={let a=ca.current().unwrap();capture_dgcl_cargo_snapshot_with_runner(&cr,native::SOURCE,&pipeline,&a.context,&mut c_reader).unwrap()};
        let parse=|s:&str|declared_capture_digest(s).unwrap();
        let pg=parse(&pipeline.program_ir.program_digest);let pl=parse(&pipeline.completion_plan.plan_digest);
        let mut bp=CodingBuildPort::checked(&br,native::SOURCE,&pipeline,parse(&lc631_analysis::dgcl_repository_root_digest(&br).unwrap()),pg,pl,&mut b_reader).unwrap();
        let mut cp=CodingBuildPort::checked(&cr,native::SOURCE,&pipeline,parse(&lc631_analysis::dgcl_repository_root_digest(&cr).unwrap()),pg,pl,&mut c_reader).unwrap();
        let before=bp.capture_current(&mut ba,&view);let expected=cp.capture_current(&mut ca,&view);
        let bnf=native::NativeProbeFixture::registered_at(&plan,bs,br.join("src/lib.rs"),bb.clone(),&issuer,&verifier,100,clocks::hidden_base);
        let cnf=native::NativeProbeFixture::registered_at(&plan,cs,cr.join("src/lib.rs"),cb.clone(),&issuer,&verifier,100,clocks::hidden_next);
        bnf.with_invocation(&verifier,|bi|cnf.with_invocation(&verifier,|ci|{
            let baseline=observe_current(&plan,bi,&view).unwrap();let candidate=observe_current(&plan,ci,&view).unwrap();
            assert_eq!(baseline.actual_outputs(),&BASE);assert_eq!(candidate.actual_outputs(),&HAND);
            assert_eq!((baseline.checked_epoch(),candidate.checked_epoch()),(100,100));
            // Setup itself used actual captures/clock functions. New scripts are
            // installed only now; owner clock pointers remain the same live fns.
            clocks::reset();
            bnf.with_invocation(&verifier,|mut bi|cnf.with_invocation(&verifier,|mut ci|{
                bi.clock=clocks::direct_base;ci.clock=clocks::direct_next;
                let mut current=g::ArtifactCurrent{baseline_invocation:bi,candidate_invocation:ci,
                    baseline_build:&mut bp,candidate_build:&mut cp,baseline_authority:&mut ba,candidate_authority:&mut ca};
                body(Harness{pipeline:&pipeline,plan:&plan,baseline:&baseline,candidate:&candidate,
                    baseline_origin:before.current().unwrap(),candidate_origin:expected.current().unwrap(),
                    baseline_root:&br,candidate_root:&cr,baseline_binary:&bb,candidate_binary:&cb,cancel:&cancel},&mut current,&view);
            }));
        }));
    }
    reservation.release_confirmed().unwrap();run.close_confirmed().unwrap();assert_eq!(broker.used(ResourceKind::Ram).unwrap(),0);
}
