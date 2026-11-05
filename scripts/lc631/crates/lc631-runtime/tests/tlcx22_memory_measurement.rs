mod tlcx22_memory_support;
use lc631_analysis::creativity_measurement::{
    ComponentMetric, ComponentState, IndependenceStatus, InputBasis, SemanticValidation,
};
use lc631_core::measurement_numeric::{BigInt, Rational};
use lc631_runtime::memory_measurement::*;
use lc631_substrate::resource::{Cancellation, ResourceError};
use tlcx22_memory_support::*;

#[test]
fn tlcx22_memory_g14_g27_actual_owned_paths_and_canonical_report() {
    for (f, k, expected, cost) in [
        (
            Fixture::three(vec![bounds(3, 1, 3, 1)], vec![(1, 3, 0)]),
            q(1, 1),
            bounds(3, 4, 3, 4),
            bounds(3, 1, 3, 1),
        ),
        (
            Fixture::three(
                vec![bounds(1, 1, 2, 1), bounds(1, 1, 2, 1), bounds(3, 1, 4, 1)],
                vec![(1, 2, 0), (2, 3, 1), (1, 3, 2)],
            ),
            q(2, 1),
            bounds(1, 2, 2, 3),
            bounds(2, 1, 4, 1),
        ),
    ] {
        let pairs = f.pairs();
        let b = binding(1);
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
        let (_broker, _run, reservation) =
            run(Cancellation::default(), b.selection.source.digest());
        let view = reservation.lease();
        let owner = execute_owned(&plan, &view).unwrap();
        assert_eq!(owner.historical_cost(), Some(&cost));
        let report = owner.report_current(&plan, &view).unwrap();
        assert_eq!(report.metric, ComponentMetric::BridgeCost);
        assert_eq!(report.basis, InputBasis::Given);
        assert_eq!(report.validation, SemanticValidation::Unassessed);
        assert_eq!(report.independence, IndependenceStatus::SharedDependency);
        assert_eq!(report.state, ComponentState::Measured(expected));
        assert_eq!(owner.historical_path_state(), PathState::Connected);
        assert!(owner.lower_path().is_some());
        assert!(owner.upper_path().is_some());
    }
}

#[test]
fn tlcx22_memory_different_endpoint_winners_are_two_witnesses() {
    let f = Fixture::three(
        vec![bounds(1, 1, 10, 1), bounds(1, 1, 3, 2), bounds(1, 1, 3, 2)],
        vec![(1, 3, 0), (1, 2, 1), (2, 3, 2)],
    );
    let pairs = f.pairs();
    let b = binding(1);
    let k = q(1, 1);
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
    let (_broker, _run, reservation) = run(Cancellation::default(), b.selection.source.digest());
    let view = reservation.lease();
    let owner = execute_owned(&plan, &view).unwrap();
    assert_eq!(owner.historical_cost(), Some(&bounds(1, 1, 3, 1)));
    assert_eq!(
        owner.lower_path().unwrap().states(),
        &[StateId(1), StateId(3)]
    );
    assert_eq!(
        owner.upper_path().unwrap().states(),
        &[StateId(1), StateId(2), StateId(3)]
    );
    assert_eq!(
        owner.historical_report().state,
        ComponentState::Measured(bounds(1, 2, 3, 4))
    );
}

#[test]
fn tlcx22_memory_open_no_path_is_unknown_closed_no_path_disconnected() {
    let f = Fixture::three(vec![], vec![]);
    let pairs = f.pairs();
    let b = binding(1);
    let k = q(1, 1);
    let closed = RegistryPlan::declared(
        b,
        &f.states,
        &pairs,
        StateId(1),
        StateId(3),
        unit(),
        &k,
        budget(),
    );
    let open = RegistryPlan::declared(
        b,
        &f.states,
        &pairs[..pairs.len() - 1],
        StateId(1),
        StateId(3),
        unit(),
        &k,
        budget(),
    );
    let (_broker, _run, reservation) = run(Cancellation::default(), b.selection.source.digest());
    let view = reservation.lease();
    let a = execute_owned(&closed, &view).unwrap();
    let z = execute_owned(&open, &view).unwrap();
    assert_eq!(a.historical_path_state(), PathState::Disconnected);
    assert_eq!(
        a.historical_report().state,
        ComponentState::Measured(bounds(1, 1, 1, 1))
    );
    assert_eq!(z.historical_path_state(), PathState::Unresolved);
    assert!(matches!(
        z.historical_report().state,
        ComponentState::Unknown(_)
    ));
    assert!(z.historical_cost().is_none());
}

