use lc631_analysis::{DgclRepairLoopReport, RepairEffectState};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EffectState {
    NotAttempted,
    MayHaveApplied,
    AppliedObserved,
    ObservedNoProgress,
    Unknown,
}

/// Errors describe evidence about effects; they do not roll effects back.
#[derive(Debug, Serialize)]
pub(crate) struct CliFailure {
    pub message: String,
    pub source_effect: EffectState,
    pub persistence_effect: EffectState,
    pub request_digest: Option<String>,
    pub journal_location: Option<String>,
}

impl From<String> for CliFailure {
    fn from(message: String) -> Self {
        Self {
            message,
            source_effect: EffectState::Unknown,
            persistence_effect: EffectState::Unknown,
            request_digest: None,
            journal_location: None,
        }
    }
}
impl From<&str> for CliFailure {
    fn from(message: &str) -> Self {
        Self::from(message.to_string())
    }
}

impl CliFailure {
    pub fn at_journal(mut self, reference: String) -> Self {
        self.journal_location = Some(reference);
        self
    }
    pub fn not_attempted(message: impl Into<String>) -> Self {
        Self {
            source_effect: EffectState::NotAttempted,
            persistence_effect: EffectState::NotAttempted,
            ..Self::from(message.into())
        }
    }
    pub fn possible_edit(message: impl Into<String>, request: &str) -> Self {
        Self {
            source_effect: EffectState::MayHaveApplied,
            persistence_effect: EffectState::MayHaveApplied,
            request_digest: Some(request.into()),
            ..Self::from(message.into())
        }
    }
    pub fn persistence_uncertain(message: impl Into<String>, request: &str) -> Self {
        Self {
            source_effect: EffectState::NotAttempted,
            persistence_effect: EffectState::MayHaveApplied,
            request_digest: Some(request.into()),
            ..Self::from(message.into())
        }
    }
    pub fn after_repair(
        message: impl Into<String>,
        request: &str,
        report: &DgclRepairLoopReport,
    ) -> Self {
        let effect = if report
            .attempts
            .iter()
            .any(|attempt| attempt.effect_state == RepairEffectState::MayHaveApplied)
        {
            EffectState::MayHaveApplied
        } else if report
            .attempts
            .iter()
            .any(|attempt| attempt.effect_state == RepairEffectState::ReceiptVerified)
        {
            EffectState::AppliedObserved
        } else if report
            .attempts
            .iter()
            .any(|attempt| attempt.effect_state == RepairEffectState::ObservedNoProgress)
        {
            EffectState::ObservedNoProgress
        } else {
            EffectState::NotAttempted
        };
        Self {
            source_effect: effect,
            persistence_effect: EffectState::MayHaveApplied,
            request_digest: Some(request.into()),
            ..Self::from(message.into())
        }
    }
    pub fn output_failure(message: String, output: &serde_json::Value) -> Self {
        let mut failure = Self::from(message);
        if let Some(attempts) = output
            .pointer("/repair/attempts")
            .and_then(serde_json::Value::as_array)
        {
            let states = attempts
                .iter()
                .filter_map(|attempt| attempt["effect_state"].as_str())
                .collect::<Vec<_>>();
            failure.source_effect = if states.contains(&"may_have_applied") {
                EffectState::MayHaveApplied
            } else if states.contains(&"receipt_verified") {
                EffectState::AppliedObserved
            } else if states.contains(&"observed_no_progress") {
                EffectState::ObservedNoProgress
            } else if !states.is_empty()
                && states.len() == attempts.len()
                && states.iter().all(|state| *state == "not_attempted")
            {
                EffectState::NotAttempted
            } else {
                EffectState::Unknown
            };
        }
        failure
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generic_errors_cannot_assert_no_mutation() {
        let failure = CliFailure::from("late unclassified error");
        assert_eq!(failure.source_effect, EffectState::Unknown);
        assert_eq!(failure.persistence_effect, EffectState::Unknown);
    }
    #[test]
    fn output_failure_preserves_an_already_observed_edit() {
        let output =
            serde_json::json!({"repair":{"attempts":[{"effect_state":"receipt_verified"}]}});
        assert_eq!(
            CliFailure::output_failure("broken pipe".into(), &output).source_effect,
            EffectState::AppliedObserved
        );
        assert_eq!(
            CliFailure::not_attempted("invalid arguments").source_effect,
            EffectState::NotAttempted
        );
    }

    #[test]
    fn unknown_or_missing_effect_states_never_become_not_attempted() {
        for attempts in [
            serde_json::json!([{"effect_state":"future_state"}]),
            serde_json::json!([{}]),
            serde_json::json!([]),
        ] {
            let output = serde_json::json!({"repair":{"attempts":attempts}});
            assert_eq!(
                CliFailure::output_failure("late failure".into(), &output).source_effect,
                EffectState::Unknown
            );
        }
    }
}
