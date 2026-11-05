//! Literal role/family fixtures; neither generator nor oracle labels.
use super::*;
use lc631_runtime::{explanation_measurement as e, transfer_measurement as t};
pub fn binding() -> e::Binding {
    e::Binding {
        definition: t::definition_revision(),
        ..super::binding(e::DatasetRole::TransferValidation)
    }
}
pub fn protocol() -> t::Protocol {
    t::Protocol {
        demonstrations: 0,
        adaptive_updates: 0,
    }
}
pub fn cases(f: &Fixture) -> [t::TransferCase<'_>; 2] {
    [
        t::TransferCase {
            key: b"t1",
            theory: &f.gamma,
            goal: &f.pq,
            weight: &f.one,
        },
        t::TransferCase {
            key: b"t2",
            theory: &f.gamma,
            goal: &f.qr,
            weight: &f.three,
        },
    ]
}
pub fn entries(f: &Fixture) -> [t::SplitEntry<'_>; 4] {
    [
        t::SplitEntry {
            key: b"search",
            family: b"atomic-projection",
            clone_family: b"source-clone",
            role: e::DatasetRole::Search,
            source_payload: b"pq,r|-p",
            theory: &f.gamma,
            goal: &f.p,
        },
        t::SplitEntry {
            key: b"explain",
            family: b"existing-atomic-premise",
            clone_family: b"explain-clone",
            role: e::DatasetRole::ExplanationValidation,
            source_payload: b"pq,r|-r",
            theory: &f.gamma,
            goal: &f.r,
        },
        t::SplitEntry {
            key: b"t1",
            family: b"compound-conjunction",
            clone_family: b"transfer-one",
            role: e::DatasetRole::TransferValidation,
            source_payload: b"pq,r|-pq",
            theory: &f.gamma,
            goal: &f.pq,
        },
        t::SplitEntry {
            key: b"t2",
            family: b"compound-conjunction",
            clone_family: b"transfer-two",
            role: e::DatasetRole::TransferValidation,
            source_payload: b"pq,r|-qr",
            theory: &f.gamma,
            goal: &f.qr,
        },
    ]
}
pub fn root<'a>(
    cases: &'a [t::TransferCase<'a>],
    entries: &'a [t::SplitEntry<'a>],
) -> t::TransferRootInput<'a> {
    t::TransferRootInput {
        binding: binding(),
        cases,
        registry: entries,
        split_revision: d(90),
        normalizer_revision: t::split_normalizer_revision(),
        baseline: t::Program::ExtractV1,
        protocol: protocol(),
        limits: limits(),
    }
}
pub fn input(n: u8, program: t::Program) -> t::TransferCandidateInput {
    t::TransferCandidateInput {
        binding: candidate(n),
        program,
        protocol: protocol(),
    }
}
