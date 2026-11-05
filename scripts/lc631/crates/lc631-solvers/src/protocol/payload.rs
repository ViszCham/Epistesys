//! Typed routing from registered SMT-LIB payload kinds to bounded syntax only.
//!
//! A requested payload kind selects a shape label; this module does not infer
//! or validate the root form, model, proof, counterexample, or solver truth.

use super::sexpr::{self, SExprDocument, SExprError, SExprLimits};
use super::{QueryResultKind, SolverProtocol};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadShape {
    Model,
    Proof,
    Counterexample,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadSyntaxError {
    UnsupportedMapping {
        protocol: SolverProtocol,
        result_kind: QueryResultKind,
    },
    Syntax(SExprError),
    SummaryCountOverflow,
}

/// Fixed-size syntax summary safe to retain in a solver report/store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayloadSyntaxSummary {
    requested_shape: PayloadShape,
    root_count: u32,
    node_count: u32,
}

impl PayloadSyntaxSummary {
    pub const fn requested_shape(self) -> PayloadShape {
        self.requested_shape
    }

    pub const fn root_count(self) -> u32 {
        self.root_count
    }

    pub const fn node_count(self) -> u32 {
        self.node_count
    }
}

/// Parsed syntax paired with the requested shape. The shape comes from the
/// query contract and is not inferred from or validated against the root AST.
pub struct ParsedPayloadSyntax<'source> {
    requested_shape: PayloadShape,
    document: SExprDocument<'source>,
}

impl<'source> ParsedPayloadSyntax<'source> {
    pub const fn requested_shape(&self) -> PayloadShape {
        self.requested_shape
    }

    pub const fn document(&self) -> &SExprDocument<'source> {
        &self.document
    }

    pub const fn semantically_validated(&self) -> bool {
        false
    }

    pub fn summary(&self) -> Result<PayloadSyntaxSummary, PayloadSyntaxError> {
        Ok(PayloadSyntaxSummary {
            requested_shape: self.requested_shape,
            root_count: u32::try_from(self.document.roots().len())
                .map_err(|_| PayloadSyntaxError::SummaryCountOverflow)?,
            node_count: u32::try_from(self.document.node_count())
                .map_err(|_| PayloadSyntaxError::SummaryCountOverflow)?,
        })
    }

    pub const fn claim_boundary(&self) -> &'static str {
        "requested payload shape plus bounded S-expression syntax only; no model/proof/counterexample validation, solver truth, authority or evidence promotion"
    }
}

/// Parse only ModelPayload/ProofPayload/CounterexamplePayload for the separate
/// SMT-LIB payload protocol. SmtLibStatusV1 remains status-only; all other
/// protocol/kind pairs are explicitly Unsupported.
pub fn parse_payload_syntax<'source>(
    protocol: SolverProtocol,
    result_kind: QueryResultKind,
    input: &'source [u8],
    limits: SExprLimits,
) -> Result<ParsedPayloadSyntax<'source>, PayloadSyntaxError> {
    parse_payload_syntax_inner(protocol, result_kind, input, limits, false)
}

pub(crate) fn parse_payload_syntax_reserved<'source>(
    protocol: SolverProtocol,
    result_kind: QueryResultKind,
    input: &'source [u8],
    limits: SExprLimits,
) -> Result<ParsedPayloadSyntax<'source>, PayloadSyntaxError> {
    parse_payload_syntax_inner(protocol, result_kind, input, limits, true)
}

fn parse_payload_syntax_inner<'source>(
    protocol: SolverProtocol,
    result_kind: QueryResultKind,
    input: &'source [u8],
    limits: SExprLimits,
    preallocated: bool,
) -> Result<ParsedPayloadSyntax<'source>, PayloadSyntaxError> {
    let requested_shape = match (protocol, result_kind) {
        (SolverProtocol::SmtLibPayloadV1, QueryResultKind::ModelPayload) => PayloadShape::Model,
        (SolverProtocol::SmtLibPayloadV1, QueryResultKind::ProofPayload) => PayloadShape::Proof,
        (SolverProtocol::SmtLibPayloadV1, QueryResultKind::CounterexamplePayload) => {
            PayloadShape::Counterexample
        }
        _ => {
            return Err(PayloadSyntaxError::UnsupportedMapping {
                protocol,
                result_kind,
            });
        }
    };

    let document = if preallocated {
        sexpr::parse_preallocated(input, limits)
    } else {
        sexpr::parse(input, limits)
    }
    .map_err(PayloadSyntaxError::Syntax)?;
    Ok(ParsedPayloadSyntax {
        requested_shape,
        document,
    })
}
