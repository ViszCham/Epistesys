use facet_ledger::*;
use lc631_runtime::facet_ledger;

fn rev(n: u8) -> Revision {
    Revision([n; 32])
}
fn scope() -> ComparisonScope {
    ComparisonScope {
        id: ScopeId([1; 32]),
        baseline: rev(2),
        root_goal: rev(3),
        probes: rev(4),
        policy: rev(5),
    }
}
fn binding(n: u8) -> TransformBinding {
    TransformBinding {
        id: TransformId([n; 32]),
        source: rev(n),
        target: rev(n + 1),
        source_context: rev(20),
        target_context: rev(20),
        direction: Direction::Forward,
        scope: scope(),
        producer: rev(30),
        query: rev(32),
        result: rev(31),
    }
}
fn defect(n: u8) -> Entry {
    Entry {
        transform: binding(1).id,
        facet: Facet::Defect(Defect {
            key: DefectKey {
                root: ObligationId([n; 32]),
                property: PropertyId([9; 32]),
                source: rev(1),
                target: rev(2),
            },
            axis: LossAxis::Semantic,
            witness: EvidenceRef([8; 32]),
        }),
    }
}
fn gain(n: u8) -> Entry {
    Entry {
        transform: binding(1).id,
        facet: Facet::Gain(Gain {
            primitive: PrimitiveId([n; 32]),
            metric: 0x0114,
            definition: rev(10),
            unit: rev(11),
            baseline_observation: EvidenceRef([12; 32]),
            candidate_observation: EvidenceRef([13; 32]),
            kind: GainKind::ObservedReported,
        }),
    }
}

#[test]
fn tlcx17_duplicate_defect_and_gain_do_not_mutate_history() {
    let mut ledger = FacetLedger::new(2, 4).unwrap();
    ledger.admit_transform(binding(1)).unwrap();
    for event in [defect(1), gain(1)] {
        ledger.append(event).unwrap();
        assert_eq!(ledger.append(event), Err(LedgerError::DuplicateEvent));
    }
    assert_eq!(ledger.entries().len(), 2);
}

#[test]
fn tlcx17_defect_admission_matches_independent_finite_set_model() {
    // Exhaust every length-four word over three obligations. The oracle is a
    // fixed boolean membership table, without production keys/comparison code.
    for word in 0..81 {
        let mut ledger = FacetLedger::new(1, 4).unwrap();
        ledger.admit_transform(binding(1)).unwrap();
        let mut oracle = [false; 3];
        let mut remaining = word;
        for _ in 0..4 {
            let symbol = remaining % 3;
            remaining /= 3;
            let expected = if oracle[symbol] {
                Err(LedgerError::DuplicateEvent)
            } else {
                Ok(())
            };
            assert_eq!(ledger.append(defect(symbol as u8)), expected);
            oracle[symbol] = true;
        }
        assert_eq!(
            ledger.entries().len(),
            oracle.into_iter().filter(|x| *x).count()
        );
    }
}

#[test]
fn tlcx17_path_requires_scope_direction_and_context_correspondence() {
    let mut ledger = FacetLedger::new(6, 1).unwrap();
    ledger.admit_transform(binding(1)).unwrap();
    ledger.admit_transform(binding(2)).unwrap();
    assert_eq!(
        ledger.check_path(&[binding(1).id, binding(2).id]),
        Ok(scope())
    );
    assert_eq!(
        ledger.check_path(&[binding(2).id, binding(1).id]),
        Err(LedgerError::BrokenPath)
    );
    let mut foreign = binding(3);
    foreign.scope.probes = rev(99);
    ledger.admit_transform(foreign).unwrap();
    assert_eq!(
        ledger.check_path(&[binding(2).id, foreign.id]),
        Err(LedgerError::ScopeMismatch)
    );
    assert_eq!(ledger.check_path(&[]), Err(LedgerError::EmptyPath));
}

