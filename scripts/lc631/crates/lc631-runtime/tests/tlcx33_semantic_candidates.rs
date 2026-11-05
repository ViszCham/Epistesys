// Exercise the registered public library, not a second copy of the source.
use lc631_runtime::semantic_accelerator;
use semantic_accelerator::*;

fn pins() -> BatchPins {
    BatchPins {
        source: [1; 32],
        scope: [2; 32],
        policy: [3; 32],
        axis_contract: [4; 32],
        observations: [5; 32],
        epoch: 1,
        generation: 2,
        fence: 3,
    }
}
fn point(value: i64) -> Bounds {
    Bounds::checked(value, value).unwrap()
}

#[test]
fn tlcx33_global_frontier_rejects_ring_only_candidate() {
    let values = [
        [point(3), point(3)],
        [point(9), point(0)],
        [point(1), point(1)],
        [point(0), point(9)],
    ];
    let rows: Vec<_> = values
        .iter()
        .enumerate()
        .map(|(i, values)| ReportedRow {
            key: [i as u8; 32],
            state: RowState::ReportedNumeric,
            values,
        })
        .collect();
    let directions = [AxisDirection::Minimize; 2];
    let batch = ReportedBatch::checked(pins(), &directions, &rows, 32).unwrap();
    // Row 0 is dominated by non-neighbor row 2, not either ring neighbor.
    assert_eq!(cpu_reference(&batch).unwrap(), [false, true, true, true]);
    let keys: Vec<_> = rows.iter().map(|r| r.key).collect();
    assert!(matches!(
        check_candidate(
            &batch,
            ReportedFrontier {
                pins: pins(),
                row_keys: &keys,
                retained: &[true; 4],
            }
        ),
        Err(CandidateError::VectorMismatch { row: 0 })
    ));
    let checked = check_candidate(
        &batch,
        ReportedFrontier {
            pins: pins(),
            row_keys: &keys,
            retained: &[false, true, true, true],
        },
    )
    .unwrap();
    assert_eq!(checked.retained(), [false, true, true, true]);
    assert_eq!(checked.pins(), batch.pins());
}

#[test]
fn tlcx33_interval_overlap_and_axis_directions_are_preserved() {
    let values = [
        [Bounds::checked(0, 2).unwrap(), point(7)],
        [Bounds::checked(1, 3).unwrap(), point(7)],
        [point(5), point(6)],
    ];
    let rows: Vec<_> = values
        .iter()
        .enumerate()
        .map(|(i, values)| ReportedRow {
            key: [i as u8; 32],
            state: RowState::ReportedNumeric,
            values,
        })
        .collect();
    let directions = [AxisDirection::Minimize, AxisDirection::Maximize];
    let batch = ReportedBatch::checked(pins(), &directions, &rows, 18).unwrap();
    assert_eq!(cpu_reference(&batch).unwrap(), [true, true, false]);
    assert_eq!(Bounds::checked(2, 1), Err(CandidateError::InvalidBounds));
}

#[test]
fn tlcx33_non_numeric_states_do_not_become_small_loss() {
    for state in [
        RowState::Unknown,
        RowState::NotApplicable,
        RowState::Incomparable,
    ] {
        let values = [point(0)];
        let rows = [ReportedRow {
            key: [1; 32],
            state,
            values: &values,
        }];
        assert!(
            matches!(ReportedBatch::checked(pins(), &[AxisDirection::Minimize], &rows, 1),
            Err(CandidateError::NeedsEvidence { row: 0, state: s }) if s == state)
        );
    }
}

#[test]
fn tlcx33_stale_partial_identity_and_work_cuts_remain_errors() {
    let values = [point(0)];
    let rows = [ReportedRow {
        key: [1; 32],
        state: RowState::ReportedNumeric,
        values: &values,
    }];
    let dirs = [AxisDirection::Minimize];
    assert!(matches!(
        ReportedBatch::checked(pins(), &dirs, &rows, 0),
        Err(CandidateError::WorkLimit)
    ));
    assert!(matches!(
        ReportedBatch::checked(pins(), &dirs, &[], 1),
        Err(CandidateError::Empty)
    ));
    let batch = ReportedBatch::checked(pins(), &dirs, &rows, 1).unwrap();
    let mut stale = pins();
    stale.fence += 1;
    assert!(matches!(
        check_candidate(
            &batch,
            ReportedFrontier {
                pins: stale,
                row_keys: &[[1; 32]],
                retained: &[true]
            }
        ),
        Err(CandidateError::StalePins)
    ));
    assert!(matches!(
        check_candidate(
            &batch,
            ReportedFrontier {
                pins: pins(),
                row_keys: &[],
                retained: &[]
            }
        ),
        Err(CandidateError::PartialVector)
    ));
    assert!(matches!(
        check_candidate(
            &batch,
            ReportedFrontier {
                pins: pins(),
                row_keys: &[[2; 32]],
                retained: &[true]
            }
        ),
        Err(CandidateError::RowIdentityMismatch)
    ));
}
