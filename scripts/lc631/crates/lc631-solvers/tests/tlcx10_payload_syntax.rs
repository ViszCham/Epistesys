use lc631_solvers::protocol::payload::{parse_payload_syntax, PayloadShape, PayloadSyntaxError};
use lc631_solvers::protocol::sexpr::{SExprError, SExprKind, SExprLimits};
use lc631_solvers::protocol::{
    parse_status, query_protocol_compatibility, QueryProtocolCompatibility, QueryResultKind,
    SolverProtocol,
};

fn limits() -> SExprLimits {
    SExprLimits::checked(4096, 64, 16).unwrap()
}

#[test]
fn tlcx10_payload_adapter_maps_only_smt_payload_kinds_to_syntax_views() {
    for (kind, expected_shape, source) in [
        (
            QueryResultKind::ModelPayload,
            PayloadShape::Model,
            b"(model (define-fun x () Int 1))".as_slice(),
        ),
        (
            QueryResultKind::ProofPayload,
            PayloadShape::Proof,
            b"(proof (asserted (= x 1)))".as_slice(),
        ),
        (
            QueryResultKind::CounterexamplePayload,
            PayloadShape::Counterexample,
            b"(counterexample (x 2))".as_slice(),
        ),
    ] {
        let parsed =
            parse_payload_syntax(SolverProtocol::SmtLibPayloadV1, kind, source, limits()).unwrap();
        assert_eq!(parsed.requested_shape(), expected_shape);
        assert_eq!(parsed.document().source_bytes(), source);
        assert_eq!(parsed.document().roots().len(), 1);
        let root = parsed
            .document()
            .node(parsed.document().roots()[0])
            .unwrap();
        assert_eq!(root.kind(), SExprKind::List);
        assert_eq!(parsed.document().raw(root.id()), Some(source));
        assert!(!parsed.semantically_validated());
    }
}

#[test]
fn tlcx10_payload_adapter_keeps_status_and_non_smt_protocols_unsupported() {
    assert!(matches!(
        parse_payload_syntax(
            SolverProtocol::SmtLibPayloadV1,
            QueryResultKind::Satisfiability,
            b"sat\n",
            limits(),
        ),
        Err(PayloadSyntaxError::UnsupportedMapping {
            protocol: SolverProtocol::SmtLibPayloadV1,
            result_kind: QueryResultKind::Satisfiability
        })
    ));
    assert!(matches!(
        parse_payload_syntax(
            SolverProtocol::ClingoStatusV1,
            QueryResultKind::ModelPayload,
            b"(model)",
            limits(),
        ),
        Err(PayloadSyntaxError::UnsupportedMapping {
            protocol: SolverProtocol::ClingoStatusV1,
            result_kind: QueryResultKind::ModelPayload
        })
    ));
}

#[test]
fn tlcx10_payload_adapter_retains_nested_sexpr_syntax_failures() {
    assert!(matches!(
        parse_payload_syntax(
            SolverProtocol::SmtLibPayloadV1,
            QueryResultKind::ModelPayload,
            b"(model (define-fun x () Int 1)",
            limits(),
        ),
        Err(PayloadSyntaxError::Syntax(SExprError::UnclosedList {
            offset: 0
        }))
    ));
    assert!(matches!(
        parse_payload_syntax(
            SolverProtocol::SmtLibPayloadV1,
            QueryResultKind::ProofPayload,
            b"(proof \xff)",
            limits(),
        ),
        Err(PayloadSyntaxError::Syntax(SExprError::InvalidUtf8 { .. }))
    ));
    assert!(matches!(
        parse_payload_syntax(
            SolverProtocol::SmtLibPayloadV1,
            QueryResultKind::CounterexamplePayload,
            b"(counterexample (x \"unfinished))",
            limits(),
        ),
        Err(PayloadSyntaxError::Syntax(
            SExprError::UnclosedString { .. }
        ))
    ));
}

#[test]
fn tlcx10_payload_adapter_propagates_parser_resource_limits() {
    let small = SExprLimits::checked(8, 8, 4).unwrap();
    assert!(matches!(
        parse_payload_syntax(
            SolverProtocol::SmtLibPayloadV1,
            QueryResultKind::ModelPayload,
            b"(model (define-fun x () Int 1))",
            small,
        ),
        Err(PayloadSyntaxError::Syntax(SExprError::InputTooLarge { .. }))
    ));
}

#[test]
fn tlcx10_payload_protocol_is_separate_from_legacy_status_protocol() {
    assert_eq!(
        query_protocol_compatibility(
            SolverProtocol::SmtLibStatusV1,
            QueryResultKind::Satisfiability
        ),
        QueryProtocolCompatibility::Supported
    );
    assert_eq!(
        query_protocol_compatibility(
            SolverProtocol::SmtLibStatusV1,
            QueryResultKind::ModelPayload
        ),
        QueryProtocolCompatibility::Unsupported
    );
    assert_eq!(
        query_protocol_compatibility(
            SolverProtocol::SmtLibPayloadV1,
            QueryResultKind::ModelPayload
        ),
        QueryProtocolCompatibility::Supported
    );
    assert!(matches!(
        parse_payload_syntax(
            SolverProtocol::SmtLibStatusV1,
            QueryResultKind::ModelPayload,
            b"(model)",
            limits(),
        ),
        Err(PayloadSyntaxError::UnsupportedMapping {
            protocol: SolverProtocol::SmtLibStatusV1,
            result_kind: QueryResultKind::ModelPayload
        })
    ));
    assert_eq!(
        parse_status(
            SolverProtocol::SmtLibStatusV1,
            QueryResultKind::Satisfiability,
            b"sat\n",
            16,
        ),
        Ok(lc631_solvers::LogicalResult::Satisfiability(
            lc631_solvers::Satisfiability::Sat
        ))
    );
}
