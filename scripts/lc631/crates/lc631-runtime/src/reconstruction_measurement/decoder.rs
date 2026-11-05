//! Audited target-only identity decoder for canonical finite K6 terms.
//! NoOriginalSource: only target bytes and bounded grammar limits enter here.
//! No source handles, digests, closures, I/O, sidecars or auxiliary dictionary.
//! Pure Rust capability boundary, NOT an OS/process sandbox or NL reconstruction.
use lc631_substrate::semantic_term::{decode, Term, TermError, TermLimits};
pub(super) const CODE: &str = include_str!("decoder.rs");
// Shared runtime is pinned separately and treated equally on both sides. Its
// bytes are not a hidden, candidate-specific dictionary or source lookup.
pub(super) const COMMON_RUNTIME: &str = concat!(
    include_str!("../../../lc631-substrate/src/semantic_term.rs"),
    include_str!("../../../lc631-substrate/src/semantic_term/reader.rs"),
    include_str!("../../../lc631-substrate/src/semantic_term/writer.rs")
);
pub(super) fn decode_target(target: &[u8], limits: TermLimits) -> Result<Term, TermError> {
    decode(target, limits)
}
