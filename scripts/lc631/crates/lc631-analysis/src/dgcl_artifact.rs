use lc631_core::stable_sha256;
use lc631_tldg::{analyze_dg1, Dg1Budget, Dg1Error, Dg1Report};
use serde::Serialize;
use std::sync::Arc;

const MAX_MANIFEST_BYTES: usize = 65_536;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SourceRevision(String);

impl SourceRevision {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct GrammarRevision(String);

impl GrammarRevision {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ProfileRevision(String);

impl ProfileRevision {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ArtifactId(String);

impl ArtifactId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSnapshot {
    revision: SourceRevision,
    text: Arc<str>,
}

impl SourceSnapshot {
    pub fn revision(&self) -> &SourceRevision {
        &self.revision
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RequirementIdentity {
    id: String,
    artifact_id: ArtifactId,
    candidate_ordinal: u32,
    source_span: std::ops::Range<usize>,
}

impl RequirementIdentity {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    pub fn candidate_ordinal(&self) -> u32 {
        self.candidate_ordinal
    }

    pub fn source_span(&self) -> std::ops::Range<usize> {
        self.source_span.clone()
    }
}

#[derive(Clone, Debug)]
pub struct DgclArtifact {
    id: ArtifactId,
    source: SourceSnapshot,
    grammar_revision: GrammarRevision,
    profile_revision: ProfileRevision,
    budget_digest: String,
    dg1: Dg1Report,
    requirement_ids: Vec<RequirementIdentity>,
}

impl DgclArtifact {
    pub fn artifact_id(&self) -> &ArtifactId {
        &self.id
    }

    pub fn source(&self) -> &SourceSnapshot {
        &self.source
    }

    pub fn source_revision(&self) -> &SourceRevision {
        self.source.revision()
    }

    pub fn grammar_revision(&self) -> &GrammarRevision {
        &self.grammar_revision
    }

    pub fn profile_revision(&self) -> &ProfileRevision {
        &self.profile_revision
    }

    pub fn budget_digest(&self) -> &str {
        &self.budget_digest
    }

    pub fn dg1(&self) -> &Dg1Report {
        &self.dg1
    }

    pub fn requirement_ids(&self) -> &[RequirementIdentity] {
        &self.requirement_ids
    }
}

pub struct DgclArtifactRequest<'a> {
    pub source: &'a str,
    pub grammar_manifest: &'a str,
    pub profile_manifest: &'a str,
    pub budget: Dg1Budget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DgclArtifactError {
    ManifestInvalid,
    Parse(Dg1Error),
    SourceRevisionMismatch,
    SourceSpanInvalid,
}

pub fn build_dgcl_artifact(
    request: DgclArtifactRequest<'_>,
) -> Result<DgclArtifact, DgclArtifactError> {
    if !valid_manifest(request.grammar_manifest) || !valid_manifest(request.profile_manifest) {
        return Err(DgclArtifactError::ManifestInvalid);
    }
    let source_revision = SourceRevision(stable_sha256(request.source));
    let grammar_revision = GrammarRevision(stable_sha256(request.grammar_manifest));
    let profile_revision = ProfileRevision(stable_sha256(request.profile_manifest));
    let budget_digest = stable_sha256(
        &serde_json::to_string(&request.budget).map_err(|_| DgclArtifactError::ManifestInvalid)?,
    );
    let artifact_id = ArtifactId(stable_sha256(&format!(
        "epistesys-dgcl-artifact.v1\0{}\0{}\0{}\0{}",
        source_revision.as_str(),
        grammar_revision.as_str(),
        profile_revision.as_str(),
        budget_digest
    )));
    let source = SourceSnapshot {
        revision: source_revision,
        text: Arc::<str>::from(request.source),
    };
    let dg1 = analyze_dg1(source.text(), request.budget).map_err(DgclArtifactError::Parse)?;
    if dg1.source_revision != source.revision.as_str() {
        return Err(DgclArtifactError::SourceRevisionMismatch);
    }
    let requirement_ids = dg1
        .requirement_candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let ordinal =
                u32::try_from(index + 1).map_err(|_| DgclArtifactError::SourceSpanInvalid)?;
            let span = candidate.source_span.clone();
            let source_text = source
                .text()
                .get(span.clone())
                .ok_or(DgclArtifactError::SourceSpanInvalid)?;
            if source_text != candidate.source_text.as_str() {
                return Err(DgclArtifactError::SourceSpanInvalid);
            }
            let id = stable_sha256(&format!(
                "epistesys-dgcl-requirement.v1\0{}\0{}\0{}\0{}\0{}",
                artifact_id.as_str(),
                ordinal,
                span.start,
                span.end,
                stable_sha256(source_text)
            ));
            Ok(RequirementIdentity {
                id,
                artifact_id: artifact_id.clone(),
                candidate_ordinal: ordinal,
                source_span: span,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DgclArtifact {
        id: artifact_id,
        source,
        grammar_revision,
        profile_revision,
        budget_digest,
        dg1,
        requirement_ids,
    })
}

fn valid_manifest(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_MANIFEST_BYTES
        && !value.chars().any(char::is_control)
}
