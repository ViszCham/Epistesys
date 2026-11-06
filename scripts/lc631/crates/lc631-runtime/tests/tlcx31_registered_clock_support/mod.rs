//! TEST-FIRST contract fixture. Calls only the proposed real factories; no owner
//! construction stub, legacy retag, fake process output or production edits.
#![allow(dead_code)]
pub mod providers;
use lc631_analysis::{
    build_dgcl_pipeline, capture_dgcl_cargo_snapshot_with_runner, declared_build_capture::*,
    BoundedToolRequest, DgclBackendSet, ResourceToolResult, ToolProcessRunner,
};
use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    AuthorityRevision,
};
use lc631_receipt_kernel::*;
use lc631_runtime::{
    contextual_repair::{measurement::CodingProbePlan, snapshot_gate::CodingBuildPort},
    goal_clock_domain as c, goal_suite_measurement as g,
    resource::*,
};
use lc631_substrate::Digest32;
use providers::{native, Grant};
pub use providers::{Clock, Registry};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn limits() -> c::Limits {
    c::Limits {
        max_registration_attempts: 8,
        max_endpoints: 4,
        max_roles: 2,
        max_samples: 1024,
        max_metadata_bytes: 4096,
        max_added_ram_bytes: 64 * 1024,
    }
}
pub fn profile<'v>(registry: &Registry, view: &'v ResourceReservationView<'_>) -> c::Profile<'v> {
    c::Profile::given_local(
        c::ProfileInput {
            domain: d(90),
            protocol: c::Protocol::EpochSecondsV1,
            trusted_registry_fingerprint: registry.verifier.key_fingerprint(),
            requirement: c::TrustRequirement::GivenLocalRegistered,
            limits: limits(),
        },
        view,
    )
    .unwrap()
}
pub fn receipt(
    candidate: &c::EndpointCandidate<'_, '_>,
    issuer: &ReceiptIssuer,
) -> UntrustedReceipt {
    issuer
        .issue(
            ReceiptClass::ToolExecution,
            SubjectRevision::checked(candidate.registration_subject()).unwrap(),
            ReceiptScope::checked(candidate.registration_scope()).unwrap(),
            candidate.registration_payload(),
            100,
            Some(400),
            None,
        )
        .unwrap()
}
pub fn bind(
    root: &Path,
    pipeline: &lc631_analysis::DgclPipelineReport,
    plan: &CodingProbePlan,
    role: c::Role,
    purpose: c::Purpose,
) -> c::EndpointBinding {
    let parse = |s: &str| declared_capture_digest(s).unwrap();
    c::EndpointBinding {
        role,
        purpose,
        root: parse(&lc631_analysis::dgcl_repository_root_digest(root).unwrap()),
        source: SourceRevision::of(native::SOURCE).digest(),
        program: parse(&pipeline.program_ir.program_digest),
        completion_plan: parse(&pipeline.completion_plan.plan_digest),
        probe_plan: plan.digest(),
        endpoint_revision: d(91),
    }
}
pub fn described(
    view: &ResourceReservationView<'_>,
    role: c::Role,
    purpose: c::Purpose,
) -> c::EndpointBinding {
    // Untrusted declaration for registration-only negatives, not physical proof.
    c::EndpointBinding {
        role,
        purpose,
        root: d(81),
        source: view.source_revision(),
        program: d(82),
        completion_plan: d(83),
        probe_plan: d(84),
        endpoint_revision: d(91),
    }
}
fn q(k: ResourceKind, n: u64) -> ResourceRequest {
    ResourceRequest::new(k, ResourceAmount::checked(n).unwrap())
}
struct Reader<'a>(&'a RunResources);
impl ToolProcessRunner for Reader<'_> {
    fn run(
        &mut self,
        _: &str,
        _: BoundedToolRequest<'_>,
        _: &mut dyn FnMut(&ResourceToolResult<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        Err(ResourceError::InvalidConfig)
    }
    fn with_input(
        &mut self,
        s: &str,
        n: usize,
        f: &mut dyn FnMut(&ResourceReservationView<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        let mut r = self.0.reserve_for_source(
            SourceRevision::of(s).digest(),
            &[q(ResourceKind::Ram, u64::try_from(n).unwrap())],
        )?;
        let result = f(&r.lease());
        r.release_confirmed()?;
        result
    }
}
fn root(role: &str, source: &str) -> PathBuf {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let p = std::env::temp_dir().join(format!(
        "tlcx31-registered-clock-{}-{role}-{n:x}",
        std::process::id()
    ));
    assert!(!p.exists());
    std::fs::create_dir(&p).unwrap();
    std::fs::create_dir(p.join("src")).unwrap();
    for (name, bytes) in [
        (
            "Cargo.toml",
            b"[package]\nname='registered_clock_fixture'\nversion='0.0.0'\nedition='2021'\n"
                .as_slice(),
        ),
        ("src/lib.rs", source.as_bytes()),
    ] {
        let mut f = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(p.join(name))
            .unwrap();
        f.write_all(bytes).unwrap();
    }
    p
}
pub struct Clocks {
    pub be: Arc<Clock>,
    pub bc: Arc<Clock>,
    pub ce: Arc<Clock>,
    pub cc: Arc<Clock>,
}
impl Clocks {
    pub fn new() -> Self {
        Self {
            be: Clock::new(),
            bc: Clock::new(),
            ce: Clock::new(),
            cc: Clock::new(),
        }
    }
    pub fn all(&self, n: u64) {
        for c in [&self.be, &self.bc, &self.ce, &self.cc] {
            c.set(n);
        }
    }
    pub fn calls(&self) -> usize {
        [&self.be, &self.bc, &self.ce, &self.cc]
            .iter()
            .map(|c| c.calls())
            .sum()
    }
}

// Passive fixture diagnostics: no clock sample or authority callback is invoked.
// libtest captures these bounded messages and displays them when a case fails.
struct FixtureDiagnostics {
    started: Instant,
    previous: Instant,
}
impl FixtureDiagnostics {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            started: now,
            previous: now,
        }
    }
    fn stage(&mut self, name: &'static str) {
        let now = Instant::now();
        eprintln!(
            "registered-clock setup stage={name} elapsed={:?} total={:?}",
            now.duration_since(self.previous),
            now.duration_since(self.started),
        );
        self.previous = now;
    }
}

