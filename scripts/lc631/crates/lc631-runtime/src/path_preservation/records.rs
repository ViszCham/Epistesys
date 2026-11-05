//! Canonical ledger raw history plus its existing unique-root-defect view.
use super::{hash, recon, Error, RootPathPlan};
use crate::facet_ledger::*;
use sha2::{Digest,Sha256};

pub(super) fn retain(
    plan: &RootPathPlan<'_,'_>, reports: &[recon::OwnedReconstruction<'_,'_>],
    target: &[u8], ordinal: usize, ledger: &mut FacetLedger,
) -> Result<(),Error> {
    for (index,(probe,report)) in plan.probes.iter().zip(reports).enumerate() {
        let mut id=Sha256::new();
        id.update(b"tlcx-root-path-actual-observation-v1");
        id.update(plan.root_revision().as_bytes());
        id.update(hash(target).as_bytes());
        id.update(plan.mapping.digest().as_bytes());
        id.update((ordinal as u64).to_le_bytes());
        id.update((index as u64).to_le_bytes());
        let transform=TransformId(id.finalize().into());
        let actual=report.binding(transform);
        let binding=TransformBinding {id:transform,
            source:Revision(*plan.root_revision().as_bytes()),
            target:Revision(*hash(target).as_bytes()),
            source_context:Revision(*plan.context.interpretation_digest().as_bytes()),
            target_context:Revision(*plan.context.interpretation_digest().as_bytes()),
            direction:Direction::Forward,scope:plan.scope,
            producer:actual.producer,query:actual.query,result:actual.result};
        ledger.admit_transform(binding).map_err(Error::Ledger)?;
        ledger.append(Entry {transform,facet:Facet::Uncertainty(Gap {
            property:probe.property,reason:GapReason::Unverified,
            required_evidence:Revision(*plan.context.source().digest().as_bytes()),
            next_action:Revision(*probe.protected.probe_digest().as_bytes()),
        })}).map_err(Error::Ledger)?;
        if report.historical_bounds().is_some_and(|q| q.lower()>&0.into()) {
            ledger.append(Entry {transform,facet:Facet::Defect(Defect {
                key:DefectKey {root:probe.obligation,property:probe.property,
                    source:binding.source,target:binding.target},
                axis:LossAxis::Reconstruction,witness:EvidenceRef(actual.result.0),
            })}).map_err(Error::Ledger)?;
        }
    }
    Ok(())
}
