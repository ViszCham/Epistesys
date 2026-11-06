//! Literal source/driver fixtures. No compiler/semantic success is fabricated.
use lc631_analysis::source_build_origin as b;
use lc631_substrate::{source::SourceStore, Digest32};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc};

pub const TASK: &str = "Please build the registered standalone source and test the package.";
pub const CHECKED: &str = "pub fn checked_add(a:u32,b:u32)->Option<u32>{a.checked_add(b)}\n";
pub const WRAPPING: &str =
    "pub fn checked_add(a:u32,b:u32)->Option<u32>{Some(a.wrapping_add(b))}\n";
// Downstream execution reuses the actual existing order, not an invented suite.
pub const DOWNSTREAM_PROBE_INPUT: &[u8] =
    lc631_runtime::contextual_repair::measurement::PROBE_INPUT;
pub const DRIVER: &str = r#"fn main(){use std::io::Read;let mut s=String::new();std::io::stdin().read_to_string(&mut s).unwrap();for line in s.lines(){let v:Vec<u32>=line.split_whitespace().map(|x|x.parse().unwrap()).collect();match checked_add(v[0],v[1]){Some(n)=>println!("some:{n}"),None=>println!("none")}}}
"#;
pub fn digest(bytes: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(bytes).into())
}
pub fn subject(text: &str) -> SourceStore {
    SourceStore::new(Arc::from(text), 16 * 1024).unwrap()
}
pub fn limits() -> b::BuildLimits {
    b::BuildLimits {
        max_builds: 1,
        max_children: 3,
        max_current_reads: 8,
        max_subject_bytes: 16 * 1024,
        max_frame_bytes: 32 * 1024,
        max_artifact_bytes: 64 * 1024 * 1024,
        max_compiler_bytes: 128 * 1024 * 1024,
        max_metadata_bytes: 8192,
        max_environment_bytes: 64 * 1024,
        max_retained_bytes: 1024 * 1024,
        child_timeout_ms: 60_000,
        total_wall_ms: 120_000,
    }
}
pub fn required(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!("selected actual build test requires {name}; no install/fallback")
    })
}
pub fn pin(name: &str) -> Digest32 {
    let raw = required(name);
    let hex = raw.strip_prefix("sha256:").unwrap_or(&raw);
    assert_eq!(hex.len(), 64);
    let mut bytes = [0u8; 32];
    for (i, part) in hex.as_bytes().chunks_exact(2).enumerate() {
        bytes[i] = u8::from_str_radix(std::str::from_utf8(part).unwrap(), 16).unwrap();
    }
    assert_ne!(bytes, [0; 32]);
    Digest32::from_bytes(bytes)
}
pub struct CompilerConfig {
    pub path: PathBuf,
    pub sysroot: PathBuf,
    pub image: Digest32,
    pub verbose: Digest32,
}
impl CompilerConfig {
    pub fn selected() -> Self {
        let path = PathBuf::from(required("EPISTESYS_SOURCE_BUILD_RUSTC"));
        let sysroot = PathBuf::from(required("EPISTESYS_SOURCE_BUILD_SYSROOT"));
        assert!(path.is_absolute() && sysroot.is_absolute());
        Self {
            path,
            sysroot,
            image: pin("EPISTESYS_SOURCE_BUILD_RUSTC_SHA256"),
            verbose: pin("EPISTESYS_SOURCE_BUILD_VERBOSE_SHA256"),
        }
    }
    pub fn spec(&self) -> b::CompilerSpec<'_> {
        b::CompilerSpec {
            executable: &self.path,
            expected_image: self.image,
            expected_verbose: self.verbose,
            declared_sysroot: &self.sysroot,
            toolchain_profile: digest(b"registered-rustc-standalone-std-link.v1"),
        }
    }
}
pub fn environment() -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for key in [
        "PATH",
        "SystemRoot",
        "SystemDrive",
        "WINDIR",
        "TEMP",
        "TMP",
        "LIB",
        "LIBPATH",
        "INCLUDE",
        "VCINSTALLDIR",
        "VCToolsInstallDir",
        "WindowsSdkDir",
        "LANG",
    ] {
        if let Ok(value) = std::env::var(key) {
            assert!(value.len() <= 16 * 1024);
            pairs.push((key.to_owned(), value));
        }
    }
    pairs.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    assert!(
        pairs.len() <= 32
            && pairs.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>() <= 64 * 1024
    );
    pairs
}
