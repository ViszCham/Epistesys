//! 日本語: 新purpose/frame scopeの独立固定vector。報告snapshotは認証しない。
//! English: Independently fixed purpose/frame scope vectors; no snapshot authentication.
//! Planned missing-API RED. Literals come from the separate .NET SHA256 oracle.
#![forbid(unsafe_code)]
use lc631_analysis::{DgclExecutionSnapshot, DGCL_EVIDENCE_LIFECYCLE_SCHEMA};
use lc631_runtime::contextual_repair::{
    attempt::{native_attempt_scope, ProbePurpose},
    execution::ProbeToolRegistration,
    measurement::CodingProbePlan,
};
use lc631_substrate::Digest32;
use std::path::Path;

fn pin(byte: &str) -> String {
    format!("sha256:{}", byte.repeat(32))
}

#[test]
fn tlcx31_native_attempt_scope_has_frozen_domain_purpose_frame_and_registration_preimage() {
    // This is deliberately UNTRUSTED reported configuration for a pure hash.
    // It constructs no permit, current observation, checked history or process.
    let snapshot = DgclExecutionSnapshot {
        schema_version: DGCL_EVIDENCE_LIFECYCLE_SCHEMA.into(),
        source_revision: pin("07"),
        build_input_digest: pin("07"),
        profile_digest: pin("07"),
        validator_digest: pin("07"),
        authority_revision_digest: pin("07"),
        targets_digest: pin("07"),
        targets: Vec::new(),
        snapshot_digest: pin("05"),
        observed_boundary: "untrusted pure-hash vector; not an actual snapshot owner".into(),
    };
    let plan =
        CodingProbePlan::checked_add(Digest32::from_bytes([1; 32]), Digest32::from_bytes([2; 32]))
            .unwrap();
    assert_eq!(
        plan.digest().as_bytes(),
        &[
            0xc7, 0x83, 0x76, 0xc6, 0x58, 0xb2, 0xc6, 0x8f, 0x22, 0xe3, 0x77, 0x5d, 0xa0, 0x9d,
            0x68, 0xae, 0x60, 0x2b, 0xc6, 0x8b, 0x40, 0x61, 0x37, 0x61, 0x9c, 0x8e, 0x4e, 0x05,
            0x24, 0x8e, 0x5a, 0xde,
        ]
    );
    let binary = pin("06");
    let registration = ProbeToolRegistration {
        program: "/TLCX-FIXTURE/native",
        executable_digest: &binary,
        target_path: Path::new("/TLCX-FIXTURE/source.rs"),
        target_ref: "src/lib.rs",
        snapshot: &snapshot,
    };
    assert_eq!(
        registration.payload_digest(&plan),
        "sha256:2adda5a7bf5eff6671208ba526671a956ec1829a9a561c50847844d89efaed52"
    );
    // Normative bytes: UTF8("Epistesys/TLCX/CodingProbe/AttemptScope/v1\0")
    // || purpose u8(Baseline=0,Current=1) || frame32 || plan32
    // || parsed existing RegisteredArtifact payload digest32.
    // No Debug/JSON/ID order, phase tag, platform path rewrite or implicit grant.
    let vectors = [
        (ProbePurpose::Baseline, 3,
            "tlcx/coding-probe/attempt/test/v1/30f251034d28e95fc8ed7bd4f6806fd8d6ed29317e7da97ea243c296aef5f6ff"),
        (ProbePurpose::Current, 3,
            "tlcx/coding-probe/attempt/test/v1/2d9d0b8212b3dceab54fe2182c48e195be1af4e178070ab1d1c6cadd4d64476e"),
        (ProbePurpose::Baseline, 4,
            "tlcx/coding-probe/attempt/test/v1/06a3860d22084c937a4a97be925d66ebb98c4faa3f0d8f6b7f2226c99e1d8568"),
        (ProbePurpose::Current, 4,
            "tlcx/coding-probe/attempt/test/v1/408c625f9a85654c11a9176e5c5273d558466e0aad2ff797df771d795501161e"),
    ];
    for (purpose, frame_byte, expected) in vectors {
        let actual = native_attempt_scope(
            &plan,
            &registration,
            purpose,
            Digest32::from_bytes([frame_byte; 32]),
        );
        assert_eq!(actual, expected);
        assert_ne!(
            actual,
            registration.execution_scope(&plan),
            "legacy native Test scope cannot substitute for purpose+frame scope"
        );
    }
}
