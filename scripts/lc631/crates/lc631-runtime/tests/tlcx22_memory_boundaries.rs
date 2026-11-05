mod tlcx22_memory_support;
use lc631_core::CandidateId;
use lc631_runtime::memory_measurement::*;
use lc631_runtime::resource::{ResourceBroker, ResourceLimits};
use lc631_substrate::resource::{Cancellation, Deadline, ResourceKind};
use std::time::Duration;
use tlcx22_memory_support::*;

#[test]
fn tlcx22_memory_duplicate_pair_negative_edge_reversed_interval_and_probe_cut() {
    let f = Fixture::three(vec![bounds(1, 1, 1, 1)], vec![(1, 3, 0)]);
    let b = binding(1);
    let k = q(1, 1);
    let negative = q(-1, 1);
    let zero = q(0, 1);
    let two = q(2, 1);
    let (_broker, _run, res) = run(Cancellation::default(), b.selection.source.digest());
    let view = res.lease();
    let mut pairs = f.pairs();
    pairs.push(Pair {
        from: StateId(1),
        to: StateId(2),
        disposition: PairDisposition::Excluded { basis: d(51) },
    });
    let plan = RegistryPlan::declared(
        b,
        &f.states,
        &pairs,
        StateId(1),
        StateId(3),
        unit(),
        &k,
        budget(),
    );
    assert!(matches!(
        execute_owned(&plan, &view),
        Err(Error::DuplicatePair)
    ));
    for (lo, hi, expected) in [
        (&negative, &zero, Error::NegativeCost),
        (&two, &zero, Error::ReversedCost),
    ] {
        let mut pairs = f.pairs();
        pairs[0].disposition = PairDisposition::Allowed {
            unit: unit(),
            lower: lo,
            upper: hi,
            canonical_payload: b"conversion",
            evidence: d(50),
            evidence_payload: b"Given provenance",
        };
        let plan = RegistryPlan::declared(
            b,
            &f.states,
            &pairs,
            StateId(1),
            StateId(3),
            unit(),
            &k,
            budget(),
        );
        assert!(matches!(execute_owned(&plan,&view),Err(e) if e==expected));
    }
    let pairs = f.pairs();
    let mut bad = b;
    bad.probe_set = d(200);
    let plan = RegistryPlan::declared(
        bad,
        &f.states,
        &pairs,
        StateId(1),
        StateId(3),
        unit(),
        &k,
        budget(),
    );
    assert!(matches!(
        execute_owned(&plan, &view),
        Err(Error::BindingMismatch)
    ));
}
#[test]
fn tlcx22_memory_opaque_candidate_and_receipt_relabel_preserves_canonical_evidence() {
    let f = Fixture::three(vec![bounds(1, 1, 1, 1)], vec![(1, 3, 0)]);
    let pairs = f.pairs();
    let b = binding(1);
    let mut relabel = b;
    relabel.candidate = CandidateId(9876);
    let mut other = f.pairs();
    for pair in &mut other {
        if let PairDisposition::Allowed { evidence, .. } = &mut pair.disposition {
            *evidence = d(201);
        }
    }
    let k = q(1, 1);
    let p = RegistryPlan::declared(
        b,
        &f.states,
        &pairs,
        StateId(1),
        StateId(3),
        unit(),
        &k,
        budget(),
    );
    let r = RegistryPlan::declared(
        relabel,
        &f.states,
        &other,
        StateId(1),
        StateId(3),
        unit(),
        &k,
        budget(),
    );
    let (_broker, _run, res) = run(Cancellation::default(), b.selection.source.digest());
    let view = res.lease();
    let a = execute_owned(&p, &view).unwrap();
    let c = execute_owned(&r, &view).unwrap();
    assert_eq!(a.historical_report(), c.historical_report());
    assert_eq!(
        a.lower_path().unwrap().canonical_payload(),
        c.lower_path().unwrap().canonical_payload()
    );
    assert_ne!(
        a.historical_evidence_refs().collect::<Vec<_>>(),
        c.historical_evidence_refs().collect::<Vec<_>>()
    );
}
#[test]
fn tlcx22_memory_positive_unit_scaling_and_canonical_zero_cycle_tie() {
    let b = binding(1);
    let (_broker, _run, res) = run(Cancellation::default(), b.selection.source.digest());
    let view = res.lease();
    let f = Fixture::three(
        vec![bounds(0, 1, 0, 1), bounds(1, 1, 1, 1)],
        vec![(1, 2, 0), (2, 1, 0), (1, 3, 1), (2, 3, 1)],
    );
    let pairs = f.pairs();
    let k = q(1, 1);
    let p = RegistryPlan::declared(
        b,
        &f.states,
        &pairs,
        StateId(1),
        StateId(3),
        unit(),
        &k,
        budget(),
    );
    let a = execute_owned(&p, &view).unwrap();
    assert_eq!(
        a.lower_path().unwrap().states(),
        &[StateId(1), StateId(2), StateId(3)]
    );
    let g = Fixture::three(
        vec![bounds(0, 1, 0, 1), bounds(7, 1, 7, 1)],
        vec![(1, 2, 0), (2, 1, 0), (1, 3, 1), (2, 3, 1)],
    );
    let mut gp = g.pairs();
    let scaled = CostUnit(d(202));
    for pair in &mut gp {
        if let PairDisposition::Allowed { unit, .. } = &mut pair.disposition {
            *unit = scaled;
        }
    }
    let kk = q(7, 1);
    let p2 = RegistryPlan::declared(
        b,
        &g.states,
        &gp,
        StateId(1),
        StateId(3),
        scaled,
        &kk,
        budget(),
    );
    let c = execute_owned(&p2, &view).unwrap();
    assert_eq!(a.historical_report().state, c.historical_report().state);
    assert_eq!(
        a.lower_path().unwrap().states(),
        c.lower_path().unwrap().states()
    );
    assert_ne!(a.historical_model_digest(), c.historical_model_digest());
}
#[test]
fn tlcx22_memory_low_ram_fails_before_publication_and_claim_is_refunded() {
    let f = Fixture::three(vec![bounds(1, 1, 1, 1)], vec![(1, 3, 0)]);
    let pairs = f.pairs();
    let b = binding(1);
    let k = q(1, 1);
    let p = RegistryPlan::declared(
        b,
        &f.states,
        &pairs,
        StateId(1),
        StateId(3),
        unit(),
        &k,
        budget(),
    );
    // 32 MiB fails the first scratch admission; 64 MiB admits scratch
    // then rejects retained ownership, exercising the scratch claim's refund.
    for quota in [32 * 1024 * 1024, 64 * 1024 * 1024] {
        let broker =
            ResourceBroker::new(ResourceLimits::uniform(128 * 1024 * 1024).unwrap()).unwrap();
        let run = broker
            .begin_run(
                1,
                b.selection.source.digest(),
                ResourceLimits::uniform(64 * 1024 * 1024).unwrap(),
                Deadline::after(Duration::from_secs(120)).unwrap(),
                Cancellation::default(),
            )
            .unwrap();
        let res = run
            .reserve(&[
                request(ResourceKind::Ram, quota),
                request(ResourceKind::Output, 8 * 1024 * 1024),
            ])
            .unwrap();
        let view = res.lease();
        assert!(matches!(execute_owned(&p, &view), Err(Error::Resource(_))));
        let available = view
            .claim_resources(&[request(ResourceKind::Ram, quota)])
            .unwrap();
        available.check().unwrap();
    }
}
