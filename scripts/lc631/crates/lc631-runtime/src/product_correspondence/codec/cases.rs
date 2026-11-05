//! Freeze semantic case slots before response attachment; no weight/ID ranking.
use super::{app, boolean, decode, fields, int, list, sym};
use crate::{
    adversarial_measurement::{Response, SemanticAtom},
    product_correspondence::{
        copy, reserved, CaseDeclaration, FixedCaseSlot, OwnedAtom, PlanData, PlanSpec,
        ProductError, ProductLimits,
    },
};
use lc631_core::measurement_numeric as numeric;
use lc631_substrate::semantic_term::Term;

pub(crate) fn preflight(spec: &PlanSpec<'_>, limits: ProductLimits) -> Result<(), ProductError> {
    if spec.cases.is_empty()
        || spec.cases.len() > limits.max_cases
        || spec.temporal_atom_meanings.len() > limits.max_temporal_atoms
    {
        return Err(ProductError::UnsupportedProjection);
    }
    let mut bytes = 0usize;
    for case in spec.cases {
        if case.native_key.is_empty()
            || case.native_key.len() > 256
            || case.native_atoms.len() > limits.max_atoms
        {
            return Err(ProductError::UnsupportedProjection);
        }
        for chunk in [case.native_key, case.semantic_spec, case.reference_contract] {
            bytes = bytes
                .checked_add(chunk.len())
                .ok_or(ProductError::UnsupportedProjection)?;
        }
        // Cheap size/sign admission only. Canonical GCD checks allocate and
        // therefore run in capture, after the owning factory's claims.
        if case.weight.numer().bits() > 1024 || case.weight.denom().bits() > 1024 {
            return Err(ProductError::UnsupportedProjection);
        }
        let zero = numeric::BigInt::from(0);
        if case.weight.numer() <= &zero || case.weight.denom() <= &zero {
            return Err(ProductError::UnsupportedProjection);
        }
        for (i, atom) in case.native_atoms.iter().enumerate() {
            if atom.meaning.is_empty()
                || atom.meaning.len() > 256
                || std::str::from_utf8(atom.meaning).is_err()
                || case.native_atoms[..i]
                    .iter()
                    .any(|a| a.symbol == atom.symbol || a.meaning == atom.meaning)
            {
                return Err(ProductError::UnsupportedProjection);
            }
            bytes = bytes
                .checked_add(atom.meaning.len())
                .ok_or(ProductError::UnsupportedProjection)?;
        }
    }
    for meaning in spec.temporal_atom_meanings {
        bytes = bytes
            .checked_add(meaning.len())
            .ok_or(ProductError::UnsupportedProjection)?;
    }
    if bytes > limits.max_bytes {
        return Err(ProductError::UnsupportedProjection);
    }
    Ok(())
}
pub(crate) fn capture(
    inputs: &[CaseDeclaration<'_>],
    limits: ProductLimits,
) -> Result<Vec<FixedCaseSlot>, ProductError> {
    let mut out: Vec<FixedCaseSlot> = reserved(inputs.len())?;
    for input in inputs {
        numeric::check(input.weight).map_err(|_| ProductError::UnsupportedProjection)?;
        let spec = decode(input.semantic_spec, limits)?;
        let f = fields(&spec, "stage-b-case-spec-v1", 5)?;
        if f[0] != sym("BooleanReferenceAgreement")
            || f[3] != sym("SoftFiniteValidation")
            || f[4] != sym("AllAdmissibleAssignments")
        {
            return Err(ProductError::UnsupportedProjection);
        }
        let mut atoms: Vec<OwnedAtom> = reserved(input.native_atoms.len())?;
        for atom in input.native_atoms {
            atoms.push(OwnedAtom {
                symbol: atom.symbol,
                meaning: copy(atom.meaning)?,
            });
        }
        atoms.sort_by(|a, b| a.meaning.cmp(&b.meaning));
        let meanings: Vec<_> = atoms.iter().map(|a| a.meaning.as_slice()).collect();
        let declared = list(&f[1], "atoms-v1")?;
        if declared.len() != meanings.len()
            || declared
                .iter()
                .zip(&meanings)
                .any(|(t, m)| !matches!(t,Term::Symbol(s) if s.as_bytes() == *m))
        {
            return Err(ProductError::CorrespondenceMismatch);
        }
        let theory = list(&f[2], "theory-v1")?;
        if theory.len() > limits.max_response_nodes {
            return Err(ProductError::UnsupportedProjection);
        }
        for term in theory {
            boolean::validate(term, &meanings, limits)?;
        }
        let reference = decode(input.reference_contract, limits)?;
        boolean::validate(&reference, &meanings, limits)?;
        // Key equality excludes responses, reference/oracle/weight and opaque IDs.
        if out.iter().any(|c| c.spec == spec) {
            return Err(ProductError::AmbiguousCaseSpec);
        }
        if out
            .iter()
            .any(|c| c.native_key.as_slice() == input.native_key)
        {
            return Err(ProductError::CorrespondenceMismatch);
        }
        out.push(FixedCaseSlot {
            native_key: copy(input.native_key)?,
            native_atoms: atoms,
            semantic_spec: copy(input.semantic_spec)?,
            spec,
            reference_contract: copy(input.reference_contract)?,
            reference,
            oracle: input.oracle,
            weight: input.weight.clone(),
        });
    }
    out.sort_by(|a, b| a.semantic_spec.cmp(&b.semantic_spec));
    Ok(out)
}
pub(crate) fn responses(data: &PlanData, responses: &[Response<'_>]) -> Result<Term, ProductError> {
    if responses.len() != data.cases.len()
        || responses
            .iter()
            .enumerate()
            .any(|(i, r)| responses[..i].iter().any(|p| p.case == r.case))
    {
        return Err(ProductError::CorrespondenceMismatch);
    }
    let mut slots = reserved(data.cases.len())?;
    for (i, case) in data.cases.iter().enumerate() {
        let response = responses
            .iter()
            .find(|r| r.case == case.native_key.as_slice())
            .ok_or(ProductError::CorrespondenceMismatch)?;
        let atoms: Vec<_> = case
            .native_atoms
            .iter()
            .map(|a| SemanticAtom {
                symbol: a.symbol,
                meaning: &a.meaning,
            })
            .collect();
        let term = boolean::boolean_term(response.term, &atoms, data.limits)?;
        slots.push(app(
            "case-slot-v1",
            vec![int(i as i128), case.spec.clone(), term],
        ));
    }
    Ok(app("response-cases-v1", slots))
}
pub(crate) fn validate_responses(data: &PlanData, term: &Term) -> Result<(), ProductError> {
    let slots = list(term, "response-cases-v1")?;
    if slots.len() != data.cases.len() {
        return Err(ProductError::UnsupportedProjection);
    }
    for (i, (slot, case)) in slots.iter().zip(&data.cases).enumerate() {
        let f = fields(slot, "case-slot-v1", 3)?;
        if f[0] != int(i as i128) || f[1] != case.spec {
            return Err(ProductError::CorrespondenceMismatch);
        }
        let meanings: Vec<_> = case
            .native_atoms
            .iter()
            .map(|a| a.meaning.as_slice())
            .collect();
        boolean::validate(&f[2], &meanings, data.limits)?;
    }
    Ok(())
}
