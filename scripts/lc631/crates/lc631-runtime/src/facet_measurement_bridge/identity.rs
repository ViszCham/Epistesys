//! Typed exact-input identities; no Debug/JSON encoding or evidence promotion.
use super::{f, m, IngressError};
use crate::facet_ledger as l;
use lc631_analysis::creativity_measurement::{Rational, RationalInterval};
use lc631_solvers::typed_query::{BoolExpr, SmtStatusQuery};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

pub(super) fn revision(digest: Digest32) -> l::Revision {
    l::Revision(*digest.as_bytes())
}
pub(super) fn digest(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    for part in parts {
        field(&mut hash, part);
    }
    hash.finalize().into()
}
fn field(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
fn rational(hash: &mut Sha256, value: &Rational) {
    field(hash, &value.numer().to_signed_bytes_le());
    field(hash, &value.denom().to_signed_bytes_le());
}
fn interval(hash: &mut Sha256, value: &RationalInterval) {
    rational(hash, value.lower());
    rational(hash, value.upper());
}

pub(super) fn query_digest(measured: &m::MeasuredFormalLoss<'_>) -> Result<[u8; 32], IngressError> {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.facet-ingress.exact-query.v1\0");
    let mut visits = 0_u64;
    let source = measured.plan().fixed_probes()[0].template.source;
    theory(&mut hash, source, &mut visits)?;
    theory(&mut hash, measured.candidate().target_query(), &mut visits)?;
    for probe in measured.plan().fixed_probes() {
        field(&mut hash, probe.identity);
        hash.update(probe.template.binding.probe_map.as_bytes());
        rational(&mut hash, probe.weight);
    }
    hash.update([direction(measured.direction())]);
    Ok(hash.finalize().into())
}

pub(super) fn measurement_digest(
    measured: &m::MeasuredFormalLoss<'_>,
) -> Result<[u8; 32], IngressError> {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.facet-ingress.measured-k1.v1\0");
    let scope = measured.plan().scope();
    for pin in [
        scope.comparison,
        scope.definition,
        scope.probe_set,
        scope.source.digest(),
        scope.context.interpretation_digest(),
        measured.candidate().id(),
        measured.candidate().revision().digest(),
        measured.candidate().context().interpretation_digest(),
    ] {
        hash.update(pin.as_bytes());
    }
    hash.update(query_digest(measured)?);
    interval(&mut hash, measured.bounds());
    for probe in measured.probes() {
        field(&mut hash, probe.fixed().identity);
        match probe.state() {
            m::ProbeState::Preserved => hash.update([0]),
            m::ProbeState::Countermodel(row) => {
                hash.update([1]);
                hash.update(row.to_le_bytes());
            }
            m::ProbeState::Gap(gap) => hash.update([2, gap_tag(gap)]),
        }
        if let Some(m::ProbeObservation {
            value: m::ObservationValue::Computed(report),
            ..
        }) = probe.observation()
        {
            hash.update([1]);
            hash.update(report.source_theory_digest().as_bytes());
            hash.update(report.target_theory_digest().as_bytes());
            hash.update(report.assignments().to_le_bytes());
            hash.update(report.ast_visits().to_le_bytes());
        } else {
            hash.update([0]);
        }
    }
    Ok(hash.finalize().into())
}
fn direction(value: m::Direction) -> u8 {
    match value {
        m::Direction::Soundness => 0,
        m::Direction::Completeness => 1,
    }
}
fn gap_tag(gap: m::Gap) -> u8 {
    match gap {
        m::Gap::Missing => 0,
        m::Gap::Unknown => 1,
        m::Gap::NoComparable(f::NoComparable::MissingTheoryScope) => 2,
        m::Gap::NoComparable(f::NoComparable::SourceContextInconsistent) => 3,
        m::Gap::NoComparable(f::NoComparable::TargetContextInconsistent) => 4,
        m::Gap::Unsupported(failure) => {
            16 + match failure {
                f::Failure::InvalidBudget => 0,
                f::Failure::AtomLimit => 1,
                f::Failure::IncompleteMap => 2,
                f::Failure::DuplicateMap => 3,
                f::Failure::SourceBindingMismatch => 4,
                f::Failure::ProbeMapMismatch => 5,
                f::Failure::UnsupportedInteger => 6,
                f::Failure::DepthLimit => 7,
                f::Failure::UndeclaredSymbol => 8,
                f::Failure::EmptyConnective => 9,
                f::Failure::ResourceExhausted => 10,
            }
        }
    }
}

pub(super) fn validate_computed(
    measured: &m::MeasuredFormalLoss<'_>,
    probe: &m::ProbeLineage<'_>,
) -> Result<(), IngressError> {
    let report = match probe.observation().map(|observation| &observation.value) {
        None if probe.state() == m::ProbeState::Gap(m::Gap::Missing) => return Ok(()),
        Some(m::ObservationValue::Gap(gap)) if probe.state() == m::ProbeState::Gap(*gap) => {
            return Ok(())
        }
        Some(m::ObservationValue::Computed(report)) => report,
        _ => return Err(IngressError::ProducerMismatch),
    };
    let request = report.request();
    let fixed = probe.fixed().template;
    let scope = measured.plan().scope();
    let state = match measured.direction() {
        m::Direction::Soundness => report.soundness(),
        m::Direction::Completeness => report.completeness(),
    };
    let expected = match state {
        f::DirectionResult::Preserved => m::ProbeState::Preserved,
        f::DirectionResult::Countermodel(row) => m::ProbeState::Countermodel(row),
    };
    if expected != probe.state()
        || !std::ptr::eq(request.source, fixed.source)
        || !std::ptr::eq(request.target, measured.candidate().target_query())
        || !std::ptr::eq(request.source_predicate, fixed.source_predicate)
        || !std::ptr::eq(request.target_predicate, fixed.target_predicate)
        || request.symbols != fixed.symbols
        || request.unconstrained != fixed.unconstrained
        || request.binding.source != scope.source
        || request.binding.source_context != scope.context
        || request.binding.target != measured.candidate().revision()
        || request.binding.target_context != measured.candidate().context()
        || request.binding.probe_set != scope.probe_set
        || request.binding.probe_map != fixed.binding.probe_map
    {
        return Err(IngressError::ProducerMismatch);
    }
    Ok(())
}

fn theory(hash: &mut Sha256, query: &SmtStatusQuery, visits: &mut u64) -> Result<(), IngressError> {
    if !query.integer_symbols().is_empty() {
        return Err(IngressError::ProducerMismatch);
    }
    hash.update((query.boolean_symbols().len() as u64).to_le_bytes());
    for symbol in query.boolean_symbols() {
        hash.update(symbol.get().to_le_bytes());
    }
    hash.update((query.assertions().len() as u64).to_le_bytes());
    for assertion in query.assertions() {
        expression(hash, assertion, 1, visits)?;
    }
    Ok(())
}
fn expression(
    hash: &mut Sha256,
    expr: &BoolExpr,
    depth: u16,
    visits: &mut u64,
) -> Result<(), IngressError> {
    if depth > 128 || *visits >= 1_000_000 {
        return Err(IngressError::Capacity);
    }
    *visits += 1;
    match expr {
        BoolExpr::Constant(value) => hash.update([0, u8::from(*value)]),
        BoolExpr::Variable(symbol) => {
            hash.update([1]);
            hash.update(symbol.get().to_le_bytes());
        }
        BoolExpr::Not(child) => {
            hash.update([2]);
            expression(hash, child, depth + 1, visits)?;
        }
        BoolExpr::And(children) | BoolExpr::Or(children) => {
            hash.update([if matches!(expr, BoolExpr::And(_)) {
                3
            } else {
                4
            }]);
            hash.update((children.len() as u64).to_le_bytes());
            for child in children {
                expression(hash, child, depth + 1, visits)?;
            }
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            return Err(IngressError::ProducerMismatch)
        }
    }
    Ok(())
}
