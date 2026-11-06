//! 日本語: 固定K6 termの全node/edgeを実ClosedGraphと照合する。
//! English: Match every fixed K6 graph node/edge against actual ClosedGraph.
//! Hashes only bind already checked bytes to the current description owner.
use super::{claim, Error, JoinBinding};
use crate::{
    description_measurement::OwnedDescription,
    structural_gain_measurement::{ClosedGraph, OwnedPreservation},
};
use lc631_analysis::creativity_measurement::{ComponentMetric, ComponentReport};
use lc631_substrate::{
    measurement_numeric::BigInt,
    resource::ResourceReservationView,
    semantic_term::{self, Term, TermLimits, GRAMMAR_REVISION},
    Digest32,
};
use sha2::{Digest, Sha256};

const PROFILE: &[u8] = b"finite-formal-v1.4/closed-graph-K6-current-join.v1\0";

pub(super) fn verify(
    preserve: &OwnedPreservation<'_, '_>,
    description: &OwnedDescription<'_, '_, '_, '_>,
    source_bytes: &[u8],
    target_bytes: &[u8],
    resources: &ResourceReservationView<'_>,
) -> Result<JoinBinding, Error> {
    // All allocating syntax work follows a bounded scratch claim. Full actual
    // graphs (including unprotected edges) are used, never only probe outcomes.
    let scratch = claim(resources, 1024 * 1024)?;
    let p = preserve
        .report_current(resources)
        .map_err(Error::Preservation)?;
    let k = description
        .report_current(resources)
        .map_err(Error::Description)?;
    let lineage = description
        .lineage_current(resources)
        .map_err(Error::Description)?;
    if !preserve.plan.complete_mapping {
        return Err(Error::IncompleteMapping);
    }
    let target = preserve.target.ok_or(Error::MissingTarget)?;
    let source = preserve.plan.source;
    let pins = preserve.binding.pins;
    let scope = lineage.candidate.scope;
    if p.metric != ComponentMetric::Preserve
        || k.metric != ComponentMetric::DescriptionBits
        || source.context != preserve.binding.context
        || target.context != preserve.binding.context
        || p.binding.scope != k.binding.scope
        || p.binding.context != k.binding.context
        || p.binding.baseline != k.binding.baseline
        || source.source.id().digest() != resources.source_revision()
        || k.binding.source != resources.source_revision()
        || scope.id.0 != *pins.scope.comparison().as_bytes()
        || scope.baseline.0 != *pins.baseline.as_bytes()
        || scope.root_goal.0 != *pins.scope.goal().as_bytes()
        || scope.probes.0 != *pins.scope.probes().as_bytes()
        || scope.policy.0 != *pins.scope.semantics().as_bytes()
        || lineage.encoding != digest(GRAMMAR_REVISION.as_bytes())
    {
        return Err(Error::ProfileMismatch);
    }
    check_graph(source, source_bytes)?;
    check_graph(target, target_bytes)?;
    if lineage.source_term != digest(source_bytes)
        || k.binding.target != digest(target_bytes)
        || lineage.candidate.target.0 != *digest(target_bytes).as_bytes()
    {
        return Err(Error::Correspondence);
    }
    // This is a separate join identity. Neither original report is modified.
    let mut h = Sha256::new();
    h.update(PROFILE);
    for bytes in [source_bytes, target_bytes] {
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    }
    hash_report(&mut h, p);
    hash_report(&mut h, k);
    for value in [
        lineage.source_term,
        lineage.encoding,
        lineage.common_runtime,
    ] {
        h.update(value.as_bytes());
    }
    h.update(lineage.baseline_bits.to_le_bytes());
    h.update(lineage.scale_bits.to_le_bytes());
    // Fresh checks after bounded decoding/hash work keep cancellation/expiry
    // visible; historical reports alone never authorize a current read.
    preserve
        .report_current(resources)
        .map_err(Error::Preservation)?;
    description
        .lineage_current(resources)
        .map_err(Error::Description)?;
    scratch.check().map_err(Error::Resource)?;
    Ok(JoinBinding(Digest32::from_bytes(h.finalize().into())))
}

fn check_graph(graph: &ClosedGraph<'_>, bytes: &[u8]) -> Result<(), Error> {
    if graph.nodes > 16 || graph.edges.len() > 64 || bytes.len() > 65_536 {
        return Err(Error::Budget);
    }
    // The checked K6 decoder rejects trailing bytes, noncanonical integers and
    // invalid lengths before materialization. The grammar below is fixed.
    let term = semantic_term::decode(
        bytes,
        TermLimits {
            max_bytes: 65_536,
            max_nodes: 1024,
            max_depth: 64,
        },
    )
    .map_err(Error::Codec)?;
    let Term::Apply(name, fields) = term else {
        return Err(Error::Correspondence);
    };
    if name != "closed-directed-graph-v1" || fields.len() != 2 || !integer(&fields[0], graph.nodes)
    {
        return Err(Error::Correspondence);
    }
    let Term::Apply(name, edges) = &fields[1] else {
        return Err(Error::Correspondence);
    };
    if name != "edges-v1" || edges.len() != graph.edges.len() {
        return Err(Error::Correspondence);
    }
    for (term, edge) in edges.iter().zip(graph.edges) {
        let Term::Apply(name, fields) = term else {
            return Err(Error::Correspondence);
        };
        if name != "edge-v1"
            || fields.len() != 2
            || !integer(&fields[0], edge.0 .0)
            || !integer(&fields[1], edge.1 .0)
        {
            return Err(Error::Correspondence);
        }
    }
    Ok(())
}
fn integer(term: &Term, expected: u16) -> bool {
    matches!(term, Term::Integer(value) if value == &BigInt::from(expected))
}
fn digest(bytes: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(bytes).into())
}
fn hash_report(h: &mut Sha256, report: &ComponentReport) {
    h.update((report.metric as u16).to_le_bytes());
    let b = report.binding;
    for value in [
        report.definition,
        b.scope,
        b.candidate,
        b.source,
        b.target,
        b.context,
        b.baseline,
        b.producer,
        b.transform,
        b.result,
        report.probe_set,
        report.oracle,
        report.dependency_group,
        report.evidence_payload,
    ] {
        h.update(value.as_bytes());
    }
}
