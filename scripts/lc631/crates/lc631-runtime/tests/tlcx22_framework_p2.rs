//! P2 construction packet only: novelty child absent is RED, not a behavior pass.
mod tlcx22_framework_p2_support;
use tlcx22_framework_p2_support::*;

#[test]
fn tlcx22_p2_n01_register_before_actual_generation_not_a_complete_flag() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        let mut framework = f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap();
        let actual = framework.generate_current(operator(), view).unwrap();
        assert_eq!(actual.operator(), operator());
        let r = registration(h.constructions, m::ArchiveInput::Unread);
        assert!(matches!(
            m::Root::preregister(framework, &r, view),
            Err(m::Error::AlreadyGenerated)
        ));
    });
}

#[test]
fn tlcx22_p2_n02_actual_canonical_duplicates_invalid_weights_and_not_are_not_features() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        let commute = BoolExpr::And(vec![var(q_id(&h)), var(p(&h))]);
        let one = q(1, 1);
        let zero = q(0, 1);
        let duplicate = [
            n::ConstructionFeature {
                formula: &h.theory.assertions()[0],
                weight: &one,
            },
            n::ConstructionFeature {
                formula: &commute,
                weight: &one,
            },
        ];
        let r = registration(&duplicate, m::ArchiveInput::Unread);
        assert!(matches!(
            m::Root::preregister(
                f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
                &r,
                view
            ),
            Err(m::Error::DuplicateFeature)
        ));
        let invalid = [n::ConstructionFeature {
            formula: &h.goal.assertions()[0],
            weight: &zero,
        }];
        let r = registration(&invalid, m::ArchiveInput::Unread);
        assert!(matches!(
            m::Root::preregister(
                f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
                &r,
                view
            ),
            Err(m::Error::InvalidWeight)
        ));
        let not = BoolExpr::Not(Box::new(var(p(&h))));
        let unsupported = [n::ConstructionFeature {
            formula: &not,
            weight: &one,
        }];
        let r = registration(&unsupported, m::ArchiveInput::Unread);
        assert!(matches!(
            m::Root::preregister(
                f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
                &r,
                view
            ),
            Err(m::Error::UnsupportedConstruction)
        ));
    });
}

#[test]
fn tlcx22_p2_n03_known_payload_and_numeric_caps_precede_clone_or_metric_work() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        let mut r = registration(h.constructions, m::ArchiveInput::Unread);
        r.limits.max_payload_bytes = 1;
        assert!(matches!(
            m::Root::preregister(
                f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
                &r,
                view
            ),
            Err(m::Error::PayloadBudget)
        ));
        let mut r = registration(h.constructions, m::ArchiveInput::Unread);
        r.limits.max_weight_bits = 2;
        assert!(matches!(
            m::Root::preregister(
                f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
                &r,
                view
            ),
            Err(m::Error::NumericBudget)
        ));
    });
}

#[test]
fn tlcx22_p2_p01_actual_signatures_context_control_and_symmetric_literal_union() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        let r = registration(h.constructions, m::ArchiveInput::Unread);
        let mut root = m::Root::preregister(
            f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
            &r,
            view,
        )
        .unwrap();
        let generated = root.generate_current(operator(), view).unwrap();
        p2_pair!(root, generated, view, c0, ce0, i1, ie1);
        let ce1 = root
            .expected_case_current(
                f::Subject::Generated(&generated),
                f::Kernel::ClassicalPointV1,
                view,
            )
            .unwrap();
        let c1 = root.execute_case_current(ce1, view).unwrap();
        for (reference, le, target, re, num, den, union) in [
            (&c0, ce0, &c1, ce1, 0, 1, 7),
            (&c0, ce0, &i1, ie1, 1, 4, 8),
            (&i1, ie1, &c0, ce0, 1, 4, 8),
        ] {
            let e = pair(&root, reference, le, target, re, view);
            let owner = root.measure_current(e, view).unwrap();
            let callbacks = Cell::new(0);
            owner
                .with_current(e, view, |v| {
                    callbacks.set(callbacks.get() + 1);
                    point(
                        v.report(ComponentMetric::FrameworkDistance).unwrap(),
                        num,
                        den,
                    );
                    assert_eq!(v.signature_union_weight(), &q(union, 1));
                    assert_eq!(v.signature_positive_rows(), 8);
                    assert_eq!(v.signature_role_counts(), &[[1u16; 10]; 2]);
                    assert!(v.signature_payload_lengths().iter().all(|n| *n > 0));
                    assert_eq!(v.signature_payload(f::Package::C), Some(COMMON_SIGNATURE));
                    assert_eq!(v.signature_payload(f::Package::Kc), Some(CLASSICAL_CLOSURE));
                    assert_eq!(
                        v.signature_payload(f::Package::Ki),
                        Some(PERSISTENT_CLOSURE)
                    );
                    assert_eq!(
                        v.report(ComponentMetric::Knownness).unwrap().state,
                        ComponentState::Unknown(MeasurementGap::MissingObservation)
                    );
                })
                .unwrap();
            assert_eq!(
                callbacks.get(),
                1,
                "actual current view must be consumed exactly once"
            );
        }
    });
}

