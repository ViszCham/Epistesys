//! Actual native finite owners. Caller replicas enter ONLY project_reported.
//! Legacy Description is retained, never retagged as a full-cost product K owner.
mod models;
mod mutations;
mod resources;
use lc631_analysis::creativity_measurement::{InputBasis, Rational, SemanticValidation};
use lc631_core::CandidateId;
use lc631_runtime::{
    adversarial_measurement as risk, checked_tl_reduction as tl, description_measurement as desc,
    facet_ledger::*,
    joint_selection::SelectionPins,
    product_correspondence::{
        inspect_native_current, CaseDeclaration, DeferredReason, IdentityDeclaration, IdentityKind,
        NativeOwnerRefs, PlanSpec, ProductInput, ProductLimits, ProductPlan, TlProfile,
    },
    reconstruction_measurement as recon,
    resource::*,
    spatiotemporal_selection::{SpatialOwner, TemporalOwner},
};
use lc631_solvers::{
    temporal::{EvaluationBudget, Truth},
    typed_query::{BoolExpr, SmtEmitLimits, SmtStatusQuery},
};
use lc631_substrate::{semantic_term::Term, Digest32};
use resources::run;
use std::time::Duration;

#[derive(Clone, Copy)]
pub enum Mode {
    Normal,
    Renamed,
    SplitTargetClock,
}
pub struct ReportedSnapshot {
    pub source: Vec<u8>,
    pub target: Vec<u8>,
    pub envelope: Vec<u8>,
}
fn pin(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn exercise(mode: Mode, expiry: bool) -> ReportedSnapshot {
    let context = models::context();
    let scope = models::scope();
    let cancel = Cancellation::default();
    let (_broker, _run, reservation) = run(cancel.clone(), expiry);
    let view = reservation.lease();
    let key: &[u8] = if matches!(mode, Mode::Renamed) {
        b"renamed-case"
    } else {
        b"native-case"
    };
    let spec = models::case_spec();
    let reference_bytes = models::encoded(Term::Bool(true));
    let one = Rational::from_integer(1.into());
    let cases = [CaseDeclaration {
        native_key: key,
        native_atoms: &[],
        semantic_spec: &spec,
        reference_contract: &reference_bytes,
        oracle: pin(11),
        weight: &one,
    }];
    let primary_clock = models::clock_meaning("primary");
    let secondary_clock = models::clock_meaning("secondary");
    let origin = models::meaning("Origin/primary", models::symbol("epoch-zero"));
    let reference = models::meaning("Reference/primary", models::symbol("first-event"));
    let frame = models::frame_meaning();
    // Both clock roles are fixed in the WHOLE plan, before any product. The
    // secondary can be used by a later target without adding/relabeling roles.
    let identities = [
        IdentityDeclaration {
            kind: IdentityKind::Clock,
            native_id: models::identity(mode, 40),
            local_role: 0,
            meaning: &primary_clock,
        },
        IdentityDeclaration {
            kind: IdentityKind::Clock,
            native_id: models::identity(mode, 44),
            local_role: 1,
            meaning: &secondary_clock,
        },
        IdentityDeclaration {
            kind: IdentityKind::Origin,
            native_id: models::identity(mode, 41),
            local_role: 0,
            meaning: &origin,
        },
        IdentityDeclaration {
            kind: IdentityKind::Reference,
            native_id: models::identity(mode, 42),
            local_role: 0,
            meaning: &reference,
        },
        IdentityDeclaration {
            kind: IdentityKind::Frame,
            native_id: models::identity(mode, 60),
            local_role: 0,
            meaning: &frame,
        },
    ];
    let atom_meaning = models::encoded(models::symbol("observed-p"));
    let atom_meanings: [&[u8]; 1] = [&atom_meaning];
    let plan_spec = PlanSpec {
        context,
        scope,
        goal_profile: None,
        tl_profile: TlProfile::TemporalSpatial1To3,
        cases: &cases,
        identities: &identities,
        temporal_atom_meanings: &atom_meanings,
        limits: ProductLimits::default(),
    };
    let plan = ProductPlan::declared(plan_spec, &view).unwrap();
    assert_eq!(plan.fixed_slots().len(), 1);
    assert_eq!(plan.fixed_slots()[0].native_key(), key);
    let source_query = models::query();
    let target_query = models::query();
    let source_trace = models::trace(mode, false, Truth::True);
    let target_trace = models::trace(mode, matches!(mode, Mode::SplitTargetClock), Truth::False);
    let source_time = models::temporal(&source_query, &source_trace);
    let target_time = models::temporal(&target_query, &target_trace);
    let source_space = models::spatial(mode, false);
    let target_space = models::spatial(mode, true);
    let yes = BoolExpr::Constant(true);
    let no = BoolExpr::Constant(false);
    // Baseline FALSE is intentionally different from the fixed TRUE oracle.
    let baseline_responses = [risk::Response {
        case: key,
        term: &no,
    }];
    let target_responses = [risk::Response {
        case: key,
        term: &yes,
    }];
    let baseline = plan
        .project_reported(
            ProductInput {
                responses: &baseline_responses,
                temporal: source_time,
                spatial: source_space,
            },
            &view,
        )
        .unwrap();
    let target = plan
        .project_reported(
            ProductInput {
                responses: &target_responses,
                temporal: target_time,
                spatial: target_space,
            },
            &view,
        )
        .unwrap();
    assert_eq!(
        baseline.scalar_status(),
        DeferredReason::FullCostOwnerMissing
    );
    assert!(!target.formula_evaluated());

    let pins = SelectionPins {
        source: context.source(),
        scope,
        baseline: pin(5),
        hard_invariants: pin(6),
        policy: pin(7),
        goal_profile: None,
        evidence_revision: pin(9),
    };
    let theory = SmtStatusQuery::checked(vec![], vec![], vec![], SmtEmitLimits::default()).unwrap();
    let attack = [risk::AttackCase {
        key,
        atoms: &[],
        theory: &theory,
        reference: &yes,
        weight: &one,
    }];
    let catalog = risk::register(
        risk::CatalogBinding {
            selection: pins,
            context,
            suite: pin(10),
            attack_scope: pin(12),
            oracle: pin(11),
            definition: risk::definition_revision(),
            probe_set: scope.probes(),
            dependency_group: pin(13),
            purpose: risk::CasePurpose::SoftFiniteValidation,
            quantification: risk::Quantification::AllAdmissibleAssignments,
        },
        &attack,
        risk::AttackLimits {
            registration_ast_visits: 100_000,
            max_ast_visits: 100_000,
            max_trials: 2,
        },
        &view,
    )
    .unwrap();
    let native_binding = |n| risk::CandidateBinding {
        id: CandidateId(n),
        candidate_digest: pin(20 + n as u8),
        context,
        target: pin(30 + n as u8),
        transform: pin(40 + n as u8),
    };
    let frozen_baseline =
        risk::bind_candidate(&catalog, native_binding(0), &baseline_responses, &view).unwrap();
    let baseline_risk = risk::execute(&frozen_baseline, &view).unwrap();
    let frozen_target =
        risk::bind_candidate(&catalog, native_binding(1), &target_responses, &view).unwrap();
    let target_risk = risk::execute(&frozen_target, &view).unwrap();

    let source_tp = models::time_policy(source_time, source_time);
    let source_sp = models::space_policy(source_space, source_space);
    let tl_root =
        tl::RootPlan::capture(source_time, source_space, &source_tp, &source_sp, &view).unwrap();
    let tl_candidate = tl::TupleCandidate::capture(&tl_root, target_time, target_space).unwrap();
    let target_tp = models::time_policy(source_time, target_time);
    let target_sp = models::space_policy(source_space, target_space);
    let time_owner = TemporalOwner::produce(
        &target_tp,
        source_time,
        target_time,
        &view,
        &view,
        EvaluationBudget::default(),
    )
    .unwrap();
    let space_owner =
        SpatialOwner::produce(&target_sp, source_space, target_space, &view, &view).unwrap();
    let reduction = tl::reduce_current(
        &tl_root,
        &tl_candidate,
        tl::Owners {
            temporal: Some(&time_owner),
            spatial: Some(&space_owner),
        },
        &view,
    );
    if matches!(mode, Mode::SplitTargetClock) {
        // This is only a reported semantic-partition comparison. The actual
        // root froze primary→primary; do not fabricate a cross-clock positive,
        // change its mapping or issue a native correspondence inspection token.
        assert!(matches!(reduction, Err(tl::Error::BindingMismatch)));
        return ReportedSnapshot {
            source: baseline.semantic_bytes().to_vec(),
            target: target.semantic_bytes().to_vec(),
            envelope: target.identity_envelope_bytes().to_vec(),
        };
    }
    let reduction = reduction.unwrap();
    assert!(reduction
        .current(&tl_root, &tl_candidate, &view)
        .unwrap()
        .total()
        .is_some());

    let description_scope = ComparisonScope {
        id: ScopeId([1; 32]),
        baseline: Revision([5; 32]),
        root_goal: Revision([2; 32]),
        probes: Revision([3; 32]),
        policy: Revision([4; 32]),
    };
    let probes = [recon::ProbeSpec {
        path: vec![],
        kind: recon::FeatureKind::NodeKind,
        weight: one.clone(),
    }];
    let feature_plan = recon::ProtectedFeatureSet::capture(
        baseline.semantic_bytes(),
        &probes,
        context,
        description_scope,
        &view,
    )
    .unwrap();
    let base_reconstruction =
        recon::produce(&feature_plan, baseline.semantic_bytes(), &view).unwrap();
    let target_reconstruction =
        recon::produce(&feature_plan, target.semantic_bytes(), &view).unwrap();
    let base_input = desc::ReconstructionInput {
        owner: &base_reconstruction,
        plan: &feature_plan,
        target: baseline.semantic_bytes(),
        context,
        transform: TransformId([80; 32]),
    };
    let description_root = desc::RootPlan::capture(base_input, 16, &view).unwrap();
    let baseline_description = desc::produce(&description_root, base_input, &view).unwrap();
    let target_description = desc::produce(
        &description_root,
        desc::ReconstructionInput {
            owner: &target_reconstruction,
            plan: &feature_plan,
            target: target.semantic_bytes(),
            context,
            transform: TransformId([81; 32]),
        },
        &view,
    )
    .unwrap();
    let owners = NativeOwnerRefs {
        baseline_description: &baseline_description,
        target_description: &target_description,
        baseline_risk: &baseline_risk,
        baseline_responses: &frozen_baseline,
        target_risk: &target_risk,
        target_responses: &frozen_target,
        tl_root: &tl_root,
        tl_candidate: &tl_candidate,
        tl_reduction: &reduction,
    };
    let inspection = inspect_native_current(&plan, &baseline, &target, owners, &view).unwrap();
    inspection
        .with_current(&view, |current| {
            assert_eq!(
                current.scalar_status(),
                DeferredReason::FullCostOwnerMissing
            );
            assert!(current.scalar().is_none());
            assert!(!current.formula_evaluated());
            assert!(std::ptr::eq(
                current.baseline_description_report(),
                baseline_description.report_current(&view).unwrap()
            ));
            assert!(std::ptr::eq(
                current.target_description_report(),
                target_description.report_current(&view).unwrap()
            ));
            assert!(std::ptr::eq(
                current.baseline_risk_report(),
                baseline_risk
                    .report_current(&frozen_baseline, &view)
                    .unwrap()
            ));
            assert!(std::ptr::eq(
                current.target_risk_report(),
                target_risk.report_current(&frozen_target, &view).unwrap()
            ));
            assert!(std::ptr::eq(
                current.translation_report(),
                reduction.current(&tl_root, &tl_candidate, &view).unwrap()
            ));
            assert_eq!(current.target_risk_report().basis, InputBasis::Given);
            assert_eq!(
                current.target_description_report().validation,
                SemanticValidation::Unassessed
            );
            assert_ne!(
                current.product_baseline_digest(),
                current.actual_tl_root_digest()
            );
        })
        .unwrap();
    let saved = [
        baseline_description.historical_report().clone(),
        target_description.historical_report().clone(),
        baseline_risk.historical_report().clone(),
        target_risk.historical_report().clone(),
    ];

    if !expiry {
        // SAME owners/root/plan for every byte-field mutation. Neither a foreign
        // object pointer rejection nor a frozen candidate hash can satisfy this.
        mutations::all_fields(&plan, &baseline, &target, owners, &view);
        mutations::frozen_plan_fields(
            &cases,
            &identities,
            &atom_meanings,
            &ProductInput {
                responses: &baseline_responses,
                temporal: source_time,
                spatial: source_space,
            },
            &ProductInput {
                responses: &target_responses,
                temporal: target_time,
                spatial: target_space,
            },
            owners,
            &view,
        );
        // Preserve the target's FALSE left-of relation, temporal truth and all
        // Risk inputs while changing actual coordinates. Matching real legacy
        // Descriptions cannot hide this difference from the actual TL visitor.
        let mut moved_target = target_space;
        moved_target.left = Some(
            lc631_solvers::causal_spatial::Rectangle::new(target_space.frame, [5, 0], [7, 2])
                .unwrap(),
        );
        let left = moved_target.left.unwrap();
        let right = moved_target.right.unwrap();
        assert_eq!(
            left.max()[0] < right.min()[0],
            target_space.left.unwrap().max()[0] < target_space.right.unwrap().min()[0]
        );
        let moved = plan
            .project_reported(
                ProductInput {
                    responses: &target_responses,
                    temporal: target_time,
                    spatial: moved_target,
                },
                &view,
            )
            .unwrap();
        mutations::rejects_with_matching_descriptions(&plan, &baseline, &moved, owners, &view);
        // Wrong source/baseline with the same TRUE relation also receives fully
        // matching Description source/target owners. Actual TL root stays SAME.
        let mut moved_source = source_space;
        moved_source.left = Some(
            lc631_solvers::causal_spatial::Rectangle::new(source_space.frame, [1, 0], [3, 2])
                .unwrap(),
        );
        moved_source.right = Some(
            lc631_solvers::causal_spatial::Rectangle::new(source_space.frame, [5, 0], [7, 2])
                .unwrap(),
        );
        let moved = plan
            .project_reported(
                ProductInput {
                    responses: &baseline_responses,
                    temporal: source_time,
                    spatial: moved_source,
                },
                &view,
            )
            .unwrap();
        mutations::rejects_with_matching_descriptions(&plan, &moved, &target, owners, &view);
        // SECOND valid actual target under the SAME plan/root. It differs in all
        // three parts and has matching fresh native owners; must inspect too.
        let second = plan
            .project_reported(
                ProductInput {
                    responses: &baseline_responses,
                    temporal: source_time,
                    spatial: source_space,
                },
                &view,
            )
            .unwrap();
        let frozen_second =
            risk::bind_candidate(&catalog, native_binding(2), &baseline_responses, &view).unwrap();
        let second_risk = risk::execute(&frozen_second, &view).unwrap();
        let second_tuple =
            tl::TupleCandidate::capture(&tl_root, source_time, source_space).unwrap();
        let second_time = TemporalOwner::produce(
            &source_tp,
            source_time,
            source_time,
            &view,
            &view,
            EvaluationBudget::default(),
        )
        .unwrap();
        let second_space =
            SpatialOwner::produce(&source_sp, source_space, source_space, &view, &view).unwrap();
        let second_reduction = tl::reduce_current(
            &tl_root,
            &second_tuple,
            tl::Owners {
                temporal: Some(&second_time),
                spatial: Some(&second_space),
            },
            &view,
        )
        .unwrap();
        let second_reconstruction =
            recon::produce(&feature_plan, second.semantic_bytes(), &view).unwrap();
        let second_description = desc::produce(
            &description_root,
            desc::ReconstructionInput {
                owner: &second_reconstruction,
                plan: &feature_plan,
                target: second.semantic_bytes(),
                context,
                transform: TransformId([82; 32]),
            },
            &view,
        )
        .unwrap();
        let second_owners = NativeOwnerRefs {
            target_description: &second_description,
            target_risk: &second_risk,
            target_responses: &frozen_second,
            tl_candidate: &second_tuple,
            tl_reduction: &second_reduction,
            ..owners
        };
        let second_inspection =
            inspect_native_current(&plan, &baseline, &second, second_owners, &view).unwrap();
        second_inspection
            .with_current(&view, |current| {
                assert!(std::ptr::eq(
                    current.target_risk_report(),
                    second_risk.report_current(&frozen_second, &view).unwrap()
                ));
                assert_ne!(current.target_risk_report().state, saved[3].state);
                assert_ne!(
                    current.translation_report().total(),
                    reduction.historical().total()
                );
                assert_eq!(
                    current.scalar_status(),
                    DeferredReason::FullCostOwnerMissing
                );
            })
            .unwrap();
        assert!(inspect_native_current(&plan, &baseline, &second, owners, &view).is_err());
        assert!(inspect_native_current(&plan, &baseline, &target, second_owners, &view).is_err());
        let (_fb, _fr, foreign_res) = run(Cancellation::default(), false);
        let foreign = foreign_res.lease();
        assert!(inspection.with_current(&foreign, |_| ()).is_err());
        assert!(inspect_native_current(&plan, &baseline, &target, owners, &foreign).is_err());
    }
    let snapshot = ReportedSnapshot {
        source: baseline.semantic_bytes().to_vec(),
        target: target.semantic_bytes().to_vec(),
        envelope: target.identity_envelope_bytes().to_vec(),
    };
    assert_eq!(baseline_description.historical_report(), &saved[0]);
    assert_eq!(target_description.historical_report(), &saved[1]);
    assert_eq!(baseline_risk.historical_report(), &saved[2]);
    assert_eq!(target_risk.historical_report(), &saved[3]);
    if expiry {
        let remaining = view
            .remaining()
            .expect("fixture first reaches a live captured inspection");
        assert!(remaining <= Duration::from_secs(2));
        std::thread::sleep(remaining + Duration::from_millis(5));
        assert!(inspection.with_current(&view, |_| ()).is_err());
    } else {
        assert!(inspection.with_current(&view, |_| cancel.cancel()).is_err());
        assert!(inspection.with_current(&view, |_| ()).is_err());
    }
    snapshot
}
