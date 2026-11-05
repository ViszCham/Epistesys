//! Numeric candidate checks; `selection` consumes actual typed measurements.
//! No device execution, metric extraction or evidence authentication.
#![forbid(unsafe_code)]

/// Exact rational measurement/selection consumer; distinct from legacy x100 views.
pub mod selection;

/// Compile-time capability metadata only. It neither initializes a device nor
/// observes a backend, dispatch, completion, correctness or authority.
pub const fn is_gpu_compiled() -> bool {
    cfg!(feature = "semantic-gpu")
}

pub const MAX_ROWS: usize = 2_048;
pub const MAX_AXES: usize = 32;
pub const MAX_COMPARISONS: usize = 8_388_608;

/// Identity references only; equality does not authenticate the referred payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchPins {
    pub source: [u8; 32],
    pub scope: [u8; 32],
    pub policy: [u8; 32],
    /// Binds axis order, definitions, units, fixed scales and directions.
    pub axis_contract: [u8; 32],
    /// Binds the complete reported row payload and evidence lineage.
    pub observations: [u8; 32],
    pub epoch: u64,
    pub generation: u64,
    pub fence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AxisDirection {
    Minimize,
    Maximize,
}

/// Integer endpoints on an explicitly registered, axis-specific fixed grid.
/// Producers must enclose rationals outward; these are not implicit x100 scores.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bounds {
    lower: i64,
    upper: i64,
}

impl Bounds {
    pub fn checked(lower: i64, upper: i64) -> Result<Self, CandidateError> {
        if lower > upper {
            return Err(CandidateError::InvalidBounds);
        }
        Ok(Self { lower, upper })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowState {
    ReportedNumeric,
    Unknown,
    NotApplicable,
    Incomparable,
}

#[derive(Clone, Copy, Debug)]
pub struct ReportedRow<'a> {
    /// Matching identity, never a ranking key or semantic coordinate.
    pub key: [u8; 32],
    pub state: RowState,
    pub values: &'a [Bounds],
}

#[derive(Debug)]
pub struct ReportedBatch<'a> {
    pins: BatchPins,
    directions: &'a [AxisDirection],
    rows: &'a [ReportedRow<'a>],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateError {
    Empty,
    Shape,
    InvalidBounds,
    RowLimit,
    AxisLimit,
    WorkLimit,
    DuplicateRow,
    NeedsEvidence { row: usize, state: RowState },
    StalePins,
    PartialVector,
    RowIdentityMismatch,
    VectorMismatch { row: usize },
    AllocationFailed,
}

impl<'a> ReportedBatch<'a> {
    pub fn checked(
        pins: BatchPins,
        directions: &'a [AxisDirection],
        rows: &'a [ReportedRow<'a>],
        comparison_budget: usize,
    ) -> Result<Self, CandidateError> {
        if rows.is_empty() || directions.is_empty() {
            return Err(CandidateError::Empty);
        }
        if rows.len() > MAX_ROWS {
            return Err(CandidateError::RowLimit);
        }
        if directions.len() > MAX_AXES {
            return Err(CandidateError::AxisLimit);
        }
        let work = rows
            .len()
            .checked_mul(rows.len())
            .and_then(|pairs| pairs.checked_mul(directions.len()))
            .ok_or(CandidateError::WorkLimit)?;
        if comparison_budget > MAX_COMPARISONS || work > comparison_budget {
            return Err(CandidateError::WorkLimit);
        }
        for (index, row) in rows.iter().enumerate() {
            if row.values.len() != directions.len() {
                return Err(CandidateError::Shape);
            }
            if rows[..index].iter().any(|other| other.key == row.key) {
                return Err(CandidateError::DuplicateRow);
            }
            match row.state {
                RowState::ReportedNumeric => {}
                RowState::Unknown | RowState::NotApplicable | RowState::Incomparable => {
                    return Err(CandidateError::NeedsEvidence {
                        row: index,
                        state: row.state,
                    });
                }
            }
        }
        Ok(Self {
            pins,
            directions,
            rows,
        })
    }

    pub const fn pins(&self) -> BatchPins {
        self.pins
    }
}

/// A full, ordered candidate vector. Summary/finalist equality is insufficient.
pub struct ReportedFrontier<'a> {
    pub pins: BatchPins,
    pub row_keys: &'a [[u8; 32]],
    pub retained: &'a [bool],
}

/// Only numeric equivalence to this CPU oracle over this borrowed batch.
/// This is not semantic equivalence, independent provenance or permission.
pub struct CpuCheckedFrontier<'a> {
    batch: &'a ReportedBatch<'a>,
    retained: Vec<bool>,
}

impl CpuCheckedFrontier<'_> {
    pub fn retained(&self) -> &[bool] {
        &self.retained
    }
    pub fn pins(&self) -> BatchPins {
        self.batch.pins
    }
}

/// Independent scalar all-pairs oracle, not the accelerator's ring/summary path.
/// An overlapping interval is retained unless another row certainly dominates.
pub fn cpu_reference(batch: &ReportedBatch<'_>) -> Result<Vec<bool>, CandidateError> {
    let mut retained = Vec::new();
    retained
        .try_reserve_exact(batch.rows.len())
        .map_err(|_| CandidateError::AllocationFailed)?;
    for (right_index, right) in batch.rows.iter().enumerate() {
        let dominated = batch.rows.iter().enumerate().any(|(left_index, left)| {
            left_index != right_index && certainly_dominates(batch.directions, left, right)
        });
        retained.push(!dominated);
    }
    Ok(retained)
}

fn certainly_dominates(
    directions: &[AxisDirection],
    left: &ReportedRow<'_>,
    right: &ReportedRow<'_>,
) -> bool {
    let mut strict = false;
    for ((direction, a), b) in directions.iter().zip(left.values).zip(right.values) {
        let (weak, stronger) = match direction {
            AxisDirection::Minimize => (a.upper <= b.lower, a.upper < b.lower),
            AxisDirection::Maximize => (a.lower >= b.upper, a.lower > b.upper),
        };
        if !weak {
            return false;
        }
        strict |= stronger;
    }
    strict
}

/// Compare every row; never publish a partial vector or repair mismatched bits.
pub fn check_candidate<'a>(
    batch: &'a ReportedBatch<'a>,
    candidate: ReportedFrontier<'_>,
) -> Result<CpuCheckedFrontier<'a>, CandidateError> {
    if candidate.pins != batch.pins {
        return Err(CandidateError::StalePins);
    }
    if candidate.retained.len() != batch.rows.len() || candidate.row_keys.len() != batch.rows.len()
    {
        return Err(CandidateError::PartialVector);
    }
    if batch
        .rows
        .iter()
        .zip(candidate.row_keys)
        .any(|(row, key)| row.key != *key)
    {
        return Err(CandidateError::RowIdentityMismatch);
    }
    let expected = cpu_reference(batch)?;
    for (row, (actual, reference)) in candidate.retained.iter().zip(&expected).enumerate() {
        if actual != reference {
            return Err(CandidateError::VectorMismatch { row });
        }
    }
    Ok(CpuCheckedFrontier {
        batch,
        retained: expected,
    })
}
