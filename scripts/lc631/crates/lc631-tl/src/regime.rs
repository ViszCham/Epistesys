use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementRegime {
    MathematicsLogic,
    Program,
    NaturalLanguage,
    CreativeArtifact,
    GeneralAction,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct MeasurementRegimeSet {
    regimes: BTreeSet<MeasurementRegime>,
}

impl MeasurementRegimeSet {
    pub fn contains(&self, regime: MeasurementRegime) -> bool {
        self.regimes.contains(&regime)
    }

    pub fn len(&self) -> usize {
        self.regimes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.regimes.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = MeasurementRegime> + '_ {
        self.regimes.iter().copied()
    }

    pub const fn can_authorize(&self) -> bool {
        false
    }
}

impl FromIterator<MeasurementRegime> for MeasurementRegimeSet {
    fn from_iter<T: IntoIterator<Item = MeasurementRegime>>(iter: T) -> Self {
        Self {
            regimes: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regimes_are_multilabel_and_non_authoritative() {
        let regimes = MeasurementRegimeSet::from_iter([
            MeasurementRegime::Program,
            MeasurementRegime::NaturalLanguage,
            MeasurementRegime::Program,
        ]);
        assert_eq!(regimes.len(), 2);
        assert!(!regimes.can_authorize());
    }
}
