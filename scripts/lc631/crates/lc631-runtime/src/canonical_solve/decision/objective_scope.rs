//! Stable actual objective-step binding and explicit remaining evidence gaps.
use crate::{
    canonical_solve::{
        types::PostprocessError, ObjectiveNormalization as N, ObjectiveStepState as S,
        OneStepBudget, OwnedObjectiveStep,
    },
    facet_ledger as l,
};
use lc631_solvers::smt::ObjectiveDirection;
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

fn field(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
fn optional_i64(hash: &mut Sha256, value: Option<i64>) {
    match value {
        None => hash.update([0]),
        Some(value) => {
            hash.update([1]);
            hash.update(value.to_le_bytes());
        }
    }
}
pub(super) fn bind(
    base: Digest32,
    step: &OwnedObjectiveStep<'_>,
    budget: OneStepBudget,
) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(b"canonical.actual-one-step-attachment.v2\0");
    field(&mut hash, base.as_bytes());
    hash.update([
        match step.state() {
            S::MissingNativeModel => 1,
            S::UnavailableStatusCapability => 2,
            S::BudgetLimited => 3,
            S::ReportedNoStrictImprovement => 4,
            S::ImprovedModelRechecked => 5,
            S::ProbeUnresolved => 6,
            S::ImprovedModelRejected => 7,
        },
        step.attempts_requested(),
        budget.max_native_attempts(),
    ]);
    hash.update(budget.max_total_wall_ms().to_le_bytes());
    hash.update([
        match step.direction() {
            ObjectiveDirection::Minimize => 1,
            ObjectiveDirection::Maximize => 2,
        },
        match step.normalization() {
            N::PositiveWidth => 1,
            N::UndefinedZeroWidth => 2,
            N::MissingSelectedValue => 3,
        },
    ]);
    hash.update(step.lower().to_le_bytes());
    hash.update(step.upper().to_le_bytes());
    optional_i64(&mut hash, step.initial_value());
    optional_i64(&mut hash, step.selected_value());
    field(&mut hash, step.probe_query_bytes());
    match step.probe_report() {
        None => hash.update([0]),
        Some(report) => {
            hash.update([1]);
            hash.update(super::report_identity::report_digest(report).as_bytes());
        }
    }
    match step.third_binding_at_render() {
        None => hash.update([0]),
        Some(binding) => {
            hash.update([1]);
            hash.update(binding.as_bytes());
        }
    }
    Digest32::from_bytes(hash.finalize().into())
}
fn property(bytes: &[u8]) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(b"canonical.objective-gap.v1\0");
    field(&mut hash, bytes);
    Digest32::from_bytes(hash.finalize().into())
}
pub(super) fn append_gaps(
    ledger: &mut l::FacetLedger,
    transform: l::TransformId,
    query: Digest32,
    step: &OwnedObjectiveStep<'_>,
) -> Result<(), PostprocessError> {
    let mut append = |name: &[u8], reason: l::GapReason| {
        let key = property(name);
        ledger
            .append(l::Entry {
                transform,
                facet: l::Facet::Uncertainty(l::Gap {
                    property: l::PropertyId(*key.as_bytes()),
                    reason,
                    required_evidence: l::Revision(*key.as_bytes()),
                    next_action: l::Revision(*query.as_bytes()),
                }),
            })
            .map_err(PostprocessError::Ledger)
    };
    append(
        b"objective-search-one-step-incomplete",
        l::GapReason::Unverified,
    )?;
    append(
        b"objective-total-wall-cutoff-not-atomic",
        l::GapReason::Unverified,
    )?;
    if step.probe_report().is_some() {
        append(
            b"objective-probe-raw-streams-unowned",
            l::GapReason::Unobserved,
        )?;
    } else {
        append(
            b"objective-status-probe-unobserved",
            l::GapReason::Unobserved,
        )?;
    }
    if step.selected_report().is_some() {
        append(
            b"objective-selected-stderr-unowned",
            l::GapReason::Unobserved,
        )?;
    }
    if step.state() == S::UnavailableStatusCapability {
        append(
            b"objective-status-capability-unavailable",
            l::GapReason::Unsupported,
        )?;
    }
    if step.state() == S::ReportedNoStrictImprovement {
        append(b"objective-unsat-proof-unchecked", l::GapReason::Unverified)?;
    }
    Ok(())
}
