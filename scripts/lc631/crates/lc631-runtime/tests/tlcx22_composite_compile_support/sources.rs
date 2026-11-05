//! 日本語: testコンパイル時のcohort bytesと現fileを比較する。rlib出自の署名ではない。
//! English: Compare this test's compile-time cohort bytes with present files; no rlib-origin signature.
use super::{snapshot, Error};
use snapshot::Stamp;
use std::path::Path;
const SOURCES: &[(&str, &[u8])] = &[
    (
        "tests/tlcx22_composite_compile_support/diagnostics.rs",
        include_bytes!("diagnostics.rs"),
    ),
    (
        "src/composite_original.rs",
        include_bytes!("../../src/composite_original.rs"),
    ),
    (
        "src/composite_original/admission.rs",
        include_bytes!("../../src/composite_original/admission.rs"),
    ),
    (
        "src/composite_original/declarations.rs",
        include_bytes!("../../src/composite_original/declarations.rs"),
    ),
    (
        "src/composite_original/resources.rs",
        include_bytes!("../../src/composite_original/resources.rs"),
    ),
    (
        "src/composite_original/pending.rs",
        include_bytes!("../../src/composite_original/pending.rs"),
    ),
    (
        "src/composite_original/bool_program.rs",
        include_bytes!("../../src/composite_original/bool_program.rs"),
    ),
    (
        "src/composite_original/bool_program/current.rs",
        include_bytes!("../../src/composite_original/bool_program/current.rs"),
    ),
    (
        "src/composite_original/bool_program/payload.rs",
        include_bytes!("../../src/composite_original/bool_program/payload.rs"),
    ),
    (
        "src/compatibility_measurement/program.rs",
        include_bytes!("../../src/compatibility_measurement/program.rs"),
    ),
    (
        "src/bridge_gain_measurement/plan.rs",
        include_bytes!("../../src/bridge_gain_measurement/plan.rs"),
    ),
    (
        "src/original_score_pending.rs",
        include_bytes!("../../src/original_score_pending.rs"),
    ),
    ("src/lib.rs", include_bytes!("../../src/lib.rs")),
    (
        "tests/tlcx22_composite_original_compile_contract.rs",
        include_bytes!("../tlcx22_composite_original_compile_contract.rs"),
    ),
    (
        "tests/tlcx22_composite_bool_native_compile_contract.rs",
        include_bytes!("../tlcx22_composite_bool_native_compile_contract.rs"),
    ),
    (
        "tests/tlcx22_composite_compile_artifact_selection.rs",
        include_bytes!("../tlcx22_composite_compile_artifact_selection.rs"),
    ),
    (
        "tests/tlcx22_composite_compile_support/mod.rs",
        include_bytes!("mod.rs"),
    ),
    (
        "tests/tlcx22_composite_compile_support/fingerprint.rs",
        include_bytes!("fingerprint.rs"),
    ),
    (
        "tests/tlcx22_composite_compile_support/linked.rs",
        include_bytes!("linked.rs"),
    ),
    (
        "tests/tlcx22_composite_compile_support/snapshot.rs",
        include_bytes!("snapshot.rs"),
    ),
    (
        "tests/tlcx22_composite_compile_support/sources.rs",
        include_bytes!("sources.rs"),
    ),
];
pub(super) fn capture() -> Result<Vec<Stamp>, Error> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut stamps = Vec::new();
    for &(relative, compiled) in SOURCES {
        if compiled.len() > 256 * 1024 {
            return Err(Error::Limit);
        }
        let path = root.join(relative);
        let stamp = Stamp::capture(path, relative.to_owned(), 256 * 1024)?;
        if !stamp.matches(compiled.len() as u64, snapshot::hash(compiled)) {
            return Err(Error::SourceChanged);
        }
        stamps.push(stamp);
    }
    Ok(stamps)
}
