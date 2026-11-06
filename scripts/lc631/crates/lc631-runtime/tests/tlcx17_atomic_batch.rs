//! 日本語: 非空の既存履歴を含む batch 全体の原子性。実行は中央検証で記録する。
//! English: Whole-batch atomicity with existing nonempty history; execution is recorded centrally.
use lc631_runtime::facet_ledger::*;

fn rev(n: u8) -> Revision {
    Revision([n; 32])
}

fn binding(n: u8) -> TransformBinding {
    TransformBinding {
        id: TransformId([n; 32]),
        source: rev(n),
        target: rev(n + 1),
        source_context: rev(20),
        target_context: rev(21),
        direction: Direction::Forward,
        scope: ComparisonScope {
            id: ScopeId([30; 32]),
            baseline: rev(31),
            root_goal: rev(32),
            probes: rev(33),
            policy: rev(34),
        },
        producer: rev(40),
        query: rev(41),
        result: rev(42),
    }
}

fn gap(transform: TransformBinding, property: u8) -> Entry {
    Entry {
        transform: transform.id,
        facet: Facet::Uncertainty(Gap {
            property: PropertyId([property; 32]),
            reason: GapReason::Unverified,
            required_evidence: rev(50),
            next_action: rev(51),
        }),
    }
}

fn defect(transform: TransformBinding, property: u8) -> Entry {
    Entry {
        transform: transform.id,
        facet: Facet::Defect(Defect {
            key: DefectKey {
                root: ObligationId([property; 32]),
                property: PropertyId([property; 32]),
                source: transform.source,
                target: transform.target,
            },
            axis: LossAxis::Semantic,
            witness: EvidenceRef([60; 32]),
        }),
    }
}

fn seeded(max_transforms: usize, max_entries: usize) -> FacetLedger {
    let mut ledger = FacetLedger::new(max_transforms, max_entries).unwrap();
    ledger.admit_transform(binding(1)).unwrap();
    ledger.append(gap(binding(1), 10)).unwrap();
    ledger
}

fn assert_sentinel_unchanged(ledger: &FacetLedger) {
    // Compare every typed field, including all digest bytes and evidence refs.
    // Native struct bytes include padding and are not a canonical wire format.
    assert_eq!(ledger.transforms(), &[binding(1)]);
    assert_eq!(ledger.entries(), &[gap(binding(1), 10)]);
}

#[test]
fn tlcx17_atomic_batch_late_unknown_transform_and_defect_revision_leave_history_intact() {
    let first = gap(binding(2), 20);
    let unknown = gap(binding(99), 21);
    let mut wrong_source = defect(binding(2), 22);
    if let Facet::Defect(ref mut value) = wrong_source.facet {
        value.key.source = rev(98);
    }
    let mut wrong_target = defect(binding(2), 23);
    if let Facet::Defect(ref mut value) = wrong_target.facet {
        value.key.target = rev(97);
    }
    for (late, expected) in [
        (unknown, LedgerError::UnknownTransform),
        (wrong_source, LedgerError::RevisionMismatch),
        (wrong_target, LedgerError::RevisionMismatch),
    ] {
        let mut ledger = seeded(3, 4);
        assert_eq!(
            ledger.admit_batch(&[binding(2)], &[first, late]),
            Err(expected)
        );
        assert_sentinel_unchanged(&ledger);
    }
}

#[test]
fn tlcx17_atomic_batch_intra_and_existing_duplicates_do_not_publish_valid_prefixes() {
    for transforms in [
        [binding(2), binding(3), binding(2)],
        [binding(2), binding(3), binding(1)],
    ] {
        let mut ledger = seeded(5, 8);
        assert_eq!(
            ledger.admit_batch(&transforms, &[gap(binding(2), 20)]),
            Err(LedgerError::DuplicateTransform)
        );
        assert_sentinel_unchanged(&ledger);
    }

    let first = gap(binding(2), 20);
    let mut duplicate_in_batch = first;
    if let Facet::Uncertainty(ref mut value) = duplicate_in_batch.facet {
        // Evidence-label changes do not create a new property/reason event.
        value.required_evidence = rev(90);
        value.next_action = rev(91);
    }
    let mut duplicate_existing = gap(binding(1), 10);
    if let Facet::Uncertainty(ref mut value) = duplicate_existing.facet {
        value.required_evidence = rev(92);
    }
    for late in [duplicate_in_batch, duplicate_existing] {
        let mut ledger = seeded(5, 8);
        assert_eq!(
            ledger.admit_batch(&[binding(2)], &[first, late]),
            Err(LedgerError::DuplicateEvent)
        );
        assert_sentinel_unchanged(&ledger);
    }
}

#[test]
fn tlcx17_atomic_batch_transform_and_event_capacity_reject_without_partial_append() {
    let mut transform_full = seeded(2, 4);
    assert_eq!(
        transform_full.admit_batch(&[binding(2), binding(3)], &[gap(binding(2), 20)]),
        Err(LedgerError::Capacity)
    );
    assert_sentinel_unchanged(&transform_full);

    let mut event_full = seeded(3, 2);
    assert_eq!(
        event_full.admit_batch(&[binding(2)], &[gap(binding(2), 20), gap(binding(1), 21)]),
        Err(LedgerError::Capacity)
    );
    assert_sentinel_unchanged(&event_full);
}

#[test]
fn tlcx17_atomic_batch_exact_caps_accept_mixed_old_new_facets_and_empty_is_explicit_noop() {
    let mut ledger = seeded(2, 4);
    ledger.admit_batch(&[], &[]).unwrap();
    assert_sentinel_unchanged(&ledger);

    let delta = Entry {
        transform: binding(1).id,
        facet: Facet::Delta(Delta {
            property: PropertyId([21; 32]),
            before: rev(20),
            after: rev(21),
            kind: EditKind::AddPremise,
            reason: EvidenceRef([70; 32]),
            permission_basis: EvidenceRef([71; 32]),
            map_back: EvidenceRef([72; 32]),
        }),
    };
    let gain = Entry {
        transform: binding(2).id,
        facet: Facet::Gain(Gain {
            primitive: PrimitiveId([22; 32]),
            metric: 0x0114,
            definition: rev(80),
            unit: rev(81),
            baseline_observation: EvidenceRef([82; 32]),
            candidate_observation: EvidenceRef([83; 32]),
            kind: GainKind::ObservedReported,
        }),
    };
    let added = [defect(binding(2), 20), delta, gain];
    ledger.admit_batch(&[binding(2)], &added).unwrap();
    let expected_transforms = [binding(1), binding(2)];
    let expected_entries = [gap(binding(1), 10), added[0], added[1], added[2]];
    assert_eq!(ledger.transforms(), &expected_transforms);
    assert_eq!(ledger.entries(), &expected_entries);

    // A full ledger still accepts an explicit no-op without touching history.
    ledger.admit_batch(&[], &[]).unwrap();
    assert_eq!(ledger.transforms(), &expected_transforms);
    assert_eq!(ledger.entries(), &expected_entries);
}
