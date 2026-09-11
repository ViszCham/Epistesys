use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct GrammarProfileId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrammarFeature {
    OpenLexicon,
    ClosedLexicon,
    GeneratedLexicon,
    ExecutableEffects,
    DenotationalSemantics,
    PragmaticActs,
    MacroExpansion,
    ContextSensitiveConstraints,
    LayoutSensitive,
    EmbeddedDialect,
    BidirectionalGeneration,
    TypeAndOwnership,
    DiscourseAndCoreference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LexiconPolicy {
    Open,
    Closed,
    Generated,
    Mixed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AmbiguityPolicy {
    PreserveForest,
    DeterministicAfterValidation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GrammarProfile {
    pub id: GrammarProfileId,
    pub name: String,
    pub revision: String,
    pub features: BTreeSet<GrammarFeature>,
    pub lexicon_policy: LexiconPolicy,
    pub ambiguity_policy: AmbiguityPolicy,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct GrammarProfileRegistry {
    profiles: BTreeMap<GrammarProfileId, GrammarProfile>,
}

impl GrammarProfileRegistry {
    pub fn insert(&mut self, profile: GrammarProfile) -> Result<(), GrammarProfileId> {
        if self.profiles.contains_key(&profile.id) {
            return Err(profile.id);
        }
        self.profiles.insert(profile.id, profile);
        Ok(())
    }

    pub fn get(&self, id: GrammarProfileId) -> Option<&GrammarProfile> {
        self.profiles.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &GrammarProfile> {
        self.profiles.values()
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

pub fn default_profile_registry() -> GrammarProfileRegistry {
    let mut registry = GrammarProfileRegistry::default();
    for profile in [
        GrammarProfile {
            id: GrammarProfileId(1),
            name: "generic-symbolic".into(),
            revision: "generic-symbolic.v1".into(),
            features: BTreeSet::from([
                GrammarFeature::DenotationalSemantics,
                GrammarFeature::ContextSensitiveConstraints,
            ]),
            lexicon_policy: LexiconPolicy::Mixed,
            ambiguity_policy: AmbiguityPolicy::PreserveForest,
        },
        GrammarProfile {
            id: GrammarProfileId(2),
            name: "open-discourse".into(),
            revision: "open-discourse.v1".into(),
            features: BTreeSet::from([
                GrammarFeature::OpenLexicon,
                GrammarFeature::PragmaticActs,
                GrammarFeature::DiscourseAndCoreference,
            ]),
            lexicon_policy: LexiconPolicy::Open,
            ambiguity_policy: AmbiguityPolicy::PreserveForest,
        },
        GrammarProfile {
            id: GrammarProfileId(3),
            name: "executable-symbolic".into(),
            revision: "executable-symbolic.v1".into(),
            features: BTreeSet::from([
                GrammarFeature::ClosedLexicon,
                GrammarFeature::GeneratedLexicon,
                GrammarFeature::ExecutableEffects,
                GrammarFeature::MacroExpansion,
                GrammarFeature::TypeAndOwnership,
            ]),
            lexicon_policy: LexiconPolicy::Mixed,
            ambiguity_policy: AmbiguityPolicy::DeterministicAfterValidation,
        },
        GrammarProfile {
            id: GrammarProfileId(4),
            name: "embedded-symbolic".into(),
            revision: "embedded-symbolic.v1".into(),
            features: BTreeSet::from([
                GrammarFeature::EmbeddedDialect,
                GrammarFeature::BidirectionalGeneration,
            ]),
            lexicon_policy: LexiconPolicy::Mixed,
            ambiguity_policy: AmbiguityPolicy::PreserveForest,
        },
    ] {
        registry.insert(profile).expect("unique builtin profile");
    }
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_features_not_natural_program_discriminants() {
        let registry = default_profile_registry();
        assert_eq!(registry.len(), 4);
        assert!(registry
            .iter()
            .any(|profile| profile.features.contains(&GrammarFeature::PragmaticActs)));
        assert!(registry.iter().any(|profile| profile
            .features
            .contains(&GrammarFeature::ExecutableEffects)));
    }
}
