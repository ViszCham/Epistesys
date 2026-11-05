//! Actual legacy Description inputs; identity-decoder cost remains incomplete for product K.
use super::{NativeOwnerRefs, ProductError, ProductPlan, ReportedProduct};
use crate::{description_measurement as desc, reconstruction_measurement as recon};
use lc631_substrate::resource::ResourceReservationView;

pub(super) fn check(
    plan: &ProductPlan<'_>,
    baseline: &ReportedProduct<'_>,
    target: &ReportedProduct<'_>,
    owners: NativeOwnerRefs<'_, '_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), ProductError> {
    // Reuse the native exact-root current check. Its arithmetic preference is
    // discarded; it supplies neither product K nor a formula/selection input.
    let _ = desc::compare_current(owners.baseline_description, owners.target_description, view)
        .map_err(error)?;
    for (owner, bytes) in [
        (owners.baseline_description, baseline.semantic_bytes()),
        (owners.target_description, target.semantic_bytes()),
    ] {
        owner
            .inspect_product_bytes_current(view, |source, actual, context, lineage, report| {
                if source != baseline.semantic_bytes()
                    || actual != bytes
                    || context != plan.data.context
                    || report.binding.context != context.interpretation_digest()
                {
                    return Err(ProductError::CorrespondenceMismatch);
                }
                for scope in [lineage.baseline.scope, lineage.candidate.scope] {
                    if scope.id.0 != *plan.data.scope.comparison().as_bytes()
                        || scope.root_goal.0 != *plan.data.scope.goal().as_bytes()
                        || scope.probes.0 != *plan.data.scope.probes().as_bytes()
                        || scope.policy.0 != *plan.data.scope.semantics().as_bytes()
                    {
                        return Err(ProductError::CorrespondenceMismatch);
                    }
                }
                // Native scope.baseline stays native; semantic product baseline and
                // TL root identities remain separate. Missing selection-policy,
                // goal-profile/hard/evidence pins are not invented for Description.
                // Full original lineage/report are borrowed and remain the native evidence.
                // No full-cost owner is inferred from a successful byte correspondence.
                Ok(())
            })
            .map_err(error)??;
    }
    Ok(())
}
pub(super) fn error(error: desc::Error) -> ProductError {
    match error {
        desc::Error::Resource(e) => ProductError::Resource(e),
        desc::Error::Reconstruction(recon::Error::Resource(e)) => ProductError::Resource(e),
        _ => ProductError::CorrespondenceMismatch,
    }
}
