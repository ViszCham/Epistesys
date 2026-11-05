//! 独立canonical vectorと署名scope負例。署名はMutationAuthorityでない。
//! Independent canonical vector and signed-scope negatives; signatures are not mutation authority.
use lc631_receipt_kernel::*;
use lc631_runtime::contextual_resume::budget::*;
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};
fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn manifest() -> BudgetRunManifest {
    BudgetRunManifest {
        binding: BudgetBinding {
            run_family: d(1),
            source: d(2),
            context: d(3),
            scope: d(4),
            goal: d(5),
            policy: d(6),
            root_target: d(7),
            initial_snapshot: d(8),
        },
        plan: d(9),
        caps: BudgetCaps {
            attempts: 2,
            per_attempt_delta_bytes: 10,
            total_delta_bytes: 20,
        },
        requests: vec![BudgetRequestSpec {
            request_digest: d(11),
            before_snapshot: d(8),
            intended_snapshot: d(10),
            reserved_delta_bytes: 7,
        }],
    }
}
fn text(d: Digest32) -> String {
    let mut s = String::from("sha256:");
    for b in d.as_bytes() {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
#[test]
fn tlcx32_verified_binding_independent_binary_vector_and_signed_mutants() {
    let m = manifest();
    // Independent declared grammar: 12-byte prefix,8 binding digests,plan,
    // four LE u64s,then request/before/after digests and LE reserved delta.
    let mut expected = Vec::new();
    expected.extend_from_slice(b"ETBRUN02\x02\0\0\0");
    for n in 1..=8 {
        expected.extend_from_slice(&[n; 32]);
    }
    expected.extend_from_slice(&[9; 32]);
    for n in [2_u64, 10, 20, 1] {
        expected.extend_from_slice(&n.to_le_bytes());
    }
    for n in [11, 8, 10] {
        expected.extend_from_slice(&[n; 32]);
    }
    expected.extend_from_slice(&7_u64.to_le_bytes());
    assert_eq!(expected.len(), 436);
    assert_eq!(m.canonical_bytes().unwrap(), expected);
    let mut h = Sha256::new();
    h.update(b"Epistesys/TLCX/RepairBudget/RunBinding/v2\0");
    h.update(&expected);
    assert_eq!(
        m.payload_digest().unwrap(),
        text(Digest32::from_bytes(h.finalize().into()))
    );
    let issuer = ReceiptIssuer::from_key_bytes("tlcx32-bound-fixture", &[94; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("tlcx32-bound-fixture", &[94; 32]).unwrap();
    let raw = issuer
        .issue(
            ReceiptClass::Validation,
            SubjectRevision::checked(text(m.binding.source)).unwrap(),
            ReceiptScope::checked(m.receipt_scope()).unwrap(),
            m.payload_digest().unwrap(),
            100,
            Some(200),
            None,
        )
        .unwrap();
    for cut in 0..6 {
        let mut altered = manifest();
        match cut {
            0 => altered.binding.source = d(40),
            1 => altered.plan = d(41),
            2 => altered.binding.root_target = d(42),
            3 => altered.requests[0].request_digest = d(43),
            4 => altered.caps.total_delta_bytes = 19,
            5 => altered.binding.run_family = d(44),
            _ => unreachable!(),
        }
        assert!(CheckedBudgetRun::verify(
            altered,
            raw.clone(),
            &verifier,
            &mut ReplayGuard::default(),
            100
        )
        .is_err());
    }
    assert!(CheckedBudgetRun::verify(
        manifest(),
        raw.clone(),
        &verifier,
        &mut ReplayGuard::default(),
        99
    )
    .is_err());
    assert!(CheckedBudgetRun::verify(
        manifest(),
        raw.clone(),
        &verifier,
        &mut ReplayGuard::default(),
        200
    )
    .is_err());
    let mut replay = ReplayGuard::default();
    assert!(CheckedBudgetRun::verify(manifest(), raw.clone(), &verifier, &mut replay, 199).is_ok());
    assert!(CheckedBudgetRun::verify(manifest(), raw, &verifier, &mut replay, 199).is_err());
}
#[test]
fn tlcx32_verified_binding_empty_zero_discontinuous_or_duplicate_requests_fail() {
    let mut empty = manifest();
    empty.requests.clear();
    assert!(empty.canonical_bytes().is_err());
    let mut zero = manifest();
    zero.requests[0].reserved_delta_bytes = 0;
    assert!(zero.canonical_bytes().is_err());
    let mut wrong = manifest();
    wrong.requests[0].before_snapshot = d(20);
    assert!(wrong.canonical_bytes().is_err());
    let mut duplicate = manifest();
    duplicate.requests.push(duplicate.requests[0]);
    assert!(duplicate.canonical_bytes().is_err());
}
