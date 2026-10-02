use lc631_core::stable_sha256;
use serde::Serialize;
use std::collections::{BTreeSet, VecDeque};
use std::ops::Range;

pub const INSTRUCTION_GRAMMAR_REVISION: &str = "controlled-ja-en-instruction-earley.v2";
const INSTRUCTION_SCHEMA: &str = "epistesys-instruction-parse.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionMarker {
    Please,
    Must,
    MustNot,
    Should,
    ShouldNot,
    May,
    DoNot,
    JapaneseRequest,
    JapaneseForbidden,
    JapanesePermission,
    JapaneseMust,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionLanguage {
    English,
    Japanese,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionParseState {
    Parsed,
    Ambiguous,
    Unresolved,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionModality {
    Requested,
    Required,
    Recommended,
    Permitted,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionPolarity {
    Positive,
    Forbidden,
    Unknown,
    Conflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionRelation {
    Sufficient,
    Necessary,
    Exception,
    Temporal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalRelation {
    Before,
    After,
    Until,
    When,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionEvidenceState {
    Supported,
    Refuted,
    Unknown,
    Conflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionScopeState {
    Resolved,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConditionPredicate {
    pub source_span: Range<usize>,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "operands", rename_all = "snake_case")]
pub enum ConditionExpression {
    Predicate(ConditionPredicate),
    All(Vec<ConditionExpression>),
    Any(Vec<ConditionExpression>),
    Not(Box<ConditionExpression>),
    Unresolved(ConditionPredicate),
}

impl ConditionExpression {
    pub fn predicates(&self) -> Vec<&ConditionPredicate> {
        let mut predicates = Vec::new();
        self.collect_predicates(&mut predicates);
        predicates
    }

    fn collect_predicates<'a>(&'a self, output: &mut Vec<&'a ConditionPredicate>) {
        match self {
            Self::Predicate(predicate) | Self::Unresolved(predicate) => output.push(predicate),
            Self::All(children) | Self::Any(children) => {
                for child in children {
                    child.collect_predicates(output);
                }
            }
            Self::Not(child) => child.collect_predicates(output),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstructionCondition {
    pub source_span: Range<usize>,
    pub marker_span: Range<usize>,
    pub relation: ConditionRelation,
    pub temporal_relation: Option<TemporalRelation>,
    pub expression: ConditionExpression,
    pub applies_to_action_indices: Range<usize>,
    pub scope_state: ConditionScopeState,
    pub evidence_state: ConditionEvidenceState,
    pub exception_precedence: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionScopeKind {
    Resource,
    Branch,
    Environment,
    Time,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionScopeState {
    Candidate,
    Ambiguous,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstructionScopeCandidate {
    pub kind: InstructionScopeKind,
    pub value: String,
    pub source_span: Range<usize>,
    pub action_indices: Range<usize>,
    pub state: InstructionScopeState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionReferenceState {
    Candidate,
    Ambiguous,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstructionReferenceCandidate {
    pub text: String,
    pub source_span: Range<usize>,
    pub candidate_action_indices: Vec<usize>,
    pub state: InstructionReferenceState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionConstraintKind {
    Requires,
    Before,
    After,
    Until,
    When,
    Conditional,
    Exception,
    Conflicts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionEdgeState {
    Candidate,
    Unresolved,
    Conflict,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstructionConstraintNode {
    pub action_index: usize,
    pub source_span: Range<usize>,
    pub surface: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstructionConstraintEdge {
    pub from_action_indices: Range<usize>,
    pub to_action_indices: Range<usize>,
    pub kind: InstructionConstraintKind,
    pub source_span: Range<usize>,
    pub target_span: Option<Range<usize>>,
    pub state: InstructionEdgeState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstructionDependencyCycle {
    pub action_indices: Vec<usize>,
    pub edge_indices: Vec<usize>,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct InstructionConstraintGraph {
    pub nodes: Vec<InstructionConstraintNode>,
    pub edges: Vec<InstructionConstraintEdge>,
    pub cycles: Vec<InstructionDependencyCycle>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionConjunction {
    And,
    Or,
    Xor,
    But,
    Then,
    JapaneseTe,
    JapaneseComma,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionCoordinationState {
    Resolved,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SharedArgumentState {
    NotApplicable,
    UnresolvedCandidate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstructionAction {
    pub source_span: Range<usize>,
    pub text: String,
    pub marker_span: Option<Range<usize>>,
    pub modality: InstructionModality,
    pub polarity: InstructionPolarity,
    pub connector_before: Option<InstructionConjunction>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstructionCoordination {
    pub left_action_index: u32,
    pub right_action_index: u32,
    pub kind: InstructionConjunction,
    pub source_span: Range<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionPolarityScopeState {
    LocalAction,
    UnresolvedGroup,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstructionPolarityScope {
    pub action_indices: Range<usize>,
    pub polarity: InstructionPolarity,
    pub marker_span: Range<usize>,
    pub state: InstructionPolarityScopeState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct InstructionParseBudget {
    pub max_source_bytes: usize,
    pub max_tokens: usize,
    pub max_chart_items: usize,
    pub max_forest_nodes: usize,
    pub max_derivation_depth: usize,
}

impl Default for InstructionParseBudget {
    fn default() -> Self {
        Self {
            max_source_bytes: 65_536,
            max_tokens: 512,
            max_chart_items: 32_768,
            max_forest_nodes: 2_048,
            max_derivation_depth: 128,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstructionAst {
    pub production_id: u32,
    pub language: InstructionLanguage,
    pub marker: InstructionMarker,
    pub source_span: Range<usize>,
    pub marker_span: Range<usize>,
    pub body_span: Range<usize>,
    pub body: String,
    pub actions: Vec<InstructionAction>,
    pub coordination: Vec<InstructionCoordination>,
    pub coordination_state: InstructionCoordinationState,
    pub polarity_scopes: Vec<InstructionPolarityScope>,
    pub conditions: Vec<InstructionCondition>,
    pub scopes: Vec<InstructionScopeCandidate>,
    pub references: Vec<InstructionReferenceCandidate>,
    pub constraint_graph: InstructionConstraintGraph,
    pub shared_argument_state: SharedArgumentState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PackedInstructionNode {
    pub id: u32,
    pub symbol: String,
    pub source_span: Range<usize>,
    pub token_span: Range<usize>,
    pub production_ids: Vec<u32>,
    pub children: Vec<u32>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PackedInstructionForest {
    pub roots: Vec<u32>,
    pub nodes: Vec<PackedInstructionNode>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstructionParseReport {
    pub schema_version: &'static str,
    pub source_revision: String,
    pub grammar_revision: &'static str,
    pub state: InstructionParseState,
    pub selected: Option<InstructionAst>,
    pub alternatives: Vec<InstructionAst>,
    pub forest: PackedInstructionForest,
    pub chart_item_count: usize,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionGrammarError {
    InvalidBudget,
    SourceBudgetExceeded,
    TokenBudgetExceeded,
    ChartItemBudgetExceeded,
    ForestBudgetExceeded,
    DerivationDepthExceeded,
    SourceSpanInvalid,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Nonterminal {
    Augmented,
    Start,
    EnglishDirective,
    EnglishContextSequence,
    JapaneseDirective,
    BodySequence,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Terminal {
    EnglishMarker,
    Context,
    Content,
    JapaneseSuffix,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Symbol {
    Nonterminal(Nonterminal),
    Terminal(Terminal),
}

#[derive(Clone, Debug)]
struct Production {
    id: u32,
    lhs: Nonterminal,
    rhs: Vec<Symbol>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EarleyItem {
    production_index: usize,
    dot: usize,
    origin: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LexicalClass {
    EnglishMarker(InstructionMarker),
    Context,
    Content,
    JapaneseSuffix(InstructionMarker),
}

#[derive(Clone, Debug)]
struct LexicalToken {
    class: LexicalClass,
    span: Range<usize>,
}

#[derive(Clone, Debug)]
struct ControlledRoute {
    language: InstructionLanguage,
    marker: InstructionMarker,
    marker_span: Range<usize>,
    body_span: Range<usize>,
    source_span: Range<usize>,
    context_span: Option<Range<usize>>,
    tokens: Vec<LexicalToken>,
    root_production_id: u32,
    directive_production_id: u32,
    body_token_span: Range<usize>,
    marker_token_index: usize,
}

#[derive(Clone, Debug)]
struct ConditionSurface {
    source_span: Range<usize>,
    marker_span: Range<usize>,
    predicate_span: Range<usize>,
    relation: ConditionRelation,
    temporal_relation: Option<TemporalRelation>,
    negate_predicate: bool,
    exception_precedence: Option<u16>,
}

type ActionSplit = (
    Vec<Range<usize>>,
    Vec<(InstructionConjunction, Range<usize>)>,
    bool,
);

/// Parse only the declared English and Japanese instruction-surface grammar.
/// Unrecognized prose is returned as Unsupported; ambiguous routes are retained
/// in the packed forest and never reduced to a first-match answer.
pub fn parse_controlled_instruction(
    source: &str,
    budget: InstructionParseBudget,
) -> Result<InstructionParseReport, InstructionGrammarError> {
    validate_budget(budget)?;
    if source.len() > budget.max_source_bytes {
        return Err(InstructionGrammarError::SourceBudgetExceeded);
    }

    let routes = [english_route(source), japanese_route(source)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let token_count = routes.iter().map(|route| route.tokens.len()).sum::<usize>();
    if token_count > budget.max_tokens {
        return Err(InstructionGrammarError::TokenBudgetExceeded);
    }
    if routes
        .iter()
        .any(|route| route.tokens.len() > budget.max_derivation_depth)
    {
        return Err(InstructionGrammarError::DerivationDepthExceeded);
    }

    let grammar = instruction_productions();
    let mut alternatives = Vec::new();
    let mut forest = PackedInstructionForest::default();
    let mut total_chart_items = 0usize;
    for route in routes {
        let mut route_budget = budget;
        route_budget.max_chart_items = budget.max_chart_items.saturating_sub(total_chart_items);
        route_budget.max_forest_nodes = budget.max_forest_nodes.saturating_sub(forest.nodes.len());
        if route_budget.max_chart_items == 0 {
            return Err(InstructionGrammarError::ChartItemBudgetExceeded);
        }
        if route_budget.max_forest_nodes == 0 {
            return Err(InstructionGrammarError::ForestBudgetExceeded);
        }
        let (root_productions, chart_item_count) =
            earley_parse(&route.tokens, &grammar, route_budget)?;
        total_chart_items = total_chart_items
            .checked_add(chart_item_count)
            .ok_or(InstructionGrammarError::ChartItemBudgetExceeded)?;
        if root_productions.contains(&route.root_production_id) {
            let body = source
                .get(route.body_span.clone())
                .ok_or(InstructionGrammarError::SourceSpanInvalid)?
                .to_string();
            let (mut actions, coordination, coordination_state) =
                build_action_candidates(source, &route)?;
            let polarity_scopes = build_polarity_scopes(&route, &mut actions);
            let conditions = build_conditions(source, &route, &actions)?;
            let shared_argument_state = if actions.len() > 1 {
                SharedArgumentState::UnresolvedCandidate
            } else {
                SharedArgumentState::NotApplicable
            };
            let scopes = build_scope_candidates(source, &actions, &conditions);
            let references = build_reference_candidates(route.language, source, &actions);
            let constraint_graph = build_constraint_graph(&actions, &coordination, &conditions);
            alternatives.push(InstructionAst {
                production_id: route.root_production_id,
                language: route.language,
                marker: route.marker,
                source_span: route.source_span.clone(),
                marker_span: route.marker_span.clone(),
                body_span: route.body_span.clone(),
                body,
                actions,
                coordination,
                coordination_state,
                polarity_scopes,
                conditions,
                scopes,
                references,
                constraint_graph,
                shared_argument_state,
            });
            let root = build_route_forest(&route, &mut forest, budget.max_forest_nodes)?;
            forest.roots.push(root);
        }
    }

    alternatives.sort_by_key(|alternative| {
        (
            alternative.source_span.start,
            alternative.source_span.end,
            alternative.production_id,
        )
    });
    let state = match alternatives.len() {
        0 => InstructionParseState::Unsupported,
        1 if alternatives[0].coordination_state == InstructionCoordinationState::Resolved
            && !alternatives[0]
                .conditions
                .iter()
                .any(|condition| condition_expression_unresolved(&condition.expression))
            && !missing_explicit_scope_operand(source, &alternatives[0]) =>
        {
            InstructionParseState::Parsed
        }
        1 => InstructionParseState::Unresolved,
        _ => InstructionParseState::Ambiguous,
    };
    let selected = (state == InstructionParseState::Parsed)
        .then(|| alternatives.first().cloned())
        .flatten();
    Ok(InstructionParseReport {
        schema_version: INSTRUCTION_SCHEMA,
        source_revision: stable_sha256(source),
        grammar_revision: INSTRUCTION_GRAMMAR_REVISION,
        state,
        selected,
        alternatives,
        forest,
        chart_item_count: total_chart_items,
        claim_boundary: "bounded controlled instruction surface parse only; no semantic truth, authority, or complete natural-language interpretation",
    })
}

// An explicit resource production needs its operand. Definite noun references
// ("the file") are retained as lexical candidates, not guessed resources.
fn missing_explicit_scope_operand(source: &str, ast: &InstructionAst) -> bool {
    ast.actions.iter().any(|action| {
        let words = whitespace_words(&action.text);
        let Some((last_span, last)) = words.last() else {
            return true;
        };
        let absolute =
            action.source_span.start + last_span.start..action.source_span.start + last_span.end;
        if overlaps_quote(&absolute, &quoted_spans(source, &action.source_span)) {
            return false;
        }
        let last = normalize_marker_word(last);
        matches!(last.as_str(), "to" | "into")
            || (matches!(last.as_str(), "file" | "path" | "branch" | "resource")
                && words
                    .get(words.len().saturating_sub(2))
                    .is_none_or(|(_, prior)| {
                        !matches!(
                            normalize_marker_word(prior).as_str(),
                            "the" | "this" | "that" | "a"
                        )
                    }))
    })
}

fn validate_budget(budget: InstructionParseBudget) -> Result<(), InstructionGrammarError> {
    if budget.max_source_bytes == 0
        || budget.max_tokens == 0
        || budget.max_chart_items == 0
        || budget.max_forest_nodes == 0
        || budget.max_derivation_depth == 0
    {
        return Err(InstructionGrammarError::InvalidBudget);
    }
    Ok(())
}

fn instruction_productions() -> Vec<Production> {
    use Nonterminal::{
        BodySequence, EnglishContextSequence, EnglishDirective, JapaneseDirective, Start,
    };
    use Symbol::{Nonterminal as N, Terminal as T};
    vec![
        Production {
            id: 0,
            lhs: Nonterminal::Augmented,
            rhs: vec![N(Start)],
        },
        Production {
            id: 1,
            lhs: Start,
            rhs: vec![N(EnglishDirective)],
        },
        Production {
            id: 2,
            lhs: Start,
            rhs: vec![N(JapaneseDirective)],
        },
        Production {
            id: 3,
            lhs: EnglishDirective,
            rhs: vec![T(Terminal::EnglishMarker), N(BodySequence)],
        },
        Production {
            id: 4,
            lhs: BodySequence,
            rhs: vec![T(Terminal::Content)],
        },
        Production {
            id: 5,
            lhs: BodySequence,
            rhs: vec![T(Terminal::Content), N(BodySequence)],
        },
        Production {
            id: 6,
            lhs: JapaneseDirective,
            rhs: vec![T(Terminal::Content), T(Terminal::JapaneseSuffix)],
        },
        Production {
            id: 7,
            lhs: EnglishDirective,
            rhs: vec![
                N(EnglishContextSequence),
                T(Terminal::EnglishMarker),
                N(BodySequence),
            ],
        },
        Production {
            id: 8,
            lhs: EnglishContextSequence,
            rhs: vec![T(Terminal::Context)],
        },
        Production {
            id: 9,
            lhs: EnglishContextSequence,
            rhs: vec![T(Terminal::Context), N(EnglishContextSequence)],
        },
    ]
}

fn earley_parse(
    tokens: &[LexicalToken],
    grammar: &[Production],
    budget: InstructionParseBudget,
) -> Result<(BTreeSet<u32>, usize), InstructionGrammarError> {
    let mut chart = vec![BTreeSet::<EarleyItem>::new(); tokens.len() + 1];
    let mut agendas = vec![VecDeque::<EarleyItem>::new(); tokens.len() + 1];
    let mut item_count = 0usize;
    insert_item(
        &mut chart,
        &mut agendas,
        &mut item_count,
        0,
        EarleyItem {
            production_index: 0,
            dot: 0,
            origin: 0,
        },
        budget.max_chart_items,
    )?;

    let mut roots = BTreeSet::new();
    for position in 0..=tokens.len() {
        while let Some(item) = agendas[position].pop_front() {
            let production = &grammar[item.production_index];
            if let Some(symbol) = production.rhs.get(item.dot).copied() {
                match symbol {
                    Symbol::Nonterminal(expected) => {
                        for (production_index, candidate) in grammar.iter().enumerate().skip(1) {
                            if candidate.lhs == expected {
                                insert_item(
                                    &mut chart,
                                    &mut agendas,
                                    &mut item_count,
                                    position,
                                    EarleyItem {
                                        production_index,
                                        dot: 0,
                                        origin: position,
                                    },
                                    budget.max_chart_items,
                                )?;
                            }
                        }
                    }
                    Symbol::Terminal(expected) => {
                        if tokens
                            .get(position)
                            .is_some_and(|token| lexical_matches(expected, token.class))
                        {
                            insert_item(
                                &mut chart,
                                &mut agendas,
                                &mut item_count,
                                position + 1,
                                EarleyItem {
                                    dot: item.dot + 1,
                                    ..item
                                },
                                budget.max_chart_items,
                            )?;
                        }
                    }
                }
                continue;
            }

            if production.lhs == Nonterminal::Start && item.origin == 0 {
                roots.insert(production.id);
            }
            let completed_lhs = production.lhs;
            let parents = chart[item.origin].iter().copied().collect::<Vec<_>>();
            for parent in parents {
                let parent_production = &grammar[parent.production_index];
                if matches!(
                    parent_production.rhs.get(parent.dot),
                    Some(Symbol::Nonterminal(expected)) if *expected == completed_lhs
                ) {
                    insert_item(
                        &mut chart,
                        &mut agendas,
                        &mut item_count,
                        position,
                        EarleyItem {
                            dot: parent.dot + 1,
                            ..parent
                        },
                        budget.max_chart_items,
                    )?;
                }
            }
        }
    }

    Ok((roots, item_count))
}

fn insert_item(
    chart: &mut [BTreeSet<EarleyItem>],
    agendas: &mut [VecDeque<EarleyItem>],
    item_count: &mut usize,
    position: usize,
    item: EarleyItem,
    maximum: usize,
) -> Result<(), InstructionGrammarError> {
    if chart
        .get_mut(position)
        .ok_or(InstructionGrammarError::SourceSpanInvalid)?
        .insert(item)
    {
        *item_count = item_count
            .checked_add(1)
            .ok_or(InstructionGrammarError::ChartItemBudgetExceeded)?;
        if *item_count > maximum {
            return Err(InstructionGrammarError::ChartItemBudgetExceeded);
        }
        agendas[position].push_back(item);
    }
    Ok(())
}

fn lexical_matches(expected: Terminal, actual: LexicalClass) -> bool {
    matches!(
        (expected, actual),
        (Terminal::EnglishMarker, LexicalClass::EnglishMarker(_))
            | (Terminal::Context, LexicalClass::Context)
            | (Terminal::Content, LexicalClass::Content)
            | (Terminal::JapaneseSuffix, LexicalClass::JapaneseSuffix(_))
    )
}

fn build_route_forest(
    route: &ControlledRoute,
    forest: &mut PackedInstructionForest,
    maximum: usize,
) -> Result<u32, InstructionGrammarError> {
    let mut leaves = Vec::with_capacity(route.tokens.len());
    for (index, token) in route.tokens.iter().enumerate() {
        let symbol = match token.class {
            LexicalClass::EnglishMarker(_) => "EnglishMarker",
            LexicalClass::Context => "Context",
            LexicalClass::Content => "Content",
            LexicalClass::JapaneseSuffix(_) => "JapaneseSuffix",
        };
        leaves.push(push_forest_node(
            forest,
            maximum,
            symbol,
            token.span.clone(),
            index..index + 1,
            Vec::new(),
            Vec::new(),
        )?);
    }

    let directive_children = match route.language {
        InstructionLanguage::English => {
            let body_indices = route.body_token_span.clone().collect::<Vec<_>>();
            let body_end = body_indices
                .last()
                .copied()
                .ok_or(InstructionGrammarError::SourceSpanInvalid)?
                + 1;
            let mut body_node = None;
            for (reverse_index, token_index) in body_indices.iter().copied().rev().enumerate() {
                let (production_id, children) = if reverse_index == 0 {
                    (4, vec![leaves[token_index]])
                } else {
                    (
                        5,
                        vec![
                            leaves[token_index],
                            body_node.ok_or(InstructionGrammarError::SourceSpanInvalid)?,
                        ],
                    )
                };
                let token_span = token_index..body_end;
                let source_span =
                    route.tokens[token_index].span.start..route.tokens[body_end - 1].span.end;
                body_node = Some(push_forest_node(
                    forest,
                    maximum,
                    "BodySequence",
                    source_span,
                    token_span,
                    vec![production_id],
                    children,
                )?);
            }
            let body_node = body_node.ok_or(InstructionGrammarError::SourceSpanInvalid)?;
            if route.directive_production_id == 3 {
                vec![leaves[route.marker_token_index], body_node]
            } else {
                let context_indices = (0..route.marker_token_index).collect::<Vec<_>>();
                let context_end = route.marker_token_index;
                let mut context_node = None;
                for (reverse_index, token_index) in
                    context_indices.iter().copied().rev().enumerate()
                {
                    let (production_id, children) = if reverse_index == 0 {
                        (8, vec![leaves[token_index]])
                    } else {
                        (
                            9,
                            vec![
                                leaves[token_index],
                                context_node.ok_or(InstructionGrammarError::SourceSpanInvalid)?,
                            ],
                        )
                    };
                    let token_span = token_index..context_end;
                    let source_span = route.tokens[token_index].span.start
                        ..route.tokens[context_end - 1].span.end;
                    context_node = Some(push_forest_node(
                        forest,
                        maximum,
                        "EnglishContextSequence",
                        source_span,
                        token_span,
                        vec![production_id],
                        children,
                    )?);
                }
                vec![
                    context_node.ok_or(InstructionGrammarError::SourceSpanInvalid)?,
                    leaves[route.marker_token_index],
                    body_node,
                ]
            }
        }
        InstructionLanguage::Japanese => vec![leaves[0], leaves[1]],
    };
    let directive_production = route.directive_production_id;
    let directive_node = push_forest_node(
        forest,
        maximum,
        match route.language {
            InstructionLanguage::English => "EnglishDirective",
            InstructionLanguage::Japanese => "JapaneseDirective",
        },
        route.source_span.clone(),
        0..route.tokens.len(),
        vec![directive_production],
        directive_children,
    )?;
    push_forest_node(
        forest,
        maximum,
        "Start",
        route.source_span.clone(),
        0..route.tokens.len(),
        vec![route.root_production_id],
        vec![directive_node],
    )
}

fn push_forest_node(
    forest: &mut PackedInstructionForest,
    maximum: usize,
    symbol: &str,
    source_span: Range<usize>,
    token_span: Range<usize>,
    production_ids: Vec<u32>,
    children: Vec<u32>,
) -> Result<u32, InstructionGrammarError> {
    if forest.nodes.len() >= maximum {
        return Err(InstructionGrammarError::ForestBudgetExceeded);
    }
    let id = u32::try_from(forest.nodes.len())
        .map_err(|_| InstructionGrammarError::ForestBudgetExceeded)?;
    forest.nodes.push(PackedInstructionNode {
        id,
        symbol: symbol.into(),
        source_span,
        token_span,
        production_ids,
        children,
    });
    Ok(id)
}

fn english_route(source: &str) -> Option<ControlledRoute> {
    let words = whitespace_words(source);
    let first = words.first()?;
    let (marker_index, marker, marker_words) = if let Some(marker) = english_marker_at(&words, 0) {
        (0, marker.0, marker.1)
    } else {
        let candidate = (1..words.len())
            .find_map(|index| english_marker_at(&words, index).map(|marker| (index, marker)))?;
        let context_is_supported = is_supported_context_prefix(&words[..candidate.0]);
        if !context_is_supported {
            return None;
        }
        (candidate.0, candidate.1 .0, candidate.1 .1)
    };
    let marker_start = words.get(marker_index)?.0.start;
    let marker_end = words.get(marker_index + marker_words - 1)?.0.end;
    let body_words = words
        .iter()
        .skip(marker_index + marker_words)
        .filter(|(_, word)| word.chars().any(char::is_alphanumeric))
        .collect::<Vec<_>>();
    let first_body = body_words.first()?;
    let last_body = body_words.last()?;
    let body_span = first_body.0.start..last_body.0.end;
    let source_span = first.0.start..words.last()?.0.end;
    let context_span = (marker_index > 0).then(|| first.0.start..words[marker_index - 1].0.end);
    let mut tokens = words[..marker_index]
        .iter()
        .map(|(span, _)| LexicalToken {
            class: LexicalClass::Context,
            span: span.clone(),
        })
        .collect::<Vec<_>>();
    let marker_token_index = tokens.len();
    tokens.push(LexicalToken {
        class: LexicalClass::EnglishMarker(marker),
        span: marker_start..marker_end,
    });
    tokens.extend(body_words.into_iter().map(|(span, _)| LexicalToken {
        class: LexicalClass::Content,
        span: span.clone(),
    }));
    let body_token_start = marker_token_index + 1;
    let body_token_end = tokens.len();
    Some(ControlledRoute {
        language: InstructionLanguage::English,
        marker,
        marker_span: marker_start..marker_end,
        body_span,
        source_span,
        context_span,
        tokens,
        root_production_id: 1,
        directive_production_id: if marker_index == 0 { 3 } else { 7 },
        body_token_span: body_token_start..body_token_end,
        marker_token_index,
    })
}

fn english_marker_at(
    words: &[(Range<usize>, String)],
    index: usize,
) -> Option<(InstructionMarker, usize)> {
    let normalized = normalize_marker_word(&words.get(index)?.1);
    Some(match normalized.as_str() {
        "please" => (InstructionMarker::Please, 1),
        "must"
            if words
                .get(index + 1)
                .is_some_and(|word| normalize_marker_word(&word.1) == "not") =>
        {
            (InstructionMarker::MustNot, 2)
        }
        "must" => (InstructionMarker::Must, 1),
        "should"
            if words
                .get(index + 1)
                .is_some_and(|word| normalize_marker_word(&word.1) == "not") =>
        {
            (InstructionMarker::ShouldNot, 2)
        }
        "should" => (InstructionMarker::Should, 1),
        "may" => (InstructionMarker::May, 1),
        "do" if words
            .get(index + 1)
            .is_some_and(|word| normalize_marker_word(&word.1) == "not") =>
        {
            (InstructionMarker::DoNot, 2)
        }
        "don't" | "dont" => (InstructionMarker::DoNot, 1),
        "mustn't" | "mustnt" => (InstructionMarker::MustNot, 1),
        _ => return None,
    })
}

fn is_supported_context_word(word: &str) -> bool {
    matches!(
        normalize_marker_word(word).as_str(),
        "you" | "i" | "we" | "they" | "user" | "operator" | "system" | "caller" | "the"
    )
}

fn is_supported_context_prefix(words: &[(Range<usize>, String)]) -> bool {
    if words
        .iter()
        .all(|(_, word)| is_supported_context_word(word))
    {
        return true;
    }
    let Some((_, first)) = words.first() else {
        return false;
    };
    let first = normalize_marker_word(first);
    let second = words.get(1).map(|(_, word)| normalize_marker_word(word));
    matches!(
        first.as_str(),
        "if" | "when" | "unless" | "until" | "before" | "after"
    ) || (first == "only" && second.as_deref() == Some("if"))
}

fn marker_semantics(marker: InstructionMarker) -> (InstructionModality, InstructionPolarity) {
    match marker {
        InstructionMarker::Please | InstructionMarker::JapaneseRequest => (
            InstructionModality::Requested,
            InstructionPolarity::Positive,
        ),
        InstructionMarker::Must | InstructionMarker::JapaneseMust => {
            (InstructionModality::Required, InstructionPolarity::Positive)
        }
        InstructionMarker::MustNot | InstructionMarker::DoNot => (
            InstructionModality::Required,
            InstructionPolarity::Forbidden,
        ),
        InstructionMarker::Should => (
            InstructionModality::Recommended,
            InstructionPolarity::Positive,
        ),
        InstructionMarker::ShouldNot => (
            InstructionModality::Recommended,
            InstructionPolarity::Forbidden,
        ),
        InstructionMarker::May | InstructionMarker::JapanesePermission => (
            InstructionModality::Permitted,
            InstructionPolarity::Positive,
        ),
        InstructionMarker::JapaneseForbidden => (
            InstructionModality::Requested,
            InstructionPolarity::Forbidden,
        ),
    }
}

fn build_action_candidates(
    source: &str,
    route: &ControlledRoute,
) -> Result<
    (
        Vec<InstructionAction>,
        Vec<InstructionCoordination>,
        InstructionCoordinationState,
    ),
    InstructionGrammarError,
> {
    let (segments, connector_spans, split_unresolved) = match route.language {
        InstructionLanguage::English => split_english_actions(source, &route.body_span),
        InstructionLanguage::Japanese => split_japanese_actions(source, &route.body_span),
    };
    let mut actions = Vec::new();
    let mut invalid_segment = false;
    for (segment_index, segment) in segments.iter().enumerate() {
        let default_marker = if route.language == InstructionLanguage::Japanese
            && route.marker == InstructionMarker::JapaneseForbidden
            && segment_index + 1 < segments.len()
        {
            InstructionMarker::JapaneseRequest
        } else {
            route.marker
        };
        let Some(action) = build_action(
            source,
            segment,
            default_marker,
            segment_index == 0,
            segment_index + 1 == segments.len(),
            route,
        ) else {
            invalid_segment = true;
            continue;
        };
        actions.push(action);
    }
    let coordination_state = if split_unresolved
        || invalid_segment
        || connector_spans.len() != actions.len().saturating_sub(1)
    {
        InstructionCoordinationState::Unresolved
    } else {
        InstructionCoordinationState::Resolved
    };
    let mut coordination = Vec::new();
    for (index, (kind, source_span)) in connector_spans.into_iter().enumerate() {
        if index + 1 >= actions.len() {
            break;
        }
        if let Some(action) = actions.get_mut(index + 1) {
            action.connector_before = Some(kind);
        }
        coordination.push(InstructionCoordination {
            left_action_index: index as u32,
            right_action_index: index as u32 + 1,
            kind,
            source_span,
        });
    }
    Ok((actions, coordination, coordination_state))
}

fn build_polarity_scopes(
    route: &ControlledRoute,
    actions: &mut [InstructionAction],
) -> Vec<InstructionPolarityScope> {
    if actions.is_empty() {
        return Vec::new();
    }
    let outer_polarity = marker_semantics(route.marker).1;
    let has_local_marker = actions.iter().any(|action| {
        action
            .marker_span
            .as_ref()
            .is_some_and(|span| span != &route.marker_span)
    });
    if outer_polarity == InstructionPolarity::Forbidden
        && actions.len() > 1
        && !has_local_marker
        && actions
            .iter()
            .all(|action| action.polarity == InstructionPolarity::Forbidden)
    {
        for action in actions.iter_mut() {
            action.polarity = InstructionPolarity::Unknown;
        }
        return vec![InstructionPolarityScope {
            action_indices: 0..actions.len(),
            polarity: InstructionPolarity::Forbidden,
            marker_span: route.marker_span.clone(),
            state: InstructionPolarityScopeState::UnresolvedGroup,
        }];
    }
    actions
        .iter()
        .enumerate()
        .filter(|(_, action)| action.polarity == InstructionPolarity::Forbidden)
        .filter_map(|(index, action)| {
            Some(InstructionPolarityScope {
                action_indices: index..index + 1,
                polarity: InstructionPolarity::Forbidden,
                marker_span: action.marker_span.clone()?,
                state: InstructionPolarityScopeState::LocalAction,
            })
        })
        .collect()
}

fn split_english_actions(source: &str, body_span: &Range<usize>) -> ActionSplit {
    let Some(body) = source.get(body_span.clone()) else {
        return (Vec::new(), Vec::new(), true);
    };
    let words = whitespace_words(body);
    let mut segments = Vec::new();
    let mut connectors = Vec::new();
    let mut cursor = body_span.start;
    let mut unresolved = false;
    let quotes = quoted_spans(source, body_span);
    let condition_marker_end = find_english_condition_surface(source, body_span)
        .map(|condition| condition.marker_span.end);
    for (relative_span, word) in words {
        let kind = match normalize_marker_word(&word).as_str() {
            "and" => Some(InstructionConjunction::And),
            "or" => Some(InstructionConjunction::Or),
            "xor" => Some(InstructionConjunction::Xor),
            "but" => Some(InstructionConjunction::But),
            "then" => Some(InstructionConjunction::Then),
            _ => None,
        };
        if let Some(kind) = kind {
            let absolute =
                body_span.start + relative_span.start..body_span.start + relative_span.end;
            if overlaps_quote(&absolute, &quotes) {
                continue;
            }
            if condition_marker_end.is_some_and(|marker_end| absolute.start > marker_end) {
                continue;
            }
            if let Some(segment) = trim_source_whitespace(source, cursor..absolute.start) {
                segments.push(segment);
                connectors.push((kind, absolute.clone()));
            } else {
                unresolved = true;
            }
            cursor = absolute.end;
        }
    }
    if let Some(segment) = trim_source_whitespace(source, cursor..body_span.end) {
        segments.push(segment);
    } else if !connectors.is_empty() {
        unresolved = true;
    }
    (segments, connectors, unresolved)
}

fn split_japanese_actions(source: &str, body_span: &Range<usize>) -> ActionSplit {
    let mut segments = Vec::new();
    let mut connectors = Vec::new();
    let mut cursor = body_span.start;
    let mut unresolved = false;
    let condition_end = find_japanese_condition_surface(source, body_span)
        .map(|condition| condition.source_span.end)
        .unwrap_or(body_span.start);
    while cursor < body_span.end {
        let Some(_remaining) = source.get(cursor..body_span.end) else {
            return (Vec::new(), Vec::new(), true);
        };
        let search_span = cursor..body_span.end;
        let te = find_unquoted_substring(source, &search_span, "し、").map(|start| {
            (
                start - cursor,
                "し、".len(),
                InstructionConjunction::JapaneseTe,
            )
        });
        let comma = find_unquoted_substring(source, &search_span, "、").map(|start| {
            (
                start - cursor,
                '、'.len_utf8(),
                InstructionConjunction::JapaneseComma,
            )
        });
        let next = match (te, comma) {
            (Some(te), Some(comma)) if te.0 <= comma.0 => Some(te),
            (Some(_), Some(comma)) => Some(comma),
            (Some(te), None) => Some(te),
            (None, Some(comma)) => Some(comma),
            (None, None) => None,
        };
        let Some((relative_start, byte_len, kind)) = next else {
            if let Some(segment) = trim_source_whitespace(source, cursor..body_span.end) {
                segments.push(segment);
            } else if !connectors.is_empty() {
                unresolved = true;
            }
            break;
        };
        let separator_start = cursor + relative_start;
        let separator_end = separator_start + byte_len;
        if separator_start < condition_end {
            cursor = condition_end;
            continue;
        }
        if let Some(segment) = trim_source_whitespace(source, cursor..separator_start) {
            segments.push(segment);
            connectors.push((kind, separator_start..separator_end));
        } else {
            unresolved = true;
        }
        cursor = separator_end;
    }
    (segments, connectors, unresolved)
}

fn build_action(
    source: &str,
    segment: &Range<usize>,
    default_marker: InstructionMarker,
    first_action: bool,
    last_action: bool,
    route: &ControlledRoute,
) -> Option<InstructionAction> {
    let segment_text = source.get(segment.clone())?;
    let mut modality_polarity = marker_semantics(default_marker);
    let mut marker_span = match route.language {
        InstructionLanguage::English if first_action => Some(route.marker_span.clone()),
        InstructionLanguage::Japanese if last_action => Some(route.marker_span.clone()),
        _ => None,
    };
    let mut action_span = segment.clone();
    if route.language == InstructionLanguage::English {
        let words = whitespace_words(segment_text);
        if let Some((marker, marker_word_count)) = english_marker_at(&words, 0) {
            let marker_end = words.get(marker_word_count - 1)?.0.end;
            let next = words.get(marker_word_count)?;
            action_span = segment.start + next.0.start..segment.start + words.last()?.0.end;
            marker_span = Some(segment.start + words[0].0.start..segment.start + marker_end);
            modality_polarity = marker_semantics(marker);
        } else if let Some(condition) = find_english_condition_surface(source, segment) {
            action_span =
                trim_source_whitespace(source, segment.start..condition.marker_span.start)
                    .unwrap_or(condition.marker_span.start..condition.marker_span.start);
        }
    } else if let Some(japanese_action) = japanese_condition_action_span(source, segment) {
        action_span = japanese_action;
    }
    let action_text = source.get(action_span.clone())?.to_string();
    if !action_text.chars().any(char::is_alphanumeric) {
        return None;
    }
    Some(InstructionAction {
        source_span: action_span,
        text: action_text,
        marker_span,
        modality: modality_polarity.0,
        polarity: modality_polarity.1,
        connector_before: None,
    })
}

fn trim_source_whitespace(source: &str, span: Range<usize>) -> Option<Range<usize>> {
    let value = source.get(span.clone())?;
    let leading = value
        .char_indices()
        .find(|(_, character)| !character.is_whitespace())
        .map(|(offset, _)| offset)?;
    let trailing = value
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_whitespace())
        .map(|(offset, character)| offset + character.len_utf8())?;
    Some(span.start + leading..span.start + trailing)
}

fn trim_condition_surface(source: &str, span: Range<usize>) -> Option<Range<usize>> {
    let mut trimmed = trim_source_whitespace(source, span)?;
    loop {
        let slice = source.get(trimmed.clone())?;
        let (offset, last) = slice.char_indices().next_back()?;
        if matches!(
            last,
            '.' | ',' | ';' | '!' | '?' | '。' | '，' | '；' | '！' | '？'
        ) {
            trimmed.end = trimmed.start + offset;
            continue;
        }
        break;
    }
    trim_source_whitespace(source, trimmed)
}

fn quoted_spans(source: &str, span: &Range<usize>) -> Vec<Range<usize>> {
    let Some(text) = source.get(span.clone()) else {
        return Vec::new();
    };
    let mut stack = Vec::<(char, char, usize)>::new();
    let mut ranges = Vec::new();
    for (offset, character) in text.char_indices() {
        let absolute = span.start + offset;
        let opening = match character {
            '"' | '`' => Some((character, character)),
            '“' => Some(('“', '”')),
            '‘' => Some(('‘', '’')),
            '「' => Some(('「', '」')),
            '『' => Some(('『', '』')),
            _ => None,
        };
        if let Some((open, close)) = opening {
            if open == close
                && stack
                    .last()
                    .is_some_and(|(_, expected, _)| *expected == close)
            {
                if let Some((_, _, start)) = stack.pop() {
                    ranges.push(start..absolute + character.len_utf8());
                }
            } else {
                stack.push((open, close, absolute));
            }
        } else if stack
            .last()
            .is_some_and(|(_, expected, _)| *expected == character)
        {
            if let Some((_, _, start)) = stack.pop() {
                ranges.push(start..absolute + character.len_utf8());
            }
        }
    }
    for (_, _, start) in stack {
        ranges.push(start..span.end);
    }
    ranges.sort_by_key(|range| (range.start, range.end));
    ranges
}

fn overlaps_quote(span: &Range<usize>, quotes: &[Range<usize>]) -> bool {
    quotes
        .iter()
        .any(|quote| span.start < quote.end && quote.start < span.end)
}

fn find_unquoted_substring(source: &str, span: &Range<usize>, needle: &str) -> Option<usize> {
    let text = source.get(span.clone())?;
    let quotes = quoted_spans(source, span);
    let mut offset = 0usize;
    while let Some(position) = text[offset..].find(needle) {
        let start = offset + position;
        let absolute = span.start + start..span.start + start + needle.len();
        if !overlaps_quote(&absolute, &quotes) {
            return Some(start);
        }
        offset = start + needle.len();
        if offset >= text.len() {
            break;
        }
    }
    None
}

fn build_conditions(
    source: &str,
    route: &ControlledRoute,
    actions: &[InstructionAction],
) -> Result<Vec<InstructionCondition>, InstructionGrammarError> {
    let mut conditions = Vec::new();
    if let Some(context_span) = &route.context_span {
        if let Some(surface) = find_english_condition_surface(source, context_span) {
            let action_indices = 0..actions.len();
            conditions.push(condition_from_surface(
                source,
                surface,
                action_indices.clone(),
                if action_indices.len() == 1 {
                    ConditionScopeState::Resolved
                } else {
                    ConditionScopeState::Unresolved
                },
            )?);
        }
    }
    match route.language {
        InstructionLanguage::English => {
            if let Some(surface) = find_english_condition_surface(source, &route.body_span) {
                let action_index = actions
                    .iter()
                    .enumerate()
                    .rfind(|(_, action)| action.source_span.end <= surface.marker_span.start)
                    .map(|(index, _)| index);
                let (action_indices, scope_state) = match action_index {
                    Some(index) if actions.len() == 1 => {
                        (index..index + 1, ConditionScopeState::Resolved)
                    }
                    Some(_) => (0..actions.len(), ConditionScopeState::Unresolved),
                    None => (0..actions.len(), ConditionScopeState::Unresolved),
                };
                conditions.push(condition_from_surface(
                    source,
                    surface,
                    action_indices,
                    scope_state,
                )?);
            }
        }
        InstructionLanguage::Japanese => {
            if let Some(surface) = find_japanese_condition_surface(source, &route.body_span) {
                let action_index = actions
                    .iter()
                    .enumerate()
                    .find(|(_, action)| action.source_span.start >= surface.source_span.end)
                    .map(|(index, _)| index)
                    .or_else(|| {
                        actions
                            .iter()
                            .enumerate()
                            .rfind(|(_, action)| {
                                action.source_span.end <= surface.marker_span.start
                            })
                            .map(|(index, _)| index)
                    });
                let (action_indices, scope_state) = match action_index {
                    Some(index) if actions.len() == 1 => {
                        (index..index + 1, ConditionScopeState::Resolved)
                    }
                    Some(_) => (0..actions.len(), ConditionScopeState::Unresolved),
                    None => (0..actions.len(), ConditionScopeState::Unresolved),
                };
                conditions.push(condition_from_surface(
                    source,
                    surface,
                    action_indices,
                    scope_state,
                )?);
            }
        }
    }
    Ok(conditions)
}

fn build_scope_candidates(
    source: &str,
    actions: &[InstructionAction],
    conditions: &[InstructionCondition],
) -> Vec<InstructionScopeCandidate> {
    let mut scopes = Vec::new();
    for (action_index, action) in actions.iter().enumerate() {
        let Some(action_text) = source.get(action.source_span.clone()) else {
            continue;
        };
        let words = whitespace_words(action_text);
        for (index, (span, word)) in words.iter().enumerate() {
            let normalized = normalize_marker_word(word);
            if matches!(
                normalized.as_str(),
                "staging" | "production" | "development" | "test"
            ) && index > 0
                && matches!(
                    normalize_marker_word(&words[index - 1].1).as_str(),
                    "to" | "in" | "into" | "on" | "at"
                )
            {
                scopes.push(InstructionScopeCandidate {
                    kind: InstructionScopeKind::Environment,
                    value: normalized.clone(),
                    source_span: action.source_span.start + span.start
                        ..action.source_span.start + span.end,
                    action_indices: action_index..action_index + 1,
                    state: InstructionScopeState::Candidate,
                });
            }
            if matches!(normalized.as_str(), "branch" | "file" | "path" | "resource") {
                let Some((value_span, value)) = words.get(index + 1) else {
                    continue;
                };
                if !value.chars().any(char::is_alphanumeric) {
                    continue;
                }
                let kind = match normalized.as_str() {
                    "branch" => InstructionScopeKind::Branch,
                    _ => InstructionScopeKind::Resource,
                };
                scopes.push(InstructionScopeCandidate {
                    kind,
                    value: normalize_scope_value(value),
                    source_span: action.source_span.start + value_span.start
                        ..action.source_span.start + value_span.end,
                    action_indices: action_index..action_index + 1,
                    state: InstructionScopeState::Candidate,
                });
            }
        }
        if action_text.contains("環境") {
            for environment in ["staging", "production", "development"] {
                if let Some(position) = action_text.find(environment) {
                    scopes.push(InstructionScopeCandidate {
                        kind: InstructionScopeKind::Environment,
                        value: environment.into(),
                        source_span: action.source_span.start + position
                            ..action.source_span.start + position + environment.len(),
                        action_indices: action_index..action_index + 1,
                        state: InstructionScopeState::Candidate,
                    });
                }
            }
        }
    }
    for condition in conditions {
        if condition.temporal_relation.is_some()
            || condition.relation == ConditionRelation::Temporal
        {
            let value = condition
                .expression
                .predicates()
                .into_iter()
                .map(|predicate| predicate.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            scopes.push(InstructionScopeCandidate {
                kind: InstructionScopeKind::Time,
                value,
                source_span: condition.source_span.clone(),
                action_indices: condition.applies_to_action_indices.clone(),
                state: if condition.scope_state == ConditionScopeState::Resolved {
                    InstructionScopeState::Candidate
                } else {
                    InstructionScopeState::Ambiguous
                },
            });
        }
    }
    for action_index in 0..actions.len() {
        for kind in [
            InstructionScopeKind::Resource,
            InstructionScopeKind::Branch,
            InstructionScopeKind::Environment,
            InstructionScopeKind::Time,
        ] {
            let distinct = scopes
                .iter()
                .filter(|scope| {
                    scope.kind == kind && scope.action_indices == (action_index..action_index + 1)
                })
                .map(|scope| scope.value.as_str())
                .collect::<BTreeSet<_>>();
            if distinct.len() > 1 {
                for scope in scopes.iter_mut().filter(|scope| {
                    scope.kind == kind && scope.action_indices == (action_index..action_index + 1)
                }) {
                    scope.state = InstructionScopeState::Ambiguous;
                }
            }
        }
    }
    scopes.sort_by_key(|scope| {
        (
            scope.action_indices.start,
            scope.source_span.start,
            scope.kind as u8,
        )
    });
    scopes
}

fn normalize_scope_value(value: &str) -> String {
    value
        .trim_matches(|character: char| matches!(character, '.' | ',' | ':' | ';' | '!' | '?'))
        .to_lowercase()
}

fn build_reference_candidates(
    language: InstructionLanguage,
    source: &str,
    actions: &[InstructionAction],
) -> Vec<InstructionReferenceCandidate> {
    let mut references = Vec::new();
    for (action_index, action) in actions.iter().enumerate() {
        let candidates = (0..action_index).collect::<Vec<_>>();
        if language == InstructionLanguage::English {
            let Some(action_text) = source.get(action.source_span.clone()) else {
                continue;
            };
            let quotes = quoted_spans(source, &action.source_span);
            for (span, word) in whitespace_words(action_text) {
                let normalized = normalize_marker_word(&word);
                if !matches!(
                    normalized.as_str(),
                    "it" | "that" | "this" | "them" | "these" | "those" | "former" | "latter"
                ) {
                    continue;
                }
                let source_span =
                    action.source_span.start + span.start..action.source_span.start + span.end;
                if overlaps_quote(&source_span, &quotes) {
                    continue;
                }
                references.push(InstructionReferenceCandidate {
                    text: word,
                    source_span,
                    candidate_action_indices: candidates.clone(),
                    state: reference_state(candidates.len()),
                });
            }
        } else {
            for marker in ["それ", "これ", "あれ", "同じ"] {
                let mut search_start = action.source_span.start;
                while let Some(start) =
                    find_unquoted_substring(source, &(search_start..action.source_span.end), marker)
                {
                    let source_span = start..start + marker.len();
                    references.push(InstructionReferenceCandidate {
                        text: marker.into(),
                        source_span: source_span.clone(),
                        candidate_action_indices: candidates.clone(),
                        state: reference_state(candidates.len()),
                    });
                    search_start = source_span.end;
                    if search_start >= action.source_span.end {
                        break;
                    }
                }
            }
        }
    }
    references.sort_by_key(|reference| (reference.source_span.start, reference.source_span.end));
    references
}

fn reference_state(candidate_count: usize) -> InstructionReferenceState {
    match candidate_count {
        0 => InstructionReferenceState::Unresolved,
        1 => InstructionReferenceState::Candidate,
        _ => InstructionReferenceState::Ambiguous,
    }
}

fn build_constraint_graph(
    actions: &[InstructionAction],
    coordination: &[InstructionCoordination],
    conditions: &[InstructionCondition],
) -> InstructionConstraintGraph {
    let nodes = actions
        .iter()
        .enumerate()
        .map(|(action_index, action)| InstructionConstraintNode {
            action_index,
            source_span: action.source_span.clone(),
            surface: action.text.clone(),
        })
        .collect::<Vec<_>>();
    let mut edges = Vec::new();
    for link in coordination {
        if link.kind == InstructionConjunction::Then {
            edges.push(InstructionConstraintEdge {
                from_action_indices: link.left_action_index as usize
                    ..link.left_action_index as usize + 1,
                to_action_indices: link.right_action_index as usize
                    ..link.right_action_index as usize + 1,
                kind: InstructionConstraintKind::Before,
                source_span: link.source_span.clone(),
                target_span: actions
                    .get(link.right_action_index as usize)
                    .map(|action| action.source_span.clone()),
                state: InstructionEdgeState::Candidate,
            });
        }
    }
    for condition in conditions {
        let kind = match condition.relation {
            ConditionRelation::Necessary => InstructionConstraintKind::Requires,
            ConditionRelation::Sufficient => InstructionConstraintKind::Conditional,
            ConditionRelation::Exception => InstructionConstraintKind::Exception,
            ConditionRelation::Temporal => match condition.temporal_relation {
                Some(TemporalRelation::Before) => InstructionConstraintKind::Before,
                Some(TemporalRelation::After) => InstructionConstraintKind::After,
                Some(TemporalRelation::Until) => InstructionConstraintKind::Until,
                Some(TemporalRelation::When) | None => InstructionConstraintKind::When,
            },
        };
        edges.push(InstructionConstraintEdge {
            from_action_indices: condition.applies_to_action_indices.clone(),
            to_action_indices: 0..0,
            kind,
            source_span: condition.source_span.clone(),
            target_span: condition_expression_span(&condition.expression),
            state: if condition.scope_state == ConditionScopeState::Resolved
                && !condition_expression_unresolved(&condition.expression)
            {
                InstructionEdgeState::Candidate
            } else {
                InstructionEdgeState::Unresolved
            },
        });
    }
    for left in 0..actions.len() {
        for right in left + 1..actions.len() {
            if normalize_action_surface(&actions[left].text)
                == normalize_action_surface(&actions[right].text)
                && actions[left].polarity != actions[right].polarity
                && matches!(
                    (actions[left].polarity, actions[right].polarity),
                    (
                        InstructionPolarity::Positive,
                        InstructionPolarity::Forbidden
                    ) | (
                        InstructionPolarity::Forbidden,
                        InstructionPolarity::Positive
                    )
                )
            {
                edges.push(InstructionConstraintEdge {
                    from_action_indices: left..left + 1,
                    to_action_indices: right..right + 1,
                    kind: InstructionConstraintKind::Conflicts,
                    source_span: actions[left].source_span.start..actions[right].source_span.end,
                    target_span: Some(actions[right].source_span.clone()),
                    state: InstructionEdgeState::Conflict,
                });
            }
        }
    }
    let cycles = detect_dependency_cycles(actions.len(), &edges);
    InstructionConstraintGraph {
        nodes,
        edges,
        cycles,
    }
}

fn condition_expression_span(expression: &ConditionExpression) -> Option<Range<usize>> {
    let predicates = expression.predicates();
    let start = predicates
        .iter()
        .map(|predicate| predicate.source_span.start)
        .min()?;
    let end = predicates
        .iter()
        .map(|predicate| predicate.source_span.end)
        .max()?;
    Some(start..end)
}

fn condition_expression_unresolved(expression: &ConditionExpression) -> bool {
    match expression {
        ConditionExpression::Unresolved(_) => true,
        ConditionExpression::All(children) | ConditionExpression::Any(children) => {
            children.iter().any(condition_expression_unresolved)
        }
        ConditionExpression::Not(child) => condition_expression_unresolved(child),
        ConditionExpression::Predicate(_) => false,
    }
}

fn normalize_action_surface(surface: &str) -> String {
    surface
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn detect_dependency_cycles(
    action_count: usize,
    edges: &[InstructionConstraintEdge],
) -> Vec<InstructionDependencyCycle> {
    let mut adjacency = vec![Vec::<(usize, usize)>::new(); action_count];
    for (edge_index, edge) in edges.iter().enumerate() {
        if edge.from_action_indices.end != edge.from_action_indices.start + 1
            || edge.to_action_indices.end != edge.to_action_indices.start + 1
            || edge.from_action_indices.start >= action_count
            || edge.to_action_indices.start >= action_count
            || !matches!(
                edge.kind,
                InstructionConstraintKind::Requires
                    | InstructionConstraintKind::Before
                    | InstructionConstraintKind::After
            )
        {
            continue;
        }
        let (from, to) = if edge.kind == InstructionConstraintKind::After {
            (edge.to_action_indices.start, edge.from_action_indices.start)
        } else {
            (edge.from_action_indices.start, edge.to_action_indices.start)
        };
        adjacency[from].push((to, edge_index));
    }
    let mut states = vec![0_u8; action_count];
    let mut nodes = Vec::new();
    let mut path_edges = Vec::new();
    let mut cycles = Vec::<InstructionDependencyCycle>::new();
    for start in 0..action_count {
        if states[start] == 0 {
            visit_dependency_graph(
                start,
                &adjacency,
                &mut states,
                &mut nodes,
                &mut path_edges,
                &mut cycles,
            );
        }
    }
    cycles.sort_by_key(|cycle| cycle.action_indices.clone());
    cycles.dedup_by(|left, right| left.action_indices == right.action_indices);
    cycles
}

fn visit_dependency_graph(
    node: usize,
    adjacency: &[Vec<(usize, usize)>],
    states: &mut [u8],
    path_nodes: &mut Vec<usize>,
    path_edges: &mut Vec<usize>,
    cycles: &mut Vec<InstructionDependencyCycle>,
) {
    states[node] = 1;
    path_nodes.push(node);
    for &(target, edge_index) in &adjacency[node] {
        if states[target] == 0 {
            path_edges.push(edge_index);
            visit_dependency_graph(target, adjacency, states, path_nodes, path_edges, cycles);
            path_edges.pop();
        } else if states[target] == 1 {
            if let Some(position) = path_nodes.iter().position(|candidate| *candidate == target) {
                let mut action_indices = path_nodes[position..].to_vec();
                action_indices.sort_unstable();
                let mut edge_indices = path_edges[position..].to_vec();
                edge_indices.push(edge_index);
                edge_indices.sort_unstable();
                cycles.push(InstructionDependencyCycle {
                    action_indices,
                    edge_indices,
                    reason: "directed_candidate_dependency_cycle".into(),
                });
            }
        }
    }
    path_nodes.pop();
    states[node] = 2;
}

fn condition_from_surface(
    source: &str,
    surface: ConditionSurface,
    action_indices: Range<usize>,
    scope_state: ConditionScopeState,
) -> Result<InstructionCondition, InstructionGrammarError> {
    let mut expression = build_condition_expression(source, &surface.predicate_span, 0);
    if surface.negate_predicate {
        expression = ConditionExpression::Not(Box::new(expression));
    }
    let empty_predicate = surface.predicate_span.is_empty()
        || source
            .get(surface.predicate_span.clone())
            .is_none_or(|text| text.trim().is_empty());
    let scope_state = if empty_predicate {
        ConditionScopeState::Unresolved
    } else {
        scope_state
    };
    Ok(InstructionCondition {
        source_span: surface.source_span,
        marker_span: surface.marker_span,
        relation: surface.relation,
        temporal_relation: surface.temporal_relation,
        expression,
        applies_to_action_indices: action_indices,
        scope_state,
        evidence_state: ConditionEvidenceState::Unknown,
        exception_precedence: surface.exception_precedence,
    })
}

fn find_english_condition_surface(source: &str, span: &Range<usize>) -> Option<ConditionSurface> {
    let text = source.get(span.clone())?;
    let words = whitespace_words(text);
    let quotes = quoted_spans(source, span);
    for index in 0..words.len() {
        let word_span = span.start + words[index].0.start..span.start + words[index].0.end;
        if overlaps_quote(&word_span, &quotes) {
            continue;
        }
        let first = normalize_marker_word(&words[index].1);
        let (marker_words, relation, temporal_relation, negate_predicate, precedence) =
            if matches!(first.as_str(), "only" | "onlywhen")
                && words.get(index + 1).is_some_and(|word| {
                    matches!(normalize_marker_word(&word.1).as_str(), "if" | "when")
                        && !overlaps_quote(
                            &(span.start + word.0.start..span.start + word.0.end),
                            &quotes,
                        )
                })
            {
                (2, ConditionRelation::Necessary, None, false, None)
            } else {
                match first.as_str() {
                    "if" => (1, ConditionRelation::Sufficient, None, false, None),
                    "when" => (
                        1,
                        ConditionRelation::Temporal,
                        Some(TemporalRelation::When),
                        false,
                        None,
                    ),
                    "unless" => (1, ConditionRelation::Exception, None, true, Some(1)),
                    "until" => (
                        1,
                        ConditionRelation::Temporal,
                        Some(TemporalRelation::Until),
                        false,
                        None,
                    ),
                    "before" => (
                        1,
                        ConditionRelation::Temporal,
                        Some(TemporalRelation::Before),
                        false,
                        None,
                    ),
                    "after" => (
                        1,
                        ConditionRelation::Temporal,
                        Some(TemporalRelation::After),
                        false,
                        None,
                    ),
                    _ => continue,
                }
            };
        let marker_start = span.start + words[index].0.start;
        let marker_end = span.start + words[index + marker_words - 1].0.end;
        let predicate_span =
            trim_condition_surface(source, marker_end..span.end).unwrap_or(marker_end..marker_end);
        return Some(ConditionSurface {
            source_span: marker_start..predicate_span.end.max(marker_end),
            marker_span: marker_start..marker_end,
            predicate_span,
            relation,
            temporal_relation,
            negate_predicate,
            exception_precedence: precedence,
        });
    }
    None
}

fn find_japanese_condition_surface(source: &str, span: &Range<usize>) -> Option<ConditionSurface> {
    source.get(span.clone())?;
    if let Some(position) = find_unquoted_substring(source, span, "場合のみ") {
        let marker_start = span.start + position;
        let marker_end = marker_start + "場合のみ".len();
        let predicate_span = trim_condition_surface(source, span.start..marker_start)
            .unwrap_or(marker_start..marker_start);
        return Some(ConditionSurface {
            source_span: predicate_span.start..marker_end,
            marker_span: marker_start..marker_end,
            predicate_span,
            relation: ConditionRelation::Necessary,
            temporal_relation: None,
            negate_predicate: false,
            exception_precedence: None,
        });
    }
    for (marker, relation, temporal_relation, negate_predicate, precedence) in [
        (
            "ない限り",
            ConditionRelation::Exception,
            None,
            true,
            Some(1),
        ),
        (
            "まで",
            ConditionRelation::Temporal,
            Some(TemporalRelation::Until),
            false,
            None,
        ),
        (
            "前に",
            ConditionRelation::Temporal,
            Some(TemporalRelation::Before),
            false,
            None,
        ),
        (
            "後に",
            ConditionRelation::Temporal,
            Some(TemporalRelation::After),
            false,
            None,
        ),
    ] {
        if let Some(position) = find_unquoted_substring(source, span, marker) {
            let marker_start = span.start + position;
            let marker_end = marker_start + marker.len();
            let predicate_span = trim_condition_surface(source, span.start..marker_start)
                .unwrap_or(marker_start..marker_start);
            return Some(ConditionSurface {
                source_span: predicate_span.start..marker_end,
                marker_span: marker_start..marker_end,
                predicate_span,
                relation,
                temporal_relation,
                negate_predicate,
                exception_precedence: precedence,
            });
        }
    }
    if let Some(position) = find_unquoted_substring(source, span, "ただし") {
        let marker_start = span.start + position;
        let marker_end = marker_start + "ただし".len();
        let predicate_span =
            trim_condition_surface(source, marker_end..span.end).unwrap_or(marker_end..marker_end);
        return Some(ConditionSurface {
            source_span: marker_start..predicate_span.end.max(marker_end),
            marker_span: marker_start..marker_end,
            predicate_span,
            relation: ConditionRelation::Exception,
            temporal_relation: None,
            negate_predicate: false,
            exception_precedence: Some(1),
        });
    }
    if let (Some(conditional_start), Some((conditional_end, closing_marker))) = (
        find_unquoted_substring(source, span, "もし"),
        find_unquoted_substring(source, span, "たら")
            .map(|position| (position, "たら"))
            .or_else(|| {
                find_unquoted_substring(source, span, "なら").map(|position| (position, "なら"))
            }),
    ) {
        let marker_start = span.start + conditional_start;
        let predicate_start = marker_start + "もし".len();
        let closing_start = span.start + conditional_end;
        let marker_end = closing_start + closing_marker.len();
        let source_end = source
            .get(marker_end..span.end)
            .and_then(|suffix| {
                suffix
                    .starts_with('、')
                    .then_some(marker_end + '、'.len_utf8())
            })
            .unwrap_or(marker_end);
        if predicate_start <= closing_start {
            let predicate_span = trim_condition_surface(source, predicate_start..closing_start)
                .unwrap_or(predicate_start..predicate_start);
            return Some(ConditionSurface {
                source_span: marker_start..source_end,
                marker_span: marker_start..predicate_start,
                predicate_span,
                relation: ConditionRelation::Sufficient,
                temporal_relation: None,
                negate_predicate: false,
                exception_precedence: None,
            });
        }
    }
    None
}

fn japanese_condition_action_span(source: &str, segment: &Range<usize>) -> Option<Range<usize>> {
    let condition = find_japanese_condition_surface(source, segment)?;
    if condition.source_span.start == segment.start && condition.source_span.end < segment.end {
        return trim_source_whitespace(source, condition.source_span.end..segment.end);
    }
    if condition.relation == ConditionRelation::Exception
        && condition.source_span.start > segment.start
    {
        return trim_source_whitespace(source, segment.start..condition.marker_span.start);
    }
    if condition.source_span.end >= segment.end {
        return None;
    }
    None
}

fn build_condition_expression(
    source: &str,
    span: &Range<usize>,
    depth: usize,
) -> ConditionExpression {
    let Some(span) = trim_condition_surface(source, span.clone()) else {
        return ConditionExpression::Unresolved(ConditionPredicate {
            source_span: span.clone(),
            text: String::new(),
        });
    };
    let Some(text) = source.get(span.clone()) else {
        return ConditionExpression::Unresolved(ConditionPredicate {
            source_span: span,
            text: String::new(),
        });
    };
    if depth >= 16
        || !text.chars().any(char::is_alphanumeric)
        || matches!(
            normalize_marker_word(text).as_str(),
            "not" | "and" | "or" | "xor"
        )
    {
        return ConditionExpression::Unresolved(ConditionPredicate {
            source_span: span,
            text: text.to_string(),
        });
    }
    if !balanced_condition_parentheses(text) {
        return ConditionExpression::Unresolved(ConditionPredicate {
            source_span: span,
            text: text.to_string(),
        });
    }
    if let Some(inner) = strip_outer_condition_parentheses(&span, text) {
        return build_condition_expression(source, &inner, depth + 1);
    }
    if text.starts_with("not(") {
        if let Some(negated_span) = negated_condition_span(source, &span) {
            return ConditionExpression::Not(Box::new(build_condition_expression(
                source,
                &negated_span,
                depth + 1,
            )));
        }
    }
    let (parts, connector_kinds) = split_condition_boolean(source, &span);
    if parts.len() == 1 && connector_kinds == [InstructionConjunction::Xor] {
        return ConditionExpression::Unresolved(ConditionPredicate {
            source_span: span,
            text: text.to_string(),
        });
    }
    if parts.len() > 1 {
        if connector_kinds
            .iter()
            .all(|kind| *kind == InstructionConjunction::And)
        {
            return ConditionExpression::All(
                parts
                    .iter()
                    .map(|part| build_condition_expression(source, part, depth + 1))
                    .collect(),
            );
        }
        if connector_kinds
            .iter()
            .all(|kind| *kind == InstructionConjunction::Or)
        {
            return ConditionExpression::Any(
                parts
                    .iter()
                    .map(|part| build_condition_expression(source, part, depth + 1))
                    .collect(),
            );
        }
        return ConditionExpression::Unresolved(ConditionPredicate {
            source_span: span,
            text: text.to_string(),
        });
    }
    if let Some(negated_span) = negated_condition_span(source, &span) {
        return ConditionExpression::Not(Box::new(build_condition_expression(
            source,
            &negated_span,
            depth + 1,
        )));
    }
    ConditionExpression::Predicate(ConditionPredicate {
        source_span: span,
        text: text.to_string(),
    })
}

fn negated_condition_span(source: &str, span: &Range<usize>) -> Option<Range<usize>> {
    let text = source.get(span.clone())?;
    for prefix in ["not ", "not("] {
        if let Some(rest) = text.strip_prefix(prefix) {
            let start = span.start + prefix.len();
            let end = if prefix == "not(" && rest.ends_with(')') {
                span.end - 1
            } else {
                span.end
            };
            return trim_condition_surface(source, start..end);
        }
    }
    None
}

fn split_condition_boolean(
    source: &str,
    span: &Range<usize>,
) -> (Vec<Range<usize>>, Vec<InstructionConjunction>) {
    let Some(text) = source.get(span.clone()) else {
        return (vec![span.clone()], Vec::new());
    };
    let words = whitespace_words(text);
    let quotes = quoted_spans(source, span);
    let mut splits = Vec::<(Range<usize>, InstructionConjunction)>::new();
    for (word_span, word) in words {
        let normalized = word
            .trim_matches(|character: char| matches!(character, '(' | ')' | ',' | '.' | ';'))
            .to_lowercase();
        let kind = match normalized.as_str() {
            "and" => Some(InstructionConjunction::And),
            "or" => Some(InstructionConjunction::Or),
            "xor" => Some(InstructionConjunction::Xor),
            _ => None,
        };
        if let Some(kind) = kind {
            let word_span = span.start + word_span.start..span.start + word_span.end;
            if parenthesis_depth_before(text, word_span.start - span.start) == 0
                && !overlaps_quote(&word_span, &quotes)
            {
                splits.push((word_span, kind));
            }
        }
    }
    for (marker, kind) in [
        ("かつ", InstructionConjunction::And),
        ("または", InstructionConjunction::Or),
    ] {
        let mut offset = 0;
        while let Some(position) = text[offset..].find(marker) {
            let start = offset + position;
            let end = start + marker.len();
            let marker_span = span.start + start..span.start + end;
            if parenthesis_depth_before(text, start) == 0 && !overlaps_quote(&marker_span, &quotes)
            {
                splits.push((marker_span, kind));
            }
            offset = end;
            if offset >= text.len() {
                break;
            }
        }
    }
    splits.sort_by_key(|(marker_span, _)| marker_span.start);
    if splits.is_empty() {
        return (vec![span.clone()], Vec::new());
    }
    let mut parts = Vec::new();
    let mut kinds = Vec::new();
    let mut cursor = span.start;
    let mut malformed = false;
    for (marker_span, kind) in splits {
        if let Some(part) = trim_condition_surface(source, cursor..marker_span.start) {
            parts.push(part);
            kinds.push(kind);
            cursor = marker_span.end;
        } else {
            malformed = true;
        }
    }
    if let Some(part) = trim_condition_surface(source, cursor..span.end) {
        parts.push(part);
    }
    if malformed || parts.len() != kinds.len() + 1 || parts.iter().any(|part| part.is_empty()) {
        return (vec![span.clone()], vec![InstructionConjunction::Xor]);
    }
    (parts, kinds)
}

fn balanced_condition_parentheses(text: &str) -> bool {
    let mut depth = 0_i32;
    for character in text.chars() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn parenthesis_depth_before(text: &str, byte_offset: usize) -> i32 {
    text.get(..byte_offset)
        .unwrap_or_default()
        .chars()
        .fold(0_i32, |depth, character| match character {
            '(' => depth.saturating_add(1),
            ')' => depth.saturating_sub(1),
            _ => depth,
        })
}

fn strip_outer_condition_parentheses(span: &Range<usize>, text: &str) -> Option<Range<usize>> {
    if !text.starts_with('(') || !text.ends_with(')') {
        return None;
    }
    let mut depth = 0_i32;
    for (offset, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && offset + character.len_utf8() != text.len() {
                    return None;
                }
                if depth < 0 {
                    return None;
                }
            }
            _ => {}
        }
    }
    (depth == 0).then_some(span.start + 1..span.end - 1)
}

fn japanese_route(source: &str) -> Option<ControlledRoute> {
    const SUFFIXES: [(&str, InstructionMarker); 10] = [
        ("してはいけません", InstructionMarker::JapaneseForbidden),
        ("してはいけない", InstructionMarker::JapaneseForbidden),
        ("しないでください", InstructionMarker::JapaneseForbidden),
        ("しないで下さい", InstructionMarker::JapaneseForbidden),
        ("してください", InstructionMarker::JapaneseRequest),
        ("して下さい", InstructionMarker::JapaneseRequest),
        ("すること", InstructionMarker::JapaneseMust),
        ("禁止", InstructionMarker::JapaneseForbidden),
        ("許可", InstructionMarker::JapanesePermission),
        ("必須", InstructionMarker::JapaneseMust),
    ];
    let (source_start, source_end) = trimmed_span(source)?;
    let trim_end = source_end - trailing_surface_punctuation(&source[source_start..source_end]);
    let content = source.get(source_start..trim_end)?;
    let (suffix, marker) = SUFFIXES
        .iter()
        .find(|(suffix, _)| content.ends_with(suffix))?;
    let suffix_start = trim_end.checked_sub(suffix.len())?;
    let body_end = source[..suffix_start]
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_whitespace())
        .map(|(offset, character)| offset + character.len_utf8())?;
    if source_start >= body_end {
        return None;
    }
    let body_span = source_start..body_end;
    let marker_span = suffix_start..trim_end;
    Some(ControlledRoute {
        language: InstructionLanguage::Japanese,
        marker: *marker,
        source_span: source_start..source_end,
        context_span: None,
        marker_span: marker_span.clone(),
        body_span: body_span.clone(),
        tokens: vec![
            LexicalToken {
                class: LexicalClass::Content,
                span: body_span,
            },
            LexicalToken {
                class: LexicalClass::JapaneseSuffix(*marker),
                span: marker_span,
            },
        ],
        root_production_id: 2,
        directive_production_id: 6,
        body_token_span: 0..1,
        marker_token_index: 1,
    })
}

fn whitespace_words(source: &str) -> Vec<(Range<usize>, String)> {
    let mut words = Vec::new();
    let mut active_start = None;
    for (offset, character) in source.char_indices() {
        if character.is_whitespace() {
            if let Some(start) = active_start.take() {
                words.push((start..offset, source[start..offset].to_string()));
            }
        } else if active_start.is_none() {
            active_start = Some(offset);
        }
    }
    if let Some(start) = active_start {
        words.push((start..source.len(), source[start..].to_string()));
    }
    words
}

fn normalize_marker_word(word: &str) -> String {
    word.trim_matches(|character: char| matches!(character, '.' | ',' | ':' | ';' | '!' | '?'))
        .to_lowercase()
}

fn trimmed_span(source: &str) -> Option<(usize, usize)> {
    let start = source
        .char_indices()
        .find(|(_, character)| !character.is_whitespace())
        .map(|(offset, _)| offset)?;
    let end = source
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_whitespace())
        .map(|(offset, character)| offset + character.len_utf8())?;
    Some((start, end))
}

fn trailing_surface_punctuation(source: &str) -> usize {
    source
        .char_indices()
        .rev()
        .take_while(|(_, character)| matches!(character, '.' | '!' | '?' | '。' | '！' | '？'))
        .map(|(_, character)| character.len_utf8())
        .sum()
}

#[cfg(test)]
mod dependency_cycle_tests {
    use super::*;

    #[test]
    fn directed_candidate_constraints_report_cycles_without_authorizing_them() {
        let edges = vec![
            InstructionConstraintEdge {
                from_action_indices: 0..1,
                to_action_indices: 1..2,
                kind: InstructionConstraintKind::Before,
                source_span: 0..5,
                target_span: Some(6..7),
                state: InstructionEdgeState::Candidate,
            },
            InstructionConstraintEdge {
                from_action_indices: 1..2,
                to_action_indices: 0..1,
                kind: InstructionConstraintKind::Requires,
                source_span: 8..13,
                target_span: Some(0..1),
                state: InstructionEdgeState::Candidate,
            },
        ];
        let cycles = detect_dependency_cycles(2, &edges);
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].action_indices, vec![0, 1]);
        assert_eq!(cycles[0].edge_indices, vec![0, 1]);
        assert_eq!(cycles[0].reason, "directed_candidate_dependency_cycle");

        let conflict_only = vec![InstructionConstraintEdge {
            kind: InstructionConstraintKind::Conflicts,
            ..edges[0].clone()
        }];
        assert!(detect_dependency_cycles(2, &conflict_only).is_empty());
    }
}
