//! Actual-owner cuts and strict existing consumers, not P3 leaf production.
mod tlcx22_framework_p2_support;
use tlcx22_framework_p2_support::*;

#[test]
fn tlcx22_p2_n10_equal_digest_foreign_root_changed_registration_archive_and_ast_are_not_current() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, run) = h;
        let fd = h.declaration();
        let base = baseline(&h);
        let entries = [archive(&h, &base, f::Kernel::ClassicalPointV1)];
        let r = registration(h.constructions, m::ArchiveInput::Read(&entries));
        let mut root = m::Root::preregister(
            f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
            &r,
            view,
        )
        .unwrap();
        let other = m::Root::preregister(
            f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
            &r,
            view,
        )
        .unwrap();
        assert_eq!(root.historical_digest(), other.historical_digest());
        let generated = root.generate_current(operator(), view).unwrap();
        p2_pair!(root, generated, view, left, le, right, re);
        let e = pair(&root, &left, le, &right, re, view);
        let owner = root.measure_current(e, view).unwrap();
        let mut changed = e;
        changed.root = &other;
        assert!(matches!(
            owner.with_current(changed, view, |_| ()),
            Err(m::Error::ForeignRoot)
        ));
        let copy = registration(h.constructions, m::ArchiveInput::Read(&entries));
        let mut changed = e;
        changed.registration = &copy;
        assert!(matches!(
            owner.with_current(changed, view, |_| ()),
            Err(m::Error::ExpectedInputMismatch)
        ));
        let copied_entries = [archive(&h, &base, f::Kernel::ClassicalPointV1)];
        let mut changed = e;
        changed.archive = m::ArchiveInput::Read(&copied_entries);
        assert!(matches!(
            owner.with_current(changed, view, |_| ()),
            Err(m::Error::ExpectedInputMismatch)
        ));
        let cloned_query = re.subject_query.clone();
        let mut changed = e;
        changed.target.expected.subject_query = &cloned_query;
        assert!(matches!(
            owner.with_current(changed, view, |_| ()),
            Err(m::Error::ExpectedInputMismatch)
        ));
        let mut changed = e;
        changed.corpus_revision = d(97);
        assert!(matches!(
            owner.with_current(changed, view, |_| ()),
            Err(m::Error::ExpectedInputMismatch)
        ));
        let foreign = run
            .reserve(&[resource(ResourceKind::Ram, 1024 * 1024)])
            .unwrap();
        let foreign_view = foreign.lease();
        let calls = Cell::new(0);
        assert!(matches!(
            owner.with_current(e, &foreign_view, |_| calls.set(1)),
            Err(m::Error::ForeignReservation)
        ));
        assert_eq!(calls.get(), 0);
    });
}

#[test]
fn tlcx22_p2_n11_actual_foreign_case_and_same_bytes_foreign_source_cannot_supply_archive_ownership()
{
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
        let mut other = m::Root::preregister(
            f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
            &r,
            view,
        )
        .unwrap();
        let own = root.generate_current(operator(), view).unwrap();
        let foreign = other.generate_current(operator(), view).unwrap();
        let le = root
            .expected_case_current(
                f::Subject::Generated(&own),
                f::Kernel::ClassicalPointV1,
                view,
            )
            .unwrap();
        let left = root.execute_case_current(le, view).unwrap();
        let re = other
            .expected_case_current(
                f::Subject::Generated(&foreign),
                f::Kernel::PersistentChain2V1,
                view,
            )
            .unwrap();
        let right = other.execute_case_current(re, view).unwrap();
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
            Err(m::Error::ForeignCase)
        ));
        let source = SourceStore::new(Arc::from(SOURCE), 262144).unwrap();
        let entries = [m::ArchiveEntry {
            source: &source,
            query: h.theory,
            atoms: h.atoms,
            kernel: f::Kernel::ClassicalPointV1,
        }];
        let bad = registration(h.constructions, m::ArchiveInput::Read(&entries));
        assert!(matches!(
            m::Root::preregister(
                f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
                &bad,
                view
            ),
            Err(m::Error::ForeignSource)
        ));
    });
}

#[test]
fn tlcx22_p2_n12_cancellation_before_or_inside_callback_rejects_held_native_metric_owner() {
    for after in [false, true] {
        framework_fixture!(h, 2, false, Duration::from_secs(60), {
            let (h, view, cancel, _) = h;
            let fd = h.declaration();
            let base = baseline(&h);
            let entries = [archive(&h, &base, f::Kernel::PersistentChain2V1)];
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
            let calls = Cell::new(0);
            if !after {
                cancel.cancel();
            }
            assert!(matches!(
                owner.with_current(e, view, |_| {
                    calls.set(1);
                    cancel.cancel();
                    7u8
                }),
                Err(m::Error::Resource(ResourceError::Cancelled))
            ));
            assert_eq!(
                calls.get(),
                usize::from(after),
                "rejection after a callback does not undo its effect"
            );
        });
    }
}

