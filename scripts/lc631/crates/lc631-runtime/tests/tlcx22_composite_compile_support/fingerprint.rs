//! 日本語: 観測Cargo内部formatだけを読む。opaque fingerprintはsource証明ではない。
//! English: Read the observed internal Cargo format only; opaque fingerprints are not source proof.
use super::Error;
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Runtime,
    Analysis,
    Substrate,
    Json,
}
impl Kind {
    pub const ALL: [Self; 4] = [Self::Runtime, Self::Analysis, Self::Substrate, Self::Json];
    pub fn crate_name(self) -> &'static str {
        match self {
            Self::Runtime => "lc631_runtime",
            Self::Analysis => "lc631_analysis",
            Self::Substrate => "lc631_substrate",
            Self::Json => "serde_json",
        }
    }
    pub fn package(self) -> &'static str {
        match self {
            Self::Runtime => "lc631-runtime",
            Self::Analysis => "lc631-analysis",
            Self::Substrate => "lc631-substrate",
            Self::Json => "serde_json",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Dependency {
    package: u64,
    name: String,
    flag: bool,
    fingerprint: u64,
}
#[derive(Clone, Debug)]
pub struct Fingerprint {
    rustc: u64,
    config: u64,
    compile_kind: u64,
    target: u64,
    profile: u64,
    path: u64,
    features: Vec<String>,
    declared_features: Vec<String>,
    rustflags: Vec<String>,
    deps: Vec<Dependency>,
    local: String,
}
#[derive(Clone, Debug)]
pub struct Candidate {
    pub kind: Kind,
    pub unit: String,
    pub marker: u64,
    pub metadata: Fingerprint,
}
fn number(v: &Value, key: &str) -> Result<u64, Error> {
    v.get(key)
        .and_then(Value::as_u64)
        .ok_or(Error::UnsupportedFormat)
}
fn strings(v: &Value) -> Result<Vec<String>, Error> {
    let rows = v.as_array().ok_or(Error::UnsupportedFormat)?;
    if rows.len() > 128 {
        return Err(Error::Limit);
    }
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for row in rows {
        let text = row.as_str().ok_or(Error::UnsupportedFormat)?;
        if text.len() > 256 || !seen.insert(text) {
            return Err(Error::UnsupportedFormat);
        }
        result.push(text.to_owned());
    }
    Ok(result)
}
fn features(v: &Value, key: &str) -> Result<Vec<String>, Error> {
    let text = v
        .get(key)
        .and_then(Value::as_str)
        .ok_or(Error::UnsupportedFormat)?;
    let value: Value = serde_json::from_str(text).map_err(|_| Error::UnsupportedFormat)?;
    strings(&value)
}
fn local(v: &Value) -> Result<String, Error> {
    let rows = v
        .get("local")
        .and_then(Value::as_array)
        .ok_or(Error::UnsupportedFormat)?;
    if rows.len() != 1 {
        return Err(Error::UnsupportedFormat);
    }
    let row = rows[0].as_object().ok_or(Error::UnsupportedFormat)?;
    if row.len() != 1 {
        return Err(Error::UnsupportedFormat);
    }
    let data = row
        .get("CheckDepInfo")
        .and_then(Value::as_object)
        .ok_or(Error::UnsupportedFormat)?;
    if data.len() != 2 || data.get("checksum") != Some(&Value::Bool(false)) {
        return Err(Error::UnsupportedFormat);
    }
    let path = data
        .get("dep_info")
        .and_then(Value::as_str)
        .ok_or(Error::UnsupportedFormat)?;
    if path.is_empty() || path.len() > 1024 {
        return Err(Error::UnsupportedFormat);
    }
    Ok(path.to_owned())
}
fn dependencies(v: &Value) -> Result<Vec<Dependency>, Error> {
    let rows = v
        .get("deps")
        .and_then(Value::as_array)
        .ok_or(Error::UnsupportedFormat)?;
    if rows.len() > 128 {
        return Err(Error::Limit);
    }
    let mut result = Vec::new();
    let mut names = BTreeSet::new();
    for row in rows {
        let row = row.as_array().ok_or(Error::UnsupportedFormat)?;
        if row.len() != 4 {
            return Err(Error::UnsupportedFormat);
        }
        let name = row[1].as_str().ok_or(Error::UnsupportedFormat)?;
        if name.is_empty() || name.len() > 128 {
            return Err(Error::UnsupportedFormat);
        }
        if !names.insert(name) {
            return Err(Error::DuplicateDependency);
        }
        result.push(Dependency {
            package: row[0].as_u64().ok_or(Error::UnsupportedFormat)?,
            name: name.to_owned(),
            flag: row[2].as_bool().ok_or(Error::UnsupportedFormat)?,
            fingerprint: row[3].as_u64().ok_or(Error::UnsupportedFormat)?,
        });
    }
    Ok(result)
}
pub fn parse(bytes: &[u8]) -> Result<Fingerprint, Error> {
    if bytes.len() > 65536 {
        return Err(Error::Limit);
    }
    let v: Value = serde_json::from_slice(bytes).map_err(|_| Error::UnsupportedFormat)?;
    unique_keys(bytes)?;
    let object = v.as_object().ok_or(Error::UnsupportedFormat)?;
    const KEYS: [&str; 11] = [
        "rustc",
        "features",
        "declared_features",
        "target",
        "profile",
        "path",
        "deps",
        "local",
        "rustflags",
        "config",
        "compile_kind",
    ];
    if object.len() != KEYS.len() || object.keys().any(|k| !KEYS.contains(&k.as_str())) {
        return Err(Error::UnsupportedFormat);
    }
    Ok(Fingerprint {
        rustc: number(&v, "rustc")?,
        config: number(&v, "config")?,
        compile_kind: number(&v, "compile_kind")?,
        target: number(&v, "target")?,
        profile: number(&v, "profile")?,
        path: number(&v, "path")?,
        features: features(&v, "features")?,
        declared_features: features(&v, "declared_features")?,
        rustflags: strings(v.get("rustflags").ok_or(Error::UnsupportedFormat)?)?,
        deps: dependencies(&v)?,
        local: local(&v)?,
    })
}
impl Fingerprint {
    pub fn expected_fingerprint(&self, kind: Kind) -> Result<u64, Error> {
        Ok(self.dependency(kind)?.fingerprint)
    }
    fn dependency(&self, kind: Kind) -> Result<&Dependency, Error> {
        self.deps
            .iter()
            .find(|d| d.name == kind.crate_name())
            .ok_or(Error::MissingDependency(kind.crate_name()))
    }
    pub fn check_local(&self, profile: &str, directory: &str, stem: &str) -> Result<(), Error> {
        let actual = self.local.replace('\\', "/");
        let expected = format!("{profile}/.fingerprint/{directory}/dep-{stem}");
        if actual != expected {
            return Err(Error::UnsupportedFormat);
        }
        Ok(())
    }
    pub fn summary(&self) -> Value {
        json!({"rustc":self.rustc,"config":self.config,"compile_kind":self.compile_kind,
            "target":self.target,"profile":self.profile,"path":self.path,
            "features":self.features,"declared_features":self.declared_features,
            "rustflags":self.rustflags,"local":self.local})
    }
}
pub fn hex_fingerprint(bytes: &[u8]) -> Result<u64, Error> {
    if bytes.len() != 16
        || !bytes
            .iter()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c))
    {
        return Err(Error::UnsupportedFormat);
    }
    let mut value = [0; 8];
    for (index, pair) in bytes.as_chunks::<2>().0.iter().enumerate() {
        let digit = |c: u8| if c <= b'9' { c - b'0' } else { c - b'a' + 10 };
        value[index] = (digit(pair[0]) << 4) | digit(pair[1]);
    }
    Ok(u64::from_le_bytes(value))
}
pub fn select(root: &Fingerprint, kind: Kind, candidates: &[Candidate]) -> Result<usize, Error> {
    let expected = root.dependency(kind)?;
    let mut found = None;
    for (index, row) in candidates.iter().enumerate() {
        if row.kind == kind && row.marker == expected.fingerprint {
            if found.is_some() {
                return Err(Error::AmbiguousArtifact(kind));
            }
            found = Some(index);
        }
    }
    let index = found.ok_or(Error::MissingArtifact(kind))?;
    let actual = &candidates[index].metadata;
    if root.rustc != actual.rustc
        || root.config != actual.config
        || root.compile_kind != actual.compile_kind
        || root.rustflags != actual.rustflags
        || (kind == Kind::Runtime
            && (root.features != actual.features
                || root.declared_features != actual.declared_features))
    {
        return Err(Error::ContextMismatch);
    }
    Ok(index)
}
pub fn verify_chain(root: &Fingerprint, selected: &[Candidate]) -> Result<(), Error> {
    for unit in selected {
        for kind in Kind::ALL {
            if let Some(dep) = unit
                .metadata
                .deps
                .iter()
                .find(|d| d.name == kind.crate_name())
            {
                if dep != root.dependency(kind)? {
                    return Err(Error::DependencyMismatch);
                }
            }
        }
    }
    if selected.len() != 4 {
        return Err(Error::DependencyMismatch);
    }
    for (owner, dependency) in [
        (Kind::Runtime, Kind::Analysis),
        (Kind::Runtime, Kind::Substrate),
        (Kind::Analysis, Kind::Substrate),
        (Kind::Analysis, Kind::Json),
    ] {
        let unit = selected
            .iter()
            .find(|u| u.kind == owner)
            .ok_or(Error::MissingArtifact(owner))?;
        if unit.metadata.dependency(dependency)? != root.dependency(dependency)? {
            return Err(Error::DependencyMismatch);
        }
    }
    Ok(())
}
fn unique_keys(bytes: &[u8]) -> Result<(), Error> {
    // Audit keys after serde_json validates syntax, rather than accepting last-key-wins.
    // Each object has a separate key set; arrays and escaped string contents are skipped.
    let mut objects: Vec<BTreeSet<String>> = Vec::new();
    let mut at = 0usize;
    while at < bytes.len() {
        match bytes[at] {
            b'{' => {
                if objects.len() >= 32 {
                    return Err(Error::Limit);
                }
                objects.push(BTreeSet::new());
                at += 1;
            }
            b'}' => {
                objects.pop().ok_or(Error::UnsupportedFormat)?;
                at += 1;
            }
            b'"' => {
                let start = at;
                at += 1;
                while at < bytes.len() && bytes[at] != b'"' {
                    at += if bytes[at] == b'\\' { 2 } else { 1 };
                }
                if at >= bytes.len() {
                    return Err(Error::UnsupportedFormat);
                }
                at += 1;
                let end = at;
                while at < bytes.len() && bytes[at].is_ascii_whitespace() {
                    at += 1;
                }
                if bytes.get(at) == Some(&b':') {
                    let key: String = serde_json::from_slice(&bytes[start..end])
                        .map_err(|_| Error::UnsupportedFormat)?;
                    let keys = objects.last_mut().ok_or(Error::UnsupportedFormat)?;
                    if !keys.insert(key) {
                        return Err(Error::DuplicateKey);
                    }
                    if keys.len() > 128 {
                        return Err(Error::Limit);
                    }
                }
            }
            _ => at += 1,
        }
    }
    if !objects.is_empty() {
        return Err(Error::UnsupportedFormat);
    }
    Ok(())
}
