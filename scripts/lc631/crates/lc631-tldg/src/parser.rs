use crate::{
    analyze_dg1, default_profile_registry, BoundaryLedger, ConstraintEdge, ConstraintKind,
    ConstraintState, DeepGrammarArtifact, Dg1Error, Dg1Language, Dg1Region, Dg1RegionKind,
    Dg1Report, GrammarProduction, GrammarProfileId, PackedDerivation, ParseDefect, RegionCandidate,
    RegionLattice, RelationKind, RelationState, SyntaxNode, SyntaxNodeId, SyntaxNodeKind,
    SyntaxRelation, Token, TokenKind, TokenLattice, TypedConstraintGraph, UnifiedGrammarIr,
    UnifiedSyntaxHypergraph,
};
use lc631_core::SourceSpan;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum TldgError {
    SourceRoundtripMismatch,
    Dg1(Dg1Error),
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
    let (artifact, _) = analyze_with_dg1(source)?;
    Ok(report_for_artifact(&artifact))
}

pub fn report_for_artifact(artifact: &DeepGrammarArtifact) -> UnifiedParseReport {
    UnifiedParseReport {
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
    }
}

pub fn analyze(source: &str) -> Result<DeepGrammarArtifact, TldgError> {
    analyze_with_dg1(source).map(|(artifact, _)| artifact)
}

pub(crate) fn analyze_with_dg1(
    source: &str,
) -> Result<(DeepGrammarArtifact, Dg1Report), TldgError> {
    let boundary = BoundaryLedger::build(source);
    if boundary.roundtrip() != source || !boundary.covers_source() {
        return Err(TldgError::SourceRoundtripMismatch);
    }
    let dg1 = analyze_dg1(source, crate::Dg1Budget::default()).map_err(TldgError::Dg1)?;
    if !dg1.exact_source_roundtrip {
        return Err(TldgError::SourceRoundtripMismatch);
    }
    let regions = build_regions(&dg1);
    let tokens = tokenize(source);
    let grammar = builtin_grammar();
    let syntax = build_syntax(source, &dg1, &regions)?;
    let constraints = build_constraints(&tokens);
    let boundary_roundtrip = boundary.roundtrip();
    Ok((
        DeepGrammarArtifact {
            source: boundary.source,
            boundary_roundtrip,
            regions,
            tokens,
            grammar,
            syntax,
            constraints,
            claim_boundary:
                "DG1 bounded CommonMark+tables/Rust CST projection; script spans are hints, rule candidates remain non-authoritative, and external natural-language semantics are unavailable"
                    .into(),
        },
        dg1,
    ))
}

fn build_regions(dg1: &Dg1Report) -> RegionLattice {
    RegionLattice {
        candidates: dg1
            .regions
            .iter()
            .map(|region| RegionCandidate {
                span: SourceSpan {
                    start: region.span.start,
                    end: region.span.end,
                },
                profile: profile_for_region(region),
                evidence: format!("dg1_commonmark:{:?}:{:?}", region.kind, region.content_role),
                embedded: matches!(
                    region.kind,
                    Dg1RegionKind::InlineCode
                        | Dg1RegionKind::FencedCode
                        | Dg1RegionKind::IndentedCode
                        | Dg1RegionKind::OpaqueEmbedded
                ),
            })
            .collect(),
    }
}

fn profile_for_region(region: &Dg1Region) -> GrammarProfileId {
    if region
        .dialect
        .as_deref()
        .is_some_and(|dialect| dialect == "rust" || dialect == "rs")
    {
        GrammarProfileId(3)
    } else if matches!(
        region.language,
        Dg1Language::Japanese | Dg1Language::English | Dg1Language::MixedJapaneseEnglish
    ) {
        GrammarProfileId(2)
    } else if matches!(
        region.kind,
        Dg1RegionKind::FencedCode
            | Dg1RegionKind::IndentedCode
            | Dg1RegionKind::InlineCode
            | Dg1RegionKind::OpaqueEmbedded
    ) {
        GrammarProfileId(4)
    } else {
        GrammarProfileId(1)
    }
}

