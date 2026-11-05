//! Additional TEST-FIRST packet; no body/shared/source repair is authorized here.
//! Construction/runtime outcomes belong to the central queue, not these comments.
#[path = "tlcx22_framework_p2_support/mod.rs"]
mod frozen;
use frozen::*;
mod tlcx22_framework_p2_native_budget_edges_support;
use tlcx22_framework_p2_native_budget_edges_support::{
    no_completed_comparison, partial, signature,
};

#[test]
fn tlcx22_p2_native_zero_fifteen_and_exact_sixteen_are_distinct() {
    for native_budget in [0, 15, 16] {
        framework_fixture!(h, 2, false, Duration::from_secs(60), {
            let (h, view, _, _) = h;
            let fd = h.declaration();
            let base = baseline(&h);
            let entries = [archive(&h, &base, f::Kernel::ClassicalPointV1)];
            let mut r = registration(h.constructions, m::ArchiveInput::Read(&entries));
            r.limits.max_archive_dispatches = native_budget;
            assert_eq!(r.limits.max_comparisons, 64);
            let mut root = m::Root::preregister(
                f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
                &r,
                view,
            )
            .unwrap();
            let generated = root.generate_current(operator(), view).unwrap();
            p2_pair!(root, generated, view, left, le, right, re);
            let expected = pair(&root, &left, le, &right, re, view);
            let owner = root.measure_current(expected, view).unwrap();
            let callbacks = Cell::new(0);
            owner
                .with_current(expected, view, |current| {
                    callbacks.set(callbacks.get() + 1);
                    signature(&current);
                    assert_eq!(current.archive_queries().len(), 1);
                    assert!(std::ptr::eq(current.archive_queries()[0], &base));
                    assert!(std::ptr::eq(current.candidate_query(), re.subject_query));
                    assert_eq!(current.corpus_revision(), r.corpus_revision);
                    assert_eq!(current.counts().native_archive_store_entries, 0);
                    match native_budget {
                        0 => {
                            partial(&current, 0, 1);
                            no_completed_comparison(&current);
                            assert_eq!(current.counts().native_archive_dispatches, 0);
                            assert!(current.counts().archive_entries_seen <= 1);
                        }
                        15 => {
                            partial(&current, 0, 1);
                            no_completed_comparison(&current);
                            // Contract permits a whole-entry preflight rejection
                            // (zero calls), OR a retained incomplete native prefix.
                            // Do not invent a compulsory15-call exhaustion policy.
                            assert!(current.counts().native_archive_dispatches <= 15);
                            assert!(current.counts().archive_entries_seen <= 1);
                        }
                        16 => {
                            assert_eq!(current.archive_status(), m::ArchiveStatus::Complete);
                            point(current.report(ComponentMetric::Knownness).unwrap(), 1, 3);
                            assert_eq!(current.counts().archive_entries_seen, 1);
                            assert_eq!(current.counts().archive_comparisons, 1);
                            // 4 point valuations x(2 actual Gamma+hard+goal) =16.
                            assert_eq!(current.counts().native_archive_dispatches, 16);
                            assert_eq!(current.knownness_union_weights(), &[q(3, 1)]);
                        }
                        _ => panic!("only the three literal native caps are exercised"),
                    }
                    strict_missing(current);
                })
                .unwrap();
            assert_eq!(callbacks.get(), 1);
        });
    }
}

