//! 登録済み構成との照合は、ローカル子 process と receipt 消費に先行する。
//! Compare the declared configuration before the local child or backend replay.
#![forbid(unsafe_code)]

mod tlcx30_media_support;

use lc631_core::context::{ContextAdmission, ContextConsumer, PremiseBasis};
use lc631_receipt_kernel::ReplayGuard;
use lc631_runtime::observation_bridge::media::{
    observe_registered_media, ExecutedMediaAdmission, MediaAdmissionLimits, MediaConnectionError,
    MediaExecutionRequest, MediaRegistration,
};
use lc631_runtime::resource::{Cancellation, ResourceReservationView};
use tlcx30_media_support::{consumer, epoch, Fixture};

fn observe<'view>(
    fixture: &Fixture,
    registration: &MediaRegistration,
    replay: &mut ReplayGuard,
    contexts: &mut ContextConsumer,
    view: &'view ResourceReservationView<'view>,
) -> Result<ExecutedMediaAdmission<'view>, MediaConnectionError> {
    observe_registered_media(
        MediaExecutionRequest {
            binding: fixture.binding(contexts),
            registration,
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
        replay,
        epoch(),
        view,
        contexts,
    )
}

#[test]
fn wrong_registered_configuration_rejects_before_child_replay_and_context_admission() {
    // An independently constructed, live fixture must actually execute its bounded
    // local marker process and admit proposals under a matching registration.
    let valid = Fixture::new(1000, false);
    let mut valid_context = consumer();
    let mut valid_replay = ReplayGuard::default();
    assert!(!valid.root.join("backend.ran").exists());
    valid.funded(Cancellation::default(), |view| {
        let owner = observe(
            &valid,
            &valid.registration,
            &mut valid_replay,
            &mut valid_context,
            view,
        )
        .expect("matching registration must exercise the configured local fixture");
        let ContextAdmission::Inserted(revision) = owner.admission() else {
            panic!("the positive control must publish the new branch");
        };
        let branch = valid_context.get(revision).unwrap();
        assert_eq!(branch.premises().len(), 2);
        assert!(branch
            .premises()
            .iter()
            .all(|p| p.basis() == PremiseBasis::Proposed));
        assert!(valid.root.join("backend.ran").is_file());
        assert_eq!(valid_replay.observed_count(), 1);
    });

    // A fresh root has no prior marker or replay. Change only the declared digest
    // to another syntactically valid value and leave the actual backend intact.
    let foreign = Fixture::new(1000, false);
    let mut registration = foreign.registration.clone();
    registration.configuration = lc631_core::stable_sha256("beta7 foreign media configuration");
    assert_ne!(
        registration.configuration,
        foreign.registration.configuration
    );
    let marker = foreign.root.join("backend.ran");
    assert!(!marker.exists());
    let mut contexts = consumer();
    let mut replay = ReplayGuard::default();
    foreign.funded(Cancellation::default(), |view| {
        let result = observe(&foreign, &registration, &mut replay, &mut contexts, view);
        assert!(matches!(
            result,
            Err(MediaConnectionError::RegistrationMismatch)
        ));
        assert_eq!(
            (
                marker.exists(),
                replay.observed_count(),
                contexts.states().len()
            ),
            (false, 0, 1),
            "registration mismatch dispatched the backend, consumed replay, or admitted a branch"
        );
        assert!(contexts.original().state().premises().is_empty());
    });
}
