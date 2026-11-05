//! Strict narrow status-plus-payload wire profile; syntax only.
//! The borrowed view retains the exact original bytes, including CRLF.
use super::payload::PayloadShape;
use super::sexpr::{self, NodeId, SExprDocument, SExprError, SExprKind, SExprLimits};
use super::QueryResultKind;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadWireStatus {
    Sat,
    Unsat,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayloadWireSpan {
    start: u32,
    end: u32,
}
impl PayloadWireSpan {
    pub const fn start(self) -> u32 {
        self.start
    }
    pub const fn end(self) -> u32 {
        self.end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadWireError {
    UnsupportedMapping,
    Syntax(SExprError),
    UnexpectedRootCount,
    InvalidStatus,
    StatusKindMismatch,
    UnsupportedShape,
    OffsetOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayloadWireSummary {
    status: PayloadWireStatus,
    shape: PayloadShape,
    status_span: PayloadWireSpan,
    payload_span: PayloadWireSpan,
    stdout_len: u32,
    stdout_digest: [u8; 32],
}
impl PayloadWireSummary {
    pub const fn status(self) -> PayloadWireStatus {
        self.status
    }
    pub const fn shape(self) -> PayloadShape {
        self.shape
    }
    pub const fn status_span(self) -> PayloadWireSpan {
        self.status_span
    }
    pub const fn payload_span(self) -> PayloadWireSpan {
        self.payload_span
    }
    pub const fn stdout_len(self) -> u32 {
        self.stdout_len
    }
    pub const fn stdout_digest(self) -> [u8; 32] {
        self.stdout_digest
    }
}

/// The view owns syntax nodes and borrows the exact entire stdout.
/// It is not a model validator, proof checker, or authentication receipt.
pub struct PayloadWireView<'source> {
    document: SExprDocument<'source>,
    summary: PayloadWireSummary,
}
impl<'source> PayloadWireView<'source> {
    pub fn source_bytes(&self) -> &'source [u8] {
        self.document.source_bytes()
    }
    pub const fn summary(&self) -> PayloadWireSummary {
        self.summary
    }
    pub const fn status(&self) -> PayloadWireStatus {
        self.summary.status
    }
    pub const fn shape(&self) -> PayloadShape {
        self.summary.shape
    }
    pub const fn status_span(&self) -> PayloadWireSpan {
        self.summary.status_span
    }
    pub const fn payload_span(&self) -> PayloadWireSpan {
        self.summary.payload_span
    }
    pub const fn stdout_digest(&self) -> [u8; 32] {
        self.summary.stdout_digest
    }
}

/// Observes only an exact whole first line, not semantic truth.
pub fn observed_wire_status(raw: &[u8]) -> Option<PayloadWireStatus> {
    if raw.starts_with(b"sat\n") || raw.starts_with(b"sat\r\n") {
        Some(PayloadWireStatus::Sat)
    } else if raw.starts_with(b"unsat\n") || raw.starts_with(b"unsat\r\n") {
        Some(PayloadWireStatus::Unsat)
    } else if raw.starts_with(b"unknown\n") || raw.starts_with(b"unknown\r\n") {
        Some(PayloadWireStatus::Unknown)
    } else {
        None
    }
}

fn raw_is(doc: &SExprDocument<'_>, id: NodeId, expected: &[u8]) -> bool {
    doc.node(id)
        .is_some_and(|node| node.kind() == SExprKind::Atom)
        && doc.raw(id) == Some(expected)
}
fn kind_is(doc: &SExprDocument<'_>, id: NodeId, kind: SExprKind) -> bool {
    doc.node(id).is_some_and(|node| node.kind() == kind)
}
fn decimal(doc: &SExprDocument<'_>, id: NodeId) -> bool {
    kind_is(doc, id, SExprKind::Atom)
        && doc.raw(id).is_some_and(|raw| {
            !raw.is_empty()
                && (raw.len() == 1 || raw[0] != b'0')
                && raw.iter().all(u8::is_ascii_digit)
        })
}
fn symbol(doc: &SExprDocument<'_>, id: NodeId) -> bool {
    let (Some(node), Some(raw)) = (doc.node(id), doc.raw(id)) else {
        return false;
    };
    match node.kind() {
        SExprKind::Atom => {
            let Some((&first, tail)) = raw.split_first() else {
                return false;
            };
            (first.is_ascii_alphabetic() || first == b'_')
                && tail.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_')
                && ![
                    b"true".as_slice(),
                    b"false",
                    b"let",
                    b"forall",
                    b"exists",
                    b"match",
                    b"par",
                    b"as",
                    b"error",
                    b"_",
                ]
                .contains(&raw)
        }
        SExprKind::QuotedSymbol => {
            raw.len() > 2
                && raw[1..raw.len() - 1]
                    .iter()
                    .all(|b| (b' '..=b'~').contains(b) && *b != b'|' && *b != b'\\')
        }
        _ => false,
    }
}
fn int_value(doc: &SExprDocument<'_>, id: NodeId) -> bool {
    if decimal(doc, id) {
        return true;
    }
    let Some(mut values) = doc.children(id) else {
        return false;
    };
    let (Some(minus), Some(number), None) = (values.next(), values.next(), values.next()) else {
        return false;
    };
    raw_is(doc, minus, b"-") && decimal(doc, number) && doc.raw(number) != Some(b"0")
}
fn model_shape(doc: &SExprDocument<'_>, id: NodeId) -> bool {
    let Some(definitions) = doc.children(id) else {
        return false;
    };
    for definition in definitions {
        let Some(mut parts) = doc.children(definition) else {
            return false;
        };
        let (Some(head), Some(name), Some(parameters), Some(sort), Some(value), None) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            return false;
        };
        if !raw_is(doc, head, b"define-fun")
            || !symbol(doc, name)
            || !doc
                .children(parameters)
                .is_some_and(|mut it| it.next().is_none())
        {
            return false;
        }
        if raw_is(doc, sort, b"Int") {
            if !int_value(doc, value) {
                return false;
            }
        } else if raw_is(doc, sort, b"Bool") {
            if !raw_is(doc, value, b"true") && !raw_is(doc, value, b"false") {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}
fn contains_error(doc: &SExprDocument<'_>, id: NodeId) -> bool {
    let Some(mut children) = doc.children(id) else {
        return false;
    };
    let Some(first) = children.next() else {
        return false;
    };
    if raw_is(doc, first, b"error") || contains_error(doc, first) {
        return true;
    }
    children.any(|child| contains_error(doc, child))
}
fn proof_shape(doc: &SExprDocument<'_>, id: NodeId) -> bool {
    let Some(mut entries) = doc.children(id) else {
        return false;
    };
    let (Some(logic), Some(proof), None) = (entries.next(), entries.next(), entries.next()) else {
        return false;
    };
    let Some(mut logic_parts) = doc.children(logic) else {
        return false;
    };
    let (Some(set), Some(profile), None) =
        (logic_parts.next(), logic_parts.next(), logic_parts.next())
    else {
        return false;
    };
    let Some(mut proof_parts) = doc.children(proof) else {
        return false;
    };
    let (Some(head), Some(_body), None) =
        (proof_parts.next(), proof_parts.next(), proof_parts.next())
    else {
        return false;
    };
    raw_is(doc, set, b"set-logic")
        && raw_is(doc, profile, b"QF_LIA")
        && raw_is(doc, head, b"proof")
        && !contains_error(doc, id)
}

/// Supported wire: exact status line at byte 0, followed by one payload.
/// Model: sat + list of zero-argument Int/Bool constant define-funs.
/// Proof: unsat + ((set-logic QF_LIA) (proof <one expression>)).
/// Other kinds/fragments are unsupported; no semantic validity is asserted.
pub fn parse_payload_wire<'source>(
    kind: QueryResultKind,
    raw: &'source [u8],
    limits: SExprLimits,
) -> Result<PayloadWireView<'source>, PayloadWireError> {
    parse_payload_wire_inner(kind, raw, limits, false)
}

pub(crate) fn parse_payload_wire_reserved<'source>(
    kind: QueryResultKind,
    raw: &'source [u8],
    limits: SExprLimits,
) -> Result<PayloadWireView<'source>, PayloadWireError> {
    parse_payload_wire_inner(kind, raw, limits, true)
}

