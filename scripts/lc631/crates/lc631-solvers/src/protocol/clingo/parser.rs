use super::{
    checked_span, ClingoAnswerBlockSpan, ClingoAtomLineSpan, ClingoOutputError, ClingoOutputLimits,
    ClingoOutputView, ClingoTerminalStatus, PendingAnswer,
};

/// Parse bounded Clingo stdout without interpreting atom/model semantics.
///
/// Leading solver banners are ignored. Answer blocks must be numbered
/// consecutively starting at one. A single status line must occur after all
/// answer blocks; only blank or recognized summary lines may follow it.
pub fn parse_clingo_output<'a>(
    output: &'a [u8],
    limits: ClingoOutputLimits,
) -> Result<ClingoOutputView<'a>, ClingoOutputError> {
    if output.len() > limits.max_output_bytes {
        return Err(ClingoOutputError::OutputTooLarge);
    }
    let source = std::str::from_utf8(output).map_err(|_| ClingoOutputError::InvalidUtf8)?;
    let mut answers = Vec::new();
    let mut atom_lines = Vec::new();
    let mut current_answer: Option<PendingAnswer> = None;
    let mut terminal_status = None;
    let mut terminal_span = None;

    for line in SourceLines::new(source) {
        let trimmed = trim_ascii_space(line.text);
        if trimmed.is_empty() {
            continue;
        }

        if let Some(status) = parse_status(trimmed) {
            if terminal_status.is_some() {
                return Err(ClingoOutputError::DuplicateStatus);
            }
            if let Some(pending) = current_answer.take() {
                finish_answer(source, pending, line.start, &atom_lines, &mut answers)?;
            }
            terminal_status = Some(status);
            let status_end = line
                .start
                .checked_add(line.text.len())
                .ok_or(ClingoOutputError::InvalidSpan)?;
            terminal_span = Some(checked_span(source, line.start, status_end)?);
            continue;
        }
        if is_unsupported_status(trimmed) {
            return Err(ClingoOutputError::UnsupportedStatus);
        }

        match parse_answer_header(trimmed)? {
            Some(number) => {
                if terminal_status.is_some() {
                    return Err(ClingoOutputError::StatusOutOfOrder);
                }
                if let Some(pending) = current_answer.take() {
                    finish_answer(source, pending, line.start, &atom_lines, &mut answers)?;
                }
                if answers.len() >= limits.max_answers {
                    return Err(ClingoOutputError::TooManyAnswers);
                }
                let expected = u32::try_from(answers.len())
                    .ok()
                    .and_then(|count| count.checked_add(1))
                    .ok_or(ClingoOutputError::AnswerNumberOutOfOrder)?;
                if number != expected {
                    return Err(ClingoOutputError::AnswerNumberOutOfOrder);
                }
                let header_end = line
                    .start
                    .checked_add(line.text.len())
                    .ok_or(ClingoOutputError::InvalidSpan)?;
                current_answer = Some(PendingAnswer {
                    number,
                    start: line.start,
                    header_span: checked_span(source, line.start, header_end)?,
                    first_atom: atom_lines.len(),
                });
                continue;
            }
            None if is_answer_like(trimmed) => {
                if terminal_status.is_some() {
                    return Err(ClingoOutputError::StatusOutOfOrder);
                }
                return Err(ClingoOutputError::MalformedAnswerHeader);
            }
            None => {}
        }

        if terminal_status.is_some() {
            if is_summary_line(trimmed) {
                continue;
            }
            return Err(ClingoOutputError::UnexpectedAfterStatus);
        }

        if current_answer.is_some() && !is_summary_line(trimmed) {
            if atom_lines.len() >= limits.max_atom_lines {
                return Err(ClingoOutputError::TooManyAtomLines);
            }
            atom_lines
                .try_reserve(1)
                .map_err(|_| ClingoOutputError::AllocationFailed)?;
            let end = line
                .start
                .checked_add(line.text.len())
                .ok_or(ClingoOutputError::InvalidSpan)?;
            atom_lines.push(ClingoAtomLineSpan {
                span: checked_span(source, line.start, end)?,
            });
        }
    }

    let status = terminal_status.ok_or(ClingoOutputError::MissingTerminalStatus)?;
    let status_span = terminal_span.ok_or(ClingoOutputError::MissingTerminalStatus)?;
    if let Some(pending) = current_answer {
        finish_answer(source, pending, source.len(), &atom_lines, &mut answers)?;
    }
    if status == ClingoTerminalStatus::Unsatisfiable && !answers.is_empty() {
        return Err(ClingoOutputError::ContradictoryAnswerStatus);
    }

    Ok(ClingoOutputView {
        source,
        status,
        status_span,
        answers,
        atom_lines,
    })
}

