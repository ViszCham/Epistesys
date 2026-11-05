//! Extra exact-input boundaries; no production success, profile or owner stub.
mod tlcx31_registered_clock_support;
use lc631_runtime::goal_clock_domain as c;
use lc631_substrate::resource::ResourceError;
use tlcx31_registered_clock_support::*;

#[test]
fn a01_foreign_root_source_or_probe_plan_rejects_before_provider() {
    for cut in 0..3 {
        with_binding(|binding, plan, grant, view| {
            let registry = Registry::new();
            let p = profile(&registry, view);
            let raw = Clock::new();
            let exec = c::EndpointCandidate::execution(
                &p,
                role_binding(binding, plan, c::Role::Baseline, c::Purpose::Execution),
                raw.clone(),
                view,
            )
            .unwrap();
            let mut request = role_binding(binding, plan, c::Role::Baseline, c::Purpose::Capture);
            match cut {
                0 => request.root = d(99),
                1 => request.source = d(99),
                _ => request.probe_plan = d(99),
            }
            let rejected =
                match c::EndpointCandidate::capture(&p, request, raw.clone(), grant, view) {
                    Err(e) => e,
                    Ok(cap) => {
                        let er = receipt(&exec, &Registry::issuer());
                        let cr = receipt(&cap, &Registry::issuer());
                        let exec = c::register(&p, exec, er, registry.clone(), view).unwrap();
                        let cap = c::register(&p, cap, cr, registry, view).unwrap();
                        c::RoleSession::bind(&p, &exec, &cap, binding, plan, view)
                            .err()
                            .expect("foreign scope cannot bind")
                    }
                };
            assert!(matches!(
                rejected,
                c::Error::Clock(c::ClockFault::BindingMismatch)
            ));
            assert_eq!(raw.calls(), 0);
        });
    }
}

#[test]
fn a02_oversize_limits_or_metadata_fail_before_registry_and_provider() {
    resources(|view, _| {
        let registry = Registry::new();
        let huge = "x".repeat(4097);
        for cut in 0..5 {
            let mut l = limits();
            match cut {
                0 => l.max_samples = 1025,
                1 => l.max_roles = 3,
                2 => l.max_added_ram_bytes = 65537,
                3 => l.max_metadata_bytes = 4097,
                _ => {}
            }
            let rejected = c::Profile::given_local(
                c::ProfileInput {
                    domain: d(90),
                    protocol: c::Protocol::EpochSecondsV1,
                    trusted_registry_fingerprint: if cut == 4 {
                        &huge
                    } else {
                        registry.verifier.key_fingerprint()
                    },
                    requirement: c::TrustRequirement::GivenLocalRegistered,
                    limits: l,
                },
                view,
            )
            .err()
            .expect("no truncation or unlimited budget");
            assert!(matches!(rejected, c::Error::InputLimit));
        }
        assert_eq!(registry.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    });
}

#[test]
fn a03_equal_source_foreign_live_lease_is_not_same_capability() {
    resources(|view, _| {
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
        resources(|foreign, _| {
            assert_eq!(view.source_revision(), foreign.source_revision());
            assert_ne!(view.owner_id(), foreign.owner_id());
            assert!(matches!(
                endpoint.check_registration_current(foreign),
                Err(c::Error::Clock(c::ClockFault::Resource(
                    ResourceError::ForeignOwner
                )))
            ));
        });
        assert_eq!(raw.calls(), 0);
    });
}
