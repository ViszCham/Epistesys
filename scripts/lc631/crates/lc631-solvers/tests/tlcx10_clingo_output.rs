use lc631_solvers::protocol::clingo::{
    parse_clingo_output, ClingoOutputError, ClingoOutputLimits, ClingoTerminalStatus,
};
use lc631_solvers::protocol::ValidationBasis;

fn limits(
    max_output_bytes: usize,
    max_answers: usize,
    max_atom_lines: usize,
) -> ClingoOutputLimits {
    ClingoOutputLimits::checked(max_output_bytes, max_answers, max_atom_lines).unwrap()
}

#[test]
fn tlcx10_clingo_582_timed_answer_header_preserves_exact_source_spans() {
    // Shape observed from the pinned pyclingo 5.8.2 executable profile.
    let output = "pyclingo version 5.8.2\r\nReading from -\r\nSolving...\r\nAnswer: 1 (Time: 0.008s)\r\np\r\nSATISFIABLE\r\n\r\nModels : 1+\r\n";
    let parsed = parse_clingo_output(output.as_bytes(), ClingoOutputLimits::default()).unwrap();
    assert_eq!(parsed.status(), ClingoTerminalStatus::Satisfiable);
    assert_eq!(parsed.answer_blocks().len(), 1);
    assert_eq!(parsed.answer_blocks()[0].number(), 1);
    assert_eq!(
        parsed.text(parsed.answer_blocks()[0].header_span()),
        Some("Answer: 1 (Time: 0.008s)")
    );
    let atoms = parsed.atom_lines_for(&parsed.answer_blocks()[0]).unwrap();
    assert_eq!(atoms.len(), 1);
    assert_eq!(parsed.text(atoms[0].span()), Some("p"));
    assert_eq!(parsed.validation_basis(), ValidationBasis::SolverReported);
}

#[test]
fn tlcx10_clingo_timed_headers_reject_malformed_or_ambiguous_suffixes() {
    for header in [
        "Answer: 1 (Time: NaNs)",
        "Answer: 1 (Time: infs)",
        "Answer: 1 (Time: -0.1s)",
        "Answer: 1 (Time: +0.1s)",
        "Answer: 1 (Time: 1e3s)",
        "Answer: 1 (Time: .1s)",
        "Answer: 1 (Time: 1.s)",
        "Answer: 1 (Time: 0.1ms)",
        "Answer: 1 (Time: 0.1s) extra",
        "Answer: 1 (Time: 0.1s) (Time: 0.2s)",
        "Answer: 1(Time: 0.1s)",
        "Answer: 01 (Time: 0.1s)",
        "Answer: 0 (Time: 0.1s)",
        "Answer: 1 (Time: ０.１s)",
    ] {
        let output = format!("{header}\np\nSATISFIABLE\n");
        assert!(
            matches!(
                parse_clingo_output(output.as_bytes(), ClingoOutputLimits::default()),
                Err(ClingoOutputError::MalformedAnswerHeader
                    | ClingoOutputError::MalformedAnswerNumber)
            ),
            "malformed header was accepted: {header}"
        );
    }
}

#[test]
fn tlcx10_clingo_preserves_answer_and_atom_byte_spans_across_crlf_and_utf8() {
    let output = "clingo 5.x\r\nAnswer: 1\r\np(猫)\r\nq(2)\r\nSATISFIABLE\r\nModels : 1\r\n";
    let parsed = parse_clingo_output(output.as_bytes(), ClingoOutputLimits::default()).unwrap();

    assert_eq!(parsed.status(), ClingoTerminalStatus::Satisfiable);
    assert_eq!(parsed.validation_basis(), ValidationBasis::SolverReported);
    assert_eq!(parsed.answer_blocks().len(), 1);
    assert_eq!(parsed.answer_blocks()[0].number(), 1);

    let atoms = parsed.atom_lines_for(&parsed.answer_blocks()[0]).unwrap();
    assert_eq!(atoms.len(), 2);
    assert_eq!(parsed.text(atoms[0].span()), Some("p(猫)"));
    assert_eq!(parsed.text(atoms[1].span()), Some("q(2)"));
    assert_eq!(
        atoms[0].span().start,
        output.find("p(猫)").expect("fixture contains the atom")
    );
    assert!(parsed.claim_boundary().contains("no atom truth"));
    assert!(parsed.claim_boundary().contains("no model validation"));
}

#[test]
fn tlcx10_clingo_accepts_status_only_unsat_and_unknown_without_inventing_answers() {
    for (output, expected) in [
        (
            b"UNSATISFIABLE\n".as_slice(),
            ClingoTerminalStatus::Unsatisfiable,
        ),
        (b"UNKNOWN\n".as_slice(), ClingoTerminalStatus::Unknown),
    ] {
        let parsed = parse_clingo_output(output, ClingoOutputLimits::default()).unwrap();
        assert_eq!(parsed.status(), expected);
        assert!(parsed.answer_blocks().is_empty());
        assert_eq!(parsed.validation_basis(), ValidationBasis::SolverReported);
    }
}