fn parse_payload_wire_inner<'source>(
    kind: QueryResultKind,
    raw: &'source [u8],
    limits: SExprLimits,
    preallocated: bool,
) -> Result<PayloadWireView<'source>, PayloadWireError> {
    let shape = match kind {
        QueryResultKind::ModelPayload => PayloadShape::Model,
        QueryResultKind::ProofPayload => PayloadShape::Proof,
        _ => return Err(PayloadWireError::UnsupportedMapping),
    };
    let document = if preallocated {
        sexpr::parse_preallocated(raw, limits)
    } else {
        sexpr::parse(raw, limits)
    }
    .map_err(PayloadWireError::Syntax)?;
    let [status_id, payload_id] = document.roots() else {
        return Err(PayloadWireError::UnexpectedRootCount);
    };
    let status = if raw_is(&document, *status_id, b"sat") {
        PayloadWireStatus::Sat
    } else if raw_is(&document, *status_id, b"unsat") {
        PayloadWireStatus::Unsat
    } else {
        return Err(PayloadWireError::InvalidStatus);
    };
    if !matches!(
        (shape, status),
        (PayloadShape::Model, PayloadWireStatus::Sat)
            | (PayloadShape::Proof, PayloadWireStatus::Unsat)
    ) {
        return Err(PayloadWireError::StatusKindMismatch);
    }
    let status_node = document
        .node(*status_id)
        .ok_or(PayloadWireError::InvalidStatus)?;
    let payload_node = document
        .node(*payload_id)
        .ok_or(PayloadWireError::UnsupportedShape)?;
    let status_span = status_node.span();
    let payload_span = payload_node.span();
    let between = raw
        .get(status_span.end()..payload_span.start())
        .ok_or(PayloadWireError::OffsetOverflow)?;
    if status_span.start() != 0
        || !(between.starts_with(b"\n") || between.starts_with(b"\r\n"))
        || !between.iter().all(u8::is_ascii_whitespace)
        || !raw
            .get(payload_span.end()..)
            .ok_or(PayloadWireError::OffsetOverflow)?
            .iter()
            .all(u8::is_ascii_whitespace)
    {
        return Err(PayloadWireError::UnsupportedShape);
    }
    let valid = match shape {
        PayloadShape::Model => model_shape(&document, *payload_id),
        PayloadShape::Proof => proof_shape(&document, *payload_id),
        PayloadShape::Counterexample => false,
    };
    if !valid {
        return Err(PayloadWireError::UnsupportedShape);
    }
    let span = |start: usize, end: usize| -> Result<PayloadWireSpan, PayloadWireError> {
        Ok(PayloadWireSpan {
            start: u32::try_from(start).map_err(|_| PayloadWireError::OffsetOverflow)?,
            end: u32::try_from(end).map_err(|_| PayloadWireError::OffsetOverflow)?,
        })
    };
    let len64 = u64::try_from(raw.len()).map_err(|_| PayloadWireError::OffsetOverflow)?;
    let mut hash = Sha256::new();
    hash.update(b"epistesys.tlcx.smt-payload-wire.v2\0");
    hash.update(len64.to_le_bytes());
    hash.update(raw);
    let summary = PayloadWireSummary {
        status,
        shape,
        status_span: span(status_span.start(), status_span.end())?,
        payload_span: span(payload_span.start(), payload_span.end())?,
        stdout_len: u32::try_from(raw.len()).map_err(|_| PayloadWireError::OffsetOverflow)?,
        stdout_digest: hash.finalize().into(),
    };
    Ok(PayloadWireView { document, summary })
}
