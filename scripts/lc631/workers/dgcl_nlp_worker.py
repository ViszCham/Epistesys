"""Pinned, offline Stanza parser for the DGCL length-prefixed JSON protocol."""

import hashlib
import importlib.metadata
import json
import os
import platform
import re
import struct
import sys

SCHEMA = "epistesys-dgcl-nlp.v1"
MAX_REQUEST_BYTES = 270_336
MAX_TEXT_BYTES = 262_144
MAX_RESPONSE_BYTES = 2_097_152
STANZA_VERSION = "1.14.0"
MAX_MANIFEST_BYTES = 65_536
MAX_REQUIREMENTS_BYTES = 65_536


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for chunk in iter(lambda: stream.read(65_536), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def read_bounded(path, maximum):
    with open(path, "rb") as stream:
        payload = stream.read(maximum + 1)
    if not payload or len(payload) > maximum:
        raise ValueError("pinned_file_size_invalid")
    return payload


def normalize_distribution_name(name):
    if not isinstance(name, str) or not name or not re.fullmatch(r"[A-Za-z0-9._-]+", name):
        raise ValueError("distribution_name_invalid")
    normalized = re.sub(r"[-_.]+", "-", name).lower()
    if normalized.startswith("-") or normalized.endswith("-"):
        raise ValueError("distribution_name_invalid")
    return normalized


def parse_distribution_lock(payload):
    result = {}
    for raw_line in payload.decode("utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if line.count("==") != 1:
            raise ValueError("distribution_pin_must_use_exact_version")
        name, version = line.split("==", 1)
        if not version or any(character.isspace() for character in version):
            raise ValueError("distribution_pin_invalid")
        normalized = normalize_distribution_name(name)
        if normalized in result:
            raise ValueError("duplicate_distribution_pin")
        result[normalized] = version
        if len(result) > 512:
            raise ValueError("distribution_count_limit_exceeded")
    if not result:
        raise ValueError("distribution_lock_empty")
    return result


def installed_distribution_set():
    result = {}
    for distribution in importlib.metadata.distributions():
        name = distribution.metadata.get("Name")
        if not name:
            continue
        normalized = normalize_distribution_name(name)
        if normalized in result:
            raise ValueError("duplicate_installed_distribution")
        result[normalized] = distribution.version
        if len(result) > 512:
            raise ValueError("installed_distribution_count_limit_exceeded")
    return result


def validate_distribution_set(lock_bytes, actual):
    expected = parse_distribution_lock(lock_bytes)
    if actual != expected:
        raise ValueError("installed_distribution_set_mismatch")
    return len(expected)


def validate_runtime(request, runtime_path, requirements_path, model_dir):
    if sys.flags.isolated != 1:
        raise ValueError("python_isolated_mode_required")
    if sys.version_info[:3] != (3, 12, 10):
        raise ValueError("python_version_mismatch")
    if sys.platform != "win32" or platform.machine().lower() not in ("amd64", "x86_64"):
        raise ValueError("python_platform_mismatch")
    if sha256_file(sys.executable) != request["python_executable_digest"]:
        raise ValueError("python_executable_digest_mismatch")

    runtime_bytes = read_bounded(runtime_path, MAX_MANIFEST_BYTES)
    if "sha256:" + hashlib.sha256(runtime_bytes).hexdigest() != request["runtime_manifest_digest"]:
        raise ValueError("runtime_manifest_digest_mismatch")
    runtime = json.loads(runtime_bytes.decode("utf-8"))
    lock_bytes = read_bounded(requirements_path, MAX_REQUIREMENTS_BYTES)
    if "sha256:" + hashlib.sha256(lock_bytes).hexdigest() != request["requirements_lock_digest"]:
        raise ValueError("requirements_lock_digest_mismatch")
    actual = installed_distribution_set()
    expected_count = validate_distribution_set(lock_bytes, actual)

    expected_worker_digest = sha256_file(sys.argv[0])
    if (
        runtime.get("schema_version") != "epistesys-dgcl-python-runtime.v1"
        or runtime.get("target_platform") != "windows_x86_64"
        or runtime.get("python", {}).get("implementation") != "CPython"
        or runtime.get("python", {}).get("version") != "3.12.10"
        or runtime.get("python", {}).get("executable_sha256") != request["python_executable_digest"]
        or runtime.get("python", {}).get("isolated_mode") is not True
        or runtime.get("requirements_lock", {}).get("sha256") != request["requirements_lock_digest"]
        or runtime.get("requirements_lock", {}).get("distribution_count") != expected_count
        or runtime.get("requirements_lock", {}).get("comparison") != "exact_normalized_name_and_version_set"
        or runtime.get("worker", {}).get("sha256") != expected_worker_digest
        or runtime.get("stanza", {}).get("version") != STANZA_VERSION
        or runtime.get("stanza", {}).get("package") != "default"
        or runtime.get("stanza", {}).get("processors") != ["tokenize", "pos", "lemma", "depparse"]
        or runtime.get("stanza", {}).get("device") != "cpu"
        or runtime.get("stanza", {}).get("download_method") != "none"
        or runtime.get("network_boundary", {}).get("model_download_during_parse") is not False
        or runtime.get("network_boundary", {}).get("offline_environment_hints") is not True
        or runtime.get("network_boundary", {}).get("os_network_isolation_enforced") is not False
    ):
        raise ValueError("runtime_manifest_profile_mismatch")

    manifest_path = os.path.join(model_dir, "manifest-" + request["language"] + ".json")
    manifest_bytes = read_bounded(manifest_path, MAX_MANIFEST_BYTES)
    if "sha256:" + hashlib.sha256(manifest_bytes).hexdigest() != request["model_manifest_digest"]:
        raise ValueError("model_manifest_digest_mismatch")
    manifest = json.loads(manifest_bytes.decode("utf-8"))
    if (
        manifest.get("schema_version") != "epistesys-dgcl-model-manifest.v1"
        or manifest.get("stanza_version") != STANZA_VERSION
        or manifest.get("language") != request["language"]
        or manifest.get("package") != request["package"]
    ):
        raise ValueError("model_manifest_profile_mismatch")
    if runtime.get("models", {}).get(request["language"] + "_manifest_sha256") != request["model_manifest_digest"]:
        raise ValueError("runtime_model_manifest_binding_mismatch")


def read_request():
    header = sys.stdin.buffer.read(4)
    if len(header) != 4:
        raise ValueError("missing_length_prefix")
    length = struct.unpack(">I", header)[0]
    if length == 0 or length > MAX_REQUEST_BYTES:
        raise ValueError("request_size_invalid")
    payload = sys.stdin.buffer.read(length)
    if len(payload) != length or sys.stdin.buffer.read(1):
        raise ValueError("request_framing_invalid")
    request = json.loads(payload.decode("utf-8"))
    if not isinstance(request, dict) or set(request) != {
        "schema_version", "request_id", "language", "source_revision",
        "text", "model_manifest_digest", "package", "runtime_manifest_digest",
        "requirements_lock_digest", "python_executable_digest"
    }:
        raise ValueError("request_shape_invalid")
    if request["schema_version"] != SCHEMA or request["language"] not in ("ja", "en"):
        raise ValueError("request_profile_invalid")
    if not isinstance(request["request_id"], str) or not request["request_id"]:
        raise ValueError("request_id_invalid")
    if not isinstance(request["text"], str) or len(request["text"].encode("utf-8")) > MAX_TEXT_BYTES:
        raise ValueError("text_size_invalid")
    expected = "sha256:" + hashlib.sha256(request["text"].encode("utf-8")).hexdigest()
    if request["source_revision"] != expected:
        raise ValueError("source_revision_mismatch")
    if not isinstance(request["model_manifest_digest"], str) or not request["model_manifest_digest"].startswith("sha256:"):
        raise ValueError("model_manifest_digest_invalid")
    for field in ("runtime_manifest_digest", "requirements_lock_digest", "python_executable_digest"):
        value = request.get(field)
        if not isinstance(value, str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", value):
            raise ValueError(field + "_invalid")
    if not isinstance(request["package"], str) or not request["package"]:
        raise ValueError("package_invalid")
    return request


def run(request, model_dir):
    import stanza

    if stanza.__version__ != STANZA_VERSION:
        raise RuntimeError("stanza_revision_mismatch")
    from stanza.pipeline.core import DownloadMethod

    pipeline = stanza.Pipeline(
        lang=request["language"],
        dir=model_dir,
        package=request["package"],
        processors="tokenize,pos,lemma,depparse",
        download_method=DownloadMethod.NONE,
        use_gpu=False,
        verbose=False,
    )
    document = pipeline(request["text"])
    sentences = []
    for sentence in document.sentences:
        tokens = []
        for token in sentence.tokens:
            if token.start_char is None or token.end_char is None:
                raise ValueError("missing_token_offset")
            tokens.append({
                "text": token.text,
                "start_char": token.start_char,
                "end_char": token.end_char,
                "words": [{
                    "id": word.id,
                    "text": word.text,
                    "head": word.head,
                    "deprel": word.deprel,
                    "upos": word.upos,
                    "lemma": word.lemma,
                    "feats": word.feats,
                } for word in token.words],
            })
        sentences.append({"tokens": tokens})
    return {
        "schema_version": SCHEMA,
        "request_id": request["request_id"],
        "language": request["language"],
        "source_revision": request["source_revision"],
        "model_manifest_digest": request["model_manifest_digest"],
        "runtime_manifest_digest": request["runtime_manifest_digest"],
        "requirements_lock_digest": request["requirements_lock_digest"],
        "python_version": platform.python_version(),
        "backend_revision": "stanza-" + STANZA_VERSION,
        "sentences": sentences,
    }


def main():
    if len(sys.argv) != 4 or not os.path.isdir(sys.argv[1]):
        raise ValueError("worker_paths_invalid")
    request = read_request()
    validate_runtime(request, sys.argv[2], sys.argv[3], sys.argv[1])
    response = json.dumps(run(request, sys.argv[1]), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    if len(response) > MAX_RESPONSE_BYTES:
        raise ValueError("response_size_invalid")
    sys.stdout.buffer.write(struct.pack(">I", len(response)))
    sys.stdout.buffer.write(response)
    sys.stdout.buffer.flush()


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        sys.stderr.write("dgcl_worker_error:" + type(error).__name__ + "\n")
        sys.exit(2)
