//! 日本語: 実Not bankでRefutedを確認し、定義済み三成分からだけT_nc=0を得る。
//! English: Check Refuted with the real Not bank and derive zero T_nc from three defined leaves.
#[path = "tlcx22_framework_p3_support/mod.rs"]
mod support;
use support::*;

fn query_counts(
    current: &s::Current<'_>,
    side: s::Side,
    role: f::QueryRole,
    status: s::CanonicalStatus,
    counts: (usize, usize, usize),
) {
    let query = current.query(side, role).unwrap();
    assert_eq!(query.status(), status);
    assert_eq!(
        (query.feasible_models(), query.forced_models(), query.negation_forced_models()),
        counts
    );
}

#[test]
fn native_negation_refutes_optional_not_p_with_a_defined_zero_tnc() {
    // 日本語: 最初に元のq/LEM/goalを実行し、その後PlacementだけをNot(p)へ変更する。
    // English: Execute the original q/LEM/goal control, then change only Placement to Not(p).
    for refuted in [false, true] {
        framework_fixture!(h, 2, false, Duration::from_secs(60), {
            let (h, view, _, _) = h;
            let one = q(1, 1);
            let not_p = BoolExpr::Not(Box::new(var(p(&h))));
            let mut probes = h.probes.to_vec();
            if refuted {
                probes[1].formula = &not_p;
            }
            assert_eq!(probes[1].role, f::QueryRole::Placement);
            assert_eq!(probes[1].duty, f::Duty::Optional);
            let mut declaration = h.declaration();
            declaration.probes = &probes;
            let semantic = [s::SemanticProbe {
                role: f::QueryRole::Placement,
                formula: probes[1].formula,
                weight: &one,
            }];
            let bridge_probes = bridges(&one);
            let semantic_input = semantic_registration(&semantic, &bridge_probes);
            let novelty_registration = registration(h.constructions, m::ArchiveInput::Unread);
            let framework = f::FrameworkRoot::preregister(h.native(view), &declaration, view).unwrap();
            let novelty = m::Root::preregister(framework, &novelty_registration, view).unwrap();
            let mut root = s::Root::preregister(novelty, &semantic_input, view).unwrap();
            let generated = root.generate_current(operator(), view).unwrap();
            p2_pair!(root, generated, view, source, source_expected, target, target_expected);
            let expected = semantic_pair(&root, &source, source_expected, &target, target_expected, view);
            let measured_owner = root.measure_current(expected, view).unwrap();
            measured_owner.with_current(expected, view, |current| {
                assert_eq!(current.status(), s::Status::Complete);
                assert_eq!(current.counts().native_not_dispatches, 39);
                assert_eq!(current.counts().native_not_store_entries, 27);
                query_counts(&current, s::Side::Source, f::QueryRole::GoalEcho,
                    s::CanonicalStatus::Supported, (1, 1, 0));
                query_counts(&current, s::Side::Target, f::QueryRole::GoalEcho,
                    s::CanonicalStatus::Supported, (3, 3, 0));
                query_counts(&current, s::Side::Source, f::QueryRole::BridgeLaw,
                    s::CanonicalStatus::Supported, (1, 1, 0));
                query_counts(&current, s::Side::Target, f::QueryRole::BridgeLaw,
                    s::CanonicalStatus::Undetermined, (3, 2, 0));
                let lem = current.query(s::Side::Target, f::QueryRole::BridgeLaw).unwrap();
                for (i, row) in lem.rows().iter().filter(|row| row.admitted()).enumerate() {
                    assert_eq!(row.query_bits(), LEM_BITS[i]);
                    assert_eq!(row.negation_bits(), NOT_LEM_BITS[i]);
                }
                assert_eq!(current.bridge_results().iter().map(|bridge| bridge.passed())
                    .collect::<Vec<_>>(), [true, false]);
                assert_eq!(current.semantic_denominator(), &q(1, 1));
                assert_eq!(current.bridge_denominator(), &q(2, 1));
                measured(&current, ComponentMetric::FrameworkDistance, 1, 4);
                measured(&current, ComponentMetric::Compatibility, 1, 2);
                if refuted {
                    query_counts(&current, s::Side::Source, f::QueryRole::Placement,
                        s::CanonicalStatus::Refuted, (1, 0, 1));
                    query_counts(&current, s::Side::Target, f::QueryRole::Placement,
                        s::CanonicalStatus::Refuted, (3, 0, 3));
                    for (side, double_negation_bits) in [(s::Side::Source, 1), (s::Side::Target, 3)] {
                        let placement = current.query(side, f::QueryRole::Placement).unwrap();
                        assert!(placement.rows().iter().filter(|row| row.admitted())
                            .all(|row| row.query_bits() == 0 && row.negation_bits() == double_negation_bits));
                    }
                    measured(&current, ComponentMetric::SemanticConfigurationDistance, 0, 1);
                    measured(&current, ComponentMetric::NoveltyCompatibility, 0, 1);
                    assert_eq!(current.tnc().unwrap(), &RationalInterval::exact_ratio(0, 1).unwrap());
                } else {
                    query_counts(&current, s::Side::Source, f::QueryRole::Placement,
                        s::CanonicalStatus::Supported, (1, 1, 0));
                    query_counts(&current, s::Side::Target, f::QueryRole::Placement,
                        s::CanonicalStatus::Undetermined, (3, 1, 1));
                    let placement = current.query(s::Side::Target, f::QueryRole::Placement).unwrap();
                    for (i, row) in placement.rows().iter().filter(|row| row.admitted()).enumerate() {
                        assert_eq!(row.query_bits(), Q_BITS[i]);
                        assert_eq!(row.negation_bits(), NOT_Q_BITS[i]);
                    }
                    measured(&current, ComponentMetric::SemanticConfigurationDistance, 1, 1);
                    measured(&current, ComponentMetric::NoveltyCompatibility, 1, 4);
                }
                no_full_f2(&current);
            }).unwrap();
        });
    }
}
