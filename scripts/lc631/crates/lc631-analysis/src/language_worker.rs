use crate::{run_bounded_tool, BoundedToolRequest, BoundedToolResult, ToolRunState};
use lc631_core::stable_sha256;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

const SCHEMA: &str = "epistesys-dgcl-nlp.v1";
const MAX_SOURCE_BYTES: usize = 262_144;
const MAX_MANIFEST_BYTES: u64 = 65_536;
const MAX_REQUIREMENTS_BYTES: u64 = 65_536;
const MAX_RESPONSE_BYTES: usize = 2_097_152;

#[derive(Clone, Debug)]
pub struct LanguageWorkerConfig {
    python: PathBuf,
    expected_python_digest: String,
    script: PathBuf,
    expected_script_digest: String,
    model_dir: PathBuf,
    manifest: PathBuf,
    expected_manifest_digest: String,
    runtime_manifest: PathBuf,
    expected_runtime_manifest_digest: String,
    requirements_lock: PathBuf,
    expected_requirements_lock_digest: String,
    timeout_ms: u64,
}

#[derive(Clone, Debug)]
pub struct DgclRuntimePins {
    python: PathBuf,
    python_digest: String,
    worker: PathBuf,
    worker_digest: String,
    model_root: PathBuf,
    model_manifests: BTreeMap<String, (PathBuf, String)>,
    runtime_manifest: PathBuf,
    runtime_manifest_digest: String,
    requirements_lock: PathBuf,
    requirements_lock_digest: String,
}