fn finish_answer(
    source: &str,
    pending: PendingAnswer,
    end: usize,
    atoms: &[ClingoAtomLineSpan],
    answers: &mut Vec<ClingoAnswerBlockSpan>,
) -> Result<(), ClingoOutputError> {
    if answers.len() == answers.capacity() {
        answers
            .try_reserve(1)
            .map_err(|_| ClingoOutputError::AllocationFailed)?;
    }
    let atom_count = atoms
        .len()
        .checked_sub(pending.first_atom)
        .ok_or(ClingoOutputError::InvalidSpan)?;
    answers.push(ClingoAnswerBlockSpan {
        number: pending.number,
        header_span: pending.header_span,
        block_span: checked_span(source, pending.start, end)?,
        first_atom: pending.first_atom,
        atom_count,
    });
    Ok(())
}

fn parse_answer_header(line: &str) -> Result<Option<u32>, ClingoOutputError> {
    if !is_answer_like(line) {
        return Ok(None);
    }
    let remainder = line
        .strip_prefix("Answer:")
        .ok_or(ClingoOutputError::MalformedAnswerHeader)?;
    let remainder = trim_ascii_space(remainder);
    // Clingo/clasp's standard answer header may include elapsed wall time.
    // It remains uninterpreted diagnostic metadata; do not parse it as a
    // floating-point score, semantic evidence, or an answer number suffix.
    let (digits, suffix) = match remainder.find(|byte: char| byte.is_ascii_whitespace()) {
        Some(index) => (&remainder[..index], trim_ascii_space(&remainder[index..])),
        None => (remainder, ""),
    };
    if !suffix.is_empty() && !valid_elapsed_annotation(suffix) {
        return Err(ClingoOutputError::MalformedAnswerHeader);
    }
    if digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
    {
        return Err(ClingoOutputError::MalformedAnswerNumber);
    }
    let number = digits
        .parse::<u32>()
        .map_err(|_| ClingoOutputError::MalformedAnswerNumber)?;
    if number == 0 {
        return Err(ClingoOutputError::MalformedAnswerNumber);
    }
    Ok(Some(number))
}

fn valid_elapsed_annotation(suffix: &str) -> bool {
    let Some(seconds) = suffix
        .strip_prefix("(Time: ")
        .and_then(|text| text.strip_suffix("s)"))
    else {
        return false;
    };
    // Match a nonnegative ASCII decimal exactly, without accepting NaN,
    // infinity, exponent notation, other units, or appended metadata.
    let (whole, fraction) = match seconds.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (seconds, None),
    };
    !whole.is_empty()
        && whole.bytes().all(|byte| byte.is_ascii_digit())
        && fraction.is_none_or(|digits| {
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn parse_status(line: &str) -> Option<ClingoTerminalStatus> {
    match line {
        "SATISFIABLE" => Some(ClingoTerminalStatus::Satisfiable),
        "UNSATISFIABLE" => Some(ClingoTerminalStatus::Unsatisfiable),
        "UNKNOWN" => Some(ClingoTerminalStatus::Unknown),
        _ => None,
    }
}

fn is_unsupported_status(line: &str) -> bool {
    matches!(line, "OPTIMUM FOUND")
        || line.starts_with("SATISFIABLE ")
        || line.starts_with("UNSATISFIABLE ")
        || line.starts_with("UNKNOWN ")
}

fn is_answer_like(line: &str) -> bool {
    line.starts_with("Answer")
}

fn is_summary_line(line: &str) -> bool {
    [
        "Models",
        "Calls",
        "Time",
        "CPU Time",
        "Optimization",
        "Optimum",
    ]
    .iter()
    .any(|prefix| {
        line.starts_with(prefix)
            && line
                .as_bytes()
                .get(prefix.len())
                .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b':')
    })
}

fn trim_ascii_space(line: &str) -> &str {
    line.trim_matches(|character| character == ' ' || character == '\t' || character == '\r')
}

struct SourceLine<'a> {
    start: usize,
    text: &'a str,
}

struct SourceLines<'a> {
    source: &'a str,
    next_start: usize,
}

impl<'a> SourceLines<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            next_start: 0,
        }
    }
}

impl<'a> Iterator for SourceLines<'a> {
    type Item = SourceLine<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_start >= self.source.len() {
            return None;
        }
        let start = self.next_start;
        let bytes = self.source.as_bytes();
        let newline_offset = bytes[start..].iter().position(|byte| *byte == b'\n');
        let (line_end, next_start) = match newline_offset {
            Some(offset) => {
                let line_end = start.checked_add(offset)?;
                (line_end, line_end.checked_add(1)?)
            }
            None => (bytes.len(), bytes.len()),
        };
        let content_end = if line_end > start && bytes[line_end - 1] == b'\r' {
            line_end - 1
        } else {
            line_end
        };
        let text = self.source.get(start..content_end)?;
        self.next_start = next_start;
        Some(SourceLine { start, text })
    }
}
