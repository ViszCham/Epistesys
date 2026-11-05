use super::*;
use lc631_substrate::Digest32;
/// Immutable historical arithmetic; current native authority is a separate owner.
#[derive(Clone)]
pub struct CostObservation {
    pub(super) baseline_bits: Option<u64>,
    pub(super) basis: InputBasis,
    pub(super) independence: IndependenceStatus,
    pub(super) native_gap: Option<CostGap>,
    pub(super) evidence_gaps: Vec<EvidenceGap>,
    pub(super) binding: Digest32,
    pub(super) profile_digest: Digest32,
    pub(super) bits: u64,
    pub(super) bundle: Vec<u8>,
    pub(super) semantic: Vec<u8>,
    pub(super) representation: Vec<u8>,
    pub(super) reconstruction: Option<Vec<u8>>,
    pub(super) inverse_state: ExecutionState,
    pub(super) reconstruction_state: ReconstructionState,
    pub(super) compression: CompressionEligibility,
    pub(super) stats: ExecutionStats,
    pub(super) declarations: Vec<u8>,
    pub(super) dictionary: Vec<u8>,
}
impl CostObservation {
    pub fn bits(&self) -> u64 {
        self.bits
    }
    pub fn baseline_bits(&self) -> Option<u64> {
        self.baseline_bits
    }
    pub fn bundle_bytes(&self) -> &[u8] {
        &self.bundle
    }
    pub fn semantic_bytes(&self) -> &[u8] {
        &self.semantic
    }
    pub fn representation_bytes(&self) -> &[u8] {
        &self.representation
    }
    pub fn reconstruction_bytes(&self) -> Option<&[u8]> {
        self.reconstruction.as_deref()
    }
    pub fn binding(&self) -> Digest32 {
        self.binding
    }
    pub fn profile_digest(&self) -> Digest32 {
        self.profile_digest
    }
    pub fn inverse_state(&self) -> ExecutionState {
        self.inverse_state
    }
    pub fn reconstruction_state(&self) -> ReconstructionState {
        self.reconstruction_state
    }
    pub fn native_cost_gap(&self) -> Option<CostGap> {
        self.native_gap
    }
    pub fn formula_evaluated(&self) -> bool {
        false
    }
    pub fn stats(&self) -> &ExecutionStats {
        &self.stats
    }
    pub fn declarations_bytes(&self) -> &[u8] {
        &self.declarations
    }
    pub fn dictionary_bytes(&self) -> &[u8] {
        &self.dictionary
    }
    pub fn compression_eligibility(&self) -> CompressionEligibility {
        self.compression
    }
    pub fn input_basis(&self) -> InputBasis {
        self.basis
    }
    pub fn independence(&self) -> IndependenceStatus {
        self.independence
    }
    pub fn evidence_gaps(&self) -> &[EvidenceGap] {
        &self.evidence_gaps
    }
    pub(super) fn retained_bytes(&self) -> Result<u64, CostError> {
        let bytes = [
            self.bundle.len(),
            self.semantic.len(),
            self.representation.len(),
            self.reconstruction.as_ref().map_or(0, Vec::len),
            self.declarations.len(),
            self.dictionary.len(),
        ];
        bytes.iter().try_fold(4096u64, |a, b| {
            a.checked_add(*b as u64).ok_or(CostError::ResourceExhausted)
        })
    }
}
