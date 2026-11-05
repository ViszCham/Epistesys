//! 独立した同時制約列挙と負例。実行結果は中央検証後に記録する。
//! Independent simultaneous-constraint enumeration and negatives; execution is recorded centrally.
use lc631_solvers::causal_spatial::*;
use lc631_substrate::context_identity::{ContextRevision, SourceRevision};

fn assignment(mask: u8, values: u8) -> Assignment {
    Assignment::new(mask, values).unwrap()
}

fn query(observed: Assignment, intervention: Assignment, target: u8) -> Query {
    Query {
        observed,
        intervention,
        target: Variable(target),
        value: true,
        work_limit: 4096,
    }
}

fn computed(value: Evaluation<CausalResult>) -> CausalResult {
    match value {
        Evaluation::Computed(value) => value,
        other => panic!("{other:?}"),
    }
}

#[test]
fn tlcx16_rejects_cycles_bad_tables_assignments_worlds_and_frames() {
    let cyclic = [Equation::Table {
        parents: 1,
        values: [2, 0, 0, 0],
    }];
    assert!(matches!(
        Scm::new(&cyclic, 0, &[0], Assignment::empty()),
        Err(Error::Cycle)
    ));
    let extra = [Equation::Table {
        parents: 0,
        values: [2, 0, 0, 0],
    }];
    assert!(matches!(
        Scm::new(&extra, 0, &[0], Assignment::empty()),
        Err(Error::InvalidTable)
    ));
    assert_eq!(Assignment::new(1, 2), Err(Error::InvalidAssignment));
    let root = [Equation::Exogenous(0)];
    assert!(matches!(
        Scm::new(&root, 1, &[0, 0], Assignment::empty()),
        Err(Error::DuplicateWorld)
    ));
    assert!(matches!(
        Scm::new(&root, 1, &[2], Assignment::empty()),
        Err(Error::InvalidDomain)
    ));
    let a = Rectangle::new(FrameId([1; 32]), [0, 0], [2, 2]).unwrap();
    let b = Rectangle::new(FrameId([2; 32]), [0, 0], [2, 2]).unwrap();
    assert_eq!(
        spatial(Some(a), Some(b), SpatialRelation::Connected),
        Err(Error::FrameMismatch)
    );
    assert_eq!(
        Rectangle::new(FrameId([1; 32]), [0, 0], [0, 2]),
        Err(Error::InvalidRectangle)
    );
}

#[test]
fn tlcx16_correlation_does_not_become_intervention() {
    // U -> X and U -> Y, without an X -> Y edge.
    let equations = [
        Equation::Exogenous(0),
        Equation::Table {
            parents: 1,
            values: [2, 0, 0, 0],
        },
        Equation::Table {
            parents: 1,
            values: [2, 0, 0, 0],
        },
    ];
    let model = Scm::new(&equations, 1, &[0, 1], Assignment::empty()).unwrap();
    let conditioned = computed(
        model
            .query(query(assignment(2, 2), Assignment::empty(), 2))
            .unwrap(),
    );
    assert_eq!((conditioned.true_worlds, conditioned.false_worlds), (1, 0));
    let intervened = computed(
        model
            .query(query(Assignment::empty(), assignment(2, 2), 2))
            .unwrap(),
    );
    assert_eq!((intervened.true_worlds, intervened.false_worlds), (1, 1));
    let counterfactual = computed(
        model
            .query(query(assignment(2, 0), assignment(2, 2), 2))
            .unwrap(),
    );
    assert_eq!(
        (counterfactual.true_worlds, counterfactual.false_worlds),
        (0, 1)
    );
    let witness = counterfactual.false_witness.unwrap();
    assert_eq!(
        (witness.exogenous, witness.factual, witness.action),
        (0, 0, 2)
    );
}

