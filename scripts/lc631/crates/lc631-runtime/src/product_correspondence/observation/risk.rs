//! Read actual normalized catalog/response ASTs, not caller replicas or report hashes.
use super::{allocation, ProductError, ProductPlan};
use crate::{adversarial_measurement as risk, product_correspondence::codec};
use lc631_solvers::typed_query::BoolSymbolId;
use lc631_substrate::{resource::ResourceReservationView, semantic_term::Term};

pub(super) fn responses(
    plan: &ProductPlan<'_>,
    frozen: &risk::FrozenResponses<'_, '_>,
    owner: &risk::OwnedCounterRisk<'_, '_>,
    view: &ResourceReservationView<'_>,
) -> Result<Term, ProductError> {
    let term = frozen
        .inspect_product_cases_current(view, |catalog, candidate, _limits, weight, cases| {
            let data = &plan.data;
            if catalog.context != data.context
                || candidate.context != data.context
                || catalog.selection.scope != data.scope
                || catalog.selection.goal_profile != data.goal_profile
                || catalog.selection.source != data.context.source()
                || catalog.purpose != risk::CasePurpose::SoftFiniteValidation
                || catalog.quantification != risk::Quantification::AllAdmissibleAssignments
                || cases.len() != data.cases.len()
            {
                return Err(ProductError::CorrespondenceMismatch);
            }
            let mut slots: Vec<Option<Term>> = Vec::new();
            slots
                .try_reserve_exact(data.cases.len())
                .map_err(|_| allocation())?;
            slots.resize_with(data.cases.len(), || None);
            let mut total = lc631_core::measurement_numeric::Rational::from_integer(0.into());
            for actual in cases {
                // Native catalog order is opaque-key order; fixed slots use semantic-spec order.
                let index = data
                    .cases
                    .iter()
                    .position(|s| s.native_key == actual.key)
                    .ok_or(ProductError::CorrespondenceMismatch)?;
                let fixed = &data.cases[index];
                if slots[index].is_some()
                    || fixed.oracle != catalog.oracle
                    || &fixed.weight != actual.weight
                    || actual.atoms.len() != fixed.native_atoms.len()
                    || actual.normalized_identity_map.len() != fixed.native_atoms.len()
                    || !actual.theory.integer_symbols().is_empty()
                    || actual.theory.boolean_symbols().len() != fixed.native_atoms.len()
                {
                    return Err(ProductError::CorrespondenceMismatch);
                }
                let mut normalized = Vec::new();
                normalized
                    .try_reserve_exact(fixed.native_atoms.len())
                    .map_err(|_| allocation())?;
                for (i, (atom, declared)) in actual.atoms.zip(&fixed.native_atoms).enumerate() {
                    let symbol = BoolSymbolId::new(i as u32); // admitted <=4; no raw-ID cast.
                    if atom.raw_symbol != declared.symbol
                        || atom.meaning != declared.meaning
                        || atom.normalized_symbol != symbol
                        || actual.normalized_identity_map[i] != (symbol, symbol)
                        || actual.theory.boolean_symbols()[i] != symbol
                    {
                        return Err(ProductError::CorrespondenceMismatch);
                    }
                    normalized.push(risk::SemanticAtom {
                        symbol,
                        meaning: atom.meaning,
                    });
                }
                let spec = codec::fields(&fixed.spec, "stage-b-case-spec-v1", 5)?;
                let expected_theory = codec::list(&spec[2], "theory-v1")?;
                if actual.theory.assertions().len() != expected_theory.len() {
                    return Err(ProductError::CorrespondenceMismatch);
                }
                for (a, expected) in actual.theory.assertions().iter().zip(expected_theory) {
                    if codec::boolean_term(a, &normalized, data.limits)? != *expected {
                        return Err(ProductError::CorrespondenceMismatch);
                    }
                }
                if codec::boolean_term(actual.reference, &normalized, data.limits)?
                    != fixed.reference
                {
                    return Err(ProductError::CorrespondenceMismatch);
                }
                let response = codec::boolean_term(actual.response, &normalized, data.limits)?;
                slots[index] = Some(codec::app(
                    "case-slot-v1",
                    vec![codec::int(index as i128), fixed.spec.clone(), response],
                ));
                total = lc631_core::measurement_numeric::checked_add(&total, actual.weight)
                    .map_err(|_| ProductError::UnsupportedProjection)?;
            }
            if &total != weight {
                return Err(ProductError::CorrespondenceMismatch);
            }
            let mut output = Vec::new();
            output
                .try_reserve_exact(slots.len())
                .map_err(|_| allocation())?;
            for slot in slots {
                output.push(slot.ok_or(ProductError::CorrespondenceMismatch)?);
            }
            Ok(codec::app("response-cases-v1", output))
        })
        .map_err(error)??;
    // The existing exact-frozen current check rechecks every actual native
    // witness. Preserve its states and borrowed query/model evidence through
    // existing getters; assignments/visits are not product payload inputs.
    let report = owner.report_current(frozen, view).map_err(error)?;
    if !std::ptr::eq(report, owner.historical_report())
        || owner.historical_trials().len() != plan.data.cases.len()
    {
        return Err(ProductError::CorrespondenceMismatch);
    }
    for (i, state) in owner.historical_trials().iter().enumerate() {
        match (
            owner.counterrow(i),
            owner.witness_query(i),
            owner.witness_model(i),
        ) {
            (Some(_), Some(_), Some(_)) if *state == risk::TrialState::Failure => (),
            (None, None, None) if *state != risk::TrialState::Failure => (),
            _ => return Err(ProductError::CorrespondenceMismatch),
        }
    }
    owner.report_current(frozen, view).map_err(error)?;
    Ok(term)
}
pub(super) fn error(error: risk::Error) -> ProductError {
    match error {
        risk::Error::Resource(e) => ProductError::Resource(e),
        risk::Error::AllocationFailed => allocation(),
        _ => ProductError::CorrespondenceMismatch,
    }
}
