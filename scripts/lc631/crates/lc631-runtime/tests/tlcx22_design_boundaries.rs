mod tlcx22_design_support;
use lc631_analysis::creativity_measurement::{ComponentState, MeasurementGap};
use lc631_runtime::design_measurement::*;
use lc631_substrate::resource::Cancellation;
use tlcx22_design_support::*;
#[test]
fn tlcx22_design_tick_limits_include_final_completion_without_extra_interval() {
    for (n, duration, replay, valid) in [(1, 1, 1, true), (8, 32, 256, true), (8, 32, 255, false)] {
        let mut f = Fixture::gold();
        let names: [&[u8]; 8] = [b"a", b"b", b"c", b"d", b"e", b"f", b"g", b"h"];
        f.jobs = (0..n)
            .map(|i| Job {
                id: JobId(i as u16),
                canonical_payload: names[i],
                duration_ticks: duration,
            })
            .collect();
        f.baseline = (0..n).map(|i| JobId(i as u16)).collect();
        f.m = q(0, 1);
        f.upper = q(256, 1);
        f.accepted_high = q(256, 1);
        let (_br, _run, res) = run(Cancellation::default());
        let view = res.lease();
        let mut l = limits();
        l.max_replay_ticks_per_run = replay;
        let result = register(
            binding(),
            &[Workload {
                workers: 1,
                ..f.workload(b"case")
            }],
            l,
            &view,
        );
        if valid {
            let suite = result.unwrap();
            assert_eq!(
                suite.baseline_observation(0).unwrap().raw.ticks,
                ModelTicks(n as u16 * duration)
            );
        } else {
            assert!(matches!(result, Err(Error::PrerequisitePending)));
        }
    }
}
#[test]
fn tlcx22_design_simultaneous_completions_skip_blocked_head_and_invalid_dag() {
    let mut f = Fixture::gold();
    f.jobs[0].duration_ticks = 2;
    f.jobs[2].duration_ticks = 1;
    f.dependencies = vec![(JobId(11), JobId(33)), (JobId(22), JobId(33))];
    f.baseline = vec![JobId(33), JobId(11), JobId(22)];
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let suite = register(binding(), &[f.workload(b"case")], limits(), &view).unwrap();
    let trace = suite.baseline_schedule(0).unwrap();
    let last = trace.jobs.iter().find(|j| j.job == JobId(33)).unwrap();
    assert_eq!(last.start, ModelTicks(2));
    assert_eq!(last.end, ModelTicks(3));
    assert_eq!(last.worker, 0);
    let mut cycle = Fixture::gold();
    cycle.dependencies = vec![(JobId(11), JobId(22)), (JobId(22), JobId(11))];
    assert!(matches!(
        register(binding(), &[cycle.workload(b"case")], limits(), &view),
        Err(Error::Cycle)
    ));
    cycle.dependencies = vec![(JobId(99), JobId(11))];
    assert!(matches!(
        register(binding(), &[cycle.workload(b"case")], limits(), &view),
        Err(Error::InvalidDependency)
    ));
    cycle.dependencies = vec![(JobId(11), JobId(22)), (JobId(11), JobId(22))];
    assert!(matches!(
        register(binding(), &[cycle.workload(b"case")], limits(), &view),
        Err(Error::InvalidDependency)
    ));
}
#[test]
fn tlcx22_design_frozen_event_batch_and_replay_budgets_leave_unknown_not_zero() {
    for event_cut in [true, false] {
        let mut f = Fixture::gold();
        if !event_cut {
            f.baseline = improved().to_vec();
        }
        let (_br, _run, res) = run(Cancellation::default());
        let view = res.lease();
        let mut l = limits();
        if event_cut {
            l.max_events_per_run = 3;
        } else {
            l.max_replay_ticks_per_run = 4;
        }
        let suite = register(binding(), &[f.workload(b"case")], l, &view).unwrap();
        let candidate_order = if event_cut {
            improved().to_vec()
        } else {
            vec![JobId(22), JobId(33), JobId(11)]
        };
        let frozen = bind_candidate(
            &suite,
            candidate(1),
            &[PriorityProgram {
                case: b"case",
                order: &candidate_order,
            }],
            &view,
        )
        .unwrap();
        let owner = execute(&frozen, &view).unwrap();
        assert_eq!(
            owner.historical_report().state,
            ComponentState::Unknown(MeasurementGap::MissingObservation)
        );
        assert_eq!(
            owner.candidate_states(),
            &[CandidateState::Pending(if event_cut {
                PendingReason::EventBudget
            } else {
                PendingReason::ReplayBudget
            })]
        );
        assert!(owner.candidate_observation(0).is_none());
    }
}
#[test]
fn tlcx22_design_raw_numeric_unit_payload_role_context_and_invalid_job_cuts() {
    let mut f = Fixture::gold();
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let bad = lc631_core::measurement_numeric::Rational::new_raw(1.into(), 0.into());
    let workload = f.workload(b"case");
    assert!(matches!(
        register(
            binding(),
            &[Workload {
                weight: &bad,
                ..workload
            }],
            limits(),
            &view
        ),
        Err(Error::Numeric(_))
    ));
    let mut wrong = workload;
    wrong.accepted.unit = TickUnit(d(200));
    assert!(matches!(
        register(binding(), &[wrong], limits(), &view),
        Err(Error::UnitMismatch)
    ));
    let mut role = binding();
    role.role = DesignRole::FinalResearchHoldout;
    assert!(matches!(
        register(role, &[workload], limits(), &view),
        Err(Error::UnsupportedRole)
    ));
    let oversized = vec![b'x'; 65537];
    assert!(matches!(
        register(
            binding(),
            &[Workload {
                case: &oversized,
                ..workload
            }],
            limits(),
            &view
        ),
        Err(Error::InvalidWorkload)
    ));
    let suite = register(binding(), &[workload], limits(), &view).unwrap();
    let mut cb = candidate(1);
    cb.context = cb.context.reconstructed([200; 32]);
    let order = improved();
    assert!(matches!(
        bind_candidate(
            &suite,
            cb,
            &[PriorityProgram {
                case: b"case",
                order: &order
            }],
            &view
        ),
        Err(Error::BindingMismatch)
    ));
    f.jobs[0].duration_ticks = 0;
    assert!(matches!(
        register(binding(), &[f.workload(b"case")], limits(), &view),
        Err(Error::InvalidWorkload)
    ));
}
