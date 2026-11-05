//! 日本語: currentへ不適格な候補の診断。未知候補をvalidへ補完しない。
//! English: Diagnose candidates ineligible for current admission, without repairing unknowns into valid data.
use super::{
    fingerprint::{self, Fingerprint, Kind},
    Error,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkerState {
    Exact,
    Foreign,
    Malformed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reason {
    ForeignMarker,
    MalformedMarker,
    MissingMarker,
}
pub fn classify_marker(raw: &[u8], expected: u64) -> Result<MarkerState, Error> {
    match fingerprint::hex_fingerprint(raw) {
        Ok(value) if value == expected => Ok(MarkerState::Exact),
        Ok(_) => Ok(MarkerState::Foreign),
        Err(Error::UnsupportedFormat) => Ok(MarkerState::Malformed),
        Err(other) => Err(other),
    }
}
pub(super) fn context(stage: &'static str, unit: &str, cause: Error) -> Error {
    Error::AtStage {
        stage,
        unit: unit.to_owned(),
        cause: Box::new(cause),
    }
}
pub fn parse_selected_metadata(
    raw: &[u8],
    profile: &str,
    directory: &str,
    stem: &str,
) -> Result<Fingerprint, Error> {
    let parsed =
        fingerprint::parse(raw).map_err(|e| context("selected_metadata_json", directory, e))?;
    parsed
        .check_local(profile, directory, stem)
        .map_err(|e| context("selected_metadata_local", directory, e))?;
    Ok(parsed)
}
struct Rejected {
    kind: Kind,
    unit: String,
    reason: Reason,
}
#[derive(Default)]
pub struct Rejections {
    total: usize,
    foreign: usize,
    malformed: usize,
    missing: usize,
    samples: Vec<Rejected>,
}
impl Rejections {
    pub fn record(&mut self, kind: Kind, unit: &str, reason: Reason) -> Result<(), Error> {
        if unit.len() > 128 {
            return Err(Error::Limit);
        }
        self.total = self.total.checked_add(1).ok_or(Error::Limit)?;
        let count = match reason {
            Reason::ForeignMarker => &mut self.foreign,
            Reason::MalformedMarker => &mut self.malformed,
            Reason::MissingMarker => &mut self.missing,
        };
        *count = count.checked_add(1).ok_or(Error::Limit)?;
        if self.samples.len() < 16 {
            self.samples
                .try_reserve_exact(1)
                .map_err(|_| Error::AllocationFailed)?;
            self.samples.push(Rejected {
                kind,
                unit: unit.to_owned(),
                reason,
            });
        }
        Ok(())
    }
    pub fn summary(&self) -> serde_json::Value {
        let samples: Vec<_> = self
            .samples
            .iter()
            .map(|r| {
                serde_json::json!({
            "crate":r.kind.crate_name(),"unit":r.unit,"reason":format!("{:?}",r.reason),
            "admitted_as_current":false})
            })
            .collect();
        serde_json::json!({"rejected_total":self.total,"foreign":self.foreign,
            "malformed":self.malformed,"missing_marker":self.missing,"samples":samples,
            "max_samples":16,"global_cache_authentication":false,
            "unknown_candidates_admitted_as_current":false})
    }
}
