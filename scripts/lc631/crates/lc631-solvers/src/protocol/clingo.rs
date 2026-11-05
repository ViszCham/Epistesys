//! Bounded, source-span-preserving views of Clingo answer text.
//!
//! This parser recognizes only answer numbering, raw model-content line spans
//! and one terminal solver status. It does not parse atom syntax, establish
//! atom truth, check a model, or infer brave/cautious entailment.

use super::ValidationBasis;
use crate::registry::MAX_SOLVER_OUTPUT_BYTES;

mod parser;
pub use parser::parse_clingo_output;

pub const MAX_CLINGO_ANSWER_BLOCKS: usize = 256;
pub const MAX_CLINGO_ATOM_LINES: usize = 16_384;
pub const MAX_CLINGO_REPORT_ANSWER_SPANS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClingoTerminalStatus {
    Satisfiable,
    Unsatisfiable,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClingoOutputError {
    InvalidLimits,
    LimitsTooLarge,
    OutputTooLarge,
    InvalidUtf8,
    MissingTerminalStatus,
    DuplicateStatus,
    ContradictoryAnswerStatus,
    StatusOutOfOrder,
    UnsupportedStatus,
    MalformedAnswerHeader,
    MalformedAnswerNumber,
    AnswerNumberOutOfOrder,
    TooManyAnswers,
    TooManyAtomLines,
    UnexpectedAfterStatus,
    InvalidSpan,
    SummaryCountOverflow,
    AllocationFailed,
}

/// Byte offsets into the exact UTF-8 stdout buffer supplied by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClingoSourceSpan {
    pub start: usize,
    pub end: usize,
}

/// Per-output caps are immutable once parsing starts. Zero answer/atom limits
/// permit status-only protocols, such as an UNSAT result with no answer block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClingoOutputLimits {
    max_output_bytes: usize,
    max_answers: usize,
    max_atom_lines: usize,
}

impl ClingoOutputLimits {
    pub fn checked(
        max_output_bytes: usize,
        max_answers: usize,
        max_atom_lines: usize,
    ) -> Result<Self, ClingoOutputError> {
        if max_output_bytes == 0 {
            return Err(ClingoOutputError::InvalidLimits);
        }
        if max_output_bytes > MAX_SOLVER_OUTPUT_BYTES
            || max_answers > MAX_CLINGO_ANSWER_BLOCKS
            || max_atom_lines > MAX_CLINGO_ATOM_LINES
        {
            return Err(ClingoOutputError::LimitsTooLarge);
        }
        Ok(Self {
            max_output_bytes,
            max_answers,
            max_atom_lines,
        })
    }

    pub const fn max_output_bytes(self) -> usize {
        self.max_output_bytes
    }

    pub const fn max_answers(self) -> usize {
        self.max_answers
    }

    pub const fn max_atom_lines(self) -> usize {
        self.max_atom_lines
    }
}

impl Default for ClingoOutputLimits {
    fn default() -> Self {
        Self {
            max_output_bytes: MAX_SOLVER_OUTPUT_BYTES,
            max_answers: MAX_CLINGO_ANSWER_BLOCKS,
            max_atom_lines: MAX_CLINGO_ATOM_LINES,
        }
    }
}

/// A raw nonblank content line inside an `Answer: n` block. Its syntax and
/// truth value are intentionally not interpreted by this type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClingoAtomLineSpan {
    span: ClingoSourceSpan,
}

impl ClingoAtomLineSpan {
    pub const fn span(self) -> ClingoSourceSpan {
        self.span
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClingoAnswerBlockSpan {
    number: u32,
    header_span: ClingoSourceSpan,
    block_span: ClingoSourceSpan,
    first_atom: usize,
    atom_count: usize,
}

/// Fixed-size summary safe to retain in a solver report. It preserves only
/// counts and the first bounded set of answer spans, never raw output bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClingoAnswerSpanSummary {
    number: u32,
    span: ClingoSourceSpan,
}

impl ClingoAnswerSpanSummary {
    pub const fn number(self) -> u32 {
        self.number
    }

