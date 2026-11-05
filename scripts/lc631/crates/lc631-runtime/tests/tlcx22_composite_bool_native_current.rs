//! 日本語: R2追加TEST-FIRST。実native対応の正例を必須にし、全F2成功とはしない。
//! English: Additive R2 test-first: require actual native correspondence, never full F2 closure.
#[path = "tlcx22_composite_bool_native_support/mod.rs"]
mod support;
use lc631_analysis::creativity_measurement::{IndependenceStatus, InputBasis, SemanticValidation};
use lc631_substrate::resource::ResourceError;
use support::*;

#[test]
fn b01_actual_bridge_gain_and_exact_inner_c_program_are_consumed_without_retagging() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        let baseline = fx.forward(true);
        let baseline_inverse = Fixture::inverse(&baseline);
        let good = fx.forward(false);
        let inverse = Fixture::inverse(&good);
        let coordinate = join::BoolCoordinateSpec {
            native: fx.input(cases),
            composite_goal: d(0x91),
            declared_relation:
                b"Given finite port contract; relation to composite coding goal remains unassessed",
        };
        let payload = join::BoolTarget {
            forward: &good,
            inverse: &inverse,
        };
        let root = b::BridgeRoot::given(
            root_input(coordinate.native, spec(&baseline, &baseline_inverse), 48),
            view,
        )
        .unwrap();
        let baseline_program = root.baseline_program(view).unwrap();
        let program = root
            .candidate_program(spec(payload.forward, payload.inverse), view)
            .unwrap();
        join::with_native_current(&root, &program, view, |native| {
            let expected = join::NativeExpected {
                plan: native.plan(),
                program: native.program(),
            };
            // Capture actual inner occurrence references BEFORE either measurement branch runs.
            let baseline_branch = root.prepare(&baseline_program, view).unwrap();
            let branch = root.prepare(&program, view).unwrap();
            let baseline_uses = baseline_branch
                .execute_uses(view)
                .unwrap_or_else(|_| panic!("actual baseline uses failed"));
            let candidate_uses = branch
                .execute_uses(view)
                .unwrap_or_else(|_| panic!("actual candidate uses failed"));
            let gain = b::measure_pair(&root, &baseline_uses, &candidate_uses, view).unwrap();
            let report = gain.report_current(&root, &program, view).unwrap();
            assert_eq!(bounds(&report.state), &ratio(1, 2));
            assert_eq!(branch.hard_current(view).unwrap(), c::HardState::Pass);
            assert_eq!(
                bounds(&branch.parent_report_current(view).unwrap().state),
                &unknown()
            );
            assert_eq!(native.forward(), payload.forward);
            assert_eq!(native.inverse(), payload.inverse);
            assert!(std::ptr::eq(
                native.input().source,
                coordinate.native.source
            ));
            assert!(std::ptr::eq(
                native.input().source_theory,
                coordinate.native.source_theory
            ));
            let owners = join::Owners {
                root: &root,
                program: &program,
                gain: &gain,
                native: expected,
            };
            let token = join::capture_current(&coordinate, payload, owners, view).unwrap();
            token
                .with_current(&coordinate, payload, owners, view, |current| {
                    assert!(std::ptr::eq(current.bridge_report(), report));
                    assert!(std::ptr::eq(current.native_plan(), native.plan()));
                    assert!(std::ptr::eq(current.native_program(), native.program()));
                    assert_eq!(current.forward(), payload.forward);
                    assert_eq!(current.inverse(), payload.inverse);
                    assert_eq!(current.consumed_metric(), M::Bridge);
                    assert_eq!(
                        current.native_missing_leaves(OriginalFormula::F2V1),
                        F2_WITHOUT_BRIDGE
                    );
                    assert_eq!(
                        current.original_missing_leaves(OriginalFormula::F2V1),
                        F2_ALL
                    );
                    assert_eq!(
                        current.composite_applicability(),
                        join::CompositeApplicability::Unassessed
                    );
                    assert_eq!(current.provenance(), join::Provenance::ConditionalGiven);
                    assert_eq!(current.bridge_report().basis, InputBasis::Given);
                    assert_eq!(
                        current.bridge_report().validation,
                        SemanticValidation::Unassessed
                    );
                    assert_eq!(
                        current.bridge_report().independence,
                        IndependenceStatus::SharedDependency
                    );
                    for gap in [
                        join::OriginalGap::FullCompositeCost,
                        join::OriginalGap::AllRootTl,
                        join::OriginalGap::RootApplicability,
                        join::OriginalGap::OtherNativeOwners,
                        join::OriginalGap::CompositeHardEvidence,
                    ] {
                        assert!(current.original_gaps().contains(&gap));
                    }
                    assert!(current.original_scalar().is_none());
                    assert!(!current.original_formula_evaluated());
                    assert_eq!(current.recommendation(), None);
                })
                .unwrap();
        })
        .unwrap();
        assert_eq!(root.work_used_current(view).unwrap(), 48);
    });
}
#[test]
fn b02_same_native_program_hash_does_not_substitute_a_different_actual_c_program() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        actual_pair!(fx,cases,view;root,program,branch,gain,coordinate,payload);
        join::with_native_current(&root, &program, view, |native| {
            let other = native
                .plan()
                .permutation(payload.forward, payload.inverse, view)
                .unwrap();
            let other_owner = c::measure(native.plan(), &other, &[], view).unwrap();
            let other_report = other_owner
                .report_current(native.plan(), &other, view)
                .unwrap();
            assert_eq!(
                other_report.binding.candidate,
                gain.report_current(&root, &program, view)
                    .expect("actual gain current report")
                    .binding
                    .candidate
            );
            assert_eq!(bounds(&other_report.state), &unknown());
            assert_eq!(root.work_used_current(view).unwrap(), 48); // empty schedule performs no stages
            let owners = join::Owners {
                root: &root,
                program: &program,
                gain: &gain,
                native: join::NativeExpected {
                    plan: native.plan(),
                    program: &other,
                },
            };
            assert!(matches!(
                join::capture_current(&coordinate, payload, owners, view),
                Err(join::Error::ForeignNativeProgram)
            ));
        })
        .unwrap();
    });
}
#[test]
fn b03_equal_native_plan_content_is_not_the_expected_plan_occurrence() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        actual_pair!(fx,cases,view;root,program,branch,gain,coordinate,payload);
        let other = c::Plan::given(
            coordinate.native,
            c::Limits {
                max_work: 48,
                max_frame_bytes: 1024,
                max_program_steps: 3,
            },
            view,
        )
        .unwrap();
        join::with_native_current(&root, &program, view, |native| {
            let owners = join::Owners {
                root: &root,
                program: &program,
                gain: &gain,
                native: join::NativeExpected {
                    plan: &other,
                    program: native.program(),
                },
            };
            assert!(matches!(
                join::capture_current(&coordinate, payload, owners, view),
                Err(join::Error::ForeignNativePlan)
            ));
        })
        .unwrap();
    });
}
#[test]
fn b04_forward_inverse_or_expected_input_cut_rejects_and_restoration_consumes_actual_owner() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        actual_pair!(fx,cases,view;root,program,branch,gain,coordinate,payload);
        join::with_native_current(&root, &program, view, |native| {
            let owners = join::Owners {
                root: &root,
                program: &program,
                gain: &gain,
                native: join::NativeExpected {
                    plan: native.plan(),
                    program: native.program(),
                },
            };
            let swapped = fx.forward(true);
            let swapped_inverse = Fixture::inverse(&swapped);
            for wrong in [
                join::BoolTarget {
                    forward: &swapped,
                    inverse: payload.inverse,
                },
                join::BoolTarget {
                    forward: payload.forward,
                    inverse: &swapped_inverse,
                },
            ] {
                assert!(matches!(
                    join::capture_current(&coordinate, wrong, owners, view),
                    Err(join::Error::CoordinatePayloadMismatch)
                ));
            }
            let clone = Fixture::new(NORMAL);
            let clone_cases = clone.cases();
            let copied = join::BoolCoordinateSpec {
                native: clone.input(&clone_cases),
                composite_goal: coordinate.composite_goal,
                declared_relation: coordinate.declared_relation,
            };
            assert_eq!(copied.native.source.id(), coordinate.native.source.id());
            assert!(matches!(
                join::capture_current(&copied, payload, owners, view),
                Err(join::Error::CoordinateInputMismatch)
            ));
            let restored = join::capture_current(&coordinate, payload, owners, view).unwrap();
            restored
                .with_current(&coordinate, payload, owners, view, |current| {
                    assert_eq!(bounds(&current.bridge_report().state), &ratio(1, 2));
                    assert_eq!(
                        current.native_missing_leaves(OriginalFormula::F2V1),
                        F2_WITHOUT_BRIDGE
                    );
                    assert!(current.original_scalar().is_none());
                })
                .unwrap();
        })
        .unwrap();
    });
}
#[test]
fn b05_equal_actual_gain_and_program_hash_from_another_b_owner_is_rejected() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        actual_pair!(fx,cases,view;root,program,branch,gain,coordinate,payload);
        actual_pair!(fx,cases,view;other_root,other_program,other_branch,other_gain,other_coordinate,other_payload);
        assert_eq!(
            bounds(
                &gain
                    .report_current(&root, &program, view)
                    .expect("actual gain current report")
                    .state
            ),
            &ratio(1, 2)
        );
        assert_eq!(
            bounds(
                &other_gain
                    .report_current(&other_root, &other_program, view)
                    .expect("other actual gain current report")
                    .state
            ),
            &ratio(1, 2)
        );
        assert_eq!(
            gain.report_current(&root, &program, view)
                .expect("actual gain current report")
                .binding
                .candidate,
            other_gain
                .report_current(&other_root, &other_program, view)
                .expect("other actual gain current report")
                .binding
                .candidate
        );
        join::with_native_current(&root, &program, view, |native| {
            let owners = join::Owners {
                root: &root,
                program: &program,
                gain: &other_gain,
                native: join::NativeExpected {
                    plan: native.plan(),
                    program: native.program(),
                },
            };
            assert!(matches!(
                join::capture_current(&coordinate, payload, owners, view),
                Err(join::Error::Bridge(b::BridgeError::BindingMismatch))
            ));
        })
        .unwrap();
        assert_eq!(root.work_used_current(view).unwrap(), 48);
        assert_eq!(other_root.work_used_current(view).unwrap(), 48);
    });
}
#[test]
fn b06_foreign_current_reservation_cannot_refresh_actual_correspondence() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        actual_pair!(fx,cases,view;root,program,branch,gain,coordinate,payload);
        join::with_native_current(&root, &program, view, |native| {
            let owners = join::Owners {
                root: &root,
                program: &program,
                gain: &gain,
                native: join::NativeExpected {
                    plan: native.plan(),
                    program: native.program(),
                },
            };
            let token = join::capture_current(&coordinate, payload, owners, view).unwrap();
            fixture(NORMAL, 2 * 1024 * 1024, |_, _, foreign, _| {
                assert_eq!(view.source_revision(), foreign.source_revision());
                assert!(matches!(
                    token.with_current(&coordinate, payload, owners, foreign, |_| ()),
                    Err(join::Error::ForeignReservation)
                ));
            });
        })
        .unwrap();
    });
}
#[test]
fn b07_cancellation_inside_consumer_invalidates_actual_report_and_native_visitor() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, cancel| {
        actual_pair!(fx,cases,view;root,program,branch,gain,coordinate,payload);
        let result = join::with_native_current(&root, &program, view, |native| {
            let owners = join::Owners {
                root: &root,
                program: &program,
                gain: &gain,
                native: join::NativeExpected {
                    plan: native.plan(),
                    program: native.program(),
                },
            };
            let token = join::capture_current(&coordinate, payload, owners, view).unwrap();
            let result = token.with_current(&coordinate, payload, owners, view, |current| {
                assert_eq!(bounds(&current.bridge_report().state), &ratio(1, 2));
                cancel.cancel();
            });
            assert!(matches!(
                result,
                Err(join::Error::Resource(ResourceError::Cancelled))
            ));
        });
        assert!(matches!(
            result,
            Err(join::Error::Resource(ResourceError::Cancelled))
        ));
    });
}