#[test]
fn tlcx22_memory_zero_cost_cycles_terminate_canonical_ties_ignore_ids_and_order() {
    let f = Fixture::three(
        vec![bounds(0, 1, 0, 1), bounds(1, 1, 1, 1)],
        vec![(1, 2, 0), (2, 1, 0), (1, 3, 1), (2, 3, 1)],
    );
    let p = f.pairs();
    let b = binding(1);
    let k = q(1, 1);
    let (_broker, _run, reservation) = run(Cancellation::default(), b.selection.source.digest());
    let view = reservation.lease();
    let plan = RegistryPlan::declared(
        b,
        &f.states,
        &p,
        StateId(1),
        StateId(3),
        unit(),
        &k,
        budget(),
    );
    let first = execute_owned(&plan, &view).unwrap();
    assert!(first.lower_path().unwrap().states().len() <= 3);
    let first_key = first.lower_path().unwrap().canonical_payload().to_vec();
    let mut reordered = p.clone();
    reordered.reverse();
    let n = [
        State {
            id: StateId(90),
            canonical_payload: b"C",
        },
        State {
            id: StateId(4),
            canonical_payload: b"A",
        },
        State {
            id: StateId(71),
            canonical_payload: b"B",
        },
    ];
    let remap = |id: StateId| match id.0 {
        1 => StateId(4),
        2 => StateId(71),
        3 => StateId(90),
        _ => panic!(),
    };
    for pair in &mut reordered {
        pair.from = remap(pair.from);
        pair.to = remap(pair.to);
    }
    let changed = RegistryPlan::declared(
        b,
        &n,
        &reordered,
        StateId(4),
        StateId(90),
        unit(),
        &k,
        budget(),
    );
    let second = execute_owned(&changed, &view).unwrap();
    assert_eq!(first_key, second.lower_path().unwrap().canonical_payload());
    assert_eq!(first.historical_report(), second.historical_report());
}

#[test]
fn tlcx22_memory_raw_kappa_and_costs_rejected_before_numeric_comparison() {
    let f = Fixture::three(vec![], vec![]);
    let pairs = f.pairs();
    let b = binding(1);
    let (_broker, _run, reservation) = run(Cancellation::default(), b.selection.source.digest());
    let view = reservation.lease();
    for k in [
        q(0, 1),
        q(-1, 1),
        Rational::new_raw(1.into(), 0.into()),
        Rational::new_raw(1.into(), (-1).into()),
        Rational::new_raw(2.into(), 2.into()),
        Rational::new_raw(BigInt::from(1) << 65_537, 1.into()),
    ] {
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
        assert!(execute_owned(&plan, &view).is_err());
    }
    let bad = Rational::new_raw(1.into(), 0.into());
    let k = q(1, 1);
    let pair = [Pair {
        from: StateId(1),
        to: StateId(3),
        disposition: PairDisposition::Allowed {
            unit: unit(),
            lower: &bad,
            upper: &k,
            canonical_payload: b"bad-raw",
            evidence: d(50),
            evidence_payload: b"Given invalid input",
        },
    }];
    let plan = RegistryPlan::declared(
        b,
        &f.states,
        &pair,
        StateId(1),
        StateId(3),
        unit(),
        &k,
        budget(),
    );
    assert!(execute_owned(&plan, &view).is_err());
}

