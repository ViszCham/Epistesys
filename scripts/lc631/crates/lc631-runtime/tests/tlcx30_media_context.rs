//! Acceptance requirements written before the new connection implementation.
mod tlcx30_media_support;
use lc631_core::context::{ContextAdmission, PremiseBasis};
use lc631_receipt_kernel::ReplayGuard;
use lc631_runtime::{
    observation_bridge::media::*,
    resource::{Cancellation, ResourceAmount, ResourceKind, ResourceRequest},
};
use tlcx30_media_support::*;

#[test]
fn tlcx30_actual_funded_media_is_consumed_as_nonempty_proposed_context() {
    let fixture = Fixture::new(1000, false);
    let mut contexts = consumer();
    fixture.funded(Cancellation::default(), |view| {
        let owner = fixture.execute(view, &mut contexts).unwrap();
        let ContextAdmission::Inserted(revision) = owner.admission() else {
            panic!("missing branch")
        };
        let branch = contexts.get(revision).unwrap();
        assert_eq!(owner.report().graph.observations().len(), 2);
        assert_eq!(branch.premises().len(), 2);
        assert!(branch
            .premises()
            .iter()
            .all(|p| p.basis() == PremiseBasis::Proposed && p.source_span().is_none()));
        assert!(contexts.original().state().premises().is_empty());
        assert_eq!(owner.report().family_validation_count, 0);
        assert!(owner.logical_payload_bytes() > 0);
        owner.check_live().unwrap();
        let full_ram = [ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(32 * 1024 * 1024).unwrap(),
        )];
        assert!(view.claim_resources(&full_ram).is_err());
        drop(owner);
        let released = view.claim_resources(&full_ram).unwrap();
        drop(released);
    });
    assert!(fixture.root.join("backend.ran").is_file());
    assert!(!fixture.root.join("__pycache__").exists());
}

#[test]
fn tlcx30_empty_actual_output_and_configuration_swap_never_publish_context() {
    for empty in [true, false] {
        let fixture = Fixture::new(1000, empty);
        let mut contexts = consumer();
        let mut registration = fixture.registration.clone();
        if !empty {
            registration.configuration = lc631_core::stable_sha256("foreign configuration");
        }
        fixture.funded(Cancellation::default(), |view| {
            let result = observe_registered_media(
                MediaExecutionRequest {
                    binding: fixture.binding(&contexts),
                    registration: &registration,
                    source_authority: &fixture.authority,
                    descriptor: &fixture.descriptor,
                    config: &fixture.config,
                    budget: &fixture.budget,
                    limits: MediaAdmissionLimits {
                        max_observations: 8,
                        max_label_bytes: 4096,
                    },
                },
                fixture.context.issuer(),
                fixture.context.verifier(),
                &mut ReplayGuard::default(),
                epoch(),
                view,
                &mut contexts,
            );
            assert!(result.is_err());
        });
        assert_eq!(contexts.states().len(), 1);
        assert_eq!(fixture.root.join("backend.ran").exists(), empty,
            "only an actually executed empty-output backend may leave a marker; registration mismatch is pre-spawn");
    }
}

#[test]
fn tlcx30_cancelled_source_swap_and_foreign_task_do_not_admit() {
    for case in 0..3 {
        let fixture = Fixture::new(1000, false);
        let mut contexts = consumer();
        let mut binding = fixture.binding(&contexts);
        let cancel = Cancellation::default();
        if case == 0 {
            cancel.cancel();
        }
        if case == 1 {
            std::fs::write(&fixture.source, b"change").unwrap();
        }
        if case == 2 {
            binding.task_source = d(90);
        }
        fixture.funded(cancel, |view| {
            let result = observe_registered_media(
                MediaExecutionRequest {
                    binding,
                    registration: &fixture.registration,
                    source_authority: &fixture.authority,
                    descriptor: &fixture.descriptor,
                    config: &fixture.config,
                    budget: &fixture.budget,
                    limits: MediaAdmissionLimits {
                        max_observations: 8,
                        max_label_bytes: 4096,
                    },
                },
                fixture.context.issuer(),
                fixture.context.verifier(),
                &mut ReplayGuard::default(),
                epoch(),
                view,
                &mut contexts,
            );
            assert!(result.is_err());
        });
        assert_eq!(contexts.states().len(), 1);
        assert!(!fixture.root.join("backend.ran").exists());
    }
}

#[test]
fn tlcx30_expired_parent_model_swap_and_label_budget_do_not_publish() {
    for case in 0..3 {
        let fixture = Fixture::new(1000, false);
        let mut contexts = consumer();
        let mut registration = fixture.registration.clone();
        if case == 1 {
            registration.model = lc631_core::stable_sha256("foreign model");
        }
        let now = if case == 0 { epoch() + 301 } else { epoch() };
        fixture.funded(Cancellation::default(), |view| {
            let result = observe_registered_media(
                MediaExecutionRequest {
                    binding: fixture.binding(&contexts),
                    registration: &registration,
                    source_authority: &fixture.authority,
                    descriptor: &fixture.descriptor,
                    config: &fixture.config,
                    budget: &fixture.budget,
                    limits: MediaAdmissionLimits {
                        max_observations: 8,
                        max_label_bytes: if case == 2 { 1 } else { 4096 },
                    },
                },
                fixture.context.issuer(),
                fixture.context.verifier(),
                &mut ReplayGuard::default(),
                now,
                view,
                &mut contexts,
            );
            assert!(result.is_err());
        });
        assert_eq!(contexts.states().len(), 1);
        assert_eq!(fixture.root.join("backend.ran").exists(), case == 2);
    }
}
