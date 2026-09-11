#![forbid(unsafe_code)]

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub const V630_COMMIT: &str = "0d933982fa392043c2fe825644a3a560963b6615";
pub const BASELINE_SCHEMA: &str = "lc631-v630-frozen-baseline.v1";

pub const PROTECTED_FILES: [(&str, &str); 3] = [
    (
        ".codex-plugin/plugin.json",
        "172961d5f06f1d8ccd395133e1a67ccf5c230e874be4a100abfbe1e5199e0362",
    ),
    (
        "scripts/reasoning-helper/Cargo.toml",
        "0ae97c5556baab8b1e80967877c37b13632bfb42d1cc50a40649a5baaf680fac",
    ),
    (
        "scripts/reasoning-helper/Cargo.lock",
        "261b3653b855e0dcf76ac8c977c2ec6ae85372b8a63b6a04e6267a171dac2823",
    ),
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BaselineFileObservation {
    pub path: String,
    pub expected_sha256: String,
    pub observed_sha256: Option<String>,
    pub matches: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BaselineGuardReport {
    pub schema_version: &'static str,
    pub expected_commit: &'static str,
    pub files: Vec<BaselineFileObservation>,
    pub baseline_intact: bool,
    pub claim_boundary: &'static str,
}

pub fn audit_v630_root(repository_root: &Path) -> BaselineGuardReport {
    let files = PROTECTED_FILES
        .iter()
        .map(|(relative, expected)| {
            let path = repository_root.join(relative);
            let observed = hash_file(&path).ok();
            BaselineFileObservation {
                path: relative.to_string(),
                expected_sha256: expected.to_string(),
                matches: observed.as_deref() == Some(*expected),
                observed_sha256: observed,
            }
        })
        .collect::<Vec<_>>();
    BaselineGuardReport {
        schema_version: BASELINE_SCHEMA,
        expected_commit: V630_COMMIT,
        baseline_intact: files.iter().all(|file| file.matches),
        files,
        claim_boundary:
            "matching protected-file digests establish only the frozen compatibility boundary, not repository-wide equivalence or safety",
    }
}

pub fn hash_file(path: &Path) -> Result<String, CompatError> {
    let bytes = fs::read(path).map_err(|_| CompatError::ReadFailed(path.to_path_buf()))?;
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    Ok(encoded)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DivergenceKind {
    None,
    Schema,
    Authority,
    TranslationWitness,
    WorldBudget,
    AcceleratorLane,
    MediaBoundary,
    OutputDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PairedObservation {
    pub seed_digest: String,
    pub v630_artifact_digest: String,
    pub v631_artifact_digest: String,
    pub divergences: Vec<DivergenceKind>,
    pub replacement_ready: bool,
}

impl PairedObservation {
    pub fn shadow(
        seed_digest: String,
        v630_artifact_digest: String,
        v631_artifact_digest: String,
        divergences: Vec<DivergenceKind>,
    ) -> Self {
        Self {
            seed_digest,
            v630_artifact_digest,
            v631_artifact_digest,
            divergences,
            replacement_ready: false,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct LegacyStrongStateInput {
    pub verified_host_user: bool,
    pub host_bound: bool,
    pub validated_observation: bool,
    pub promotion_allowed: bool,
    pub release_complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LegacyStrongStateQuarantine {
    pub legacy_fields_preserved: LegacyStrongStateInput,
    pub strong_state_accepted: bool,
    pub shadow_only: bool,
    pub blockers: Vec<String>,
    pub claim_boundary: &'static str,
}

pub fn quarantine_legacy_strong_state(
    input: LegacyStrongStateInput,
) -> LegacyStrongStateQuarantine {
    LegacyStrongStateQuarantine {
        legacy_fields_preserved: input,
        strong_state_accepted: false,
        shadow_only: true,
        blockers: vec![
            "legacy verified-looking booleans and enums lack an authenticated receipt envelope"
                .into(),
        ],
        claim_boundary:
            "legacy fields remain serializable for compatibility but cannot enter v6.3.1 strong gates",
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum CompatError {
    ReadFailed(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_has_three_distinct_protected_paths() {
        let mut paths = PROTECTED_FILES
            .iter()
            .map(|entry| entry.0)
            .collect::<Vec<_>>();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), PROTECTED_FILES.len());
        assert!(PROTECTED_FILES.iter().all(
            |(_, digest)| digest.len() == 64 && digest.chars().all(|ch| ch.is_ascii_hexdigit())
        ));
    }

    #[test]
    fn paired_observation_never_self_promotes() {
        let observation = PairedObservation::shadow(
            "seed".into(),
            "v630".into(),
            "v631".into(),
            vec![DivergenceKind::TranslationWitness],
        );
        assert!(!observation.replacement_ready);
    }

    #[test]
    fn every_legacy_strong_boolean_is_quarantined() {
        let report = quarantine_legacy_strong_state(LegacyStrongStateInput {
            verified_host_user: true,
            host_bound: true,
            validated_observation: true,
            promotion_allowed: true,
            release_complete: true,
        });
        assert!(!report.strong_state_accepted);
        assert!(report.shadow_only);
    }
}