#[test]
fn tlcx22_p2_p02_cross_knownness_all_entries_exact_match_and_revised_corpus() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        let base = baseline(&h);
        let exact = exact(&h);
        let entries = [
            archive(&h, &base, f::Kernel::ClassicalPointV1),
            archive(&h, &base, f::Kernel::PersistentChain2V1),
            archive(&h, &exact, f::Kernel::ClassicalPointV1),
        ];
        for len in [1, 3] {
            let mut r = registration(h.constructions, m::ArchiveInput::Read(&entries[..len]));
            r.corpus_revision = d(if len == 1 { 90 } else { 97 });
            let mut root = m::Root::preregister(
                f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
                &r,
                view,
            )
            .unwrap();
            let generated = root.generate_current(operator(), view).unwrap();
            p2_pair!(root, generated, view, left, le, right, re);
            let e = pair(&root, &left, le, &right, re, view);
            let owner = root.measure_current(e, view).unwrap();
            owner
                .with_current(e, view, |v| {
                    point(
                        v.report(ComponentMetric::Knownness).unwrap(),
                        1,
                        if len == 1 { 3 } else { 1 },
                    );
                    assert_eq!(v.archive_status(), m::ArchiveStatus::Complete);
                    assert_eq!(v.counts().archive_entries_seen, len);
                    assert_eq!(v.counts().archive_comparisons, len);
                    assert_eq!(
                        v.counts().native_archive_dispatches,
                        if len == 1 { 16 } else { 68 }
                    );
                    assert_eq!(
                        v.counts().native_archive_store_entries,
                        if len == 1 { 0 } else { 36 }
                    );
                    assert_eq!(v.archive_queries().len(), len);
                    assert!(std::ptr::eq(v.archive_queries()[0], &base));
                    assert!(std::ptr::eq(v.candidate_query(), re.subject_query));
                    assert_eq!(v.corpus_revision(), r.corpus_revision);
                    assert_eq!(
                        v.knownness_union_weights(),
                        if len == 1 {
                            vec![q(3, 1)]
                        } else {
                            vec![q(3, 1), q(3, 1), q(2, 1)]
                        }
                        .as_slice()
                    );
                })
                .unwrap();
        }
    });
}

#[test]
fn tlcx22_p2_n04_confirmed_empty_unread_and_unavailable_never_become_zero() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        for mode in [
            m::ArchiveInput::Read(&[]),
            m::ArchiveInput::Unread,
            m::ArchiveInput::Unavailable,
        ] {
            let r = registration(h.constructions, mode);
            let mut root = m::Root::preregister(
                f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
                &r,
                view,
            )
            .unwrap();
            let generated = root.generate_current(operator(), view).unwrap();
            p2_pair!(root, generated, view, left, le, right, re);
            let e = pair(&root, &left, le, &right, re, view);
            let owner = root.measure_current(e, view).unwrap();
            owner
                .with_current(e, view, |v| {
                    point(v.report(ComponentMetric::FrameworkDistance).unwrap(), 1, 4);
                    let (status, state) = match mode {
                        m::ArchiveInput::Read(_) => (
                            m::ArchiveStatus::Empty,
                            ComponentState::Undefined(MeasurementError::EmptyCorpus),
                        ),
                        m::ArchiveInput::Unread => (
                            m::ArchiveStatus::Unread,
                            ComponentState::Unknown(MeasurementGap::MissingObservation),
                        ),
                        m::ArchiveInput::Unavailable => (
                            m::ArchiveStatus::Unavailable,
                            ComponentState::Unknown(MeasurementGap::MissingObservation),
                        ),
                    };
                    assert_eq!(v.archive_status(), status);
                    assert_eq!(v.report(ComponentMetric::Knownness).unwrap().state, state);
                    assert_eq!(v.counts().native_archive_dispatches, 0);
                })
                .unwrap();
        }
    });
}

#[test]
fn tlcx22_p2_n05_maximum_one_does_not_skip_unsupported_late_entry() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        let exact = exact(&h);
        let not = query(&h, vec![var(p(&h)), BoolExpr::Not(Box::new(var(q_id(&h))))]);
        let entries = [
            archive(&h, &exact, f::Kernel::ClassicalPointV1),
            archive(&h, &not, f::Kernel::PersistentChain2V1),
        ];
        let r = registration(h.constructions, m::ArchiveInput::Read(&entries));
        let mut root = m::Root::preregister(
            f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
            &r,
            view,
        )
        .unwrap();
        let generated = root.generate_current(operator(), view).unwrap();
        p2_pair!(root, generated, view, left, le, right, re);
        let e = pair(&root, &left, le, &right, re, view);
        let owner = root.measure_current(e, view).unwrap();
        owner
            .with_current(e, view, |v| {
                assert_eq!(v.archive_status(), m::ArchiveStatus::Unsupported);
                assert_eq!(
                    v.report(ComponentMetric::Knownness).unwrap().state,
                    ComponentState::Unknown(MeasurementGap::Unsupported)
                );
                assert_eq!(v.counts().archive_entries_seen, 2);
            })
            .unwrap();
    });
}

