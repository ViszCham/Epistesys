use crate::protocol::sexpr::{NodeId, SExprDocument, SExprKind};
use crate::typed_query::{BoolSymbolId, IntSymbolId, SmtStatusQuery};

const MAX_SYMBOLS: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelValidationError {
    ModelShape,
    UnsupportedFunction,
    UnsupportedValue,
    UndeclaredSymbol,
    DuplicateAssignment,
    MissingAssignment,
    SortMismatch,
    IntegerOutOfRange,
    ArithmeticOverflow,
    AssertionFalse { index: usize },
}

/// A complete, locally rechecked witness for precisely this borrowed query.
/// No trust-root, proof, process-authentication or independence token is held.
pub struct CheckedModel<'query> {
    query: &'query SmtStatusQuery,
    booleans: [Option<bool>; MAX_SYMBOLS],
    integers: [Option<i128>; MAX_SYMBOLS],
}

impl CheckedModel<'_> {
    pub fn query(&self) -> &SmtStatusQuery {
        self.query
    }

    pub fn boolean(&self, symbol: BoolSymbolId) -> Option<bool> {
        let index = self.query.boolean_symbols().binary_search(&symbol).ok()?;
        self.booleans.get(index).copied().flatten()
    }

    pub fn integer(&self, symbol: IntSymbolId) -> Option<i128> {
        let index = self.query.integer_symbols().binary_search(&symbol).ok()?;
        self.integers.get(index).copied().flatten()
    }

    pub fn assertions_checked(&self) -> usize {
        self.query.assertions().len()
    }
}

/// Reevaluate every original assertion. Unsupported values never become false
/// or zero. This consumes model-only syntax, not a status line or a proof.
pub fn validate_model<'query>(
    query: &'query SmtStatusQuery,
    document: &SExprDocument<'_>,
) -> Result<CheckedModel<'query>, ModelValidationError> {
    let [root] = document.roots() else {
        return Err(ModelValidationError::ModelShape);
    };
    let mut entries = document
        .children(*root)
        .ok_or(ModelValidationError::ModelShape)?;
    let mut model = CheckedModel {
        query,
        booleans: [None; MAX_SYMBOLS],
        integers: [None; MAX_SYMBOLS],
    };
    if let Some(first) = entries.next() {
        if atom(document, first) != Some(b"model".as_slice()) {
            assignment(&mut model, document, first)?;
        }
    }
    for entry in entries {
        assignment(&mut model, document, entry)?;
    }
    if model.booleans[..query.boolean_symbols().len()]
        .iter()
        .any(Option::is_none)
        || model.integers[..query.integer_symbols().len()]
            .iter()
            .any(Option::is_none)
    {
        return Err(ModelValidationError::MissingAssignment);
    }
    for (index, assertion) in query.assertions().iter().enumerate() {
        if !super::evaluate::boolean(assertion, &model)? {
            return Err(ModelValidationError::AssertionFalse { index });
        }
    }
    Ok(model)
}

