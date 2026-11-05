//! Actual existing projection/DTO serialization and legacy decoder over bounded bytes.
use super::*;
use crate::{
    project_dgcl_consumer_view, DgclConsumerVersion, LegacyDgclConsumerView, LegacyDgclViewSchema,
};
use serde::Serialize;
use std::io::{self, Write};
pub(super) fn encode<T: Serialize>(value: &T, cap: usize, error: Error) -> Result<Vec<u8>, Error> {
    let mut out = Bounded {
        bytes: resources::vector(cap)?,
        cap,
        exhausted: false,
    };
    if serde_json::to_writer(&mut out, value).is_err() {
        return Err(if out.exhausted {
            error
        } else {
            Error::ProducerFailure
        });
    }
    Ok(out.bytes)
}
struct Bounded {
    bytes: Vec<u8>,
    cap: usize,
    exhausted: bool,
}
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let end = self.bytes.len().checked_add(bytes.len());
        if end.map(|n| n > self.cap).unwrap_or(true) {
            self.exhausted = true;
            return Err(io::Error::other("bounded legacy frame"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn produce(
    raw: &DgclStandaloneCandidate,
    kind: AdapterKind,
    cap: usize,
) -> Result<Vec<u8>, Error> {
    match kind {
        AdapterKind::LegacyProjectionV1 | AdapterKind::BoundedProjectionV2 => {
            let receiver = if kind == AdapterKind::LegacyProjectionV1 {
                DgclConsumerVersion::LegacyV1
            } else {
                DgclConsumerVersion::BoundedV2
            };
            let actual = project_dgcl_consumer_view(raw, receiver, schema(kind))
                .map_err(|_| Error::ProducerFailure)?;
            encode(&actual, cap, Error::OutputBudget)
        }
        AdapterKind::DirectLegacyDtoSerializeV1 => encode(
            &LegacyDgclConsumerView {
                schema_version: LegacyDgclViewSchema::V1,
                source_revision: raw.source_revision.clone(),
                candidate_digest: raw.candidate_digest.clone(),
                implementation_complete_candidate: raw.implementation_complete_candidate,
                host_send_authorized: raw.host_send_authorized,
                output_commit_allowed: raw.output_commit_allowed,
            },
            cap,
            Error::OutputBudget,
        ),
    }
}
pub(super) fn property(
    p: LegacyProperty,
    value: &serde_json::Value,
    bytes: &[u8],
    raw: &DgclStandaloneCandidate,
    kind: AdapterKind,
) -> PropertyOutcome {
    let boolean = |key| match value.get(key).and_then(serde_json::Value::as_bool) {
        Some(false) => PropertyOutcome::Preserved,
        Some(true) => PropertyOutcome::ConfirmedViolation,
        None => PropertyOutcome::NotObserved(NotObservedReason::ProducerFailure),
    };
    let confirmed = |b| {
        if b {
            PropertyOutcome::Preserved
        } else {
            PropertyOutcome::ConfirmedViolation
        }
    };
    let string = |key, wanted: &str| match value.get(key).and_then(serde_json::Value::as_str) {
        Some(actual) => confirmed(actual == wanted),
        None => PropertyOutcome::NotObserved(NotObservedReason::ProducerFailure),
    };
    match p {
        LegacyProperty::JsonObjectEnvelope => confirmed(value.is_object()),
        LegacyProperty::V1StrictConsumerAcceptance => {
            confirmed(serde_json::from_slice::<LegacyDgclConsumerView>(bytes).is_ok())
        }
        LegacyProperty::SourceIdentity => string("source_revision", &raw.source_revision),
        LegacyProperty::CandidateIdentity => string("candidate_digest", &raw.candidate_digest),
        LegacyProperty::NoHostSend => boolean("host_send_authorized"),
        LegacyProperty::NoOutputCommit => boolean("output_commit_allowed"),
        LegacyProperty::NoLegacyCompletion => {
            if kind == AdapterKind::BoundedProjectionV2
                && value.get("implementation_complete_candidate").is_none()
            {
                PropertyOutcome::Preserved
            } else {
                boolean("implementation_complete_candidate")
            }
        }
        LegacyProperty::NoAuthorityCreation => {
            if kind == AdapterKind::BoundedProjectionV2 || value.get("authority_created").is_some()
            {
                boolean("authority_created")
            } else {
                PropertyOutcome::Preserved
            }
        }
        LegacyProperty::NegotiatedSchema => string("schema_version", schema(kind)),
    }
}