#[test]
fn tlcx16_missing_information_budget_and_hard_constraints_are_visible() {
    let missing = [Equation::Missing];
    let model = Scm::new(&missing, 0, &[0], Assignment::empty()).unwrap();
    assert_eq!(
        model
            .query(query(Assignment::empty(), Assignment::empty(), 0))
            .unwrap(),
        Evaluation::Unknown(UnknownReason::MissingEquation(Variable(0)))
    );
    // A do-value does not retroactively invent the missing factual equation.
    assert_eq!(
        model
            .query(query(Assignment::empty(), assignment(1, 1), 0))
            .unwrap(),
        Evaluation::Unknown(UnknownReason::MissingEquation(Variable(0)))
    );
    let equations = [
        Equation::Exogenous(0),
        Equation::Table {
            parents: 1,
            values: [2, 0, 0, 0],
        },
    ];
    let hard = Scm::new(&equations, 1, &[0], assignment(2, 0)).unwrap();
    assert_eq!(
        hard.query(query(Assignment::empty(), assignment(1, 1), 1)),
        Err(Error::HardConstraintViolation)
    );
    assert_eq!(
        hard.query(query(Assignment::empty(), assignment(2, 2), 1)),
        Err(Error::HardConstraintViolation)
    );
    assert_eq!(
        hard.query(query(assignment(1, 1), Assignment::empty(), 1))
            .unwrap(),
        Evaluation::Unknown(UnknownReason::NoCompatibleWorld)
    );
    let absent = Scm::new(&equations, 1, &[], Assignment::empty()).unwrap();
    assert_eq!(
        absent
            .query(query(Assignment::empty(), Assignment::empty(), 1))
            .unwrap(),
        Evaluation::Unknown(UnknownReason::MissingWorlds)
    );
    let mut bounded = query(Assignment::empty(), Assignment::empty(), 1);
    bounded.work_limit = 0;
    let context = ContextRevision::original(SourceRevision::of("SCM fixture"));
    let report = hard.evaluate(context, bounded).unwrap();
    assert_eq!(report.binding().context(), context);
    assert_eq!(
        report.binding().source(),
        *context.source().digest().as_bytes()
    );
    assert_eq!(report.profile(), Profile::BooleanAcyclicScmV1);
    assert_eq!(report.termination(), Termination::ResourceExhausted);
    assert_eq!(
        report.outcome(),
        Evaluation::Unknown(UnknownReason::WorkBudgetExhausted)
    );
    assert_eq!(
        spatial(None, None, SpatialRelation::Contains).unwrap(),
        Evaluation::Unknown(UnknownReason::MissingSpatialObject)
    );
}

// Independent model oracle: enumerate simultaneous total endogenous assignments.
// This does not call production state(), pack_parents(), or topological_order().
fn oracle_state(world: u8, table: u8, action_mask: u8, action_value: u8) -> u8 {
    let mut solutions = Vec::new();
    for candidate in 0u8..8 {
        let actual = [candidate & 1 != 0, candidate & 2 != 0, candidate & 4 != 0];
        let structural = [
            world & 1 != 0,
            world & 2 != 0,
            table & (1 << (candidate & 3)) != 0,
        ];
        let mut consistent = true;
        for index in 0..3 {
            let expected = if action_mask & (1 << index) != 0 {
                action_value & (1 << index) != 0
            } else {
                structural[index]
            };
            consistent &= actual[index] == expected;
        }
        if consistent {
            solutions.push(candidate);
        }
    }
    assert_eq!(solutions.len(), 1);
    solutions[0]
}

