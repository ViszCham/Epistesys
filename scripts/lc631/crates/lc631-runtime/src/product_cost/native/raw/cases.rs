//! Native catalog order, actual normalized AST and retained raw mapping.
use super::*;
use crate::adversarial_measurement as risk;
use lc631_solvers::typed_query::BoolSymbolId;
pub(in crate::product_cost::native) struct Cases {
    pub(in crate::product_cost::native) term: Term,
    pub(in crate::product_cost::native) declarations: Vec<Term>,
    pub(in crate::product_cost::native) binding: risk::CatalogBinding,
}
pub(in crate::product_cost::native) fn collect_cases(
    plan: &ProductPlan<'_>,
    frozen: &risk::FrozenResponses<'_, '_>,
    owner: &risk::OwnedCounterRisk<'_, '_>,
    r: &Registry,
    view: &ResourceReservationView<'_>,
) -> Result<Cases, CostError> {
    owner.report_current(frozen, view).map_err(errors::risk)?;
    let collected = frozen
        .inspect_product_cases_current(view, |catalog, candidate, _limits, total, cases| {
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
                return Err(CostError::CorrespondenceMismatch);
            }
            let mut seen = vec![false; data.cases.len()];
            let mut records = Vec::new();
            let mut declarations = Vec::new();
            let mut sum = lc631_core::measurement_numeric::Rational::from_integer(0.into());
            for actual in cases {
                let index = data
                    .cases
                    .iter()
                    .position(|c| c.native_key == actual.key)
                    .ok_or(CostError::CorrespondenceMismatch)?;
                let fixed = &data.cases[index];
                if seen[index]
                    || fixed.oracle != catalog.oracle
                    || &fixed.weight != actual.weight
                    || actual.atoms.len() != fixed.native_atoms.len()
                    || actual.normalized_identity_map.len() != fixed.native_atoms.len()
                    || !actual.theory.integer_symbols().is_empty()
                    || actual.theory.boolean_symbols().len() != fixed.native_atoms.len()
                {
                    return Err(CostError::CorrespondenceMismatch);
                }
                seen[index] = true;
                let mut atoms = Vec::new();
                for (i, atom) in actual.atoms.enumerate() {
                    let symbol = BoolSymbolId::new(
                        u32::try_from(i).map_err(|_| CostError::ResourceExhausted)?,
                    );
                    let declared = fixed
                        .native_atoms
                        .iter()
                        .find(|a| a.symbol == atom.raw_symbol)
                        .ok_or(CostError::CorrespondenceMismatch)?;
                    if atom.meaning != declared.meaning
                        || atom.normalized_symbol != symbol
                        || actual.normalized_identity_map[i] != (symbol, symbol)
                        || actual.theory.boolean_symbols()[i] != symbol
                    {
                        return Err(CostError::CorrespondenceMismatch);
                    }
                    let meaning = std::str::from_utf8(atom.meaning)
                        .map_err(|_| CostError::UnsupportedProfile)?;
                    atoms.push(app(
                        "atom-fields-v1",
                        vec![
                            r.symbol(actual.key, atom.raw_symbol.get())?,
                            int(i as i128),
                            sym(meaning),
                        ],
                    ));
                    declarations.push(declaration(
                        "BooleanAtom",
                        &format!("Case/{index}/Atom/{i}"),
                        sym(meaning),
                    ));
                }
                let mut theory = Vec::new();
                for assertion in actual.theory.assertions() {
                    theory.push(boolean::flatten(assertion, data.limits.max_response_nodes)?);
                }
                declarations.push(declaration(
                    "CaseSpec",
                    &format!("Case/{index}/Spec"),
                    fixed.spec.clone(),
                ));
                // Full Given reference is validated against actual normalized reference by charged A.
                declarations.push(declaration(
                    "CaseReference",
                    &format!("Case/{index}/Reference"),
                    fixed.reference.clone(),
                ));
                records.push(app(
                    "case-fields-v1",
                    vec![
                        r.existing("Case", actual.key)?,
                        app("atoms-fields-v1", atoms),
                        app("theory-fields-v1", theory),
                        boolean::flatten(actual.reference, data.limits.max_response_nodes)?,
                        boolean::flatten(actual.response, data.limits.max_response_nodes)?,
                        sym("SoftFiniteValidation"),
                        sym("AllAdmissibleAssignments"),
                        fixed.spec.clone(),
                    ],
                ));
                sum = lc631_core::measurement_numeric::checked_add(&sum, actual.weight)
                    .map_err(|_| CostError::ResourceExhausted)?;
            }
            if seen.contains(&false) || &sum != total {
                return Err(CostError::CorrespondenceMismatch);
            }
            Ok(Cases {
                term: app("cases-v1", records),
                declarations,
                binding: catalog,
            })
        })
        .map_err(errors::risk)??;
    // Exact FrozenResponses and all existing native witness/query/model checks.
    owner.report_current(frozen, view).map_err(errors::risk)?;
    Ok(collected)
}
