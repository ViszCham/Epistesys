//! Reported states are data, never validation or execution capabilities.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationGap {
    MissingEvaluator,
    UnsupportedFragment,
    Timeout,
    MissingEvidence,
    Uncalibrated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Measurement<T> {
    Known(T),
    Unobserved(ObservationGap),
    NotApplicable,
    Incomparable,
}

impl<T> Measurement<T> {
    /// Only exposes an explicitly reported value; it does not verify it.
    pub fn known_value(&self) -> Option<&T> {
        match self {
            Self::Known(value) => Some(value),
            Self::Unobserved(_) | Self::NotApplicable | Self::Incomparable => None,
        }
    }
}
