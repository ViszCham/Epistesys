//! 日本語: 同じrole/weightの登録順序は、同じrootの有限数値admissionを変えない。
//! English: Reordering the same role weights preserves numeric admission under one root identity.
#[path = "tlcx22_framework_p3_support/mod.rs"]
mod support;
use support::*;

fn assert_incomparable_total<'a, 'v>(
    root: &mut s::Root<'a, 'v>,
    view: &'v ResourceReservationView<'_>,
) {
    let generated = root
        .generate_current(operator(), view)
        .expect("actual candidate generation");
    p2_pair!(
        root,
        generated,
        view,
        source,
        source_expected,
        target,
        target_expected
    );
    let expected = semantic_pair(
        root,
        &source,
        source_expected,
        &target,
        target_expected,
        view,
    );
    let owner = root
        .measure_current(expected, view)
        .expect("canonical role totals fit the five-bit cap before unavailable mappings");
    owner
        .with_current(expected, view, |current| {
            assert_eq!(current.status(), s::Status::NoComparable);
            assert_eq!(current.semantic_denominator(), &q(6, 5));
            assert_eq!(current.bridge_denominator(), &q(2, 1));
            assert_eq!(current.counts().native_not_dispatches, 39);
            assert_eq!(current.counts().native_not_store_entries, 27);
            measured(&current, ComponentMetric::FrameworkDistance, 1, 4);
            for metric in [
                ComponentMetric::SemanticConfigurationDistance,
                ComponentMetric::Compatibility,
                ComponentMetric::NoveltyCompatibility,
            ] {
                assert!(matches!(
                    current.report(metric).unwrap().state,
                    ComponentState::Incomparable { .. }
                ));
            }
            assert!(matches!(current.tnc(), Err(s::Error::NoScalar)));
            no_full_f2(&current);
        })
        .unwrap();
}

#[test]
fn role_order_preserves_one_numeric_admission_and_exact_denominator() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let half = q(1, 2);
        let fifth = q(1, 5);
        let one = q(1, 1);
        let canonical = [
            s::SemanticProbe {
                role: f::QueryRole::GoalEcho,
                formula: h.probes[0].formula,
                weight: &half,
            },
            s::SemanticProbe {
                role: f::QueryRole::Placement,
                formula: h.probes[1].formula,
                weight: &half,
            },
            s::SemanticProbe {
                role: f::QueryRole::BridgeLaw,
                formula: h.probes[2].formula,
                weight: &fifth,
            },
        ];
        let permuted = [canonical[0], canonical[2], canonical[1]];
        let bridge_probes = bridges(&one);
        let mut canonical_input = semantic_registration(&canonical, &bridge_probes);
        canonical_input.limits.max_intermediate_bits = 5;
        canonical_input.status_map = None;
        canonical_input.bridge_map = None;
        let mut permuted_input = canonical_input;
        permuted_input.semantic = &permuted;
        let declaration = h.declaration();
        let novelty_input = registration(h.constructions, m::ArchiveInput::Unread);
        let canonical_framework = f::FrameworkRoot::preregister(h.native(view), &declaration, view)
            .expect("canonical P1 setup");
        let canonical_novelty = m::Root::preregister(canonical_framework, &novelty_input, view)
            .expect("canonical P2 setup");
        let mut canonical_root = s::Root::preregister(canonical_novelty, &canonical_input, view)
            .expect("canonical P3 setup");
        let permuted_framework = f::FrameworkRoot::preregister(h.native(view), &declaration, view)
            .expect("permuted P1 setup");
        let permuted_novelty = m::Root::preregister(permuted_framework, &novelty_input, view)
            .expect("permuted P2 setup");
        let mut permuted_root = s::Root::preregister(permuted_novelty, &permuted_input, view)
            .expect("permuted P3 setup");
        assert_eq!(
            canonical_root.historical_digest(),
            permuted_root.historical_digest()
        );
        assert_incomparable_total(&mut canonical_root, view);
        assert_incomparable_total(&mut permuted_root, view);
    });
}
