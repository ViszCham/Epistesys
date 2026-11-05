//! Current native/map/owner cuts, separate from capability admission and controls.
use lc631_runtime::context_novelty_measurement::framework as f;
mod tlcx22_framework_p1_support;
use tlcx22_framework_p1_support::*;

#[test]
fn tlcx22_p1_n05_foreign_root_equal_digest_cloned_ast_and_changed_backing_are_not_current() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let declaration = h.declaration();
        let root = f::FrameworkRoot::preregister(h.native(view), &declaration, view).unwrap();
        let other = f::FrameworkRoot::preregister(h.native(view), &declaration, view).unwrap();
        assert_eq!(root.historical_digest(), other.historical_digest());
        let expected = root
            .expected_inputs_current(f::Subject::Original, f::Kernel::PersistentChain2V1, view)
            .unwrap();
        let owner = root.execute_case_current(expected, view).unwrap();
        let foreign = other
            .expected_inputs_current(f::Subject::Original, f::Kernel::PersistentChain2V1, view)
            .unwrap();
        assert!(matches!(
            owner.with_current(foreign, view, |_| ()),
            Err(f::Error::ForeignRoot)
        ));
        let clone = expected.subject_query.clone();
        let mut changed = expected;
        changed.subject_query = &clone;
        assert!(matches!(
            owner.with_current(changed, view, |_| ()),
            Err(f::Error::ExpectedInputMismatch)
        ));
        let equal_declaration = h.declaration();
        let mut changed = expected;
        changed.declaration = &equal_declaration;
        assert!(matches!(
            owner.with_current(changed, view, |_| ()),
            Err(f::Error::ExpectedInputMismatch)
        ));
        let source_copy = SourceStore::new(Arc::from(SOURCE), 262144).unwrap();
        let mut changed = expected;
        changed.source = &source_copy;
        assert!(matches!(
            owner.with_current(changed, view, |_| ()),
            Err(f::Error::ExpectedInputMismatch)
        ));
        let mut changed = expected;
        changed.kernel = f::Kernel::ClassicalPointV1;
        assert!(matches!(
            owner.with_current(changed, view, |_| ()),
            Err(f::Error::ExpectedInputMismatch)
        ));
    });
}

#[test]
fn tlcx22_p1_n06_foreign_resource_and_callback_cancellation_cut_native_current() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, cancel, run) = h;
        let declaration = h.declaration();
        let root = f::FrameworkRoot::preregister(h.native(view), &declaration, view).unwrap();
        let expected = root
            .expected_inputs_current(f::Subject::Original, f::Kernel::PersistentChain2V1, view)
            .unwrap();
        let owner = root.execute_case_current(expected, view).unwrap();
        let foreign = run
            .reserve(&[resource(ResourceKind::Ram, 1024 * 1024)])
            .unwrap();
        let foreign_view = foreign.lease();
        let calls = Cell::new(0);
        assert!(matches!(
            owner.with_current(expected, &foreign_view, |_| calls.set(1)),
            Err(f::Error::ForeignReservation)
        ));
        assert_eq!(calls.get(), 0);
        assert!(matches!(
            owner.with_current(expected, view, |_| {
                calls.set(1);
                cancel.cancel();
                7u8
            }),
            Err(f::Error::Resource(ResourceError::Cancelled))
        ));
        assert_eq!(
            calls.get(),
            1,
            "post-callback rejection is not rollback/atomicity"
        );
    });
}

#[test]
fn tlcx22_p1_n09_actual_candidate_from_equal_given_foreign_root_is_rejected() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let declaration = h.declaration();
        let root = f::FrameworkRoot::preregister(h.native(view), &declaration, view).unwrap();
        let mut other = f::FrameworkRoot::preregister(h.native(view), &declaration, view).unwrap();
        let actual_foreign = other.generate_current(operator(), view).unwrap();
        assert!(matches!(
            root.expected_inputs_current(
                f::Subject::Generated(&actual_foreign),
                f::Kernel::PersistentChain2V1,
                view
            ),
            Err(f::Error::ForeignCandidate)
        ));
    });
}

#[test]
fn tlcx22_p1_n10_deadline_cuts_held_owner_not_a_copied_historical_native_report() {
    framework_fixture!(h, 2, false, Duration::from_secs(3), {
        let (h, view, _, _) = h;
        let declaration = h.declaration();
        let root = f::FrameworkRoot::preregister(h.native(view), &declaration, view).unwrap();
        let expected = root
            .expected_inputs_current(f::Subject::Original, f::Kernel::PersistentChain2V1, view)
            .unwrap();
        let owner = root.execute_case_current(expected, view).unwrap();
        owner.with_current(expected, view, |_| ()).unwrap();
        let remaining = view.remaining().unwrap();
        std::thread::sleep(remaining.checked_add(Duration::from_millis(1)).unwrap());
        let calls = Cell::new(0);
        assert!(matches!(
            owner.with_current(expected, view, |_| calls.set(1)),
            Err(f::Error::Resource(ResourceError::DeadlineExceeded))
        ));
        assert_eq!(calls.get(), 0);
    });
}

#[test]
fn tlcx22_p1_p04_display_aliases_do_not_create_different_executable_signatures() {
    framework_fixture!(h, 2, true, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let declaration = h.declaration();
        let mut aliases = PROFILES;
        aliases[0].display_name = "persistent";
        aliases[1].display_name = "classical";
        let mut renamed = h.declaration();
        renamed.profiles = &aliases;
        let first = f::FrameworkRoot::preregister(h.native(view), &declaration, view).unwrap();
        let second = f::FrameworkRoot::preregister(h.native(view), &renamed, view).unwrap();
        assert_eq!(
            first.historical_digest(),
            second.historical_digest(),
            "display aliases are not scope/semantic identity"
        );
        for kernel in [f::Kernel::ClassicalPointV1, f::Kernel::PersistentChain2V1] {
            let a = first
                .expected_inputs_current(f::Subject::Original, kernel, view)
                .unwrap();
            let b = second
                .expected_inputs_current(f::Subject::Original, kernel, view)
                .unwrap();
            let ao = first.execute_case_current(a, view).unwrap();
            let bo = second.execute_case_current(b, view).unwrap();
            let packages = ao
                .with_current(a, view, |v| v.signature_packages().to_vec())
                .unwrap();
            bo.with_current(b, view, |v| {
                assert_eq!(v.signature_packages(), packages.as_slice())
            })
            .unwrap();
        }
    });
}