pub struct NativeDiagnostics {
    started: Instant,
    remaining_before: Result<Duration, ResourceError>,
    calls_before: [usize; 5],
}
impl NativeDiagnostics {
    fn calls(clocks: &Clocks, registry: &Registry) -> [usize; 5] {
        [
            registry.calls.load(std::sync::atomic::Ordering::SeqCst),
            clocks.be.calls(),
            clocks.bc.calls(),
            clocks.ce.calls(),
            clocks.cc.calls(),
        ]
    }
    fn start(view: &ResourceReservationView<'_>, clocks: &Clocks, registry: &Registry) -> Self {
        Self {
            started: Instant::now(),
            remaining_before: view.remaining(),
            calls_before: Self::calls(clocks, registry),
        }
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "failure diagnostic includes independent clock, registry and native marker observations"
    )]
    fn report(
        self,
        stage: &'static str,
        error: &c::Error,
        session: &c::RoleSession<'_, '_, '_>,
        view: &ResourceReservationView<'_>,
        clocks: &Clocks,
        registry: &Registry,
        marker: Option<&Path>,
    ) {
        let marker = marker.map(|path| {
            std::fs::symlink_metadata(path)
                .map(|m| (m.is_file(), m.file_type().is_symlink(), m.len()))
                .map_err(|error| error.kind())
        });
        let error = format!("{error:?}").chars().take(256).collect::<String>();
        eprintln!(
            "registered-clock native stage={stage} elapsed={:?} error={error} remaining_before={:?} remaining_after={:?} calls_before={:?} calls_after={:?} marker_metadata={marker:?}",
            self.started.elapsed(), self.remaining_before, view.remaining(),
            self.calls_before, Self::calls(clocks, registry),
        );
        let inspected = session.with_native_failure(|history| {
            let Some(history) = history else {
                eprintln!("registered-clock native stage={stage} history=none");
                return;
            };
            let cause = format!("{:?}", history.cause()).chars().take(256).collect::<String>();
            eprintln!(
                "registered-clock native stage={stage} cause={cause} entered_lower={} process_present={} parsed_outputs={:?}",
                history.lower_dispatch_entered(), history.process().is_some(), history.parsed_outputs(),
            );
            if let Some(process) = history.process() {
                let report = process.reported();
                let pin = report.executable_sha256.as_deref()
                    .map(|s| s.chars().take(80).collect::<String>());
                eprintln!(
                    "registered-clock native stage={stage} state={:?} exit={:?} timed_out={} diagnostic_present={} stdout_bytes={} stderr_bytes={} truncated={:?} kill_attempted={} kill_verified={} executable_pin={pin:?}",
                    report.state, report.exit_code, report.timed_out, report.diagnostic.is_some(),
                    process.stdout().len(), process.stderr().len(),
                    (report.stdout_truncated, report.stderr_truncated),
                    report.process_tree_kill_attempted, report.process_tree_kill_verified,
                );
            }
        });
        if let Err(error) = inspected {
            let error = format!("{error:?}").chars().take(256).collect::<String>();
            eprintln!("registered-clock native stage={stage} history_unavailable={error}");
        }
    }
}

