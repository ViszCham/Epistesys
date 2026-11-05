use std::collections::{BTreeMap, BTreeSet};

use lc631_analysis::{
    build_dgcl_pipeline,
    context_lowering::lower_dgcl_context,
    logical_lowering::{
        lower_dgcl_logical, AtomRole, FormulaId, FormulaKind, LogicProfile, LogicalLoweringBudget,
        LogicalResidualReason, LogicalTask, TaskObligation, TaskObligationApplicability,
    },
    ConditionEvidenceState, ConditionRelation, DgclBackendSet,
};
use lc631_core::{
    context::{ContextLimits, ContextScope, SourceRevision, TaskKind},
    AuthorityRevision,
};
use lc631_runtime::solver_lowering::{
    prepare_abstract_condition_satisfiability, AbstractSatisfiabilityBudget,
    AbstractSatisfiabilityDecision, AbstractSatisfiabilityInterpretation,
    PreparedAbstractSatisfiabilityQuery,
};
use lc631_solvers::{runner::QuerySourceIdentity, typed_query::SmtEmitLimits};
use lc631_substrate::source::SourceSpan;

fn lower(source: &str) -> LogicalTask {
    let report = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let pin = SourceRevision::of("TLCX-10 independent abstract SAT oracle").digest();
    let context = lower_dgcl_context(
        &report,
        ContextScope::declared(pin, pin, pin, pin),
        AuthorityRevision(10),
        ContextLimits {
            max_contexts: 16,
            max_premises: 4096,
            max_edits: 32,
            max_statement_bytes: 262_144,
        },
    )
    .unwrap();
    lower_dgcl_logical(
        &report,
        &context,
        TaskKind::Reasoning,
        LogicProfile::ClassicalPropositionalV1,
        LogicalLoweringBudget::default(),
    )
    .unwrap()
}

