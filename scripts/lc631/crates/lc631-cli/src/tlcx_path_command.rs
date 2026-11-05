//! Explicit caller-supplied K6 artifacts only; no NL or host-final assertion.
#![forbid(unsafe_code)]
use crate::{effect_error::CliFailure, io::CommandIo};
use lc631_analysis::creativity_measurement::{Rational, RationalInterval};
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::{
    facet_ledger::{ComparisonScope, Facet, ObligationId, PropertyId, Revision, ScopeId},
    path_preservation::{
        consume_root_gate, evaluate_path, PathEdge, RegisteredMap, RootGate, RootPathPlan,
        RootProbeSpec,
    },
    reconstruction_measurement::{FeatureKind, Gap, ProbeSpec},
    resource::{Digest32, ResourceAmount, ResourceKind, ResourceRequest, ResourceReservationView},
};
use serde::{Deserialize, Serialize};
use std::io::Write;

const MAX_CONFIG: usize = 256 * 1024;
const MAX_TERM: usize = 65_536;
const MAX_OUTPUT: usize = 64 * 1024;
const ADAPTER_RAM: u64 = 2 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config<'a> {
    schema_version: &'a str,
    root_hex: &'a str,
    final_hex: &'a str,
    #[serde(borrow)]
    edges: Vec<Edge<'a>>,
    #[serde(borrow)]
    namespace: Namespace<'a>,
    #[serde(borrow)]
    scope: Scope<'a>,
    #[serde(borrow)]
    probes: Vec<Probe<'a>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edge<'a> {
    from_hex: &'a str,
    to_hex: &'a str,
    #[serde(borrow)]
    namespace: Namespace<'a>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Namespace<'a> {
    id_hex: &'a str,
    revision_hex: &'a str,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scope<'a> {
    id_hex: &'a str,
    baseline_hex: &'a str,
    root_goal_hex: &'a str,
    probes_hex: &'a str,
    policy_hex: &'a str,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Probe<'a> {
    obligation_hex: &'a str,
    property_hex: &'a str,
    path: Vec<usize>,
    kind: FeatureKindInput,
    weight_numerator: u64,
    weight_denominator: u64,
    hard: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum FeatureKindInput {
    NodeKind,
    Symbol,
    Boolean,
    Integer,
    Operator,
    Arity,
    Binder,
    Sort,
    BoundIndex,
}
impl FeatureKindInput {
    fn feature(&self) -> FeatureKind {
        match self {
            Self::NodeKind => FeatureKind::NodeKind,
            Self::Symbol => FeatureKind::Symbol,
            Self::Boolean => FeatureKind::Boolean,
            Self::Integer => FeatureKind::Integer,
            Self::Operator => FeatureKind::Operator,
            Self::Arity => FeatureKind::Arity,
            Self::Binder => FeatureKind::Binder,
            Self::Sort => FeatureKind::Sort,
            Self::BoundIndex => FeatureKind::BoundIndex,
        }
    }
}

#[derive(Serialize)]
struct Interval {
    lower: String,
    upper: String,
}
#[derive(Serialize)]
struct Observation {
    target_artifact_digest_hex: String,
    loss: Option<Interval>,
    gap: Option<&'static str>,
}
#[derive(Serialize)]
struct Defect {
    obligation_hex: String,
    property_hex: String,
    root_revision_hex: String,
    target_revision_hex: String,
}
#[derive(Serialize)]
struct Output {
    schema_version: &'static str,
    profile: &'static str,
    input_basis: &'static str,
    semantic_validation: &'static str,
    source_revision_hex: String,
    artifact_digest_scheme: &'static str,
    root_artifact_digest_hex: String,
    final_artifact_digest_hex: String,
    original_text_to_k6_verified: bool,
    host_final_bound: bool,
    authority_created: bool,
    output_commit_allowed: bool,
    history: Vec<Observation>,
    final_observation: Observation,
    actual_evaluation_count: usize,
    unique_defect_count: usize,
    defects: Vec<Defect>,
    uncertainty_count: usize,
    gate: &'static str,
}

/// Dispatcher must require exactly `--config-stdin` before invoking this entry.
pub(crate) fn run(resources: &CommandIo) -> Result<(), CliFailure> {
    let source = resources
        .read_from(&mut std::io::stdin().lock(), MAX_CONFIG as u64)
        .map_err(CliFailure::not_attempted)?;
    if source.len() > MAX_CONFIG {
        return Err(CliFailure::not_attempted("path_config:Limit"));
    }
    let bytes = resources
        .with_path_services(&source, |view| produce(&source, view))
        .map_err(CliFailure::not_attempted)?;
    std::io::stdout()
        .lock()
        .write_all(&bytes)
        .map_err(|_| CliFailure::from("path_output:UnknownDeliveryNoRetry"))
}

fn produce(source: &str, view: &ResourceReservationView<'_>) -> Result<Vec<u8>, String> {
    let amount = ResourceAmount::checked(ADAPTER_RAM).map_err(|e| format!("path_claim:{e:?}"))?;
    let output_amount =
        ResourceAmount::checked(MAX_OUTPUT as u64).map_err(|e| format!("path_claim:{e:?}"))?;
    let claim = view
        .claim_resources(&[
            ResourceRequest::new(ResourceKind::Ram, amount),
            ResourceRequest::new(ResourceKind::Output, output_amount),
        ])
        .map_err(|e| format!("path_claim:{e:?}"))?;
    let config: Config<'_> =
        serde_json::from_str(source).map_err(|e| format!("path_config:{e}"))?;
    if config.schema_version != "epistesys-given-k6-path.v1"
        || config.edges.len() > 2
        || config.probes.is_empty()
        || config.probes.len() > 4
    {
        return Err("path_config:InvalidProfileOrCount".into());
    }
    let scope = scope(&config.scope)?;
    let mapping = namespace(&config.namespace)?;
    let root = unhex(config.root_hex, MAX_TERM)?;
    let final_bytes = unhex(config.final_hex, MAX_TERM)?;
    let probes = probes(&config.probes)?;
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(config.edges.len())
        .map_err(|_| "path_config:Allocation")?;
    for edge in &config.edges {
        decoded.push((
            unhex(edge.from_hex, MAX_TERM)?,
            unhex(edge.to_hex, MAX_TERM)?,
            namespace(&edge.namespace)?,
        ));
    }
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(decoded.len())
        .map_err(|_| "path_config:Allocation")?;
    for (from, to, map) in &decoded {
        edges.push(PathEdge::new(from, to, *map));
    }
    let revision = SourceRevision::of(source);
    let plan = RootPathPlan::capture(
        &root,
        &probes,
        ContextRevision::original(revision),
        scope,
        mapping,
        view,
    )
    .map_err(|e| format!("path_capture:{e:?}"))?;
    let result = evaluate_path(&plan, &edges, &final_bytes, view)
        .map_err(|e| format!("path_evaluate:{e:?}"))?;
    let gate = consume_root_gate(Some(&result), &plan, &final_bytes, view)
        .map_err(|e| format!("path_gate:{e:?}"))?;
    let mut history = Vec::new();
    history
        .try_reserve_exact(result.history().len())
        .map_err(|_| "path_output:Allocation")?;
    for (item, (_, target, _)) in result.history().iter().zip(&decoded) {
        history.push(observation(item.loss(), item.gap(), target));
    }
    let mut defects = Vec::new();
    defects
        .try_reserve_exact(12)
        .map_err(|_| "path_output:Allocation")?;
    for entry in result.ledger().unique_defects(scope) {
        if let Facet::Defect(d) = entry.facet {
            defects.push(Defect {
                obligation_hex: hex(&d.key.root.0),
                property_hex: hex(&d.key.property.0),
                root_revision_hex: hex(&d.key.source.0),
                target_revision_hex: hex(&d.key.target.0),
            });
        }
    }
    let value = Output {
        schema_version: "epistesys-given-k6-path-result.v1",
        profile: "given-k6-path-v1",
        input_basis: "given",
        semantic_validation: "unverified",
        source_revision_hex: hex(revision.digest().as_bytes()),
        artifact_digest_scheme: "sha256-domain-nul-lowercase-hex-v1",
        root_artifact_digest_hex: artifact_digest(&root),
        final_artifact_digest_hex: artifact_digest(&final_bytes),
        original_text_to_k6_verified: false,
        host_final_bound: false,
        authority_created: false,
        output_commit_allowed: false,
        history,
        final_observation: observation(
            result.final_observation().loss(),
            result.final_observation().gap(),
            &final_bytes,
        ),
        actual_evaluation_count: result.actual_evaluation_count(),
        unique_defect_count: defects.len(),
        defects,
        uncertainty_count: result
            .ledger()
            .entries()
            .iter()
            .filter(|e| matches!(e.facet, Facet::Uncertainty(_)))
            .count(),
        gate: match gate {
            RootGate::PassInGivenFiniteProfile => "pass_in_given_finite_profile",
            RootGate::FailInGivenFiniteProfile => "fail_in_given_finite_profile",
            RootGate::NeedsEvidence => "needs_evidence",
        },
    };
    let bytes = serde_json::to_vec(&value).map_err(|e| format!("path_output:{e}"))?;
    if bytes.len() > MAX_OUTPUT {
        return Err("path_output:Limit".into());
    }
    claim.check().map_err(|e| format!("path_claim:{e:?}"))?;
    consume_root_gate(Some(&result), &plan, &final_bytes, view)
        .map_err(|e| format!("path_gate:{e:?}"))?;
    Ok(bytes)
}
fn observation(value: Option<&RationalInterval>, gap: Option<Gap>, target: &[u8]) -> Observation {
    Observation {
        target_artifact_digest_hex: artifact_digest(target),
        loss: value.map(|v| Interval {
            lower: v.lower().to_string(),
            upper: v.upper().to_string(),
        }),
        gap: gap.map(|v| match v {
            Gap::Unsupported => "unsupported",
            Gap::InvalidTarget => "invalid_target",
            Gap::MissingDecoderCost => "missing_decoder_cost",
        }),
    }
}
fn scope(value: &Scope<'_>) -> Result<ComparisonScope, String> {
    Ok(ComparisonScope {
        id: ScopeId(id(value.id_hex)?),
        baseline: Revision(id(value.baseline_hex)?),
        root_goal: Revision(id(value.root_goal_hex)?),
        probes: Revision(id(value.probes_hex)?),
        policy: Revision(id(value.policy_hex)?),
    })
}
fn namespace(value: &Namespace<'_>) -> Result<RegisteredMap, String> {
    RegisteredMap::identity_namespace(
        Digest32::from_bytes(id(value.id_hex)?),
        Digest32::from_bytes(id(value.revision_hex)?),
    )
    .map_err(|e| format!("path_map:{e:?}"))
}
fn probes(values: &[Probe<'_>]) -> Result<Vec<RootProbeSpec>, String> {
    let mut out = Vec::new();
    out.try_reserve_exact(values.len())
        .map_err(|_| "path_config:Allocation")?;
    for p in values {
        if p.weight_numerator == 0
            || p.weight_denominator == 0
            || p.path.len() > 64
            || p.path.iter().any(|n| *n > 1024)
        {
            return Err("path_config:InvalidProbe".into());
        }
        out.push(RootProbeSpec {
            obligation: ObligationId(id(p.obligation_hex)?),
            property: PropertyId(id(p.property_hex)?),
            hard: p.hard,
            feature: ProbeSpec {
                path: p.path.clone(),
                kind: p.kind.feature(),
                weight: Rational::new(p.weight_numerator.into(), p.weight_denominator.into()),
            },
        });
    }
    Ok(out)
}
fn id(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err("path_config:InvalidId".into());
    }
    let bytes = unhex(value, 32)?;
    bytes.try_into().map_err(|_| "path_config:InvalidId".into())
}
fn unhex(value: &str, max: usize) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2) || value.len() / 2 > max {
        return Err("path_config:InvalidHex".into());
    }
    let mut out = Vec::new();
    out.try_reserve_exact(value.len() / 2)
        .map_err(|_| "path_config:Allocation")?;
    for pair in value.as_bytes().as_chunks::<2>().0 {
        let digit = |b| match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            _ => None,
        };
        let a = digit(pair[0]).ok_or("path_config:InvalidHex")?;
        let b = digit(pair[1]).ok_or("path_config:InvalidHex")?;
        out.push(a * 16 + b);
    }
    Ok(out)
}
fn artifact_digest(bytes: &[u8]) -> String {
    // Hex is generated from actual decoded bytes, not a submitted digest.
    let preimage = format!("epistesys.given-k6-path.artifact.v1\0{}", hex(bytes));
    hex(SourceRevision::of(&preimage).digest().as_bytes())
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        result.push(DIGITS[(b >> 4) as usize] as char);
        result.push(DIGITS[(b & 15) as usize] as char);
    }
    result
}
