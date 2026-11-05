//! Given finite zero-shot program transfer; no learning, independent gold or full F2.
//! All work nonrefund is ROOT-SERIES LOCAL, not recapture/run-global enforcement.
#![forbid(unsafe_code)]
mod measure;
mod plan;
mod program;
mod selection;
mod split;
mod types;
use crate::explanation_measurement::{self as e, kernel as k};
use e::{
    ComponentMetric, ComponentReport, Digest, Digest32, Rational, ResourceClaim,
    ResourceReservationView, ScoreNode, Sha256,
};
pub use measure::{execute_transfer, OwnedTransferGain};
pub use plan::{admit_transfer, capture_transfer, TransferCandidate, TransferRoot};
pub use selection::{select_transfer_current, OwnedTransferSelection, TransferRow};
pub use types::*;
pub fn definition_revision() -> Digest32 {
    k::digest(b"finite-formal-v1.4/CR-TRANSFER/ConjunctionProgramTransferV1/actual-program-certificate.v1")
}
pub fn split_normalizer_revision() -> Digest32 {
    k::digest(b"finite-Boolean-at-most8/full-atom-permutation-AndOr-AC-multiset.v1")
}
fn mapped(error: e::Error) -> Error {
    match error {
        e::Error::InvalidInput(e) => Error::InvalidInput(e),
        e::Error::NoComparable(e) => Error::NoComparable(e),
        e::Error::Resource(e) => Error::Resource(e),
        e::Error::ForeignReservation => Error::ForeignReservation,
        e::Error::WorkAdmissionExhausted => Error::WorkAdmissionExhausted,
        e::Error::AllocationFailed => Error::AllocationFailed,
        e::Error::DefinitionMismatch => Error::DefinitionMismatch,
        e::Error::BindingMismatch => Error::BindingMismatch,
        e::Error::ExpectedInputMismatch => Error::ExpectedInputMismatch,
        e::Error::CandidateMismatch => Error::CandidateMismatch,
        e::Error::InvalidRows => Error::InvalidRows,
        e::Error::UnsupportedRole => Error::UnsupportedRole,
        e::Error::UnsupportedFragment => Error::UnsupportedFragment,
        e::Error::Numeric(e) => Error::Numeric(e),
        e::Error::Measurement(e) => Error::Measurement(e),
        e::Error::Selection(e) => Error::Selection(e),
    }
}
#[doc = "Current owners cannot be deserialized."]
#[doc = "```compile_fail"]
#[doc = include_str!("../tests/tlcx22_explain_transfer_support/compile_fail/deserialize_owner.rs")]
#[doc = "```"]
#[doc = "The finite program registry has no score/callback/case-ID lookup variants."]
#[doc = "```compile_fail"]
#[doc = include_str!("../tests/tlcx22_explain_transfer_support/compile_fail/score_or_callback.rs")]
#[doc = "```"]
pub struct ProgramConstructionBoundary;
