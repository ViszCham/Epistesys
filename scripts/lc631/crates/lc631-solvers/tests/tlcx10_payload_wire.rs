//! Syntax-only wire regression tests; neither models nor proofs are validated.
use lc631_solvers::protocol::payload::PayloadShape;
use lc631_solvers::protocol::payload_wire::{
    parse_payload_wire, PayloadWireError, PayloadWireStatus,
};
use lc631_solvers::protocol::sexpr::{SExprError, SExprLimits};
use lc631_solvers::protocol::QueryResultKind;

const MODEL_LF: &[u8] = b"sat\n((define-fun x () Int 2))\n";
const MODEL_CRLF: &[u8] = b"sat\r\n((define-fun x () Int 2))\r\n";
const PROOF: &[u8] = b"unsat\n((set-logic QF_LIA) (proof (asserted false)))\n";
fn limits() -> SExprLimits {
    SExprLimits::checked(4096, 512, 64).unwrap()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn wire_exact_source_spans_and_independent_domain_hash_vectors() {
    // SHA256(domain || LE64 length || raw) computed separately, not via parser.
    for (kind, raw, status, shape, status_end, start, end, digest) in [
        (
            QueryResultKind::ModelPayload,
            MODEL_LF,
            PayloadWireStatus::Sat,
            PayloadShape::Model,
            3,
            4,
            29,
            "d7fd46063f0fb8a3bf31e7c2057a5fb83e41e4b885884cf0f763e5bc0dae5a2f",
        ),
        (
            QueryResultKind::ModelPayload,
            MODEL_CRLF,
            PayloadWireStatus::Sat,
            PayloadShape::Model,
            3,
            5,
            30,
            "73b9a04b225f21798b7880e801b9de598a4e2f9e0a2090616fe7ca44a404c4ac",
        ),
        (
            QueryResultKind::ProofPayload,
            PROOF,
            PayloadWireStatus::Unsat,
            PayloadShape::Proof,
            5,
            6,
            51,
            "33f423e8094cf7c83d60448af773bc275d3e1948c9c55b982237047cac524262",
        ),
    ] {
        let view = parse_payload_wire(kind, raw, limits()).unwrap();
        assert_eq!(view.source_bytes(), raw);
        assert_eq!(view.status(), status);
        assert_eq!(view.shape(), shape);
        assert_eq!(
            (view.status_span().start(), view.status_span().end()),
            (0, status_end)
        );
        assert_eq!(
            (view.payload_span().start(), view.payload_span().end()),
            (start, end)
        );
        assert_eq!(view.summary().stdout_len() as usize, raw.len());
        assert_eq!(hex(&view.stdout_digest()), digest);
    }
}

#[test]
fn wire_rejects_status_scope_and_extra_roots() {
    for (kind, raw) in [
        (QueryResultKind::ModelPayload, b"unsat\n()\n".as_slice()),
        (QueryResultKind::ModelPayload, b"unknown\n()\n".as_slice()),
        (
            QueryResultKind::ProofPayload,
            b"sat\n((set-logic QF_LIA) (proof p))\n".as_slice(),
        ),
        (QueryResultKind::ModelPayload, b"sat\nsat\n()\n".as_slice()),
        (QueryResultKind::ModelPayload, b"sat\n()\n()\n".as_slice()),
        (
            QueryResultKind::ModelPayload,
            b"sat\n()\ntrailing\n".as_slice(),
        ),
        (
            QueryResultKind::ModelPayload,
            b"; leading\nsat\n()\n".as_slice(),
        ),
        (QueryResultKind::ModelPayload, b" sat\n()\n".as_slice()),
        (QueryResultKind::ModelPayload, b"sat ()\n".as_slice()),
        (
            QueryResultKind::ModelPayload,
            b"sat\n; interstitial\n()\n".as_slice(),
        ),
        (
            QueryResultKind::ModelPayload,
            b"sat\n()\n; trailing\n".as_slice(),
        ),
    ] {
        assert!(parse_payload_wire(kind, raw, limits()).is_err(), "{raw:?}");
    }
    assert!(matches!(
        parse_payload_wire(QueryResultKind::CounterexamplePayload, MODEL_LF, limits()),
        Err(PayloadWireError::UnsupportedMapping)
    ));
}

#[test]
fn wire_rejects_outside_narrow_shapes() {
    for raw in [
        b"sat\n((define-fun x () Bool 1))\n".as_slice(),
        b"sat\n((define-fun x () Int true))\n".as_slice(),
        b"sat\n((define-fun x ((a Int)) Int 2))\n".as_slice(),
        b"sat\n((define-fun x () Real 2))\n".as_slice(),
        b"sat\n((define-fun x () Int 01))\n".as_slice(),
        b"sat\n((define-fun x () Int -2))\n".as_slice(),
        b"sat\n((define-fun x () Int (- 0)))\n".as_slice(),
        b"sat\n((define-fun || () Int 2))\n".as_slice(),
        b"sat\n((define-fun |bad\\name| () Int 2))\n".as_slice(),
        b"sat\n(error \"not a model\")\n".as_slice(),
    ] {
        assert!(
            parse_payload_wire(QueryResultKind::ModelPayload, raw, limits()).is_err(),
            "{raw:?}"
        );
    }
    for raw in [
        b"unsat\n((set-logic QF_UF) (proof p))\n".as_slice(),
        b"unsat\n((set-logic QF_LIA) (proof (foo (error \"bad\"))))\n".as_slice(),
        b"unsat\n((set-logic QF_LIA) (proof p q))\n".as_slice(),
    ] {
        assert!(
            parse_payload_wire(QueryResultKind::ProofPayload, raw, limits()).is_err(),
            "{raw:?}"
        );
    }
}

#[test]
fn wire_rejects_every_prefix_before_payload_completion() {
    for (kind, bytes, payload_end) in [
        (QueryResultKind::ModelPayload, MODEL_LF, 29),
        (QueryResultKind::ModelPayload, MODEL_CRLF, 30),
        (QueryResultKind::ProofPayload, PROOF, 51),
    ] {
        for end in 0..payload_end {
            assert!(
                parse_payload_wire(kind, &bytes[..end], limits()).is_err(),
                "accepted prefix {end} of {bytes:?}"
            );
        }
        // Complete payload without terminal whitespace is valid; do not call it truncated.
        assert!(parse_payload_wire(kind, &bytes[..payload_end], limits()).is_ok());
    }
}

#[test]
fn wire_enforces_byte_node_depth_and_utf8_limits() {
    let cases = [
        SExprLimits::checked(MODEL_LF.len() - 1, 512, 64).unwrap(),
        SExprLimits::checked(4096, 1, 64).unwrap(),
        SExprLimits::checked(4096, 512, 1).unwrap(),
    ];
    for limit in cases {
        assert!(matches!(
            parse_payload_wire(QueryResultKind::ModelPayload, MODEL_LF, limit),
            Err(PayloadWireError::Syntax(_))
        ));
    }
    assert!(matches!(
        parse_payload_wire(QueryResultKind::ModelPayload, b"sat\n(\xff)\n", limits()),
        Err(PayloadWireError::Syntax(SExprError::InvalidUtf8 { .. }))
    ));
}
