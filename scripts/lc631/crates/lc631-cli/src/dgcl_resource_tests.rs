//! Portable package-parser resource wiring; no NLP/compiler truth is claimed.
use super::build_package_pipeline_with_resources;
use crate::io::CommandIo;
use lc631_core::context::SourceRevision;
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use std::time::Duration;

fn limits() -> ResourceLimits {
    ResourceLimits::checked([
        128 * 1024 * 1024,
        256 * 1024 * 1024,
        64 * 1024 * 1024,
        16 * 1024 * 1024,
        2,
        2 * 1024 * 1024,
    ])
    .unwrap()
}

#[test]
fn tlcx05_package_helper_cannot_escape_a_saturated_same_parent_budget() {
    let broker = ResourceBroker::new(limits()).unwrap();
    let io = CommandIo::begin(&broker).unwrap();
    let mut saturator = broker
        .begin_run(
            900_001,
            SourceRevision::of("saturator").digest(),
            limits(),
            Deadline::after(Duration::from_secs(10)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut held = saturator
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(128 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    let error =
        build_package_pipeline_with_resources("Please verify the result.", &io).unwrap_err();
    assert!(error.contains("Exhausted"), "{error}");
    held.release_confirmed().unwrap();
    saturator.close_confirmed().unwrap();
    io.finish().unwrap();
}

#[test]
fn tlcx05_package_helper_positive_keeps_unconfigured_backend_and_authority_pending() {
    let broker = ResourceBroker::new(limits()).unwrap();
    let io = CommandIo::begin(&broker).unwrap();
    let report = build_package_pipeline_with_resources(
        "Please verify the result.\n\n結果を確認してください。",
        &io,
    )
    .unwrap();
    assert!(report.language_regions.is_empty());
    assert!(!report.unsupported_operative_region_ids.is_empty());
    assert!(!report.authority_created && !report.complete_instruction_grammar_claim);
    io.finish().unwrap();
    assert_eq!(broker.used(ResourceKind::Child).unwrap(), 0);
}
