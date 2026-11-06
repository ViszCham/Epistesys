//! Real native producers live through the consumer; no source-side report constructor.
// Shared fixture surface: individual integration targets use different subsets.
#![allow(dead_code)]
use super::{
    atoms::{self, CaseInput},
    expected, models, resources, wire,
};
use lc631_analysis::creativity_measurement::{ComponentState, Rational, RationalInterval};
use lc631_core::CandidateId;
use lc631_runtime::{
    adversarial_measurement as risk, checked_tl_reduction as tl, description_measurement as desc,
    facet_ledger::*,
    joint_selection::SelectionPins,
    product_correspondence::{
        CaseDeclaration, IdentityDeclaration, IdentityKind, NativeOwnerRefs, PlanSpec,
        ProductInput, ProductLimits, ProductPlan, TlProfile,
    },
    product_cost::NativeProductInput,
    reconstruction_measurement as recon,
    resource::*,
    spatiotemporal_selection::{SpatialOwner, TemporalOwner},
};
use lc631_solvers::temporal::{EvaluationBudget, Truth};

pub struct Fixture<'a, 'v> {
    pub input: NativeProductInput<'a, 'v>,
    pub equal_owners: NativeOwnerRefs<'a, 'v>,
    pub second: NativeProductInput<'a, 'v>,
    pub equal_plan: NativeProductInput<'a, 'v>,
    pub view: &'v ResourceReservationView<'v>,
    pub cancellation: &'a Cancellation,
    pub cases: &'a [CaseInput; 2],
    pub identities: &'a [IdentityDeclaration<'a>; 5],
    pub temporal_meanings: &'a [&'a [u8]; 1],
    pub source_temporal: lc631_runtime::temporal_preservation::TemporalModelInput<'a>,
    pub target_temporal: lc631_runtime::temporal_preservation::TemporalModelInput<'a>,
    pub source_spatial: lc631_runtime::spatial_preservation::SpatialModelInput,
    pub target_spatial: lc631_runtime::spatial_preservation::SpatialModelInput,
    pub expected_source: Vec<u8>,
    pub expected_target: Vec<u8>,
}

#[derive(Clone, Copy, Default)]
pub struct Options {
    pub renamed: bool,
    pub reverse_responses: bool,
    pub expiry: bool,
    pub single_atom: bool,
    pub changed_unused: bool,
    pub secondary_clock: bool,
}