#[test]
fn tlcx17_delta_does_not_excuse_defect_and_unknown_stays_explicit() {
    let mut ledger = FacetLedger::new(1, 3).unwrap();
    ledger.admit_transform(binding(1)).unwrap();
    ledger.append(defect(1)).unwrap();
    ledger
        .append(Entry {
            transform: binding(1).id,
            facet: Facet::Delta(Delta {
                property: PropertyId([9; 32]),
                before: rev(1),
                after: rev(2),
                kind: EditKind::RetractPremise,
                reason: EvidenceRef([1; 32]),
                permission_basis: EvidenceRef([2; 32]),
                map_back: EvidenceRef([3; 32]),
            }),
        })
        .unwrap();
    ledger
        .append(Entry {
            transform: binding(1).id,
            facet: Facet::Uncertainty(Gap {
                property: PropertyId([9; 32]),
                reason: GapReason::Unverified,
                required_evidence: rev(40),
                next_action: rev(41),
            }),
        })
        .unwrap();
    assert!(matches!(ledger.entries()[0].facet, Facet::Defect(_)));
    assert!(matches!(
        ledger.entries()[2].facet,
        Facet::Uncertainty(Gap {
            reason: GapReason::Unverified,
            ..
        })
    ));
    assert_eq!(ledger.for_transform(binding(1).id).count(), 3);
}

#[test]
fn tlcx17_mixed_units_revisions_and_capacity_fail_without_append() {
    let mut ledger = FacetLedger::new(1, 1).unwrap();
    ledger.admit_transform(binding(1)).unwrap();
    let mut other = gain(2);
    if let Facet::Gain(ref mut value) = other.facet {
        value.unit = rev(99);
    }
    assert_eq!(
        ledger.comparable_gains(gain(1), other),
        Err(LedgerError::IncomparableMeasure)
    );
    let mut wrong = defect(1);
    if let Facet::Defect(ref mut value) = wrong.facet {
        value.key.target = rev(99);
    }
    assert_eq!(ledger.append(wrong), Err(LedgerError::RevisionMismatch));
    ledger.append(gain(1)).unwrap();
    assert_eq!(ledger.append(defect(1)), Err(LedgerError::Capacity));
    assert_eq!(ledger.entries().len(), 1);
}

#[test]
fn tlcx17_target_only_gain_cannot_be_promoted_by_store() {
    // No constructor or store method produces a Verified/Eligible/Authority
    // state; even a caller-supplied observed tag remains ObservedReported.
    let mut ledger = FacetLedger::new(1, 1).unwrap();
    ledger.admit_transform(binding(1)).unwrap();
    ledger.append(gain(1)).unwrap();
    assert!(matches!(
        ledger.entries()[0].facet,
        Facet::Gain(Gain {
            kind: GainKind::ObservedReported,
            ..
        })
    ));
}

#[test]
fn tlcx17_native_observation_binding_swap_is_rejected_without_truth_promotion() {
    let mut ledger = FacetLedger::new(1, 1).unwrap();
    ledger.admit_transform(binding(1)).unwrap();
    let valid = FiniteObservationBinding {
        source: rev(1),
        context: rev(20),
        query: rev(32),
        result: rev(31),
    };
    for axis in 0..4 {
        let mut wrong = valid;
        match axis {
            0 => wrong.source = rev(99),
            1 => wrong.context = rev(99),
            2 => wrong.query = rev(99),
            3 => wrong.result = rev(99),
            _ => unreachable!(),
        }
        assert_eq!(
            ledger.retain_finite_observation(
                binding(1).id,
                wrong,
                PropertyId([1; 32]),
                rev(40),
                rev(41)
            ),
            Err(LedgerError::RevisionMismatch)
        );
        assert!(ledger.entries().is_empty());
    }
    ledger
        .retain_finite_observation(binding(1).id, valid, PropertyId([1; 32]), rev(40), rev(41))
        .unwrap();
    assert!(matches!(
        ledger.entries()[0].facet,
        Facet::Uncertainty(Gap {
            reason: GapReason::Unverified,
            ..
        })
    ));
}

#[test]
fn tlcx17_cross_transform_logical_views_deduplicate_but_retain_raw_witnesses() {
    let mut ledger = FacetLedger::new(2, 4).unwrap();
    let first = binding(1);
    let mut second = first;
    second.id = TransformId([99; 32]);
    ledger.admit_transform(first).unwrap();
    ledger.admit_transform(second).unwrap();
    for transform in [first.id, second.id] {
        let mut d = defect(1);
        d.transform = transform;
        ledger.append(d).unwrap();
        let mut g = gain(1);
        g.transform = transform;
        if transform == second.id {
            if let Facet::Gain(ref mut value) = g.facet {
                value.candidate_observation = EvidenceRef([99; 32]);
            }
        }
        ledger.append(g).unwrap();
    }
    // Independent expected sets: one obligation/property pair, one bridge
    // primitive, despite two path/observation aliases. All four witnesses remain.
    assert_eq!(ledger.entries().len(), 4);
    assert_eq!(ledger.unique_defects(scope()).count(), 1);
    assert_eq!(ledger.unique_gains(scope()).count(), 1);
}
