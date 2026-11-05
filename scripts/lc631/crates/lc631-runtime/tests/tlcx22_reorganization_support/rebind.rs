//! Future producer calls only. Literal gold is never submitted as producer input.
use super::support::*;
use lc631_runtime::formal_boolean::{self as formal, Binding, Request, UnconstrainedPolicy};

fn check_literal_row(fx: &Fixture, actual: &BoolExpr, input: bool, expected: bool) {
    // Observe an internally composed AST with the EXISTING finite evaluator.
    // The literal expected value is a test assertion, never an owner constructor input.
    let condition = if input {
        fx.functions[1].clone()
    } else {
        BoolExpr::Not(Box::new(fx.functions[1].clone()))
    };
    let theory = SmtStatusQuery::checked(
        vec![fx.parameter],
        vec![],
        vec![condition],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let expected_ast = BoolExpr::Constant(expected);
    let symbols = [(fx.parameter, fx.parameter)];
    let mut request = Request {
        source: &theory,
        target: &theory,
        source_predicate: &expected_ast,
        target_predicate: actual,
        symbols: &symbols,
        unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
        binding: Binding {
            source: fx.context.source(),
            target: fx.context.source(),
            source_context: fx.context,
            target_context: fx.context,
            probe_set: d(30),
            probe_map: d(0),
        },
    };
    request.binding.probe_map = formal::probe_map_digest(&request, 4096).unwrap();
    let Evaluation::Computed(result) = formal::evaluate(&request, 4096).unwrap() else {
        panic!("literal input row not actually computed")
    };
    assert_eq!(result.soundness(), DirectionResult::Preserved);
    assert_eq!(result.completeness(), DirectionResult::Preserved);
    assert_eq!(result.assignments(), 2);
    assert!(std::ptr::eq(result.request(), &request));
}

#[test]
fn tlcx22_rebind_all_unary_function_map_pairs() {
    normal(|fx, view, _| {
        let baseline_bindings = fx.bindings([1, 1], false);
        let uses = fx.root_uses();
        let baseline = fx.model(&baseline_bindings, &uses);
        let probes = fx.probes([1, 3]);
        let plan =
            r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), limits(), view).unwrap();
        for role in 0..4 {
            for map in 0..4 {
                let bindings = fx.bindings([1, role], false);
                let candidate = fx.model(&bindings, &uses);
                let maps = fx.maps([1, map]);
                let program = r::PreparedRebindProgram::prepare(
                    &plan,
                    &candidate,
                    &maps,
                    CandidateId(10 + (role * 4 + map) as u64),
                    view,
                )
                .unwrap();
                let actual = program.composition(fx.units[1]).unwrap();
                for (input_index, input) in [false, true].into_iter().enumerate() {
                    check_literal_row(fx, actual, input, COMPOSED[role][map][input_index]);
                }
                let queries = r::RebindQueries::capture(&program, view).unwrap();
                match r::OwnedRebind::evaluate(&queries, view) {
                    Ok(owner) => {
                        assert!(ADMISSIBLE[role][map], "invalid pair accepted: {role}/{map}");
                        owner
                            .with_current(view, |current| {
                                assert_point(current.report(), if role == 2 { 3 } else { 0 }, 4);
                                assert_eq!(current.checks().len(), 6);
                                for check in current.checks() {
                                    let Evaluation::Computed(result) = check.evaluation() else {
                                        panic!("admitted pair lacks actual formal result")
                                    };
                                    assert_eq!(result.validation(),
                                FormalValidation::GivenMappingSourceInterpretationUnverified);
                                    assert!(std::ptr::eq(result.request(), check.request()));
                                    if check.kind() == r::CheckKind::RoleDifference {
                                        let changed = check.unit() == fx.units[1] && role == 2;
                                        assert_eq!(
                                            result.soundness() != DirectionResult::Preserved
                                                || result.completeness()
                                                    != DirectionResult::Preserved,
                                            changed
                                        );
                                    } else {
                                        assert_eq!(result.soundness(), DirectionResult::Preserved);
                                        assert_eq!(
                                            result.completeness(),
                                            DirectionResult::Preserved
                                        );
                                    }
                                    if check.kind() == r::CheckKind::MapBack {
                                        assert!(std::ptr::eq(
                                            check.request().target_predicate,
                                            program.composition(check.unit()).unwrap()
                                        ));
                                    }
                                    if check.kind() == r::CheckKind::RootUse {
                                        assert!(std::ptr::eq(
                                            check.request().target_predicate,
                                            program.root_use(check.unit()).unwrap()
                                        ));
                                    }
                                }
                            })
                            .unwrap();
                    }
                    Err(r::Error::PrerequisiteFailed {
                        unit,
                        check: r::CheckKind::MapBack,
                    }) => {
                        assert!(!ADMISSIBLE[role][map]);
                        assert_eq!(unit, fx.units[1]);
                    }
                    Err(other) => panic!("unexpected pair failure: {other:?}"),
                }
            }
        }
        assert_eq!(plan.denominator(), 4);
    });
}

