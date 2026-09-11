use crate::ParseDefect;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CalibrationPartition {
    pub training_revision: String,
    pub calibration_revision: String,
    pub holdout_revision: String,
    pub adversarial_revision: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CalibrationError {
    EmptyRevision,
    PartitionLeakage,
}

impl CalibrationPartition {
    pub fn checked(
        training: impl Into<String>,
        calibration: impl Into<String>,
        holdout: impl Into<String>,
        adversarial: impl Into<String>,
    ) -> Result<Self, CalibrationError> {
        let partition = Self {
            training_revision: training.into(),
            calibration_revision: calibration.into(),
            holdout_revision: holdout.into(),
            adversarial_revision: adversarial.into(),
        };
        let values = [
            partition.training_revision.as_str(),
            partition.calibration_revision.as_str(),
            partition.holdout_revision.as_str(),
            partition.adversarial_revision.as_str(),
        ];
        if values.iter().any(|value| value.trim().is_empty()) {
            return Err(CalibrationError::EmptyRevision);
        }
        if values.into_iter().collect::<BTreeSet<_>>().len() != 4 {
            return Err(CalibrationError::PartitionLeakage);
        }
        Ok(partition)
    }

    pub fn reference() -> Self {
        Self::checked(
            "tldg-train.unobserved",
            "tldg-calibration.unobserved",
            "tldg-holdout.unobserved",
            "tldg-adversarial.local-v1",
        )
        .expect("distinct reference partitions")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmpiricalRiskSource {
    LocalDeterministicDefectInventory,
    ExternalCalibrationUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EmpiricalRiskOverlay {
    pub partition: CalibrationPartition,
    pub prediction_set: BTreeSet<String>,
    pub source: EmpiricalRiskSource,
    pub coverage_assumption: String,
    pub advisory_only: bool,
    pub abstained: bool,
}

impl EmpiricalRiskOverlay {
    pub fn from_defects(defects: &[ParseDefect], partition: &CalibrationPartition) -> Self {
        let prediction_set = defects
            .iter()
            .map(|defect| format!("{defect:?}"))
            .collect::<BTreeSet<_>>();
        Self {
            partition: partition.clone(),
            source: if prediction_set.is_empty() {
                EmpiricalRiskSource::ExternalCalibrationUnavailable
            } else {
                EmpiricalRiskSource::LocalDeterministicDefectInventory
            },
            prediction_set,
            coverage_assumption:
                "no statistical coverage claim; external exchangeable calibration is unavailable"
                    .into(),
            advisory_only: true,
            abstained: true,
        }
    }

    pub const fn can_authorize(&self) -> bool {
        false
    }

    pub const fn can_replace_defect_graph(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_leakage_is_rejected() {
        assert_eq!(
            CalibrationPartition::checked("same", "same", "holdout", "adversarial"),
            Err(CalibrationError::PartitionLeakage)
        );
    }
}
