//! P1 TEST-FIRST/API only. The framework child is intentionally not implemented.
//! Missing construction is RED, never a compile_fail success or semantic result.
use lc631_runtime::context_novelty_measurement::framework as f;
mod tlcx22_framework_p1_support;
use tlcx22_framework_p1_support::*;

fn node_count(e: &BoolExpr) -> usize {
    match e {
        BoolExpr::Constant(_) | BoolExpr::Variable(_) => 1,
        BoolExpr::Not(child) => 1 + node_count(child),
        BoolExpr::And(children) | BoolExpr::Or(children) => {
            1 + children.iter().map(node_count).sum::<usize>()
        }
        _ => panic!("test oracle only admits the preregistered Boolean grammar"),
    }
}

#[test]
fn tlcx22_p1_n01_registry_partition_and_semantics_cannot_be_labels() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let mut bad = h.declaration();
        bad.profiles = &[];
        assert!(matches!(
            f::FrameworkRoot::preregister(h.native(view), &bad, view),
            Err(f::Error::RegistryIncomplete)
        ));
        let mut bad = h.declaration();
        bad.partition = &ROLES[..19];
        assert!(matches!(
            f::FrameworkRoot::preregister(h.native(view), &bad, view),
            Err(f::Error::InvalidPartition)
        ));
        let mut duplicate = ROLES;
        duplicate[19] = duplicate[18];
        let mut bad = h.declaration();
        bad.partition = &duplicate;
        assert!(matches!(
            f::FrameworkRoot::preregister(h.native(view), &bad, view),
            Err(f::Error::InvalidPartition)
        ));
        let mut wrong = PROFILES;
        wrong[1].display_name = "classical";
        wrong[1].contract = f::Contract::ClassicalPointV1;
        let mut bad = h.declaration();
        bad.profiles = &wrong;
        assert!(matches!(
            f::FrameworkRoot::preregister(h.native(view), &bad, view),
            Err(f::Error::ProfileContractMismatch)
        ));
    });
}

#[test]
fn tlcx22_p1_n02_total_atom_operator_and_world_maps_are_not_complete_flags() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let mut bad = h.declaration();
        bad.map.atoms = &h.framework_atoms[..1];
        assert!(matches!(
            f::FrameworkRoot::preregister(h.native(view), &bad, view),
            Err(f::Error::InvalidMap)
        ));
        let mut bad = h.declaration();
        bad.map.operators = &OPERATORS[..4];
        assert!(matches!(
            f::FrameworkRoot::preregister(h.native(view), &bad, view),
            Err(f::Error::InvalidMap)
        ));
        let mut wrong = OPERATORS;
        wrong[4].target = f::NativeOperator::And;
        let mut bad = h.declaration();
        bad.map.operators = &wrong;
        assert!(matches!(
            f::FrameworkRoot::preregister(h.native(view), &bad, view),
            Err(f::Error::InvalidMap)
        ));
        let mut bad = h.declaration();
        bad.map.chain_root = 1;
        bad.map.chain_future = 0;
        assert!(matches!(
            f::FrameworkRoot::preregister(h.native(view), &bad, view),
            Err(f::Error::InvalidMap)
        ));
    });
}

#[test]
fn tlcx22_p1_n03_preregistration_does_not_reconstruct_root_goal_or_authority() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let mut bad = h.declaration();
        bad.root_goal_revision = d(99);
        assert!(matches!(
            f::FrameworkRoot::preregister(h.native(view), &bad, view),
            Err(f::Error::RootScopeMismatch)
        ));
        let mut bad = h.declaration();
        bad.authority_revision = AuthorityRevision(8);
        assert!(matches!(
            f::FrameworkRoot::preregister(h.native(view), &bad, view),
            Err(f::Error::RootScopeMismatch)
        ));
        let mut native = h.native(view);
        let actual = native.generate_current(operator(), view).unwrap();
        assert_eq!(actual.operator(), operator());
        assert!(matches!(
            f::FrameworkRoot::preregister(native, &h.declaration(), view),
            Err(f::Error::AlreadyGenerated)
        ));
    });
}

