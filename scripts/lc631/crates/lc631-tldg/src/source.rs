use lc631_core::{stable_sha256, SourceSpan};
use serde::Serialize;

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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryClass {
    Alphanumeric,
    Whitespace,
    Punctuation,
    Symbol,
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
            let class = if character.is_alphanumeric() || character == '_' {
                BoundaryClass::Alphanumeric
            } else if character.is_whitespace() {
                BoundaryClass::Whitespace
            } else if character.is_ascii_punctuation() {
                BoundaryClass::Punctuation
            } else {
                BoundaryClass::Symbol
            };
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
        if self.source.byte_len == 0 {
            return self.cells.is_empty();
        }
        self.cells.first().is_some_and(|cell| cell.span.start == 0)
            && self
                .cells
                .last()
                .is_some_and(|cell| cell.span.end == self.source.byte_len)
            && self
                .cells
                .windows(2)
                .all(|pair| pair[0].span.end == pair[1].span.start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_boundary_ledger_roundtrips() {
        let source = "日本語 + Rust\n";
        let ledger = BoundaryLedger::build(source);
        assert_eq!(ledger.roundtrip(), source);
        assert!(ledger.covers_source());
    }
}