fn assignment(
    model: &mut CheckedModel<'_>,
    doc: &SExprDocument<'_>,
    node: NodeId,
) -> Result<(), ModelValidationError> {
    let mut fields = doc.children(node).ok_or(ModelValidationError::ModelShape)?;
    let (Some(command), Some(name), Some(arguments), Some(sort), Some(value), None) = (
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
    ) else {
        return Err(ModelValidationError::ModelShape);
    };
    if atom(doc, command) != Some(b"define-fun".as_slice()) {
        return Err(ModelValidationError::ModelShape);
    }
    if doc
        .children(arguments)
        .ok_or(ModelValidationError::ModelShape)?
        .next()
        .is_some()
    {
        return Err(ModelValidationError::UnsupportedFunction);
    }
    let (prefix, id) = symbol(doc, name)?;
    match prefix {
        b'b' => {
            let index = model
                .query
                .boolean_symbols()
                .binary_search(&BoolSymbolId::new(id))
                .map_err(|_| ModelValidationError::UndeclaredSymbol)?;
            if atom(doc, sort) != Some(b"Bool".as_slice()) {
                return Err(ModelValidationError::SortMismatch);
            }
            if model.booleans[index].is_some() {
                return Err(ModelValidationError::DuplicateAssignment);
            }
            model.booleans[index] = Some(match atom(doc, value) {
                Some(b"true") => true,
                Some(b"false") => false,
                _ => return Err(ModelValidationError::UnsupportedValue),
            });
        }
        b'i' => {
            let index = model
                .query
                .integer_symbols()
                .binary_search(&IntSymbolId::new(id))
                .map_err(|_| ModelValidationError::UndeclaredSymbol)?;
            if atom(doc, sort) != Some(b"Int".as_slice()) {
                return Err(ModelValidationError::SortMismatch);
            }
            if model.integers[index].is_some() {
                return Err(ModelValidationError::DuplicateAssignment);
            }
            model.integers[index] = Some(integer_value(doc, value)?);
        }
        _ => return Err(ModelValidationError::UndeclaredSymbol),
    }
    Ok(())
}

fn atom<'source>(doc: &SExprDocument<'source>, id: NodeId) -> Option<&'source [u8]> {
    if doc.node(id)?.kind() == SExprKind::Atom {
        doc.raw(id)
    } else {
        None
    }
}

fn symbol(doc: &SExprDocument<'_>, id: NodeId) -> Result<(u8, u32), ModelValidationError> {
    let node = doc.node(id).ok_or(ModelValidationError::ModelShape)?;
    let raw = match node.kind() {
        SExprKind::Atom => doc.raw(id),
        SExprKind::QuotedSymbol => node.content_span().and_then(|span| doc.raw_span(span)),
        SExprKind::List | SExprKind::StringLiteral => None,
    }
    .ok_or(ModelValidationError::UndeclaredSymbol)?;
    let (prefix, digits) = raw
        .split_first()
        .ok_or(ModelValidationError::UndeclaredSymbol)?;
    let number = decimal(digits).ok_or(ModelValidationError::UndeclaredSymbol)?;
    Ok((
        *prefix,
        u32::try_from(number).map_err(|_| ModelValidationError::UndeclaredSymbol)?,
    ))
}

fn decimal(bytes: &[u8]) -> Option<u128> {
    if bytes.is_empty() || (bytes.len() > 1 && bytes[0] == b'0') {
        return None;
    }
    bytes.iter().try_fold(0u128, |value, byte| {
        if !byte.is_ascii_digit() {
            return None;
        }
        value.checked_mul(10)?.checked_add(u128::from(byte - b'0'))
    })
}

fn integer_value(doc: &SExprDocument<'_>, id: NodeId) -> Result<i128, ModelValidationError> {
    if let Some(raw) = atom(doc, id) {
        let magnitude = decimal(raw).ok_or(ModelValidationError::UnsupportedValue)?;
        return i128::try_from(magnitude).map_err(|_| ModelValidationError::IntegerOutOfRange);
    }
    let mut children = doc
        .children(id)
        .ok_or(ModelValidationError::UnsupportedValue)?;
    let (Some(operator), Some(value), None) = (children.next(), children.next(), children.next())
    else {
        return Err(ModelValidationError::UnsupportedValue);
    };
    if atom(doc, operator) != Some(b"-".as_slice()) {
        return Err(ModelValidationError::UnsupportedValue);
    }
    let magnitude = atom(doc, value)
        .and_then(decimal)
        .ok_or(ModelValidationError::UnsupportedValue)?;
    if magnitude == 0 {
        return Err(ModelValidationError::UnsupportedValue);
    }
    if magnitude == (1u128 << 127) {
        return Ok(i128::MIN);
    }
    i128::try_from(magnitude)
        .ok()
        .and_then(i128::checked_neg)
        .ok_or(ModelValidationError::IntegerOutOfRange)
}
