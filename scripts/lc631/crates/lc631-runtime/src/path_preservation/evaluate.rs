//! Every destination is actually decoded against ROOT; FINAL is a separate run.
use super::{claim, hash, policy, recon, records, reserved, Error, OwnedPathEvaluation,
    PathEdge, PathObservation, RootGate, RootPathPlan};
use crate::{facet_ledger::FacetLedger, joint_selection::AxisValue};
use lc631_analysis::creativity_measurement::{weighted_assessment, RationalInterval, WeightedProbe};
use lc631_substrate::{resource::ResourceReservationView, Digest32};

pub fn evaluate_path<'plan,'root,'artifact,'view>(
    plan: &'plan RootPathPlan<'root,'view>,
    edges: &'artifact [PathEdge<'artifact>], final_bytes: &'artifact [u8],
    resources: &'view ResourceReservationView<'_>,
) -> Result<OwnedPathEvaluation<'plan,'root,'artifact,'view>,Error> {
    plan.current(resources)?;
    // Outer metadata/ledger/snapshot scratch and retained claims precede ANY
    // vector/ledger allocation. Existing captures/producers preclaim their own
    // 16MiB scratch and2MiB retained backing; limits never relax mid-path.
    let scratch=claim(resources,512*1024,false)?;
    let held=claim(resources,512*1024,true)?;
    policy::check_path(plan,edges,final_bytes)?;
    let count=(edges.len()+1)*plan.probes.len(); //bounded17*8, no overflow
    let mut ledger=FacetLedger::new(count,count*2).map_err(Error::Ledger)?;
    let mut history=reserved(edges.len())?;
    let mut actual_evaluations=0;
    for (index,edge) in edges.iter().enumerate() {
        let reports=produce_destination(plan,edge.to,resources)?;
        actual_evaluations+=reports.len();
        let (observation,_)=observe(plan,&reports,edge.to,resources)?;
        records::retain(plan,&reports,edge.to,index,&mut ledger)?;
        history.push(observation);
        // History snapshot is derived from actual retained input artifacts;
        // per-destination producer backing is released, never copied as FINAL.
    }
    let final_reports=produce_destination(plan,final_bytes,resources)?;
    actual_evaluations+=final_reports.len();
    let (final_observation,gate)=observe(plan,&final_reports,final_bytes,resources)?;
    records::retain(plan,&final_reports,final_bytes,edges.len(),&mut ledger)?;
    plan.current(resources)?;
    scratch.check().map_err(Error::Resource)?;
    held.check().map_err(Error::Resource)?;
    Ok(OwnedPathEvaluation {final_reports,history,final_observation,ledger,gate,
        plan,edges,final_bytes,actual_evaluations,held})
}
fn produce_destination<'plan,'view>(
    plan: &'plan RootPathPlan<'_,'view>, target: &[u8],
    resources: &'view ResourceReservationView<'_>,
) -> Result<Vec<recon::OwnedReconstruction<'plan,'view>>,Error> {
    let mut reports=reserved(plan.probes.len())?;
    for probe in &plan.probes {
        reports.push(recon::produce(&probe.protected,target,resources)
            .map_err(Error::Reconstruction)?);
    }
    Ok(reports)
}
fn observe(
    plan: &RootPathPlan<'_,'_>, reports: &[recon::OwnedReconstruction<'_,'_>],
    target: &[u8], resources: &ResourceReservationView<'_>,
) -> Result<(PathObservation,RootGate),Error> {
    let mut values=reserved(reports.len())?;
    let mut actual_results=[Digest32::from_bytes([0;32]);8];
    let mut gap=None;
    let mut gate=RootGate::PassInGivenFiniteProfile;
    for (index,(probe,report)) in plan.probes.iter().zip(reports).enumerate() {
        let axis=report.axis_current(&probe.protected,target,plan.context,resources)
            .map_err(Error::Reconstruction)?;
        actual_results[index]=Digest32::from_bytes(report.binding(
            crate::facet_ledger::TransformId([0;32])).result.0);
        match axis {
            AxisValue::Exact(bounds) => {
                if probe.hard && bounds.upper()>&0.into() {
                    gate=RootGate::FailInGivenFiniteProfile;
                }
                values.push(bounds.clone());
            }
            AxisValue::Unavailable(_) => {
                gap=report.historical_gap();
                if probe.hard && gate!=RootGate::FailInGivenFiniteProfile {
                    gate=RootGate::NeedsEvidence;
                }
            }
            _ => return Err(Error::Arithmetic), //this producer exposes exact or no scalar only
        }
    }
    let loss=if values.len()==reports.len() {Some(aggregate(plan,&values)?)} else {None};
    Ok((PathObservation {target:hash(target),loss,gap,actual_results,
        probe_count:reports.len()},gate))
}
fn aggregate(plan: &RootPathPlan<'_,'_>, values: &[RationalInterval]) -> Result<RationalInterval,Error> {
    let mut probes=reserved(values.len())?;
    for (probe,value) in plan.probes.iter().zip(values) {
        probes.push(WeightedProbe {canonical_payload:&probe.property.0,
            weight:&probe.weight,value});
    }
    weighted_assessment(&probes).map_err(|_|Error::Arithmetic)
}
