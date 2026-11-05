//! 日本語: CR-COMPRESSIONの既存K6/算術境界だけを監査。私有のleaf producerは未実装。
//! English: Audit existing K6/arithmetic boundaries only; no checked compression
//! producer is implemented here. ReportedChecked below is explicitly REPORTED,
//! never an independently checked preservation token or RuntimeObserved truth.
//! Declared decoder/dictionary TERMS are cost inputs, not executed decoder proof.
use lc631_analysis::creativity_measurement::{
    compression_gain, MeasurementError, PreservationPrerequisite, RationalInterval,
};
use lc631_core::context::SourceRevision;
use lc631_runtime::resource::*;
use lc631_substrate::semantic_term::{self, Term, TermError, TermLimits};
use std::time::Duration;
fn limits() -> TermLimits {
    TermLimits {
        max_bytes: 4096,
        max_nodes: 512,
        max_depth: 32,
    }
}
fn bounded(body: impl FnOnce()) {
    let cap = ResourceLimits::uniform(16 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let source = SourceRevision::of("Given K6 cost audit");
    let run = broker
        .begin_run(
            22,
            source.digest(),
            cap,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let req = |kind, n| ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap());
    let reservation = run
        .reserve(&[
            req(ResourceKind::Ram, 8 * 1024 * 1024),
            req(ResourceKind::Output, 1024 * 1024),
        ])
        .unwrap();
    let view = reservation.lease();
    let held = view
        .claim_resources(&[
            req(ResourceKind::Ram, 1024 * 1024),
            req(ResourceKind::Output, 4096),
        ])
        .unwrap();
    body();
    held.check().unwrap();
}
#[test]
fn tlcx22_creative_audit_k6_handgold_bundle_and_aggregate_limits() {
    bounded(|| {
        let semantic = Term::Bool(true);
        let decoder = Term::Bool(false);
        let dictionary = Term::Bool(false);
        // Apply+operatorSymbol("bundle-v1")+arity=13 bytes; three Bool terms=6.
        let bits = semantic_term::bundle_bits(&semantic, &decoder, &dictionary, limits()).unwrap();
        assert_eq!(bits, 152);
        let full = Term::Apply(
            "bundle-v1".into(),
            vec![semantic.clone(), decoder.clone(), dictionary.clone()],
        );
        let bytes = semantic_term::encode(&full, limits()).unwrap();
        assert_eq!(bytes.len(), 19);
        assert_eq!(semantic_term::decode(&bytes, limits()).unwrap(), full);
        // Each component separately fits; the aggregate has5nodes anddepth2.
        assert!(semantic_term::encoded_len(
            &semantic,
            TermLimits {
                max_nodes: 4,
                ..limits()
            }
        )
        .is_ok());
        assert_eq!(
            semantic_term::bundle_bits(
                &semantic,
                &decoder,
                &dictionary,
                TermLimits {
                    max_nodes: 4,
                    ..limits()
                }
            ),
            Err(TermError::Budget)
        );
        assert_eq!(
            semantic_term::bundle_bits(
                &semantic,
                &decoder,
                &dictionary,
                TermLimits {
                    max_depth: 1,
                    ..limits()
                }
            ),
            Err(TermError::Budget)
        );
    });
}
#[test]
fn tlcx22_creative_audit_shorter_payload_can_have_negative_total_bundle_gain() {
    bounded(|| {
        let baseline = Term::Symbol("abcdefghij".into());
        let candidate = Term::Bool(true);
        let empty = Term::Bool(false);
        let decoder = Term::Symbol("decode-v1".into());
        let dictionary = Term::Symbol("ABCDEFGHIJ".repeat(4));
        let baseline_payload = 8 * semantic_term::encoded_len(&baseline, limits()).unwrap() as u64;
        let candidate_payload =
            8 * semantic_term::encoded_len(&candidate, limits()).unwrap() as u64;
        assert_eq!((baseline_payload, candidate_payload), (96, 16));
        let baseline_total =
            semantic_term::bundle_bits(&baseline, &empty, &empty, limits()).unwrap();
        let candidate_total =
            semantic_term::bundle_bits(&candidate, &decoder, &dictionary, limits()).unwrap();
        assert_eq!((baseline_total, candidate_total), (232, 544));
        // Conditional arithmetic only: no claim that the declared decoder preserves
        // information. A future owned producer must check that prerequisite itself.
        assert_eq!(
            compression_gain(
                baseline_payload,
                candidate_payload,
                PreservationPrerequisite::ReportedChecked
            )
            .unwrap(),
            RationalInterval::exact_ratio(5, 6).unwrap()
        );
        assert_eq!(
            compression_gain(
                baseline_total,
                candidate_total,
                PreservationPrerequisite::ReportedChecked
            )
            .unwrap(),
            RationalInterval::exact_ratio(-39, 68).unwrap()
        );
    });
}
#[test]
fn tlcx22_creative_audit_missing_preservation_or_costs_never_becomes_zero_gain() {
    bounded(|| {
        for (baseline, candidate) in [(232, 16), (0, 0)] {
            assert_eq!(
                compression_gain(baseline, candidate, PreservationPrerequisite::Pending),
                Err(MeasurementError::PrerequisitePending)
            );
            assert_eq!(
                compression_gain(baseline, candidate, PreservationPrerequisite::Failed),
                Err(MeasurementError::PrerequisiteFailed)
            );
        }
        assert_eq!(
            compression_gain(0, 16, PreservationPrerequisite::ReportedChecked),
            Err(MeasurementError::InvalidInput)
        );
        assert_eq!(
            compression_gain(232, 0, PreservationPrerequisite::ReportedChecked),
            Err(MeasurementError::InvalidInput)
        );
    });
}