#[test]
fn tlcx22_p1_p01_four_cell_controls_use_actual_generated_ast_and_both_kernels() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let declaration = h.declaration();
        let mut root = f::FrameworkRoot::preregister(h.native(view), &declaration, view).unwrap();
        let generated = root.generate_current(operator(), view).unwrap();
        for (index, cell) in CELLS.iter().copied().enumerate() {
            let subject = match cell.subject {
                f::SubjectKind::Original => f::Subject::Original,
                f::SubjectKind::Generated => f::Subject::Generated(&generated),
            };
            let expected = root
                .expected_inputs_current(subject, cell.kernel, view)
                .unwrap();
            let owner = root.execute_case_current(expected, view).unwrap();
            owner
                .with_current(expected, view, |current| {
                    let classical = cell.kernel == f::Kernel::ClassicalPointV1;
                    let count = if classical { 4 } else { 9 };
                    assert_eq!(current.rows().len(), count);
                    let admitted: Vec<_> = current
                        .rows()
                        .iter()
                        .enumerate()
                        .filter_map(|(i, row)| row.admitted().then_some(i))
                        .collect();
                    assert_eq!(admitted.as_slice(), ADMITTED[index]);
                    assert!(std::ptr::eq(
                        current.subject_query(),
                        expected.subject_query
                    ));
                    assert_eq!(current.lowering_source_roots().len(), 7);
                    assert!(std::ptr::eq(
                        current.lowering_source_roots()[0],
                        &expected.subject_query.assertions()[0]
                    ));
                    assert!(std::ptr::eq(
                        current.lowering_source_roots()[1],
                        &expected.subject_query.assertions()[1]
                    ));
                    let nodes: usize = current
                        .lowering_source_roots()
                        .iter()
                        .map(|e| node_count(e))
                        .sum();
                    assert_eq!(current.counts().lowered_source_nodes, nodes);
                    assert_eq!(current.counts().unmapped_source_nodes, 0);
                    assert_eq!(current.counts().native_dispatches, count * 7);
                    assert_eq!(
                        current.counts().native_store_entries,
                        if classical { 0 } else { count * 7 }
                    );
                    assert_eq!(current.signature_roles().len(), 10);
                    assert_eq!(
                        current.capability_model_counts(),
                        if classical {
                            &[1, 2, 4, 8][..]
                        } else {
                            &[1, 3, 9, 27][..]
                        }
                    );
                    assert_eq!(
                        current.signature_packages(),
                        if classical {
                            &[f::Package::C, f::Package::Kc][..]
                        } else {
                            &[f::Package::C, f::Package::Ki][..]
                        }
                    );
                    assert_eq!(current.basis(), InputBasis::Given);
                    assert_eq!(current.validation(), SemanticValidation::Unassessed);
                    assert_eq!(current.independence(), IndependenceStatus::SharedDependency);
                    for row in current.rows() {
                        let q_state = usize::from(row.valuation_states()[1]);
                        assert_eq!(
                            row.query_bits()[2],
                            if classical {
                                CLASSICAL_LEM[q_state]
                            } else {
                                CHAIN_LEM[q_state]
                            }
                        );
                        if row.admitted() {
                            assert_eq!(row.goal_bits(), if classical { 1 } else { 3 });
                        }
                    }
                    for metric in [
                        ComponentMetric::FrameworkDistance,
                        ComponentMetric::Knownness,
                        ComponentMetric::SemanticConfigurationDistance,
                        ComponentMetric::Compatibility,
                        ComponentMetric::NoveltyCompatibility,
                    ] {
                        assert!(matches!(
                            current.metric_state(metric),
                            f::MetricState::PendingLaterPacket
                        ));
                    }
                })
                .unwrap();
        }
        assert_eq!(
            declaration.comparison.attribution,
            f::Attribution::JointContextAndFramework
        );
    });
}

#[test]
fn tlcx22_p1_p02_all_two_and_three_atom_legal_models_not_a_selected_witness() {
    for atoms in [2, 3] {
        framework_fixture!(h, atoms, true, Duration::from_secs(60), {
            let (h, view, _, _) = h;
            let declaration = h.declaration();
            let root = f::FrameworkRoot::preregister(h.native(view), &declaration, view).unwrap();
            for (kernel, radix, rows) in [
                (
                    f::Kernel::ClassicalPointV1,
                    2u8,
                    if atoms == 2 { 4 } else { 8 },
                ),
                (
                    f::Kernel::PersistentChain2V1,
                    3u8,
                    if atoms == 2 { 9 } else { 27 },
                ),
            ] {
                let expected = root
                    .expected_inputs_current(f::Subject::Original, kernel, view)
                    .unwrap();
                let owner = root.execute_case_current(expected, view).unwrap();
                owner
                    .with_current(expected, view, |current| {
                        assert_eq!(current.rows().len(), rows);
                        for (i, row) in current.rows().iter().enumerate() {
                            let mut number = i;
                            let mut literal = vec![0; atoms];
                            for slot in literal.iter_mut().rev() {
                                *slot = (number % usize::from(radix)) as u8;
                                number /= usize::from(radix);
                            }
                            assert_eq!(row.valuation_states(), literal.as_slice());
                        }
                    })
                    .unwrap();
            }
        });
    }
}

