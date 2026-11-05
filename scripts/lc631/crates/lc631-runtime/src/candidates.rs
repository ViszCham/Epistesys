use lc631_core::CandidateId;
use std::collections::BTreeSet;

const MAX_REPORTED_CANDIDATES: usize = 2_048;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateSetError {
    InvalidLimit,
    Empty,
    TooMany,
    Duplicate,
}

/// A nonempty set of reported identities, not a set of validated solutions.
#[derive(Debug)]
pub struct CandidateSet<'a> {
    ids: &'a [CandidateId],
}

impl<'a> CandidateSet<'a> {
    pub fn checked(ids: &'a [CandidateId], limit: usize) -> Result<Self, CandidateSetError> {
        if limit == 0 || limit > MAX_REPORTED_CANDIDATES {
            return Err(CandidateSetError::InvalidLimit);
        }
        if ids.is_empty() {
            return Err(CandidateSetError::Empty);
        }
        if ids.len() > limit {
            return Err(CandidateSetError::TooMany);
        }
        if ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len() {
            return Err(CandidateSetError::Duplicate);
        }
        Ok(Self { ids })
    }

    pub const fn ids(&self) -> &'a [CandidateId] {
        self.ids
    }
}
