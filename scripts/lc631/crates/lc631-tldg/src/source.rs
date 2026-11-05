use lc631_core::{stable_sha256, SourceSpan};
use lc631_substrate::source::{SourceError, SourceSpan as CheckedSourceSpan, SourceStore};
use serde::ser::{SerializeSeq, SerializeStruct};
use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceRevision {
    pub revision: String,
    pub byte_len: usize,
}

impl SourceRevision {
    pub fn from_source(source: &str) -> Self {
        Self {
            revision: stable_sha256(source),
            byte_len: source.len(),
        }
    }

    pub fn from_store(source: &SourceStore) -> Self {
        Self {
            revision: legacy_digest(source.id().digest().as_bytes()),
            byte_len: source.text().len(),
        }
    }
}

fn legacy_digest(digest: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(71);
    encoded.push_str("sha256:");
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

/// Immutable checked source slice. Clone shares bytes; equality compares text,
/// while source/span accessors retain the separate provenance binding.
#[derive(Clone)]
pub struct SourceTextView {
    source: SourceStore,
    span: CheckedSourceSpan,
}

impl SourceTextView {
    pub fn checked(source: &SourceStore, start: usize, end: usize) -> Result<Self, SourceError> {
        Self::from_span(source, CheckedSourceSpan::checked(source, start, end)?)
    }

    pub fn from_span(source: &SourceStore, span: CheckedSourceSpan) -> Result<Self, SourceError> {
        source.slice(span)?;
        Ok(Self {
            source: source.clone(),
            span,
        })
    }

    pub fn full(source: &SourceStore) -> Result<Self, SourceError> {
        Self::checked(source, 0, source.text().len())
    }

    pub fn source(&self) -> &SourceStore {
        &self.source
    }

    pub const fn span(&self) -> CheckedSourceSpan {
        self.span
    }

    pub fn as_str(&self) -> &str {
        // Private fields and immutable SourceStore preserve constructor checks.
        &self.source.text()[self.span.start()..self.span.end()]
    }
}

impl Default for SourceTextView {
    fn default() -> Self {
        let source = SourceStore::new(Arc::from(""), 1)
            .expect("the static empty source satisfies a positive one-byte limit");
        Self::full(&source).expect("the static empty source has a valid full span")
    }
}

impl Deref for SourceTextView {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for SourceTextView {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Debug for SourceTextView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl PartialEq for SourceTextView {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for SourceTextView {}

impl PartialEq<str> for SourceTextView {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for SourceTextView {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for SourceTextView {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Serialize for SourceTextView {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryClass {
    Alphanumeric,
    Whitespace,
    Punctuation,
    Symbol,
}

fn boundary_class(character: char) -> BoundaryClass {
    if character.is_alphanumeric() || character == '_' {
        BoundaryClass::Alphanumeric
    } else if character.is_whitespace() {
        BoundaryClass::Whitespace
    } else if character.is_ascii_punctuation() {
        BoundaryClass::Punctuation
    } else {
        BoundaryClass::Symbol
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryError {
    Source(SourceError),
    CellBudgetExceeded,
    AllocationFailed,
}

/// Canonical cells retain checked offsets, never a separate surface String.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalBoundaryCell {
    span: CheckedSourceSpan,
    class: BoundaryClass,
}

impl CanonicalBoundaryCell {
    pub const fn span(self) -> CheckedSourceSpan {
        self.span
    }

    pub const fn class(self) -> BoundaryClass {
        self.class
    }
}

/// A borrowed view of the caller-owned, immutable source allocation.
#[derive(Debug)]
pub struct CanonicalBoundaryLedger<'source> {
    source: &'source SourceStore,
    cells: Vec<CanonicalBoundaryCell>,
}

impl<'source> CanonicalBoundaryLedger<'source> {
    pub fn from_store(
        source: &'source SourceStore,
        max_cells: usize,
    ) -> Result<Self, BoundaryError> {
        let count = source.text().chars().count();
        if count > max_cells {
            return Err(BoundaryError::CellBudgetExceeded);
        }
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(count)
            .map_err(|_| BoundaryError::AllocationFailed)?;
        for (start, character) in source.text().char_indices() {
            let end = start + character.len_utf8();
            let span =
                CheckedSourceSpan::checked(source, start, end).map_err(BoundaryError::Source)?;
            cells.push(CanonicalBoundaryCell {
                span,
                class: boundary_class(character),
            });
        }
        Ok(Self { source, cells })
    }

    pub const fn source(&self) -> &'source SourceStore {
        self.source
    }

    pub fn cells(&self) -> &[CanonicalBoundaryCell] {
        &self.cells
    }

    pub fn surface(&self, span: CheckedSourceSpan) -> Result<&'source str, SourceError> {
        self.source.slice(span)
    }

    pub fn roundtrip(&self) -> &'source str {
        self.source.text()
    }

    pub fn covers_source(&self) -> bool {
        let mut offset = 0;
        for cell in &self.cells {
            if cell.span.start() != offset || self.source.slice(cell.span).is_err() {
                return false;
            }
            offset = cell.span.end();
        }
        offset == self.source.text().len()
    }
}

#[derive(Serialize)]
struct BoundaryCellView<'a> {
    span: SourceSpan,
    class: BoundaryClass,
    surface: &'a str,
}

struct BoundaryCellsView<'a, 'source>(&'a CanonicalBoundaryLedger<'source>);

impl Serialize for BoundaryCellsView<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.cells.len()))?;
        for cell in &self.0.cells {
            let surface = self
                .0
                .surface(cell.span)
                .map_err(serde::ser::Error::custom)?;
            sequence.serialize_element(&BoundaryCellView {
                span: SourceSpan {
                    start: cell.span.start(),
                    end: cell.span.end(),
                },
                class: cell.class,
                surface,
            })?;
        }
        sequence.end()
    }
}