#[test]
fn tlcx10_clingo_rejects_unsatisfiable_status_after_any_answer_block() {
    assert!(matches!(
        parse_clingo_output(
            b"Answer: 1\na(1)\nUNSATISFIABLE\n",
            ClingoOutputLimits::default()
        ),
        Err(ClingoOutputError::ContradictoryAnswerStatus)
    ));
}

#[test]
fn tlcx10_clingo_does_not_require_answer_lines_for_satisfiable_status() {
    let parsed = parse_clingo_output(b"SATISFIABLE\n", ClingoOutputLimits::default()).unwrap();
    assert_eq!(parsed.status(), ClingoTerminalStatus::Satisfiable);
    assert!(parsed.answer_blocks().is_empty());
}

#[test]
fn tlcx10_clingo_rejects_missing_duplicate_and_out_of_order_status() {
    assert!(matches!(
        parse_clingo_output(b"Answer: 1\na(1)\n", ClingoOutputLimits::default()),
        Err(ClingoOutputError::MissingTerminalStatus)
    ));
    assert!(matches!(
        parse_clingo_output(
            b"SATISFIABLE\nUNSATISFIABLE\n",
            ClingoOutputLimits::default()
        ),
        Err(ClingoOutputError::DuplicateStatus)
    ));
    assert!(matches!(
        parse_clingo_output(
            b"SATISFIABLE\nAnswer: 1\na(1)\n",
            ClingoOutputLimits::default()
        ),
        Err(ClingoOutputError::StatusOutOfOrder)
    ));
    assert!(matches!(
        parse_clingo_output(
            b"Answer: 1\na(1)\nSATISFIABLE\nunknown-trailing-content\n",
            ClingoOutputLimits::default()
        ),
        Err(ClingoOutputError::UnexpectedAfterStatus)
    ));
}

#[test]
fn tlcx10_clingo_rejects_malformed_or_nonmonotonic_answer_numbers() {
    for output in [
        b"Answer: 0\nSATISFIABLE\n".as_slice(),
        b"Answer: 01\nSATISFIABLE\n".as_slice(),
        b"Answer: x\nSATISFIABLE\n".as_slice(),
    ] {
        assert!(matches!(
            parse_clingo_output(output, ClingoOutputLimits::default()),
            Err(ClingoOutputError::MalformedAnswerNumber)
                | Err(ClingoOutputError::MalformedAnswerHeader)
        ));
    }
    assert!(matches!(
        parse_clingo_output(
            b"Answer: 1\na(1)\nAnswer: 3\na(3)\nSATISFIABLE\n",
            ClingoOutputLimits::default()
        ),
        Err(ClingoOutputError::AnswerNumberOutOfOrder)
    ));
}

#[test]
fn tlcx10_clingo_enforces_output_answer_atom_and_utf8_budgets() {
    let one_answer = b"Answer: 1\na(1)\nSATISFIABLE\n";
    assert!(matches!(
        parse_clingo_output(one_answer, limits(8, 4, 4)),
        Err(ClingoOutputError::OutputTooLarge)
    ));

    let two_answers = b"Answer: 1\na(1)\nAnswer: 2\na(2)\nSATISFIABLE\n";
    assert!(matches!(
        parse_clingo_output(two_answers, limits(1024, 1, 10)),
        Err(ClingoOutputError::TooManyAnswers)
    ));

    assert!(matches!(
        parse_clingo_output(one_answer, limits(1024, 4, 0)),
        Err(ClingoOutputError::TooManyAtomLines)
    ));
    assert!(matches!(
        parse_clingo_output(
            b"Answer: 1\n\xff\nSATISFIABLE\n",
            ClingoOutputLimits::default()
        ),
        Err(ClingoOutputError::InvalidUtf8)
    ));
}

#[test]
fn tlcx10_clingo_never_claims_atom_truth_or_brave_cautious_entailment() {
    let parsed = parse_clingo_output(
        b"Answer: 1\na(1)\nSATISFIABLE\n",
        ClingoOutputLimits::default(),
    )
    .unwrap();

    assert_eq!(parsed.status(), ClingoTerminalStatus::Satisfiable);
    assert_eq!(parsed.validation_basis(), ValidationBasis::SolverReported);
    assert!(parsed.answer_blocks().len() == 1);
    assert_eq!(
        parsed
            .atom_lines_for(&parsed.answer_blocks()[0])
            .unwrap()
            .len(),
        1
    );
    assert!(parsed.claim_boundary().contains("no proof"));
}
