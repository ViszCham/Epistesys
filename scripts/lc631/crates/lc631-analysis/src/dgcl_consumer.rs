use crate::DgclStandaloneCandidate;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DgclConsumerVersion {
    LegacyV1,
    BoundedV2,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum LegacyDgclViewSchema {
    #[serde(rename = "epistesys-dgcl-consumer-view.v1")]
    V1,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyDgclConsumerView {
    pub schema_version: LegacyDgclViewSchema,
    pub source_revision: String,
    pub candidate_digest: String,
    #[serde(deserialize_with = "decode_false")]
    pub implementation_complete_candidate: bool,
    #[serde(deserialize_with = "decode_false")]
    pub host_send_authorized: bool,
    #[serde(deserialize_with = "decode_false")]
    pub output_commit_allowed: bool,
}

fn decode_false<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<bool, D::Error> {
    if bool::deserialize(decoder)? {
        return Err(serde::de::Error::custom(
            "legacy view cannot confer success or permission",
        ));
    }
    Ok(false)
}

/// Explicit receiver-version negotiation. This is a rendering adapter, not a
/// verifier or evidence producer; modern success remains explicitly reported.
pub fn project_dgcl_consumer_view(
    candidate: &DgclStandaloneCandidate,
    receiver: DgclConsumerVersion,
    requested_schema: &str,
) -> Result<serde_json::Value, &'static str> {
    match (receiver, requested_schema) {
        (DgclConsumerVersion::LegacyV1, "epistesys-dgcl-consumer-view.v1") => {
            serde_json::to_value(LegacyDgclConsumerView {
                schema_version: LegacyDgclViewSchema::V1,
                source_revision: candidate.source_revision.clone(),
                candidate_digest: candidate.candidate_digest.clone(),
                implementation_complete_candidate: false,
                host_send_authorized: false,
                output_commit_allowed: false,
            })
            .map_err(|_| "consumer_view_encoding")
        }
        (DgclConsumerVersion::BoundedV2, "epistesys-dgcl-consumer-view.v2") => {
            Ok(serde_json::json!({
                "schema_version": "epistesys-dgcl-consumer-view.v2", "source_revision": candidate.source_revision,
                "candidate_digest": candidate.candidate_digest,
                "reported_implementation_complete_candidate": candidate.implementation_complete_candidate,
                "requirement_realizations": candidate.requirement_realizations,
                "host_observation": candidate.host_observation,
                "host_send_authorized": false, "output_commit_allowed": false, "authority_created": false,
                "claim_boundary": "explicit report rendering only; not receipt validation or execution permission"
            }))
        }
        _ => Err("consumer_schema_not_negotiated"),
    }
}
