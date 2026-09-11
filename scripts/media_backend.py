#!/usr/bin/env python3
"""Bounded local media backend for Labyrinth-Codex v6.3.1.

The script analyzes only an already-authorized local file. It never downloads
media and never emits identity, emotion, intent, or mental-state claims.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
import math
import subprocess
import tempfile
import wave
from pathlib import Path
from typing import Any

SCHEMA = "lc631-media-backend.v1"
MAX_FRAMES = 120
MAX_OBSERVATIONS = 10_000
MAX_TEMP_BYTES = 8 * 1024 * 1024 * 1024
MAX_PCM_SAMPLES = 16_000 * 60 * 60


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(64 * 1024), b""):
            digest.update(block)
    return "sha256:" + digest.hexdigest()


def canonical_digest(value: Any) -> str:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def tree_manifest_digest(root: Path) -> str:
    entries: list[str] = []
    if root.is_dir():
        for path in sorted(item for item in root.rglob("*") if item.is_file()):
            entries.append(
                f"{path.relative_to(root).as_posix()}:{path.stat().st_size}:{sha256_file(path)}"
            )
    basis = "lc631-directory-manifest.v2\0" + "\x1f".join(entries)
    return "sha256:" + hashlib.sha256(basis.encode("utf-8")).hexdigest()


def tree_size(root: Path) -> int:
    return sum(path.stat().st_size for path in root.rglob("*") if path.is_file())


def model_card_license_hint(root: Path) -> str:
    readmes = sorted(root.rglob("README.md")) if root.is_dir() else []
    for readme in readmes:
        for line in readme.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.strip().lower().startswith("license:"):
                return line.split(":", 1)[1].strip()[:128] or "unobserved"
    return "unobserved"


def package_version(name: str) -> str:
    try:
        return importlib.metadata.version(name)
    except importlib.metadata.PackageNotFoundError:
        return "unavailable"


def run_checked(command: list[str], timeout: int = 180) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        check=True,
        capture_output=True,
        text=True,
        timeout=timeout,
        encoding="utf-8",
        errors="replace",
    )


def add_observation(
    observations: list[dict[str, Any]],
    kind: str,
    start_ms: int,
    end_ms: int,
    label: str,
    track_id: int | None = None,
) -> None:
    if len(observations) >= MAX_OBSERVATIONS:
        return
    basis = {
        "kind": kind,
        "start_ms": max(0, int(start_ms)),
        "end_ms": max(max(0, int(start_ms)), int(end_ms)),
        "track_id": track_id,
        "label": label[:4096],
    }
    digest = canonical_digest(basis)
    observations.append(
        {
            "observation_id": f"obs-{len(observations):06d}-{digest[7:19]}",
            **basis,
            "evidence_digest": digest,
        }
    )


def probe_media(ffprobe: Path, media: Path) -> dict[str, Any]:
    result = run_checked(
        [
            str(ffprobe),
            "-v",
            "error",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
            str(media),
        ],
        timeout=60,
    )
    return json.loads(result.stdout)


def extract_media(ffmpeg: Path, media: Path, root: Path, diagnostics: list[str]) -> tuple[list[Path], Path | None]:
    frame_pattern = root / "frame-%05d.jpg"
    try:
        run_checked(
            [
                str(ffmpeg),
                "-hide_banner",
                "-loglevel",
                "error",
                "-i",
                str(media),
                "-vf",
                "fps=2,scale=640:-2",
                "-frames:v",
                str(MAX_FRAMES),
                str(frame_pattern),
            ],
            timeout=180,
        )
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        diagnostics.append(f"video_decode_unavailable:{type(error).__name__}")
    audio_path = root / "audio-16khz-mono.wav"
    try:
        run_checked(
            [
                str(ffmpeg),
                "-hide_banner",
                "-loglevel",
                "error",
                "-i",
                str(media),
                "-vn",
                "-ac",
                "1",
                "-ar",
                "16000",
                "-c:a",
                "pcm_s16le",
                str(audio_path),
            ],
            timeout=180,
        )
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        diagnostics.append(f"audio_decode_unavailable:{type(error).__name__}")
    return sorted(root.glob("frame-*.jpg")), audio_path if audio_path.is_file() else None


def analyze_vision(
    frames: list[Path],
    observations: list[dict[str, Any]],
    executed: set[str],
    diagnostics: list[str],
) -> None:
    if not frames:
        return
    try:
        import cv2
        import numpy as np
    except Exception as error:  # pragma: no cover - diagnostics boundary
        diagnostics.append(f"opencv_import_failed:{type(error).__name__}")
        return

    executed.update({"shot_boundary", "object_detection"})
    hog = cv2.HOGDescriptor()
    hog.setSVMDetector(cv2.HOGDescriptor_getDefaultPeopleDetector())
    cascade_path = Path(cv2.data.haarcascades) / "haarcascade_frontalface_default.xml"
    face = cv2.CascadeClassifier(str(cascade_path))
    previous_gray = None
    previous_hist = None
    for index, frame_path in enumerate(frames):
        image = cv2.imread(str(frame_path))
        if image is None:
            diagnostics.append(f"frame_decode_failed:{frame_path.name}")
            continue
        start_ms = index * 500
        gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)
        if previous_gray is not None:
            motion_score = float(np.mean(cv2.absdiff(gray, previous_gray)))
            if motion_score >= 8.0:
                add_observation(
                    observations,
                    "motion_candidate",
                    start_ms,
                    start_ms + 500,
                    f"frame_motion_candidate:mean_absdiff={motion_score:.3f}",
                )
                executed.add("frame_difference")
        hist = cv2.calcHist([image], [0, 1], None, [16, 16], [0, 256, 0, 256])
        cv2.normalize(hist, hist)
        if previous_hist is not None:
            shot_score = float(cv2.compareHist(previous_hist, hist, cv2.HISTCMP_BHATTACHARYYA))
            if shot_score >= 0.45:
                add_observation(
                    observations,
                    "shot_boundary",
                    start_ms,
                    start_ms,
                    f"shot_boundary_candidate:bhattacharyya={shot_score:.3f}",
                )
        boxes, weights = hog.detectMultiScale(image, winStride=(8, 8), padding=(8, 8), scale=1.05)
        for box, weight in zip(boxes[:16], weights[:16]):
            x, y, width, height = [int(value) for value in box]
            add_observation(
                observations,
                "face_region_candidate",
                start_ms,
                start_ms + 500,
                f"anonymous_person_box_candidate:x={x},y={y},w={width},h={height},score={float(weight):.4f}",
            )
        faces = face.detectMultiScale(gray, scaleFactor=1.1, minNeighbors=5, minSize=(24, 24))
        for x, y, width, height in faces[:16]:
            add_observation(
                observations,
                "object_box",
                start_ms,
                start_ms + 500,
                f"face_region_candidate:x={int(x)},y={int(y)},w={int(width)},h={int(height)};identity_disabled",
            )
        previous_gray = gray
        previous_hist = hist


def analyze_asr(
    audio: Path | None,
    model_cache: Path,
    observations: list[dict[str, Any]],
    executed: set[str],
    diagnostics: list[str],
) -> str:
    if audio is None:
        return "asr-not-executed"
    try:
        from faster_whisper import WhisperModel
    except Exception as error:  # pragma: no cover - diagnostics boundary
        diagnostics.append(f"faster_whisper_import_failed:{type(error).__name__}")
        return "asr-import-failed"
    model_candidates = sorted(
        path.parent for path in model_cache.rglob("model.bin") if path.is_file()
    )
    if not model_candidates:
        diagnostics.append("asr_local_model_cache_missing")
        return "asr-local-model-unavailable"
    model_path = model_candidates[0]
    model_id = f"local:{tree_manifest_digest(model_path)}"
    selected_device = "cpu"
    try:
        model = WhisperModel(
            str(model_path),
            device="cuda",
            compute_type="float16",
            local_files_only=True,
        )
        selected_device = "cuda"
    except Exception as error:
        diagnostics.append(f"whisper_cuda_fallback:{type(error).__name__}")
        model = WhisperModel(
            str(model_path),
            device="cpu",
            compute_type="int8",
            local_files_only=True,
        )
    def transcribe(active_model: Any) -> list[Any]:
        segments, _info = active_model.transcribe(
            str(audio),
            beam_size=1,
            vad_filter=True,
            condition_on_previous_text=False,
        )
        return list(segments)

    try:
        segments = transcribe(model)
    except Exception as error:
        if selected_device != "cuda":
            raise
        diagnostics.append(f"whisper_cuda_runtime_fallback:{type(error).__name__}")
        model = WhisperModel(
            str(model_path),
            device="cpu",
            compute_type="int8",
            local_files_only=True,
        )
        selected_device = "cpu"
        segments = transcribe(model)
    for segment in segments:
        text = segment.text.strip()
        if text:
            add_observation(
                observations,
                "transcript_candidate",
                round(float(segment.start) * 1000),
                round(float(segment.end) * 1000),
                text,
            )
    executed.add("asr")
    return f"{model_id};device={selected_device};faster-whisper={package_version('faster-whisper')}"


def _kmeans_two(features: Any) -> list[int]:
    import numpy as np

    matrix = np.asarray(features, dtype=np.float64)
    if len(matrix) < 2:
        return [0] * len(matrix)
    scale = matrix.std(axis=0)
    scale[scale < 1e-8] = 1.0
    matrix = (matrix - matrix.mean(axis=0)) / scale
    first = matrix[0].copy()
    distances = ((matrix - first) ** 2).sum(axis=1)
    second = matrix[int(np.argmax(distances))].copy()
    centroids = np.stack([first, second])
    labels = np.zeros(len(matrix), dtype=np.int64)
    for _ in range(20):
        distances = ((matrix[:, None, :] - centroids[None, :, :]) ** 2).sum(axis=2)
        updated = distances.argmin(axis=1)
        if np.array_equal(updated, labels):
            break
        labels = updated
        for cluster in (0, 1):
            members = matrix[labels == cluster]
            if len(members):
                centroids[cluster] = members.mean(axis=0)
    return [int(value) for value in labels]


def analyze_diarization(
    audio: Path | None,
    observations: list[dict[str, Any]],
    executed: set[str],
    diagnostics: list[str],
) -> None:
    if audio is None:
        return
    try:
        import numpy as np

        with wave.open(str(audio), "rb") as stream:
            rate = stream.getframerate()
            channels = stream.getnchannels()
            width = stream.getsampwidth()
            payload = stream.readframes(stream.getnframes())
        if channels != 1 or width != 2 or rate <= 0:
            diagnostics.append("diarization_requires_pcm16_mono")
            return
        samples = np.frombuffer(payload, dtype="<i2")
        if len(samples) > MAX_PCM_SAMPLES:
            diagnostics.append("acoustic_cluster_pcm_budget_exceeded")
            return
        samples = samples.astype(np.float64)
        window = rate
        features: list[list[float]] = []
        windows: list[tuple[int, int]] = []
        for start in range(0, len(samples), window):
            chunk = samples[start : start + window]
            if len(chunk) < rate // 4:
                continue
            rms = float(math.sqrt(float(np.mean(chunk * chunk)) + 1e-9))
            if rms < 120.0:
                continue
            sign_changes = float(np.mean(np.abs(np.diff(np.signbit(chunk)))))
            spectrum = np.abs(np.fft.rfft(chunk * np.hanning(len(chunk))))
            frequencies = np.fft.rfftfreq(len(chunk), 1.0 / rate)
            centroid = float((spectrum * frequencies).sum() / max(float(spectrum.sum()), 1e-9))
            features.append([math.log1p(rms), sign_changes, centroid / rate])
            windows.append((round(start * 1000 / rate), round((start + len(chunk)) * 1000 / rate)))
        labels = _kmeans_two(features)
        merged: list[tuple[int, int, int]] = []
        for (start_ms, end_ms), label in zip(windows, labels):
            if merged and merged[-1][2] == label and merged[-1][1] == start_ms:
                previous = merged[-1]
                merged[-1] = (previous[0], end_ms, label)
            else:
                merged.append((start_ms, end_ms, label))
        for start_ms, end_ms, label in merged:
            add_observation(
                observations,
                "acoustic_cluster_candidate",
                start_ms,
                end_ms,
                f"anonymous_acoustic_cluster_{label}_candidate;not_identity_or_diarization",
                track_id=label,
            )
        executed.add("acoustic_clustering")
    except Exception as error:  # pragma: no cover - diagnostics boundary
        diagnostics.append(f"diarization_failed:{type(error).__name__}")


def main() -> int:
    global MAX_FRAMES, MAX_OBSERVATIONS, MAX_TEMP_BYTES, MAX_PCM_SAMPLES
    parser = argparse.ArgumentParser()
    parser.add_argument("--media", required=True, type=Path)
    parser.add_argument("--ffmpeg", required=True, type=Path)
    parser.add_argument("--model-cache", required=True, type=Path)
    parser.add_argument("--mode", choices=["all"], default="all")
    parser.add_argument("--max-temp-bytes", required=True, type=int)
    parser.add_argument("--max-frames", required=True, type=int)
    parser.add_argument("--max-pcm-samples", required=True, type=int)
    parser.add_argument("--max-observations", required=True, type=int)
    args = parser.parse_args()
    if min(
        args.max_temp_bytes,
        args.max_frames,
        args.max_pcm_samples,
        args.max_observations,
    ) <= 0:
        raise ValueError("resource budgets must be positive")
    MAX_TEMP_BYTES = args.max_temp_bytes
    MAX_FRAMES = args.max_frames
    MAX_PCM_SAMPLES = args.max_pcm_samples
    MAX_OBSERVATIONS = args.max_observations
    media = args.media.resolve(strict=True)
    ffmpeg = args.ffmpeg.resolve(strict=True)
    ffprobe = ffmpeg.with_name("ffprobe.exe" if ffmpeg.suffix.lower() == ".exe" else "ffprobe")
    if not ffprobe.is_file():
        raise FileNotFoundError(f"ffprobe sibling not found: {ffprobe}")

    diagnostics: list[str] = []
    observations: list[dict[str, Any]] = []
    executed: set[str] = {"decode"}
    probe = probe_media(ffprobe, media)
    diagnostics.append(f"ffprobe_streams={len(probe.get('streams', []))}")
    with tempfile.TemporaryDirectory(prefix="lc631-media-") as temporary:
        frames, audio = extract_media(ffmpeg, media, Path(temporary), diagnostics)
        if tree_size(Path(temporary)) > MAX_TEMP_BYTES:
            raise RuntimeError("temporary decode budget exceeded")
        analyze_vision(frames, observations, executed, diagnostics)
        asr_revision = analyze_asr(audio, args.model_cache, observations, executed, diagnostics)
        analyze_diarization(audio, observations, executed, diagnostics)
        if any(item["kind"] == "transcript_candidate" for item in observations) and frames:
            executed.add("temporal_co_presence")
            add_observation(
                observations,
                "temporal_co_presence_candidate",
                0,
                0,
                "audio-video temporal co-presence candidate; semantic alignment unverified",
            )

    versions = {
        "opencv-python-headless": package_version("opencv-python-headless"),
        "numpy": package_version("numpy"),
        "faster-whisper": package_version("faster-whisper"),
    }
    backend_version = canonical_digest(versions)
    result = {
        "schema_version": SCHEMA,
        "source_sha256": sha256_file(media),
        "backend_id": "lc631-local-ffmpeg-opencv-whisper-anonymous-diarization",
        "backend_version": backend_version,
        "model_revision": asr_revision,
        "model_cache_manifest_sha256": tree_manifest_digest(args.model_cache),
        "model_card_license_hint": model_card_license_hint(args.model_cache),
        "executed_families": sorted(executed),
        "executed_methods": {
            "decode": "ffmpeg-cli",
            "shot_boundary": "opencv-hsv-histogram-bhattacharyya",
            "object_detection": "opencv-hog-default-people-detector",
            "frame_difference": "opencv-mean-absolute-frame-difference",
            "asr": "faster-whisper-local-cache",
            "acoustic_clustering": "pcm-feature-two-cluster",
            "temporal_co_presence": "timestamp-overlap-only",
        },
        "observations": observations,
        "diagnostics": diagnostics,
    }
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
