use crate::{
    default_profile_registry, BoundaryLedger, ConstraintEdge, ConstraintKind, ConstraintState,
    DeepGrammarArtifact, GrammarProduction, GrammarProfileId, PackedDerivation, ParseDefect,
    RegionCandidate, RegionLattice, RelationKind, RelationState, SyntaxNode, SyntaxNodeId,
    SyntaxNodeKind, SyntaxRelation, Token, TokenKind, TokenLattice, TypedConstraintGraph,
    UnifiedGrammarIr, UnifiedSyntaxHypergraph,
};
use lc631_core::SourceSpan;
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum TldgError {
    SourceRoundtripMismatch,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct UnifiedParseReport {
    pub source_roundtrip: String,
    pub profile_count: usize,
    pub region_count: usize,
    pub token_count: usize,
    pub derivation_count: usize,
    pub error_node_count: usize,
    pub relation_count: usize,
    pub constraint_count: usize,
    pub embedded_region_count: usize,
    pub natural_program_binary_split: bool,
}

pub fn analyze_document(source: &str) -> Result<UnifiedParseReport, TldgError> {
    let artifact = analyze(source)?;
    Ok(UnifiedParseReport {
        source_roundtrip: artifact.boundary_roundtrip.clone(),
        profile_count: default_profile_registry().len(),
        region_count: artifact.regions.candidates.len(),
        token_count: artifact.tokens.tokens.len(),
        derivation_count: artifact.syntax.derivations.len(),
        error_node_count: artifact
            .syntax
            .nodes
            .iter()
            .filter(|node| node.kind == SyntaxNodeKind::Error)
            .count(),
        relation_count: artifact.syntax.relations.len(),
        constraint_count: artifact.constraints.edges.len(),
        embedded_region_count: artifact
            .regions
            .candidates
            .iter()
            .filter(|region| region.embedded)
            .count(),
        natural_program_binary_split: false,
    })
}

pub fn analyze(source: &str) -> Result<DeepGrammarArtifact, TldgError> {
    let boundary = BoundaryLedger::build(source);
    if boundary.roundtrip() != source || !boundary.covers_source() {
        return Err(TldgError::SourceRoundtripMismatch);
    }
    let regions = build_regions(source);
    let tokens = tokenize(source);
    let grammar = builtin_grammar();
    let syntax = build_syntax(source, &regions, &tokens, &grammar);
    let constraints = build_constraints(&tokens);
    let boundary_roundtrip = boundary.roundtrip();
    Ok(DeepGrammarArtifact {
        source: boundary.source,
        boundary_roundtrip,
        regions,
        tokens,
        grammar,
        syntax,
        constraints,
        claim_boundary:
            "lossless reference parse only; not semantic truth, compiler acceptance, or authority"
                .into(),
    })
}

fn build_regions(source: &str) -> RegionLattice {
    let full = SourceSpan {
        start: 0,
        end: source.len(),
    };
    let mut candidates = vec![RegionCandidate {
        span: full,
        profile: GrammarProfileId(1),
        evidence: "total_generic_symbolic_region".into(),
        embedded: false,
    }];
    if source.chars().any(|character| character.is_alphabetic()) {
        candidates.push(RegionCandidate {
            span: full,
            profile: GrammarProfileId(2),
            evidence: "open_lexicon_candidate".into(),
            embedded: false,
        });
    }
    if contains_code_surface(source) {
        candidates.push(RegionCandidate {
            span: full,
            profile: GrammarProfileId(3),
            evidence: "executable_symbolic_candidate".into(),
            embedded: false,
        });
    }
    for span in embedded_spans(source) {
        candidates.push(RegionCandidate {
            span,
            profile: GrammarProfileId(4),
            evidence: "embedded_dialect_boundary".into(),
            embedded: true,
        });
    }
    RegionLattice { candidates }
}

fn contains_code_surface(source: &str) -> bool {
    ["fn ", "let ", "const ", "::", "();", "{", "}", "sql!("]
        .iter()
        .any(|needle| source.contains(needle))
}

fn embedded_spans(source: &str) -> Vec<SourceSpan> {
    let mut spans = Vec::new();
    let mut search = 0;
    while let Some(open_relative) = source[search..].find("~~~") {
        let open = search + open_relative;
        let content_start = source[open + 3..]
            .find('\n')
            .map_or(open + 3, |offset| open + 4 + offset);
        if let Some(close_relative) = source[content_start..].find("~~~") {
            let close = content_start + close_relative;
            spans.push(SourceSpan {
                start: content_start,
                end: close,
            });
            search = close + 3;
        } else {
            break;
        }
    }
    if let Some(sql) = source.find("sql!(") {
        let start = sql + "sql!(".len();
        let end = source[start..]
            .find(')')
            .map_or(source.len(), |offset| start + offset);
        spans.push(SourceSpan { start, end });
    }
    if let Some(comment) = source.find("//") {
        spans.push(SourceSpan {
            start: comment,
            end: source.len(),
        });
    }
    spans
}

fn tokenize(source: &str) -> TokenLattice {
    let mut tokens = Vec::new();
    let mut start = 0;
    while start < source.len() {
        let character = source[start..].chars().next().expect("valid boundary");
        let mut end = start + character.len_utf8();
        let kind = classify(character);
        while end < source.len() {
            let next = source[end..].chars().next().expect("valid boundary");
            if !can_merge(kind, next) {
                break;
            }
            end += next.len_utf8();
        }
        tokens.push(Token {
            id: tokens.len() as u32,
            span: SourceSpan { start, end },
            kind,
            surface: source[start..end].to_string(),
        });
        start = end;
    }
    let primary = tokens.iter().map(|token| token.id).collect::<Vec<_>>();
    let competing_segmentations = if source.contains(" or ") {
        let whitespace_elided = tokens
            .iter()
            .filter(|token| token.kind != TokenKind::Whitespace)
            .map(|token| token.id)
            .collect();
        vec![primary, whitespace_elided]
    } else {
        vec![primary]
    };
    TokenLattice {
        tokens,
        competing_segmentations,
    }
}

fn build_constraints(tokens: &TokenLattice) -> TypedConstraintGraph {
    let mut edges = Vec::new();
    for pair in tokens.tokens.windows(2) {
        let left = SyntaxNodeId(u64::from(pair[0].id) + 1);
        let right = SyntaxNodeId(u64::from(pair[1].id) + 1);
        let lowered = pair[0].surface.to_lowercase();
        let kind = if matches!(lowered.as_str(), "let" | "const" | "fn") {
            ConstraintKind::Binding
        } else if matches!(lowered.as_str(), "if" | "when" | "before" | "after") {
            ConstraintKind::Scope
        } else if matches!(lowered.as_str(), "return" | "delete" | "write" | "say") {
            ConstraintKind::EffectOrSpeechAct
        } else {
            ConstraintKind::Dependency
        };
        edges.push(ConstraintEdge {
            from: left,
            to: right,
            kind,
            state: if pair[0].kind == TokenKind::Whitespace {
                ConstraintState::Unknown
            } else {
                ConstraintState::Candidate
            },
            source: "surface_constraint_reference".into(),
        });
    }
    TypedConstraintGraph { edges }
}

fn classify(character: char) -> TokenKind {
    if character.is_whitespace() {
        TokenKind::Whitespace
    } else if character.is_numeric() {
        TokenKind::Number
    } else if character.is_alphabetic() || character == '_' {
        TokenKind::Word
    } else if "(){}[]".contains(character) {
        TokenKind::Delimiter
    } else if "'\"".contains(character) {
        TokenKind::Quote
    } else if "+-*/=<>!&|:;,.%".contains(character) {
        TokenKind::Operator
    } else {
        TokenKind::Symbol
    }
}

fn can_merge(kind: TokenKind, next: char) -> bool {
    match kind {
        TokenKind::Whitespace => next.is_whitespace(),
        TokenKind::Word => next.is_alphabetic() || next.is_numeric() || next == '_',
        TokenKind::Number => next.is_numeric(),
        TokenKind::Operator => "+-*/=<>!&|:%".contains(next),
        TokenKind::Delimiter | TokenKind::Quote | TokenKind::Symbol => false,
    }
}

fn builtin_grammar() -> UnifiedGrammarIr {
    UnifiedGrammarIr {
        productions: vec![
            GrammarProduction {
                id: 1,
                name: "document_contains_symbol_sequence".into(),
                profile: GrammarProfileId(1),
            },
            GrammarProduction {
                id: 2,
                name: "group_delimiter_scope".into(),
                profile: GrammarProfileId(1),
            },
            GrammarProduction {
                id: 3,
                name: "open_discourse_sequence".into(),
                profile: GrammarProfileId(2),
            },
            GrammarProduction {
                id: 4,
                name: "executable_symbol_sequence".into(),
                profile: GrammarProfileId(3),
            },
        ],
    }
}

fn build_syntax(
    source: &str,
    regions: &RegionLattice,
    tokens: &TokenLattice,
    grammar: &UnifiedGrammarIr,
) -> UnifiedSyntaxHypergraph {
    let root = SyntaxNodeId(0);
    let profile_candidates = regions
        .candidates
        .iter()
        .map(|region| region.profile)
        .collect::<Vec<_>>();
    let mut nodes = vec![SyntaxNode {
        id: root,
        spans: vec![SourceSpan {
            start: 0,
            end: source.len(),
        }],
        kind: SyntaxNodeKind::Document,
        profile_candidates: profile_candidates.clone(),
        origin: "source_document".into(),
    }];
    let mut relations = Vec::new();
    for token in &tokens.tokens {
        let id = SyntaxNodeId(nodes.len() as u64);
        nodes.push(SyntaxNode {
            id,
            spans: vec![token.span],
            kind: SyntaxNodeKind::Token,
            profile_candidates: profile_candidates.clone(),
            origin: "lossless_token_lattice".into(),
        });
        relations.push(SyntaxRelation {
            from: root,
            to: id,
            kind: RelationKind::Contains,
            state: RelationState::Verified,
            evidence: "source_span_coverage".into(),
        });
        if id.0 > 1 {
            relations.push(SyntaxRelation {
                from: SyntaxNodeId(id.0 - 1),
                to: id,
                kind: RelationKind::Sequence,
                state: RelationState::Verified,
                evidence: "source_order".into(),
            });
        }
    }
    let mut defects = Vec::new();
    let mut stack = Vec::<(char, SyntaxNodeId, SourceSpan)>::new();
    for token in &tokens.tokens {
        if token.kind != TokenKind::Delimiter {
            continue;
        }
        let character = token.surface.chars().next().unwrap_or_default();
        let token_node = SyntaxNodeId(u64::from(token.id) + 1);
        if "({[".contains(character) {
            stack.push((character, token_node, token.span));
        } else if let Some((opening, opener, opening_span)) = stack.pop() {
            if matching(opening) == character {
                relations.push(SyntaxRelation {
                    from: opener,
                    to: token_node,
                    kind: RelationKind::Scope,
                    state: RelationState::Verified,
                    evidence: "matched_delimiter".into(),
                });
            } else {
                defects.push(ParseDefect::UnmatchedDelimiter(token.span));
                insert_error(&mut nodes, token.span, &profile_candidates);
                stack.push((opening, opener, opening_span));
            }
        } else {
            defects.push(ParseDefect::UnmatchedDelimiter(token.span));
            insert_error(&mut nodes, token.span, &profile_candidates);
        }
    }
    for (_, _, span) in stack {
        defects.push(ParseDefect::UnmatchedDelimiter(span));
        insert_error(&mut nodes, span, &profile_candidates);
    }
    if regions.candidates.len() > 1 {
        defects.push(ParseDefect::AmbiguousRegionBoundary(SourceSpan {
            start: 0,
            end: source.len(),
        }));
    }
    let derivation_count = tokens.competing_segmentations.len().max(1);
    if derivation_count > 1 {
        defects.push(ParseDefect::CompetingDerivations);
    }
    let derivations = (0..derivation_count)
        .map(|index| PackedDerivation {
            id: index as u32 + 1,
            root,
            production_ids: grammar
                .productions
                .iter()
                .map(|production| production.id)
                .collect(),
            status: if index == 0 {
                "reference_surface_derivation".into()
            } else {
                "competing_derivation".into()
            },
        })
        .collect();
    UnifiedSyntaxHypergraph {
        nodes,
        relations,
        derivations,
        defects,
    }
}

fn insert_error(nodes: &mut Vec<SyntaxNode>, span: SourceSpan, profiles: &[GrammarProfileId]) {
    nodes.push(SyntaxNode {
        id: SyntaxNodeId(nodes.len() as u64),
        spans: vec![span],
        kind: SyntaxNodeKind::Error,
        profile_candidates: profiles.to_vec(),
        origin: "delimiter_error_recovery".into(),
    });
}

fn matching(opening: char) -> char {
    match opening {
        '(' => ')',
        '{' => '}',
        '[' => ']',
        _ => opening,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_is_a_total_parse() {
        let artifact = analyze("").unwrap();
        assert_eq!(artifact.boundary_roundtrip, "");
        assert_eq!(artifact.syntax.derivations.len(), 1);
    }
}
