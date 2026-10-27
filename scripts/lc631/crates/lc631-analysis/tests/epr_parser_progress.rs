use lc631_analysis::{parse_controlled_instruction, InstructionParseBudget};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn bounded_parser_child() {
    let Ok(case) = std::env::var("EPR_PARSER_CASE") else {
        return;
    };
    let flood =
        (case == "clause_budget").then(|| format!("{}確認してください", "編し、".repeat(7000)));
    let source = match case.as_str() {
        "clause_budget" => flood.as_deref().unwrap(),
        "references" => "それそれを確認してください",
        "three_clauses" => "編集し、テストし、公開してください",
        "offset" => "補足です。\nこれこれを確認してください",
        "quoted" => "「それそれ、これ」を確認してください",
        "crlf" => "補足です。\r\nこれこれを確認してください",
        "unicode" => "😀 e\u{301} それそれを確認してください",
        "ten_clauses" => "編集し、検査し、計算し、比較し、保存し、再開し、確認し、整理し、構築し、報告してください",
        _ => panic!("unknown owned fixture"),
    };
    if case == "clause_budget" {
        let result = parse_controlled_instruction(
            source,
            InstructionParseBudget {
                max_tokens: 2,
                ..InstructionParseBudget::default()
            },
        );
        assert!(matches!(
            result,
            Err(lc631_analysis::InstructionGrammarError::PostprocessBudgetExceeded)
        ));
        return;
    }
    let report = parse_controlled_instruction(source, InstructionParseBudget::default()).unwrap();
    let ast = report
        .selected
        .as_ref()
        .or_else(|| report.alternatives.first())
        .unwrap();
    for reference in &ast.references {
        assert_eq!(
            source.get(reference.source_span.clone()),
            Some(reference.text.as_str())
        );
    }
    if case == "three_clauses" {
        assert_eq!(ast.actions.len(), 3);
    }
    if case == "ten_clauses" {
        assert_eq!(ast.actions.len(), 10);
    }
    if case == "references" || case == "offset" {
        assert_eq!(ast.references.len(), 2);
    }
    if case == "quoted" {
        assert!(ast.references.is_empty());
    }
}

fn run_case(case: &str) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "bounded_parser_child", "--nocapture"])
        .env("EPR_PARSER_CASE", case)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if start.elapsed() > Duration::from_millis(400) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("parser failed to make bounded progress: {case}");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{case}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn japanese_references_and_coordination_terminate_in_source_coordinates() {
    for case in [
        "references",
        "three_clauses",
        "offset",
        "quoted",
        "crlf",
        "unicode",
        "ten_clauses",
        "clause_budget",
    ] {
        run_case(case);
    }
}

#[test]
fn postprocessing_materialization_respects_the_token_budget() {
    let source = format!("{}を確認してください", "それ".repeat(32));
    let result = parse_controlled_instruction(
        &source,
        InstructionParseBudget {
            max_tokens: 4,
            ..InstructionParseBudget::default()
        },
    );
    assert!(
        result.is_err(),
        "materialized references must have a bounded budget"
    );
}
