//! Registered clock controls. No actor report constructs a current endpoint.
mod tlcx31_registered_clock_support;
use lc631_receipt_kernel::ReceiptIssuer;
use lc631_runtime::goal_clock_domain as clock;
use std::sync::{atomic::Ordering, Arc};
use tlcx31_registered_clock_support::*;

#[derive(Clone, Copy)]
enum Delivery { Omit, Repeat, Swallow }
struct AlteredRegistry { registry: Arc<Registry>, mode: Delivery }
impl clock::RegistryAuthority for AlteredRegistry {
    fn with_current(&self, use_view: &mut dyn FnMut(clock::RegistryView<'_>) -> Result<(), clock::Error>)
        -> Result<(), clock::Error>
    {
        match self.mode {
            Delivery::Omit => {}
            Delivery::Repeat => {
                let _ = clock::RegistryAuthority::with_current(self.registry.as_ref(), use_view);
                let _ = clock::RegistryAuthority::with_current(self.registry.as_ref(), use_view);
            }
            Delivery::Swallow => { let _ = clock::RegistryAuthority::with_current(self.registry.as_ref(), use_view); }
        }
        Ok(())
    }
}

#[test]
fn registry_must_deliver_exactly_one_valid_current_view() {
    for mode in [Delivery::Omit, Delivery::Repeat, Delivery::Swallow] {
        resources(|view, _| {
            let registry = Registry::new();
            let profile = profile(&registry, view);
            let raw = Clock::new();
            let candidate = clock::EndpointCandidate::execution(
                &profile, described(view, clock::Role::Baseline, clock::Purpose::Execution),
                raw.clone(), view,
            ).unwrap();
            let issuer = match mode {
                Delivery::Swallow => ReceiptIssuer::from_key_bytes("foreign-local-clock", &[0xf1; 32]).unwrap(),
                _ => Registry::issuer(),
            };
            let signed = receipt(&candidate, &issuer);
            let error = clock::register(
                &profile, candidate, signed, Arc::new(AlteredRegistry { registry, mode }), view,
            ).err().expect("no valid registry delivery");
            let expected = match mode {
                Delivery::Swallow => clock::ClockGap::UntrustedIssuer,
                _ => clock::ClockGap::CurrentRegistryUnavailable,
            };
            assert!(matches!(error, clock::Error::Clock(clock::ClockFault::NeedsEvidence(gap)) if gap == expected));
            assert_eq!(profile.work_current(view).unwrap().registration_attempts, 1);
            assert_eq!(raw.calls(), 0);
        });
    }
}

#[test]
fn observed_expiry_and_revocation_cannot_be_erased_by_a_later_registry_view() {
    for expiry in [false, true] {
        resources(|view, _| {
            let registry = Registry::new();
            let p = profile(&registry, view);
            let raw = Clock::new();
            let candidate = clock::EndpointCandidate::execution(
                &p, described(view, clock::Role::Baseline, clock::Purpose::Execution),
                raw.clone(), view,
            ).unwrap();
            let signed = receipt(&candidate, &Registry::issuer());
            let digest = signed.receipt_digest();
            let endpoint = clock::register(&p, candidate, signed, registry.clone(), view).unwrap();
            endpoint.check_registration_current(view).unwrap();
            if expiry { registry.now.store(400, Ordering::SeqCst); } else { registry.revoke(digest); }
            let expected = if expiry { clock::ClockFault::RegistrationExpired } else { clock::ClockFault::RegistrationRevoked };
            assert!(matches!(endpoint.check_registration_current(view), Err(clock::Error::Clock(ref f)) if f == &expected));
            registry.now.store(100, Ordering::SeqCst);
            registry.revoked.lock().unwrap().clear();
            registry.revision.fetch_add(1, Ordering::SeqCst);
            assert!(matches!(endpoint.check_registration_current(view), Err(clock::Error::Clock(ref f)) if f == &expected));
            assert_eq!(raw.calls(), 0);
        });
    }
}

#[test]
fn higher_baseline_epoch_does_not_create_a_candidate_rollback() {
    full(|_, root, pair, ports, clocks, _, _, _, view| {
        clocks.all(150);
        clocks.be.set(200);
        clocks.bc.set(200);
        let measured = clock::measure_clock_bound(root, pair, ports, view).unwrap();
        measured.with_current(root, pair, ports, view, |value| {
            assert_eq!(value.baseline_outputs()[2], Some(0));
            assert_eq!(value.candidate_outputs()[2], None);
            assert_eq!(value.clock_basis(), clock::ClockBasis::GivenLocalRegistered);
            assert_eq!(value.requested_source_goal(), None);
        }).unwrap();
    });
}