#[test]
fn tlcx22_p2_native_root_total_is_nonrefundable_with_comparisons_to_spare() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        let base = baseline(&h);
        let entries = [
            archive(&h, &base, f::Kernel::ClassicalPointV1),
            archive(&h, &base, f::Kernel::ClassicalPointV1),
        ];
        let mut r = registration(h.constructions, m::ArchiveInput::Read(&entries));
        r.limits.max_archive_dispatches = 16;
        assert_eq!(r.limits.max_comparisons, 64);
        let mut root = m::Root::preregister(
            f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
            &r,
            view,
        )
        .unwrap();
        let generated = root.generate_current(operator(), view).unwrap();
        p2_pair!(root, generated, view, left, le, right, re);
        let expected = pair(&root, &left, le, &right, re, view);
        let before = root.work_remaining_current(view).unwrap();
        let callbacks = Cell::new(0);
        for attempt in 0..4 {
            let owner = root.measure_current(expected, view).unwrap();
            owner
                .with_current(expected, view, |current| {
                    callbacks.set(callbacks.get() + 1);
                    signature(&current);
                    assert_eq!(current.counts().native_archive_store_entries, 0);
                    if attempt == 0 {
                        partial(&current, 1, 3);
                        assert_eq!(current.counts().archive_comparisons, 1);
                        assert_eq!(current.counts().native_archive_dispatches, 16);
                        assert_eq!(current.knownness_union_weights(), &[q(3, 1)]);
                        assert!((1..=2).contains(&current.counts().archive_entries_seen));
                    } else {
                        partial(&current, 0, 1);
                        no_completed_comparison(&current);
                        assert_eq!(current.counts().native_archive_dispatches, 0);
                        assert!(current.counts().archive_entries_seen <= 1);
                    }
                })
                .unwrap();
            // Dropping an assessment frees backing, not root dispatch quota.
            drop(owner);
        }
        assert_eq!(callbacks.get(), 4);
        assert!(matches!(
            root.measure_current(expected, view),
            Err(m::Error::AssessmentBudget)
        ));
        assert!(root.work_remaining_current(view).unwrap() < before);
    });
}

#[test]
fn tlcx22_p2_two_complete_duplicate_entries_really_pay_thirty_two_native_calls() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        let base = baseline(&h);
        let entries = [
            archive(&h, &base, f::Kernel::ClassicalPointV1),
            archive(&h, &base, f::Kernel::ClassicalPointV1),
        ];
        let mut r = registration(h.constructions, m::ArchiveInput::Read(&entries));
        r.limits.max_archive_dispatches = 32;
        let mut root = m::Root::preregister(
            f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
            &r,
            view,
        )
        .unwrap();
        let generated = root.generate_current(operator(), view).unwrap();
        p2_pair!(root, generated, view, left, le, right, re);
        let expected = pair(&root, &left, le, &right, re, view);
        let owner = root.measure_current(expected, view).unwrap();
        let callbacks = Cell::new(0);
        owner
            .with_current(expected, view, |current| {
                callbacks.set(callbacks.get() + 1);
                signature(&current);
                point(current.report(ComponentMetric::Knownness).unwrap(), 1, 3);
                assert_eq!(current.archive_status(), m::ArchiveStatus::Complete);
                assert_eq!(current.counts().archive_entries_seen, 2);
                assert_eq!(current.counts().archive_comparisons, 2);
                assert_eq!(current.counts().native_archive_dispatches, 32);
                assert_eq!(current.counts().native_archive_store_entries, 0);
                assert_eq!(current.knownness_union_weights(), &[q(3, 1), q(3, 1)]);
            })
            .unwrap();
        assert_eq!(callbacks.get(), 1);
    });
}

