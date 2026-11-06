//! The diagnostic IΓ0 reference and the registered CΓ0 reference share the actual IΓ1 target.
#[path = "tlcx22_framework_p2_support/mod.rs"]
mod support;
use support::*;

#[test]
fn p2_primary_and_control_with_one_target_keep_distinct_reference_baselines() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let declaration = h.declaration();
        let archive_query = baseline(&h);
        let entries = [archive(&h, &archive_query, f::Kernel::ClassicalPointV1)];
        let registered = registration(h.constructions, m::ArchiveInput::Read(&entries));
        let framework = f::FrameworkRoot::preregister(h.native(view), &declaration, view).unwrap();
        let mut root = m::Root::preregister(framework, &registered, view).unwrap();
        let generated = root.generate_current(operator(), view).unwrap();
        p2_pair!(root, generated, view, classical, ce, target, te);

        let ie = root
            .expected_case_current(f::Subject::Original, f::Kernel::PersistentChain2V1, view)
            .unwrap();
        let persistent = root.execute_case_current(ie, view).unwrap();
        let primary = pair(&root, &classical, ce, &target, te, view);
        let control = pair(&root, &persistent, ie, &target, te, view);
        assert!(!std::ptr::eq(primary.source.owner, control.source.owner));
        assert!(std::ptr::eq(primary.target.owner, control.target.owner));
        assert_eq!(ce.kernel, declaration.comparison.source.kernel);
        assert_eq!(te.kernel, declaration.comparison.target.kernel);
        assert_ne!(ie.kernel, ce.kernel);

        let observed_primary = root.measure_current(primary, view).unwrap();
        let observed_control = root.measure_current(control, view).unwrap();
        observed_primary
            .with_current(primary, view, |p| {
                observed_control
                    .with_current(control, view, |c| {
                        assert!(std::ptr::eq(p.candidate_query(), c.candidate_query()));
                        let p_distance = p.report(ComponentMetric::FrameworkDistance).unwrap();
                        let c_distance = c.report(ComponentMetric::FrameworkDistance).unwrap();
                        let p_known = p.report(ComponentMetric::Knownness).unwrap();
                        let c_known = c.report(ComponentMetric::Knownness).unwrap();
                        point(p_distance, 1, 4);
                        point(c_distance, 0, 1);
                        point(p_known, 1, 3);
                        point(c_known, 1, 3);

                        let ps = CandidateSubject::from_binding(p_distance.binding);
                        let cs = CandidateSubject::from_binding(c_distance.binding);
                        assert_eq!(ps.target, cs.target);
                        let metrics = [ComponentMetric::FrameworkDistance, ComponentMetric::Knownness];
                        let policy = |s: CandidateSubject| AssessmentPolicy {
                            scope: s.scope,
                            baseline: s.baseline,
                            required: &metrics,
                        };
                        ComponentAssessment::from_reports(&policy(ps), ps, &[p_distance, p_known])
                            .expect("two leaves of the registered primary comparison");
                        ComponentAssessment::from_reports(&policy(cs), cs, &[c_distance, c_known])
                            .expect("two leaves of the diagnostic control comparison");
                        let mixed_rejected = matches!(
                            ComponentAssessment::from_reports(&policy(ps), ps, &[p_distance, c_known]),
                            Err(MeasurementError::ScopeMismatch)
                        );
                        assert_ne!(
                            ps.baseline, cs.baseline,
                            "changing the actual reference CΓ0 to IΓ0 must change baseline; mixed leaves rejected: {mixed_rejected}"
                        );
                        assert!(mixed_rejected, "a control leaf was admitted into a primary assessment");
                    })
                    .unwrap();
            })
            .unwrap();
    });
}
