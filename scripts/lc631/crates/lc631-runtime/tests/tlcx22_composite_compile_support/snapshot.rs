//! 日本語: artifactとmetadataの内容を前後で固定。mtime順や途中変更を採用しない。
//! English: Seal artifact/metadata content before and after, without mtime ranking or change acceptance.
use super::{check_digest, Error};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, Metadata},
    io::Read,
    path::{Path, PathBuf},
};
pub(super) const MAX_ARTIFACT: u64 = 512 * 1024 * 1024;
pub(super) struct Stamp {
    path: PathBuf,
    label: String,
    bytes: u64,
    digest: [u8; 32],
    limit: u64,
}
fn regular(path: &Path) -> Result<(), Error> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| Error::Io(e.kind()))?;
    if !meta.is_file() || reparse(&meta) {
        return Err(Error::InvalidFile);
    }
    Ok(())
}
fn reparse(meta: &Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}
pub(super) fn directory(path: &Path) -> Result<(), Error> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| Error::Io(e.kind()))?;
    if !meta.is_dir() || reparse(&meta) {
        return Err(Error::InvalidFile);
    }
    Ok(())
}
fn open(path: &Path, limit: u64) -> Result<File, Error> {
    regular(path)?;
    let file = File::open(path).map_err(|e| Error::Io(e.kind()))?;
    let meta = file.metadata().map_err(|e| Error::Io(e.kind()))?;
    if !meta.is_file() || reparse(&meta) {
        return Err(Error::InvalidFile);
    }
    if meta.len() > limit {
        return Err(Error::Limit);
    }
    Ok(file)
}
pub(super) fn digest(path: &Path, limit: u64) -> Result<(u64, [u8; 32]), Error> {
    let mut file = open(path, limit)?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer).map_err(|e| Error::Io(e.kind()))?;
        if n == 0 {
            break;
        }
        total = total.checked_add(n as u64).ok_or(Error::Limit)?;
        if total > limit {
            return Err(Error::Limit);
        }
        hash.update(&buffer[..n]);
    }
    regular(path)?;
    Ok((total, hash.finalize().into()))
}
pub(super) fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
pub(super) fn small(path: &Path, label: String, limit: usize) -> Result<(Vec<u8>, Stamp), Error> {
    let mut file = open(path, limit as u64)?;
    let mut result = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        let n = file.read(&mut buffer).map_err(|e| Error::Io(e.kind()))?;
        if n == 0 {
            break;
        }
        let size = result.len().checked_add(n).ok_or(Error::Limit)?;
        if size > limit {
            return Err(Error::Limit);
        }
        result
            .try_reserve_exact(n)
            .map_err(|_| Error::AllocationFailed)?;
        result.extend_from_slice(&buffer[..n]);
    }
    regular(path)?;
    let stamp = Stamp {
        path: path.to_owned(),
        label,
        bytes: result.len() as u64,
        digest: hash(&result),
        limit: limit as u64,
    };
    stamp.verify()?;
    Ok((result, stamp))
}
impl Stamp {
    pub(super) fn label(&self) -> &str {
        &self.label
    }
    pub(super) fn matches(&self, bytes: u64, digest: [u8; 32]) -> bool {
        self.bytes == bytes && self.digest == digest
    }
    pub(super) fn capture(path: PathBuf, label: String, limit: u64) -> Result<Self, Error> {
        let (bytes, digest) = digest(&path, limit)?;
        Ok(Self {
            path,
            label,
            bytes,
            digest,
            limit,
        })
    }
    pub(super) fn verify(&self) -> Result<(), Error> {
        let (bytes, digest) = digest(&self.path, self.limit)?;
        if bytes != self.bytes {
            return Err(Error::ArtifactChanged);
        }
        check_digest(self.digest, digest)
    }
    pub(super) fn summary(&self) -> serde_json::Value {
        let hex: String = self.digest.iter().map(|b| format!("{b:02x}")).collect();
        serde_json::json!({"label":self.label,"bytes":self.bytes,"sha256":hex})
    }
}