#[test]
fn tlcx22_p2_n13_actual_deadline_does_not_reactivate_copied_archive_or_cached_scores() {
    framework_fixture!(h, 2, false, Duration::from_secs(10), {
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
        p2_pair!(root, generated, view, left, le, right, re);
        let e = pair(&root, &left, le, &right, re, view);
        let owner = root.measure_current(e, view).unwrap();
        owner
            .with_current(e, view, |v| {
                point(v.report(ComponentMetric::FrameworkDistance).unwrap(), 1, 4)
            })
            .unwrap();
        std::thread::sleep(
            view.remaining()
                .unwrap()
                .checked_add(Duration::from_millis(1))
                .unwrap(),
        );
        let calls = Cell::new(0);
        assert!(matches!(
            owner.with_current(e, view, |_| calls.set(1)),
            Err(m::Error::Resource(ResourceError::DeadlineExceeded))
        ));
        assert_eq!(calls.get(), 0);
    });
}

#[test]
fn tlcx22_p2_p03_given_atom_rename_and_canonical_duplicate_archive_do_not_inflate_similarity() {
    for rename in [false, true] {
        framework_fixture!(h, 2, rename, Duration::from_secs(60), {
            let (h, view, _, _) = h;
            let fd = h.declaration();
            let base = baseline(&h);
            let commute = query(
                &h,
                vec![var(p(&h)), BoolExpr::And(vec![var(q_id(&h)), var(p(&h))])],
            );
            let entries = [
                archive(&h, &base, f::Kernel::ClassicalPointV1),
                archive(&h, &commute, f::Kernel::PersistentChain2V1),
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
                    point(v.report(ComponentMetric::FrameworkDistance).unwrap(), 1, 4);
                    point(v.report(ComponentMetric::Knownness).unwrap(), 1, 3);
                    assert_eq!(v.counts().archive_comparisons, 2);
                    assert_eq!(v.counts().native_archive_dispatches, 52);
                    assert_eq!(v.knownness_union_weights(), &[q(3, 1), q(3, 1)]);
                })
                .unwrap();
        });
    }
}

#[test]
fn tlcx22_p2_p04_actual_reports_reach_strict_existing_consumers_without_p3_or_zero_rescue() {
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
        p2_pair!(root, generated, view, left, le, right, re);
        let e = pair(&root, &left, le, &right, re, view);
        let owner = root.measure_current(e, view).unwrap();
        owner.with_current(e, view, strict_missing).unwrap();
    });
}

#[test]
fn tlcx22_p2_p05_repeated_assertion_is_a_set_and_uses_actual_two_feature_union() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        let repeated = query(&h, vec![var(p(&h)), var(p(&h))]);
        let entries = [archive(&h, &repeated, f::Kernel::ClassicalPointV1)];
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
                point(v.report(ComponentMetric::Knownness).unwrap(), 1, 2);
                assert_eq!(v.knownness_union_weights(), &[q(2, 1)]);
                assert_eq!(v.counts().archive_comparisons, 1);
            })
            .unwrap();
    });
}

#[test]
fn tlcx22_p2_n14_root_total_archive_comparisons_and_attempts_cannot_be_reset_by_remeasure() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        let base = baseline(&h);
        let entries = [
            archive(&h, &base, f::Kernel::ClassicalPointV1),
            archive(&h, &base, f::Kernel::PersistentChain2V1),
        ];
        let mut r = registration(h.constructions, m::ArchiveInput::Read(&entries));
        r.limits.max_comparisons = 1;
        let mut root = m::Root::preregister(
            f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
            &r,
            view,
        )
        .unwrap();
        let generated = root.generate_current(operator(), view).unwrap();
        p2_pair!(root, generated, view, left, le, right, re);
        let e = pair(&root, &left, le, &right, re, view);
        let before = root.work_remaining_current(view).unwrap();
        for i in 0..4 {
            let owner = root.measure_current(e, view).unwrap();
            owner
                .with_current(e, view, |v| {
                    assert_eq!(v.archive_status(), m::ArchiveStatus::Partial);
                    assert_eq!(v.counts().archive_comparisons, usize::from(i == 0));
                    assert_eq!(
                        v.counts().native_archive_dispatches,
                        if i == 0 { 16 } else { 0 }
                    );
                    assert_eq!(
                        v.report(ComponentMetric::Knownness).unwrap().state,
                        ComponentState::ResourceExhausted {
                            partial: Some(if i == 0 {
                                RationalInterval::from_ratio(1, 3, 1, 1).unwrap()
                            } else {
                                RationalInterval::from_ratio(0, 1, 1, 1).unwrap()
                            })
                        }
                    );
                })
                .unwrap();
        }
        assert!(matches!(
            root.measure_current(e, view),
            Err(m::Error::AssessmentBudget)
        ));
        assert!(root.work_remaining_current(view).unwrap() < before);
    });
}