impl Serialize for CanonicalBoundaryLedger<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut record = serializer.serialize_struct("BoundaryLedger", 2)?;
        record.serialize_field("source", &SourceRevision::from_store(self.source))?;
        record.serialize_field("cells", &BoundaryCellsView(self))?;
        record.end()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BoundaryCell {
    pub span: SourceSpan,
    pub class: BoundaryClass,
    pub surface: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BoundaryLedger {
    pub source: SourceRevision,
    pub cells: Vec<BoundaryCell>,
}

impl BoundaryLedger {
    pub fn build(source: &str) -> Self {
        let mut cells = Vec::with_capacity(source.chars().count());
        let mut iter = source.char_indices().peekable();
        while let Some((start, character)) = iter.next() {
            let end = iter.peek().map_or(source.len(), |(index, _)| *index);
            let class = boundary_class(character);
            cells.push(BoundaryCell {
                span: SourceSpan { start, end },
                class,
                surface: character.to_string(),
            });
        }
        Self {
            source: SourceRevision::from_source(source),
            cells,
        }
    }

    pub fn roundtrip(&self) -> String {
        self.cells
            .iter()
            .map(|cell| cell.surface.as_str())
            .collect()
    }

    pub fn covers_source(&self) -> bool {
        let mut offset = 0usize;
        let mut digest = Sha256::new();
        for cell in &self.cells {
            let mut characters = cell.surface.chars();
            let Some(character) = characters.next() else {
                return false;
            };
            let Some(end) = offset.checked_add(cell.surface.len()) else {
                return false;
            };
            if characters.next().is_some()
                || cell.span.start != offset
                || cell.span.end != end
                || end > self.source.byte_len
                || cell.class != boundary_class(character)
            {
                return false;
            }
            digest.update(cell.surface.as_bytes());
            offset = end;
        }
        let bytes: [u8; 32] = digest.finalize().into();
        offset == self.source.byte_len && legacy_digest(&bytes) == self.source.revision
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn utf8_boundary_ledger_roundtrips() {
        let source = "日本語 + Rust\n";
        let ledger = BoundaryLedger::build(source);
        assert_eq!(ledger.roundtrip(), source);
        assert!(ledger.covers_source());
    }

    #[test]
    fn tlcx_02_canonical_ledger_rejects_foreign_spans_and_cell_budget() {
        let source = SourceStore::new(Arc::from("日"), 64).unwrap();
        let other = SourceStore::new(Arc::from("A"), 64).unwrap();
        assert!(matches!(
            CanonicalBoundaryLedger::from_store(&source, 0),
            Err(BoundaryError::CellBudgetExceeded)
        ));
        assert_eq!(
            CheckedSourceSpan::checked(&source, 1, 3),
            Err(SourceError::InvalidUtf8Boundary)
        );
        let mut ledger = CanonicalBoundaryLedger::from_store(&source, 1).unwrap();
        let foreign = CheckedSourceSpan::checked(&other, 0, 1).unwrap();
        assert_eq!(ledger.surface(foreign), Err(SourceError::SourceMismatch));
        ledger.cells[0].span = foreign;
        assert!(!ledger.covers_source());
        assert!(serde_json::to_string(&ledger).is_err());
    }
}
