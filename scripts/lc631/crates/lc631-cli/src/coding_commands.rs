//! Explicit new-profile default entry: an unavailable host provider is a Hold.
//! No receipt locator, seed, --execute or local key creates a provider.
#![forbid(unsafe_code)]
use crate::{effect_error::CliFailure, io::CommandIo};
use lc631_runtime::resource::ResourceBroker;
use serde::Serialize;

const REPAIR: &str = "lc631-dgcl-coding-repair";
const RESUME: &str = "lc631-dgcl-coding-resume";
#[derive(Serialize)]
struct Effects {
    source: &'static str,
    native: &'static str,
    persistence: &'static str,
}
#[derive(Serialize)]
struct MissingProviderSummary<'a> {
    schema_version: &'static str,
    profile: &'a str,
    status: &'static str,
    reason: &'static str,
    authority_created: bool,
    current_recommendation: Option<bool>,
    effects: Effects,
    source_writes: u64,
    native_invocations: u64,
    repair_reapplied: bool,
    head_renewed: bool,
    quota_reset: bool,
    resend_attempted: bool,
    claim_boundary: &'static str,
}
fn missing_provider(profile: &str) -> MissingProviderSummary<'_> {
    MissingProviderSummary{schema_version:"epistesys-coding-command-summary.v1",profile,
        status:"hold",reason:"missing_external_host_provider",authority_created:false,
        current_recommendation:None,effects:Effects{source:"not_attempted",native:"not_attempted",
            persistence:"not_attempted"},source_writes:0,native_invocations:0,
        repair_reapplied:false,head_renewed:false,quota_reset:false,resend_attempted:false,
        claim_boundary:"provider unavailable before packet/source/native/persistence admission; reported effects only; no configured binary positive, package or host closure"}
}

/// Central dispatch calls this BEFORE the legacy command dispatcher. The default
/// executable has no host-provider registration. Configured SDK callers use the
/// runtime attempt API, not environment-selected keys or a fixture-only CLI hook.
/// Recognized commands terminate only after CommandIo::finish has run.
pub(crate) fn dispatch_default(owner: &ResourceBroker) -> bool {
    let Some(command) = std::env::args_os().nth(1) else {
        return false;
    };
    let Some(command) = command.to_str() else {
        return false;
    };
    if command != REPAIR && command != RESUME {
        return false;
    }
    let resources = match CommandIo::begin(owner) {
        Ok(r) => r,
        Err(error) => {
            crate::report_failure(CliFailure::not_attempted(error));
            return true;
        }
    };
    let envelope = lc631_wire::WireEnvelope::shadow(command, missing_provider(command));
    let written = resources.with_view(65536, |view| {
        crate::io::write_pretty_json(&envelope, view, &mut std::io::stdout().lock())
            .map_err(|e| format!("coding_hold_output:{e:?}"))
    });
    let cleanup = resources.finish();
    if let Err(error) = written.and(cleanup) {
        crate::report_failure(CliFailure::not_attempted(error));
        return true;
    }
    // A semantic Hold is a handled non-success, distinct from malformed CLI2.
    std::process::exit(1)
}
