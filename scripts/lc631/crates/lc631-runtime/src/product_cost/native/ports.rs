//! Exact native current inputs and metadata; no reported-product lowering.
use super::*;
use crate::{adversarial_measurement as risk, description_measurement as desc};
pub(super) fn description(
    plan: &ProductPlan<'_>,
    baseline: &ReportedProduct<'_>,
    expected: &ReportedProduct<'_>,
    owner: &desc::OwnedDescription<'_, '_, '_, '_>,
    native_baseline: Digest32,
    view: &ResourceReservationView<'_>,
) -> Result<(), CostError> {
    owner
        .inspect_product_bytes_current(view, |source, actual, context, lineage, report| {
            if source != baseline.semantic_bytes()
                || actual != expected.semantic_bytes()
                || context != plan.data.context
                || report.binding.context != context.interpretation_digest()
            {
                return Err(CostError::CorrespondenceMismatch);
            }
            for scope in [lineage.baseline.scope, lineage.candidate.scope] {
                if scope.id.0 != *plan.data.scope.comparison().as_bytes()
                    || scope.root_goal.0 != *plan.data.scope.goal().as_bytes()
                    || scope.probes.0 != *plan.data.scope.probes().as_bytes()
                    || scope.policy.0 != *plan.data.scope.semantics().as_bytes()
                    || scope.baseline.0 != *native_baseline.as_bytes()
                {
                    return Err(CostError::CorrespondenceMismatch);
                }
            }
            Ok(())
        })
        .map_err(errors::description)?
}
pub(super) struct Collected {
    pub(super) fields: Term,
    pub(super) declarations: Term,
    pub(super) catalog: risk::CatalogBinding,
    pub(super) root: Digest32,
    pub(super) admission_work: u64,
}
pub(super) fn baseline(
    plan: &ProductPlan<'_>,
    reported: &ReportedProduct<'_>,
    ports: BaselinePorts<'_, '_>,
    view: &ResourceReservationView<'_>,
) -> Result<Collected, CostError> {
    plan.check_current(view).map_err(errors::product)?;
    reported
        .check_current(plan, view)
        .map_err(errors::product)?;
    let registry = raw::Registry::declared(&plan.data)?;
    let cases = raw::collect_cases(plan, ports.responses, ports.risk, &registry, view)?;
    description(
        plan,
        reported,
        reported,
        ports.description,
        cases.binding.selection.baseline,
        view,
    )?;
    // CENTRAL pending additive source-only visitor. Never use candidate/reduction
    // current here: target ports cannot become K0 dependencies.
    ports
        .root
        .inspect_product_source_current(view, |time, space, _time_policy, _space_policy, root| {
            let temporal = raw::temporal(&plan.data, time, &registry)?;
            let spatial = raw::spatial(&plan.data, space, &registry)?;
            let envelope = crate::product_correspondence::codec::roles::envelope(&plan.data)
                .map_err(errors::product)?;
            if envelope != reported.identity_envelope_bytes() {
                return Err(CostError::CorrespondenceMismatch);
            }
            assemble(plan, &registry, cases, temporal, spatial, root)
        })
        .map_err(errors::tl)?
}
pub(super) fn candidate(
    input: &NativeProductInput<'_, '_>,
    source_catalog: risk::CatalogBinding,
    view: &ResourceReservationView<'_>,
) -> Result<Collected, CostError> {
    input.plan.check_current(view).map_err(errors::product)?;
    input
        .target
        .check_current(input.plan, view)
        .map_err(errors::product)?;
    let registry = raw::Registry::declared(&input.plan.data)?;
    let cases = raw::collect_cases(
        input.plan,
        input.owners.target_responses,
        input.owners.target_risk,
        &registry,
        view,
    )?;
    if cases.binding != source_catalog {
        return Err(CostError::CorrespondenceMismatch);
    }
    desc::compare_current(
        input.owners.baseline_description,
        input.owners.target_description,
        view,
    )
    .map_err(errors::description)?;
    description(
        input.plan,
        input.baseline,
        input.target,
        input.owners.target_description,
        source_catalog.selection.baseline,
        view,
    )?;
    input
        .owners
        .tl_reduction
        .inspect_product_tuple_current(
            input.owners.tl_root,
            input.owners.tl_candidate,
            view,
            |actual| {
                if actual.report.denominator() != 4
                    || actual.report.profile() != crate::checked_tl_reduction::PROFILE
                    || actual.root_digest != input.owners.tl_root.digest()
                {
                    return Err(CostError::CorrespondenceMismatch);
                }
                let temporal = raw::temporal(&input.plan.data, actual.target_temporal, &registry)?;
                let spatial = raw::spatial(&input.plan.data, actual.target_spatial, &registry)?;
                // Exact producer current guard validates root probes/map; additionally read
                // full owner inputs and prove they are the actual tuple inputs, without lowering S.
                actual
                    .temporal_owner
                    .ok_or(CostError::UnsupportedProfile)?
                    .inspect_product_current(view, |source, target, policy, report| {
                        if raw::temporal(&input.plan.data, source, &registry)?
                            != raw::temporal(&input.plan.data, actual.source_temporal, &registry)?
                            || raw::temporal(&input.plan.data, target, &registry)? != temporal
                            || policy.digest() != report.policy_digest()
                            || policy.binding_digest() != report.binding_digest()
                            || policy.probes() != actual.source_temporal_policy.probes()
                            || policy.mapping() != actual.source_temporal_policy.mapping()
                        {
                            return Err(CostError::CorrespondenceMismatch);
                        }
                        Ok(())
                    })
                    .map_err(errors::selection)??;
                actual
                    .spatial_owner
                    .ok_or(CostError::UnsupportedProfile)?
                    .inspect_product_current(view, |source, target, policy, report| {
                        if raw::spatial(&input.plan.data, source, &registry)?
                            != raw::spatial(&input.plan.data, actual.source_spatial, &registry)?
                            || raw::spatial(&input.plan.data, target, &registry)? != spatial
                            || policy.digest() != report.policy_digest()
                            || policy.binding_digest() != report.binding_digest()
                            || policy.probes() != actual.source_spatial_policy.probes()
                            || policy.mapping() != actual.source_spatial_policy.mapping()
                        {
                            return Err(CostError::CorrespondenceMismatch);
                        }
                        Ok(())
                    })
                    .map_err(errors::selection)??;
                if crate::product_correspondence::codec::roles::envelope(&input.plan.data)
                    .map_err(errors::product)?
                    != input.target.identity_envelope_bytes()
                {
                    return Err(CostError::CorrespondenceMismatch);
                }
                assemble(
                    input.plan,
                    &registry,
                    cases,
                    temporal,
                    spatial,
                    actual.root_digest,
                )
            },
        )
        .map_err(errors::tl)?
}
fn assemble(
    plan: &ProductPlan<'_>,
    registry: &raw::Registry,
    mut cases: raw::cases::Cases,
    time: Term,
    space: Term,
    root: Digest32,
) -> Result<Collected, CostError> {
    raw::remaining_declarations(&plan.data, &mut cases.declarations)?;
    let declarations = app("declarations-v1", cases.declarations);
    let bindings = registry.bindings(&plan.data)?;
    let fields = app(
        "field-stream-v1",
        vec![
            app("native-fields-v1", vec![cases.term, time, space]),
            declarations.clone(),
            bindings,
        ],
    );
    Ok(Collected {
        fields,
        declarations,
        catalog: cases.binding,
        root,
        admission_work: registry.work(),
    })
}
