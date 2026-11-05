//! 日本語: 実brokerの確定expired入口と逐次RAM cuts。既存9試験は変更しない。
//! English: Actual-broker expired-entry and staged RAM cuts; preserve the frozen nine tests.
// Reuse the frozen input fixture, not its producer/oracle expectations. Some
// helpers serve only the original target; this test-only module permits those.
#[allow(dead_code)]
#[path = "tlcx22_bridge_actual_support/mod.rs"]
mod support;
use lc631_runtime::{
    bridge_gain_measurement as b, compatibility_measurement as c,
    resource::{ResourceBroker, ResourceLimits},
};
use lc631_substrate::resource::{
    Cancellation, Deadline, ResourceError, ResourceKind, ResourceReservationView,
};
use std::time::{Duration, Instant};
use support::{bounds, ratio, resource, root_input, spec, Fixture, NORMAL};

fn fixture(
    ram: u64,
    expired: bool,
    body: impl FnOnce(&Fixture, &[c::ProbeCase<'_>], &ResourceReservationView<'_>),
) {
    let fx = Fixture::new(NORMAL);
    let cases = fx.cases();
    let limits = ResourceLimits::uniform(64 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let deadline = if expired {
        Deadline::after(Duration::from_secs(1)).unwrap()
    } else {
        Deadline::after(Duration::from_secs(60)).unwrap()
    };
    let mut run = broker
        .begin_run(
            2204,
            fx.source.id().digest(),
            limits,
            deadline,
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            resource(ResourceKind::Ram, ram),
            resource(ResourceKind::Output, 2 * 1024 * 1024),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        if expired {
            expire(&view);
        }
        body(&fx, &cases, &view);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
    assert_eq!(broker.used(ResourceKind::Output).unwrap(), 0);
}
fn expire(view: &ResourceReservationView<'_>) {
    // Wait for the actual monotonic deadline, rather than assume a short sleep
    // crossed it. Never alter system time or the production clock/resource API.
    loop {
        match view.remaining() {
            Ok(left) => std::thread::sleep(left),
            Err(ResourceError::DeadlineExceeded) => break,
            Err(e) => panic!("unexpected expiry cause {e:?}"),
        }
    }
    assert_eq!(view.check(), Err(ResourceError::DeadlineExceeded));
}

#[test]
fn tlcx22_bridge_expired_on_entry_is_deterministic_and_cannot_admit_a_root() {
    fixture(16 * 1024 * 1024, true, |fx, cases, view| {
        let base = fx.forward(true);
        let bi = Fixture::inverse(&base);
        assert!(matches!(
            b::BridgeRoot::given(root_input(fx.input(cases), spec(&base, &bi), 48), view),
            Err(b::BridgeError::Resource(ResourceError::DeadlineExceeded))
        ));
    });
}

#[test]
fn tlcx22_bridge_deadline_rejects_already_owned_pair_without_cached_gain() {
    let fx = Fixture::new(NORMAL);
    let cases = fx.cases();
    let limits = ResourceLimits::uniform(64 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            2205,
            fx.source.id().digest(),
            limits,
            Deadline::at(Instant::now() + Duration::from_secs(3)),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            resource(ResourceKind::Ram, 16 * 1024 * 1024),
            resource(ResourceKind::Output, 2 * 1024 * 1024),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        let base = fx.forward(true);
        let bi = Fixture::inverse(&base);
        let good = fx.forward(false);
        let gi = Fixture::inverse(&good);
        let root = b::BridgeRoot::given(root_input(fx.input(&cases), spec(&base, &bi), 48), &view)
            .unwrap();
        let bp = root.baseline_program(&view).unwrap();
        let cp = root.candidate_program(spec(&good, &gi), &view).unwrap();
        let bb = root.prepare(&bp, &view).unwrap();
        let cb = root.prepare(&cp, &view).unwrap();
        let bu = bb
            .execute_uses(&view)
            .unwrap_or_else(|_| panic!("baseline use"));
        let cu = cb
            .execute_uses(&view)
            .unwrap_or_else(|_| panic!("candidate use"));
        let gain = b::measure_pair(&root, &bu, &cu, &view).unwrap();
        let historical = gain.report_current(&root, &cp, &view).unwrap().clone();
        assert_eq!(bounds(&historical.state), &ratio(1, 2));
        expire(&view);
        assert!(matches!(
            gain.report_current(&root, &cp, &view),
            Err(b::BridgeError::Resource(ResourceError::DeadlineExceeded))
        ));
        assert!(matches!(
            cu.use_owner_current(0, &view),
            Err(b::BridgeError::Resource(ResourceError::DeadlineExceeded))
        ));
        assert!(matches!(
            b::measure_pair(&root, &bu, &cu, &view),
            Err(b::BridgeError::Resource(ResourceError::DeadlineExceeded))
        ));
        assert_eq!(bounds(&historical.state), &ratio(1, 2));
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx22_bridge_ram_cuts_root_program_preparation_and_fresh_use_without_zero() {
    for ram in [1, 256 * 1024, 512 * 1024, 2 * 1024 * 1024] {
        fixture(ram, false, |fx, cases, view| {
            let map = fx.forward(true);
            let inverse = Fixture::inverse(&map);
            let input = root_input(fx.input(cases), spec(&map, &inverse), 48);
            let root = match b::BridgeRoot::given(input, view) {
                Ok(root) => root,
                Err(b::BridgeError::Resource(ResourceError::Exhausted(ResourceKind::Ram))) => {
                    assert_eq!(ram, 1);
                    return;
                }
                Err(e) => panic!("unexpected root error {e:?}"),
            };
            let program = match root.baseline_program(view) {
                Ok(program) => program,
                Err(b::BridgeError::Resource(ResourceError::Exhausted(ResourceKind::Ram))) => {
                    assert_eq!(ram, 256 * 1024);
                    assert_eq!(root.work_used_current(view).unwrap(), 0);
                    return;
                }
                Err(e) => panic!("unexpected program error {e:?}"),
            };
            let branch = match root.prepare(&program, view) {
                Ok(branch) => branch,
                Err(b::BridgeError::Resource(ResourceError::Exhausted(ResourceKind::Ram))) => {
                    assert_eq!(ram, 512 * 1024);
                    assert_eq!(root.work_used_current(view).unwrap(), 0);
                    return;
                }
                Err(e) => panic!("unexpected preparation error {e:?}"),
            };
            assert_eq!(ram, 2 * 1024 * 1024);
            let failure = branch
                .execute_uses(view)
                .err()
                .expect("fresh-use scratch exceeds the fixed RAM cut");
            assert!(matches!(
                failure.cause(),
                b::BridgeError::Resource(ResourceError::Exhausted(ResourceKind::Ram))
            ));
            assert_eq!(failure.completed_use_count(), 0);
            assert_eq!(failure.scalar(), None);
            assert_eq!(root.work_used_current(view).unwrap(), 16);
            drop(failure);
            assert_eq!(root.work_used_current(view).unwrap(), 16);
        });
    }
}

#[test]
fn tlcx22_bridge_ram_cut_at_pair_measurement_preserves_actual_uses_and_work() {
    fixture(16 * 1024 * 1024, false, |fx, cases, view| {
        let base = fx.forward(true);
        let bi = Fixture::inverse(&base);
        let good = fx.forward(false);
        let gi = Fixture::inverse(&good);
        let root =
            b::BridgeRoot::given(root_input(fx.input(cases), spec(&base, &bi), 48), view).unwrap();
        let bp = root.baseline_program(view).unwrap();
        let cp = root.candidate_program(spec(&good, &gi), view).unwrap();
        let bb = root.prepare(&bp, view).unwrap();
        let cb = root.prepare(&cp, view).unwrap();
        let bu = bb.execute_uses(view).unwrap_or_else(|_| panic!("base use"));
        let cu = cb
            .execute_uses(view)
            .unwrap_or_else(|_| panic!("candidate use"));
        let mut fills = Vec::new();
        let mut exhausted = false;
        // Fill remaining logical RAM in bounded 4KiB chunks, avoiding a derived
        // production-layout oracle. At most 4096 chunks fit a16MiB reservation.
        for _ in 0..4096 {
            match view.claim_resources(&[resource(ResourceKind::Ram, 4096)]) {
                Ok(fill) => fills.push(fill),
                Err(ResourceError::Exhausted(ResourceKind::Ram)) => {
                    exhausted = true;
                    break;
                }
                Err(e) => panic!("unexpected fill error {e:?}"),
            }
        }
        assert!(exhausted);
        assert!(matches!(
            b::measure_pair(&root, &bu, &cu, view),
            Err(b::BridgeError::Resource(ResourceError::Exhausted(
                ResourceKind::Ram
            )))
        ));
        assert_eq!(root.work_used_current(view).unwrap(), 48);
        for case in 0..4 {
            assert_eq!(
                cu.use_owner_current(case, view).unwrap().actual_use_count(),
                2
            );
        }
        drop(fills);
        let gain = b::measure_pair(&root, &bu, &cu, view).unwrap();
        assert_eq!(
            bounds(&gain.report_current(&root, &cp, view).unwrap().state),
            &ratio(1, 2)
        );
        assert_eq!(root.work_used_current(view).unwrap(), 48);
    });
}
