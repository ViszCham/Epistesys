//! 日本語: 明示Given有限MODEL tupleのみ。自然言語やhost最終回答へ結び付けない。
//! English: Explicit Given finite tuple only; no NL or host-final correspondence.
#![forbid(unsafe_code)]
mod config;
mod evaluate;
use crate::{effect_error::CliFailure, io::CommandIo};
use std::io::Write;
pub(crate) const MAX_CONFIG: usize = 16 * 1024;
pub(crate) const MAX_OUTPUT: usize = 64 * 1024;

/// Parent dispatcher must require exactly `epistesys-tl-check --config-stdin`.
pub(crate) fn run(resources: &CommandIo) -> Result<(), CliFailure> {
    let source = resources
        .read_from(&mut std::io::stdin().lock(), MAX_CONFIG as u64)
        .map_err(CliFailure::not_attempted)?;
    if source.len() > MAX_CONFIG {
        return Err(CliFailure::not_attempted("tl_config:Limit"));
    }
    let bytes = resources
        .with_checked_tl_view(&source, |view| evaluate::produce(&source, view))
        .map_err(CliFailure::not_attempted)?;
    std::io::stdout()
        .lock()
        .write_all(&bytes)
        .map_err(|_| CliFailure::from("tl_output:UnknownDeliveryNoRetry"))
}
