//! Fixed K6 application grammar, not a source-language or equivalence oracle.
pub(crate) mod boolean;
pub(crate) mod cases;
pub(crate) mod roles;
mod spatial;
mod temporal;
use super::{digest, PlanData, ProductError, ProductInput, ProductLimits};
pub(crate) use boolean::boolean_term;
use lc631_substrate::{
    measurement_numeric::BigInt,
    resource::{ResourceError, ResourceKind},
    semantic_term::{self, Term, TermError},
    Digest32,
};
pub(crate) use spatial::project as spatial_term;
pub(crate) use temporal::project as temporal_term;

pub(crate) fn sym(value: &str) -> Term {
    Term::Symbol(value.to_owned())
}
pub(crate) fn app(name: &str, fields: Vec<Term>) -> Term {
    Term::Apply(name.to_owned(), fields)
}
pub(crate) fn int(value: i128) -> Term {
    Term::Integer(BigInt::from(value))
}
pub(crate) fn fields<'a>(
    term: &'a Term,
    name: &str,
    count: usize,
) -> Result<&'a [Term], ProductError> {
    match term {
        Term::Apply(actual, fields) if actual == name && fields.len() == count => Ok(fields),
        _ => Err(ProductError::UnsupportedProjection),
    }
}
pub(crate) fn list<'a>(term: &'a Term, name: &str) -> Result<&'a [Term], ProductError> {
    match term {
        Term::Apply(actual, fields) if actual == name => Ok(fields),
        _ => Err(ProductError::UnsupportedProjection),
    }
}
pub(crate) fn integer(term: &Term) -> Result<i128, ProductError> {
    match term {
        Term::Integer(v) => {
            i128::try_from(v.clone()).map_err(|_| ProductError::UnsupportedProjection)
        }
        _ => Err(ProductError::UnsupportedProjection),
    }
}
fn codec_error(error: TermError) -> ProductError {
    match error {
        TermError::InvalidInput => ProductError::InvalidCanonicalEncoding,
        TermError::Unsupported => ProductError::UnsupportedProjection,
        TermError::Budget => ProductError::Resource(ResourceError::Exhausted(ResourceKind::Ram)),
        TermError::Allocation => ProductError::Resource(ResourceError::AllocationFailed),
    }
}
pub(crate) fn decode(bytes: &[u8], limits: ProductLimits) -> Result<Term, ProductError> {
    if bytes.len() > limits.max_bytes {
        return Err(ProductError::UnsupportedProjection);
    }
    semantic_term::decode(bytes, limits.term()).map_err(codec_error)
}
pub(crate) fn encode(term: &Term, limits: ProductLimits) -> Result<Vec<u8>, ProductError> {
    semantic_term::encode(term, limits.term()).map_err(codec_error)
}

pub(crate) fn project(data: &PlanData, input: ProductInput<'_>) -> Result<Term, ProductError> {
    let response = cases::responses(data, input.responses)?;
    let temporal = temporal_term(data, input.temporal)?;
    let spatial = spatial_term(data, input.spatial)?;
    Ok(app(
        "tlcx-finite-product-v1",
        vec![response, temporal, spatial],
    ))
}
pub(crate) fn validate(data: &PlanData, term: &Term) -> Result<(), ProductError> {
    let parts = fields(term, "tlcx-finite-product-v1", 3)?;
    cases::validate_responses(data, &parts[0])?;
    temporal::validate(data, &parts[1])?;
    spatial::validate(data, &parts[2])
}
pub(crate) fn identity_envelope(
    data: &PlanData,
    temporal: crate::temporal_preservation::TemporalModelInput<'_>,
    spatial: crate::spatial_preservation::SpatialModelInput,
) -> Result<Vec<u8>, ProductError> {
    let _ = temporal_term(data, temporal)?;
    let _ = spatial_term(data, spatial)?;
    roles::envelope(data)
}

/// Canonical typed metadata binds raw identity/provenance separately from K.
pub(crate) fn plan_binding(data: &PlanData) -> Result<Digest32, ProductError> {
    let mut h = sha2::Sha256::new();
    use sha2::Digest;
    h.update(b"epistesys.stage-b.correspondence-plan.v1\0");
    for d in [
        data.context.source().digest(),
        data.context.interpretation_digest(),
        data.scope.comparison(),
        data.scope.goal(),
        data.scope.probes(),
        data.scope.semantics(),
    ] {
        h.update(d.as_bytes());
    }
    match data.goal_profile {
        Some(d) => {
            h.update([1]);
            h.update(d.as_bytes());
        }
        None => h.update([0]),
    }
    h.update(b"Given;TemporalSpatial1To3;FullCostOwnerMissing");
    let l = data.limits;
    for value in [
        l.max_bytes,
        l.max_nodes,
        l.max_depth,
        l.max_cases,
        l.max_atoms,
        l.max_response_nodes,
        l.max_temporal_nodes,
        l.max_trace_steps,
        l.max_temporal_atoms,
    ] {
        h.update((value as u64).to_le_bytes());
    }
    let field = |h: &mut sha2::Sha256, bytes: &[u8]| {
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    };
    h.update((data.cases.len() as u16).to_le_bytes());
    for case in &data.cases {
        field(&mut h, &case.native_key);
        field(&mut h, &case.semantic_spec);
        field(&mut h, &case.reference_contract);
        h.update(case.oracle.as_bytes());
        field(
            &mut h,
            &encode(
                &app(
                    "weight-v1",
                    vec![
                        Term::Integer(case.weight.numer().clone()),
                        Term::Integer(case.weight.denom().clone()),
                    ],
                ),
                l,
            )?,
        );
        h.update((case.native_atoms.len() as u16).to_le_bytes());
        for atom in &case.native_atoms {
            h.update(atom.symbol.get().to_le_bytes());
            field(&mut h, &atom.meaning);
        }
    }
    h.update((data.identities.len() as u16).to_le_bytes());
    for identity in &data.identities {
        h.update([roles::kind_tag(identity.kind)]);
        h.update(identity.local_role.to_le_bytes());
        h.update(identity.native_id);
        field(&mut h, &identity.meaning_bytes);
    }
    h.update((data.temporal_atom_meanings.len() as u16).to_le_bytes());
    for meaning in &data.temporal_atom_meanings {
        field(&mut h, &encode(meaning, l)?);
    }
    // This digest is a content binding, never a proof or an external receipt.
    Ok(digest(
        b"epistesys.stage-b.plan-binding-final.v1\0",
        &h.finalize(),
    ))
}
