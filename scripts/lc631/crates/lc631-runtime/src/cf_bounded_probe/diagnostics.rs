//! Bounded rustc NDJSON validation. No diagnostic can create a capture owner.
use super::{Failure, Purpose};
use lc631_substrate::resource::ResourceReservationView;
use serde::Deserialize;

const MAX_LINE: usize = 64 * 1024;
const MAX_RECORDS: usize = 256;
const MAX_NODES: usize = 1024;
const MAX_DEPTH: usize = 16;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Diagnostic {
    #[serde(rename = "$message_type")]
    message_type: Option<String>,
    message: String,
    code: Option<DiagnosticCode>,
    level: String,
    spans: Vec<DiagnosticSpan>,
    children: Vec<Diagnostic>,
    rendered: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticCode {
    code: String,
    explanation: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticSpan {
    file_name: String,
    byte_start: u64,
    byte_end: u64,
    line_start: u64,
    line_end: u64,
    column_start: u64,
    column_end: u64,
    is_primary: bool,
    text: Vec<SpanText>,
    label: Option<String>,
    suggested_replacement: Option<String>,
    suggestion_applicability: Option<String>,
    expansion: Option<Box<Expansion>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpanText {
    text: String,
    highlight_start: u64,
    highlight_end: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expansion {
    span: DiagnosticSpan,
    macro_decl_name: String,
    def_site_span: Option<DiagnosticSpan>,
}

pub(super) fn validate(
    stderr: &[u8],
    purpose: Purpose,
    view: &ResourceReservationView<'_>,
) -> Result<(), Failure> {
    // Metadata stdout is intentionally never decoded as text.
    let text = std::str::from_utf8(stderr).map_err(|_| Failure::InvalidUtf8)?;
    if text.is_empty() {
        return match purpose {
            Purpose::PositiveControl => Ok(()),
            Purpose::NegativeCompile => Err(Failure::MissingDiagnostics),
        };
    }
    if !text.ends_with('\n') {
        return Err(Failure::PartialDiagnostic);
    }
    let mut records = 0;
    let mut nodes = 0;
    let mut errors = 0;
    for raw_line in text.split_terminator('\n') {
        view.check().map_err(Failure::Resource)?;
        records += 1;
        if records > MAX_RECORDS || raw_line.len() > MAX_LINE {
            return Err(Failure::UnknownDiagnosticSchema);
        }
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        // Bound all structural/scalar nodes before serde can allocate. The
        // private request owner retains the separately funded 8 MiB scratch.
        bounded_record(line.as_bytes(), &mut nodes)?;
        let diagnostic: Diagnostic = serde_json::from_str(line).map_err(|error| {
            if error.is_eof() {
                Failure::PartialDiagnostic
            } else {
                Failure::UnknownDiagnosticSchema
            }
        })?;
        check_diagnostic(&diagnostic, true, &mut errors)?;
        view.check().map_err(Failure::Resource)?;
    }
    match purpose {
        Purpose::NegativeCompile if errors == 0 => Err(Failure::MissingDiagnostics),
        Purpose::PositiveControl if errors != 0 => Err(Failure::UnexpectedExit(Some(0))),
        _ => Ok(()),
    }
}

fn check_diagnostic(
    diagnostic: &Diagnostic,
    root: bool,
    errors: &mut usize,
) -> Result<(), Failure> {
    match diagnostic.message_type.as_deref() {
        Some("diagnostic") => {}
        None if !root => {}
        _ => return Err(Failure::UnknownDiagnosticSchema),
    }
    if matches!(
        diagnostic.level.as_str(),
        "ice" | "error: internal compiler error"
    ) || diagnostic.message.contains("internal compiler error")
    {
        return Err(Failure::Ice);
    }
    match diagnostic.level.as_str() {
        "error" => *errors += 1,
        "warning" | "note" | "help" | "failure-note" => {}
        _ => return Err(Failure::UnknownDiagnosticSchema),
    }
    if let Some(code) = &diagnostic.code {
        if matches!(code.code.as_str(), "E0432" | "E0463") {
            return Err(Failure::MissingImport);
        }
        // Deserialization checks these optional fields even though this gate
        // does not infer semantics from explanation/rendered text.
        let _ = &code.explanation;
    }
    let _ = &diagnostic.rendered;
    for span in &diagnostic.spans {
        check_span(span)?;
    }
    for child in &diagnostic.children {
        check_diagnostic(child, false, errors)?;
    }
    Ok(())
}

fn check_span(span: &DiagnosticSpan) -> Result<(), Failure> {
    // Span fields remain transport data. The caller must compare exact source
    // identity and expected locations; this gate only accepts the known shape.
    let _ = (
        &span.file_name,
        span.byte_start,
        span.byte_end,
        span.line_start,
        span.line_end,
        span.column_start,
        span.column_end,
        span.is_primary,
        &span.label,
        &span.suggested_replacement,
    );
    for text in &span.text {
        let _ = (&text.text, text.highlight_start, text.highlight_end);
    }
    if let Some(applicability) = &span.suggestion_applicability {
        if !matches!(
            applicability.as_str(),
            "MachineApplicable" | "MaybeIncorrect" | "HasPlaceholders" | "Unspecified"
        ) {
            return Err(Failure::UnknownDiagnosticSchema);
        }
    }
    if let Some(expansion) = &span.expansion {
        let _ = &expansion.macro_decl_name;
        check_span(&expansion.span)?;
        if let Some(definition) = &expansion.def_site_span {
            check_span(definition)?;
        }
    }
    Ok(())
}

/// Allocation-free structural preflight, followed by serde's full grammar and
/// duplicate/unknown-field checks. Keys count as nodes too, conservatively.
fn bounded_record(bytes: &[u8], nodes: &mut usize) -> Result<(), Failure> {
    let mut stack = [0u8; MAX_DEPTH];
    let mut depth = 0;
    let mut index = 0;
    let mut any = false;
    while index < bytes.len() {
        match bytes[index] {
            b' ' | b'\t' | b'\r' => index += 1,
            b'{' | b'[' => {
                node(nodes)?;
                any = true;
                if depth == MAX_DEPTH {
                    return Err(Failure::UnknownDiagnosticSchema);
                }
                stack[depth] = bytes[index];
                depth += 1;
                index += 1;
            }
            b'}' | b']' => {
                let open = if bytes[index] == b'}' { b'{' } else { b'[' };
                if depth == 0 || stack[depth - 1] != open {
                    return Err(Failure::UnknownDiagnosticSchema);
                }
                depth -= 1;
                index += 1;
            }
            b',' | b':' => index += 1,
            b'"' => {
                node(nodes)?;
                any = true;
                index += 1;
                let mut closed = false;
                while index < bytes.len() {
                    match bytes[index] {
                        b'"' => {
                            index += 1;
                            closed = true;
                            break;
                        }
                        b'\\' => {
                            // Escape syntax itself is checked by serde.
                            index += 1;
                            if index == bytes.len() {
                                return Err(Failure::PartialDiagnostic);
                            }
                            index += 1;
                        }
                        0..=31 => return Err(Failure::UnknownDiagnosticSchema),
                        _ => index += 1,
                    }
                }
                if !closed {
                    return Err(Failure::PartialDiagnostic);
                }
            }
            _ => {
                node(nodes)?;
                any = true;
                index += 1;
                while index < bytes.len()
                    && !matches!(
                        bytes[index],
                        b' ' | b'\t' | b'\r' | b',' | b':' | b']' | b'}' | b'[' | b'{' | b'"'
                    )
                {
                    index += 1;
                }
            }
        }
    }
    if depth != 0 {
        return Err(Failure::PartialDiagnostic);
    }
    if !any {
        return Err(Failure::UnknownDiagnosticSchema);
    }
    Ok(())
}

fn node(nodes: &mut usize) -> Result<(), Failure> {
    *nodes += 1;
    if *nodes > MAX_NODES {
        Err(Failure::UnknownDiagnosticSchema)
    } else {
        Ok(())
    }
}