#[test]
fn tlcx22_p2_exact_prefix_one_cannot_hide_late_unbridged_archive_gamma() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let fd = h.declaration();
        let exact_query = exact(&h);
        // At p=00,q=11 this original Gamma is feasible but hard/goal p fail.
        // Appending hard/goal to Gamma would incorrectly erase that witness.
        let unbridged = query(&h, vec![var(q_id(&h)), var(q_id(&h))]);
        let entries = [
            archive(&h, &exact_query, f::Kernel::ClassicalPointV1),
            archive(&h, &unbridged, f::Kernel::PersistentChain2V1),
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
        let expected = pair(&root, &left, le, &right, re, view);
        let owner = root.measure_current(expected, view).unwrap();
        let callbacks = Cell::new(0);
        owner
            .with_current(expected, view, |current| {
                callbacks.set(callbacks.get() + 1);
                signature(&current);
                assert_eq!(current.archive_status(), m::ArchiveStatus::NoComparable);
                assert!(matches!(
                    current.report(ComponentMetric::Knownness).unwrap().state,
                    ComponentState::Incomparable { .. }
                ));
                assert_eq!(current.counts().archive_entries_seen, 2);
                assert_eq!(current.counts().archive_comparisons, 1);
                // Exact prefix:16; complete late chain bank:9models x4roots=36.
                assert_eq!(current.counts().native_archive_dispatches, 52);
                assert_eq!(current.counts().native_archive_store_entries, 36);
                assert_eq!(current.knownness_union_weights(), &[q(2, 1)]);
                assert_eq!(current.archive_queries().len(), 2);
                assert!(std::ptr::eq(current.archive_queries()[0], &exact_query));
                assert!(std::ptr::eq(current.archive_queries()[1], &unbridged));
                strict_missing(current);
            })
            .unwrap();
        assert_eq!(callbacks.get(), 1);
    });
}

#[test]
fn tlcx22_p2_complete_archive_max_is_invariant_under_preregistered_order() {
    for reverse in [false, true] {
        framework_fixture!(h, 2, false, Duration::from_secs(60), {
            let (h, view, _, _) = h;
            let fd = h.declaration();
            let base = baseline(&h);
            let exact_query = exact(&h);
            let entries = if reverse {
                [
                    archive(&h, &exact_query, f::Kernel::PersistentChain2V1),
                    archive(&h, &base, f::Kernel::ClassicalPointV1),
                ]
            } else {
                [
                    archive(&h, &base, f::Kernel::ClassicalPointV1),
                    archive(&h, &exact_query, f::Kernel::PersistentChain2V1),
                ]
            };
            let mut r = registration(h.constructions, m::ArchiveInput::Read(&entries));
            r.corpus_revision = d(if reverse { 99 } else { 98 });
            let mut root = m::Root::preregister(
                f::FrameworkRoot::preregister(h.native(view), &fd, view).unwrap(),
                &r,
                view,
            )
            .unwrap();
            let generated = root.generate_current(operator(), view).unwrap();
            p2_pair!(root, generated, view, left, le, right, re);
            let expected = pair(&root, &left, le, &right, re, view);
            let owner = root.measure_current(expected, view).unwrap();
            let callbacks = Cell::new(0);
            owner
                .with_current(expected, view, |current| {
                    callbacks.set(callbacks.get() + 1);
                    signature(&current);
                    point(current.report(ComponentMetric::Knownness).unwrap(), 1, 1);
                    assert_eq!(current.archive_status(), m::ArchiveStatus::Complete);
                    assert_eq!(current.counts().archive_entries_seen, 2);
                    assert_eq!(current.counts().archive_comparisons, 2);
                    assert_eq!(current.counts().native_archive_dispatches, 52);
                    assert_eq!(current.counts().native_archive_store_entries, 36);
                    let unions = if reverse {
                        [q(2, 1), q(3, 1)]
                    } else {
                        [q(3, 1), q(2, 1)]
                    };
                    assert_eq!(current.knownness_union_weights(), &unions);
                    assert_eq!(current.archive_queries().len(), 2);
                    assert!(std::ptr::eq(
                        current.archive_queries()[0],
                        if reverse { &exact_query } else { &base }
                    ));
                    assert!(std::ptr::eq(
                        current.archive_queries()[1],
                        if reverse { &base } else { &exact_query }
                    ));
                    assert!(std::ptr::eq(current.candidate_query(), re.subject_query));
                    assert_eq!(current.corpus_revision(), r.corpus_revision);
                    strict_missing(current);
                })
                .unwrap();
            assert_eq!(callbacks.get(), 1);
        });
    }
}