pub fn resources(
    f: impl for<'v, 'lease> FnOnce(&'v ResourceReservationView<'lease>, &Cancellation),
) {
    let mut a = [0; lc631_substrate::resource::RESOURCE_KINDS];
    for (k, n) in [
        (ResourceKind::Ram, 256 * 1024 * 1024),
        (ResourceKind::Disk, 64 * 1024 * 1024),
        (ResourceKind::Output, 2 * 1024 * 1024),
        (ResourceKind::Child, 2),
        (ResourceKind::Cleanup, 2),
    ] {
        a[k.index()] = n;
    }
    let cap = ResourceLimits::checked(a).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let cancel = Cancellation::default();
    let mut run = broker
        .begin_run(
            3133,
            SourceRevision::of(native::SOURCE).digest(),
            cap,
            Deadline::after(Duration::from_secs(120)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let mut r = run
        .reserve(&[
            q(ResourceKind::Ram, 96 * 1024 * 1024),
            q(ResourceKind::Disk, 64 * 1024 * 1024),
            q(ResourceKind::Output, 2 * 1024 * 1024),
            q(ResourceKind::Child, 2),
            q(ResourceKind::Cleanup, 2),
        ])
        .unwrap();
    f(&r.lease(), &cancel);
    r.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}

pub fn full(
    f: impl for<'v, 'lease, 'p, 'o, 'bi, 'br, 'ci, 'cr> FnOnce(
        &c::Profile<'v>,
        &c::RegisteredArtifactRoot<'o, 'p, 'v>,
        c::RegisteredPair<'o, 'p, 'v>,
        &mut c::CurrentPorts<'o, 'bi, 'br, 'ci, 'cr>,
        &Clocks,
        &Arc<Registry>,
        &c::RoleSession<'o, 'p, 'v>,
        &c::RoleSession<'o, 'p, 'v>,
        &'v ResourceReservationView<'lease>,
    ),
) {
    full_staged(false, NativeBirthProfile::Default5s, |_| {}, f);
}

/// The same registered fixture stopped immediately before its first native
/// child. The birth-only executable records each start in its own root.
pub struct NativeBirth<'o, 'p, 'v, 'lease> {
    pub session: &'o c::RoleSession<'o, 'p, 'v>,
    pub origin: &'o c::RegisteredCapture<'o, 'p, 'v>,
    pub plan: &'o CodingProbePlan,
    pub input: c::ProbeInputs<'o>,
    pub clocks: &'o Clocks,
    pub registry: &'o Registry,
    pub marker: &'o Path,
    pub view: &'v ResourceReservationView<'lease>,
}
impl NativeBirth<'_, '_, '_, '_> {
    pub fn native_diagnostic(&self) -> NativeDiagnostics {
        NativeDiagnostics::start(self.view, self.clocks, self.registry)
    }
    pub fn report_native_diagnostic(
        &self,
        stage: &'static str,
        diagnostic: NativeDiagnostics,
        error: &c::Error,
    ) {
        diagnostic.report(
            stage,
            error,
            self.session,
            self.view,
            self.clocks,
            self.registry,
            Some(self.marker),
        );
    }
}

pub fn at_native_birth(
    inspect: impl for<'o, 'p, 'v, 'lease> FnOnce(NativeBirth<'o, 'p, 'v, 'lease>),
) {
    full_staged(
        true,
        NativeBirthProfile::Default5s,
        inspect,
        |_, _, _, _, _, _, _, _, _| {
            unreachable!("the birth-only fixture returns before constructing the pair")
        },
    );
}

/// Explicit supplemental positive profile; the original five-second paths stay fixed.
pub fn at_native_birth_supplemental_30s(
    inspect: impl for<'o, 'p, 'v, 'lease> FnOnce(NativeBirth<'o, 'p, 'v, 'lease>),
) {
    full_staged(
        true,
        NativeBirthProfile::Supplemental30s,
        inspect,
        |_, _, _, _, _, _, _, _, _| {
            unreachable!("the supplemental birth-only fixture returns before constructing the pair")
        },
    );
}

#[derive(Clone, Copy)]
enum NativeBirthProfile {
    Default5s,
    Supplemental30s,
}

fn full_staged(
    birth_only: bool,
    native_profile: NativeBirthProfile,
    inspect: impl for<'o, 'p, 'v, 'lease> FnOnce(NativeBirth<'o, 'p, 'v, 'lease>),
    f: impl for<'v, 'lease, 'p, 'o, 'bi, 'br, 'ci, 'cr> FnOnce(
        &c::Profile<'v>,
        &c::RegisteredArtifactRoot<'o, 'p, 'v>,
        c::RegisteredPair<'o, 'p, 'v>,
        &mut c::CurrentPorts<'o, 'bi, 'br, 'ci, 'cr>,
        &Clocks,
        &Arc<Registry>,
        &c::RoleSession<'o, 'p, 'v>,
        &c::RoleSession<'o, 'p, 'v>,
        &'v ResourceReservationView<'lease>,
    ),
) {
    assert!(birth_only || matches!(native_profile, NativeBirthProfile::Default5s));
    let mut diagnostics = FixtureDiagnostics::new();
    // New own roots only. Actual compile/native/capture happens only when central
    // selects this target; current source has no registered factory implementation.
    let entry = |s: &str| {
        s.replace("fn add(", "fn checked_add(")
            .replace("match add(", "match checked_add(")
    };
    let br = root(
        "base",
        &entry(&native::BEFORE.replace("{None}", "{Some(a.wrapping_add(b))}")),
    );
    let cr = root("next", &entry(native::AFTER));
    let marker = br.join("registered-native-starts.marker");
    if birth_only {
        let path = br.join("src/lib.rs");
        let source = std::fs::read_to_string(&path).unwrap();
        let marked = format!(
            "fn main(){{{{use std::io::Write;std::fs::OpenOptions::new().create(true).append(true).open({:?}).unwrap().write_all(b\"X\").unwrap();}}",
            marker.to_str().unwrap()
        );
        let source_with_marker = source.replacen("fn main(){", &marked, 1);
        assert_ne!(source, source_with_marker);
        std::fs::write(path, source_with_marker).unwrap();
    }
    let bb = br.join(format!("base{}", std::env::consts::EXE_SUFFIX));
    let cb = cr.join(format!("next{}", std::env::consts::EXE_SUFFIX));
    assert!(!bb.exists() && !cb.exists());
    diagnostics.stage("roots_and_marker_source_ready");
    native::compile_native(&br.join("src/lib.rs"), &bb);
    diagnostics.stage("baseline_compiled");
    native::compile_native(&cr.join("src/lib.rs"), &cb);
    diagnostics.stage("candidate_compiled");
    let pipeline = build_dgcl_pipeline(
        native::SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    diagnostics.stage("pipeline_ready");
    let scope = ContextScope::declared(d(21), d(22), d(23), d(24));
    let plan = CodingProbePlan::checked_add(scope.goal(), scope.semantics()).unwrap();
    let native_issuer =
        ReceiptIssuer::from_key_bytes("registered-clock-native-test", &[136; 32]).unwrap();
    let native_verifier =
        ReceiptVerifier::from_key_bytes("registered-clock-native-test", &[136; 32]).unwrap();
    // One shared accounting run also funds inventory readers. Do not make fresh
    // brokers with equal labels to bypass actual same-reservation checks.
    let cap = ResourceLimits::checked([
        256 * 1024 * 1024,
        64 * 1024 * 1024,
        2 * 1024 * 1024,
        0,
        2,
        2,
    ])
    .unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let mut run = broker
        .begin_run(
            3134,
            SourceRevision::of(native::SOURCE).digest(),
            cap,
            Deadline::after(Duration::from_secs(120)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut r = run
        .reserve(&[
            q(ResourceKind::Ram, 96 * 1024 * 1024),
            q(ResourceKind::Disk, 64 * 1024 * 1024),
            q(ResourceKind::Output, 2 * 1024 * 1024),
            q(ResourceKind::Child, 2),
            q(ResourceKind::Cleanup, 2),
        ])
        .unwrap();
    diagnostics.stage("shared_run_reserved");
    {
        let view = r.lease();
        let registry = Registry::new();
        let p = profile(&registry, &view);
        let clocks = Clocks::new();
        let mut b_reader = Reader(&run);
        let mut c_reader = Reader(&run);
        let mut bmeta = native::snapshot_authority(
            &br,
            &native_issuer,
            &native_verifier,
            100,
            AuthorityRevision(1),
        );
        let mut cmeta = native::snapshot_authority(
            &cr,
            &native_issuer,
            &native_verifier,
            100,
            AuthorityRevision(1),
        );
        let bs = {
            let a = bmeta.current().unwrap();
            capture_dgcl_cargo_snapshot_with_runner(
                &br,
                native::SOURCE,
                &pipeline,
                &a.context,
                &mut b_reader,
            )
            .unwrap()
        };
        let cs = {
            let a = cmeta.current().unwrap();
            capture_dgcl_cargo_snapshot_with_runner(
                &cr,
                native::SOURCE,
                &pipeline,
                &a.context,
                &mut c_reader,
            )
            .unwrap()
        };
        diagnostics.stage("metadata_snapshots_ready");
        let be = c::EndpointCandidate::execution(
            &p,
            bind(
                &br,
                &pipeline,
                &plan,
                c::Role::Baseline,
                c::Purpose::Execution,
            ),
            clocks.be.clone(),
            &view,
        )
        .unwrap();
        let bc = c::EndpointCandidate::capture(
            &p,
            bind(
                &br,
                &pipeline,
                &plan,
                c::Role::Baseline,
                c::Purpose::Capture,
            ),
            clocks.bc.clone(),
            Arc::new(Grant(Mutex::new(bmeta))),
            &view,
        )
        .unwrap();
        let ce = c::EndpointCandidate::execution(
            &p,
            bind(
                &cr,
                &pipeline,
                &plan,
                c::Role::Candidate,
                c::Purpose::Execution,
            ),
            clocks.ce.clone(),
            &view,
        )
        .unwrap();
        let cc = c::EndpointCandidate::capture(
            &p,
            bind(
                &cr,
                &pipeline,
                &plan,
                c::Role::Candidate,
                c::Purpose::Capture,
            ),
            clocks.cc.clone(),
            Arc::new(Grant(Mutex::new(cmeta))),
            &view,
        )
        .unwrap();
        let issuer = Registry::issuer();
        let ber = receipt(&be, &issuer);
        let bcr = receipt(&bc, &issuer);
        let cer = receipt(&ce, &issuer);
        let ccr = receipt(&cc, &issuer);
        let be = c::register(&p, be, ber, registry.clone(), &view).unwrap();
        let bc = c::register(&p, bc, bcr, registry.clone(), &view).unwrap();
        let ce = c::register(&p, ce, cer, registry.clone(), &view).unwrap();
        let cc = c::register(&p, cc, ccr, registry.clone(), &view).unwrap();
        diagnostics.stage("clock_endpoints_registered");
        let parse = |s: &str| declared_capture_digest(s).unwrap();
        let pg = parse(&pipeline.program_ir.program_digest);
        let pl = parse(&pipeline.completion_plan.plan_digest);
        let bbinding = SourceReadBinding::checked(
            &br,
            native::SOURCE,
            &pipeline,
            bind(
                &br,
                &pipeline,
                &plan,
                c::Role::Baseline,
                c::Purpose::Execution,
            )
            .root,
            pg,
            pl,
        )
        .unwrap();
        let cbinding = SourceReadBinding::checked(
            &cr,
            native::SOURCE,
            &pipeline,
            bind(
                &cr,
                &pipeline,
                &plan,
                c::Role::Candidate,
                c::Purpose::Execution,
            )
            .root,
            pg,
            pl,
        )
        .unwrap();
        let bsession = c::RoleSession::bind(&p, &be, &bc, &bbinding, &plan, &view).unwrap();
        let csession = c::RoleSession::bind(&p, &ce, &cc, &cbinding, &plan, &view).unwrap();
        diagnostics.stage("role_sessions_bound");
        let mut bp = CodingBuildPort::checked(
            &br,
            native::SOURCE,
            &pipeline,
            bbinding.root(),
            pg,
            pl,
            &mut b_reader,
        )
        .unwrap();
        let mut cp = CodingBuildPort::checked(
            &cr,
            native::SOURCE,
            &pipeline,
            cbinding.root(),
            pg,
            pl,
            &mut c_reader,
        )
        .unwrap();
        let before = bsession.capture_before(&mut bp, &view);
        let expected = csession.capture_before(&mut cp, &view);
        diagnostics.stage("actual_before_captures_returned");
        let bnf = native::NativeProbeFixture::registered(
            &plan,
            bs,
            br.join("src/lib.rs"),
            bb,
            &native_issuer,
            &native_verifier,
        );
        let cnf = native::NativeProbeFixture::registered(
            &plan,
            cs,
            cr.join("src/lib.rs"),
            cb,
            &native_issuer,
            &native_verifier,
        );
        diagnostics.stage("native_registrations_ready");
        bnf.with_invocation(&native_verifier, |mut bi| {
            assert_eq!(bi.timeout_ms, 5_000);
            if matches!(native_profile, NativeBirthProfile::Supplemental30s) {
                assert!(birth_only);
                bi.timeout_ms = 30_000;
                assert_eq!(bi.timeout_ms, 30_000);
            }
            cnf.with_invocation(&native_verifier, |ci| {
                assert_eq!(ci.timeout_ms, 5_000);
                eprintln!(
                    "registered-clock native configuration baseline_timeout_ms={} candidate_timeout_ms={} baseline_executable_pin={} candidate_executable_pin={}",
                    bi.timeout_ms, ci.timeout_ms,
                    bi.registration.executable_digest, ci.registration.executable_digest,
                );
                if birth_only {
                    diagnostics.stage("native_birth_callback_entered");
                    inspect(NativeBirth {
                        session: &bsession,
                        origin: before.current().unwrap(),
                        plan: &plan,
                        input: c::ProbeInputs::without_clock(&bi),
                        clocks: &clocks,
                        registry: &registry,
                        marker: &marker,
                        view: &view,
                    });
                    diagnostics.stage("native_birth_callback_returned");
                    return;
                }
                let baseline_diagnostic = NativeDiagnostics::start(&view, &clocks, &registry);
                let baseline_attempt = bsession
                    .observe_native(
                        &plan,
                        c::ProbeInputs::without_clock(&bi),
                        before.current().unwrap(),
                        &view,
                    );
                if let Err(error) = &baseline_attempt {
                    baseline_diagnostic.report(
                        "baseline_native", error, &bsession, &view, &clocks, &registry, None,
                    );
                }
                let b = baseline_attempt.unwrap();
                diagnostics.stage("baseline_native_returned");
                let candidate_diagnostic = NativeDiagnostics::start(&view, &clocks, &registry);
                let candidate_attempt = csession
                    .observe_native(
                        &plan,
                        c::ProbeInputs::without_clock(&ci),
                        expected.current().unwrap(),
                        &view,
                    );
                if let Err(error) = &candidate_attempt {
                    candidate_diagnostic.report(
                        "candidate_native", error, &csession, &view, &clocks, &registry, None,
                    );
                }
                let next = candidate_attempt.unwrap();
                diagnostics.stage("candidate_native_returned");
                assert_eq!(
                    b.actual_outputs(),
                    &[Some(0), Some(3), Some(0), Some(4294967295)]
                );
                assert_eq!(
                    next.actual_outputs(),
                    &[Some(0), Some(3), None, Some(4294967295)]
                );
                assert_eq!((b.checked_epoch(), next.checked_epoch()), (100, 100));
                assert!(
                    clocks.be.calls() >= 4 && clocks.ce.calls() >= 4,
                    "actual registered native worker must sample its retained endpoints"
                );
                assert_eq!(clocks.bc.calls(), 2);
                assert_eq!(clocks.cc.calls(), 2);
                let bcap = before
                    .actual_capture()
                    .expect("actual registered source inventory");
                assert_eq!(bcap.authority_before().checked_epoch, 100);
                assert_eq!(bcap.authority_after().unwrap().checked_epoch, 100);
                let root = c::RegisteredArtifactRoot::checked(
                    c::RegisteredRootInput {
                        plan: &plan,
                        pins: g::ArtifactPins {
                            context: ContextRevision::original(SourceRevision::of(native::SOURCE)),
                            scope,
                            definition: d(0xc1),
                            oracle: d(0xc2),
                            hard: d(25),
                            evidence: d(26),
                        },
                        limits: g::Limits {
                            max_queries: 4,
                            max_pairs: 1,
                            max_current_checks: 8,
                            max_added_ram_bytes: 64 * 1024,
                            max_summary_bytes: 64 * 1024,
                        },
                        baseline_session: &bsession,
                        candidate_session: &csession,
                        baseline_before: before.current().unwrap(),
                        candidate_expected: expected.current().unwrap(),
                    },
                    &view,
                )
                .unwrap();
                diagnostics.stage("registered_root_ready");
                let pair = c::RegisteredPair {
                    baseline: &b,
                    candidate: &next,
                };
                let mut ports = c::CurrentPorts {
                    baseline_probe: c::ProbeInputs::without_clock(&bi),
                    candidate_probe: c::ProbeInputs::without_clock(&ci),
                    baseline_build: &mut bp,
                    candidate_build: &mut cp,
                };
                f(
                    &p, &root, pair, &mut ports, &clocks, &registry, &bsession, &csession, &view,
                );
                diagnostics.stage("registered_pair_callback_returned");
            })
        });
    }
    r.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}
pub fn with_binding(
    f: impl for<'a, 'v, 'lease> FnOnce(
        &SourceReadBinding<'a>,
        &CodingProbePlan,
        Arc<Grant>,
        &'v ResourceReservationView<'lease>,
    ),
) {
    let r = root(
        "binding",
        &native::AFTER
            .replace("fn add(", "fn checked_add(")
            .replace("match add(", "match checked_add("),
    );
    let pipeline = build_dgcl_pipeline(
        native::SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let plan = CodingProbePlan::checked_add(d(21), d(24)).unwrap();
    let issuer = ReceiptIssuer::from_key_bytes("registered-clock-native-test", &[136; 32]).unwrap();
    let verifier =
        ReceiptVerifier::from_key_bytes("registered-clock-native-test", &[136; 32]).unwrap();
    let grant = Arc::new(Grant(Mutex::new(native::snapshot_authority(
        &r,
        &issuer,
        &verifier,
        100,
        AuthorityRevision(1),
    ))));
    let parse = |s: &str| declared_capture_digest(s).unwrap();
    let binding = SourceReadBinding::checked(
        &r,
        native::SOURCE,
        &pipeline,
        parse(&lc631_analysis::dgcl_repository_root_digest(&r).unwrap()),
        parse(&pipeline.program_ir.program_digest),
        parse(&pipeline.completion_plan.plan_digest),
    )
    .unwrap();
    resources(|view, _| f(&binding, &plan, grant, view));
}
pub fn role_binding(
    binding: &SourceReadBinding<'_>,
    plan: &CodingProbePlan,
    role: c::Role,
    purpose: c::Purpose,
) -> c::EndpointBinding {
    c::EndpointBinding {
        role,
        purpose,
        root: binding.root(),
        source: binding.source(),
        program: binding.program(),
        completion_plan: binding.plan(),
        probe_plan: plan.digest(),
        endpoint_revision: d(91),
    }
}
