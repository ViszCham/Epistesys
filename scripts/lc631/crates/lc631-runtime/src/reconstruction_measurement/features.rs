use super::{Error, ProtectedFeatureSet, Rational, RationalInterval};
use lc631_analysis::creativity_measurement::{weighted_assessment, WeightedProbe};
use lc631_substrate::semantic_term::{BinderKind, Term};
pub(super) const CODE: &str = include_str!("features.rs");
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureGap {
    ExtractionBudget,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureOutcome {
    Preserved,
    Violated,
    Unknown(FeatureGap),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureKind {
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
#[derive(Clone, Debug)]
pub struct ProbeSpec {
    pub path: Vec<usize>,
    pub kind: FeatureKind,
    pub weight: Rational,
}
/// Fixed K1 with the existing bounded extractor. Only its explicit per-probe
/// budget gap becomes [0,1]. Source capture stays strict; run resource failures,
/// backing allocation failure and numeric failure do not become observations.
pub(super) fn measure(
    plan: &ProtectedFeatureSet<'_>,
    target: &Term,
) -> Result<(Vec<FeatureOutcome>, RationalInterval), Error> {
    let mut outcomes = Vec::new();
    let mut values = Vec::new();
    let mut probes = Vec::new();
    outcomes.try_reserve_exact(plan.features.len()).map_err(|_| Error::Budget)?;
    values.try_reserve_exact(plan.features.len()).map_err(|_| Error::Budget)?;
    probes.try_reserve_exact(plan.features.len()).map_err(|_| Error::Budget)?;
    for feature in &plan.features {
        let outcome = match extract(target, &feature.spec) {
            Ok(actual) if actual.as_ref() == Some(&feature.expected) => FeatureOutcome::Preserved,
            Ok(_) => FeatureOutcome::Violated,
            Err(Error::Budget) => FeatureOutcome::Unknown(FeatureGap::ExtractionBudget),
            Err(error) => return Err(error),
        };
        let (lower, upper) = match outcome {
            FeatureOutcome::Preserved => (0, 0),
            FeatureOutcome::Violated => (1, 1),
            FeatureOutcome::Unknown(FeatureGap::ExtractionBudget) => (0, 1),
        };
        values.push(RationalInterval::from_ratio(lower, 1, upper, 1).map_err(|_| Error::Arithmetic)?);
        outcomes.push(outcome);
    }
    for (feature, value) in plan.features.iter().zip(&values) {
        probes.push(WeightedProbe {
            canonical_payload: &feature.canonical,
            weight: &feature.spec.weight,
            value,
        });
    }
    let bounds = weighted_assessment(&probes).map_err(|_| Error::Arithmetic)?;
    Ok((outcomes, bounds))
}
pub(super) fn canonical(spec: &ProbeSpec) -> Vec<u8> {
    let mut out = Vec::with_capacity(10 + spec.path.len() * 8);
    out.push(spec.kind as u8);
    out.extend_from_slice(&(spec.path.len() as u64).to_le_bytes());
    for index in &spec.path {
        out.extend_from_slice(&(*index as u64).to_le_bytes());
    }
    out
}
pub(super) fn extract(term: &Term, spec: &ProbeSpec) -> Result<Option<Vec<u8>>, Error> {
    let mut node = term;
    for index in &spec.path {
        node = match node {
            Term::Apply(_, args) => match args.get(*index) {
                Some(n) => n,
                None => return Ok(None),
            },
            Term::Binder(_, _, body) if *index == 0 => body,
            _ => return Ok(None),
        };
    }
    let value = match (spec.kind, node) {
        (FeatureKind::NodeKind, t) => vec![match t {
            Term::Bool(_) => 1,
            Term::Integer(_) => 2,
            Term::Symbol(_) => 3,
            Term::Apply(..) => 4,
            Term::Binder(..) => 5,
            Term::BoundVar(_) => 6,
        }],
        (FeatureKind::Symbol, Term::Symbol(s))
        | (FeatureKind::Operator, Term::Apply(s, _))
        | (FeatureKind::Sort, Term::Binder(_, s, _)) => {
            if s.len() > 4096 {
                return Err(Error::Budget);
            }
            s.as_bytes().to_vec()
        }
        (FeatureKind::Boolean, Term::Bool(b)) => vec![u8::from(*b)],
        (FeatureKind::Integer, Term::Integer(n)) => {
            if n.bits() > 8192 {
                return Err(Error::Budget);
            }
            n.to_string().into_bytes()
        }
        (FeatureKind::Arity, Term::Apply(_, args)) => (args.len() as u64).to_le_bytes().to_vec(),
        (FeatureKind::Binder, Term::Binder(k, _, _)) => vec![match k {
            BinderKind::Forall => 0,
            BinderKind::Exists => 1,
            BinderKind::Lambda => 2,
        }],
        (FeatureKind::BoundIndex, Term::BoundVar(i)) => (*i as u64).to_le_bytes().to_vec(),
        _ => return Ok(None),
    };
    Ok(Some(value))
}
