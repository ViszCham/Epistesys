//! Public status-view agreement; these bytes are not native solver observations.
use lc631_solvers::protocol::clingo::{parse_clingo_output, ClingoOutputError, ClingoOutputLimits};
use lc631_solvers::protocol::{parse_status, ProtocolError, QueryResultKind, SolverProtocol};
use lc631_solvers::{LogicalResult, Satisfiability};

#[test]
fn tlcx10_public_clingo_status_cannot_bypass_terminal_shape_rejection() {
    let fixtures: &[(&[u8], ClingoOutputError)] = &[
        (
            b"SATISFIABLE\nunknown-trailing-content\n",
            ClingoOutputError::UnexpectedAfterStatus,
        ),
        (
            b"Answer: 1\na0\nUNSATISFIABLE\n",
            ClingoOutputError::ContradictoryAnswerStatus,
        ),
        (
            b"SATISFIABLE\nAnswer: 1\na0\n",
            ClingoOutputError::StatusOutOfOrder,
        ),
        (
            b"SATISFIABLE\nOPTIMUM FOUND\n",
            ClingoOutputError::UnsupportedStatus,
        ),
        (
            b"Answer: 2\na0\nSATISFIABLE\n",
            ClingoOutputError::AnswerNumberOutOfOrder,
        ),
    ];
    let mut accepted = Vec::new();
    for (index, (output, expected)) in fixtures.iter().enumerate() {
        assert_eq!(
            parse_clingo_output(output, ClingoOutputLimits::default()).err(),
            Some(*expected),
        );
        let parsed = parse_status(
            SolverProtocol::ClingoStatusV1,
            QueryResultKind::StableModelExistence,
            output,
            4096,
        );
        if let Ok(result) = parsed {
            accepted.push((index, result));
        } else {
            let expected_protocol = match expected {
                ClingoOutputError::UnexpectedAfterStatus
                | ClingoOutputError::ContradictoryAnswerStatus
                | ClingoOutputError::StatusOutOfOrder => ProtocolError::DuplicateOrTrailingStatus,
                ClingoOutputError::UnsupportedStatus => ProtocolError::UnknownStatus,
                ClingoOutputError::AnswerNumberOutOfOrder => ProtocolError::ClingoOutput(*expected),
                _ => panic!("unexpected test fixture"),
            };
            assert_eq!(parsed, Err(expected_protocol));
        }
    }
    assert!(accepted.is_empty(), "public status bypasses: {accepted:?}");
}

#[test]
fn tlcx10_public_clingo_status_keeps_supported_statuses_without_entailment_promotion() {
    for (output, expected) in [
        (
            b"pyclingo version 5.8.2\r\nAnswer: 1 (Time: 0.008s)\r\na0\r\nSATISFIABLE\r\nModels : 1+\r\n".as_slice(),
            Satisfiability::Sat,
        ),
        (b"UNSATISFIABLE\n".as_slice(), Satisfiability::Unsat),
        (b"UNKNOWN\n".as_slice(), Satisfiability::Unknown),
        (b"SATISFIABLE\n".as_slice(), Satisfiability::Sat),
    ] {
        assert_eq!(
            parse_status(
                SolverProtocol::ClingoStatusV1,
                QueryResultKind::StableModelExistence,
                output,
                4096,
            ),
            Ok(LogicalResult::Satisfiability(expected)),
        );
        assert!(parse_status(
            SolverProtocol::ClingoStatusV1,
            QueryResultKind::Inference,
            output,
            4096,
        )
        .is_err());
    }
}

#[test]
fn tlcx10_public_clingo_status_keeps_typed_byte_utf8_missing_and_answer_caps() {
    for (output, limit, expected) in [
        (
            b"SATISFIABLE\n".as_slice(),
            1,
            ProtocolError::OutputTooLarge,
        ),
        (
            b"\xff\nSATISFIABLE\n".as_slice(),
            64,
            ProtocolError::InvalidUtf8,
        ),
        (
            b"Answer: 1\na0\n".as_slice(),
            64,
            ProtocolError::MissingStatus,
        ),
    ] {
        assert_eq!(
            parse_status(
                SolverProtocol::ClingoStatusV1,
                QueryResultKind::StableModelExistence,
                output,
                limit,
            ),
            Err(expected),
        );
    }
    let mut output = String::new();
    for number in 1..=257 {
        output.push_str(&format!("Answer: {number}\n\n"));
    }
    output.push_str("SATISFIABLE\n");
    assert_eq!(
        parse_status(
            SolverProtocol::ClingoStatusV1,
            QueryResultKind::StableModelExistence,
            output.as_bytes(),
            8192,
        ),
        Err(ProtocolError::ClingoOutput(
            ClingoOutputError::TooManyAnswers
        )),
    );
}
