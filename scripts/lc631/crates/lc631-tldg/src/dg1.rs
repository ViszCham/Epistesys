use crate::BoundaryLedger;
use lc631_core::stable_sha256;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use serde::Serialize;

pub const DG1_SCHEMA: &str = "epistesys-dg1-parse.v1";
const DOCUMENT_GRAMMAR_REVISION: &str = "commonmark+tables-pulldown-cmark-0.13.4";
const RUST_GRAMMAR_REVISION: &str = "tree-sitter-rust-0.24.2-tree-sitter-0.27.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct Dg1Budget {
    pub max_source_bytes: usize,
    pub max_region_depth: usize,
    pub max_regions: usize,
    pub max_syntax_nodes: usize,
    pub max_requirements: usize,
    pub max_alternatives: usize,
}

impl Default for Dg1Budget {
    fn default() -> Self {
        Self {
            max_source_bytes: 262_144,
            max_region_depth: 64,
            max_regions: 100_000,
            max_syntax_nodes: 100_000,
            max_requirements: 1_024,
            max_alternatives: 32,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dg1Language {
    Japanese,
    English,
    MixedJapaneseEnglish,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dg1RegionKind {
    Document,
    Paragraph,
    Heading,
    BlockQuote,
    List,
    ListItem,
    Link,
    Emphasis,
    Strong,
    Table,
    TableRow,
    TableCell,
    Prose,
    InlineCode,
    FencedCode,
    IndentedCode,
    OpaqueEmbedded,
    OpaqueHtmlBlock,
    OpaqueHtml,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dg1ContentRole {
    OperativeProse,
    Quotation,
    Code,
    Opaque,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Dg1Region {
    pub id: u32,
    pub parent: Option<u32>,
    pub kind: Dg1RegionKind,
    pub content_role: Dg1ContentRole,
    pub span: std::ops::Range<usize>,
    pub language: Dg1Language,
    pub dialect: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Dg1SyntaxNode {
    pub id: u32,
    pub parent_id: Option<u32>,
    pub kind: String,
    pub named: bool,
    pub span: std::ops::Range<usize>,
    pub has_error: bool,
    pub is_error: bool,
    pub missing: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Dg1SyntaxTree {
    pub region_id: u32,
    pub grammar_revision: String,
    pub root_has_error: bool,
    pub nodes: Vec<Dg1SyntaxNode>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dg1Status {
    ParsedWithinBoundedProfiles,
    ParsedWithSyntaxErrors,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dg1RequirementStrength {
    Must,
    Should,
    May,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dg1RequirementPolarity {
    Positive,
    Negative,
    Unknown,
    Conflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dg1ConditionKind {
    Necessary,
    Conditional,
    Temporal,
    Exception,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dg1ResidualReason {
    NoSupportedInstructionMarker,
    QuotedText,
    MixedOrUnknownLanguage,
    UnresolvedReference,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Dg1RequirementCandidate {
    pub id: u32,
    pub source_span: std::ops::Range<usize>,
    pub source_text: String,
    pub language: Dg1Language,
    pub strength: Dg1RequirementStrength,
    pub polarity: Dg1RequirementPolarity,
    pub conditional: bool,
    pub exception_present: bool,
    pub condition_kind: Option<Dg1ConditionKind>,
    pub scope: Option<String>,
    pub unresolved_reference: bool,
    pub authority_grant: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Dg1InstructionResidual {
    pub source_span: std::ops::Range<usize>,
    pub source_text: String,
    pub language: Dg1Language,
    pub reason: Dg1ResidualReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dg1SourceRole {
    InstructionCandidate,
    Context,
    Quoted,
    Code,
    Opaque,
    Unsupported,
    Ambiguous,
    MarkdownSyntax,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Dg1SourceAccountingSegment {
    pub source_span: std::ops::Range<usize>,
    pub role: Dg1SourceRole,
    pub region_id: Option<u32>,
    pub residual_reason: Option<Dg1ResidualReason>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Dg1LanguageSpan {
    pub id: u32,
    pub region_id: u32,
    pub source_span: std::ops::Range<usize>,
    pub language: Dg1Language,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Dg1Report {
    pub schema_version: &'static str,
    pub source_revision: String,
    pub source_bytes: usize,
    pub exact_source_roundtrip: bool,
    pub regions: Vec<Dg1Region>,
    pub rust_trees: Vec<Dg1SyntaxTree>,
    pub requirement_candidates: Vec<Dg1RequirementCandidate>,
    pub instruction_residuals: Vec<Dg1InstructionResidual>,
    pub source_accounting: Vec<Dg1SourceAccountingSegment>,
    pub language_span_lattice: Vec<Dg1LanguageSpan>,
    pub status: Dg1Status,
    pub semantic_backend_state: &'static str,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Dg1Error {
    SourceBudgetExceeded,
    RegionBudgetExceeded,
    RegionDepthExceeded,
    SyntaxNodeBudgetExceeded,
    RequirementBudgetExceeded,
    AlternativeBudgetExceeded,
    SourceOffsetOverflow,
    RustParserUnavailable,
}

#[derive(Clone, Debug)]
struct OpenCodeBlock {
    language: String,
    start: Option<usize>,
    end: Option<usize>,
    empty_offset: usize,
    parent: u32,
    kind: Dg1RegionKind,
}

#[derive(Clone, Debug)]
struct OpenMarkdownRegion {
    id: u32,
    end_tag: TagEnd,
    start: usize,
    end: usize,
}

pub fn analyze_dg1(source: &str, budget: Dg1Budget) -> Result<Dg1Report, Dg1Error> {
    if budget.max_regions == 0 {
        return Err(Dg1Error::RegionBudgetExceeded);
    }
    if budget.max_alternatives == 0 {
        return Err(Dg1Error::AlternativeBudgetExceeded);
    }
    if source.len() > budget.max_source_bytes {
        return Err(Dg1Error::SourceBudgetExceeded);
    }
    if source.len() > u32::MAX as usize {
        return Err(Dg1Error::SourceOffsetOverflow);
    }

    let mut regions = vec![Dg1Region {
        id: 0,
        parent: None,
        kind: Dg1RegionKind::Document,
        content_role: Dg1ContentRole::Unknown,
        span: 0..source.len(),
        language: Dg1Language::Unknown,
        dialect: Some(DOCUMENT_GRAMMAR_REVISION.into()),
    }];
    let mut trees = Vec::new();
    let mut code_block: Option<OpenCodeBlock> = None;
    let mut markdown_regions = Vec::<OpenMarkdownRegion>::new();
    let mut depth = 0usize;
    let mut syntax_count = 0usize;

    for (event, range) in Parser::new_ext(source, Options::ENABLE_TABLES).into_offset_iter() {
        extend_markdown_regions(&mut markdown_regions, &mut regions, range.clone());
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                depth = depth.checked_add(1).ok_or(Dg1Error::RegionDepthExceeded)?;
                if depth > budget.max_region_depth {
                    return Err(Dg1Error::RegionDepthExceeded);
                }
                let (language, region_kind) = match kind {
                    CodeBlockKind::Fenced(info) => (
                        info.split_whitespace()
                            .next()
                            .unwrap_or("")
                            .to_ascii_lowercase(),
                        Dg1RegionKind::FencedCode,
                    ),
                    CodeBlockKind::Indented => (String::new(), Dg1RegionKind::IndentedCode),
                };
                code_block = Some(OpenCodeBlock {
                    language,
                    start: None,
                    end: None,
                    empty_offset: range.end,
                    parent: markdown_regions.last().map_or(0, |region| region.id),
                    kind: region_kind,
                });
            }
            Event::Start(tag) => {
                depth = depth.checked_add(1).ok_or(Dg1Error::RegionDepthExceeded)?;
                if depth > budget.max_region_depth {
                    return Err(Dg1Error::RegionDepthExceeded);
                }
                if let Some((kind, end_tag)) = markdown_region_for_tag(&tag) {
                    let id = push_region(
                        &mut regions,
                        budget,
                        Dg1Region {
                            id: 0,
                            parent: Some(markdown_regions.last().map_or(0, |region| region.id)),
                            kind,
                            content_role: match kind {
                                Dg1RegionKind::OpaqueHtmlBlock => Dg1ContentRole::Opaque,
                                _ => Dg1ContentRole::Unknown,
                            },
                            span: range.clone(),
                            language: Dg1Language::Unknown,
                            dialect: None,
                        },
                    )?;
                    markdown_regions.push(OpenMarkdownRegion {
                        id,
                        end_tag,
                        start: range.start,
                        end: range.end,
                    });
                }
                extend_markdown_regions(&mut markdown_regions, &mut regions, range.clone());
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(block) = code_block.take() {
                    let start = block.start.unwrap_or(block.empty_offset);
                    let end = block.end.unwrap_or(block.empty_offset);
                    let id = push_region(
                        &mut regions,
                        budget,
                        Dg1Region {
                            id: 0,
                            parent: Some(block.parent),
                            kind: block.kind,
                            content_role: Dg1ContentRole::Code,
                            span: start..end,
                            language: Dg1Language::Unknown,
                            dialect: (!block.language.is_empty()).then_some(block.language.clone()),
                        },
                    )?;
                    if block.language == "rust" || block.language == "rs" {
                        let text = source
                            .get(start..end)
                            .ok_or(Dg1Error::SourceOffsetOverflow)?;
                        let tree = parse_rust_region(id, start, text, budget, &mut syntax_count)?;
                        trees.push(tree);
                    }
                }
                extend_markdown_regions(&mut markdown_regions, &mut regions, range.clone());
                depth = depth.saturating_sub(1);
            }
            Event::End(end_tag) => {
                extend_markdown_regions(&mut markdown_regions, &mut regions, range.clone());
                if markdown_regions
                    .last()
                    .is_some_and(|region| region.end_tag == end_tag)
                {
                    markdown_regions.pop();
                }
                depth = depth.saturating_sub(1);
            }
            Event::Text(_) if code_block.is_some() => {
                if let Some(block) = &mut code_block {
                    block.start = Some(
                        block
                            .start
                            .map_or(range.start, |start| start.min(range.start)),
                    );
                    block.end = Some(block.end.map_or(range.end, |end| end.max(range.end)));
                }
            }
            Event::Text(text) => {
                if !text.is_empty() {
                    let in_block_quote = markdown_regions.iter().any(|open| {
                        regions
                            .get(open.id as usize)
                            .is_some_and(|region| region.kind == Dg1RegionKind::BlockQuote)
                    });
                    push_region(
                        &mut regions,
                        budget,
                        Dg1Region {
                            id: 0,
                            parent: Some(markdown_regions.last().map_or(0, |region| region.id)),
                            kind: Dg1RegionKind::Prose,
                            content_role: if in_block_quote {
                                Dg1ContentRole::Quotation
                            } else {
                                Dg1ContentRole::OperativeProse
                            },
                            span: range,
                            language: classify_language(&text),
                            dialect: None,
                        },
                    )?;
                }
            }
            Event::Code(text) => {
                if !text.is_empty() {
                    push_region(
                        &mut regions,
                        budget,
                        Dg1Region {
                            id: 0,
                            parent: Some(markdown_regions.last().map_or(0, |region| region.id)),
                            kind: Dg1RegionKind::InlineCode,
                            content_role: Dg1ContentRole::Code,
                            span: range,
                            language: Dg1Language::Unknown,
                            dialect: None,
                        },
                    )?;
                }
            }
            Event::Html(_) | Event::InlineHtml(_) => {
                push_region(
                    &mut regions,
                    budget,
                    Dg1Region {
                        id: 0,
                        parent: Some(markdown_regions.last().map_or(0, |region| region.id)),
                        kind: Dg1RegionKind::OpaqueHtml,
                        content_role: Dg1ContentRole::Opaque,
                        span: range,
                        language: Dg1Language::Unknown,
                        dialect: Some("not_executed".into()),
                    },
                )?;
            }
            _ => {}
        }
    }

    if looks_like_rust_source(source) {
        regions.retain(|region| region.id == 0);
        if let Some(root) = regions.first_mut() {
            root.dialect = Some("rust".into());
        }
        trees.clear();
        syntax_count = 0;
        trees.push(parse_rust_region(0, 0, source, budget, &mut syntax_count)?);
    }

    let opaque_macros = trees
        .iter()
        .flat_map(|tree| {
            tree.nodes
                .iter()
                .filter(|node| node.kind == "macro_invocation")
                .filter_map(|node| {
                    let snippet = source.get(node.span.clone())?.trim_start();
                    snippet
                        .starts_with("sql!")
                        .then_some((tree.region_id, node.span.clone()))
                })
        })
        .collect::<Vec<_>>();
    for (parent, span) in opaque_macros {
        push_region(
            &mut regions,
            budget,
            Dg1Region {
                id: 0,
                parent: Some(parent),
                kind: Dg1RegionKind::OpaqueEmbedded,
                content_role: Dg1ContentRole::Opaque,
                span,
                language: Dg1Language::Unknown,
                dialect: Some("sql_macro_unavailable".into()),
            },
        )?;
    }

    if regions.len().saturating_sub(1).saturating_add(syntax_count) > budget.max_syntax_nodes {
        return Err(Dg1Error::SyntaxNodeBudgetExceeded);
    }

    let (requirement_candidates, instruction_residuals) =
        extract_requirements(source, &regions, budget)?;
    let source_accounting = build_source_accounting(
        source,
        &regions,
        &requirement_candidates,
        &instruction_residuals,
        budget,
    )?;
    let language_span_lattice = build_language_span_lattice(source, &regions, budget)?;
    if regions
        .len()
        .saturating_add(syntax_count)
        .saturating_add(source_accounting.len())
        .saturating_add(language_span_lattice.len())
        > budget.max_syntax_nodes
    {
        return Err(Dg1Error::SyntaxNodeBudgetExceeded);
    }
    let status = if trees.iter().any(|tree| tree.root_has_error) {
        Dg1Status::ParsedWithSyntaxErrors
    } else {
        Dg1Status::ParsedWithinBoundedProfiles
    };
    let boundary = BoundaryLedger::build(source);
    let exact_source_roundtrip = boundary.covers_source() && boundary.roundtrip() == source;
    Ok(Dg1Report {
        schema_version: DG1_SCHEMA,
        source_revision: stable_sha256(source),
        source_bytes: source.len(),
        exact_source_roundtrip,
        regions,
        rust_trees: trees,
        requirement_candidates,
        instruction_residuals,
        source_accounting,
        language_span_lattice,
        status,
        semantic_backend_state: "unavailable_requires_explicit_external_backend",
        claim_boundary: "bounded CommonMark plus table-extension regions and Rust CST; language spans are script hints, not language semantics, type checking, authority, or correctness",
    })
}

fn build_source_accounting(
    source: &str,
    regions: &[Dg1Region],
    candidates: &[Dg1RequirementCandidate],
    residuals: &[Dg1InstructionResidual],
    budget: Dg1Budget,
) -> Result<Vec<Dg1SourceAccountingSegment>, Dg1Error> {
    let quoted = inline_quote_spans(source, regions);
    let mut boundaries = vec![0, source.len()];
    for span in regions
        .iter()
        .map(|region| &region.span)
        .chain(candidates.iter().map(|item| &item.source_span))
        .chain(residuals.iter().map(|item| &item.source_span))
        .chain(quoted.iter().map(|(span, _)| span))
    {
        if span.start > span.end || span.end > source.len() || source.get(span.clone()).is_none() {
            return Err(Dg1Error::SourceOffsetOverflow);
        }
        boundaries.push(span.start);
        boundaries.push(span.end);
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let mut segments = Vec::<Dg1SourceAccountingSegment>::new();
    for pair in boundaries.windows(2) {
        let start = pair[0];
        let end = pair[1];
        if start == end {
            continue;
        }
        if source.get(start..end).is_none() {
            return Err(Dg1Error::SourceOffsetOverflow);
        }
        let interval = start..end;
        let quote = quoted.iter().find(|(span, _)| contains(span, &interval));
        let candidate = candidates
            .iter()
            .find(|item| contains(&item.source_span, &interval));
        let residual = residuals
            .iter()
            .find(|item| contains(&item.source_span, &interval));
        let region = regions
            .iter()
            .filter(|region| contains(&region.span, &interval))
            .filter(|region| {
                matches!(
                    region.kind,
                    Dg1RegionKind::Prose
                        | Dg1RegionKind::InlineCode
                        | Dg1RegionKind::FencedCode
                        | Dg1RegionKind::IndentedCode
                        | Dg1RegionKind::OpaqueEmbedded
                        | Dg1RegionKind::OpaqueHtmlBlock
                        | Dg1RegionKind::OpaqueHtml
                )
            })
            .min_by_key(|region| region.span.end - region.span.start);

        let (role, region_id, residual_reason) = if let Some((_, region_id)) = quote {
            (Dg1SourceRole::Quoted, Some(*region_id), None)
        } else if let Some(item) = candidate {
            let role = if item.unresolved_reference
                || matches!(
                    item.polarity,
                    Dg1RequirementPolarity::Unknown | Dg1RequirementPolarity::Conflict
                ) {
                Dg1SourceRole::Ambiguous
            } else {
                Dg1SourceRole::InstructionCandidate
            };
            (role, region_for_span(regions, &item.source_span), None)
        } else if let Some(item) = residual {
            let role = match item.reason {
                Dg1ResidualReason::QuotedText => Dg1SourceRole::Quoted,
                Dg1ResidualReason::MixedOrUnknownLanguage => Dg1SourceRole::Unsupported,
                Dg1ResidualReason::UnresolvedReference => Dg1SourceRole::Ambiguous,
                Dg1ResidualReason::NoSupportedInstructionMarker => Dg1SourceRole::Context,
            };
            (
                role,
                region_for_span(regions, &item.source_span),
                Some(item.reason),
            )
        } else if let Some(region) = region {
            let role = match region.content_role {
                Dg1ContentRole::Quotation => Dg1SourceRole::Quoted,
                Dg1ContentRole::Code => Dg1SourceRole::Code,
                Dg1ContentRole::Opaque => Dg1SourceRole::Opaque,
                Dg1ContentRole::OperativeProse => Dg1SourceRole::Context,
                Dg1ContentRole::Unknown => Dg1SourceRole::MarkdownSyntax,
            };
            (role, Some(region.id), None)
        } else {
            (Dg1SourceRole::MarkdownSyntax, None, None)
        };

        if let Some(previous) = segments.last_mut() {
            if previous.source_span.end == start
                && previous.role == role
                && previous.region_id == region_id
                && previous.residual_reason == residual_reason
            {
                previous.source_span.end = end;
                continue;
            }
        }
        segments.push(Dg1SourceAccountingSegment {
            source_span: interval,
            role,
            region_id,
            residual_reason,
        });
        if segments.len() > budget.max_syntax_nodes {
            return Err(Dg1Error::SyntaxNodeBudgetExceeded);
        }
    }
    Ok(segments)
}

fn build_language_span_lattice(
    source: &str,
    regions: &[Dg1Region],
    budget: Dg1Budget,
) -> Result<Vec<Dg1LanguageSpan>, Dg1Error> {
    let mut spans = Vec::new();
    for region in regions
        .iter()
        .filter(|region| region.kind == Dg1RegionKind::Prose)
    {
        let text = source
            .get(region.span.clone())
            .ok_or(Dg1Error::SourceOffsetOverflow)?;
        let has_japanese_kana = text
            .chars()
            .any(|character| matches!(character as u32, 0x3040..=0x30ff));
        let mut current_language = Dg1Language::Unknown;
        let mut run_start = 0usize;
        for (offset, character) in text.char_indices() {
            let Some(observed) = character_language(character, has_japanese_kana) else {
                continue;
            };
            if observed == current_language {
                continue;
            }
            if offset > run_start {
                push_language_span(
                    &mut spans,
                    region,
                    run_start,
                    offset,
                    current_language,
                    budget,
                )?;
            }
            run_start = offset;
            current_language = observed;
        }
        if run_start < text.len() {
            push_language_span(
                &mut spans,
                region,
                run_start,
                text.len(),
                current_language,
                budget,
            )?;
        }
    }
    Ok(spans)
}

fn push_language_span(
    spans: &mut Vec<Dg1LanguageSpan>,
    region: &Dg1Region,
    relative_start: usize,
    relative_end: usize,
    language: Dg1Language,
    budget: Dg1Budget,
) -> Result<(), Dg1Error> {
    if relative_start >= relative_end
        || spans.len() >= budget.max_syntax_nodes
        || spans.len() >= u32::MAX as usize
    {
        return Err(Dg1Error::SyntaxNodeBudgetExceeded);
    }
    let id = u32::try_from(spans.len()).map_err(|_| Dg1Error::SourceOffsetOverflow)?;
    spans.push(Dg1LanguageSpan {
        id,
        region_id: region.id,
        source_span: region.span.start + relative_start..region.span.start + relative_end,
        language,
    });
    Ok(())
}

fn character_language(character: char, has_japanese_kana: bool) -> Option<Dg1Language> {
    if matches!(character as u32, 0x3040..=0x30ff) {
        Some(Dg1Language::Japanese)
    } else if matches!(character as u32, 0x3400..=0x9fff | 0xf900..=0xfaff) {
        Some(if has_japanese_kana {
            Dg1Language::Japanese
        } else {
            Dg1Language::Unknown
        })
    } else if character.is_ascii_alphabetic() {
        Some(Dg1Language::English)
    } else {
        None
    }
}

fn contains(outer: &std::ops::Range<usize>, inner: &std::ops::Range<usize>) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn region_for_span(regions: &[Dg1Region], span: &std::ops::Range<usize>) -> Option<u32> {
    regions
        .iter()
        .filter(|region| region.kind == Dg1RegionKind::Prose && contains(&region.span, span))
        .min_by_key(|region| region.span.end - region.span.start)
        .map(|region| region.id)
}

fn inline_quote_spans(source: &str, regions: &[Dg1Region]) -> Vec<(std::ops::Range<usize>, u32)> {
    let mut quoted = Vec::new();
    for region in regions
        .iter()
        .filter(|region| region.kind == Dg1RegionKind::Prose)
    {
        let Some(text) = source.get(region.span.clone()) else {
            continue;
        };
        let mut stack = Vec::<(char, usize)>::new();
        for (offset, character) in text.char_indices() {
            if matches!(character, '"' | '“' | '「' | '『' | '‘') {
                if character == '"' && stack.last().is_some_and(|(open, _)| *open == '"') {
                    if let Some((_, start)) = stack.pop() {
                        quoted.push((
                            region.span.start + start..region.span.start + offset + 1,
                            region.id,
                        ));
                    }
                } else {
                    stack.push((character, offset));
                }
                continue;
            }
            let expected_open = match character {
                '”' => Some('“'),
                '」' => Some('「'),
                '』' => Some('『'),
                '’' => Some('‘'),
                _ => None,
            };
            if let Some(expected) = expected_open {
                if stack.last().is_some_and(|(open, _)| *open == expected) {
                    if let Some((_, start)) = stack.pop() {
                        quoted.push((
                            region.span.start + start
                                ..region.span.start + offset + character.len_utf8(),
                            region.id,
                        ));
                    }
                }
            }
        }
    }
    quoted.sort_by_key(|(span, region_id)| (span.start, span.end, *region_id));
    quoted
}

fn extract_requirements(
    source: &str,
    regions: &[Dg1Region],
    budget: Dg1Budget,
) -> Result<(Vec<Dg1RequirementCandidate>, Vec<Dg1InstructionResidual>), Dg1Error> {
    let mut candidates = Vec::new();
    let mut residuals = Vec::new();
    for region in regions
        .iter()
        .filter(|region| region.kind == Dg1RegionKind::Prose)
    {
        let Some(text) = source.get(region.span.clone()) else {
            continue;
        };
        for (start, end) in instruction_clause_ranges(text) {
            let sentence = &text[start..end];
            if sentence.trim().is_empty() {
                continue;
            }
            let span = (region.span.start + start)..(region.span.start + end);
            let language = classify_language(sentence);
            if region.content_role == Dg1ContentRole::Quotation || is_quoted(sentence) {
                residuals.push(Dg1InstructionResidual {
                    source_span: span,
                    source_text: sentence.to_string(),
                    language,
                    reason: Dg1ResidualReason::QuotedText,
                });
                continue;
            }
            let (operative_surface, quote_was_removed) = remove_quoted_content(sentence);
            let Some((strength, polarity)) = instruction_markers(&operative_surface, language)
            else {
                residuals.push(Dg1InstructionResidual {
                    source_span: span,
                    source_text: sentence.to_string(),
                    language,
                    reason: if has_unresolved_reference(&operative_surface) {
                        Dg1ResidualReason::UnresolvedReference
                    } else if quote_was_removed {
                        Dg1ResidualReason::QuotedText
                    } else if matches!(
                        language,
                        Dg1Language::Unknown | Dg1Language::MixedJapaneseEnglish
                    ) {
                        Dg1ResidualReason::MixedOrUnknownLanguage
                    } else {
                        Dg1ResidualReason::NoSupportedInstructionMarker
                    },
                });
                continue;
            };
            if candidates.len() >= budget.max_requirements || candidates.len() >= u32::MAX as usize
            {
                return Err(Dg1Error::RequirementBudgetExceeded);
            }
            let lowered = operative_surface.to_lowercase();
            let condition_kind = if ["only if", "unless ", "場合のみ", "ない限り"]
                .iter()
                .any(|marker| lowered.contains(marker))
            {
                Some(Dg1ConditionKind::Necessary)
            } else if ["until ", "after ", "before ", "まで", "のあと"]
                .iter()
                .any(|marker| lowered.contains(marker))
            {
                Some(Dg1ConditionKind::Temporal)
            } else if ["ただし", "except "]
                .iter()
                .any(|marker| lowered.contains(marker))
            {
                Some(Dg1ConditionKind::Exception)
            } else if ["if ", "when ", "場合", "とき", "なら"]
                .iter()
                .any(|marker| lowered.contains(marker))
            {
                Some(Dg1ConditionKind::Conditional)
            } else {
                None
            };
            let conditional = condition_kind.is_some();
            let exception_present = ["unless ", "except ", "ただし", "場合を除", "ない限り"]
                .iter()
                .any(|marker| lowered.contains(marker));
            let scope = if lowered.contains("staging") {
                Some("staging".to_string())
            } else if lowered.contains("production") || lowered.contains("本番") {
                Some("production".to_string())
            } else {
                None
            };
            candidates.push(Dg1RequirementCandidate {
                id: candidates.len() as u32 + 1,
                source_span: span,
                source_text: sentence.to_string(),
                language,
                strength,
                polarity,
                conditional,
                exception_present,
                condition_kind,
                scope,
                unresolved_reference: has_unresolved_reference(&operative_surface),
                // Textual modality is not host-delivered authority.
                authority_grant: false,
            });
        }
    }
    Ok((candidates, residuals))
}

fn instruction_clause_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut clauses = Vec::new();
    for (start, end) in sentence_ranges(text) {
        let sentence = &text[start..end];
        let split = ["実行し、", "確認し、", "実装し、", "作成し、"]
            .iter()
            .find_map(|marker| {
                sentence
                    .find(marker)
                    .map(|offset| start + offset + marker.len())
            });
        if let Some(split) = split.filter(|split| *split < end) {
            clauses.push((start, split));
            clauses.push((split, end));
        } else {
            clauses.push((start, end));
        }
    }
    clauses
}

fn has_unresolved_reference(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("それを")
        || lower.contains("あれを")
        || lower.starts_with("fix it")
        || lower.starts_with("change that")
}

fn sentence_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0usize;
    let mut quote_close = None;
    for (index, character) in text.char_indices() {
        if let Some(close) = quote_close {
            if character == close {
                quote_close = None;
            }
            continue;
        }
        quote_close = match character {
            '"' => Some('"'),
            '“' => Some('”'),
            '‘' => Some('’'),
            '「' => Some('」'),
            '『' => Some('』'),
            _ => None,
        };
        if quote_close.is_some() {
            continue;
        }
        if matches!(
            character,
            '.' | '!' | '?' | '。' | '！' | '？' | '\n' | '\r'
        ) {
            let end = index + character.len_utf8();
            if start < end {
                ranges.push((start, end));
            }
            start = end;
        }
    }
    if start < text.len() {
        ranges.push((start, text.len()));
    }
    ranges
}

fn is_quoted(sentence: &str) -> bool {
    let trimmed = sentence.trim();
    [
        ('"', '"'),
        ('\'', '\''),
        ('“', '”'),
        ('‘', '’'),
        ('「', '」'),
        ('『', '』'),
        ('`', '`'),
    ]
    .iter()
    .any(|(left, right)| {
        trimmed.starts_with(*left)
            && trimmed.ends_with(*right)
            && trimmed.len() > left.len_utf8() + right.len_utf8()
    })
}

fn remove_quoted_content(text: &str) -> (String, bool) {
    let mut output = String::with_capacity(text.len());
    let mut quote_closers = Vec::<char>::new();
    let mut removed = false;
    for character in text.chars() {
        if let Some(close) = quote_closers.last().copied() {
            removed = true;
            if character == close {
                quote_closers.pop();
            } else if let Some(nested) = quote_close(character) {
                quote_closers.push(nested);
            }
            continue;
        }
        if let Some(close) = quote_close(character) {
            quote_closers.push(close);
            removed = true;
        } else {
            output.push(character);
        }
    }
    (output, removed)
}

fn quote_close(open: char) -> Option<char> {
    match open {
        '"' => Some('"'),
        '“' => Some('”'),
        '‘' => Some('’'),
        '「' => Some('」'),
        '『' => Some('』'),
        _ => None,
    }
}

fn instruction_markers(
    sentence: &str,
    language: Dg1Language,
) -> Option<(Dg1RequirementStrength, Dg1RequirementPolarity)> {
    let normalized = sentence.to_lowercase();
    let has_japanese = matches!(
        language,
        Dg1Language::Japanese | Dg1Language::MixedJapaneseEnglish
    );
    let has_english = matches!(
        language,
        Dg1Language::English | Dg1Language::MixedJapaneseEnglish
    );
    let negative_japanese = if has_japanese {
        [
            "してはいけない",
            "しないで",
            "しないこと",
            "禁止",
            "行わない",
            "しません",
        ]
        .iter()
        .any(|marker| normalized.contains(marker))
    } else {
        false
    };
    let negative_english = if has_english {
        [
            "must not",
            "mustn't",
            "do not",
            "don't",
            "never ",
            "shall not",
            "should not",
            "cannot",
        ]
        .iter()
        .any(|marker| normalized.contains(marker))
    } else {
        false
    };
    let negative = negative_japanese || negative_english;
    let positive_scope = [
        "してはいけない",
        "しないで",
        "しないこと",
        "禁止",
        "行わない",
        "しません",
        "must not",
        "mustn't",
        "do not",
        "don't",
        "never ",
        "shall not",
        "should not",
        "cannot",
    ]
    .iter()
    .fold(normalized.clone(), |text, marker| text.replace(marker, " "));
    let positive_japanese = if has_japanese {
        [
            "してください",
            "して下さい",
            "すること",
            "必須",
            "必要",
            "行ってください",
            "実装する",
            "追加する",
            "確認する",
            "実行し",
            "作る",
            "説明して",
            "許可",
        ]
        .iter()
        .any(|marker| normalized.contains(marker))
    } else {
        false
    };
    let positive_english = if has_english {
        [
            "must ",
            "must(",
            "required",
            "please ",
            "should ",
            "shall ",
            "ensure ",
            "implement ",
            "add ",
            "update ",
            "verify ",
            "preserve ",
            "create ",
            "run ",
            "do ",
            "document ",
            "may ",
            "allowed",
            "permitted",
        ]
        .iter()
        .any(|marker| positive_scope.contains(marker))
    } else {
        false
    };
    let positive = positive_japanese || positive_english;
    if !negative && !positive {
        return None;
    }
    let strength = if (negative && !normalized.contains("should not"))
        || (has_japanese && normalized.contains("必須"))
    {
        Dg1RequirementStrength::Must
    } else if (has_japanese && normalized.contains("許可"))
        || (has_english
            && ["may ", "allowed", "permitted"]
                .iter()
                .any(|word| normalized.contains(word)))
    {
        Dg1RequirementStrength::May
    } else if normalized.contains("should ")
        || normalized.contains("必要")
        || normalized.contains("should not")
    {
        Dg1RequirementStrength::Should
    } else {
        Dg1RequirementStrength::Must
    };
    let polarity = match (positive, negative) {
        (true, true) => Dg1RequirementPolarity::Conflict,
        (false, true) => Dg1RequirementPolarity::Negative,
        (true, false) => Dg1RequirementPolarity::Positive,
        (false, false) => Dg1RequirementPolarity::Unknown,
    };
    Some((strength, polarity))
}

fn push_region(
    regions: &mut Vec<Dg1Region>,
    budget: Dg1Budget,
    mut region: Dg1Region,
) -> Result<u32, Dg1Error> {
    if regions.len() >= budget.max_regions || regions.len() >= u32::MAX as usize {
        return Err(Dg1Error::RegionBudgetExceeded);
    }
    let id = regions.len() as u32;
    region.id = id;
    regions.push(region);
    Ok(id)
}

fn markdown_region_for_tag(tag: &Tag<'_>) -> Option<(Dg1RegionKind, TagEnd)> {
    let (kind, end) = match tag {
        Tag::Paragraph => (Dg1RegionKind::Paragraph, TagEnd::Paragraph),
        Tag::Heading { level, .. } => (Dg1RegionKind::Heading, TagEnd::Heading(*level)),
        Tag::BlockQuote(kind) => (Dg1RegionKind::BlockQuote, TagEnd::BlockQuote(*kind)),
        Tag::List(start) => (Dg1RegionKind::List, TagEnd::List(start.is_some())),
        Tag::Item => (Dg1RegionKind::ListItem, TagEnd::Item),
        Tag::Link { .. } => (Dg1RegionKind::Link, TagEnd::Link),
        Tag::Emphasis => (Dg1RegionKind::Emphasis, TagEnd::Emphasis),
        Tag::Strong => (Dg1RegionKind::Strong, TagEnd::Strong),
        Tag::Table(_) => (Dg1RegionKind::Table, TagEnd::Table),
        Tag::TableRow => (Dg1RegionKind::TableRow, TagEnd::TableRow),
        Tag::TableCell => (Dg1RegionKind::TableCell, TagEnd::TableCell),
        Tag::HtmlBlock => (Dg1RegionKind::OpaqueHtmlBlock, TagEnd::HtmlBlock),
        _ => return None,
    };
    Some((kind, end))
}

fn extend_markdown_regions(
    open: &mut [OpenMarkdownRegion],
    regions: &mut [Dg1Region],
    range: std::ops::Range<usize>,
) {
    for open_region in open {
        open_region.start = open_region.start.min(range.start);
        open_region.end = open_region.end.max(range.end);
        if let Some(region) = regions.get_mut(open_region.id as usize) {
            region.span = open_region.start..open_region.end;
        }
    }
}

fn parse_rust_region(
    region_id: u32,
    base_offset: usize,
    source: &str,
    budget: Dg1Budget,
    syntax_count: &mut usize,
) -> Result<Dg1SyntaxTree, Dg1Error> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|_| Dg1Error::RustParserUnavailable)?;
    let tree = parser
        .parse(source, None)
        .ok_or(Dg1Error::RustParserUnavailable)?;
    let root = tree.root_node();
    let mut pending = vec![(root, None)];
    let mut nodes = Vec::new();
    while let Some((node, parent_id)) = pending.pop() {
        *syntax_count = syntax_count
            .checked_add(1)
            .ok_or(Dg1Error::SyntaxNodeBudgetExceeded)?;
        if *syntax_count > budget.max_syntax_nodes || nodes.len() >= u32::MAX as usize {
            return Err(Dg1Error::SyntaxNodeBudgetExceeded);
        }
        let start = base_offset
            .checked_add(node.start_byte())
            .ok_or(Dg1Error::SourceOffsetOverflow)?;
        let end = base_offset
            .checked_add(node.end_byte())
            .ok_or(Dg1Error::SourceOffsetOverflow)?;
        let node_id = u32::try_from(nodes.len()).map_err(|_| Dg1Error::SyntaxNodeBudgetExceeded)?;
        nodes.push(Dg1SyntaxNode {
            id: node_id,
            parent_id,
            kind: node.kind().to_string(),
            named: node.is_named(),
            span: start..end,
            has_error: node.has_error(),
            is_error: node.is_error(),
            missing: node.is_missing(),
        });
        for index in (0..node.child_count()).rev() {
            if let Some(child) = node.child(index) {
                pending.push((child, Some(node_id)));
            }
        }
    }
    Ok(Dg1SyntaxTree {
        region_id,
        grammar_revision: RUST_GRAMMAR_REVISION.into(),
        root_has_error: root.has_error(),
        nodes,
    })
}

fn looks_like_rust_source(source: &str) -> bool {
    let trimmed = source.trim_start();
    [
        "#![",
        "fn ",
        "pub ",
        "use ",
        "struct ",
        "enum ",
        "impl ",
        "mod ",
        "trait ",
        "const ",
        "static ",
        "type ",
        "extern ",
        "async fn ",
        "unsafe fn ",
        "sql!(",
    ]
    .iter()
    .any(|prefix| trimmed.starts_with(prefix))
}

fn classify_language(text: &str) -> Dg1Language {
    let mut japanese = 0usize;
    let mut latin = 0usize;
    for character in text.chars() {
        if matches!(character as u32, 0x3040..=0x30ff | 0x3400..=0x9fff | 0xf900..=0xfaff) {
            japanese += 1;
        } else if character.is_ascii_alphabetic() {
            latin += 1;
        }
    }
    match (japanese > 0, latin > 0) {
        (true, true) => Dg1Language::MixedJapaneseEnglish,
        (true, false) => Dg1Language::Japanese,
        (false, true) => Dg1Language::English,
        (false, false) => Dg1Language::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_budget_is_explicit_and_fail_visible() {
        let budget = Dg1Budget {
            max_source_bytes: 3,
            ..Dg1Budget::default()
        };
        assert_eq!(
            analyze_dg1("日本", budget),
            Err(Dg1Error::SourceBudgetExceeded)
        );
        assert_eq!(
            analyze_dg1(
                "text",
                Dg1Budget {
                    max_alternatives: 0,
                    ..Dg1Budget::default()
                }
            ),
            Err(Dg1Error::AlternativeBudgetExceeded)
        );
    }

    #[test]
    fn region_accounting_and_language_spans_share_the_declared_node_budget() {
        assert_eq!(
            analyze_dg1(
                "Please parse this.",
                Dg1Budget {
                    max_syntax_nodes: 2,
                    ..Dg1Budget::default()
                }
            ),
            Err(Dg1Error::SyntaxNodeBudgetExceeded)
        );
    }

    #[test]
    fn markdown_fence_and_rust_tree_keep_original_byte_offsets() {
        let source = "説明\n```rust\nfn f() { let s = \"(]\"; }\n```\n終わり";
        let report = analyze_dg1(source, Dg1Budget::default()).unwrap();
        assert_eq!(report.source_revision, stable_sha256(source));
        let code = report
            .regions
            .iter()
            .find(|region| region.kind == Dg1RegionKind::FencedCode)
            .unwrap();
        assert!(source.is_char_boundary(code.span.start));
        assert!(source.is_char_boundary(code.span.end));
        assert_eq!(report.rust_trees.len(), 1);
        assert!(
            !report.rust_trees[0].root_has_error,
            "region={:?} tree={:?}",
            code.span, report.rust_trees[0]
        );
        assert!(report.rust_trees[0]
            .nodes
            .iter()
            .any(|node| node.kind == "string_literal"));
    }

    #[test]
    fn prose_language_labels_are_only_script_hints() {
        assert_eq!(classify_language("日本語の要求"), Dg1Language::Japanese);
        assert_eq!(
            classify_language("English instruction"),
            Dg1Language::English
        );
        assert_eq!(
            classify_language("日本語 and English"),
            Dg1Language::MixedJapaneseEnglish
        );
    }

    #[test]
    fn bounded_instruction_rules_preserve_polarity_condition_and_quote_boundary() {
        let source = "Do not delete the source. If tests pass, update docs. \"Publish it.\"\nテストが通過した場合、実装してください。";
        let report = analyze_dg1(source, Dg1Budget::default()).unwrap();
        assert_eq!(report.requirement_candidates.len(), 3);
        assert_eq!(
            report.requirement_candidates[0].polarity,
            Dg1RequirementPolarity::Negative
        );
        assert!(report.requirement_candidates[1].conditional);
        assert_eq!(
            report.requirement_candidates[1].polarity,
            Dg1RequirementPolarity::Positive
        );
        assert_eq!(
            report.requirement_candidates[2].language,
            Dg1Language::Japanese
        );
        assert!(report
            .instruction_residuals
            .iter()
            .any(|item| item.reason == Dg1ResidualReason::QuotedText));
        assert!(report
            .requirement_candidates
            .iter()
            .all(|candidate| !candidate.authority_grant));
        for candidate in &report.requirement_candidates {
            assert_eq!(
                source.get(candidate.source_span.clone()),
                Some(candidate.source_text.as_str())
            );
        }
    }

    #[test]
    fn explicit_conflicting_polarity_is_not_collapsed() {
        let report = analyze_dg1(
            "Must not delete; must retain the file.",
            Dg1Budget::default(),
        )
        .unwrap();
        assert_eq!(report.requirement_candidates.len(), 1);
        assert_eq!(
            report.requirement_candidates[0].polarity,
            Dg1RequirementPolarity::Conflict
        );
    }

    #[test]
    fn mixed_japanese_english_instruction_keeps_detected_polarity() {
        let report = analyze_dg1(
            "この操作では Do not publish the package.",
            Dg1Budget::default(),
        )
        .unwrap();
        assert_eq!(report.requirement_candidates.len(), 1);
        assert_eq!(
            report.requirement_candidates[0].polarity,
            Dg1RequirementPolarity::Negative
        );
        assert_eq!(
            report.requirement_candidates[0].language,
            Dg1Language::MixedJapaneseEnglish
        );
    }

    #[test]
    fn crlf_and_multibyte_spans_stay_on_utf8_boundaries() {
        let source = "説明\r\n```rust\r\nfn f() { let _ = r#\"}\"#; }\r\n```\r\n";
        let report = analyze_dg1(source, Dg1Budget::default()).unwrap();
        assert!(report.exact_source_roundtrip);
        let tree = report.rust_trees.first().unwrap();
        assert!(!tree.root_has_error);
        assert!(tree.nodes.iter().all(|node| {
            source.is_char_boundary(node.span.start) && source.is_char_boundary(node.span.end)
        }));
    }

    #[test]
    fn markdown_block_regions_preserve_quote_role_without_granting_authority() {
        let source = "# Notes\n\n> Please delete the repository.\n\n- Please retain the source.\n";
        let report = analyze_dg1(source, Dg1Budget::default()).unwrap();
        assert!(report
            .regions
            .iter()
            .any(|region| region.kind == Dg1RegionKind::Heading));
        assert!(report
            .regions
            .iter()
            .any(|region| region.kind == Dg1RegionKind::BlockQuote));
        assert!(report
            .regions
            .iter()
            .any(|region| region.kind == Dg1RegionKind::ListItem));
        assert_eq!(report.requirement_candidates.len(), 1);
        assert!(report
            .instruction_residuals
            .iter()
            .any(|residual| residual.reason == Dg1ResidualReason::QuotedText));
        assert!(!report.requirement_candidates[0].authority_grant);
        for region in report.regions.iter().filter(|region| region.id != 0) {
            assert!(region.span.start <= region.span.end);
            assert!(source.get(region.span.clone()).is_some());
        }
    }
}
