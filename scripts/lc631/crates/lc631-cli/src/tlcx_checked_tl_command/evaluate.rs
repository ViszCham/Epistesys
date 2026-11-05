//! One raw-config-bound run of real finite producers and current owned reduction.
use super::{config, MAX_CONFIG, MAX_OUTPUT};
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::{
    checked_tl_reduction::input::*,
    checked_tl_reduction::{self as reduction, Gate, Owners, RootPlan, TupleCandidate},
    joint_selection::AxisValue,
    resource::*,
    spatiotemporal_selection::{SpatialOwner, TemporalOwner},
    temporal_preservation::TemporalModelInput,
};
use serde::Serialize;

#[derive(Serialize)]
struct Interval {
    lower: String,
    upper: String,
}
#[derive(Serialize)]
struct Output {
    schema_version: &'static str,
    profile: &'static str,
    input_basis: &'static str,
    cross_domain_association: &'static str,
    source_semantics_verified: bool,
    verified_eligibility: bool,
    authority_created: bool,
    output_commit_allowed: bool,
    host_final_bound: bool,
    source_revision_hex: String,
    root_plan_digest_hex: String,
    weights: [u8; 2],
    denominator: u64,
    temporal: Option<Interval>,
    spatial: Option<Interval>,
    total: Option<Interval>,
    gaps: [Option<String>; 2],
    gates: [&'static str; 2],
    actual_temporal_observations: usize,
    actual_spatial_observations: usize,
}
fn interval(value: &lc631_core::measurement_numeric::RationalInterval) -> Interval {
    Interval {
        lower: value.lower().to_string(),
        upper: value.upper().to_string(),
    }
}
fn axis(value: AxisValue<'_>) -> Option<Interval> {
    value.interval().map(interval)
}
fn gate(value: Gate) -> &'static str {
    match value {
        Gate::Pass => "pass",
        Gate::Fail => "fail",
        Gate::NeedsEvidence => "needs_evidence",
        Gate::NoScalar => "no_scalar",
    }
}
fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}

pub(super) fn produce(raw: &str, view: &ResourceReservationView<'_>) -> Result<Vec<u8>, String> {
    if raw.is_empty() || raw.len() > MAX_CONFIG {
        return Err("tl_config:Limit".into());
    }
    view.check().map_err(|e| format!("tl_resource:{e:?}"))?;
    let source = SourceRevision::of(raw);
    if source.digest() != view.source_revision() {
        return Err("tl_config:SourceMismatch".into());
    }
    let requests = [
        ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(2 * 1024 * 1024).map_err(|e| format!("tl_resource:{e:?}"))?,
        ),
        ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(MAX_OUTPUT as u64).map_err(|e| format!("tl_resource:{e:?}"))?,
        ),
    ];
    let claim = view
        .claim_resources(&requests)
        .map_err(|e| format!("tl_resource:{e:?}"))?;
    let config: config::Config = serde_json::from_str(raw).map_err(|e| format!("tl_config:{e}"))?;
    config.check()?;
    let context = ContextRevision::original(source);
    let scope = config::scope(raw);
    let query = TemporalQuery::checked(
        context,
        vec![TemporalNode::Atom(0)],
        0,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .map_err(|e| format!("tl_query:{e:?}"))?;
    let source_trace = config.root.temporal.build(config.clock())?;
    let a = TemporalModelInput {
        query: &query,
        trace: &source_trace,
        scope,
    };
    let sa = config.root.spatial.build(&config, context, scope)?;
    let root_tp = config.time_policy(a, a)?;
    let root_sp = config.space_policy(sa, sa)?;
    let root =
        RootPlan::capture(a, sa, &root_tp, &root_sp, view).map_err(|e| format!("tl_root:{e:?}"))?;
    // Typed target construction follows immutable root capture. DTO presence
    // before this point is not authenticated preregistration or blindness.
    let target_trace = config.target.temporal.build(config.clock())?;
    let b = TemporalModelInput {
        query: &query,
        trace: &target_trace,
        scope,
    };
    let sb = config.target.spatial.build(&config, context, scope)?;
    let candidate =
        TupleCandidate::capture(&root, b, sb).map_err(|e| format!("tl_candidate:{e:?}"))?;
    let tp = config.time_policy(a, b)?;
    let sp = config.space_policy(sa, sb)?;
    let temporal = TemporalOwner::produce(&tp, a, b, view, view, EvaluationBudget::default())
        .map_err(|e| format!("tl_temporal:{e:?}"))?;
    let spatial =
        SpatialOwner::produce(&sp, sa, sb, view, view).map_err(|e| format!("tl_spatial:{e:?}"))?;
    let reduced = reduction::reduce_current(
        &root,
        &candidate,
        Owners {
            temporal: Some(&temporal),
            spatial: Some(&spatial),
        },
        view,
    )
    .map_err(|e| format!("tl_reduce:{e:?}"))?;
    let report = reduced
        .current(&root, &candidate, view)
        .map_err(|e| format!("tl_current:{e:?}"))?;
    let output = Output {
        schema_version: "epistesys-given-tl-tuple-result.v1",
        profile: reduction::PROFILE,
        input_basis: "given",
        cross_domain_association: "given_unverified",
        source_semantics_verified: false,
        verified_eligibility: false,
        authority_created: false,
        output_commit_allowed: false,
        host_final_bound: false,
        source_revision_hex: hex(source.digest().as_bytes()),
        root_plan_digest_hex: hex(root.digest().as_bytes()),
        weights: [1, 3],
        denominator: report.denominator(),
        temporal: axis(
            temporal
                .axis_current(view)
                .map_err(|e| format!("tl_temporal:{e:?}"))?,
        ),
        spatial: axis(
            spatial
                .axis_current(view)
                .map_err(|e| format!("tl_spatial:{e:?}"))?,
        ),
        total: report.total().map(interval),
        gaps: report.gaps().map(|gap| gap.map(|v| format!("{v:?}"))),
        gates: report.hard_gates().map(gate),
        actual_temporal_observations: usize::from(
            temporal.historical_report().source_observation().is_some(),
        ) + usize::from(
            temporal.historical_report().target_observation().is_some(),
        ),
        actual_spatial_observations: spatial.historical_report().source_observations().len()
            + spatial.historical_report().target_observations().len(),
    };
    let bytes = serde_json::to_vec(&output).map_err(|e| format!("tl_output:{e}"))?;
    if bytes.len() > MAX_OUTPUT {
        return Err("tl_output:Limit".into());
    }
    reduced
        .current(&root, &candidate, view)
        .map_err(|e| format!("tl_current:{e:?}"))?;
    claim.check().map_err(|e| format!("tl_resource:{e:?}"))?;
    Ok(bytes)
}