#[test]
fn tlcx22_p2_n06_candidate_feature_outside_fixed_universe_is_not_dropped() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        let base = baseline(&h);
        let entries = [archive(&h, &base, f::Kernel::ClassicalPointV1)];
        let r = registration(&h.constructions[..2], m::ArchiveInput::Read(&entries));
        let mut root = m::Root::preregister(
            f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
            &r,
            view,
        )
        .unwrap();
        let generated = root.generate_current(operator(), view).unwrap();
        p2_pair!(root, generated, view, left, le, right, re);
        let e = pair(&root, &left, le, &right, re, view);
        assert!(matches!(
            root.measure_current(e, view),
            Err(m::Error::UniverseMismatch)
        ));
    });
}

#[test]
fn tlcx22_p2_n07_partial_max_one_remains_resource_exhausted_not_measured() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        let exact = exact(&h);
        let base = baseline(&h);
        let entries = [
            archive(&h, &exact, f::Kernel::ClassicalPointV1),
            archive(&h, &base, f::Kernel::PersistentChain2V1),
        ];
        for budget in [0, 1] {
            let mut r = registration(h.constructions, m::ArchiveInput::Read(&entries));
            r.limits.max_comparisons = budget;
            let mut root = m::Root::preregister(
                f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
                &r,
                view,
            )
            .unwrap();
            let generated = root.generate_current(operator(), view).unwrap();
            p2_pair!(root, generated, view, left, le, right, re);
            let e = pair(&root, &left, le, &right, re, view);
            let owner = root.measure_current(e, view).unwrap();
            owner
                .with_current(e, view, |v| {
                    assert_eq!(v.archive_status(), m::ArchiveStatus::Partial);
                    assert_eq!(
                        v.report(ComponentMetric::Knownness).unwrap().state,
                        ComponentState::ResourceExhausted {
                            partial: Some(
                                RationalInterval::from_ratio(
                                    if budget == 0 { 0 } else { 1 },
                                    1,
                                    1,
                                    1
                                )
                                .unwrap()
                            )
                        }
                    );
                    assert_eq!(v.counts().archive_comparisons, budget);
                })
                .unwrap();
        }
    });
}

#[test]
fn tlcx22_p2_n08_empty_construction_artifact_is_not_a_read_empty_archive() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        let empty = query(&h, vec![]);
        let entries = [archive(&h, &empty, f::Kernel::ClassicalPointV1)];
        let r = registration(h.constructions, m::ArchiveInput::Read(&entries));
        assert!(matches!(
            m::Root::preregister(
                f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
                &r,
                view
            ),
            Err(m::Error::EmptyConstruction)
        ));
    });
}

#[test]
fn tlcx22_p2_n09_actual_archive_without_root_goal_map_is_no_comparable() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        let unbridged = query(&h, vec![var(q_id(&h)), var(q_id(&h))]);
        let entries = [archive(&h, &unbridged, f::Kernel::PersistentChain2V1)];
        let r = registration(h.constructions, m::ArchiveInput::Read(&entries));
        let mut root = m::Root::preregister(
            f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
            &r,
            view,
        )
        .unwrap();
        let generated = root.generate_current(operator(), view).unwrap();
        p2_pair!(root, generated, view, left, le, right, re);
        let e = pair(&root, &left, le, &right, re, view);
        let owner = root.measure_current(e, view).unwrap();
        owner
            .with_current(e, view, |v| {
                assert_eq!(v.archive_status(), m::ArchiveStatus::NoComparable);
                assert!(matches!(
                    v.report(ComponentMetric::Knownness).unwrap().state,
                    ComponentState::Incomparable { .. }
                ));
            })
            .unwrap();
    });
}

#[test]
fn tlcx22_p2_n15_actual_required_lem_failed_owner_cannot_gain_measurement_eligibility() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let mut probes = h.probes.to_vec();
        probes[2].duty = f::Duty::Required;
        let mut fd = h.declaration();
        fd.probes = &probes;
        fd.definition_revision = d(82);
        let r = registration(h.constructions, m::ArchiveInput::Unread);
        let mut root = m::Root::preregister(
            f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
            &r,
            view,
        )
        .unwrap();
        let generated = root.generate_current(operator(), view).unwrap();
        p2_pair!(root, generated, view, left, le, right, re);
        assert!(matches!(
            root.expected_pair_current(
                m::CaseRef {
                    owner: &left,
                    expected: le
                },
                m::CaseRef {
                    owner: &right,
                    expected: re
                },
                view
            ),
            Err(m::Error::Framework(f::Error::HardInvariant))
        ));
    });
}