#[test]
fn tlcx22_memory_unit_alias_duplicate_pair_source_and_budget_cuts() {
    let f = Fixture::three(vec![bounds(1, 1, 1, 1)], vec![(1, 3, 0)]);
    let mut pairs = f.pairs();
    let b = binding(1);
    let k = q(1, 1);
    let (_broker, _run, reservation) = run(Cancellation::default(), b.selection.source.digest());
    let view = reservation.lease();
    if let PairDisposition::Allowed { unit, .. } = &mut pairs
        .iter_mut()
        .find(|p| p.from == StateId(1) && p.to == StateId(3))
        .unwrap()
        .disposition
    {
        *unit = CostUnit(d(99));
    }
    let bad = RegistryPlan::declared(
        b,
        &f.states,
        &pairs,
        StateId(1),
        StateId(3),
        unit(),
        &k,
        budget(),
    );
    assert!(execute_owned(&bad, &view).is_err());
    let mut aliases = f.states.clone();
    aliases[1].canonical_payload = aliases[0].canonical_payload;
    let ps = f.pairs();
    let bad = RegistryPlan::declared(
        b,
        &aliases,
        &ps,
        StateId(1),
        StateId(3),
        unit(),
        &k,
        budget(),
    );
    assert!(execute_owned(&bad, &view).is_err());
    let tiny = RegistryPlan::declared(
        b,
        &f.states,
        &ps,
        StateId(1),
        StateId(3),
        unit(),
        &k,
        WorkBudget { max_steps: 1 },
    );
    assert!(matches!(
        execute_owned(&tiny, &view),
        Err(Error::WorkBudget)
    ));
    let (_broker, _run, wrong) = run(Cancellation::default(), d(99));
    let valid = RegistryPlan::declared(
        b,
        &f.states,
        &ps,
        StateId(1),
        StateId(3),
        unit(),
        &k,
        budget(),
    );
    assert!(matches!(
        execute_owned(&valid, &wrong.lease()),
        Err(Error::Resource(ResourceError::SourceMismatch))
    ));
}

#[test]
fn tlcx22_memory_cancelled_current_owner_and_foreign_plan_cannot_project() {
    let f = Fixture::three(vec![bounds(1, 1, 1, 1)], vec![(1, 3, 0)]);
    let pairs = f.pairs();
    let b = binding(1);
    let k = q(1, 1);
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
    let foreign = RegistryPlan::declared(
        b,
        &f.states,
        &pairs,
        StateId(1),
        StateId(3),
        unit(),
        &k,
        budget(),
    );
    let cancel = Cancellation::default();
    let (_broker, _run, reservation) = run(cancel.clone(), b.selection.source.digest());
    let view = reservation.lease();
    let owner = execute_owned(&plan, &view).unwrap();
    assert!(owner.report_current(&foreign, &view).is_err());
    cancel.cancel();
    assert!(matches!(
        owner.report_current(&plan, &view),
        Err(Error::Resource(ResourceError::Cancelled))
    ));
    assert!(matches!(
        owner.historical_report().state,
        ComponentState::Measured(_)
    ));
}

// Independent enumeration of all SIMPLE paths for three states; no Dijkstra call.
fn oracle(costs: &[(usize, usize, i64)], at: usize, target: usize, seen: u8) -> Option<i64> {
    if at == target {
        return Some(0);
    }
    costs
        .iter()
        .filter(|(from, to, _)| *from == at && seen & (1 << to) == 0)
        .filter_map(|(_, to, c)| oracle(costs, *to, target, seen | (1 << to)).map(|tail| c + tail))
        .min()
}
#[test]
fn tlcx22_memory_independent_all_small_paths_integer_oracle() {
    for weights in [[1, 2, 4], [0, 1, 1], [3, 0, 2], [2, 2, 3]] {
        let costs = weights.iter().map(|&v| bounds(v, 1, v, 1)).collect();
        let f = Fixture::three(costs, vec![(1, 2, 0), (2, 3, 1), (1, 3, 2)]);
        let pairs = f.pairs();
        let b = binding(1);
        let k = q(1, 1);
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
        let (_broker, _run, reservation) =
            run(Cancellation::default(), b.selection.source.digest());
        let view = reservation.lease();
        let owner = execute_owned(&plan, &view).unwrap();
        let expected = oracle(
            &[(0, 1, weights[0]), (1, 2, weights[1]), (0, 2, weights[2])],
            0,
            2,
            1,
        )
        .unwrap();
        assert_eq!(
            owner.historical_cost(),
            Some(&bounds(expected, 1, expected, 1))
        );
    }
}
