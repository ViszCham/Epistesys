//! TEST-FIRST missing registered profile API. No production body or success stub.
#[path = "tlcx31_goal_dynamic_clock_support/mod.rs"]
mod legacy;
mod tlcx31_registered_clock_support;
use lc631_receipt_kernel::ReceiptIssuer;
use lc631_runtime::{
    contextual_repair::measurement::{paired_gain, ProbeError},
    goal_clock_domain as c,
    goal_suite_measurement::SourceEligibility,
};
use lc631_substrate::resource::ResourceError;
use std::sync::atomic::Ordering;
use tlcx31_registered_clock_support::*;

#[test]
fn r01_unknown_domain_protocol_and_host_anchor_are_not_clock_truth() {
    resources(|view, _| {
        let registry = Registry::new();
        for n in 0..3 {
            let input = c::ProfileInput {
                domain: if n == 0 { d(0) } else { d(90) },
                protocol: if n == 1 {
                    c::Protocol::Unknown(d(99))
                } else {
                    c::Protocol::EpochSecondsV1
                },
                trusted_registry_fingerprint: registry.verifier.key_fingerprint(),
                requirement: if n == 2 {
                    c::TrustRequirement::HostAnchored
                } else {
                    c::TrustRequirement::GivenLocalRegistered
                },
                limits: limits(),
            };
            let error = c::Profile::given_local(input, view)
                .err()
                .expect("unknown evidence cannot become current");
            match n {
                0 => assert!(matches!(
                    error,
                    c::Error::Clock(c::ClockFault::NeedsEvidence(c::ClockGap::UnknownDomain))
                )),
                1 => assert!(matches!(
                    error,
                    c::Error::Clock(c::ClockFault::NeedsEvidence(c::ClockGap::UnknownProtocol))
                )),
                _ => assert!(matches!(
                    error,
                    c::Error::Clock(c::ClockFault::NeedsEvidence(
                        c::ClockGap::HostAnchorUnavailable
                    ))
                )),
            }
        }
        assert_eq!(registry.calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn r02_actual_legacy_owner_is_unassessed_not_retagged_or_default_success() {
    legacy::fixture(|h, _, view| {
        let registry = Registry::new();
        let p = profile(&registry, view);
        legacy::clocks::clear_trace();
        let result = c::check_observation_for_registered_profile(h.baseline, h.plan, &p, view);
        assert!(matches!(
            result,
            Err(c::Error::Clock(c::ClockFault::NeedsEvidence(
                c::ClockGap::LegacyClockProvenanceUnassessed
            )))
        ));
        assert!(
            legacy::clocks::trace().is_empty(),
            "missing origin must reject before clock read"
        );
    });
}

#[test]
fn r03_signed_receipt_for_other_actual_instance_cannot_register_same_label() {
    resources(|view, _| {
        let registry = Registry::new();
        let p = profile(&registry, view);
        let clock = Clock::new();
        let a = c::EndpointCandidate::execution(
            &p,
            described(view, c::Role::Baseline, c::Purpose::Execution),
            clock.clone(),
            view,
        )
        .unwrap();
        let b = c::EndpointCandidate::execution(
            &p,
            described(view, c::Role::Baseline, c::Purpose::Execution),
            clock.clone(),
            view,
        )
        .unwrap();
        let signed = receipt(&a, &Registry::issuer());
        let rejected = c::register(&p, b, signed, registry, view)
            .err()
            .expect("instance binding");
        assert!(matches!(
            rejected,
            c::Error::Clock(c::ClockFault::BindingMismatch)
        ));
        assert_eq!(clock.calls(), 0);
    });
}

#[test]
fn r04_expired_revoked_and_wrong_issuer_are_pre_provider_typed_cuts() {
    for mode in 0..3 {
        resources(|view, _| {
            let registry = Registry::new();
            let p = profile(&registry, view);
            let clock = Clock::new();
            let candidate = c::EndpointCandidate::execution(
                &p,
                described(view, c::Role::Baseline, c::Purpose::Execution),
                clock.clone(),
                view,
            )
            .unwrap();
            if mode == 2 {
                let wrong =
                    ReceiptIssuer::from_key_bytes("wrong-clock-registry", &[134; 32]).unwrap();
                let signed = receipt(&candidate, &wrong);
                let rejected = c::register(&p, candidate, signed, registry, view)
                    .err()
                    .expect("untrusted issuer");
                assert!(matches!(
                    rejected,
                    c::Error::Clock(c::ClockFault::NeedsEvidence(c::ClockGap::UntrustedIssuer))
                ));
            } else {
                let signed = receipt(&candidate, &Registry::issuer());
                let id = signed.receipt_digest();
                let endpoint = c::register(&p, candidate, signed, registry.clone(), view).unwrap();
                if mode == 0 {
                    registry.now.store(400, Ordering::SeqCst);
                } else {
                    registry.revoke(id);
                }
                let rejected = endpoint
                    .check_registration_current(view)
                    .err()
                    .expect("credential no longer current");
                if mode == 0 {
                    assert!(matches!(
                        rejected,
                        c::Error::Clock(c::ClockFault::RegistrationExpired)
                    ));
                } else {
                    assert!(matches!(
                        rejected,
                        c::Error::Clock(c::ClockFault::RegistrationRevoked)
                    ));
                }
            }
            assert_eq!(clock.calls(), 0);
        });
    }
}

#[test]
fn r05_cross_domain_and_foreign_role_are_incomparable_not_numeric_rollback() {
    for role_cut in [false, true] {
        with_binding(|binding, plan, grant, view| {
            let registry = Registry::new();
            let p = profile(&registry, view);
            let raw = Clock::new();
            let other = c::Profile::given_local(
                c::ProfileInput {
                    domain: if role_cut { d(90) } else { d(92) },
                    protocol: c::Protocol::EpochSecondsV1,
                    trusted_registry_fingerprint: registry.verifier.key_fingerprint(),
                    requirement: c::TrustRequirement::GivenLocalRegistered,
                    limits: limits(),
                },
                view,
            )
            .unwrap();
            let e = c::EndpointCandidate::execution(
                &p,
                role_binding(binding, plan, c::Role::Baseline, c::Purpose::Execution),
                raw.clone(),
                view,
            )
            .unwrap();
            let cap = c::EndpointCandidate::capture(
                &other,
                role_binding(
                    binding,
                    plan,
                    if role_cut {
                        c::Role::Candidate
                    } else {
                        c::Role::Baseline
                    },
                    c::Purpose::Capture,
                ),
                raw.clone(),
                grant,
                view,
            )
            .unwrap();
            let er = receipt(&e, &Registry::issuer());
            let cr = receipt(&cap, &Registry::issuer());
            let e = c::register(&p, e, er, registry.clone(), view).unwrap();
            let cap = c::register(&other, cap, cr, registry, view).unwrap();
            let rejected = c::RoleSession::bind(&p, &e, &cap, binding, plan, view)
                .err()
                .expect("same labels cannot join incompatible capabilities");
            if role_cut {
                assert!(matches!(
                    rejected,
                    c::Error::Clock(c::ClockFault::BindingMismatch)
                ));
            } else {
                assert!(matches!(
                    rejected,
                    c::Error::Clock(c::ClockFault::NeedsEvidence(
                        c::ClockGap::IncomparableDomains
                    ))
                ));
            }
            assert_eq!(raw.calls(), 0);
        });
    }
}

#[test]
fn r06_registration_failures_spend_attempts_without_provider_or_refund() {
    resources(|view, _| {
        let registry = Registry::new();
        let mut l = limits();
        l.max_registration_attempts = 2;
        let p = c::Profile::given_local(
            c::ProfileInput {
                domain: d(90),
                protocol: c::Protocol::EpochSecondsV1,
                trusted_registry_fingerprint: registry.verifier.key_fingerprint(),
                requirement: c::TrustRequirement::GivenLocalRegistered,
                limits: l,
            },
            view,
        )
        .unwrap();
        let raw = Clock::new();
        let wrong = ReceiptIssuer::from_key_bytes("wrong-clock-registry", &[134; 32]).unwrap();
        for expected in 1..=2 {
            let candidate = c::EndpointCandidate::execution(
                &p,
                described(view, c::Role::Baseline, c::Purpose::Execution),
                raw.clone(),
                view,
            )
            .unwrap();
            let r = receipt(&candidate, &wrong);
            let rejected = c::register(&p, candidate, r, registry.clone(), view)
                .err()
                .expect("failed attempt");
            assert!(matches!(
                rejected,
                c::Error::Clock(c::ClockFault::NeedsEvidence(c::ClockGap::UntrustedIssuer))
            ));
            assert_eq!(
                p.work_current(view).unwrap().registration_attempts,
                expected
            );
        }
        let candidate = c::EndpointCandidate::execution(
            &p,
            described(view, c::Role::Baseline, c::Purpose::Execution),
            raw.clone(),
            view,
        )
        .unwrap();
        let r = receipt(&candidate, &Registry::issuer());
        assert!(matches!(
            c::register(&p, candidate, r, registry, view),
            Err(c::Error::Clock(c::ClockFault::BudgetExhausted))
        ));
        assert_eq!(p.work_current(view).unwrap().registration_attempts, 2);
        assert_eq!(raw.calls(), 0);
    });
}

#[test]
fn r07_cancelled_lease_rejects_before_clock_provider() {
    resources(|view, cancel| {
        let registry = Registry::new();
        let p = profile(&registry, view);
        let raw = Clock::new();
        let candidate = c::EndpointCandidate::execution(
            &p,
            described(view, c::Role::Baseline, c::Purpose::Execution),
            raw.clone(),
            view,
        )
        .unwrap();
        let r = receipt(&candidate, &Registry::issuer());
        let endpoint = c::register(&p, candidate, r, registry, view).unwrap();
        cancel.cancel();
        assert!(matches!(
            endpoint.check_registration_current(view),
            Err(c::Error::Clock(c::ClockFault::Resource(
                ResourceError::Cancelled
            )))
        ));
        assert_eq!(raw.calls(), 0);
    });
}

#[test]
fn r08_registered_actual_common_domain_rejects_capture100_after_execution200() {
    full(|_, root, pair, ports, clocks, _, _, _, view| {
        clocks.all(200);
        clocks.bc.set(100);
        clocks.cc.set(100);
        let error = c::measure_clock_bound(root, pair, ports, view)
            .err()
            .expect("true registered rollback");
        assert!(matches!(
            error,
            c::Error::Clock(c::ClockFault::ClockRollback)
        ));
        assert!(
            clocks.bc.calls() > 2,
            "actual post-execution capture provider must be read"
        );
    });
}

#[test]
fn r09_actual_partial_capture_keeps_before_stamp_and_original_error() {
    full(|_, _, _, ports, clocks, _, bsession, _, view| {
        clocks.bc.sequence(&[200, 150], 150);
        let attempt = bsession.capture_current(ports.baseline_build, view);
        assert!(matches!(
            attempt.current(),
            Err(c::Error::Snapshot(
                lc631_analysis::declared_build_capture::SnapshotCaptureError::ClockRollback
            ))
        ));
        assert!(matches!(
            attempt.clock_fault(),
            Some(c::ClockFault::ClockRollback)
        ));
        let actual = attempt
            .actual_capture()
            .expect("real inventory retained after post-cut");
        assert_eq!(actual.authority_before().checked_epoch, 200);
        assert_eq!(actual.authority_after(), None);
    });
}

#[test]
fn r10_failed_max_and_existing_gain_share_same_observed_arc() {
    full(|_, root, pair, _, clocks, _, _, _, view| {
        clocks.all(200);
        let gain = paired_gain(
            root.plan(),
            pair.baseline.probe_observation(),
            pair.candidate.probe_observation(),
            view,
        )
        .unwrap();
        clocks.be.set(400);
        assert!(matches!(
            pair.baseline.check_current(view),
            Err(c::Error::Clock(c::ClockFault::ObservedExpiry))
        ));
        clocks.be.set(150);
        let owner = pair.baseline.check_current(view);
        let shared = gain.check_current(root.plan(), view);
        assert!(matches!(
            owner,
            Err(c::Error::Clock(c::ClockFault::ClockRollback))
        ));
        assert!(matches!(
            shared,
            Err(ProbeError::ClockDomain(c::ClockFault::ClockRollback))
        ));
        assert_eq!(pair.baseline.checked_epoch(), 100);
    });
}

#[test]
fn r11_provider_unavailable_is_needs_evidence_and_read_attempt_is_not_refunded() {
    full(|p, _, pair, _, clocks, _, _, _, view| {
        let before = p.work_current(view).unwrap().endpoint_reads;
        let calls = clocks.be.calls();
        clocks.be.unavailable.store(true, Ordering::SeqCst);
        assert!(matches!(
            pair.baseline.check_current(view),
            Err(c::Error::Clock(c::ClockFault::NeedsEvidence(
                c::ClockGap::ProviderUnavailable
            )))
        ));
        assert_eq!(p.work_current(view).unwrap().endpoint_reads, before + 1);
        assert_eq!(clocks.be.calls(), calls + 1);
    });
}

#[test]
fn p01_registered_actual_positive_has_no_host_or_numeric_promotion() {
    full(|_, root, pair, ports, clocks, _, _, _, view| {
        clocks.all(200);
        let owner = c::measure_clock_bound(root, pair, ports, view).unwrap();
        owner
            .with_current(root, pair, ports, view, |v| {
                assert_eq!(
                    v.baseline_outputs(),
                    &[Some(0), Some(3), Some(0), Some(4294967295)]
                );
                assert_eq!(
                    v.candidate_outputs(),
                    &[Some(0), Some(3), None, Some(4294967295)]
                );
                assert_eq!(v.clock_basis(), c::ClockBasis::GivenLocalRegistered);
                assert_eq!(v.host_authenticity(), c::HostAuthenticity::Unavailable);
                assert_eq!(v.requested_source_goal(), None);
                assert_eq!(v.source_eligibility(), SourceEligibility::Pending);
            })
            .unwrap();
        assert_eq!(
            (
                pair.baseline.checked_epoch(),
                pair.candidate.checked_epoch()
            ),
            (100, 100)
        );
    });
}

#[test]
fn r12_actual_capture_advances_the_existing_owner_and_gain_arc() {
    full(|_, root, pair, ports, clocks, _, bsession, _, view| {
        let gain = paired_gain(
            root.plan(),
            pair.baseline.probe_observation(),
            pair.candidate.probe_observation(),
            view,
        )
        .unwrap();
        clocks.bc.set(200);
        let attempt = bsession.capture_current(ports.baseline_build, view);
        attempt.current().unwrap();
        let actual = attempt.actual_capture().unwrap();
        assert_eq!(actual.authority_before().checked_epoch, 200);
        assert_eq!(actual.authority_after().unwrap().checked_epoch, 200);
        clocks.be.set(150);
        let owner = pair.baseline.check_current(view);
        let shared = gain.check_current(root.plan(), view);
        assert!(matches!(
            owner,
            Err(c::Error::Clock(c::ClockFault::ClockRollback))
        ));
        assert!(matches!(
            shared,
            Err(ProbeError::ClockDomain(c::ClockFault::ClockRollback))
        ));
        assert_eq!(pair.baseline.checked_epoch(), 100);
    });
}