#[test]
fn tlcx22_rebind_rename_zero_useful_change_positive() {
    normal(|fx, view, _| {
        let original = fx.bindings([1, 1], false);
        let uses = fx.root_uses();
        let baseline = fx.model(&original, &uses);
        let probes = fx.probes([1, 3]);
        let plan =
            r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), limits(), view).unwrap();
        // Same role IDs with changed executable function must still count.
        // Different IDs with equivalent function must never count.
        for (functions, renamed, map, expected) in [
            ([1, 1], true, [1, 1], 0),
            ([1, 4], true, [1, 1], 0),
            ([1, 2], false, [1, 2], 3),
        ] {
            let bindings = fx.bindings(functions, renamed);
            let candidate = fx.model(&bindings, &uses);
            let maps = fx.maps(map);
            let p =
                r::PreparedRebindProgram::prepare(&plan, &candidate, &maps, CandidateId(2), view)
                    .unwrap();
            let q = r::RebindQueries::capture(&p, view).unwrap();
            let owner = r::OwnedRebind::evaluate(&q, view).unwrap();
            owner
                .with_current(view, |c| assert_point(c.report(), expected, 4))
                .unwrap();
        }
    });
}

#[test]
fn tlcx22_rebind_multiple_or_dangling_binding_rejected() {
    normal(|fx, view, _| {
        let b = fx.bindings([1, 1], false);
        let uses = fx.root_uses();
        let duplicate = [b[0], b[1], b[1]];
        assert!(matches!(
            r::RoleModel::checked(&fx.source, fx.context, &fx.theory, &duplicate, &uses),
            Err(r::Error::DuplicateBinding)
        ));
        let baseline = fx.model(&b, &uses);
        let probes = fx.probes([1, 3]);
        let plan =
            r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), limits(), view).unwrap();
        let missing = fx.model(&b[..1], &uses[..1]);
        let maps = fx.maps([1, 1]);
        assert!(matches!(
            r::PreparedRebindProgram::prepare(&plan, &missing, &maps, CandidateId(2), view),
            Err(r::Error::UnitSetMismatch)
        ));
        let dangling = r::UseExpr::MappedUnit(r::UnitId(400));
        let bad_uses = [
            uses[0],
            r::RootUseInput {
                unit: fx.units[1],
                body: &dangling,
            },
        ];
        assert!(matches!(
            r::RoleModel::checked(&fx.source, fx.context, &fx.theory, &b, &bad_uses),
            Err(r::Error::DanglingUse)
        ));
        assert!(matches!(
            r::PreparedRebindProgram::prepare(&plan, &baseline, &maps[..1], CandidateId(2), view),
            Err(r::Error::MapSetMismatch)
        ));
    });
}

#[test]
fn tlcx22_rebind_wrong_program_mapback_and_goal_use_cut() {
    normal(|fx, view, _| {
        let b = fx.bindings([1, 1], false);
        let good = fx.bindings([1, 2], false);
        let uses = fx.root_uses();
        let baseline = fx.model(&b, &uses);
        let probes = fx.probes([1, 3]);
        let plan =
            r::RebindPlan::capture_fixed(&baseline, &probes, fx.pins(), limits(), view).unwrap();
        let candidate = fx.model(&good, &uses);
        for mapping in [[1, 1], [1, 0], [1, 3]] {
            let maps = fx.maps(mapping);
            let p =
                r::PreparedRebindProgram::prepare(&plan, &candidate, &maps, CandidateId(2), view)
                    .unwrap();
            let q = r::RebindQueries::capture(&p, view).unwrap();
            assert!(matches!(
                r::OwnedRebind::evaluate(&q, view),
                Err(r::Error::PrerequisiteFailed {
                    check: r::CheckKind::MapBack,
                    ..
                })
            ));
        }
        // An unrelated root-correct raw-input path is NOT use of this candidate.
        let bypass = r::UseExpr::RawInput(fx.parameter);
        let use_cut = [
            uses[0],
            r::RootUseInput {
                unit: fx.units[1],
                body: &bypass,
            },
        ];
        let unused = fx.model(&good, &use_cut);
        let maps = fx.maps([1, 2]);
        assert!(matches!(
            r::PreparedRebindProgram::prepare(&plan, &unused, &maps, CandidateId(2), view),
            Err(r::Error::MissingActualUse)
        ));
        let foreign_unit = r::UseExpr::MappedUnit(fx.units[0]);
        let wrong = [
            uses[0],
            r::RootUseInput {
                unit: fx.units[1],
                body: &foreign_unit,
            },
        ];
        let wrong_model = fx.model(&good, &wrong);
        assert!(matches!(
            r::PreparedRebindProgram::prepare(&plan, &wrong_model, &maps, CandidateId(2), view),
            Err(r::Error::UseUnitMismatch)
        ));
    });
}
