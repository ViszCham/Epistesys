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
import os
import re
import time
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
    entries: list[list[Any]] = []
    if root.is_dir():
        paths = list(root.rglob("*"))
        if any(path.is_symlink() for path in paths):
            raise ValueError("symlink model entries are not authorized")
        files = sorted((path for path in paths if path.is_file()),
                       key=lambda path: path.relative_to(root).as_posix().encode("utf-8"))
        if len(files) > 100_000:
            raise ValueError("model entry budget exceeded")
        for path in files:
            entries.append([path.relative_to(root).as_posix(), path.stat().st_size, sha256_file(path)])
    basis = "lc631-directory-manifest.v3\0" + json.dumps(entries, ensure_ascii=False, separators=(",", ":"))
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


class ResourceBudgetExceeded(RuntimeError):
    """A limit was reached before additional output was retained or written."""


def _pipe_chunk(stream: Any, limit: int = 65536) -> bytes | None:
    if os.name == "nt":
        import ctypes
        import msvcrt
        from ctypes import wintypes

        available = wintypes.DWORD()
        peek = ctypes.WinDLL("kernel32", use_last_error=True).PeekNamedPipe
        peek.argtypes = [wintypes.HANDLE, wintypes.LPVOID, wintypes.DWORD,
                         wintypes.LPVOID, ctypes.POINTER(wintypes.DWORD), wintypes.LPVOID]
        peek.restype = wintypes.BOOL
        if not peek(msvcrt.get_osfhandle(stream.fileno()), None, 0, None,
                    ctypes.byref(available), None):
            error = ctypes.get_last_error()
            if error in (109, 233):  # Broken/disconnected owned anonymous pipe.
                return b""
            raise OSError(error, "owned subprocess pipe observation failed")
        if not available.value:
            return None
        return os.read(stream.fileno(), min(limit, available.value))
    return stream.read(limit)  # FileIO is made nonblocking by the caller.


def _run_binary_bounded(
    command: list[str],
    timeout: float,
    stdout_limit: int,
    stderr_limit: int = 65536,
    stdout_sink: Any = None,
) -> subprocess.CompletedProcess[bytes]:
    capture_limit = 16000 * 60 * 60 * 3 if stdout_sink is not None else 16 * 1024 * 1024
    if (not isinstance(timeout, (int, float)) or not math.isfinite(timeout)
            or timeout <= 0 or timeout > 180
            or type(stdout_limit) is not int or not 0 <= stdout_limit <= capture_limit
            or type(stderr_limit) is not int or not 0 <= stderr_limit <= 1024 * 1024):
        raise ValueError("invalid subprocess resource budget")
    process = subprocess.Popen(
        command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, bufsize=0,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )
    assert process.stdout is not None and process.stderr is not None
    if os.name != "nt":
        os.set_blocking(process.stdout.fileno(), False)
        os.set_blocking(process.stderr.fileno(), False)
    deadline = time.monotonic() + timeout
    output = bytearray()
    errors = bytearray()
    observed = 0
    stdout_eof = stderr_eof = False
    try:
        while True:
            progressed = False
            if not stdout_eof:
                block = _pipe_chunk(process.stdout)
                if block == b"":
                    stdout_eof = True
                elif block is not None:
                    progressed = True
                    if observed + len(block) > stdout_limit:
                        raise ResourceBudgetExceeded("subprocess stdout budget exceeded; coverage incomplete")
                    observed += len(block)
                    if stdout_sink is None:
                        output.extend(block)
                    else:
                        stdout_sink.feed(block)
            if not stderr_eof:
                block = _pipe_chunk(process.stderr)
                if block == b"":
                    stderr_eof = True
                elif block is not None:
                    progressed = True
                    if len(errors) + len(block) > stderr_limit:
                        raise ResourceBudgetExceeded("subprocess stderr budget exceeded; coverage incomplete")
                    errors.extend(block)
            status = process.poll()
            if status is not None and stdout_eof and stderr_eof:
                break
            if time.monotonic() >= deadline:
                raise subprocess.TimeoutExpired(command, timeout)
            if not progressed:
                time.sleep(0.002)
        if status:
            raise subprocess.CalledProcessError(status, command, bytes(output), bytes(errors))
        return subprocess.CompletedProcess(command, status, bytes(output), bytes(errors))
    finally:
        if process.poll() is None:
            process.kill()
        try:
            process.wait(timeout=2)
        finally:
            process.stdout.close()
            process.stderr.close()
        # The external Rust supervisor owns the entire Job/process group. This
        # inner runner owns no detached reader thread and claims no OS sandbox.


