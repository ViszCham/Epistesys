//! Actual native outputs; fixture issuers are not real host or independent humans.
use lc631_analysis::{
    build_dgcl_execution_snapshot, issue_dgcl_evidence_lease, DgclTargetIdentity,
};
use lc631_core::{
    context::SourceRevision, stable_sha256, Action, AuthorityRevision, CallerOrigin,
    ExecutionPermitContext,
};
use lc631_receipt_kernel::*;
use lc631_runtime::{
    contextual_repair::{execution::*, measurement::*},
    resource::*,
};
use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
#[path = "../../lc631-analysis/tests/tlcx31_file_repair_fixture/authority.rs"]
#[allow(
    dead_code,
    reason = "Native probes reuse permit/file_hash; Cargo registration and connection_spec are exercised by analysis repair tests."
)]
mod authority;
const SOURCE: &str = "Please edit the source and test the package.";
fn now() -> u64 {
    100
}
fn root() -> PathBuf {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let p = std::env::temp_dir().join(format!("tlcx31-probes-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn native(root: &Path, name: &str, expression: &str) -> (PathBuf, String) {
    let code = format!("use std::io::{{self,Read}}; fn main(){{ let mut s=String::new();io::stdin().read_to_string(&mut s).unwrap();for line in s.lines(){{let v:Vec<u32>=line.split_whitespace().map(|x|x.parse().unwrap()).collect();let a=v[0];let b=v[1];let x:Option<u32>={expression};match x{{Some(n)=>println!(\"some:{{n}}\"),None=>println!(\"none\")}}}}}}");
    let source = root.join(format!("{name}.rs"));
    std::fs::write(&source, &code).unwrap();
    let executable = root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    let status = std::process::Command::new("rustc")
        .arg("--edition=2021")
        .arg("-o")
        .arg(&executable)
        .arg(&source)
        .status()
        .unwrap();
    assert!(status.success());
    (executable, code)
}
fn snapshot(code: &str) -> lc631_analysis::DgclExecutionSnapshot {
    build_dgcl_execution_snapshot(
        SOURCE,
        code.as_bytes(),
        &stable_sha256("profile"),
        &stable_sha256("validator"),
        &stable_sha256("authority"),
        vec![DgclTargetIdentity {
            target_ref: "src/lib.rs".into(),
            target_digest: stable_sha256(code),
        }],
    )
    .unwrap()
}
#[test]
fn tlcx31_actual_native_pair_is_consumed_and_wrong_test_authority_prevents_spawn() {
    let root = root();
    let (base, base_code) = native(&root, "baseline", "None");
    let (middle, middle_code) = native(&root, "middle", "Some(a.wrapping_add(b))");
    let (final_program, final_code) = native(&root, "final_program", "a.checked_add(b)");
    let issuer = ReceiptIssuer::from_key_bytes("tlcx31-probe-tool", &[107; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("tlcx31-probe-tool", &[107; 32]).unwrap();
    let plan = CodingProbePlan::checked_add(digest(b"goal"), digest(b"policy")).unwrap();
    let limits = ResourceLimits::checked([64 * 1024 * 1024, 0, 1024 * 1024, 0, 2, 2]).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            1,
            SourceRevision::of(SOURCE).digest(),
            limits,
            Deadline::after(Duration::from_secs(90)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve_for_source(
            SourceRevision::of(SOURCE).digest(),
            &[
                q(ResourceKind::Ram, 32 * 1024 * 1024),
                q(ResourceKind::Output, 65536),
                q(ResourceKind::Child, 2),
                q(ResourceKind::Cleanup, 2),
            ],
        )
        .unwrap();
    let view = reservation.lease();
    let observe = |program: &Path, code: &str, action: Action| {
        let snapshot = snapshot(code);
        let lease = issue_dgcl_evidence_lease(&snapshot, 100, 200).unwrap();
        let executable_digest = authority::file_hash(program);
        let target_path = program.with_extension("rs");
        let registration = ProbeToolRegistration {
            program: program.to_str().unwrap(),
            executable_digest: &executable_digest,
            target_path: &target_path,
            target_ref: "src/lib.rs",
            snapshot: &snapshot,
        };
        let scope = registration.receipt_scope(&plan);
        let raw = issuer
            .issue(
                ReceiptClass::ToolExecution,
                SubjectRevision::checked(&snapshot.source_revision).unwrap(),
                ReceiptScope::checked(&scope).unwrap(),
                registration.payload_digest(&plan),
                100,
                Some(400),
                None,
            )
            .unwrap();
        let scope = registration.execution_scope(&plan);
        let permit = authority::permit(SOURCE, &scope, &issuer, &verifier, action);
        let context = ExecutionPermitContext {
            principal: permit.principal_binding(),
            action: Action::Test,
            scope: &scope,
            source_revision: &snapshot.source_revision,
            authority_revision: AuthorityRevision(1),
            now_epoch: 100,
            revocation_revision: 0,
            revoked_permit_digests: &[],
            caller_origin: CallerOrigin::HostVerifiedUser,
            trusted_host_fingerprint: verifier.key_fingerprint(),
        };
        observe_current(
            &plan,
            ProbeInvocation {
                source: SOURCE,
                registration,
                lease: &lease,
                tool_receipt: &raw,
                tool_verifier: &verifier,
                permit: &permit,
                authority: &context,
                clock: now,
                timeout_ms: 5000,
            },
            &view,
        )
    };
    assert!(matches!(
        observe(&base, &base_code, Action::Edit),
        Err(ProbeError::Unauthorized)
    ));
    let baseline = observe(&base, &base_code, Action::Test).unwrap();
    let a = observe(&middle, &middle_code, Action::Test).unwrap();
    let b = observe(&final_program, &final_code, Action::Test).unwrap();
    assert_eq!(baseline.correct_probes(), 1);
    assert_eq!(a.correct_probes(), 3);
    assert_eq!(b.correct_probes(), 4);
    let ga = paired_gain(&plan, &baseline, &a, &view).unwrap();
    let gb = paired_gain(&plan, &baseline, &b, &view).unwrap();
    assert_eq!(ga.gain_quarters(), 2);
    assert_eq!(gb.gain_quarters(), 3);
    assert_eq!(select_current(&plan, &[&ga, &gb], &view).unwrap(), Some(1));
    assert_eq!(select_current(&plan, &[], &view).unwrap(), None);
    assert!(!std::fs::read(&base).unwrap().is_empty());
    // No cleanup here: central owns bounded failure retention / known-owned cleanup.
}
fn q(kind: ResourceKind, n: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap())
}
