//! Bounded, non-semantic views for registered solver-output protocols.
//!
//! A parsed status remains solver-reported. This module does not validate
//! models, prove UNSAT, or promote a result to semantic truth.

use crate::logical::{LogicalResult, Satisfiability};

/// Bounded, syntax-only Clingo answer and terminal-status spans.
pub mod clingo;
/// Syntax-only parser selection for registered structured payload kinds.
pub mod payload;
/// Strict status-plus-payload profile; not a model/proof validator.
pub mod payload_wire;
/// Bounded S-expression syntax views; separate from the status-only parser.
pub mod sexpr;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SolverProtocol {
    SmtLibStatusV1,
    SmtLibPayloadV1,
    SmtLibPayloadWireV2,
    ClingoStatusV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum QueryResultKind {
    Satisfiability,
    Entailment,
    StableModelExistence,
    ModelPayload,
    ProofPayload,
    CounterexamplePayload,
    ModelCheck,
    Inference,
    Optimization,
    Intervention,
    SpatialRelation,
}

/// Compatibility is a protocol-shape fact only; `Supported` does not imply
/// that a backend exists, ran, or produced a valid logical result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryProtocolCompatibility {
    Supported,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationBasis {
    SolverReported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    OutputTooLarge,
    InvalidUtf8,
    MissingStatus,
    DuplicateOrTrailingStatus,
    UnsupportedQueryKind,
    UnknownStatus,
    /// A bounded Clingo shape/resource failure with no legacy equivalent.
    ClingoOutput(clingo::ClingoOutputError),
}

/// The currently declared protocols only parse satisfiability/status output.
/// Model, proof and counterexample payloads remain explicitly unsupported.
pub const fn query_protocol_compatibility(
    protocol: SolverProtocol,
    query_kind: QueryResultKind,
) -> QueryProtocolCompatibility {
    if matches!(
        (protocol, query_kind),
        (
            SolverProtocol::SmtLibStatusV1,
            QueryResultKind::Satisfiability
        ) | (
            SolverProtocol::ClingoStatusV1,
            QueryResultKind::StableModelExistence
        ) | (
            SolverProtocol::SmtLibPayloadV1,
            QueryResultKind::ModelPayload
        ) | (
            SolverProtocol::SmtLibPayloadV1,
            QueryResultKind::ProofPayload
        ) | (
            SolverProtocol::SmtLibPayloadV1,
            QueryResultKind::CounterexamplePayload
        ) | (
            SolverProtocol::SmtLibPayloadWireV2,
            QueryResultKind::ModelPayload
        ) | (
            SolverProtocol::SmtLibPayloadWireV2,
            QueryResultKind::ProofPayload
        )
    ) {
        QueryProtocolCompatibility::Supported
    } else {
        QueryProtocolCompatibility::Unsupported
    }
}

/// Convenience predicate for callers that need a preflight boolean. Prefer
/// `query_protocol_compatibility` when the unsupported state must stay typed.
pub const fn supports_query_kind(protocol: SolverProtocol, query_kind: QueryResultKind) -> bool {
    matches!(
        query_protocol_compatibility(protocol, query_kind),
        QueryProtocolCompatibility::Supported
    )
}

/// Parse one deliberately narrow status protocol.
///
/// SMT-LIB status mode accepts exactly one token (`sat`, `unsat`, or
/// `unknown`). Clingo status mode accepts one exact terminal status line from
/// its bounded text output. This function intentionally rejects
/// `SmtLibPayloadV1`; payload syntax is handled by `payload::parse_payload_syntax`
/// and does not produce a logical result. No parser here authenticates the
/// executable or proves the reported logical result; callers must retain
/// `ValidationBasis::SolverReported` for status outcomes.
pub fn parse_status(
    protocol: SolverProtocol,
    query_kind: QueryResultKind,
    output: &[u8],
    max_output_bytes: usize,
) -> Result<LogicalResult, ProtocolError> {
    if output.len() > max_output_bytes {
        return Err(ProtocolError::OutputTooLarge);
    }

    if !supports_query_kind(protocol, query_kind) {
        return Err(ProtocolError::UnsupportedQueryKind);
    }
    match protocol {
        SolverProtocol::SmtLibStatusV1 => parse_smtlib_status(output),
        SolverProtocol::SmtLibPayloadV1 | SolverProtocol::SmtLibPayloadWireV2 => {
            Err(ProtocolError::UnsupportedQueryKind)
        }
        SolverProtocol::ClingoStatusV1 => parse_clingo_status(output),
    }
}

fn parse_smtlib_status(output: &[u8]) -> Result<LogicalResult, ProtocolError> {
    let token = trim_ascii_whitespace(output);
    if token.is_empty() {
        return Err(ProtocolError::MissingStatus);
    }
    if token.iter().any(u8::is_ascii_whitespace) {
        return Err(ProtocolError::DuplicateOrTrailingStatus);
    }
    status_word(token).map(LogicalResult::Satisfiability)
}

fn parse_clingo_status(output: &[u8]) -> Result<LogicalResult, ProtocolError> {
    // Share the actual runner's bounded terminal-shape admission. A line
    // containing a status cannot hide contradictory answers or trailing text.
    let view = clingo::parse_clingo_output(output, clingo::ClingoOutputLimits::default())
        .map_err(clingo_protocol_error)?;
    let status = match view.status() {
        clingo::ClingoTerminalStatus::Satisfiable => Satisfiability::Sat,
        clingo::ClingoTerminalStatus::Unsatisfiable => Satisfiability::Unsat,
        clingo::ClingoTerminalStatus::Unknown => Satisfiability::Unknown,
    };
    Ok(LogicalResult::Satisfiability(status))
}

fn clingo_protocol_error(error: clingo::ClingoOutputError) -> ProtocolError {
    use clingo::ClingoOutputError;
    match error {
        ClingoOutputError::OutputTooLarge => ProtocolError::OutputTooLarge,
        ClingoOutputError::InvalidUtf8 => ProtocolError::InvalidUtf8,
        ClingoOutputError::MissingTerminalStatus => ProtocolError::MissingStatus,
        ClingoOutputError::DuplicateStatus
        | ClingoOutputError::ContradictoryAnswerStatus
        | ClingoOutputError::StatusOutOfOrder
        | ClingoOutputError::UnexpectedAfterStatus => ProtocolError::DuplicateOrTrailingStatus,
        ClingoOutputError::UnsupportedStatus => ProtocolError::UnknownStatus,
        ClingoOutputError::InvalidLimits
        | ClingoOutputError::LimitsTooLarge
        | ClingoOutputError::MalformedAnswerHeader
        | ClingoOutputError::MalformedAnswerNumber
        | ClingoOutputError::AnswerNumberOutOfOrder
        | ClingoOutputError::TooManyAnswers
        | ClingoOutputError::TooManyAtomLines
        | ClingoOutputError::InvalidSpan
        | ClingoOutputError::SummaryCountOverflow
        | ClingoOutputError::AllocationFailed => ProtocolError::ClingoOutput(error),
    }
}

fn status_word(word: &[u8]) -> Result<Satisfiability, ProtocolError> {
    match word {
        b"sat" => Ok(Satisfiability::Sat),
        b"unsat" => Ok(Satisfiability::Unsat),
        b"unknown" => Ok(Satisfiability::Unknown),
        _ => Err(ProtocolError::UnknownStatus),
    }
}

fn trim_ascii_whitespace(mut bytes: &[u8]) -> &[u8] {
    while bytes.first().is_some_and(u8::is_ascii_whitespace) {
        bytes = &bytes[1..];
    }
    while bytes.last().is_some_and(u8::is_ascii_whitespace) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tlcx10_smt_status_is_exact_and_single() {
        assert_eq!(
            parse_status(
                SolverProtocol::SmtLibStatusV1,
                QueryResultKind::Satisfiability,
                b"unsat\n",
                16,
            ),
            Ok(LogicalResult::Satisfiability(Satisfiability::Unsat))
        );
        assert_eq!(
            parse_status(
                SolverProtocol::SmtLibStatusV1,
                QueryResultKind::Satisfiability,
                b"sat\nunsat\n",
                32,
            ),
            Err(ProtocolError::DuplicateOrTrailingStatus)
        );
    }

    #[test]
    fn tlcx10_clingo_text_status_does_not_claim_cautious_consequence() {
        assert_eq!(
            parse_status(
                SolverProtocol::ClingoStatusV1,
                QueryResultKind::StableModelExistence,
                b"Answer: 1\na(1)\nSATISFIABLE\n",
                128,
            ),
            Ok(LogicalResult::Satisfiability(Satisfiability::Sat))
        );
        assert_eq!(
            parse_status(
                SolverProtocol::ClingoStatusV1,
                QueryResultKind::Inference,
                b"SATISFIABLE\n",
                32,
            ),
            Err(ProtocolError::UnsupportedQueryKind)
        );
    }

    #[test]
    fn tlcx10_protocol_bound_and_unknown_are_explicit() {
        assert_eq!(
            parse_status(
                SolverProtocol::SmtLibStatusV1,
                QueryResultKind::Satisfiability,
                b"unknown\n",
                8,
            ),
            Ok(LogicalResult::Satisfiability(Satisfiability::Unknown))
        );
        assert_eq!(
            parse_status(
                SolverProtocol::SmtLibStatusV1,
                QueryResultKind::Satisfiability,
                b"sat\n",
                3,
            ),
            Err(ProtocolError::OutputTooLarge)
        );
    }

    #[test]
    fn tlcx10_model_proof_and_counterexample_payloads_are_not_supported() {
        for result_kind in [
            QueryResultKind::ModelPayload,
            QueryResultKind::ProofPayload,
            QueryResultKind::CounterexamplePayload,
        ] {
            assert_eq!(
                query_protocol_compatibility(SolverProtocol::SmtLibStatusV1, result_kind),
                QueryProtocolCompatibility::Unsupported
            );
            assert_eq!(
                parse_status(SolverProtocol::SmtLibStatusV1, result_kind, b"sat\n", 16),
                Err(ProtocolError::UnsupportedQueryKind)
            );
            assert_eq!(
                query_protocol_compatibility(SolverProtocol::ClingoStatusV1, result_kind),
                QueryProtocolCompatibility::Unsupported
            );
        }
    }
}
