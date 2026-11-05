//! 日本語: 元traceを共有し、loopやopen prefixを有限閉鎖へ変換しない。
//! English: Share the original trace; never convert loops/open prefixes to closed traces.
use super::{Instant, TemporalError};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Truth {
    False,
    Unknown,
    True,
}

impl Truth {
    pub(super) fn not(self) -> Self {
        match self {
            Self::False => Self::True,
            Self::Unknown => Self::Unknown,
            Self::True => Self::False,
        }
    }
    pub(super) fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::True, Self::True) => Self::True,
            (Self::Unknown, Self::Unknown | Self::True) | (Self::True, Self::Unknown) => {
                Self::Unknown
            }
        }
    }
    pub(super) fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::True, _) | (_, Self::True) => Self::True,
            (Self::False, Self::False) => Self::False,
            (Self::Unknown, Self::Unknown | Self::False) | (Self::False, Self::Unknown) => {
                Self::Unknown
            }
        }
    }
    pub(super) fn code(self) -> u8 {
        match self {
            Self::False => 0,
            Self::Unknown => 1,
            Self::True => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceClosure {
    Closed,
    OpenPrefix,
    LoopTo(u16),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceStep {
    instant: Instant,
    values: Vec<Truth>,
}

impl TraceStep {
    pub fn new(instant: Instant, values: Vec<Truth>) -> Self {
        Self { instant, values }
    }
    pub const fn instant(&self) -> Instant {
        self.instant
    }
    pub fn values(&self) -> &[Truth] {
        &self.values
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FiniteTrace {
    steps: Arc<[TraceStep]>,
    digest: Digest32,
    atoms: usize,
}

impl FiniteTrace {
    pub fn checked(steps: Vec<TraceStep>, closure: TraceClosure) -> Result<Self, TemporalError> {
        match closure {
            TraceClosure::Closed => {}
            TraceClosure::OpenPrefix => return Err(TemporalError::UnsupportedOpenPrefix),
            TraceClosure::LoopTo(_) => return Err(TemporalError::UnsupportedLoop),
        }
        let first = steps.first().ok_or(TemporalError::EmptyTrace)?;
        if steps.len() > 256 {
            return Err(TemporalError::TraceLimitExceeded);
        }
        let atoms = first.values.len();
        if atoms > 256 {
            return Err(TemporalError::AtomLimitExceeded);
        }
        for step in &steps {
            if step.instant.clock() != first.instant.clock() {
                return Err(TemporalError::ClockMismatch);
            }
            if step.values.len() != atoms {
                return Err(TemporalError::InconsistentAtomWidth);
            }
        }
        if steps
            .windows(2)
            .any(|w| w[0].instant.nanoseconds() >= w[1].instant.nanoseconds())
        {
            return Err(TemporalError::NonIncreasingTime);
        }
        let mut hash = Sha256::new();
        hash.update(b"epistesys.temporal-trace.closed-kleene.v1\0");
        hash.update((steps.len() as u16).to_le_bytes());
        hash.update((atoms as u16).to_le_bytes());
        for step in &steps {
            step.instant.hash_into(&mut hash);
            for truth in &step.values {
                hash.update([truth.code()]);
            }
        }
        Ok(Self {
            steps: steps.into(),
            digest: Digest32::from_bytes(hash.finalize().into()),
            atoms,
        })
    }
    pub fn steps(&self) -> &[TraceStep] {
        &self.steps
    }
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }
    pub const fn atom_count(&self) -> usize {
        self.atoms
    }
}
