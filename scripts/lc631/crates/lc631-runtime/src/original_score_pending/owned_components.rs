//! 日本語: 六種の実ownerを保持する原式preflight。異表現とTL欠測は残す。
//! English: Six concrete current-owner ports for original-score preflight.
//! Exact reported subjects are not semantic correspondence/authentication.
//! Original producer reports stay borrowed; no TL binding or scalar is minted.
//! Safe private borrowing is sufficient: no unsafe, new ABI, atomics, parallel
//! backend or performance claim. Fixed requirements never follow a candidate cut.
#![forbid(unsafe_code)]
mod current;
mod ports;

pub use current::{CurrentComponents, OwnedComponentCollection};
pub use ports::ComponentOwnerRef;

use crate::joint_selection::SelectionPins;
use lc631_analysis::creativity_measurement::{
    formula::OriginalFormula, ComponentMetric as M, MeasurementError,
};
use lc631_substrate::resource::ResourceError;

#[derive(Debug)]
pub enum Error {
    Resource(ResourceError),
    Description(crate::description_measurement::Error),
    Preserve(crate::structural_gain_measurement::Error),
    Design(crate::design_measurement::Error),
    Prediction(crate::prediction_measurement::Error),
    CounterRisk(crate::adversarial_measurement::Error),
    BridgeCost(crate::memory_measurement::Error),
    Assessment(MeasurementError),
    InvalidPlan,
    InvalidOwners,
    DuplicateMetric,
    ScopeMismatch,
    ForeignReservation,
    AllocationFailed,
}

/// Declare before producing/assessing candidates. A subset controls only the
/// diagnostic ComponentAssessment; original F1/F2 requirements stay independent.
/// Registry/goal labels are Given, never verified intent, oracle or permission.
pub struct CollectionPlan {
    pins: SelectionPins,
    formula: OriginalFormula,
    required: [M; 19],
    required_len: usize,
}
impl CollectionPlan {
    pub fn declared(pins: SelectionPins, formula: OriginalFormula) -> Result<Self, Error> {
        Self::declared_subset(pins, formula, original_required(formula))
    }

    pub fn declared_subset(
        pins: SelectionPins,
        formula: OriginalFormula,
        required: &[M],
    ) -> Result<Self, Error> {
        let original = original_required(formula);
        if required.is_empty() || required.len() > original.len() {
            return Err(Error::InvalidPlan);
        }
        for (index, metric) in required.iter().enumerate() {
            if !original.contains(metric) || required[..index].contains(metric) {
                return Err(Error::InvalidPlan);
            }
        }
        let mut fixed = [M::DescriptionBits; 19];
        fixed[..required.len()].copy_from_slice(required);
        Ok(Self { pins, formula, required: fixed, required_len: required.len() })
    }

    pub fn required_subset(&self) -> &[M] { &self.required[..self.required_len] }
    pub fn original_required(&self) -> &'static [M] { original_required(self.formula) }
}

fn original_required(formula: OriginalFormula) -> &'static [M] {
    // Reuse the existing full strict sets without editing the legacy preflight.
    match formula {
        OriginalFormula::F1V1 => super::F1,
        OriginalFormula::F2V1 => super::F2,
    }
}
