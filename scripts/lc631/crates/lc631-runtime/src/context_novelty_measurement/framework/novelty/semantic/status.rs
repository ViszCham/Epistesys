//! 日本語: 宣言有限族の空でない実行可能行に対するcanonical forcing。
//! English: Canonical forcing on the nonempty feasible rows of the declared finite family.
use super::*;

pub struct QueryRow {
    states: [u8; 3],
    len: usize,
    admitted: bool,
    query_bits: u64,
    negation_bits: u64,
}
impl QueryRow {
    pub(super) fn checked(row: &f::ModelRow, query_bits: u64, negation_bits: u64,
        kernel: f::Kernel) -> Result<Self, Error> {
        let input = row.valuation_states();
        let mask = if kernel == f::Kernel::ClassicalPointV1 { 1 } else { 3 };
        if input.len() > 3 || (query_bits | negation_bits) & !mask != 0
            || query_bits & negation_bits != 0 {
            return Err(Error::ExpectedInputMismatch);
        }
        let mut states = [0; 3];
        states[..input.len()].copy_from_slice(input);
        Ok(Self { states, len: input.len(), admitted: row.admitted(), query_bits, negation_bits })
    }
    pub fn valuation_states(&self) -> &[u8] { &self.states[..self.len] }
    pub fn admitted(&self) -> bool { self.admitted }
    pub fn query_bits(&self) -> u64 { self.query_bits }
    pub fn negation_bits(&self) -> u64 { self.negation_bits }
}
pub struct Query {
    pub(super) role: f::QueryRole,
    rows: Vec<QueryRow>,
    state: CanonicalStatus,
    feasible: usize,
    forced: usize,
    negated: usize,
}
impl Query {
    pub(super) fn from_rows(role: f::QueryRole, rows: Vec<QueryRow>) -> Result<Self, Error> {
        let feasible = rows.iter().filter(|r| r.admitted()).count();
        if feasible == 0 { return Err(framework(f::Error::ContextInconsistent)); }
        let forced = rows.iter().filter(|r| r.admitted() && r.query_bits() & 1 != 0).count();
        let negated = rows.iter().filter(|r| r.admitted() && r.negation_bits() & 1 != 0).count();
        let state = if forced == feasible { CanonicalStatus::Supported }
            else if negated == feasible { CanonicalStatus::Refuted } else { CanonicalStatus::Undetermined };
        Ok(Self { role, rows, state, feasible, forced, negated })
    }
    pub fn status(&self) -> CanonicalStatus { self.state }
    pub fn feasible_models(&self) -> usize { self.feasible }
    pub fn forced_models(&self) -> usize { self.forced }
    pub fn negation_forced_models(&self) -> usize { self.negated }
    pub fn rows(&self) -> &[QueryRow] { &self.rows }
}
