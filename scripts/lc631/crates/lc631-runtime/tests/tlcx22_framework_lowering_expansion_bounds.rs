//! New TEST-FIRST packet: source occurrences and expanded native nodes differ.
//! Existing P1 bodies/15 tests/helpers remain unchanged; central runs all gates.
//! Missing framework construction is NOT evidence of these runtime assertions.
use lc631_runtime::context_novelty_measurement::framework as f;
#[path = "tlcx22_framework_p1_support/mod.rs"]
mod frozen;
use frozen::*;
mod tlcx22_framework_lowering_expansion_bounds_support;
use tlcx22_framework_lowering_expansion_bounds_support::{bank, placement, Sizes};

fn rejects_expansion(children: usize, top_tail: bool, literal: Sizes) {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let expanded = placement(children, top_tail);
        let probes = [
            h.probes[0],
            f::Probe {
                role: f::QueryRole::Placement,
                formula: &expanded,
                duty: f::Duty::Optional,
            },
            h.probes[2],
        ];
        assert_eq!(bank(&h, &probes), literal);
        assert!(
            literal.source < 128,
            "not a source-occurrence cap rejection"
        );
        // Original theory/facts/constructions/hard/goal/maps are unmodified.
        // The replacement Placement is q by idempotence (and q AND Top).
        let mut declaration = h.declaration();
        declaration.probes = &probes;
        declaration.definition_revision = d(85);
        assert_eq!(declaration.limits.max_lowered_nodes, 128);
        let admission = f::FrameworkRoot::preregister(h.native(view), &declaration, view);
        let result = match admission {
            Err(error) => Err(error),
            Ok(root) => {
                let expected = root
                    .expected_inputs_current(
                        f::Subject::Original,
                        f::Kernel::PersistentChain2V1,
                        view,
                    )
                    .expect("actual original owner admission");
                root.execute_case_current(expected, view).map(drop)
            }
        };
        // Either preregistration or the real case boundary may preflight it.
        // No callback/Modal report fabrication is used as a rejection oracle.
        assert!(
            matches!(result, Err(f::Error::LoweringBudget)),
            "native expansion must reject before dispatch: literal={literal:?}, actual={result:?}"
        );
    });
}

#[test]
fn tlcx22_framework_nary_bank_129_rejects_with_only_72_source_occurrences() {
    // Other six roots:11 nodes. Placement has58*q + Not Bottom:
    // source=1+58+2=61; native=58+2+58=118. Bank72/129.
    rejects_expansion(
        59,
        true,
        Sizes {
            source: 72,
            native: 129,
            largest_native: 118,
        },
    );
}

#[test]
fn tlcx22_framework_single_native_dag_129_rejects_with_77_source_occurrences() {
    // Placement65*q: source66, native65+64=129. Whole bank77/140.
    // This also rejects if the profile cap is interpreted per native DAG.
    rejects_expansion(
        65,
        false,
        Sizes {
            source: 77,
            native: 140,
            largest_native: 129,
        },
    );
}

#[test]
fn tlcx22_framework_nary_bank_128_funded_control_retains_all_seven_roles() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        let expanded = placement(59, false);
        let probes = [
            h.probes[0],
            f::Probe {
                role: f::QueryRole::Placement,
                formula: &expanded,
                duty: f::Duty::Optional,
            },
            h.probes[2],
        ];
        // Other six roots11 + Placement(source60/native117) =71/128.
        assert_eq!(
            bank(&h, &probes),
            Sizes {
                source: 71,
                native: 128,
                largest_native: 117
            }
        );
        let mut declaration = h.declaration();
        declaration.probes = &probes;
        declaration.definition_revision = d(85);
        let root = f::FrameworkRoot::preregister(h.native(view), &declaration, view)
            .expect("fully funded boundary control");
        let expected = root
            .expected_inputs_current(f::Subject::Original, f::Kernel::PersistentChain2V1, view)
            .unwrap();
        let owner = root.execute_case_current(expected, view).unwrap();
        let callbacks = Cell::new(0);
        owner
            .with_current(expected, view, |current| {
                callbacks.set(callbacks.get() + 1);
                assert_eq!(current.lowering_source_roots().len(), 7);
                assert_eq!(current.rows().len(), 9);
                assert_eq!(current.counts().lowered_source_nodes, 71);
                assert_eq!(current.counts().unmapped_source_nodes, 0);
                assert_eq!(current.counts().native_dispatches, 63);
                assert_eq!(current.counts().native_store_entries, 63);
                assert!(std::ptr::eq(current.lowering_source_roots()[5], &expanded));
                for row in current.rows() {
                    assert_eq!(
                        row.query_bits()[1],
                        CHAIN_Q[usize::from(row.valuation_states()[1])]
                    );
                }
            })
            .unwrap();
        assert_eq!(
            callbacks.get(),
            1,
            "the funded current control must actually consume its view"
        );
    });
}

#[test]
fn tlcx22_framework_original_seven_root_control_is_unmodified() {
    framework_fixture!(h, 2, false, Duration::from_secs(60), {
        let (h, view, _, _) = h;
        assert_eq!(
            bank(&h, h.probes),
            Sizes {
                source: 12,
                native: 12,
                largest_native: 4
            }
        );
        let declaration = h.declaration();
        let root = f::FrameworkRoot::preregister(h.native(view), &declaration, view).unwrap();
        let expected = root
            .expected_inputs_current(f::Subject::Original, f::Kernel::PersistentChain2V1, view)
            .unwrap();
        let owner = root.execute_case_current(expected, view).unwrap();
        let callbacks = Cell::new(0);
        owner
            .with_current(expected, view, |current| {
                callbacks.set(callbacks.get() + 1);
                assert_eq!(current.lowering_source_roots().len(), 7);
                assert_eq!(current.counts().lowered_source_nodes, 12);
                assert_eq!(current.counts().native_dispatches, 63);
                assert_eq!(current.counts().native_store_entries, 63);
                let admitted: Vec<_> = current
                    .rows()
                    .iter()
                    .enumerate()
                    .filter_map(|(i, row)| row.admitted().then_some(i))
                    .collect();
                assert_eq!(admitted, [8]);
            })
            .unwrap();
        assert_eq!(
            callbacks.get(),
            1,
            "the original current control must actually consume its view"
        );
    });
}
