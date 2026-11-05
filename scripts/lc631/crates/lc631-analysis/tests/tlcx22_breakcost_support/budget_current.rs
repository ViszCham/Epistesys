//! Resource/current-consumption cases N08--N12; no producer implementation.
use super::*;
#[test]
fn n08_one_soft_check_retains_fixed_denominator_partial_not_measured() {
    let source = source();
    let raw = probe();
    let one = q(1, 1);
    let cases = [case(1, b"legacy-case", &raw, &one)];
    let rules = rules();
    let (_issuer, live) = lease(Cancellation::default());
    let view = live.view();
    let mut cap = budget();
    cap.max_soft_checks_per_run = 1;
    let root = register(
        &source,
        binding(),
        &cases,
        &rules,
        AdapterKind::LegacyProjectionV1,
        cap,
        &view,
    )
    .unwrap();
    let base = observe_baseline_current(&root, &view).unwrap();
    let req = requests(AdapterKind::BoundedProjectionV2);
    let frozen = bind_candidate(&root, candidate(1), &req, &view).unwrap();
    let owner = observe_breakcost_current(&root, &base, &frozen, &view).unwrap();
    assert_eq!(owner.historical_denominator(), &q(4, 1));
    assert_eq!(
        owner.historical_report().state,
        ComponentState::ResourceExhausted {
            partial: Some(RationalInterval::new(q(0, 1), q(3, 4)).unwrap())
        }
    );
    assert_eq!(owner.historical_counters().soft_checks, 1);
    assert_eq!(
        owner.historical_property(0, LegacyProperty::V1StrictConsumerAcceptance),
        Some(PropertyOutcome::NotObserved(
            NotObservedReason::SoftCheckBudget
        ))
    );
}
#[test]
fn n09_cancel_expiry_and_same_numeric_run_foreign_lease_cannot_revive_history() {
    fixture!(f, AdapterKind::LegacyProjectionV1, {
        let (source, _, cases, root, base, frozen, view) = f;
        let owner = observe_breakcost_current(root, base, frozen, view).unwrap();
        let (_i, foreign) = lease(Cancellation::default());
        let foreign = foreign.view();
        assert_eq!(view.run_id(), foreign.run_id());
        assert!(matches!(
            owner.report_current(inputs(source, root, cases, base, frozen), &foreign),
            Err(Error::ForeignReservation)
        ));
    });
    for expire in [false, true] {
        let source = source();
        let raw = probe();
        let one = q(1, 1);
        let cases = [case(1, b"legacy-case", &raw, &one)];
        let rules = rules();
        let cancel = Cancellation::default();
        let deadline = Deadline::after(if expire {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(60)
        })
        .unwrap();
        let (_i, live) = lease_with(cancel.clone(), 32 * 1024 * 1024, 4 * 1024 * 1024, deadline);
        let view = live.view();
        let root = register(
            &source,
            binding(),
            &cases,
            &rules,
            AdapterKind::LegacyProjectionV1,
            budget(),
            &view,
        )
        .unwrap();
        let base = observe_baseline_current(&root, &view).unwrap();
        let req = requests(AdapterKind::LegacyProjectionV1);
        let frozen = bind_candidate(&root, candidate(1), &req, &view).unwrap();
        let owner = observe_breakcost_current(&root, &base, &frozen, &view).unwrap();
        if expire {
            if let Ok(remaining) = deadline.remaining() {
                std::thread::sleep(remaining + Duration::from_millis(1));
            }
            assert!(matches!(
                owner.report_current(inputs(&source, &root, &cases, &base, &frozen), &view),
                Err(Error::Resource(ResourceError::DeadlineExceeded))
            ));
        } else {
            cancel.cancel();
            assert!(matches!(
                owner.report_current(inputs(&source, &root, &cases, &base, &frozen), &view),
                Err(Error::Resource(ResourceError::Cancelled))
            ));
        }
        assert_cost(owner.historical_report(), 0, 1);
    }
    assert_eq!(
        Deadline::at(Instant::now() - Duration::from_millis(1)).check(),
        Err(ResourceError::DeadlineExceeded)
    );
}
#[test]
fn n10_count_input_output_ram_and_numeric_caps_are_typed_not_clipped() {
    fixture!(f, AdapterKind::LegacyProjectionV1, {
        let (source, _, cases, _, _, _, view) = f;
        let rules = rules();
        for cut in 0..6 {
            let mut cap = budget();
            let expected = match cut {
                0 => {
                    cap.max_input_bytes_per_case = 1;
                    Error::InputBudget
                }
                1 => {
                    cap.max_input_bytes_total = 1;
                    Error::InputBudget
                }
                2 => {
                    cap.max_candidate_text_bytes = 1;
                    Error::InputBudget
                }
                3 => {
                    cap.max_cases = 0;
                    Error::InvalidConfig
                }
                4 => {
                    cap.max_weight_bits = 0;
                    Error::InvalidConfig
                }
                5 => {
                    cap.max_intermediate_bits = 0;
                    Error::InvalidConfig
                }
                _ => unreachable!(),
            };
            assert_eq!(
                register(
                    source,
                    binding(),
                    cases,
                    &rules,
                    AdapterKind::LegacyProjectionV1,
                    cap,
                    view
                )
                .err(),
                Some(expected)
            );
        }
        let mut cap = budget();
        cap.max_output_bytes_per_case = 1;
        cap.max_output_bytes_per_run = 1;
        let root = register(
            source,
            binding(),
            cases,
            &rules,
            AdapterKind::LegacyProjectionV1,
            cap,
            view,
        )
        .unwrap();
        assert!(matches!(
            observe_baseline_current(&root, view),
            Err(Error::OutputBudget)
        ));
        for cut in 0..3 {
            let mut cap = budget();
            let expected = match cut {
                0 => {
                    cap.max_adapter_calls_per_run = 0;
                    Error::DispatchBudget
                }
                1 => {
                    cap.max_serializations_per_run = 0;
                    Error::DispatchBudget
                }
                2 => {
                    cap.max_property_checks_per_run = 0;
                    Error::PropertyBudget
                }
                _ => unreachable!(),
            };
            let root = register(
                source,
                binding(),
                cases,
                &rules,
                AdapterKind::LegacyProjectionV1,
                cap,
                view,
            )
            .unwrap();
            assert_eq!(observe_baseline_current(&root, view).err(), Some(expected));
        }
        let (_i, tiny) = lease_with(
            Cancellation::default(),
            1,
            1,
            Deadline::after(Duration::from_secs(60)).unwrap(),
        );
        assert!(matches!(
            register(
                source,
                binding(),
                cases,
                &rules,
                AdapterKind::LegacyProjectionV1,
                budget(),
                &tiny.view()
            ),
            Err(Error::Resource(ResourceError::Exhausted(_)))
        ));
    });
}
#[test]
fn n11_empty_zero_negative_huge_weights_and_duplicate_ids_are_not_zero() {
    fixture!(f, AdapterKind::LegacyProjectionV1, {
        let (source, raw, cases, _, _, _, view) = f;
        for bad in [
            vec![],
            vec![PropertyRule {
                property: LegacyProperty::JsonObjectEnvelope,
                class: PropertyClass::Optional,
                weight: q(0, 1),
            }],
            vec![PropertyRule {
                property: LegacyProperty::JsonObjectEnvelope,
                class: PropertyClass::Optional,
                weight: q(-1, 1),
            }],
        ] {
            assert!(matches!(
                register(
                    source,
                    binding(),
                    cases,
                    &bad,
                    AdapterKind::LegacyProjectionV1,
                    budget(),
                    view
                ),
                Err(Error::InvalidConfig)
            ));
        }
        let mut huge = rules();
        huge[0].weight = Rational::from_integer(q(2, 1).numer().pow(65));
        assert!(matches!(
            register(
                source,
                binding(),
                cases,
                &huge,
                AdapterKind::LegacyProjectionV1,
                budget(),
                view
            ),
            Err(Error::NumericBudget)
        ));
        let mut dup = rules();
        dup.push(dup[0].clone());
        assert!(matches!(
            register(
                source,
                binding(),
                cases,
                &dup,
                AdapterKind::LegacyProjectionV1,
                budget(),
                view
            ),
            Err(Error::InvalidSuite)
        ));
        let one = q(1, 1);
        let duplicates = [case(1, b"a", raw, &one), case(1, b"b", raw, &one)];
        assert!(matches!(
            register(
                source,
                binding(),
                &duplicates,
                &rules(),
                AdapterKind::LegacyProjectionV1,
                budget(),
                view
            ),
            Err(Error::InvalidSuite)
        ));
    });
}
#[test]
fn n12_equal_numeric_owner_does_not_replace_exact_candidate_or_baseline() {
    fixture!(f, AdapterKind::LegacyProjectionV1, {
        let (source, _, cases, root, base, frozen, view) = f;
        let owner = observe_breakcost_current(root, base, frozen, view).unwrap();
        let other_base = observe_baseline_current(root, view).unwrap();
        let req = requests(AdapterKind::LegacyProjectionV1);
        let duplicate = bind_candidate(root, candidate(1), &req, view).unwrap();
        let other = observe_breakcost_current(root, base, &duplicate, view).unwrap();
        assert_eq!(
            owner.historical_report().state,
            other.historical_report().state
        );
        for expected in [
            inputs(source, root, cases, &other_base, frozen),
            inputs(source, root, cases, base, &duplicate),
        ] {
            assert!(matches!(
                owner.report_current(expected, view),
                Err(Error::BindingMismatch)
            ));
        }
    });
}