    pub const fn span(self) -> ClingoSourceSpan {
        self.span
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClingoOutputSummary {
    status: ClingoTerminalStatus,
    answer_count: u32,
    atom_line_count: u32,
    spans: [Option<ClingoAnswerSpanSummary>; MAX_CLINGO_REPORT_ANSWER_SPANS],
    recorded_span_count: u8,
    spans_truncated: bool,
}

impl ClingoOutputSummary {
    pub const fn status(self) -> ClingoTerminalStatus {
        self.status
    }

    pub const fn answer_count(self) -> u32 {
        self.answer_count
    }

    pub const fn atom_line_count(self) -> u32 {
        self.atom_line_count
    }

    pub const fn recorded_answer_span_count(self) -> u8 {
        self.recorded_span_count
    }

    pub fn answer_span(self, index: usize) -> Option<ClingoAnswerSpanSummary> {
        if index >= usize::from(self.recorded_span_count) {
            return None;
        }
        self.spans.get(index).copied().flatten()
    }

    pub const fn spans_truncated(self) -> bool {
        self.spans_truncated
    }
}

impl ClingoAnswerBlockSpan {
    pub const fn number(self) -> u32 {
        self.number
    }

    pub const fn header_span(self) -> ClingoSourceSpan {
        self.header_span
    }

    pub const fn block_span(self) -> ClingoSourceSpan {
        self.block_span
    }

    pub const fn atom_count(self) -> usize {
        self.atom_count
    }
}

/// Bounded borrowed view. The source bytes remain owned by the caller, and all
/// exposed spans refer to those exact UTF-8 bytes rather than reserialized text.
pub struct ClingoOutputView<'a> {
    source: &'a str,
    status: ClingoTerminalStatus,
    status_span: ClingoSourceSpan,
    answers: Vec<ClingoAnswerBlockSpan>,
    atom_lines: Vec<ClingoAtomLineSpan>,
}

impl<'a> ClingoOutputView<'a> {
    pub fn source(&self) -> &'a str {
        self.source
    }

    pub const fn status(&self) -> ClingoTerminalStatus {
        self.status
    }

    pub const fn status_span(&self) -> ClingoSourceSpan {
        self.status_span
    }

    pub const fn validation_basis(&self) -> ValidationBasis {
        ValidationBasis::SolverReported
    }

    pub fn answer_blocks(&self) -> &[ClingoAnswerBlockSpan] {
        &self.answers
    }

    pub fn atom_lines_for(&self, answer: &ClingoAnswerBlockSpan) -> Option<&[ClingoAtomLineSpan]> {
        let end = answer.first_atom.checked_add(answer.atom_count)?;
        self.atom_lines.get(answer.first_atom..end)
    }

    pub fn summary(&self) -> Result<ClingoOutputSummary, ClingoOutputError> {
        let answer_count = u32::try_from(self.answers.len())
            .map_err(|_| ClingoOutputError::SummaryCountOverflow)?;
        let atom_line_count = u32::try_from(self.atom_lines.len())
            .map_err(|_| ClingoOutputError::SummaryCountOverflow)?;
        let mut spans = [None; MAX_CLINGO_REPORT_ANSWER_SPANS];
        let recorded = self.answers.len().min(MAX_CLINGO_REPORT_ANSWER_SPANS);
        for (slot, answer) in spans.iter_mut().zip(self.answers.iter().take(recorded)) {
            *slot = Some(ClingoAnswerSpanSummary {
                number: answer.number,
                span: answer.block_span,
            });
        }
        let recorded_span_count =
            u8::try_from(recorded).map_err(|_| ClingoOutputError::SummaryCountOverflow)?;
        Ok(ClingoOutputSummary {
            status: self.status,
            answer_count,
            atom_line_count,
            spans,
            recorded_span_count,
            spans_truncated: self.answers.len() > recorded,
        })
    }

    pub fn text(&self, span: ClingoSourceSpan) -> Option<&'a str> {
        self.source.get(span.start..span.end)
    }

    pub const fn claim_boundary(&self) -> &'static str {
        "bounded raw spans and one solver-reported status only; no atom truth, no model validation, no brave/cautious entailment, no proof, no Authority, no commit"
    }
}

#[derive(Clone, Copy)]
struct PendingAnswer {
    number: u32,
    start: usize,
    header_span: ClingoSourceSpan,
    first_atom: usize,
}

fn checked_span(
    source: &str,
    start: usize,
    end: usize,
) -> Result<ClingoSourceSpan, ClingoOutputError> {
    if start > end
        || end > source.len()
        || !source.is_char_boundary(start)
        || !source.is_char_boundary(end)
    {
        return Err(ClingoOutputError::InvalidSpan);
    }
    Ok(ClingoSourceSpan { start, end })
}
