//! Compare the actual root-ledger cost of one P2 measurement with its own call cap.
#[path = "tlcx22_framework_p2_support/mod.rs"]
mod support;
use support::*;
#[path = "beta7_framework_fixture_support/mod.rs"]
mod native_fixture;
use native_fixture::capture_native;

const ARCHIVE_ENTRIES: usize = 8;

fn measure_at(
    h: &Inputs<'_>,
    view: &ResourceReservationView<'_>,
    call_limit: u64,
    verify_funded: bool,
) -> Result<u64, m::Error> {
    let mut limits = context_limits();
    limits.max_call_work = call_limit;
    let native = capture_native(h, limits, view).expect("native root admission");
    let declaration = h.declaration();
    let framework = f::FrameworkRoot::preregister(native, &declaration, view)
        .expect("P1 capability must be funded before this P2 cut");
    let query = baseline(h);
    let entries = [archive(h, &query, f::Kernel::PersistentChain2V1); ARCHIVE_ENTRIES];
    let registered = registration(h.constructions, m::ArchiveInput::Read(&entries));
    let mut root = m::Root::preregister(framework, &registered, view)
        .expect("P2 archive/feature admission must succeed before measurement");
    let generated = root.generate_current(operator(), view).expect("generated candidate");
    p2_pair!(root, generated, view, left, le, right, re);
    let expected = pair(&root, &left, le, &right, re, view);

    // Each public meter read performs its own current checks. Observe that
    // cost independently, then remove exactly one read from the measured delta.
    let earlier = root.work_remaining_current(view).unwrap();
    let before = root.work_remaining_current(view).unwrap();
    let meter_read = earlier.checked_sub(before).expect("monotone root meter");
    let measured = root.measure_current(expected, view)?;
    let after = root.work_remaining_current(view).unwrap();
    let charged = before
        .checked_sub(after)
        .and_then(|delta| delta.checked_sub(meter_read))
        .expect("the root meter includes the final read");
    if verify_funded {
        measured.with_current(expected, view, |current| {
            assert_eq!(current.archive_status(), m::ArchiveStatus::Complete);
            assert_eq!(current.counts().archive_comparisons, ARCHIVE_ENTRIES);
            // 9 legal valuations × (2 original assertions + hard + root goal).
            assert_eq!(current.counts().native_archive_dispatches, ARCHIVE_ENTRIES * 36);
            point(current.report(ComponentMetric::FrameworkDistance).unwrap(), 1, 4);
            point(current.report(ComponentMetric::Knownness).unwrap(), 1, 3);
        }).unwrap();
    }
    Ok(charged)
}

#[test]
fn p2_measurement_one_work_unit_short_refuses_a_published_owner() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let full_cap = context_limits().max_call_work;
        let charged = measure_at(&h, view, full_cap, true).expect("funded primary archive measurement");
        assert!((1..=full_cap).contains(&charged), "funded measurement charged {charged} of {full_cap}");
        let short = charged - 1;
        // A fresh native root has a fresh total. Only the per-operation ceiling
        // changes; the same real reference, candidate, corpus and work are used.
        let result = measure_at(&h, view, short, false);
        assert!(
            matches!(result, Err(m::Error::WorkBudget)),
            "one measure_current charged {charged} work; call cap {short} returned {result:?}"
        );
    });
}