fn tokenize(source: &str) -> TokenLattice {
    let mut tokens = Vec::new();
    let mut start = 0;
    while start < source.len() {
        let Some(character) = source.get(start..).and_then(|suffix| suffix.chars().next()) else {
            break;
        };
        let mut end = start + character.len_utf8();
        let kind = classify(character);
        while end < source.len() {
            let Some(next) = source.get(end..).and_then(|suffix| suffix.chars().next()) else {
                break;
            };
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
    TokenLattice {
        tokens,
        competing_segmentations: vec![primary],
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
    dg1: &Dg1Report,
    regions: &RegionLattice,
) -> Result<UnifiedSyntaxHypergraph, TldgError> {
    let root = SyntaxNodeId(0);
    let profile_set = regions
        .candidates
        .iter()
        .map(|region| region.profile)
        .collect::<BTreeSet<_>>();
    let profile_candidates = profile_set.into_iter().collect::<Vec<_>>();
    let mut nodes = vec![SyntaxNode {
        id: root,
        spans: vec![SourceSpan {
            start: 0,
            end: source.len(),
        }],
        kind: SyntaxNodeKind::Document,
        profile_candidates,
        origin: "dg1_document_root".into(),
    }];
    let mut relations = Vec::new();
    let mut defects = Vec::new();
    let mut region_nodes = BTreeMap::<u32, SyntaxNodeId>::from([(0, root)]);
    let mut region_children = BTreeMap::<u32, Vec<SyntaxNodeId>>::new();

    for region in dg1.regions.iter().filter(|region| region.id != 0) {
        if source.get(region.span.clone()).is_none() {
            return Err(TldgError::SourceRoundtripMismatch);
        }
        let parent_region = region.parent.unwrap_or(0);
        let Some(parent_id) = region_nodes.get(&parent_region).copied() else {
            return Err(TldgError::SourceRoundtripMismatch);
        };
        let id = SyntaxNodeId(
            u64::try_from(nodes.len()).map_err(|_| TldgError::SourceRoundtripMismatch)?,
        );
        let profile = profile_for_region(region);
        nodes.push(SyntaxNode {
            id,
            spans: vec![SourceSpan {
                start: region.span.start,
                end: region.span.end,
            }],
            kind: SyntaxNodeKind::Group,
            profile_candidates: vec![profile],
            origin: format!("dg1_commonmark_region:{:?}", region.kind),
        });
        region_nodes.insert(region.id, id);
        region_children.entry(parent_region).or_default().push(id);
        relations.push(SyntaxRelation {
            from: parent_id,
            to: id,
            kind: RelationKind::Contains,
            state: RelationState::Verified,
            evidence: "pinned_commonmark_event_region_parent".into(),
        });
    }
    for siblings in region_children.values() {
        for pair in siblings.windows(2) {
            relations.push(SyntaxRelation {
                from: pair[0],
                to: pair[1],
                kind: RelationKind::Sequence,
                state: RelationState::Verified,
                evidence: "pinned_commonmark_event_order".into(),
            });
        }
    }

    for tree in &dg1.rust_trees {
        let Some(region_parent) = region_nodes.get(&tree.region_id).copied() else {
            return Err(TldgError::SourceRoundtripMismatch);
        };
        let mut local_to_global = BTreeMap::<u32, SyntaxNodeId>::new();
        let mut syntax_children = BTreeMap::<Option<u32>, Vec<SyntaxNodeId>>::new();
        for node in &tree.nodes {
            if source.get(node.span.clone()).is_none() {
                return Err(TldgError::SourceRoundtripMismatch);
            }
            let parent_id = match node.parent_id {
                Some(parent) => local_to_global
                    .get(&parent)
                    .copied()
                    .ok_or(TldgError::SourceRoundtripMismatch)?,
                None => region_parent,
            };
            let id = SyntaxNodeId(
                u64::try_from(nodes.len()).map_err(|_| TldgError::SourceRoundtripMismatch)?,
            );
            let error = node.is_error || node.missing || node.kind == "ERROR";
            nodes.push(SyntaxNode {
                id,
                spans: vec![SourceSpan {
                    start: node.span.start,
                    end: node.span.end,
                }],
                kind: if error {
                    SyntaxNodeKind::Error
                } else if node.named {
                    SyntaxNodeKind::Group
                } else {
                    SyntaxNodeKind::Token
                },
                profile_candidates: vec![GrammarProfileId(3)],
                origin: format!("tree_sitter_rust_cst:{}", node.kind),
            });
            local_to_global.insert(node.id, id);
            syntax_children.entry(node.parent_id).or_default().push(id);
            relations.push(SyntaxRelation {
                from: parent_id,
                to: id,
                kind: RelationKind::Contains,
                state: RelationState::Verified,
                evidence: "pinned_rust_cst_parent_child_relation".into(),
            });
            if error {
                defects.push(ParseDefect::ErrorRecoveryInsertedNode(SourceSpan {
                    start: node.span.start,
                    end: node.span.end,
                }));
            }
        }
        for siblings in syntax_children
            .iter()
            .filter(|(parent, _)| parent.is_some())
            .map(|(_, children)| children)
        {
            for pair in siblings.windows(2) {
                relations.push(SyntaxRelation {
                    from: pair[0],
                    to: pair[1],
                    kind: RelationKind::Sequence,
                    state: RelationState::Verified,
                    evidence: "pinned_rust_cst_sibling_order".into(),
                });
            }
        }
    }

    Ok(UnifiedSyntaxHypergraph {
        nodes,
        relations,
        derivations: vec![PackedDerivation {
            id: 1,
            root,
            production_ids: Vec::new(),
            status: format!("dg1_profile_parse:{}", crate::DG1_SCHEMA),
        }],
        defects,
    })
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