fn prepare<'task>(task: &'task LogicalTask) -> PreparedAbstractSatisfiabilityQuery<'task> {
    let condition = task.conditions().first().expect("public condition input");
    match prepare_abstract_condition_satisfiability(
        task,
        condition.formula(),
        AbstractSatisfiabilityInterpretation::FreeBooleanAtoms,
        SmtEmitLimits::default(),
        AbstractSatisfiabilityBudget::default(),
    ) {
        AbstractSatisfiabilityDecision::Prepared(prepared) => prepared,
        AbstractSatisfiabilityDecision::Hold(held) => {
            panic!(
                "constructible public Boolean condition should prepare: {:?}",
                held.reason()
            )
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum OracleExpr {
    Constant(bool),
    Variable(u32),
    Not(Box<OracleExpr>),
    And(Vec<OracleExpr>),
    Or(Vec<OracleExpr>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OracleParseError {
    InvalidUtf8,
    MalformedQuery,
    CommandOutOfOrder,
    DuplicateCommand,
    TrailingCommand,
    DuplicateDeclaration,
    UndeclaredSymbol,
    UnusedDeclaration,
    UnsupportedSort,
    UnsupportedAtom,
    UnknownOperator,
    InvalidArity,
    UnexpectedClose,
    UnexpectedEnd,
    UnterminatedList,
    TrailingTokens,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Token {
    Open,
    Close,
    Atom,
}

#[derive(Debug)]
struct TokenizedExpr<'a> {
    tokens: Vec<(Token, &'a str)>,
}

#[derive(Debug, Eq, PartialEq)]
struct ParsedQuery {
    declarations: Vec<u32>,
    assertion: OracleExpr,
}

// このパーサーは抽象条件loweringが出力する小さなBoolean部分集合だけを受理し、本番emitter/evaluatorを再利用しない。
// This parser intentionally accepts only the small Boolean subset emitted by
// abstract condition lowering; it does not reuse the production SMT emitter or
// any production formula evaluator.
fn parse_payload(payload: &[u8]) -> Result<ParsedQuery, OracleParseError> {
    let text = std::str::from_utf8(payload).map_err(|_| OracleParseError::InvalidUtf8)?;
    let mut saw_logic = false;
    let mut saw_assertion = false;
    let mut saw_check_sat = false;
    let mut declarations = Vec::new();
    let mut declared = BTreeSet::new();
    let mut assertions = Vec::new();

    for line in text.lines() {
        if saw_check_sat {
            return Err(OracleParseError::TrailingCommand);
        }
        if line == "(set-logic QF_LIA)" {
            if saw_logic {
                return Err(OracleParseError::DuplicateCommand);
            }
            if !declarations.is_empty() || saw_assertion {
                return Err(OracleParseError::CommandOutOfOrder);
            }
            saw_logic = true;
            continue;
        }
        if let Some(declaration) = line
            .strip_prefix("(declare-const ")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            if !saw_logic || saw_assertion {
                return Err(OracleParseError::CommandOutOfOrder);
            }
            let mut parts = declaration.split_whitespace();
            let symbol = parts.next().ok_or(OracleParseError::MalformedQuery)?;
            let sort = parts.next().ok_or(OracleParseError::MalformedQuery)?;
            if parts.next().is_some() {
                return Err(OracleParseError::MalformedQuery);
            }
            if sort != "Bool" {
                return Err(OracleParseError::UnsupportedSort);
            }
            let id = parse_symbol(symbol).ok_or(OracleParseError::UnsupportedAtom)?;
            if !declared.insert(id) {
                return Err(OracleParseError::DuplicateDeclaration);
            }
            declarations.push(id);
            continue;
        }
        if let Some(rest) = line.strip_prefix("(assert ") {
            if !saw_logic {
                return Err(OracleParseError::CommandOutOfOrder);
            }
            if saw_assertion {
                return Err(OracleParseError::DuplicateCommand);
            }
            let expression = rest
                .strip_suffix(')')
                .ok_or(OracleParseError::MalformedQuery)?;
            assertions.push(parse_expression(expression)?);
            saw_assertion = true;
            continue;
        }
        if line == "(check-sat)" {
            if !saw_logic || !saw_assertion {
                return Err(OracleParseError::CommandOutOfOrder);
            }
            saw_check_sat = true;
            continue;
        }
        return Err(OracleParseError::MalformedQuery);
    }

    if !saw_logic || !saw_assertion || !saw_check_sat || assertions.len() != 1 {
        return Err(OracleParseError::MalformedQuery);
    }
    if declarations.is_empty() {
        return Err(OracleParseError::MalformedQuery);
    }
    let mut referenced = BTreeSet::new();
    collect_variables(&assertions[0], &mut referenced);
    if !referenced.is_subset(&declared) {
        return Err(OracleParseError::UndeclaredSymbol);
    }
    if referenced != declared {
        return Err(OracleParseError::UnusedDeclaration);
    }
    Ok(ParsedQuery {
        declarations,
        assertion: assertions.pop().expect("one assertion was checked"),
    })
}

fn tokenize(expression: &str) -> Result<TokenizedExpr<'_>, OracleParseError> {
    let bytes = expression.as_bytes();
    let mut cursor = 0;
    let mut tokens = Vec::new();
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if byte.is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        if !byte.is_ascii() {
            return Err(OracleParseError::UnsupportedAtom);
        }
        match byte {
            b'(' => {
                tokens.push((Token::Open, &expression[cursor..cursor + 1]));
                cursor += 1;
            }
            b')' => {
                tokens.push((Token::Close, &expression[cursor..cursor + 1]));
                cursor += 1;
            }
            _ => {
                let start = cursor;
                while cursor < bytes.len()
                    && !bytes[cursor].is_ascii_whitespace()
                    && bytes[cursor] != b'('
                    && bytes[cursor] != b')'
                {
                    if !bytes[cursor].is_ascii() {
                        return Err(OracleParseError::UnsupportedAtom);
                    }
                    cursor += 1;
                }
                tokens.push((Token::Atom, &expression[start..cursor]));
            }
        }
    }
    Ok(TokenizedExpr { tokens })
}

fn parse_expression(expression: &str) -> Result<OracleExpr, OracleParseError> {
    let tokenized = tokenize(expression)?;
    let mut cursor = 0;
    let parsed = parse_expression_tokens(&tokenized.tokens, &mut cursor)?;
    if cursor != tokenized.tokens.len() {
        return Err(OracleParseError::TrailingTokens);
    }
    Ok(parsed)
}

fn parse_expression_tokens(
    tokens: &[(Token, &str)],
    cursor: &mut usize,
) -> Result<OracleExpr, OracleParseError> {
    let (kind, text) = tokens
        .get(*cursor)
        .copied()
        .ok_or(OracleParseError::UnexpectedEnd)?;
    *cursor += 1;
    match kind {
        Token::Atom => match text {
            "true" => Ok(OracleExpr::Constant(true)),
            "false" => Ok(OracleExpr::Constant(false)),
            _ => parse_symbol(text)
                .map(OracleExpr::Variable)
                .ok_or(OracleParseError::UnsupportedAtom),
        },
        Token::Close => Err(OracleParseError::UnexpectedClose),
        Token::Open => {
            let (operator_kind, operator) = tokens
                .get(*cursor)
                .copied()
                .ok_or(OracleParseError::UnterminatedList)?;
            if operator_kind != Token::Atom {
                return Err(OracleParseError::UnknownOperator);
            }
            *cursor += 1;
            let mut arguments = Vec::new();
            loop {
                match tokens.get(*cursor).copied() {
                    Some((Token::Close, _)) => {
                        *cursor += 1;
                        break;
                    }
                    Some(_) => arguments.push(parse_expression_tokens(tokens, cursor)?),
                    None => return Err(OracleParseError::UnterminatedList),
                }
            }
            match operator {
                "not" if arguments.len() == 1 => Ok(OracleExpr::Not(Box::new(arguments.remove(0)))),
                "not" => Err(OracleParseError::InvalidArity),
                "and" if arguments.len() >= 2 => Ok(OracleExpr::And(arguments)),
                "or" if arguments.len() >= 2 => Ok(OracleExpr::Or(arguments)),
                "and" | "or" => Err(OracleParseError::InvalidArity),
                _ => Err(OracleParseError::UnknownOperator),
            }
        }
    }
}

fn parse_symbol(atom: &str) -> Option<u32> {
    let digits = atom.strip_prefix('b')?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

fn collect_variables(expression: &OracleExpr, output: &mut BTreeSet<u32>) {
    match expression {
        OracleExpr::Constant(_) => {}
        OracleExpr::Variable(symbol) => {
            output.insert(*symbol);
        }
        OracleExpr::Not(inner) => collect_variables(inner, output),
        OracleExpr::And(children) | OracleExpr::Or(children) => {
            for child in children {
                collect_variables(child, output);
            }
        }
    }
}

fn evaluate(
    expression: &OracleExpr,
    assignment: &BTreeMap<u32, bool>,
) -> Result<bool, OracleParseError> {
    match expression {
        OracleExpr::Constant(value) => Ok(*value),
        OracleExpr::Variable(symbol) => assignment
            .get(symbol)
            .copied()
            .ok_or(OracleParseError::UndeclaredSymbol),
        OracleExpr::Not(inner) => Ok(!evaluate(inner, assignment)?),
        OracleExpr::And(children) => {
            for child in children {
                if !evaluate(child, assignment)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        OracleExpr::Or(children) => {
            for child in children {
                if evaluate(child, assignment)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

fn truth_table(query: &ParsedQuery, source_order: &[u32]) -> Vec<bool> {
    assert!(
        source_order.len() <= 8,
        "the test oracle is intentionally small"
    );
    let declared: BTreeSet<_> = query.declarations.iter().copied().collect();
    let ordered: BTreeSet<_> = source_order.iter().copied().collect();
    assert_eq!(
        declared, ordered,
        "every free atom is declared exactly by the query"
    );

    (0..(1_usize << source_order.len()))
        .map(|mask| {
            let assignment: BTreeMap<_, _> = source_order
                .iter()
                .enumerate()
                .map(|(bit, symbol)| (*symbol, mask & (1 << bit) != 0))
                .collect();
            evaluate(&query.assertion, &assignment).expect("declared symbols have values")
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExpectedAtom {
    formula_id: FormulaId,
    symbol_id: u32,
    source_start: usize,
    source_end: usize,
    source_text: &'static str,
}

// 以下のbyte spanは下記ASCII source literalに対して手書きしたもの。
// 公開LogicalTask formula inventoryは各spanをFormulaIdとinventory indexに結び付けるだけで、prepared atom bindingは期待symbol IDやtruth-table順序の導出に使わない。
// 自然言語parseや意味の独立oracleではない。
// These byte spans are handwritten against the ASCII source literals below.
// The public LogicalTask formula inventory is used only to bind each expected
// span to its public FormulaId and inventory index; no prepared atom binding
// participates in deriving expected symbol ids or truth-table order. This is
// not an independent natural-language parser or semantics oracle.
fn expected_atom_inventory(
    task: &LogicalTask,
    expected_spans: &[(&'static str, usize, usize)],
) -> Vec<ExpectedAtom> {
    let source = task.source_store().text();
    assert!(
        source.is_ascii(),
        "handwritten spans are byte offsets in ASCII fixtures"
    );
    let condition_span = task
        .conditions()
        .first()
        .expect("one public condition")
        .span();
    for (text, start, end) in expected_spans {
        assert_eq!(source.get(*start..*end), Some(*text));
        assert!(*start >= condition_span.start());
        assert!(*end <= condition_span.end());
    }

    let all_condition_atoms: Vec<_> = task
        .formulas()
        .iter()
        .enumerate()
        .filter(|(_, formula)| {
            matches!(
                formula.kind(),
                FormulaKind::Atom {
                    role: AtomRole::UninterpretedCondition
                }
            )
        })
        .map(|(index, formula)| {
            (
                index,
                formula.id(),
                formula.span(),
                formula.span().start(),
                formula.span().end(),
            )
        })
        .collect();
    let mut selected_atoms: Vec<_> = all_condition_atoms
        .iter()
        .filter(|(_, _, span, _, _)| {
            span.start() >= condition_span.start() && span.end() <= condition_span.end()
        })
        .cloned()
        .collect();
    selected_atoms.sort_by_key(|(_, _, _, start, _)| *start);
    assert_eq!(
        selected_atoms.len(),
        expected_spans.len(),
        "the public formula inventory must have exactly the handwritten condition atoms"
    );
    assert_eq!(
        all_condition_atoms.len(),
        selected_atoms.len(),
        "these fixtures have one selected condition and no other condition atoms"
    );

    selected_atoms
        .into_iter()
        .zip(expected_spans)
        .map(
            |((index, formula_id, span, start, end), (text, expected_start, expected_end))| {
                assert_eq!(start, *expected_start);
                assert_eq!(end, *expected_end);
                assert_eq!(task.source_store().slice(span).unwrap(), *text);
                ExpectedAtom {
                    formula_id,
                    symbol_id: u32::try_from(index).expect("bounded public formula inventory"),
                    source_start: *expected_start,
                    source_end: *expected_end,
                    source_text: text,
                }
            },
        )
        .collect()
}

fn assert_inventory_matches_payload(
    task: &LogicalTask,
    prepared: &PreparedAbstractSatisfiabilityQuery<'_>,
    parsed: &ParsedQuery,
    expected_atoms: &[ExpectedAtom],
) -> Vec<u32> {
    let mut actual_bindings: Vec<_> = prepared
        .atom_bindings()
        .iter()
        .map(|binding| {
            let formula = task.program_ir().formula(binding.formula_id()).unwrap();
            let span = formula.span();
            (
                span.start(),
                span.end(),
                binding.formula_id(),
                binding.symbol().get(),
            )
        })
        .collect();
    actual_bindings.sort_by_key(|(start, _, _, _)| *start);
    let expected_bindings: Vec<_> = expected_atoms
        .iter()
        .map(|atom| {
            (
                atom.source_start,
                atom.source_end,
                atom.formula_id,
                atom.symbol_id,
            )
        })
        .collect();
    assert_eq!(actual_bindings, expected_bindings);

    let mut expected_declarations: Vec<_> =
        expected_atoms.iter().map(|atom| atom.symbol_id).collect();
    expected_declarations.sort_unstable();
    assert_eq!(parsed.declarations, expected_declarations);
    expected_atoms.iter().map(|atom| atom.symbol_id).collect()
}

fn expected_symbol_for_source(expected_atoms: &[ExpectedAtom], source_atom: &str) -> u32 {
    let found: Vec<_> = expected_atoms
        .iter()
        .filter(|atom| atom.source_text == source_atom)
        .map(|atom| atom.symbol_id)
        .collect();
    assert_eq!(
        found.len(),
        1,
        "expected one handwritten source atom: {source_atom}"
    );
    found[0]
}

fn query_and_task_state_is_read_only(
    task: &LogicalTask,
    prepared: &PreparedAbstractSatisfiabilityQuery<'_>,
) {
    assert_eq!(
        prepared.interpretation(),
        AbstractSatisfiabilityInterpretation::FreeBooleanAtoms
    );
    assert_eq!(
        prepared.binding().source_identity(),
        QuerySourceIdentity::from_bytes(*task.source_store().id().digest().as_bytes())
    );
    assert!(prepared
        .claim_boundary()
        .contains("not source truth, evidence, entailment, obligation completion, authority"));
    assert!(prepared.claim_boundary().contains("output commit"));
    assert_eq!(
        task.conditions()[0].reported_state(),
        ConditionEvidenceState::Unknown
    );
    assert!(task
        .residuals()
        .iter()
        .any(|residual| residual.reason() == LogicalResidualReason::UnknownCondition));
    assert_eq!(
        task.completion_plan().obligations().len(),
        task.obligations().len()
    );
}

#[derive(Debug, Eq, PartialEq)]
struct TaskSnapshot {
    conditions: Vec<(FormulaId, SourceSpan, ConditionEvidenceState)>,
    residuals: Vec<(SourceSpan, LogicalResidualReason)>,
    obligations: Vec<(FormulaId, TaskObligationApplicability, Vec<usize>)>,
    completion_obligations: Vec<(FormulaId, TaskObligationApplicability, Vec<usize>)>,
}

fn obligation_snapshot(
    items: &[TaskObligation],
) -> Vec<(FormulaId, TaskObligationApplicability, Vec<usize>)> {
    items
        .iter()
        .map(|obligation| {
            (
                obligation.target(),
                obligation.applicability(),
                obligation.condition_indices().to_vec(),
            )
        })
        .collect()
}

fn snapshot(task: &LogicalTask) -> TaskSnapshot {
    TaskSnapshot {
        conditions: task
            .conditions()
            .iter()
            .map(|condition| {
                (
                    condition.formula(),
                    condition.span(),
                    condition.reported_state(),
                )
            })
            .collect(),
        residuals: task
            .residuals()
            .iter()
            .map(|residual| (residual.span(), residual.reason()))
            .collect(),
        obligations: obligation_snapshot(task.obligations()),
        completion_obligations: obligation_snapshot(task.completion_plan().obligations()),
    }
}

enum StructureSpec {
    Not(&'static str),
    And(&'static str, &'static str),
    Or(&'static str, &'static str),
    AndThenOr(&'static str, &'static str, &'static str),
}

const UNKNOWN_READY_SPAN: &[(&str, usize, usize)] = &[("ready", 28, 33)];
const READY_AND_SAFE_SPANS: &[(&str, usize, usize)] = &[("ready", 28, 33), ("safe", 38, 42)];
const READY_OR_SAFE_SPANS: &[(&str, usize, usize)] = &[("ready", 28, 33), ("safe", 37, 41)];
const NOT_READY_SPANS: &[(&str, usize, usize)] = &[("ready", 32, 37)];
const NESTED_BOOLEAN_SPANS: &[(&str, usize, usize)] =
    &[("ready", 28, 33), ("safe", 39, 43), ("available", 47, 56)];

struct PublicSourceCase {
    source: &'static str,
    atom_spans: &'static [(&'static str, usize, usize)],
    structure: StructureSpec,
    truth_rows: &'static [bool],
}

fn expected_expression(expected_atoms: &[ExpectedAtom], spec: StructureSpec) -> OracleExpr {
    let variable = |name| OracleExpr::Variable(expected_symbol_for_source(expected_atoms, name));
    match spec {
        StructureSpec::Not(name) => OracleExpr::Not(Box::new(variable(name))),
        StructureSpec::And(left, right) => OracleExpr::And(vec![variable(left), variable(right)]),
        StructureSpec::Or(left, right) => OracleExpr::Or(vec![variable(left), variable(right)]),
        StructureSpec::AndThenOr(left, right_left, right_right) => OracleExpr::And(vec![
            variable(left),
            OracleExpr::Or(vec![variable(right_left), variable(right_right)]),
        ]),
    }
}

#[test]
fn tlcx10_unknown_atom_is_free_not_a_source_truth_assignment() {
    let task = lower("Please verify the result if ready.");
    let before = snapshot(&task);
    let prepared = prepare(&task);
    let parsed = parse_payload(prepared.smt_payload()).unwrap();
    let expected_atoms = expected_atom_inventory(&task, UNKNOWN_READY_SPAN);
    let source_order = assert_inventory_matches_payload(&task, &prepared, &parsed, &expected_atoms);
    let symbol = expected_symbol_for_source(&expected_atoms, "ready");

    assert_eq!(
        task.conditions()[0].reported_state(),
        ConditionEvidenceState::Unknown
    );
    assert!(prepared
        .selected_condition_residuals()
        .iter()
        .any(|residual| residual.reason() == LogicalResidualReason::UnknownCondition));
    assert_eq!(parsed.assertion, OracleExpr::Variable(symbol));
    assert_eq!(
        truth_table(&parsed, &source_order).as_slice(),
        &[false, true]
    );
    query_and_task_state_is_read_only(&task, &prepared);
    assert_eq!(snapshot(&task), before);
}

#[test]
fn tlcx10_emitted_boolean_structure_matches_constructible_public_conditions() {
    let cases = [
        PublicSourceCase {
            source: "Please verify the result if ready and safe.",
            atom_spans: READY_AND_SAFE_SPANS,
            structure: StructureSpec::And("ready", "safe"),
            truth_rows: &[false, false, false, true],
        },
        PublicSourceCase {
            source: "Please verify the result if ready or safe.",
            atom_spans: READY_OR_SAFE_SPANS,
            structure: StructureSpec::Or("ready", "safe"),
            truth_rows: &[false, true, true, true],
        },
        PublicSourceCase {
            source: "Please verify the result if not ready.",
            atom_spans: NOT_READY_SPANS,
            structure: StructureSpec::Not("ready"),
            truth_rows: &[true, false],
        },
        PublicSourceCase {
            source: "Please verify the result if ready and (safe or available).",
            atom_spans: NESTED_BOOLEAN_SPANS,
            structure: StructureSpec::AndThenOr("ready", "safe", "available"),
            truth_rows: &[false, false, false, true, false, true, false, true],
        },
    ];

    for case in cases {
        let task = lower(case.source);
        let before = snapshot(&task);
        let prepared = prepare(&task);
        let condition = &task.conditions()[0];
        let parsed = parse_payload(prepared.smt_payload()).unwrap();
        let expected_atoms = expected_atom_inventory(&task, case.atom_spans);
        let source_order =
            assert_inventory_matches_payload(&task, &prepared, &parsed, &expected_atoms);
        let expected = expected_expression(&expected_atoms, case.structure);
        let payload = std::str::from_utf8(prepared.smt_payload()).unwrap();

        assert_eq!(prepared.binding().formula_id(), condition.formula());
        assert_eq!(prepared.condition_span(), condition.span());
        assert_eq!(condition.reported_state(), ConditionEvidenceState::Unknown);
        assert_eq!(prepared.relation(), ConditionRelation::Sufficient);
        assert!(!prepared.guard_uses().is_empty());
        assert_eq!(parsed.assertion, expected, "source: {}", case.source);
        assert_eq!(
            truth_table(&parsed, &source_order).as_slice(),
            case.truth_rows,
            "source: {}",
            case.source
        );
        assert_eq!(parsed.declarations.len(), expected_atoms.len());

        // assertionにはcondition predicateだけがsource-bound atomとして入り、action textと外側のguardはSMT項にならない。
        // The condition predicates are the only source-bound atoms in the
        // assertion. Action text and the enclosing guard are not SMT terms.
        let selected_span = condition.span();
        for atom in prepared.atom_bindings() {
            let formula = task.program_ir().formula(atom.formula_id()).unwrap();
            assert!(formula.span().start() >= selected_span.start());
            assert!(formula.span().end() <= selected_span.end());
        }
        let action_ids: Vec<_> = task
            .formulas()
            .iter()
            .filter(|formula| {
                matches!(
                    formula.kind(),
                    FormulaKind::Atom {
                        role: AtomRole::UninterpretedAction
                    }
                )
            })
            .map(|formula| formula.id())
            .collect();
        let guard_ids: Vec<_> = task
            .formulas()
            .iter()
            .filter(|formula| matches!(formula.kind(), FormulaKind::Guarded { .. }))
            .map(|formula| formula.id())
            .collect();
        let directive_ids: Vec<_> = task
            .formulas()
            .iter()
            .filter(|formula| matches!(formula.kind(), FormulaKind::Directive { .. }))
            .map(|formula| formula.id())
            .collect();
        assert!(!action_ids.is_empty());
        assert!(!guard_ids.is_empty());
        assert!(!directive_ids.is_empty());
        assert!(prepared
            .atom_bindings()
            .iter()
            .all(|atom| !action_ids.contains(&atom.formula_id())
                && !guard_ids.contains(&atom.formula_id())
                && !directive_ids.contains(&atom.formula_id())));
        assert!(!payload.contains("verify"));
        assert!(!payload.contains("result"));
        assert!(!payload.contains("if"));

        query_and_task_state_is_read_only(&task, &prepared);
        assert_eq!(snapshot(&task), before);
    }
}

fn payload_with(expression: &str) -> String {
    format!(
        "(set-logic QF_LIA)\n(declare-const b0 Bool)\n(declare-const b1 Bool)\n(assert {expression})\n(check-sat)\n"
    )
}

#[test]
fn tlcx10_oracle_fails_closed_on_malformed_and_unknown_boolean_operators() {
    for (expression, expected) in [
        ("(xor b0 b1)", OracleParseError::UnknownOperator),
        ("(= b0 b1)", OracleParseError::UnknownOperator),
        ("(and)", OracleParseError::InvalidArity),
        ("(and b0)", OracleParseError::InvalidArity),
        ("(not b0 b1)", OracleParseError::InvalidArity),
        ("(and b0", OracleParseError::UnterminatedList),
        ("b0 b1", OracleParseError::TrailingTokens),
    ] {
        let payload = payload_with(expression);
        assert_eq!(
            parse_payload(payload.as_bytes()),
            Err(expected),
            "{expression}"
        );
    }

    let undeclared = "(set-logic QF_LIA)\n(declare-const b0 Bool)\n(assert b1)\n(check-sat)\n";
    assert_eq!(
        parse_payload(undeclared.as_bytes()),
        Err(OracleParseError::UndeclaredSymbol)
    );
}

#[test]
fn tlcx10_oracle_rejects_noncanonical_top_level_command_order_and_trailing_commands() {
    // 以下のfail-first例は、以前の存在確認だけの検査では受理され得たが、check-satがdeclaration/assertより前にある。
    // These fail-first cases would have passed the prior presence-only
    // command checks despite placing check-sat before the declarations or
    // assertion required by the emitted abstract payload grammar.
    for payload in [
        "(set-logic QF_LIA)\n(check-sat)\n(declare-const b0 Bool)\n(assert b0)\n",
        "(set-logic QF_LIA)\n(declare-const b0 Bool)\n(check-sat)\n(assert b0)\n",
        "(set-logic QF_LIA)\n(assert b0)\n(declare-const b0 Bool)\n(check-sat)\n",
        "(declare-const b0 Bool)\n(set-logic QF_LIA)\n(assert b0)\n(check-sat)\n",
    ] {
        assert_eq!(
            parse_payload(payload.as_bytes()),
            Err(OracleParseError::CommandOutOfOrder),
            "{payload:?}"
        );
    }

    let duplicate_logic = "(set-logic QF_LIA)\n(set-logic QF_LIA)\n(declare-const b0 Bool)\n(assert b0)\n(check-sat)\n";
    assert_eq!(
        parse_payload(duplicate_logic.as_bytes()),
        Err(OracleParseError::DuplicateCommand)
    );

    let duplicate_assertion =
        "(set-logic QF_LIA)\n(declare-const b0 Bool)\n(assert b0)\n(assert b0)\n(check-sat)\n";
    assert_eq!(
        parse_payload(duplicate_assertion.as_bytes()),
        Err(OracleParseError::DuplicateCommand)
    );

    let duplicate_declaration = "(set-logic QF_LIA)\n(declare-const b0 Bool)\n(declare-const b0 Bool)\n(assert b0)\n(check-sat)\n";
    assert_eq!(
        parse_payload(duplicate_declaration.as_bytes()),
        Err(OracleParseError::DuplicateDeclaration)
    );

    let trailing_command =
        "(set-logic QF_LIA)\n(declare-const b0 Bool)\n(assert b0)\n(check-sat)\n(exit)\n";
    assert_eq!(
        parse_payload(trailing_command.as_bytes()),
        Err(OracleParseError::TrailingCommand)
    );

    let repeated_check_sat =
        "(set-logic QF_LIA)\n(declare-const b0 Bool)\n(assert b0)\n(check-sat)\n(check-sat)\n";
    assert_eq!(
        parse_payload(repeated_check_sat.as_bytes()),
        Err(OracleParseError::TrailingCommand)
    );
}

// Append to tlcx10_abstract_sat_oracle.rs; reuses its private test helpers.
// These are syntax/provenance checks, not real-solver or source-truth evidence.

fn decide_at_scope_budget(task: &LogicalTask, budget: usize) -> AbstractSatisfiabilityDecision<'_> {
    prepare_abstract_condition_satisfiability(
        task,
        task.conditions()[0].formula(),
        AbstractSatisfiabilityInterpretation::FreeBooleanAtoms,
        SmtEmitLimits::default(),
        AbstractSatisfiabilityBudget::with_max_scope_work(budget),
    )
}

#[test]
fn tlcx10_scope_budget_covers_each_provenance_phase_and_exact_boundary() {
    use lc631_runtime::solver_lowering::AbstractSatisfiabilityHoldReason;

    let task = lower("Please verify the result if ready.");
    let before = snapshot(&task);
    let condition = &task.conditions()[0];
    let span = condition.span();
    let overlaps = |s: SourceSpan| s.start() < span.end() && span.start() < s.end();
    let selected: Vec<_> = task
        .residuals()
        .iter()
        .filter(|r| overlaps(r.span()))
        .collect();
    let predicate_count = selected
        .iter()
        .filter(|r| r.reason() == LogicalResidualReason::UninterpretedPredicate)
        .count();
    assert_eq!(predicate_count, 1);
    assert!(matches!(
        task.program_ir()
            .formula(condition.formula())
            .unwrap()
            .kind(),
        FormulaKind::Atom {
            role: AtomRole::UninterpretedCondition
        }
    ));

    // Independently account for the documented loops for this one-atom public fixture.
    // Check successive partial prefixes, including the late residual-to-atom join
    // and declaration stage. A deleted charge changes the exact-success boundary.
    let conditions_end = task.conditions().len();
    let residuals_end =
        conditions_end + task.residuals().len() + predicate_count * task.formulas().len();
    let guards = task.formulas().iter().filter(|f| matches!(f.kind(), FormulaKind::Guarded { predicate, .. } if *predicate == condition.formula())).count();
    assert!(guards > 0);
    let guards_end = residuals_end + task.formulas().len() + guards;
    let obligations_end = guards_end
        + task
            .obligations()
            .iter()
            .map(|o| 1 + o.condition_indices().len())
            .sum::<usize>();
    let atom_inventory_index = task
        .formulas()
        .iter()
        .position(|f| f.id() == condition.formula())
        .unwrap();
    // One node visit, one comparison for each selected residual, then the
    // inclusive scan to the atom's inventory index; active/previous atoms are empty.
    let lowering_end = obligations_end + 1 + selected.len() + atom_inventory_index + 1;
    let joins_end = lowering_end + selected.len() + predicate_count;
    let success_boundary = joins_end + 1; // one declared atom
    assert!(
        conditions_end < residuals_end
            && residuals_end < guards_end
            && guards_end < obligations_end
    );
    assert!(
        obligations_end < lowering_end && lowering_end < joins_end && joins_end < success_boundary
    );

    for (phase, exhausted_at) in [
        ("condition inventory", conditions_end - 1),
        ("residual matching", residuals_end - 1),
        ("guard use", guards_end - 1),
        ("obligation links", obligations_end - 1),
        ("formula and atom provenance", lowering_end - 1),
        ("selected residual-to-atom joins", joins_end - 1),
        ("declaration", success_boundary - 1),
    ] {
        match decide_at_scope_budget(&task, exhausted_at) {
            AbstractSatisfiabilityDecision::Hold(hold) => assert_eq!(
                hold.reason(),
                AbstractSatisfiabilityHoldReason::ScopeWorkLimitExceeded,
                "{phase}"
            ),
            AbstractSatisfiabilityDecision::Prepared(_) => panic!("budget skipped {phase}"),
        }
    }
    assert!(matches!(
        decide_at_scope_budget(&task, success_boundary),
        AbstractSatisfiabilityDecision::Prepared(_)
    ));
    assert_eq!(snapshot(&task), before);
}

#[test]
fn tlcx10_unvalidated_residuals_preserve_exact_index_reason_span_and_order() {
    let task = lower("Please verify the result if ready and safe.");
    let before = snapshot(&task);
    let condition_span = task.conditions()[0].span();
    let expected: Vec<_> = task
        .residuals()
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            let s = r.span();
            !(s.start() < condition_span.end() && condition_span.start() < s.end())
        })
        .map(|(index, r)| (u32::try_from(index).unwrap(), r.reason(), r.span()))
        .collect();
    assert!(
        !expected.is_empty(),
        "fixture must exercise an out-of-scope residual"
    );
    let prepared = prepare(&task);
    let observed: Vec<_> = prepared
        .unvalidated_task_residuals()
        .iter()
        .map(|r| (r.residual_index(), r.reason(), r.source_span()))
        .collect();
    assert_eq!(observed, expected);
    assert_eq!(prepared.unvalidated_task_residual_count(), expected.len());
    assert!(prepared
        .claim_boundary()
        .contains("unvalidated task residuals"));
    assert_eq!(snapshot(&task), before);
}

#[test]
fn tlcx10_oracle_detects_structure_and_symbol_detachment_mutants() {
    let task = lower("Please verify the result if ready and safe.");
    let expected_atoms = expected_atom_inventory(&task, READY_AND_SAFE_SPANS);
    let ready = expected_symbol_for_source(&expected_atoms, "ready");
    let safe = expected_symbol_for_source(&expected_atoms, "safe");
    let prepared = prepare(&task);
    let parsed = parse_payload(prepared.smt_payload()).unwrap();
    let source_order = assert_inventory_matches_payload(&task, &prepared, &parsed, &expected_atoms);
    let expected = OracleExpr::And(vec![
        OracleExpr::Variable(ready),
        OracleExpr::Variable(safe),
    ]);
    assert_eq!(parsed.assertion, expected);
    let expected_rows = [false, false, false, true];
    assert_eq!(truth_table(&parsed, &source_order), expected_rows);

    // Deliberately mutate the independent parsed tree, not producer bindings.
    // A swap alone is commutative, so use a sign change and AND-to-OR too.
    for mutant in [
        OracleExpr::Or(vec![
            OracleExpr::Variable(ready),
            OracleExpr::Variable(safe),
        ]),
        OracleExpr::And(vec![
            OracleExpr::Not(Box::new(OracleExpr::Variable(ready))),
            OracleExpr::Variable(safe),
        ]),
        OracleExpr::And(vec![
            OracleExpr::Variable(ready),
            OracleExpr::Variable(ready),
        ]),
    ] {
        let altered = ParsedQuery {
            declarations: parsed.declarations.clone(),
            assertion: mutant,
        };
        assert_ne!(altered.assertion, expected);
        assert_ne!(truth_table(&altered, &source_order), expected_rows);
    }
    query_and_task_state_is_read_only(&task, &prepared);
}