/// Two cases/two atoms, including reversed raw/meaning and native-key/slot order.
/// Lifetimes prevent returning actual current owners outside the native fixture.
pub fn with_fixture<R>(
    options: Options,
    consume: impl for<'a, 'v> FnOnce(Fixture<'a, 'v>) -> R,
) -> R {
    // Test-only admission serialization bounds simultaneous real fixture stacks.
    let _serial = resources::FIXTURE_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    wire::literal_goldens();
    atoms::hand_oracle();
    let Options {
        renamed,
        reverse_responses,
        expiry,
        single_atom,
        changed_unused,
        secondary_clock,
    } = options;
    let cancellation = Cancellation::default();
    let (_broker, _run, reservation) = resources::run(cancellation.clone(), expiry);
    let view = reservation.lease();
    let context = models::context();
    let scope = models::scope();
    let inputs = atoms::inputs(renamed, single_atom, changed_unused);
    let one = Rational::from_integer(1.into());
    let cases = inputs.each_ref().map(|c| CaseDeclaration {
        native_key: c.key,
        native_atoms: &c.atoms,
        semantic_spec: &c.spec,
        reference_contract: &c.contract,
        oracle: models::pin(11),
        weight: &one,
    });
    let clock = models::clock_meaning(false);
    let second_clock = models::clock_meaning(true);
    let origin = models::meaning("Origin/primary", wire::symbol("epoch-zero"));
    let reference = models::meaning("Reference/primary", wire::symbol("first-event"));
    let frame = models::frame_meaning();
    let identities = [
        IdentityDeclaration {
            kind: IdentityKind::Clock,
            native_id: models::identity(renamed, 40),
            local_role: 0,
            meaning: &clock,
        },
        IdentityDeclaration {
            kind: IdentityKind::Clock,
            native_id: models::identity(renamed, 44),
            local_role: 1,
            meaning: &second_clock,
        },
        IdentityDeclaration {
            kind: IdentityKind::Origin,
            native_id: models::identity(renamed, 41),
            local_role: 0,
            meaning: &origin,
        },
        IdentityDeclaration {
            kind: IdentityKind::Reference,
            native_id: models::identity(renamed, 42),
            local_role: 0,
            meaning: &reference,
        },
        IdentityDeclaration {
            kind: IdentityKind::Frame,
            native_id: models::identity(renamed, 60),
            local_role: 0,
            meaning: &frame,
        },
    ];
    let time_atom = wire::encode(&wire::symbol("observed-p"));
    let time_atoms: [&[u8]; 1] = [&time_atom];
    let declaration = || PlanSpec {
        context,
        scope,
        goal_profile: None,
        tl_profile: TlProfile::TemporalSpatial1To3,
        cases: &cases,
        identities: &identities,
        temporal_atom_meanings: &time_atoms,
        limits: ProductLimits::default(),
    };
    let plan = ProductPlan::declared(declaration(), &view).unwrap();
    let equal_plan = ProductPlan::declared(declaration(), &view).unwrap();
    let mut base_responses = inputs.each_ref().map(|c| risk::Response {
        case: c.key,
        term: &c.baseline,
    });
    let mut target_responses = inputs.each_ref().map(|c| risk::Response {
        case: c.key,
        term: &c.target,
    });
    if reverse_responses {
        base_responses.reverse();
        target_responses.reverse();
    }
    let source_query = models::query();
    let target_query = models::query();
    let source_trace = models::trace(renamed, secondary_clock, Truth::True);
    let target_trace = models::trace(renamed, secondary_clock, Truth::False);
    let source_time = models::temporal(&source_query, &source_trace);
    let target_time = models::temporal(&target_query, &target_trace);
    let source_space = models::spatial(renamed, false);
    let target_space = models::spatial(renamed, true);
    let base_input = || ProductInput {
        responses: &base_responses,
        temporal: source_time,
        spatial: source_space,
    };
    let target_input = || ProductInput {
        responses: &target_responses,
        temporal: target_time,
        spatial: target_space,
    };
    let baseline = plan.project_reported(base_input(), &view).unwrap();
    let target = plan.project_reported(target_input(), &view).unwrap();
    let equal_baseline = equal_plan.project_reported(base_input(), &view).unwrap();
    let equal_target = equal_plan.project_reported(target_input(), &view).unwrap();
    let expected_source = expected::product(&inputs, false, secondary_clock);
    let expected_target = expected::product(&inputs, true, secondary_clock);
    assert_eq!(baseline.semantic_bytes(), expected_source);
    assert_eq!(target.semantic_bytes(), expected_target);

    let pins = SelectionPins {
        source: context.source(),
        scope,
        baseline: models::pin(5),
        hard_invariants: models::pin(6),
        policy: models::pin(7),
        goal_profile: None,
        evidence_revision: models::pin(9),
    };
    let attacks = inputs.each_ref().map(|c| risk::AttackCase {
        key: c.key,
        atoms: &c.atoms,
        theory: &c.theory,
        reference: &c.reference,
        weight: &one,
    });
    let catalog = risk::register(
        risk::CatalogBinding {
            selection: pins,
            context,
            suite: models::pin(10),
            attack_scope: models::pin(12),
            oracle: models::pin(11),
            definition: risk::definition_revision(),
            probe_set: scope.probes(),
            dependency_group: models::pin(13),
            purpose: risk::CasePurpose::SoftFiniteValidation,
            quantification: risk::Quantification::AllAdmissibleAssignments,
        },
        &attacks,
        risk::AttackLimits {
            registration_ast_visits: 100_000,
            max_ast_visits: 100_000,
            max_trials: 2,
        },
        &view,
    )
    .unwrap();
    let binding = |n| risk::CandidateBinding {
        id: CandidateId(n),
        candidate_digest: models::pin(20 + n as u8),
        context,
        target: models::pin(30 + n as u8),
        transform: models::pin(40 + n as u8),
    };
    let frozen_base = risk::bind_candidate(&catalog, binding(0), &base_responses, &view).unwrap();
    let base_risk = risk::execute(&frozen_base, &view).unwrap();
    let frozen_target =
        risk::bind_candidate(&catalog, binding(1), &target_responses, &view).unwrap();
    let target_risk = risk::execute(&frozen_target, &view).unwrap();
    let equal_frozen_base =
        risk::bind_candidate(&catalog, binding(0), &base_responses, &view).unwrap();
    let equal_base_risk = risk::execute(&equal_frozen_base, &view).unwrap();
    let equal_frozen_target =
        risk::bind_candidate(&catalog, binding(1), &target_responses, &view).unwrap();
    let equal_target_risk = risk::execute(&equal_frozen_target, &view).unwrap();
    assert_eq!(
        base_risk.historical_trials(),
        [risk::TrialState::Failure; 2]
    );
    assert_eq!(target_risk.historical_trials(), [risk::TrialState::Pass; 2]);
    assert_eq!(
        base_risk.historical_report().state,
        ComponentState::Measured(RationalInterval::exact_ratio(1, 1).unwrap())
    );
    assert_eq!(
        target_risk.historical_report().state,
        ComponentState::Measured(RationalInterval::exact_ratio(0, 1).unwrap())
    );

    let source_tp = models::time_policy(source_time, source_time);
    let source_sp = models::space_policy(source_space, source_space);
    let target_tp = models::time_policy(source_time, target_time);
    let target_sp = models::space_policy(source_space, target_space);
    let tl_root =
        tl::RootPlan::capture(source_time, source_space, &source_tp, &source_sp, &view).unwrap();
    let tuple = tl::TupleCandidate::capture(&tl_root, target_time, target_space).unwrap();
    let time = TemporalOwner::produce(
        &target_tp,
        source_time,
        target_time,
        &view,
        &view,
        EvaluationBudget::default(),
    )
    .unwrap();
    let space =
        SpatialOwner::produce(&target_sp, source_space, target_space, &view, &view).unwrap();
    let reduction = tl::reduce_current(
        &tl_root,
        &tuple,
        tl::Owners {
            temporal: Some(&time),
            spatial: Some(&space),
        },
        &view,
    )
    .unwrap();
    let equal_root =
        tl::RootPlan::capture(source_time, source_space, &source_tp, &source_sp, &view).unwrap();
    let equal_tuple = tl::TupleCandidate::capture(&equal_root, target_time, target_space).unwrap();
    let equal_reduction = tl::reduce_current(
        &equal_root,
        &equal_tuple,
        tl::Owners {
            temporal: Some(&time),
            spatial: Some(&space),
        },
        &view,
    )
    .unwrap();

    let desc_scope = ComparisonScope {
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
    let features = recon::ProtectedFeatureSet::capture(
        baseline.semantic_bytes(),
        &probes,
        context,
        desc_scope,
        &view,
    )
    .unwrap();
    let base_recon = recon::produce(&features, baseline.semantic_bytes(), &view).unwrap();
    let target_recon = recon::produce(&features, target.semantic_bytes(), &view).unwrap();
    let base_desc_input = desc::ReconstructionInput {
        owner: &base_recon,
        plan: &features,
        target: baseline.semantic_bytes(),
        context,
        transform: TransformId([80; 32]),
    };
    let target_desc_input = desc::ReconstructionInput {
        owner: &target_recon,
        plan: &features,
        target: target.semantic_bytes(),
        context,
        transform: TransformId([81; 32]),
    };
    let desc_root = desc::RootPlan::capture(base_desc_input, 16, &view).unwrap();
    let base_desc = desc::produce(&desc_root, base_desc_input, &view).unwrap();
    let target_desc = desc::produce(&desc_root, target_desc_input, &view).unwrap();
    let equal_desc_root = desc::RootPlan::capture(base_desc_input, 16, &view).unwrap();
    let equal_base_desc = desc::produce(&equal_desc_root, base_desc_input, &view).unwrap();
    let equal_target_desc = desc::produce(&equal_desc_root, target_desc_input, &view).unwrap();
    let owners = NativeOwnerRefs {
        baseline_description: &base_desc,
        target_description: &target_desc,
        baseline_risk: &base_risk,
        baseline_responses: &frozen_base,
        target_risk: &target_risk,
        target_responses: &frozen_target,
        tl_root: &tl_root,
        tl_candidate: &tuple,
        tl_reduction: &reduction,
    };
    let equal_owners = NativeOwnerRefs {
        baseline_description: &equal_base_desc,
        target_description: &equal_target_desc,
        baseline_risk: &equal_base_risk,
        baseline_responses: &equal_frozen_base,
        target_risk: &equal_target_risk,
        target_responses: &equal_frozen_target,
        tl_root: &equal_root,
        tl_candidate: &equal_tuple,
        tl_reduction: &equal_reduction,
    };

    // Fresh second actual candidate reuses ONLY the fixed source/baseline subset.
    let second = plan.project_reported(base_input(), &view).unwrap();
    let second_frozen = risk::bind_candidate(&catalog, binding(2), &base_responses, &view).unwrap();
    let second_risk = risk::execute(&second_frozen, &view).unwrap();
    let second_tuple = tl::TupleCandidate::capture(&tl_root, source_time, source_space).unwrap();
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
    let second_recon = recon::produce(&features, second.semantic_bytes(), &view).unwrap();
    let second_desc = desc::produce(
        &desc_root,
        desc::ReconstructionInput {
            owner: &second_recon,
            plan: &features,
            target: second.semantic_bytes(),
            context,
            transform: TransformId([82; 32]),
        },
        &view,
    )
    .unwrap();
    let second_owners = NativeOwnerRefs {
        target_description: &second_desc,
        target_risk: &second_risk,
        target_responses: &second_frozen,
        tl_candidate: &second_tuple,
        tl_reduction: &second_reduction,
        ..owners
    };
    consume(Fixture {
        input: NativeProductInput {
            plan: &plan,
            baseline: &baseline,
            target: &target,
            owners,
        },
        equal_owners,
        second: NativeProductInput {
            plan: &plan,
            baseline: &baseline,
            target: &second,
            owners: second_owners,
        },
        equal_plan: NativeProductInput {
            plan: &equal_plan,
            baseline: &equal_baseline,
            target: &equal_target,
            owners,
        },
        view: &view,
        cancellation: &cancellation,
        cases: &inputs,
        identities: &identities,
        temporal_meanings: &time_atoms,
        expected_source,
        expected_target,
        source_temporal: source_time,
        target_temporal: target_time,
        source_spatial: source_space,
        target_spatial: target_space,
    })
}