def run_checked(
    command: list[str], timeout: int = 180,
    stdout_limit: int = 1024 * 1024, stderr_limit: int = 65536,
) -> subprocess.CompletedProcess[str]:
    result = _run_binary_bounded(command, timeout, stdout_limit, stderr_limit)
    return subprocess.CompletedProcess(
        result.args, result.returncode,
        result.stdout.decode("utf-8", errors="strict"),
        result.stderr.decode("utf-8", errors="replace"),
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
        raise RuntimeError("observation budget exceeded; coverage incomplete")
    label_bytes = label.encode("utf-8")
    truncated = len(label_bytes) > 4096
    bounded_label = label_bytes[:4096].decode("utf-8", errors="ignore")
    basis = {
        "kind": kind,
        "start_ms": max(0, int(start_ms)),
        "end_ms": max(max(0, int(start_ms)), int(end_ms)),
        "track_id": track_id,
        "label": bounded_label,
        "label_truncated": truncated,
        "original_label_bytes": len(label_bytes),
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


class _TempArena:
    def __init__(self, root: Path, limit: int):
        if type(limit) is not int or limit < 46 or limit > 8 * 1024 * 1024 * 1024:
            raise ValueError("temporary budget cannot hold one PCM sample and WAV header")
        if not root.is_dir() or root.is_symlink() or any(root.iterdir()):
            raise ValueError("decode requires an empty owned temporary directory")
        self.root = root
        self.limit = limit
        self.reserved = 0

    def reserve(self, count: int) -> None:
        if count < 0 or count > self.limit - self.reserved:
            raise ResourceBudgetExceeded("temporary decode budget exceeded before write; coverage incomplete")
        self.reserved += count

    def write(self, name: str, payload: bytes) -> Path:
        self.reserve(len(payload))
        path = self.root / name
        with path.open("xb") as stream:
            stream.write(payload)
        return path


class _PpmFrames:
    MAX_PIXELS = 640 * 640
    MAX_FRAME_BYTES = MAX_PIXELS * 3 + 64
    HEADER = re.compile(rb"^P6\n([0-9]{1,3}) ([0-9]{1,3})\n255\n")

    def __init__(self, arena: _TempArena, maximum: int):
        self.arena = arena
        self.maximum = maximum
        self.pending = bytearray()
        self.paths: list[Path] = []

    def feed(self, block: bytes) -> None:
        if len(block) > 65536:
            raise ResourceBudgetExceeded("decoder chunk exceeds bounded pump size")
        self.pending.extend(block)
        while self.pending:
            header = self.HEADER.match(self.pending)
            if header is None:
                if len(self.pending) > 64:
                    raise ValueError("decoder emitted an unsupported PPM header")
                return
            width, height = map(int, header.groups())
            if not (0 < width <= 640 and 0 < height <= 640):
                raise ResourceBudgetExceeded("decoded frame dimensions exceed bounded profile")
            size = header.end() + width * height * 3
            if len(self.pending) < size:
                return
            if len(self.paths) >= self.maximum:
                raise ResourceBudgetExceeded("decoded frame count exceeded; coverage incomplete")
            payload = bytes(self.pending[:size])
            self.paths.append(self.arena.write(f"frame-{len(self.paths):05d}.ppm", payload))
            del self.pending[:size]

    def finish(self) -> None:
        if self.pending:
            raise ValueError("decoder ended with an incomplete frame; coverage incomplete")


class _PcmAudio:
    def __init__(self, arena: _TempArena, maximum_samples: int):
        self.arena = arena
        self.maximum_bytes = maximum_samples * 2
        self.written = 0
        self.pending = b""
        self.writer: Any = None
        self.path: Path | None = None

    def feed(self, block: bytes) -> None:
        if len(block) > 65536:
            raise ResourceBudgetExceeded("PCM chunk exceeds bounded pump size")
        data = self.pending + block
        complete = len(data) - len(data) % 2
        self.pending = data[complete:]
        data = data[:complete]
        if not data:
            return
        if len(data) > self.maximum_bytes - self.written:
            raise ResourceBudgetExceeded("PCM sample budget exceeded before write; coverage incomplete")
        if self.writer is None:
            self.arena.reserve(44)  # Python wave emits a fixed PCM16/mono RIFF header.
            self.path = self.arena.root / "audio-16khz-mono.wav"
            self.writer = wave.open(str(self.path), "wb")
            self.writer.setnchannels(1)
            self.writer.setsampwidth(2)
            self.writer.setframerate(16000)
        self.arena.reserve(len(data))
        self.writer.writeframesraw(data)
        self.written += len(data)

    def close(self) -> None:
        if self.writer is not None:
            self.writer.close()
            self.writer = None

    def finish(self) -> None:
        self.close()
        if self.pending:
            raise ValueError("decoder emitted an incomplete PCM sample")


def extract_media(ffmpeg: Path, media: Path, root: Path, diagnostics: list[str]) -> tuple[list[Path], Path | None]:
    if not (0 < MAX_FRAMES <= 120 and 0 < MAX_PCM_SAMPLES <= 16000 * 60 * 60):
        raise ValueError("decode request exceeds bounded frame/sample profile")
    arena = _TempArena(root, MAX_TEMP_BYTES)
    frames = _PpmFrames(arena, MAX_FRAMES)
    common = [str(ffmpeg), "-nostdin", "-hide_banner", "-loglevel", "error",
              "-max_alloc", str(64 * 1024 * 1024), "-protocol_whitelist", "file,pipe",
              "-i", str(media)]
    try:
        _run_binary_bounded(
            common + ["-vf", "fps=2,scale=640:640:force_original_aspect_ratio=decrease",
                      "-frames:v", str(MAX_FRAMES), "-pix_fmt", "rgb24",
                      "-vcodec", "ppm", "-f", "image2pipe", "pipe:1"],
            180, MAX_FRAMES * _PpmFrames.MAX_FRAME_BYTES, stdout_sink=frames,
        )
        frames.finish()
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        diagnostics.append(f"video_decode_unavailable:{type(error).__name__};coverage_incomplete")
    if len(frames.paths) == MAX_FRAMES:
        diagnostics.append("video_frame_limit_reached;coverage_may_be_bounded")
    if arena.limit - arena.reserved < 46:
        raise ResourceBudgetExceeded("remaining temporary budget cannot hold one PCM sample; audio not started")
    audio = _PcmAudio(arena, MAX_PCM_SAMPLES)
    try:
        _run_binary_bounded(
            common + ["-vn", "-ac", "1", "-ar", "16000", "-af",
                      f"atrim=end_sample={MAX_PCM_SAMPLES}", "-t", str(MAX_PCM_SAMPLES / 16000),
                      "-c:a", "pcm_s16le", "-f", "s16le", "pipe:1"],
            180, MAX_PCM_SAMPLES * 2, stdout_sink=audio,
        )
        audio.finish()
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        diagnostics.append(f"audio_decode_unavailable:{type(error).__name__};coverage_incomplete")
    finally:
        audio.close()
    if audio.written == MAX_PCM_SAMPLES * 2:
        diagnostics.append("audio_sample_limit_reached;coverage_may_be_bounded")
    diagnostics.append(f"temporary_bytes_reserved_before_write={arena.reserved}")
    return frames.paths, audio.path


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
            executed.add("frame_difference")
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
            executed.add("shot_boundary")
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
        executed.add("object_detection")
        for box, weight in zip(boxes[:16], weights[:16]):
            x, y, width, height = [int(value) for value in box]
            add_observation(
                observations,
                "object_box",
                start_ms,
                start_ms + 500,
                f"anonymous_person_box_candidate:x={x},y={y},w={width},h={height},score={float(weight):.4f}",
            )
        faces = face.detectMultiScale(gray, scaleFactor=1.1, minNeighbors=5, minSize=(24, 24))
        executed.add("face_detection")
        for x, y, width, height in faces[:16]:
            add_observation(
                observations,
                "face_region_candidate",
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
        bounded = []
        for segment in segments:
            if len(bounded) >= MAX_OBSERVATIONS:
                raise RuntimeError("ASR observation budget exceeded")
            bounded.append(segment)
        return bounded

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
            if stream.getnframes() * channels > MAX_PCM_SAMPLES:
                raise RuntimeError("PCM sample budget exceeded before reading")
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


def configure_worker_resource_limits() -> dict[str, Any]:
    """Expose the actual platform scope, never equating commit/AS with RSS."""
    if os.name != "nt":
        import resource

        requested = 4 * 1024 * 1024 * 1024
        soft, hard = resource.getrlimit(resource.RLIMIT_AS)
        limits = [requested] + [value for value in (soft, hard) if value != resource.RLIM_INFINITY]
        limit = min(limits)
        resource.setrlimit(resource.RLIMIT_AS, (limit, limit))
        return {
            "memory_limit_enforced": True, "memory_limit_bytes": limit,
            "memory_observation_state": "enforced_per_process_address_space",
            "memory_limit_kind": "per_process_address_space",
            "rss_hard_limit": False, "aggregate_process_tree_limit": False,
            "temporary_limit_bytes": MAX_TEMP_BYTES, "temporary_enforcement": "reserve_before_write",
        }
    import ctypes
    from ctypes import wintypes

    size = ctypes.c_size_t
    class Basic(ctypes.Structure):
        _fields_ = [
            ("process_time", ctypes.c_longlong), ("job_time", ctypes.c_longlong),
            ("flags", wintypes.DWORD), ("minimum_working_set", size),
            ("maximum_working_set", size), ("active_processes", wintypes.DWORD),
            ("affinity", size), ("priority", wintypes.DWORD), ("scheduling", wintypes.DWORD),
        ]
    class Counters(ctypes.Structure):
        _fields_ = [(name, ctypes.c_ulonglong) for name in
                    ("read_ops", "write_ops", "other_ops", "read_bytes", "write_bytes", "other_bytes")]
    class Extended(ctypes.Structure):
        _fields_ = [
            ("basic", Basic), ("io", Counters), ("process_memory", size),
            ("job_memory", size), ("peak_process_memory", size), ("peak_job_memory", size),
        ]
    query = ctypes.WinDLL("kernel32", use_last_error=True).QueryInformationJobObject
    query.argtypes = [wintypes.HANDLE, ctypes.c_int, wintypes.LPVOID,
                      wintypes.DWORD, ctypes.POINTER(wintypes.DWORD)]
    query.restype = wintypes.BOOL
    limits = Extended()
    if not query(None, 9, ctypes.byref(limits), ctypes.sizeof(limits), None):
        return {"memory_limit_enforced": None, "memory_limit_kind": "host_job_observation_required",
                "memory_observation_state": "unobserved_in_guest",
                "rss_hard_limit": False, "aggregate_process_tree_limit": False,
                "temporary_limit_bytes": MAX_TEMP_BYTES, "temporary_enforcement": "reserve_before_write"}
    if not limits.basic.flags & 0x200 or not limits.job_memory:
        return {"memory_limit_enforced": None, "memory_limit_kind": "host_job_observation_required",
                "memory_observation_state": "unobserved_in_guest",
                "rss_hard_limit": False, "aggregate_process_tree_limit": False,
                "temporary_limit_bytes": MAX_TEMP_BYTES, "temporary_enforcement": "reserve_before_write"}
    return {
        "memory_limit_enforced": True, "memory_limit_bytes": limits.job_memory,
        "memory_observation_state": "enforced_current_job_commit",
        "memory_limit_kind": "job_wide_committed_memory",
        "rss_hard_limit": False, "aggregate_process_tree_limit": True,
        "temporary_limit_bytes": MAX_TEMP_BYTES, "temporary_enforcement": "reserve_before_write",
    }


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
    resource_limits = configure_worker_resource_limits()
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
        if audio is not None:
            with wave.open(str(audio), "rb") as stream:
                if stream.getnframes() * stream.getnchannels() > MAX_PCM_SAMPLES:
                    raise RuntimeError("PCM sample budget exceeded before ASR")
        diagnostics.append("coverage_bounded_by_frame_pcm_and_temporary_budgets")
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
        "resource_limits": resource_limits,
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
            "face_detection": "opencv-haar-frontalface",
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