#[test]
fn tlcx22_p1_p03_mapback_future_01_loss_is_explicit_not_an_isomorphism() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let declaration = h.declaration();
        let mut root = f::FrameworkRoot::preregister(h.native(view), &declaration, view).unwrap();
        let generated = root.generate_current(operator(), view).unwrap();
        let expected = root
            .expected_inputs_current(
                f::Subject::Generated(&generated),
                f::Kernel::PersistentChain2V1,
                view,
            )
            .unwrap();
        let owner = root.execute_case_current(expected, view).unwrap();
        owner
            .with_current(expected, view, |current| {
                let row = &current.rows()[7];
                assert_eq!(row.valuation_states(), &[2, 1]);
                assert!(row.admitted());
                assert_eq!(row.query_bits(), &[3, 2, 2]);
                let back = &current.mapbacks()[7];
                assert_eq!(back.root_assignment(), &[true, false]);
                assert_eq!(back.forward_world_bits(), &[3, 0]);
                assert_eq!(back.lost_future_atoms(), &[1]);
            })
            .unwrap();
    });
}

#[test]
fn tlcx22_p1_n04_required_lem_is_a_hard_counterexample_not_novelty_credit() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let mut probes = h.probes.to_vec();
        probes[2].duty = f::Duty::Required;
        let mut declaration = h.declaration();
        declaration.probes = &probes;
        declaration.definition_revision = d(82); // A stricter preregistered policy, not a changed live oracle.
        let mut root = f::FrameworkRoot::preregister(h.native(view), &declaration, view).unwrap();
        let generated = root.generate_current(operator(), view).unwrap();
        let expected = root
            .expected_inputs_current(
                f::Subject::Generated(&generated),
                f::Kernel::PersistentChain2V1,
                view,
            )
            .unwrap();
        let attempted = root.execute_case_current(expected, view).unwrap();
        let calls = Cell::new(0);
        assert!(matches!(
            attempted.with_current(expected, view, |_| calls.set(calls.get() + 1)),
            Err(f::Error::HardInvariant)
        ));
        assert_eq!(calls.get(), 0);
    });
}

#[test]
fn tlcx22_p1_n07_known_native_budget_is_prepaid_and_failed_attempts_do_not_reset() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let mut declaration = h.declaration();
        declaration.limits.max_native_dispatches_per_case = 1;
        let root = f::FrameworkRoot::preregister(h.native(view), &declaration, view).unwrap();
        let expected = root
            .expected_inputs_current(f::Subject::Original, f::Kernel::PersistentChain2V1, view)
            .unwrap();
        let before = root.work_remaining_current(view).unwrap();
        for _ in 0..4 {
            assert!(matches!(
                root.execute_case_current(expected, view),
                Err(f::Error::NativeBudget)
            ));
        }
        assert!(matches!(
            root.execute_case_current(expected, view),
            Err(f::Error::CaseBudget)
        ));
        assert!(root.work_remaining_current(view).unwrap() < before);
    });
}

#[test]
fn tlcx22_p1_n08_four_atoms_and_wrong_given_partition_do_not_widen_the_profile() {
    framework_fixture!(h, 4, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        assert!(matches!(
            f::FrameworkRoot::preregister(h.native(view), &h.declaration(), view),
            Err(f::Error::UnsupportedProfile)
        ));
    });
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let mut bad = h.declaration();
        bad.universe = &h.universe[..2];
        assert!(matches!(
            f::FrameworkRoot::preregister(h.native(view), &bad, view),
            Err(f::Error::InvalidPartition)
        ));
    });
}

#[test]
fn tlcx22_p1_n11_known_model_and_claim_caps_reject_before_execution() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let mut bad = h.declaration();
        bad.limits.max_models = 8;
        assert!(matches!(
            f::FrameworkRoot::preregister(h.native(view), &bad, view),
            Err(f::Error::ModelBudget)
        ));
        let mut bad = h.declaration();
        bad.limits.plan_ram = 1;
        assert!(matches!(
            f::FrameworkRoot::preregister(h.native(view), &bad, view),
            Err(f::Error::RamBudget)
        ));
        let mut bad = h.declaration();
        bad.limits.max_lowered_nodes = 1;
        assert!(matches!(
            f::FrameworkRoot::preregister(h.native(view), &bad, view),
            Err(f::Error::LoweringBudget)
        ));
    });
}