#[test]
fn tlcx16_all_two_parent_truth_tables_match_simultaneous_model_oracle() {
    let mut cases = 0;
    for table in 0u8..16 {
        let equations = [
            Equation::Exogenous(0),
            Equation::Exogenous(1),
            Equation::Table {
                parents: 3,
                values: [u64::from(table), 0, 0, 0],
            },
        ];
        let model = Scm::new(&equations, 2, &[0, 1, 2, 3], Assignment::empty()).unwrap();
        for observed_mask in 0u8..8 {
            for observed_value in 0u8..8 {
                if observed_value & !observed_mask != 0 {
                    continue;
                }
                for action_mask in 0u8..8 {
                    for action_value in 0u8..8 {
                        if action_value & !action_mask != 0 {
                            continue;
                        }
                        let mut expected = (0, 0);
                        for world in 0..4 {
                            let factual = oracle_state(world, table, 0, 0);
                            if factual & observed_mask != observed_value {
                                continue;
                            }
                            let action = oracle_state(world, table, action_mask, action_value);
                            if action & 4 != 0 {
                                expected.0 += 1;
                            } else {
                                expected.1 += 1;
                            }
                        }
                        let outcome = model
                            .query(query(
                                assignment(observed_mask, observed_value),
                                assignment(action_mask, action_value),
                                2,
                            ))
                            .unwrap();
                        if expected == (0, 0) {
                            assert_eq!(
                                outcome,
                                Evaluation::Unknown(UnknownReason::NoCompatibleWorld)
                            );
                        } else {
                            let result = computed(outcome);
                            assert_eq!((result.true_worlds, result.false_worlds), expected);
                        }
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 11_664);
}

#[test]
fn tlcx16_forward_references_are_topologically_evaluated() {
    let equations = [
        Equation::Table {
            parents: 2,
            values: [2, 0, 0, 0],
        },
        Equation::Exogenous(0),
    ];
    let model = Scm::new(&equations, 1, &[1], Assignment::empty()).unwrap();
    assert_eq!(
        computed(
            model
                .query(query(Assignment::empty(), Assignment::empty(), 0))
                .unwrap()
        )
        .true_worlds,
        1
    );
}

#[test]
fn tlcx16_report_pins_context_and_exact_typed_payloads() {
    let context = ContextRevision::original(SourceRevision::of("binding fixture"));
    let equations = [Equation::Exogenous(0)];
    let first = Scm::new(&equations, 1, &[0, 1], Assignment::empty()).unwrap();
    let second = Scm::new(&equations, 1, &[0], Assignment::empty()).unwrap();
    let query = query(Assignment::empty(), Assignment::empty(), 0);
    let a = first.evaluate(context, query).unwrap();
    let b = second.evaluate(context, query).unwrap();
    assert_ne!(a.binding().model(), b.binding().model());
    assert_eq!(a.binding().query(), b.binding().query());
    let revised = context.reconstructed([9; 32]);
    let c = first.evaluate(revised, query).unwrap();
    assert_eq!(a.binding().source(), c.binding().source());
    assert_ne!(a.binding().context(), c.binding().context());
    let changed = Query {
        value: false,
        ..query
    };
    assert_ne!(
        a.binding().query(),
        first.evaluate(context, changed).unwrap().binding().query()
    );
    let rectangle = Rectangle::new(FrameId([1; 32]), [0, 0], [1, 1]).unwrap();
    let spatial_a =
        evaluate_spatial(context, Some(rectangle), None, SpatialRelation::Contains).unwrap();
    let spatial_b =
        evaluate_spatial(context, Some(rectangle), None, SpatialRelation::Connected).unwrap();
    assert_eq!(spatial_a.binding().model(), spatial_b.binding().model());
    assert_ne!(spatial_a.binding().query(), spatial_b.binding().query());
}

#[test]
fn tlcx16_spatial_closed_set_relations_and_extreme_coordinates() {
    let frame = FrameId([5; 32]);
    let a = Rectangle::new(frame, [0, 0], [2, 2]).unwrap();
    let inside = Rectangle::new(frame, [0, 0], [1, 1]).unwrap();
    let touch = Rectangle::new(frame, [2, 2], [3, 3]).unwrap();
    let far = Rectangle::new(frame, [3, 0], [4, 1]).unwrap();
    assert_eq!(
        spatial(Some(a), Some(inside), SpatialRelation::Contains).unwrap(),
        Evaluation::Computed(true)
    );
    assert_eq!(
        spatial(Some(a), Some(inside), SpatialRelation::Touches).unwrap(),
        Evaluation::Computed(false)
    );
    assert_eq!(
        spatial(Some(a), Some(touch), SpatialRelation::Connected).unwrap(),
        Evaluation::Computed(true)
    );
    assert_eq!(
        spatial(Some(a), Some(touch), SpatialRelation::Touches).unwrap(),
        Evaluation::Computed(true)
    );
    assert_eq!(
        spatial(Some(a), Some(far), SpatialRelation::Connected).unwrap(),
        Evaluation::Computed(false)
    );
    assert_eq!(
        spatial(Some(a), Some(far), SpatialRelation::StrictlyLeftOf).unwrap(),
        Evaluation::Computed(true)
    );
    let extreme = Rectangle::new(frame, [i64::MIN, i64::MIN], [i64::MAX, i64::MAX]).unwrap();
    assert_eq!(
        spatial(Some(extreme), Some(a), SpatialRelation::Contains).unwrap(),
        Evaluation::Computed(true)
    );
}

#[test]
fn tlcx16_spatial_oracle_enumerates_integer_points_in_small_boxes() {
    let frame = FrameId([0; 32]);
    let mut checked = 0;
    for ax in -1..2 {
        for ay in -1..2 {
            for bx in -1..2 {
                for by in -1..2 {
                    let a = Rectangle::new(frame, [ax, ay], [ax + 1, ay + 1]).unwrap();
                    let b = Rectangle::new(frame, [bx, by], [bx + 1, by + 1]).unwrap();
                    let a_points = [(ax, ay), (ax + 1, ay), (ax, ay + 1), (ax + 1, ay + 1)];
                    let b_points = [(bx, by), (bx + 1, by), (bx, by + 1), (bx + 1, by + 1)];
                    let connected = a_points.iter().any(|point| b_points.contains(point));
                    let contains = b_points.iter().all(|point| a_points.contains(point));
                    assert_eq!(
                        spatial(Some(a), Some(b), SpatialRelation::Connected).unwrap(),
                        Evaluation::Computed(connected)
                    );
                    assert_eq!(
                        spatial(Some(a), Some(b), SpatialRelation::Contains).unwrap(),
                        Evaluation::Computed(contains)
                    );
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 81);
}
