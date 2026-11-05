//! 日本語: current_exeから同testのFPへ戻り、一意な実rlibだけを固定する。
//! English: Resolve current_exe to its own fingerprint and seal only unique actual rlibs.
use super::{
    diagnostics::{self, context, MarkerState, Reason, Rejections},
    fingerprint::{self, Candidate, Fingerprint, Kind},
    snapshot::{self, Stamp},
    sources, Error,
};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};
struct Entry {
    candidate: Candidate,
    marker: Stamp,
    metadata: Stamp,
}
struct Artifact {
    kind: Kind,
    unit: String,
    marker: u64,
    path: PathBuf,
}
struct Scan {
    entries: Vec<Entry>,
    rejected: Rejections,
}
pub struct LinkedArtifacts {
    deps: PathBuf,
    fingerprints: PathBuf,
    profile: String,
    root: Fingerprint,
    artifacts: Vec<Artifact>,
    stamps: Vec<Stamp>,
    rejected: Rejections,
}
fn unit_hash(name: &str, prefix: &str) -> Result<String, Error> {
    let hash = name.strip_prefix(prefix).ok_or(Error::InvalidCurrentExe)?;
    if hash.len() != 16
        || !hash
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(Error::InvalidCurrentExe);
    }
    Ok(hash.to_owned())
}
fn locations(exe: &Path, target: &str) -> Result<(PathBuf, PathBuf, String, String), Error> {
    if target.is_empty()
        || target.len() > 128
        || !target
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return Err(Error::InvalidCurrentExe);
    }
    let file = exe
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or(Error::InvalidCurrentExe)?;
    let file = file.strip_suffix(".exe").unwrap_or(file);
    let unit = unit_hash(file, &format!("{target}-"))?;
    let deps = exe.parent().ok_or(Error::InvalidCurrentExe)?;
    if deps.file_name() != Some(OsStr::new("deps")) {
        return Err(Error::InvalidCurrentExe);
    }
    let parent = deps.parent().ok_or(Error::InvalidCurrentExe)?;
    let profile = parent
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or(Error::InvalidCurrentExe)?;
    if !["debug", "release"].contains(&profile) {
        return Err(Error::UnsupportedFormat);
    }
    let fingerprints = parent.join(".fingerprint");
    snapshot::directory(deps)?;
    snapshot::directory(&fingerprints)?;
    Ok((deps.to_owned(), fingerprints, profile.to_owned(), unit))
}
fn root_files(
    base: &Path,
    profile: &str,
    unit: &str,
    target: &str,
) -> Result<(Fingerprint, Vec<Stamp>), Error> {
    let directory = format!("lc631-runtime-{unit}");
    let dir = base.join(&directory);
    snapshot::directory(&dir)?;
    let stem = format!("test-integration-test-{target}");
    let (marker, ms) = snapshot::small(&dir.join(&stem), format!("{directory}/{stem}"), 16)?;
    fingerprint::hex_fingerprint(&marker).map_err(|e| context("root_marker", &directory, e))?;
    let name = format!("{stem}.json");
    let (bytes, js) = snapshot::small(&dir.join(&name), format!("{directory}/{name}"), 65536)?;
    let root =
        fingerprint::parse(&bytes).map_err(|e| context("root_metadata_json", &directory, e))?;
    root.check_local(profile, &directory, &stem)
        .map_err(|e| context("root_metadata_local", &directory, e))?;
    Ok((root, vec![ms, js]))
}
fn marker_for_entry(
    root: &Fingerprint,
    dir: &Path,
    directory: &str,
    kind: Kind,
    rejected: &mut Rejections,
) -> Result<Option<(u64, Stamp)>, Error> {
    let stem = format!("lib-{}", kind.crate_name());
    let path = dir.join(&stem);
    let exists = |p: &Path| p.try_exists().map_err(|e| Error::Io(e.kind()));
    if !exists(&path)? {
        if exists(&dir.join(format!("{stem}.json")))? {
            rejected.record(kind, directory, Reason::MissingMarker)?;
        }
        return Ok(None);
    }
    let (bytes, stamp) = snapshot::small(&path, format!("{directory}/{stem}"), 16)
        .map_err(|e| context("candidate_marker_read", directory, e))?;
    let expected = root.expected_fingerprint(kind)?;
    let reason = match diagnostics::classify_marker(&bytes, expected)? {
        MarkerState::Exact => return Ok(Some((expected, stamp))),
        MarkerState::Foreign => Reason::ForeignMarker,
        MarkerState::Malformed => Reason::MalformedMarker,
    };
    rejected.record(kind, directory, reason)?;
    Ok(None)
}
fn entry(
    root: &Fingerprint,
    base: &Path,
    profile: &str,
    directory: &str,
    kind: Kind,
    unit: String,
    rejected: &mut Rejections,
) -> Result<Option<Entry>, Error> {
    let dir = base.join(directory);
    snapshot::directory(&dir).map_err(|e| context("candidate_directory", directory, e))?;
    let Some((value, marker)) = marker_for_entry(root, &dir, directory, kind, rejected)? else {
        return Ok(None);
    };
    // Only exact matching marker bytes reach strict selected-metadata admission.
    let stem = format!("lib-{}", kind.crate_name());
    let metadata_path = dir.join(format!("{stem}.json"));
    let (bytes, metadata) =
        snapshot::small(&metadata_path, format!("{directory}/{stem}.json"), 65536)
            .map_err(|e| context("selected_metadata_read", directory, e))?;
    let parsed = diagnostics::parse_selected_metadata(&bytes, profile, directory, &stem)?;
    Ok(Some(Entry {
        candidate: Candidate {
            kind,
            unit,
            marker: value,
            metadata: parsed,
        },
        marker,
        metadata,
    }))
}
fn scan(root: &Fingerprint, base: &Path, profile: &str) -> Result<Scan, Error> {
    let rows = std::fs::read_dir(base).map_err(|e| Error::Io(e.kind()))?;
    let mut result: Vec<Entry> = Vec::new();
    let mut rejected = Rejections::default();
    let mut visited = 0usize;
    for row in rows {
        visited += 1;
        if visited > 16384 {
            return Err(Error::Limit);
        }
        let row = row.map_err(|e| Error::Io(e.kind()))?;
        let name = row.file_name();
        let Some(name) = name.to_str() else {
            return Err(Error::UnsupportedFormat);
        };
        for kind in Kind::ALL {
            let prefix = format!("{}-", kind.package());
            if name.starts_with(&prefix) {
                let unit = unit_hash(name, &prefix)?;
                if let Some(found) = entry(root, base, profile, name, kind, unit, &mut rejected)? {
                    if result.iter().any(|e| e.candidate.kind == kind) {
                        return Err(Error::AmbiguousArtifact(kind));
                    }
                    result.push(found);
                }
            }
        }
    }
    Ok(Scan {
        entries: result,
        rejected,
    })
}
fn choose(root: &Fingerprint, entries: &[Entry]) -> Result<Vec<usize>, Error> {
    let candidates: Vec<_> = entries.iter().map(|e| e.candidate.clone()).collect();
    let mut indices = Vec::new();
    for kind in Kind::ALL {
        indices.push(fingerprint::select(root, kind, &candidates)?);
    }
    let selected: Vec<_> = indices.iter().map(|i| candidates[*i].clone()).collect();
    fingerprint::verify_chain(root, &selected)?;
    Ok(indices)
}
impl LinkedArtifacts {
    pub fn current(target: &str) -> Result<Self, Error> {
        let exe = std::env::current_exe().map_err(|e| Error::Io(e.kind()))?;
        let (deps, fingerprints, profile, unit) = locations(&exe, target)?;
        let (root, mut stamps) = root_files(&fingerprints, &profile, &unit, target)?;
        stamps.push(Stamp::capture(
            exe,
            format!("test:{target}-{unit}"),
            snapshot::MAX_ARTIFACT,
        )?);
        let scan = scan(&root, &fingerprints, &profile)?;
        let entries = scan.entries;
        let indices =
            choose(&root, &entries).map_err(|e| context("select_current_set", target, e))?;
        let mut artifacts = Vec::new();
        for index in indices {
            let candidate = &entries[index].candidate;
            let name = format!("lib{}-{}.rlib", candidate.kind.crate_name(), candidate.unit);
            let path = deps.join(&name);
            stamps.push(Stamp::capture(
                path.clone(),
                format!("deps/{name}"),
                snapshot::MAX_ARTIFACT,
            )?);
            artifacts.push(Artifact {
                kind: candidate.kind,
                unit: candidate.unit.clone(),
                marker: candidate.marker,
                path,
            });
        }
        // Preserve the exact selected metadata bytes, not stale candidate reports.
        for entry in entries {
            stamps.push(entry.marker);
            stamps.push(entry.metadata);
        }
        stamps.extend(sources::capture()?);
        let linked = Self {
            deps,
            fingerprints,
            profile,
            root,
            artifacts,
            stamps,
            rejected: scan.rejected,
        };
        linked.verify_current()?;
        Ok(linked)
    }
    pub fn verify_current(&self) -> Result<(), Error> {
        for stamp in &self.stamps {
            stamp.verify()?;
        }
        // A new same-fingerprint match must not become an unnoticed ambiguity.
        let scan = scan(&self.root, &self.fingerprints, &self.profile)?;
        let entries = scan.entries;
        for index in choose(&self.root, &entries)? {
            let row = &entries[index].candidate;
            if !self
                .artifacts
                .iter()
                .any(|a| a.kind == row.kind && a.unit == row.unit)
            {
                return Err(Error::ArtifactChanged);
            }
        }
        Ok(())
    }
    pub fn deps(&self) -> &Path {
        &self.deps
    }
    pub fn externs(&self) -> impl Iterator<Item = (&'static str, &Path)> {
        self.artifacts
            .iter()
            .map(|a| (a.kind.crate_name(), a.path.as_path()))
    }
    pub fn receipt(&self) -> Result<String, Error> {
        let artifacts: Vec<_> = self
            .artifacts
            .iter()
            .map(|a| {
                serde_json::json!({
                    "crate":a.kind.crate_name(),"unit":a.unit,
                    "dependency_fingerprint":a.marker
                })
            })
            .collect();
        let mut ordered: Vec<_> = self.stamps.iter().collect();
        ordered.sort_by(|a, b| a.label().cmp(b.label()));
        let stamps: Vec<_> = ordered.into_iter().map(Stamp::summary).collect();
        let mut receipt = serde_json::json!({"kind":"current-test-compiled-artifact-snapshot",
            "cargo_format":"observed-internal-not-stable-public-API","root":self.root.summary(),
            "artifacts":artifacts,"content_pins":stamps,
            "independent_proof":false,"source_library_origin_authentication":false});
        let digest = snapshot::hash(receipt.to_string().as_bytes());
        let revision: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        receipt["snapshot_revision_sha256"] = serde_json::Value::String(revision);
        receipt["unselected_candidate_rejections"] = self.rejected.summary();
        receipt["rejection_diagnostics_in_snapshot_revision"] = serde_json::Value::Bool(false);
        let text = receipt.to_string();
        if text.len() > 65536 {
            return Err(Error::Limit);
        }
        Ok(text)
    }
}