impl DgclRuntimePins {
    pub fn bind_pinned_worker(
        &self,
        language: &str,
        timeout_ms: u64,
    ) -> Result<LanguageWorkerConfig, LanguageWorkerError> {
        let Some((manifest, manifest_digest)) = self.model_manifests.get(language) else {
            return Err(LanguageWorkerError::InvalidLanguage);
        };
        if !(1..=60_000).contains(&timeout_ms) {
            return Err(LanguageWorkerError::RuntimeManifest(
                "worker_timeout_out_of_range".into(),
            ));
        }
        Ok(LanguageWorkerConfig {
            python: self.python.clone(),
            expected_python_digest: self.python_digest.clone(),
            script: self.worker.clone(),
            expected_script_digest: self.worker_digest.clone(),
            model_dir: self.model_root.clone(),
            manifest: manifest.clone(),
            expected_manifest_digest: manifest_digest.clone(),
            runtime_manifest: self.runtime_manifest.clone(),
            expected_runtime_manifest_digest: self.runtime_manifest_digest.clone(),
            requirements_lock: self.requirements_lock.clone(),
            expected_requirements_lock_digest: self.requirements_lock_digest.clone(),
            timeout_ms,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn bind_worker(
        &self,
        language: &str,
        python: &Path,
        python_digest: Option<String>,
        script: &Path,
        script_digest: String,
        model_root: &Path,
        model_manifest: &Path,
        model_manifest_digest: String,
        timeout_ms: u64,
    ) -> Result<LanguageWorkerConfig, LanguageWorkerError> {
        let Some((expected_model_manifest, expected_manifest_digest)) =
            self.model_manifests.get(language)
        else {
            return Err(LanguageWorkerError::InvalidLanguage);
        };
        if !(1..=60_000).contains(&timeout_ms)
            || canonical_path(python)? != self.python
            || python_digest.as_deref() != Some(self.python_digest.as_str())
            || canonical_path(script)? != self.worker
            || script_digest != self.worker_digest
            || canonical_path(model_root)? != self.model_root
            || canonical_path(model_manifest)? != *expected_model_manifest
            || model_manifest_digest != *expected_manifest_digest
        {
            return Err(LanguageWorkerError::RuntimeManifest(
                "caller_paths_or_digests_do_not_match_repository_pins".into(),
            ));
        }
        let _ = python_digest;
        self.bind_pinned_worker(language, timeout_ms)
    }
}

pub fn load_dgcl_runtime_pins(repo_root: &Path) -> Result<DgclRuntimePins, LanguageWorkerError> {
    if !cfg!(windows) {
        return Err(LanguageWorkerError::RuntimeManifest(
            "alpha2_runtime_profile_requires_windows_x86_64".into(),
        ));
    }
    let root = canonical_path(repo_root)?;
    if !fs::metadata(&root)
        .map_err(|_| LanguageWorkerError::RuntimeManifest("repo_root_unavailable".into()))?
        .is_dir()
    {
        return Err(LanguageWorkerError::RuntimeManifest(
            "repo_root_not_directory".into(),
        ));
    }
    let pin_path = checked_relative_path(&root, "validation/dgcl-nlp-model-pins.v1.json")?;
    let pin_raw = read_bounded_text(&pin_path, MAX_MANIFEST_BYTES)?;
    let pins: DgclPinFile = serde_json::from_str(&pin_raw)
        .map_err(|_| LanguageWorkerError::RuntimeManifest("pin_manifest_invalid".into()))?;
    if pins.schema_version != "epistesys-dgcl-model-pins.v1"
        || pins.stanza_version != "1.14.0"
        || !valid_digest(&pins.worker_script_digest)
        || !valid_digest(&pins.python_executable_observed_digest)
        || pins.model_root != ".cache/dgcl-models"
        || pins.license_record.trim().is_empty()
        || pins.license_boundary.trim().is_empty()
        || pins.manifests.len() != 2
        || !pins.manifests.contains_key("en")
        || !pins.manifests.contains_key("ja")
        || !valid_digest(&pins.manifests["en"])
        || !valid_digest(&pins.manifests["ja"])
        || !valid_relative_path(&pins.runtime_manifest.path)
        || !valid_digest(&pins.runtime_manifest.sha256)
        || !valid_relative_path(&pins.requirements_lock.path)
        || !valid_digest(&pins.requirements_lock.sha256)
    {
        return Err(LanguageWorkerError::RuntimeManifest(
            "pin_manifest_profile_invalid".into(),
        ));
    }

    let runtime_manifest = checked_relative_path(&root, &pins.runtime_manifest.path)?;
    let runtime_raw = read_bounded_text(&runtime_manifest, MAX_MANIFEST_BYTES)?;
    if stable_sha256(&runtime_raw) != pins.runtime_manifest.sha256 {
        return Err(LanguageWorkerError::RuntimeManifest(
            "runtime_manifest_digest_mismatch".into(),
        ));
    }
    let runtime: RuntimeManifest = serde_json::from_str(&runtime_raw)
        .map_err(|_| LanguageWorkerError::RuntimeManifest("runtime_manifest_invalid".into()))?;
    let requirements_lock = checked_relative_path(&root, &pins.requirements_lock.path)?;
    if stable_sha256(&read_bounded_text(&requirements_lock, 65_536)?)
        != pins.requirements_lock.sha256
    {
        return Err(LanguageWorkerError::RuntimeManifest(
            "requirements_lock_digest_mismatch".into(),
        ));
    }
    let requirements_count = validate_requirements_lock(&requirements_lock)?;
    let python = checked_relative_path(&root, &runtime.python.executable_path)?;
    let worker = checked_relative_path(&root, &runtime.worker.path)?;
    let model_root = checked_relative_path(&root, &pins.model_root)?;
    let manifest_paths = [("en", "manifest-en.json"), ("ja", "manifest-ja.json")]
        .into_iter()
        .map(|(language, path)| {
            let relative = format!("{}/{path}", pins.model_root);
            Ok((
                language.to_string(),
                checked_relative_path(&root, &relative)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, LanguageWorkerError>>()?;

    if runtime.schema_version != "epistesys-dgcl-python-runtime.v1"
        || runtime.target_platform != "windows_x86_64"
        || runtime.python.implementation != "CPython"
        || runtime.python.version != "3.12.10"
        || runtime.python.executable_path != ".cache/dgcl-python/Scripts/python.exe"
        || runtime.python.executable_sha256 != pins.python_executable_observed_digest
        || !runtime.python.isolated_mode
        || pins.runtime_manifest.path != "validation/dgcl-python-runtime.v1.json"
        || pins.requirements_lock.path != "validation/dgcl-python-requirements.lock"
        || runtime.requirements_lock.path != pins.requirements_lock.path
        || runtime.requirements_lock.sha256 != pins.requirements_lock.sha256
        || runtime.requirements_lock.distribution_count != requirements_count
        || runtime.requirements_lock.comparison != "exact_normalized_name_and_version_set"
        || runtime.worker.path != "scripts/lc631/workers/dgcl_nlp_worker.py"
        || runtime.worker.sha256 != pins.worker_script_digest
        || runtime.stanza.version != pins.stanza_version
        || runtime.stanza.package != "default"
        || runtime.stanza.processors != ["tokenize", "pos", "lemma", "depparse"]
        || runtime.stanza.device != "cpu"
        || runtime.stanza.download_method != "none"
        || runtime.models.en_manifest_sha256 != pins.manifests["en"]
        || runtime.models.ja_manifest_sha256 != pins.manifests["ja"]
        || runtime.models.model_files_bundled
        || runtime.network_boundary.model_download_during_parse
        || !runtime.network_boundary.offline_environment_hints
        || runtime.network_boundary.os_network_isolation_enforced
        || runtime.claim_boundary.trim().is_empty()
        || digest_file(&python).map_err(|_| {
            LanguageWorkerError::RuntimeManifest("python_executable_unreadable".into())
        })? != pins.python_executable_observed_digest
        || digest_file(&worker)
            .map_err(|_| LanguageWorkerError::RuntimeManifest("worker_script_unreadable".into()))?
            != pins.worker_script_digest
    {
        return Err(LanguageWorkerError::RuntimeManifest(
            "runtime_manifest_profile_mismatch".into(),
        ));
    }
    Ok(DgclRuntimePins {
        python,
        python_digest: pins.python_executable_observed_digest,
        worker,
        worker_digest: pins.worker_script_digest,
        model_root,
        model_manifests: manifest_paths
            .into_iter()
            .map(|(language, path)| {
                let digest = pins.manifests[&language].clone();
                (language, (path, digest))
            })
            .collect(),
        runtime_manifest,
        runtime_manifest_digest: pins.runtime_manifest.sha256,
        requirements_lock,
        requirements_lock_digest: pins.requirements_lock.sha256,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DgclPinFile {
    schema_version: String,
    stanza_version: String,
    worker_script_digest: String,
    python_executable_observed_digest: String,
    model_root: String,
    manifests: BTreeMap<String, String>,
    runtime_manifest: PinnedFile,
    requirements_lock: PinnedFile,
    license_record: String,
    license_boundary: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PinnedFile {
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeManifest {
    schema_version: String,
    target_platform: String,
    python: RuntimePython,
    requirements_lock: RuntimeLock,
    worker: RuntimeWorker,
    stanza: RuntimeStanza,
    models: RuntimeModels,
    network_boundary: RuntimeNetworkBoundary,
    claim_boundary: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimePython {
    implementation: String,
    version: String,
    executable_path: String,
    executable_sha256: String,
    isolated_mode: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeLock {
    path: String,
    sha256: String,
    distribution_count: usize,
    comparison: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeWorker {
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeStanza {
    version: String,
    package: String,
    processors: Vec<String>,
    device: String,
    download_method: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeModels {
    en_manifest_sha256: String,
    ja_manifest_sha256: String,
    model_files_bundled: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeNetworkBoundary {
    model_download_during_parse: bool,
    offline_environment_hints: bool,
    os_network_isolation_enforced: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelManifest {
    schema_version: String,
    stanza_version: String,
    language: String,
    package: String,
    license_name: String,
    license_url: String,
    files: Vec<ModelFile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelFile {
    path: String,
    sha256: String,
}

#[derive(Clone, Debug, Serialize)]
struct WorkerRequest<'a> {
    schema_version: &'static str,
    request_id: &'a str,
    language: &'a str,
    source_revision: &'a str,
    text: &'a str,
    model_manifest_digest: &'a str,
    package: &'a str,
    runtime_manifest_digest: &'a str,
    requirements_lock_digest: &'a str,
    python_executable_digest: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerResponse {
    schema_version: String,
    request_id: String,
    language: String,
    source_revision: String,
    model_manifest_digest: String,
    runtime_manifest_digest: String,
    requirements_lock_digest: String,
    python_version: String,
    backend_revision: String,
    sentences: Vec<WorkerSentence>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerSentence {
    tokens: Vec<WorkerToken>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerToken {
    text: String,
    start_char: usize,
    end_char: usize,
    words: Vec<WorkerWord>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerWord {
    id: usize,
    text: String,
    head: usize,
    deprel: String,
    upos: Option<String>,
    lemma: Option<String>,
    feats: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DependencyWord {
    pub id: usize,
    pub text: String,
    pub head: usize,
    pub deprel: String,
    pub upos: Option<String>,
    pub lemma: Option<String>,
    pub feats: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DependencyToken {
    pub text: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub words: Vec<DependencyWord>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DependencySentence {
    pub tokens: Vec<DependencyToken>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LanguageObservation {
    pub schema_version: &'static str,
    pub source_revision: String,
    pub language: String,
    pub backend_revision: String,
    pub model_manifest_digest: String,
    pub runtime_manifest_digest: String,
    pub requirements_lock_digest: String,
    pub python_version: String,
    pub payload_digest: String,
    pub sentences: Vec<DependencySentence>,
    pub process: BoundedToolResult,
    pub semantic_truth_claim: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum LanguageWorkerError {
    InvalidLanguage,
    SourceBudget,
    Manifest(String),
    RuntimeManifest(String),
    ModelFile(String),
    RequestEncoding,
    WorkerUnavailable(String),
    ResponseFraming,
    ResponseBinding,
    InvalidOffset,
    InvalidDependency,
}

pub fn run_language_worker(
    source: &str,
    language: &str,
    config: &LanguageWorkerConfig,
) -> Result<LanguageObservation, LanguageWorkerError> {
    if !matches!(language, "ja" | "en") {
        return Err(LanguageWorkerError::InvalidLanguage);
    }
    if source.len() > MAX_SOURCE_BYTES {
        return Err(LanguageWorkerError::SourceBudget);
    }
    let python_version = validate_runtime_binding(config)?;
    let manifest = validate_manifest(config, language)?;
    if !valid_digest(&config.expected_script_digest)
        || hash_file(&config.script)
            .map_err(|_| LanguageWorkerError::Manifest("script_unavailable".into()))?
            != config.expected_script_digest
    {
        return Err(LanguageWorkerError::Manifest(
            "script_digest_mismatch".into(),
        ));
    }
    let source_revision = stable_sha256(source);
    let request_id = stable_sha256(&format!(
        "{}\0{}\0{}\0{}\0{}\0{}",
        source_revision,
        language,
        config.expected_manifest_digest,
        config.expected_runtime_manifest_digest,
        config.expected_requirements_lock_digest,
        config.expected_python_digest
    ));
    let request = WorkerRequest {
        schema_version: SCHEMA,
        request_id: &request_id,
        language,
        source_revision: &source_revision,
        text: source,
        model_manifest_digest: &config.expected_manifest_digest,
        package: &manifest.package,
        runtime_manifest_digest: &config.expected_runtime_manifest_digest,
        requirements_lock_digest: &config.expected_requirements_lock_digest,
        python_executable_digest: &config.expected_python_digest,
    };
    let payload = serde_json::to_vec(&request).map_err(|_| LanguageWorkerError::RequestEncoding)?;
    if payload.is_empty() || payload.len() > 270_336 {
        return Err(LanguageWorkerError::SourceBudget);
    }
    let length = u32::try_from(payload.len()).map_err(|_| LanguageWorkerError::SourceBudget)?;
    let mut framed = Vec::with_capacity(payload.len() + 4);
    framed.extend_from_slice(&length.to_be_bytes());
    framed.extend_from_slice(&payload);
    let python = config
        .python
        .to_str()
        .ok_or(LanguageWorkerError::RequestEncoding)?;
    let script = config
        .script
        .to_str()
        .ok_or(LanguageWorkerError::RequestEncoding)?;
    let model_dir = config
        .model_dir
        .to_str()
        .ok_or(LanguageWorkerError::RequestEncoding)?;
    let isolated_flag = "-I";
    let runtime_manifest = config
        .runtime_manifest
        .to_str()
        .ok_or(LanguageWorkerError::RequestEncoding)?;
    let requirements_lock = config
        .requirements_lock
        .to_str()
        .ok_or(LanguageWorkerError::RequestEncoding)?;
    let process = run_bounded_tool(BoundedToolRequest {
        program: python,
        args: &[
            isolated_flag,
            script,
            model_dir,
            runtime_manifest,
            requirements_lock,
        ],
        stdin: Some(&framed),
        timeout_ms: config.timeout_ms,
        stdout_limit: MAX_RESPONSE_BYTES + 4,
        stderr_limit: 4_096,
    });
    if process.state != ToolRunState::Observed
        || process.stdout_truncated
        || process.stderr_truncated
    {
        return Err(LanguageWorkerError::WorkerUnavailable(
            process
                .diagnostic
                .clone()
                .unwrap_or_else(|| "worker_not_observed".into()),
        ));
    }
    if process.executable_sha256.as_ref() != Some(&config.expected_python_digest) {
        return Err(LanguageWorkerError::ResponseBinding);
    }
    if hash_file(&config.script)
        .map_err(|_| LanguageWorkerError::Manifest("script_unavailable_after_run".into()))?
        != config.expected_script_digest
        || validate_manifest(config, language).is_err()
        || validate_runtime_binding(config).as_ref() != Ok(&python_version)
    {
        return Err(LanguageWorkerError::ResponseBinding);
    }
    let response = decode_response(&process.stdout)?;
    if response.schema_version != SCHEMA
        || response.request_id != request_id
        || response.language != language
        || response.source_revision != source_revision
        || response.model_manifest_digest != config.expected_manifest_digest
        || response.runtime_manifest_digest != config.expected_runtime_manifest_digest
        || response.requirements_lock_digest != config.expected_requirements_lock_digest
        || response.python_version != python_version
        || response.backend_revision != "stanza-1.14.0"
    {
        return Err(LanguageWorkerError::ResponseBinding);
    }
    if !has_observed_tokens(source, &response.sentences) {
        return Err(LanguageWorkerError::ResponseBinding);
    }
    let sentences = decode_dependency_sentences(response.sentences, source)?;
    let canonical = serde_json::to_string(&(
        &config.expected_runtime_manifest_digest,
        &config.expected_requirements_lock_digest,
        &python_version,
        &sentences,
    ))
    .map_err(|_| LanguageWorkerError::RequestEncoding)?;
    Ok(LanguageObservation {
        schema_version: SCHEMA,
        source_revision,
        language: language.into(),
        backend_revision: response.backend_revision,
        model_manifest_digest: config.expected_manifest_digest.clone(),
        runtime_manifest_digest: config.expected_runtime_manifest_digest.clone(),
        requirements_lock_digest: config.expected_requirements_lock_digest.clone(),
        python_version,
        payload_digest: stable_sha256(&canonical),
        sentences,
        process,
        semantic_truth_claim: false,
    })
}

fn decode_response(bytes: &[u8]) -> Result<WorkerResponse, LanguageWorkerError> {
    if bytes.len() < 5 {
        return Err(LanguageWorkerError::ResponseFraming);
    }
    let length = u32::from_be_bytes(
        bytes[..4]
            .try_into()
            .map_err(|_| LanguageWorkerError::ResponseFraming)?,
    ) as usize;
    if length == 0 || length > MAX_RESPONSE_BYTES || bytes.len() != length + 4 {
        return Err(LanguageWorkerError::ResponseFraming);
    }
    serde_json::from_slice(&bytes[4..]).map_err(|_| LanguageWorkerError::ResponseFraming)
}

fn decode_dependency_sentences(
    worker_sentences: Vec<WorkerSentence>,
    source: &str,
) -> Result<Vec<DependencySentence>, LanguageWorkerError> {
    let character_boundaries = source
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(source.len()))
        .collect::<Vec<_>>();
    let mut previous_sentence_end = 0usize;
    let mut decoded_sentences = Vec::with_capacity(worker_sentences.len());
    for sentence in worker_sentences {
        if sentence.tokens.is_empty() {
            return Err(LanguageWorkerError::InvalidDependency);
        }
        let word_count = sentence
            .tokens
            .iter()
            .map(|token| token.words.len())
            .sum::<usize>();
        if word_count == 0 {
            return Err(LanguageWorkerError::InvalidDependency);
        }
        let mut heads = vec![None; word_count + 1];
        let mut relations = vec![None::<String>; word_count + 1];
        let mut seen_words = BTreeSet::new();
        let mut previous_token_end = previous_sentence_end;
        let mut tokens = Vec::with_capacity(sentence.tokens.len());

        for token in sentence.tokens {
            let Some(&start_byte) = character_boundaries.get(token.start_char) else {
                return Err(LanguageWorkerError::InvalidOffset);
            };
            let Some(&end_byte) = character_boundaries.get(token.end_char) else {
                return Err(LanguageWorkerError::InvalidOffset);
            };
            if start_byte >= end_byte
                || start_byte < previous_token_end
                || source.get(start_byte..end_byte) != Some(token.text.as_str())
            {
                return Err(LanguageWorkerError::InvalidOffset);
            }
            previous_token_end = end_byte;
            if token.words.is_empty() {
                return Err(LanguageWorkerError::InvalidDependency);
            }
            let mut words = Vec::with_capacity(token.words.len());
            for word in token.words {
                if word.id == 0
                    || word.id > word_count
                    || word.head > word_count
                    || word.id == word.head
                    || word.text.trim().is_empty()
                    || !valid_ud_relation(&word.deprel)
                    || word
                        .upos
                        .as_deref()
                        .is_some_and(|upos| !valid_ud_upos(upos))
                    || word.lemma.as_deref().is_some_and(str::is_empty)
                    || word
                        .feats
                        .as_deref()
                        .is_some_and(|features| !valid_ud_features(features))
                    || !seen_words.insert(word.id)
                {
                    return Err(LanguageWorkerError::InvalidDependency);
                }
                heads[word.id] = Some(word.head);
                relations[word.id] = Some(word.deprel.clone());
                words.push(DependencyWord {
                    id: word.id,
                    text: word.text,
                    head: word.head,
                    deprel: word.deprel,
                    upos: word.upos,
                    lemma: word.lemma,
                    feats: word.feats,
                });
            }
            tokens.push(DependencyToken {
                text: token.text,
                start_byte,
                end_byte,
                words,
            });
        }
        if seen_words.len() != word_count {
            return Err(LanguageWorkerError::InvalidDependency);
        }

        let mut root_count = 0usize;
        for id in 1..=word_count {
            let Some(head) = heads[id] else {
                return Err(LanguageWorkerError::InvalidDependency);
            };
            let Some(relation) = relations[id].as_deref() else {
                return Err(LanguageWorkerError::InvalidDependency);
            };
            if head == 0 {
                root_count += 1;
                if relation != "root" {
                    return Err(LanguageWorkerError::InvalidDependency);
                }
            } else if relation == "root" {
                return Err(LanguageWorkerError::InvalidDependency);
            }
        }
        if root_count != 1 || contains_dependency_cycle(&heads) {
            return Err(LanguageWorkerError::InvalidDependency);
        }
        previous_sentence_end = previous_token_end;
        decoded_sentences.push(DependencySentence { tokens });
    }
    Ok(decoded_sentences)
}

fn contains_dependency_cycle(heads: &[Option<usize>]) -> bool {
    let mut state = vec![0_u8; heads.len()];
    for start in 1..heads.len() {
        if state[start] != 0 {
            continue;
        }
        let mut path = Vec::new();
        let mut current = start;
        while current != 0 && state[current] == 0 {
            state[current] = 1;
            path.push(current);
            let Some(head) = heads[current] else {
                return true;
            };
            current = head;
        }
        if current != 0 && state[current] == 1 {
            return true;
        }
        for id in path {
            state[id] = 2;
        }
    }
    false
}

fn valid_ud_relation(relation: &str) -> bool {
    let mut components = relation.split(':');
    components.next().is_some_and(valid_ud_relation_component)
        && components.all(valid_ud_relation_component)
}

fn valid_ud_relation_component(component: &str) -> bool {
    !component.is_empty()
        && component
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_ud_upos(upos: &str) -> bool {
    matches!(
        upos,
        "ADJ"
            | "ADP"
            | "ADV"
            | "AUX"
            | "CCONJ"
            | "DET"
            | "INTJ"
            | "NOUN"
            | "NUM"
            | "PART"
            | "PRON"
            | "PROPN"
            | "PUNCT"
            | "SCONJ"
            | "SYM"
            | "VERB"
            | "X"
    )
}

fn valid_ud_features(features: &str) -> bool {
    if features == "_" {
        return true;
    }
    if features.is_empty() {
        return false;
    }
    let mut seen = BTreeSet::new();
    for feature in features.split('|') {
        let Some((key, value)) = feature.split_once('=') else {
            return false;
        };
        if key.is_empty()
            || value.is_empty()
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            || !seen.insert(key)
            || value.split(',').any(|member| {
                member.is_empty()
                    || !member.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
                    })
            })
        {
            return false;
        }
    }
    true
}

fn has_observed_tokens(source: &str, sentences: &[WorkerSentence]) -> bool {
    source.trim().is_empty()
        || sentences
            .iter()
            .flat_map(|sentence| &sentence.tokens)
            .any(|token| !token.text.trim().is_empty())
}

fn validate_manifest(
    config: &LanguageWorkerConfig,
    language: &str,
) -> Result<ModelManifest, LanguageWorkerError> {
    let model_root = config
        .model_dir
        .canonicalize()
        .map_err(|_| LanguageWorkerError::Manifest("model_dir_unavailable".into()))?;
    let metadata = fs::metadata(&config.manifest)
        .map_err(|_| LanguageWorkerError::Manifest("manifest_unavailable".into()))?;
    if metadata.len() == 0 || metadata.len() > MAX_MANIFEST_BYTES {
        return Err(LanguageWorkerError::Manifest(
            "manifest_size_invalid".into(),
        ));
    }
    let raw = fs::read_to_string(&config.manifest)
        .map_err(|_| LanguageWorkerError::Manifest("manifest_invalid_utf8".into()))?;
    if stable_sha256(&raw) != config.expected_manifest_digest {
        return Err(LanguageWorkerError::Manifest(
            "manifest_digest_mismatch".into(),
        ));
    }
    let manifest: ModelManifest = serde_json::from_str(&raw)
        .map_err(|_| LanguageWorkerError::Manifest("manifest_shape_invalid".into()))?;
    if manifest.schema_version != "epistesys-dgcl-model-manifest.v1"
        || manifest.stanza_version != "1.14.0"
        || manifest.language != language
        || manifest.package.is_empty()
        || manifest.license_name.is_empty()
        || !manifest.license_url.starts_with("https://")
        || manifest.files.is_empty()
        || manifest.files.len() > 128
    {
        return Err(LanguageWorkerError::Manifest(
            "manifest_profile_invalid".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    for entry in &manifest.files {
        let path = Path::new(&entry.path);
        if path.is_absolute()
            || !path
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)))
            || !seen.insert(entry.path.clone())
            || !valid_digest(&entry.sha256)
        {
            return Err(LanguageWorkerError::ModelFile(
                "model_path_or_hash_invalid".into(),
            ));
        }
        let resolved = model_root
            .join(path)
            .canonicalize()
            .map_err(|_| LanguageWorkerError::ModelFile("model_file_missing".into()))?;
        if !resolved.starts_with(&model_root) || !resolved.is_file() {
            return Err(LanguageWorkerError::ModelFile(
                "model_file_outside_root".into(),
            ));
        }
        if hash_file(&resolved)
            .map_err(|_| LanguageWorkerError::ModelFile("model_file_unreadable".into()))?
            != entry.sha256
        {
            return Err(LanguageWorkerError::ModelFile(
                "model_digest_mismatch".into(),
            ));
        }
    }
    Ok(manifest)
}

fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 65_536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let mut output = String::from("sha256:");
    for byte in hasher.finalize() {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    Ok(output)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn canonical_path(path: &Path) -> Result<PathBuf, LanguageWorkerError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| LanguageWorkerError::RuntimeManifest("path_unavailable".into()))?;
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(LanguageWorkerError::RuntimeManifest(
            "reparse_point_path_rejected".into(),
        ));
    }
    path.canonicalize()
        .map_err(|_| LanguageWorkerError::RuntimeManifest("path_canonicalization_failed".into()))
}

fn checked_relative_path(root: &Path, relative: &str) -> Result<PathBuf, LanguageWorkerError> {
    if !valid_relative_path(relative) {
        return Err(LanguageWorkerError::RuntimeManifest(
            "relative_path_invalid".into(),
        ));
    }
    let mut current = root.to_path_buf();
    for component in Path::new(relative).components() {
        let std::path::Component::Normal(part) = component else {
            return Err(LanguageWorkerError::RuntimeManifest(
                "relative_path_invalid".into(),
            ));
        };
        current.push(part);
        let metadata = fs::symlink_metadata(&current)
            .map_err(|_| LanguageWorkerError::RuntimeManifest("pinned_path_unavailable".into()))?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(LanguageWorkerError::RuntimeManifest(
                "reparse_point_path_rejected".into(),
            ));
        }
    }
    let resolved = current
        .canonicalize()
        .map_err(|_| LanguageWorkerError::RuntimeManifest("pinned_path_unavailable".into()))?;
    if !resolved.starts_with(root) {
        return Err(LanguageWorkerError::RuntimeManifest(
            "pinned_path_escaped_repository".into(),
        ));
    }
    Ok(resolved)
}

fn valid_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn read_bounded_text(path: &Path, limit: u64) -> Result<String, LanguageWorkerError> {
    let file = File::open(path)
        .map_err(|_| LanguageWorkerError::RuntimeManifest("pinned_file_unavailable".into()))?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| LanguageWorkerError::RuntimeManifest("pinned_file_unreadable".into()))?;
    if bytes.is_empty() || bytes.len() as u64 > limit {
        return Err(LanguageWorkerError::RuntimeManifest(
            "pinned_file_size_invalid".into(),
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| LanguageWorkerError::RuntimeManifest("pinned_file_not_utf8".into()))
}

fn normalize_distribution_name(name: &str) -> Option<String> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return None;
    }
    let mut normalized = String::new();
    let mut separator = false;
    for byte in name.bytes() {
        if matches!(byte, b'-' | b'_' | b'.') {
            if !separator {
                normalized.push('-');
            }
            separator = true;
        } else {
            normalized.push((byte as char).to_ascii_lowercase());
            separator = false;
        }
    }
    if normalized.starts_with('-') || normalized.ends_with('-') {
        None
    } else {
        Some(normalized)
    }
}

fn parse_distribution_lines(
    contents: &str,
) -> Result<BTreeMap<String, String>, LanguageWorkerError> {
    let mut parsed = BTreeMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, version)) = line.split_once("==") else {
            return Err(LanguageWorkerError::RuntimeManifest(
                "distribution_pin_must_use_exact_version".into(),
            ));
        };
        if version.is_empty()
            || version.bytes().any(|byte| byte.is_ascii_whitespace())
            || line.matches("==").count() != 1
        {
            return Err(LanguageWorkerError::RuntimeManifest(
                "distribution_pin_invalid".into(),
            ));
        }
        let name = normalize_distribution_name(name).ok_or_else(|| {
            LanguageWorkerError::RuntimeManifest("distribution_name_invalid".into())
        })?;
        if parsed.insert(name, version.to_string()).is_some() {
            return Err(LanguageWorkerError::RuntimeManifest(
                "duplicate_distribution_pin".into(),
            ));
        }
        if parsed.len() > 512 {
            return Err(LanguageWorkerError::RuntimeManifest(
                "distribution_count_limit_exceeded".into(),
            ));
        }
    }
    if parsed.is_empty() {
        return Err(LanguageWorkerError::RuntimeManifest(
            "distribution_lock_empty".into(),
        ));
    }
    Ok(parsed)
}

#[cfg(test)]
fn validate_installed_distribution_set(
    lock: &str,
    installed: &[&str],
) -> Result<usize, LanguageWorkerError> {
    let expected = parse_distribution_lines(lock)?;
    let mut actual = BTreeMap::new();
    for entry in installed {
        let parsed = parse_distribution_lines(entry)?;
        if parsed.len() != 1 {
            return Err(LanguageWorkerError::RuntimeManifest(
                "installed_distribution_entry_invalid".into(),
            ));
        }
        let Some((name, version)) = parsed.into_iter().next() else {
            return Err(LanguageWorkerError::RuntimeManifest(
                "installed_distribution_entry_invalid".into(),
            ));
        };
        if actual.insert(name, version).is_some() {
            return Err(LanguageWorkerError::RuntimeManifest(
                "duplicate_installed_distribution".into(),
            ));
        }
    }
    if expected != actual {
        return Err(LanguageWorkerError::RuntimeManifest(
            "installed_distribution_set_mismatch".into(),
        ));
    }
    Ok(expected.len())
}

fn validate_requirements_lock(path: &Path) -> Result<usize, LanguageWorkerError> {
    let contents = read_bounded_text(path, MAX_REQUIREMENTS_BYTES)?;
    Ok(parse_distribution_lines(&contents)?.len())
}

fn digest_file(path: &Path) -> std::io::Result<String> {
    hash_file(path)
}

fn validate_runtime_binding(config: &LanguageWorkerConfig) -> Result<String, LanguageWorkerError> {
    if !(1..=60_000).contains(&config.timeout_ms)
        || !valid_digest(&config.expected_python_digest)
        || !valid_digest(&config.expected_script_digest)
        || !valid_digest(&config.expected_manifest_digest)
        || !valid_digest(&config.expected_runtime_manifest_digest)
        || !valid_digest(&config.expected_requirements_lock_digest)
    {
        return Err(LanguageWorkerError::RuntimeManifest(
            "worker_profile_digest_or_timeout_invalid".into(),
        ));
    }
    for path in [
        &config.python,
        &config.script,
        &config.model_dir,
        &config.manifest,
        &config.runtime_manifest,
        &config.requirements_lock,
    ] {
        let resolved = canonical_path(path)?;
        if resolved != *path {
            return Err(LanguageWorkerError::RuntimeManifest(
                "worker_path_changed_after_pin_binding".into(),
            ));
        }
    }
    let runtime_raw = read_bounded_text(&config.runtime_manifest, MAX_MANIFEST_BYTES)?;
    if stable_sha256(&runtime_raw) != config.expected_runtime_manifest_digest {
        return Err(LanguageWorkerError::RuntimeManifest(
            "runtime_manifest_digest_mismatch".into(),
        ));
    }
    let runtime: RuntimeManifest = serde_json::from_str(&runtime_raw)
        .map_err(|_| LanguageWorkerError::RuntimeManifest("runtime_manifest_invalid".into()))?;
    let requirements_raw = read_bounded_text(&config.requirements_lock, MAX_REQUIREMENTS_BYTES)?;
    if stable_sha256(&requirements_raw) != config.expected_requirements_lock_digest {
        return Err(LanguageWorkerError::RuntimeManifest(
            "requirements_lock_digest_mismatch".into(),
        ));
    }
    let requirements_count = parse_distribution_lines(&requirements_raw)?.len();
    if runtime.schema_version != "epistesys-dgcl-python-runtime.v1"
        || runtime.target_platform != "windows_x86_64"
        || runtime.python.implementation != "CPython"
        || runtime.python.version != "3.12.10"
        || runtime.python.executable_sha256 != config.expected_python_digest
        || !runtime.python.isolated_mode
        || !valid_relative_path(&runtime.python.executable_path)
        || runtime.requirements_lock.sha256 != config.expected_requirements_lock_digest
        || runtime.requirements_lock.distribution_count != requirements_count
        || runtime.requirements_lock.comparison != "exact_normalized_name_and_version_set"
        || runtime.worker.sha256 != config.expected_script_digest
        || runtime.stanza.version != "1.14.0"
        || runtime.stanza.package != "default"
        || runtime.stanza.processors != ["tokenize", "pos", "lemma", "depparse"]
        || runtime.stanza.device != "cpu"
        || runtime.stanza.download_method != "none"
        || runtime.models.model_files_bundled
        || runtime.network_boundary.model_download_during_parse
        || !runtime.network_boundary.offline_environment_hints
        || runtime.network_boundary.os_network_isolation_enforced
        || runtime.claim_boundary.trim().is_empty()
        || digest_file(&config.python).map_err(|_| {
            LanguageWorkerError::RuntimeManifest("python_executable_unreadable".into())
        })? != config.expected_python_digest
        || digest_file(&config.script)
            .map_err(|_| LanguageWorkerError::RuntimeManifest("worker_script_unreadable".into()))?
            != config.expected_script_digest
    {
        return Err(LanguageWorkerError::RuntimeManifest(
            "runtime_manifest_profile_mismatch".into(),
        ));
    }
    Ok(runtime.python.version)
}

#[cfg(test)]
mod runtime_lock_tests {
    use super::*;

    #[test]
    fn runtime_lock_rejects_missing_extra_duplicate_and_version_drift() {
        let lock = "Stanza==1.14.0\nSome_Pkg==2.0\n";
        let exact = ["stanza==1.14.0", "some-pkg==2.0"];
        assert!(validate_installed_distribution_set(lock, &exact).is_ok());
        assert!(validate_installed_distribution_set(lock, &["stanza==1.14.0"]).is_err());
        assert!(validate_installed_distribution_set(
            lock,
            &["stanza==1.14.0", "some-pkg==2.0", "unlocked==1"]
        )
        .is_err());
        assert!(
            validate_installed_distribution_set(lock, &["stanza==1.14.0", "some-pkg==2.1"])
                .is_err()
        );
        assert!(validate_installed_distribution_set(
            lock,
            &["stanza==1.14.0", "some-pkg==2.0", "Some_Pkg==2.0"]
        )
        .is_err());
    }

    #[test]
    fn runtime_lock_rejects_unpinned_and_duplicate_normalized_names() {
        assert!(
            validate_installed_distribution_set("stanza>=1.14\n", &["stanza==1.14.0"]).is_err()
        );
        assert!(validate_installed_distribution_set(
            "Some_Pkg==2.0\nsome-pkg==2.0\n",
            &["some-pkg==2.0"]
        )
        .is_err());
    }

    #[test]
    fn nonempty_source_rejects_a_well_formed_but_empty_worker_observation() {
        assert!(!has_observed_tokens("Please parse this.", &[]));
        assert!(!has_observed_tokens(
            "Please parse this.",
            &[WorkerSentence { tokens: Vec::new() }]
        ));
        let observed = [WorkerSentence {
            tokens: vec![WorkerToken {
                text: "parse".into(),
                start_char: 7,
                end_char: 12,
                words: Vec::new(),
            }],
        }];
        assert!(has_observed_tokens("Please parse this.", &observed));
        assert!(has_observed_tokens(" \n", &[]));
    }

    #[test]
    fn dependency_validator_rejects_self_loops_cycles_multiple_roots_and_bad_heads() {
        let self_loop = vec![sentence(vec![token(
            "a",
            0,
            1,
            vec![word(1, "a", 1, "root")],
        )])];
        assert!(matches!(
            decode_dependency_sentences(self_loop, "a"),
            Err(LanguageWorkerError::InvalidDependency)
        ));

        let cycle_with_separate_root = vec![sentence(vec![
            token("a", 0, 1, vec![word(1, "a", 2, "nsubj")]),
            token("b", 2, 3, vec![word(2, "b", 1, "obj")]),
            token("c", 4, 5, vec![word(3, "c", 0, "root")]),
        ])];
        assert!(matches!(
            decode_dependency_sentences(cycle_with_separate_root, "a b c"),
            Err(LanguageWorkerError::InvalidDependency)
        ));

        let multiple_roots = vec![sentence(vec![
            token("a", 0, 1, vec![word(1, "a", 0, "root")]),
            token("b", 2, 3, vec![word(2, "b", 0, "root")]),
        ])];
        assert!(matches!(
            decode_dependency_sentences(multiple_roots, "a b"),
            Err(LanguageWorkerError::InvalidDependency)
        ));
    }

    #[test]
    fn dependency_validator_rejects_reordered_tokens_and_invalid_id_coverage() {
        let reordered = vec![sentence(vec![
            token("b", 2, 3, vec![word(1, "b", 0, "root")]),
            token("a", 0, 1, vec![word(2, "a", 1, "dep")]),
        ])];
        assert!(matches!(
            decode_dependency_sentences(reordered, "a b"),
            Err(LanguageWorkerError::InvalidOffset)
        ));

        let missing_id = vec![sentence(vec![
            token("a", 0, 1, vec![word(1, "a", 0, "root")]),
            token("b", 2, 3, vec![word(3, "b", 1, "dep")]),
        ])];
        assert!(matches!(
            decode_dependency_sentences(missing_id, "a b"),
            Err(LanguageWorkerError::InvalidDependency)
        ));
    }

    #[test]
    fn dependency_validator_preserves_multiword_tokens_and_utf8_byte_spans() {
        let multiword = vec![sentence(vec![token(
            "can't",
            0,
            5,
            vec![word(1, "ca", 0, "root"), word(2, "n't", 1, "advmod")],
        )])];
        let decoded = decode_dependency_sentences(multiword, "can't").unwrap();
        assert_eq!(decoded[0].tokens[0].words.len(), 2);

        let japanese = vec![sentence(vec![token(
            "日本",
            0,
            2,
            vec![word(1, "日本", 0, "root")],
        )])];
        let decoded = decode_dependency_sentences(japanese, "日本").unwrap();
        assert_eq!(decoded[0].tokens[0].start_byte, 0);
        assert_eq!(decoded[0].tokens[0].end_byte, "日本".len());
    }

    #[test]
    fn dependency_validator_preserves_ud_relation_subtypes_and_morphology() {
        let valid = vec![sentence(vec![
            token(
                "cats",
                0,
                4,
                vec![word_with_features(
                    1,
                    "cats",
                    0,
                    "root",
                    "NOUN",
                    "cat",
                    Some("Number=Plur"),
                )],
            ),
            token(
                "run",
                5,
                8,
                vec![word_with_features(
                    2,
                    "run",
                    1,
                    "nsubj:pass",
                    "VERB",
                    "run",
                    Some("VerbForm=Fin"),
                )],
            ),
        ])];
        let decoded = decode_dependency_sentences(valid, "cats run").unwrap();
        let word = &decoded[0].tokens[0].words[0];
        assert_eq!(word.deprel, "root");
        assert_eq!(word.feats.as_deref(), Some("Number=Plur"));
        assert_eq!(decoded[0].tokens[1].words[0].deprel, "nsubj:pass");
        assert_eq!(
            decoded[0].tokens[1].words[0].feats.as_deref(),
            Some("VerbForm=Fin")
        );

        let invalid_features = vec![sentence(vec![token(
            "cats",
            0,
            4,
            vec![word_with_features(
                1,
                "cats",
                0,
                "root",
                "NOUN",
                "cat",
                Some("Number"),
            )],
        )])];
        assert!(matches!(
            decode_dependency_sentences(invalid_features, "cats"),
            Err(LanguageWorkerError::InvalidDependency)
        ));

        let invalid_relation = vec![sentence(vec![token(
            "cats",
            0,
            4,
            vec![word_with_features(
                1,
                "cats",
                0,
                "root:bad:",
                "NOUN",
                "cat",
                None,
            )],
        )])];
        assert!(matches!(
            decode_dependency_sentences(invalid_relation, "cats"),
            Err(LanguageWorkerError::InvalidDependency)
        ));

        let invalid_upos = vec![sentence(vec![token(
            "cats",
            0,
            4,
            vec![word_with_features(
                1, "cats", 0, "root", "ROOT", "cat", None,
            )],
        )])];
        assert!(matches!(
            decode_dependency_sentences(invalid_upos, "cats"),
            Err(LanguageWorkerError::InvalidDependency)
        ));
    }

    fn sentence(tokens: Vec<WorkerToken>) -> WorkerSentence {
        WorkerSentence { tokens }
    }

    fn token(
        text: &str,
        start_char: usize,
        end_char: usize,
        words: Vec<WorkerWord>,
    ) -> WorkerToken {
        WorkerToken {
            text: text.into(),
            start_char,
            end_char,
            words,
        }
    }

    fn word(id: usize, text: &str, head: usize, deprel: &str) -> WorkerWord {
        WorkerWord {
            id,
            text: text.into(),
            head,
            deprel: deprel.into(),
            upos: None,
            lemma: None,
            feats: None,
        }
    }

    fn word_with_features(
        id: usize,
        text: &str,
        head: usize,
        deprel: &str,
        upos: &str,
        lemma: &str,
        feats: Option<&str>,
    ) -> WorkerWord {
        WorkerWord {
            id,
            text: text.into(),
            head,
            deprel: deprel.into(),
            upos: Some(upos.into()),
            lemma: Some(lemma.into()),
            feats: feats.map(str::to_string),
        }
    }

    #[cfg(windows)]
    #[test]
    fn runtime_pins_reject_a_tampered_runtime_manifest_before_worker_binding() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../");
        let source_pins = fs::read(repository.join("validation/dgcl-nlp-model-pins.v1.json"))
            .expect("repository pin fixture exists");
        let source_runtime =
            fs::read_to_string(repository.join("validation/dgcl-python-runtime.v1.json"))
                .expect("repository runtime fixture exists");
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after epoch")
            .as_nanos();
        let temporary =
            std::env::temp_dir().join(format!("dgcl-runtime-pins-{}-{unique}", std::process::id()));
        let validation = temporary.join("validation");
        fs::create_dir_all(&validation).expect("temporary validation fixture is created");
        fs::write(validation.join("dgcl-nlp-model-pins.v1.json"), source_pins)
            .expect("pin fixture is copied");
        fs::write(
            validation.join("dgcl-python-runtime.v1.json"),
            format!("{source_runtime} "),
        )
        .expect("runtime fixture is tampered");

        let result = load_dgcl_runtime_pins(&temporary);
        assert!(matches!(
            result,
            Err(LanguageWorkerError::RuntimeManifest(message))
                if message == "runtime_manifest_digest_mismatch"
        ));
        fs::remove_dir_all(&temporary).expect("only the isolated test fixture is removed");
    }
}
