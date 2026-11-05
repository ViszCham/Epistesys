//! TEST-FIRST: missing planned API RED, not a behavioral RED of old code.
//! CR-BREAK is absolute optional legacy breakage, never 1 - A_compat.
mod tlcx22_breakcost_support;
use lc631_analysis::{breakcost_measurement::*, creativity_measurement::*, *};
use lc631_substrate::resource::*;
use std::time::{Duration, Instant};
use tlcx22_breakcost_support::*;

macro_rules! fixture {
    ($name:ident, $baseline:expr, $body:block) => {
        let source = source();
        let raw = probe();
        let weight = q(1, 1);
        let cases = [case(1, b"legacy-case", &raw, &weight)];
        let rules = rules();
        let (_issuer, live) = lease(Cancellation::default());
        let view = live.view();
        let root = register(
            &source,
            binding(),
            &cases,
            &rules,
            $baseline,
            budget(),
            &view,
        )
        .unwrap();
        let base = observe_baseline_current(&root, &view).unwrap();
        let req = requests(AdapterKind::LegacyProjectionV1);
        let frozen = bind_candidate(&root, candidate(1), &req, &view).unwrap();
        let $name = (&source, &raw, &cases, &root, &base, &frozen, &view);
        $body
    };
}

#[test]
fn p01_real_v1_output_has_zero_absolute_break_cost() {
    fixture!(f, AdapterKind::LegacyProjectionV1, {
        let (source, raw, cases, root, base, frozen, view) = f;
        let owner = observe_breakcost_current(root, base, frozen, view).unwrap();
        let report = owner
            .report_current(inputs(source, root, cases, base, frozen), view)
            .unwrap();
        assert_cost(report, 0, 1);
        hand_v1(&owner.historical_outputs()[0], raw);
        let old: LegacyDgclConsumerView =
            serde_json::from_slice(&owner.historical_outputs()[0]).unwrap();
        assert!(!old.host_send_authorized && !old.output_commit_allowed);
        let control = project_dgcl_consumer_view(raw, DgclConsumerVersion::LegacyV1, V1).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&owner.historical_outputs()[0]).unwrap(),
            control
        );
    });
}
#[test]
fn p02_real_v2_breaks_only_optional_old_receiver_three_quarters() {
    fixture!(f, AdapterKind::LegacyProjectionV1, {
        let (source, _, cases, root, base, _, view) = f;
        let req = requests(AdapterKind::BoundedProjectionV2);
        let frozen = bind_candidate(root, candidate(2), &req, view).unwrap();
        let owner = observe_breakcost_current(root, base, &frozen, view).unwrap();
        assert_cost(
            owner
                .report_current(inputs(source, root, cases, base, &frozen), view)
                .unwrap(),
            3,
            4,
        );
        assert_eq!(owner.historical_hard_status(), LocalHardStatus::NotRejected);
        let output = &owner.historical_outputs()[0];
        let value: serde_json::Value = serde_json::from_slice(output).unwrap();
        assert!(value.is_object());
        assert_eq!(value["schema_version"], V2);
        for key in [
            "host_send_authorized",
            "output_commit_allowed",
            "authority_created",
        ] {
            assert_eq!(value[key], false);
        }
        assert!(serde_json::from_slice::<LegacyDgclConsumerView>(output).is_err());
        assert_eq!(owner.historical_denominator(), &q(4, 1));
    });
}
#[test]
fn p03_already_broken_baseline_never_exempts_candidate_or_becomes_gain() {
    fixture!(f, AdapterKind::BoundedProjectionV2, {
        let (_, _, _, root, base, good, view) = f;
        assert_cost(base.historical_report(), 3, 4);
        let req = requests(AdapterKind::BoundedProjectionV2);
        let broken = bind_candidate(root, candidate(2), &req, view).unwrap();
        let b = observe_breakcost_current(root, base, &broken, view).unwrap();
        let g = observe_breakcost_current(root, base, good, view).unwrap();
        assert_cost(b.historical_report(), 3, 4);
        assert_cost(g.historical_report(), 0, 1);
    });
}
#[test]
fn p04_two_case_hand_weight_oracle_is_order_and_opaque_id_invariant() {
    let source = source();
    let raw = probe();
    let raw_b = probe_with(b"{\"schema_version\":\"local.v1\",\"case\":2}");
    let one = q(1, 1);
    let two = q(2, 1);
    let rules = rules();
    let (_issuer, live) = lease(Cancellation::default());
    let view = live.view();
    for relabel in [false, true] {
        let cases = if relabel {
            vec![case(92, b"b", &raw_b, &two), case(91, b"a", &raw, &one)]
        } else {
            vec![case(1, b"a", &raw, &one), case(2, b"b", &raw_b, &two)]
        };
        let root = register(
            &source,
            binding(),
            &cases,
            &rules,
            AdapterKind::LegacyProjectionV1,
            budget(),
            &view,
        )
        .unwrap();
        let base = observe_baseline_current(&root, &view).unwrap();
        let req = [
            AdapterRequest {
                case: if relabel { 91 } else { 1 },
                kind: AdapterKind::LegacyProjectionV1,
                requested_schema: V1,
            },
            AdapterRequest {
                case: if relabel { 92 } else { 2 },
                kind: AdapterKind::BoundedProjectionV2,
                requested_schema: V2,
            },
        ];
        let frozen = bind_candidate(&root, candidate(1), &req, &view).unwrap();
        let owner = observe_breakcost_current(&root, &base, &frozen, &view).unwrap();
        // W=(1+2)*(1+3)=12; only case b fails weight 2*3=6; 6/12=1/2.
        assert_eq!(owner.historical_denominator(), &q(12, 1));
        assert_cost(owner.historical_report(), 1, 2);
    }
}
#[test]
fn n01_actual_owner_types_do_not_clone_or_deserialize_reports() {
    // Ambiguity is a compile-time error if a forbidden trait is implemented.
    macro_rules! not_impl {
        ($ty:ty: $bound:path) => {{
            trait Ambiguous<A> {
                fn check() {}
            }
            impl<T: ?Sized> Ambiguous<()> for T {}
            struct IfImpl;
            impl<T: ?Sized + $bound> Ambiguous<IfImpl> for T {}
            let _ = <$ty as Ambiguous<_>>::check;
        }};
    }
    not_impl!(RootDefinition<'static, 'static>: Clone);
    not_impl!(AdapterCandidate<'static, 'static>: Clone);
    not_impl!(OwnedLegacyBaseline<'static, 'static>: Clone);
    not_impl!(OwnedBreakCost<'static, 'static>: Clone);
    not_impl!(OwnedBreakCost<'static, 'static>: serde::de::DeserializeOwned);
    // Missing-owner/copied-report runtime rejection is exercised by S04.
}
#[test]
fn n02_current_rejects_expected_input_root_mode_source_context_substitution() {
    fixture!(f, AdapterKind::LegacyProjectionV1, {
        let (source, raw, cases, root, base, frozen, view) = f;
        let owner = observe_breakcost_current(root, base, frozen, view).unwrap();
        let weight = q(1, 1);
        let copied_cases = [case(1, b"legacy-case", raw, &weight)];
        let rules = rules();
        let mut b = binding();
        b.evidence_revision = d(201);
        let other = register(
            source,
            b,
            cases,
            &rules,
            AdapterKind::LegacyProjectionV1,
            budget(),
            view,
        )
        .unwrap();
        let other_source = tlcx22_breakcost_support::source();
        let req = requests(AdapterKind::BoundedProjectionV2);
        let other_candidate = bind_candidate(root, candidate(1), &req, view).unwrap();
        for expected in [
            inputs(source, &other, cases, base, frozen),
            inputs(source, root, &copied_cases, base, frozen),
            inputs(&other_source, root, cases, base, frozen),
            inputs(source, root, cases, base, &other_candidate),
        ] {
            assert!(matches!(
                owner.report_current(expected, view),
                Err(Error::BindingMismatch)
            ));
        }
        let mut wrong = binding();
        wrong.context = lc631_core::context::ContextRevision::original(
            lc631_core::context::SourceRevision::of("other"),
        );
        assert!(matches!(
            register(
                source,
                wrong,
                cases,
                &rules,
                AdapterKind::LegacyProjectionV1,
                budget(),
                view
            ),
            Err(Error::BindingMismatch)
        ));
        for cut in 0..5 {
            let mut changed = binding();
            let mut changed_rules = rules.clone();
            let mut cap = budget();
            match cut {
                0 => changed.goal_profile = Some(d(210)),
                1 => changed.policy = d(211),
                2 => changed_rules[1].weight = q(4, 1),
                3 => {
                    changed_rules.pop();
                }
                4 => cap.max_soft_checks_per_run = 2,
                _ => unreachable!(),
            }
            let changed_root = register(
                source,
                changed,
                cases,
                &changed_rules,
                AdapterKind::LegacyProjectionV1,
                cap,
                view,
            )
            .unwrap();
            assert!(matches!(
                owner.report_current(inputs(source, &changed_root, cases, base, frozen), view),
                Err(Error::BindingMismatch)
            ));
        }
    });
}
#[test]
fn n03_nontradeable_hard_cannot_be_soft_and_elected_hard_root_is_fixed() {
    fixture!(f, AdapterKind::LegacyProjectionV1, {
        let (source, _, cases, root, base, frozen, view) = f;
        for property in [
            LegacyProperty::SourceIdentity,
            LegacyProperty::CandidateIdentity,
            LegacyProperty::NoHostSend,
            LegacyProperty::NoOutputCommit,
            LegacyProperty::NoLegacyCompletion,
            LegacyProperty::NoAuthorityCreation,
            LegacyProperty::NegotiatedSchema,
        ] {
            let mut tampered = rules();
            tampered.push(PropertyRule {
                property,
                class: PropertyClass::Optional,
                weight: q(1, 1),
            });
            assert!(matches!(
                register(
                    source,
                    binding(),
                    cases,
                    &tampered,
                    AdapterKind::LegacyProjectionV1,
                    budget(),
                    view
                ),
                Err(Error::NonTradeableHard)
            ));
        }
        let mut hard = rules();
        hard[1].class = PropertyClass::Hard;
        let elected = register(
            source,
            binding(),
            cases,
            &hard,
            AdapterKind::LegacyProjectionV1,
            budget(),
            view,
        )
        .unwrap();
        let eb = observe_baseline_current(&elected, view).unwrap();
        let req = requests(AdapterKind::BoundedProjectionV2);
        let ef = bind_candidate(&elected, candidate(2), &req, view).unwrap();
        let eo = observe_breakcost_current(&elected, &eb, &ef, view).unwrap();
        assert_no_scalar(eo.historical_report());
        assert!(matches!(
            eo.report_current(inputs(source, root, cases, base, frozen), view),
            Err(Error::BindingMismatch)
        ));
    });
}
#[test]
fn n04_candidate_cannot_omit_duplicate_or_redefine_fixed_suite() {
    fixture!(f, AdapterKind::LegacyProjectionV1, {
        let (_, _, _, root, _, _, view) = f;
        assert!(matches!(
            bind_candidate(root, candidate(1), &[], view),
            Err(Error::InvalidSuite)
        ));
        let mut req = requests(AdapterKind::LegacyProjectionV1);
        req.push(req[0]);
        assert!(matches!(
            bind_candidate(root, candidate(1), &req, view),
            Err(Error::InvalidSuite)
        ));
        req.truncate(1);
        req[0].case = 404;
        assert!(matches!(
            bind_candidate(root, candidate(1), &req, view),
            Err(Error::InvalidSuite)
        ));
        // Requests expose only case/mode/schema: no per-candidate mask/weight/goal fields.
    });
}
#[test]
fn n05_true_legacy_completion_send_or_commit_is_hard_failed_without_cost() {
    for flag in 0..3 {
        let source = source();
        let mut raw = probe();
        match flag {
            0 => raw.implementation_complete_candidate = true,
            1 => raw.host_send_authorized = true,
            2 => raw.output_commit_allowed = true,
            _ => unreachable!(),
        }
        let one = q(1, 1);
        let cases = [case(1, b"legacy-case", &raw, &one)];
        let rules = rules();
        let (_issuer, live) = lease(Cancellation::default());
        let view = live.view();
        let root = register(
            &source,
            binding(),
            &cases,
            &rules,
            AdapterKind::LegacyProjectionV1,
            budget(),
            &view,
        )
        .unwrap();
        let base = observe_baseline_current(&root, &view).unwrap();
        let req = requests(AdapterKind::DirectLegacyDtoSerializeV1);
        let frozen = bind_candidate(&root, candidate(1), &req, &view).unwrap();
        let owner = observe_breakcost_current(&root, &base, &frozen, &view).unwrap();
        assert_eq!(owner.historical_hard_status(), LocalHardStatus::Failed);
        assert_no_scalar(owner.historical_report());
        assert_eq!(owner.historical_counters().soft_checks, 0);
    }
}
#[test]
fn n06_real_old_decoder_rejection_is_confirmed_optional_not_false_pass_or_unknown() {
    fixture!(f, AdapterKind::LegacyProjectionV1, {
        let (_, _, _, root, base, _, view) = f;
        let req = requests(AdapterKind::BoundedProjectionV2);
        let frozen = bind_candidate(root, candidate(2), &req, view).unwrap();
        let owner = observe_breakcost_current(root, base, &frozen, view).unwrap();
        assert_cost(owner.historical_report(), 3, 4);
        assert_eq!(
            owner.historical_property(0, LegacyProperty::V1StrictConsumerAcceptance),
            Some(PropertyOutcome::ConfirmedViolation)
        );
        assert_eq!(
            owner.historical_property(0, LegacyProperty::JsonObjectEnvelope),
            Some(PropertyOutcome::Preserved)
        );
    });
}
#[test]
fn n07_unnegotiated_producer_has_no_output_and_no_observed_failure_scalar() {
    fixture!(f, AdapterKind::LegacyProjectionV1, {
        let (_, _, _, root, _, _, view) = f;
        let mut req = requests(AdapterKind::LegacyProjectionV1);
        req[0].requested_schema = V2;
        assert!(matches!(
            bind_candidate(root, candidate(1), &req, view),
            Err(Error::InvalidInput)
        ));
        // Closed adapters have no injectable encoding-failure callback/report path.
    });
}
#[path = "tlcx22_breakcost_support/budget_current.rs"]
mod budget_current;
