//! 日本語: 実native ownerの32-read境界。中央Cargo前は未実行、成功を主張しない。
//! English: Actual native-owner read boundaries; unrun until central Cargo validation.
#![forbid(unsafe_code)]
#[path = "tlcx22_composite_bool_native_support/mod.rs"]
mod support;

use lc631_analysis::creativity_measurement::ComponentReport;
use std::cell::Cell;
use support::*;

fn assert_actual(
    current: join::CurrentBoolCorrespondence<'_, '_, '_, '_>,
    report: &ComponentReport,
) {
    assert!(std::ptr::eq(current.bridge_report(), report));
    assert_eq!(bounds(&current.bridge_report().state), &ratio(1, 2));
    assert_eq!(current.consumed_metric(), M::Bridge);
    assert_eq!(
        current.native_missing_leaves(OriginalFormula::F2V1),
        F2_WITHOUT_BRIDGE
    );
    assert_eq!(
        current.original_missing_leaves(OriginalFormula::F2V1),
        F2_ALL
    );
    assert!(current.original_scalar().is_none());
    assert!(!current.original_formula_evaluated());
    assert_eq!(current.recommendation(), None);
}

#[test]
fn tlcx22_composite_capture_uses_zero_reads_then_32_succeed_and_33_is_rejected() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        actual_pair!(fx,cases,view;root,program,branch,gain,coordinate,payload);
        assert_eq!(branch.hard_current(view).unwrap(), c::HardState::Pass);
        let report = gain.report_current(&root, &program, view).unwrap();
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
            // Capture initializes reads=0. It does not consume one of these 32
            // current calls; the literal boundary must not be copied from a getter.
            let token = join::capture_current(&coordinate, payload, owners, view).unwrap();
            let callbacks = Cell::new(0usize);
            for ordinal in 1..=32 {
                token
                    .with_current(&coordinate, payload, owners, view, |current| {
                        assert!(std::ptr::eq(current.native_plan(), native.plan()));
                        assert!(std::ptr::eq(current.native_program(), native.program()));
                        assert_actual(current, report);
                        callbacks.set(callbacks.get() + 1);
                    })
                    .unwrap();
                assert_eq!(callbacks.get(), ordinal);
            }
            let exhausted = token.with_current(&coordinate, payload, owners, view, |_| {
                callbacks.set(callbacks.get() + 1);
            });
            assert!(matches!(exhausted, Err(join::Error::ReadBudget)));
            assert_eq!(
                callbacks.get(),
                32,
                "the 33rd read must not invoke its callback"
            );
        })
        .unwrap();
        assert_eq!(root.work_used_current(view).unwrap(), 48);
    });
}

#[test]
fn tlcx22_composite_31_failed_coordinate_reads_leave_exactly_one_current_read() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        actual_pair!(fx,cases,view;root,program,branch,gain,coordinate,payload);
        assert_eq!(branch.hard_current(view).unwrap(), c::HardState::Pass);
        let report = gain.report_current(&root, &program, view).unwrap();
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
            // Equal payload in a different coordinate occurrence reaches the
            // pre-payload owner cut, after the actual current read was debited.
            let foreign = join::BoolCoordinateSpec {
                native: coordinate.native,
                composite_goal: coordinate.composite_goal,
                declared_relation: coordinate.declared_relation,
            };
            assert!(!std::ptr::eq(&coordinate, &foreign));
            let callbacks = Cell::new(0usize);
            for _ in 0..31 {
                let rejected = token.with_current(&foreign, payload, owners, view, |_| {
                    callbacks.set(callbacks.get() + 1);
                });
                assert!(matches!(rejected, Err(join::Error::ForeignCoordinate)));
            }
            assert_eq!(
                callbacks.get(),
                0,
                "failed owner checks must not call the consumer"
            );
            token
                .with_current(&coordinate, payload, owners, view, |current| {
                    assert_actual(current, report);
                    callbacks.set(callbacks.get() + 1);
                })
                .unwrap();
            assert_eq!(
                callbacks.get(),
                1,
                "31 failures leave the 32nd read available"
            );
            let exhausted = token.with_current(&coordinate, payload, owners, view, |_| {
                callbacks.set(callbacks.get() + 1);
            });
            assert!(matches!(exhausted, Err(join::Error::ReadBudget)));
            assert_eq!(callbacks.get(), 1, "failed reads must not be refunded");
        })
        .unwrap();
        assert_eq!(root.work_used_current(view).unwrap(), 48);
    });
}

#[test]
fn tlcx22_composite_32_failed_payload_reads_exhaust_budget_with_zero_callbacks() {
    fixture(NORMAL, 2 * 1024 * 1024, |fx, cases, view, _| {
        actual_pair!(fx,cases,view;root,program,branch,gain,coordinate,payload);
        assert_eq!(branch.hard_current(view).unwrap(), c::HardState::Pass);
        assert_eq!(
            bounds(&gain.report_current(&root, &program, view).unwrap().state),
            &ratio(1, 2)
        );
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
            let wrong_forward = fx.forward(true);
            assert_ne!(wrong_forward.as_slice(), payload.forward);
            let wrong = join::BoolTarget {
                forward: &wrong_forward,
                inverse: payload.inverse,
            };
            let callbacks = Cell::new(0usize);
            // This cut occurs inside the real native visitor, after scratch
            // funding. Dropping those claims on failure must not refund reads.
            for _ in 0..32 {
                let rejected = token.with_current(&coordinate, wrong, owners, view, |_| {
                    callbacks.set(callbacks.get() + 1);
                });
                assert!(matches!(
                    rejected,
                    Err(join::Error::CoordinatePayloadMismatch)
                ));
            }
            assert_eq!(callbacks.get(), 0);
            // Restore the exact correct payload on this same token, not a fresh
            // token with a reset account. Exhaustion must take precedence now.
            let exhausted = token.with_current(&coordinate, payload, owners, view, |_| {
                callbacks.set(callbacks.get() + 1);
            });
            assert!(matches!(exhausted, Err(join::Error::ReadBudget)));
            assert_eq!(
                callbacks.get(),
                0,
                "no rejected read may invoke the consumer"
            );
        })
        .unwrap();
        assert_eq!(root.work_used_current(view).unwrap(), 48);
    });
}
