//! 日本語: 外部入力の長さ・形状をhash/parse/IOより先に制限する。
//! English: Bound caller input shape and length before hashing, parsing or I/O.
use super::{BuildError, BuildPlanInput, CompilerSpec, Emission};
use std::path::{Component, Path};

pub(super) const PLAN_RAM: u64 = 128 * 1024;
pub(super) const PARSE_RAM: u64 = 2 * 1024 * 1024;

fn absolute(path: &Path) -> bool {
    path.is_absolute() && path.to_str().is_some_and(|s| !s.is_empty() && s.len() <= 4096)
        && path.components().all(|c| matches!(c, Component::Prefix(_) | Component::RootDir | Component::Normal(_)))
}
fn relative(path: &Path) -> bool {
    !path.is_absolute() && path.to_str().is_some_and(|s| !s.is_empty() && s.len() <= 4096)
        && path.components().count() <= 32
        && path.components().all(|c| matches!(c, Component::Normal(_)))
}
pub(super) fn compiler_shape(spec: CompilerSpec<'_>) -> bool {
    let zero = lc631_substrate::Digest32::from_bytes([0; 32]);
    absolute(spec.executable) && absolute(spec.declared_sysroot)
        && spec.expected_image != zero && spec.expected_verbose != zero
        && spec.toolchain_profile != zero
}
pub(super) fn validate(input: &BuildPlanInput<'_>) -> Result<(), BuildError> {
    let l = input.limits;
    if l.max_builds != 1 || l.max_children != 3 || !(1..=8).contains(&l.max_current_reads)
        || !(1..=16384).contains(&l.max_subject_bytes)
        || !(1..=32768).contains(&l.max_frame_bytes)
        || !(1..=64 * 1024 * 1024).contains(&l.max_artifact_bytes)
        || !(1..=128 * 1024 * 1024).contains(&l.max_compiler_bytes)
        || !(1..=8192).contains(&l.max_metadata_bytes)
        || !(1..=65536).contains(&l.max_environment_bytes)
        || !(PLAN_RAM as usize..=1024 * 1024).contains(&l.max_retained_bytes)
        || !(1..=60000).contains(&l.child_timeout_ms)
        || !(1..=120000).contains(&l.total_wall_ms)
    { return Err(BuildError::InputLimit); }
    if input.subject.text().is_empty() || input.subject.text().len() > l.max_subject_bytes
        || input.subject.text().len().checked_add(super::plan::DRIVER.len() + 1)
            .is_none_or(|n| n > l.max_frame_bytes)
        || input.entrypoint.len() > 128
    { return Err(BuildError::InputLimit); }
    if input.entrypoint != "checked_add" { return Err(BuildError::FrameMismatch); }
    if input.emission != Emission::LinkExecutable {
        return Err(BuildError::Unsupported("metadata emission is not a linked origin"));
    }
    if !compiler_shape(input.compiler) { return Err(BuildError::CompilerRegistrationMismatch); }
    if !absolute(input.root) || !absolute(input.frame.cwd) || input.frame.cwd != input.root {
        return Err(BuildError::FrameMismatch);
    }
    let relatives = [input.subject_relative, input.driver_relative, input.frame_relative, input.output_relative];
    for (index, path) in relatives.iter().enumerate() {
        if !relative(path) || relatives[..index].contains(path)
            || input.root.as_os_str().len().checked_add(path.as_os_str().len() + 1)
                .is_none_or(|n| n > 4096)
        { return Err(BuildError::FrameMismatch); }
    }
    input.compiler_registration.check_size_bounds().map_err(|_| BuildError::InputLimit)?;
    if input.frame.environment.len() > 32 { return Err(BuildError::InputLimit); }
    let allowed = ["PATH", "SystemRoot", "SystemDrive", "WINDIR", "TEMP", "TMP", "LIB",
        "LIBPATH", "INCLUDE", "VCINSTALLDIR", "VCToolsInstallDir", "WindowsSdkDir", "LANG"];
    let mut total = 0usize;
    let mut previous: Option<&str> = None;
    for (index, entry) in input.frame.environment.iter().enumerate() {
        if entry.name.is_empty() || entry.name.len() > 256 || !entry.name.is_ascii()
            || entry.name.contains(['=', '\0']) || entry.value.contains('\0')
            || entry.value.len() > 16384
        { return Err(BuildError::InputLimit); }
        if !allowed.contains(&entry.name) {
            return Err(BuildError::Unsupported("environment key outside registered profile"));
        }
        if previous.is_some_and(|p| p >= entry.name)
            || (cfg!(windows) && input.frame.environment[..index].iter()
                .any(|p| p.name.eq_ignore_ascii_case(entry.name)))
        { return Err(BuildError::FrameMismatch); }
        previous = Some(entry.name);
        total = total.checked_add(entry.name.len()).and_then(|v| v.checked_add(entry.value.len()))
            .ok_or(BuildError::InputLimit)?;
        if total > l.max_environment_bytes { return Err(BuildError::InputLimit); }
    }
    Ok(())
}
