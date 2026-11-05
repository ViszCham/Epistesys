"""Bounded, read-only revalidation of historical TLCX source manifests.

The checker validates a manifest's own recorded file list and aggregate, then
compares those historical SHA-256 pins with bytes currently reachable beneath
the requested repository root. It never rewrites a manifest or creates evidence.
"""

from __future__ import annotations

import argparse
import errno
import hashlib
import json
import os
import re
import stat
import sys
from pathlib import Path, PureWindowsPath
from typing import Any


SCHEMA_VERSION = "epistesys-tlcx-source-snapshot.v1"
OUTPUT_SCHEMA_VERSION = "epistesys-tlcx-snapshot-check.v1"
DIGEST_CONTRACT = (
    "Sort repository-relative paths lexically; concatenate UTF-8 "
    "path<TAB>raw-file-sha256<LF> rows; SHA-256 the resulting UTF-8 bytes."
)

# Public resource limits keep both input and serialized output bounded. The
# current TLCX manifests are far below these limits.
MAX_MANIFEST_BYTES = 1 * 1024 * 1024
MAX_FILE_COUNT = 1024
MAX_PATH_CHARS = 512
MAX_FILE_SIZE_BYTES = 16 * 1024 * 1024
MAX_TOTAL_FILE_BYTES = 256 * 1024 * 1024
READ_CHUNK_BYTES = 64 * 1024
MAX_JSON_NESTING = 128

_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
_WINDOWS_RESERVED_NAMES = {
    "con",
    "prn",
    "aux",
    "nul",
    *(f"com{index}" for index in range(1, 10)),
    *(f"lpt{index}" for index in range(1, 10)),
}


class _CheckError(Exception):
    def __init__(self, code: str) -> None:
        super().__init__(code)
        self.code = code


class _DuplicateJsonKey(ValueError):
    pass


class _OpenedFile:
    """A file descriptor plus any directory handles that must stay pinned."""

    def __init__(
        self,
        fd: int,
        *,
        guard_handles: list[Any] | None = None,
        close_native: Any = None,
    ) -> None:
        self.fd: int | None = fd
        self._guard_handles = list(guard_handles or [])
        self._close_native = close_native

    @property
    def guard_count(self) -> int:
        return len(self._guard_handles)

    def close(self) -> None:
        if self.fd is not None:
            _close_fd(self.fd)
            self.fd = None
        if self._close_native is not None:
            for handle in reversed(self._guard_handles):
                self._close_native(handle)
        self._guard_handles.clear()


class _RootLease:
    """Pin one root generation for a complete manifest scan."""

    def __init__(
        self,
        root: Path,
        *,
        posix_fd: int | None = None,
        windows_pin: _OpenedFile | None = None,
    ) -> None:
        self.root = root
        self.posix_fd = posix_fd
        self._windows_pin = windows_pin

    def verify_current(self) -> None:
        if self.posix_fd is not None:
            try:
                pinned = os.fstat(self.posix_fd)
                current_fd = _open_posix_directory(self.root)
            except (OSError, _CheckError):
                raise _CheckError("repository_root_changed_during_check") from None
            try:
                current = os.fstat(current_fd)
            except OSError:
                raise _CheckError("repository_root_changed_during_check") from None
            finally:
                _close_fd(current_fd)
            if (pinned.st_dev, pinned.st_ino) != (current.st_dev, current.st_ino):
                raise _CheckError("repository_root_changed_during_check")
            return
        if self._windows_pin is None or self._windows_pin.guard_count < 1:
            raise _CheckError("safe_root_identity_pin_unavailable")

    @property
    def stable_status(self) -> str:
        return "same_posix_root_fd_verified_at_end" if self.posix_fd is not None else "windows_ancestors_pinned_through_scan"

    def close(self) -> None:
        if self.posix_fd is not None:
            _close_fd(self.posix_fd)
            self.posix_fd = None
        if self._windows_pin is not None:
            self._windows_pin.close()
            self._windows_pin = None


class _ReadBudget:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.consumed = 0

    @property
    def remaining(self) -> int:
        return max(0, self.limit - self.consumed)

    def charge(self, amount: int) -> None:
        if amount < 0:
            raise _CheckError("read_budget_invariant_failed")
        if amount > self.remaining:
            # os.read is bounded to remaining before each call. Saturate here
            # so an impossible over-return can never create reusable budget.
            self.consumed = self.limit
            raise _CheckError("read_budget_exhausted")
        self.consumed += amount


def _base_report(manifest_path: str | None) -> dict[str, Any]:
    return {
        "schema_version": OUTPUT_SCHEMA_VERSION,
        "historical_evidence": {
            "state": "recorded_manifest_only",
            "authenticated": False,
            "rewritten": False,
            "evidence_minted": False,
        },
        "historical_manifest": {
            "path": manifest_path,
            "integrity": "invalid",
            "sha256": None,
            "recorded_file_count": None,
            "recomputed_recorded_aggregate_sha256": None,
        },
        "current_validation": {
            "status": "not_run",
            "consistency": "per_file_non_atomic",
            "atomic_snapshot": False,
            "release_or_slice_completion_claimed": False,
            "root_identity_status": "not_checked",
            "checked_count": 0,
            "matched_count": 0,
            "mismatch_count": 0,
            "unsupported_count": 0,
            "bytes_read": 0,
            "bytes_limit": MAX_TOTAL_FILE_BYTES,
            "files": [],
        },
        "errors": [],
        "claim_boundary": (
            "internal manifest consistency and current raw-file comparison only; "
            "not authenticated history, semantic validation, evidence, authority, or completion; "
            "symlink/reparse paths, hard-link aliases, or unavailable link counts are Unsupported; "
            "files are observed separately, not as an atomic snapshot or release/slice completion; "
            "Windows directory handles are pinned for the scan, but this is not a host-wide sandbox "
            "or defense against privileged filesystem mutation"
        ),
    }


def _error(report: dict[str, Any], code: str) -> None:
    if code not in report["errors"]:
        report["errors"].append(code)


def _validate_relative_path(value: Any) -> tuple[str, ...] | None:
    if not isinstance(value, str) or not value or len(value) > MAX_PATH_CHARS:
        return None
    if "\\" in value or value.startswith("/") or value.startswith("\\"):
        return None
    if PureWindowsPath(value).drive or PureWindowsPath(value).is_absolute():
        return None
    try:
        value.encode("utf-8", errors="strict")
    except UnicodeEncodeError:
        return None
    parts = value.split("/")
    if any(not part or part in {".", ".."} for part in parts):
        return None
    for part in parts:
        if any(ord(character) < 32 for character in part):
            return None
        if any(character in part for character in ':*?"<>|'):
            return None
        if part.endswith((".", " ")):
            return None
        device_name = part.split(".", 1)[0].casefold()
        if device_name in _WINDOWS_RESERVED_NAMES:
            return None
    return tuple(parts)


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise _DuplicateJsonKey
        result[key] = value
    return result


def _recorded_aggregate(entries: list[dict[str, Any]]) -> str:
    digest = hashlib.sha256()
    for entry in sorted(entries, key=lambda item: item["path"]):
        digest.update(entry["path"].encode("utf-8"))
        digest.update(b"\t")
        digest.update(entry["sha256"].encode("ascii"))
        digest.update(b"\n")
    return digest.hexdigest()


def _validate_manifest_record(record: Any) -> tuple[list[dict[str, Any]] | None, list[str], int | None, str | None]:
    errors: list[str] = []
    if not isinstance(record, dict):
        return None, ["manifest_not_object"], None, None
    if record.get("schema_version") != SCHEMA_VERSION:
        errors.append("manifest_schema_unsupported")
    if record.get("digest_contract") != DIGEST_CONTRACT:
        errors.append("digest_contract_unsupported")

    raw_entries = record.get("files")
    if not isinstance(raw_entries, list):
        errors.append("files_not_array")
        raw_entries = []
    if not raw_entries:
        errors.append("empty_file_list")
    if len(raw_entries) > MAX_FILE_COUNT:
        errors.append("file_count_limit_exceeded")

    declared_count = record.get("file_count")
    if not isinstance(declared_count, int) or isinstance(declared_count, bool) or declared_count < 0:
        errors.append("file_count_invalid")
        declared_count = None
    elif declared_count > MAX_FILE_COUNT:
        errors.append("file_count_limit_exceeded")
    if declared_count is not None and declared_count != len(raw_entries):
        errors.append("file_count_mismatch")

    entries: list[dict[str, Any]] = []
    exact_paths: set[str] = set()
    portable_paths: set[str] = set()
    for raw_entry in raw_entries:
        if not isinstance(raw_entry, dict):
            errors.append("file_entry_not_object")
            continue
        path = raw_entry.get("path")
        parts = _validate_relative_path(path)
        if parts is None:
            errors.append("invalid_repository_relative_path")
            continue
        path_key = path.casefold()
        if path in exact_paths or path_key in portable_paths:
            errors.append("duplicate_path")
            continue
        exact_paths.add(path)
        portable_paths.add(path_key)

        expected_hash = raw_entry.get("sha256")
        if not isinstance(expected_hash, str) or _SHA256_RE.fullmatch(expected_hash) is None:
            errors.append("invalid_sha256")
            continue
        entry: dict[str, Any] = {"path": path, "sha256": expected_hash, "parts": parts}
        if "bytes" in raw_entry:
            expected_bytes = raw_entry["bytes"]
            if (
                not isinstance(expected_bytes, int)
                or isinstance(expected_bytes, bool)
                or expected_bytes < 0
                or expected_bytes > (2**63 - 1)
            ):
                errors.append("invalid_recorded_bytes")
                continue
            entry["bytes"] = expected_bytes
        entries.append(entry)

    aggregate = record.get("aggregate_sha256")
    if not isinstance(aggregate, str) or _SHA256_RE.fullmatch(aggregate) is None:
        errors.append("invalid_aggregate_sha256")
        aggregate = None
    computed = _recorded_aggregate(entries) if len(entries) == len(raw_entries) else None
    if aggregate is not None and computed is not None and aggregate != computed:
        errors.append("aggregate_sha256_mismatch")
    if errors:
        return None, sorted(set(errors)), declared_count, computed
    return entries, [], declared_count, computed


def _close_fd(fd: int) -> None:
    try:
        os.close(fd)
    except OSError:
        pass


def _open_posix_directory(root: Path) -> int:
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0)
    try:
        current_fd = os.open(root.anchor, flags)
    except OSError:
        raise _CheckError("repository_root_unavailable") from None
    try:
        for component in root.parts[1:]:
            try:
                next_fd = os.open(component, flags, dir_fd=current_fd)
            except OSError as exc:
                if exc.errno == errno.ELOOP:
                    raise _CheckError("reparse_or_symlink_unsupported") from None
                raise _CheckError("repository_root_unavailable") from None
            _close_fd(current_fd)
            current_fd = next_fd
        return current_fd
    except Exception:
        _close_fd(current_fd)
        raise


def _open_posix_beneath(
    root: Path,
    parts: tuple[str, ...],
    *,
    pinned_root_fd: int | None = None,
) -> _OpenedFile:
    required = ("O_DIRECTORY", "O_NOFOLLOW")
    if any(not hasattr(os, name) for name in required) or os.open not in os.supports_dir_fd:
        raise _CheckError("safe_anchored_open_unsupported")

    try:
        current_fd = (
            os.dup(pinned_root_fd)
            if pinned_root_fd is not None
            else _open_posix_directory(root)
        )
    except OSError:
        raise _CheckError("safe_anchored_open_unsupported") from None
    except _CheckError:
        raise

    root_fd = current_fd
    try:
        for component in parts[:-1]:
            flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0)
            try:
                child_fd = os.open(component, flags, dir_fd=current_fd)
            except FileNotFoundError:
                raise _CheckError("current_file_missing") from None
            except OSError as exc:
                if exc.errno == errno.ELOOP:
                    raise _CheckError("reparse_or_symlink_unsupported") from None
                if exc.errno == errno.ENOTDIR:
                    raise _CheckError("current_path_not_directory") from None
                raise _CheckError("current_file_unreadable") from None
            if current_fd != root_fd:
                _close_fd(current_fd)
            current_fd = child_fd

        flags = os.O_RDONLY | os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0)
        flags |= getattr(os, "O_NONBLOCK", 0)
        try:
            file_fd = os.open(parts[-1], flags, dir_fd=current_fd)
        except FileNotFoundError:
            raise _CheckError("current_file_missing") from None
        except OSError as exc:
            if exc.errno == errno.ELOOP:
                raise _CheckError("reparse_or_symlink_unsupported") from None
            if exc.errno == errno.ENOTDIR:
                raise _CheckError("current_target_not_regular_file") from None
            raise _CheckError("current_file_unreadable") from None
    finally:
        if current_fd != root_fd:
            _close_fd(current_fd)
        _close_fd(root_fd)

    try:
        file_stat = os.fstat(file_fd)
    except OSError:
        _close_fd(file_fd)
        raise _CheckError("current_file_unreadable") from None
    if not stat.S_ISREG(file_stat.st_mode):
        _close_fd(file_fd)
        raise _CheckError("current_target_not_regular_file")
    return _OpenedFile(file_fd)


def _open_windows_beneath(root: Path, parts: tuple[str, ...]) -> _OpenedFile:
    """Open beneath pinned, non-renamable Windows directory handles.

    Every root/ancestor directory handle stays open until the caller finishes
    reading. Its share mode omits FILE_SHARE_DELETE, preventing rename/delete
    replacement during the read. The final handle also denies writes/deletes.
    If these pins cannot be acquired, validation is typed Unsupported.
    """
    try:
        import ctypes
        import msvcrt
        from ctypes import wintypes
    except ImportError:
        raise _CheckError("safe_windows_handle_open_unsupported") from None

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    create_file = kernel32.CreateFileW
    create_file.argtypes = (
        wintypes.LPCWSTR,
        wintypes.DWORD,
        wintypes.DWORD,
        wintypes.LPVOID,
        wintypes.DWORD,
        wintypes.DWORD,
        wintypes.HANDLE,
    )
    create_file.restype = wintypes.HANDLE
    get_info = kernel32.GetFileInformationByHandleEx
    get_info.argtypes = (wintypes.HANDLE, ctypes.c_int, wintypes.LPVOID, wintypes.DWORD)
    get_info.restype = wintypes.BOOL
    get_final_path = kernel32.GetFinalPathNameByHandleW
    get_final_path.argtypes = (wintypes.HANDLE, wintypes.LPWSTR, wintypes.DWORD, wintypes.DWORD)
    get_final_path.restype = wintypes.DWORD
    get_size = kernel32.GetFileSizeEx
    get_size.argtypes = (wintypes.HANDLE, ctypes.POINTER(ctypes.c_longlong))
    get_size.restype = wintypes.BOOL
    close_handle = kernel32.CloseHandle
    close_handle.argtypes = (wintypes.HANDLE,)
    close_handle.restype = wintypes.BOOL

    invalid_handle = ctypes.c_void_p(-1).value
    file_read_attributes = 0x0080
    generic_read = 0x80000000
    share_read = 0x00000001
    share_write = 0x00000002
    open_existing = 3
    file_flag_open_reparse_point = 0x00200000
    file_flag_backup_semantics = 0x02000000
    file_attribute_directory = 0x00000010
    file_attribute_reparse_point = 0x00000400
    file_attribute_device = 0x00000040
    file_attribute_tag_info_class = 9

    class _FileAttributeTagInfo(ctypes.Structure):
        _fields_ = [("FileAttributes", wintypes.DWORD), ("ReparseTag", wintypes.DWORD)]

    def _open_handle(
        path: str,
        *,
        directory: bool,
        deny_writes: bool = False,
        read_contents: bool = False,
    ) -> Any:
        # No directory handle shares deletion/rename. The read handle also
        # denies writers so the current bytes remain stable until hashing ends.
        share = share_read if deny_writes else share_read | share_write
        flags = file_flag_open_reparse_point
        if directory:
            flags |= file_flag_backup_semantics
        desired_access = file_read_attributes | (generic_read if read_contents else 0)
        handle = create_file(path, desired_access, share, None, open_existing, flags, None)
        if handle == invalid_handle:
            error = ctypes.get_last_error()
            if error == 2 or error == 3:
                raise _CheckError("current_file_missing")
            if error == 5:
                raise _CheckError("reparse_or_symlink_unsupported")
            if error in {32, 33} and directory:
                raise _CheckError("safe_windows_ancestor_pin_unavailable")
            raise _CheckError("current_file_unreadable")
        return handle

    def _attributes(handle: Any) -> int:
        info = _FileAttributeTagInfo()
        if not get_info(handle, file_attribute_tag_info_class, ctypes.byref(info), ctypes.sizeof(info)):
            raise _CheckError("safe_windows_handle_open_unsupported")
        return int(info.FileAttributes)

    def _final_path(handle: Any) -> str:
        buffer = ctypes.create_unicode_buffer(32768)
        length = get_final_path(handle, buffer, len(buffer), 0)
        if length == 0 or length >= len(buffer):
            raise _CheckError("safe_windows_handle_open_unsupported")
        return buffer.value.rstrip("\\")

    def _expected_final_path(path: Path) -> str:
        value = str(path)
        if value.startswith("\\\\?\\"):
            expected = value
        elif value.startswith("\\\\"):
            expected = "\\\\?\\UNC\\" + value[2:]
        else:
            expected = "\\\\?\\" + value
        return expected.rstrip("\\")

    root_prefixes = [root.parts[0]]
    for component in root.parts[1:]:
        root_prefixes.append(os.path.join(root_prefixes[-1], component))
    guard_handles: list[Any] = []
    file_handle = None
    transferred = False
    try:
        root_final = None
        for index, prefix in enumerate(root_prefixes):
            handle = _open_handle(prefix, directory=True)
            guard_handles.append(handle)
            attributes = _attributes(handle)
            if attributes & file_attribute_reparse_point:
                raise _CheckError("reparse_or_symlink_unsupported")
            if not attributes & file_attribute_directory:
                raise _CheckError("repository_root_unavailable")
            if index == len(root_prefixes) - 1:
                root_final = _final_path(handle)
                if root_final.casefold() != _expected_final_path(root).casefold():
                    raise _CheckError("repository_root_identity_unresolved")

        for index in range(len(parts) - 1):
            prefix = root.joinpath(*parts[: index + 1])
            component_handle = _open_handle(str(prefix), directory=True)
            guard_handles.append(component_handle)
            attributes = _attributes(component_handle)
            if attributes & file_attribute_reparse_point:
                raise _CheckError("reparse_or_symlink_unsupported")
            if not attributes & file_attribute_directory:
                raise _CheckError("current_path_not_directory")

        target = root.joinpath(*parts)
        file_handle = _open_handle(
            str(target), directory=False, deny_writes=True, read_contents=True
        )
        attributes = _attributes(file_handle)
        if attributes & file_attribute_reparse_point:
            raise _CheckError("reparse_or_symlink_unsupported")
        if attributes & (file_attribute_directory | file_attribute_device):
            raise _CheckError("current_target_not_regular_file")
        target_final = _final_path(file_handle)
        assert root_final is not None
        if not target_final.casefold().startswith((root_final + "\\").casefold()):
            raise _CheckError("path_outside_repository")

        size = ctypes.c_longlong()
        if not get_size(file_handle, ctypes.byref(size)) or size.value < 0:
            raise _CheckError("current_file_unreadable")
        if size.value > MAX_FILE_SIZE_BYTES:
            raise _CheckError("file_size_limit_exceeded")

        descriptor = msvcrt.open_osfhandle(int(file_handle), os.O_RDONLY | os.O_BINARY)
        if descriptor < 0:
            raise _CheckError("safe_windows_handle_open_unsupported")
        file_handle = None  # ownership transferred to the file descriptor
        opened = _OpenedFile(
            descriptor, guard_handles=guard_handles, close_native=close_handle
        )
        guard_handles = []
        transferred = True
        return opened
    finally:
        if file_handle is not None:
            close_handle(file_handle)
        if not transferred:
            for handle in reversed(guard_handles):
                close_handle(handle)


def _open_file_beneath(
    root: Path,
    parts: tuple[str, ...],
    *,
    root_lease: _RootLease | None = None,
) -> _OpenedFile:
    if not parts:
        raise _CheckError("invalid_repository_relative_path")
    if os.name == "nt":
        opened = _open_windows_beneath(root, parts)
    else:
        opened = _open_posix_beneath(
            root,
            parts,
            pinned_root_fd=root_lease.posix_fd if root_lease is not None else None,
        )
    try:
        file_stat = os.fstat(opened.fd)
    except OSError:
        opened.close()
        raise _CheckError("current_file_unreadable") from None
    link_count = getattr(file_stat, "st_nlink", None)
    if not isinstance(link_count, int) or link_count < 1:
        opened.close()
        raise _CheckError("hardlink_count_unavailable")
    if link_count > 1:
        opened.close()
        raise _CheckError("hardlink_alias_unsupported")
    return opened


def _read_manifest(
    root: Path,
    parts: tuple[str, ...],
    *,
    root_lease: _RootLease | None = None,
) -> tuple[bytes, _OpenedFile]:
    try:
        opened = _open_file_beneath(root, parts, root_lease=root_lease)
    except _CheckError as exc:
        manifest_error = {
            "current_file_missing": "manifest_missing",
            "current_path_not_directory": "manifest_path_not_directory",
            "current_target_not_regular_file": "manifest_not_regular_file",
            "current_file_unreadable": "manifest_read_failed",
        }.get(exc.code, exc.code)
        raise _CheckError(manifest_error) from None
    try:
        assert opened.fd is not None
        fd = opened.fd
        file_stat = os.fstat(fd)
        if file_stat.st_size > MAX_MANIFEST_BYTES:
            raise _CheckError("manifest_too_large")
        chunks: list[bytes] = []
        remaining = MAX_MANIFEST_BYTES + 1
        while remaining > 0:
            chunk = os.read(fd, min(READ_CHUNK_BYTES, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        data = b"".join(chunks)
        if len(data) > MAX_MANIFEST_BYTES:
            raise _CheckError("manifest_too_large")
        return data, opened
    except _CheckError:
        opened.close()
        raise
    except OSError:
        opened.close()
        raise _CheckError("manifest_read_failed") from None


def _hash_current_file(
    root: Path,
    entry: dict[str, Any],
    budget: _ReadBudget,
    root_lease: _RootLease | None = None,
) -> tuple[str, int]:
    opened = _open_file_beneath(root, entry["parts"], root_lease=root_lease)
    try:
        assert opened.fd is not None
        fd = opened.fd
        before = os.fstat(fd)
        if before.st_size < 0:
            raise _CheckError("current_file_unreadable")
        if before.st_size > MAX_FILE_SIZE_BYTES or before.st_size > budget.remaining:
            raise _CheckError("file_size_limit_exceeded")
        digest = hashlib.sha256()
        bytes_read = 0
        while True:
            remaining_file = MAX_FILE_SIZE_BYTES - bytes_read
            remaining_total = budget.remaining
            read_limit = min(READ_CHUNK_BYTES, remaining_file, remaining_total)
            if read_limit <= 0:
                current = os.fstat(fd)
                if current.st_size > bytes_read:
                    raise _CheckError("file_size_limit_exceeded")
                break
            chunk = os.read(fd, read_limit)
            if not chunk:
                break
            if len(chunk) > read_limit:
                raise _CheckError("read_budget_invariant_failed")
            budget.charge(len(chunk))
            bytes_read += len(chunk)
            if bytes_read > MAX_FILE_SIZE_BYTES:
                raise _CheckError("file_size_limit_exceeded")
            digest.update(chunk)
        after = os.fstat(fd)
        if after.st_size > bytes_read and budget.remaining == 0:
            raise _CheckError("file_size_limit_exceeded")
        stable_before = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns)
        stable_after = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns)
        if stable_before != stable_after or bytes_read != after.st_size:
            raise _CheckError("current_file_changed_during_read")
        return digest.hexdigest(), bytes_read
    except OSError:
        raise _CheckError("current_file_unreadable") from None
    finally:
        opened.close()


def _manifest_sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _json_exceeds_nesting_limit(text: str) -> bool:
    """Bound object/array nesting before handing input to the JSON decoder."""
    depth = 0
    in_string = False
    escaped = False
    for character in text:
        if in_string:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                in_string = False
            continue
        if character == '"':
            in_string = True
        elif character in "[{":
            depth += 1
            if depth > MAX_JSON_NESTING:
                return True
        elif character in "]}":
            depth -= 1
    return False


def check_manifest(repo_root: str | os.PathLike[str], manifest_path: str) -> dict[str, Any]:
    """Return bounded JSON-compatible diagnostics without mutating any input."""
    relative_manifest = _validate_relative_path(manifest_path)
    safe_manifest_label = manifest_path if relative_manifest is not None else None
    report = _base_report(safe_manifest_label)
    if relative_manifest is None:
        _error(report, "invalid_repository_relative_manifest_path")
        return report

    try:
        root = Path(repo_root).absolute()
    except (OSError, RuntimeError, TypeError, ValueError):
        _error(report, "repository_root_unavailable")
        report["historical_manifest"]["integrity"] = "unavailable"
        return report

    root_lease: _RootLease | None = None
    manifest_opened: _OpenedFile | None = None
    try:
        if os.name == "nt":
            manifest_bytes, manifest_opened = _read_manifest(root, relative_manifest)
            # This file lease retains the non-renamable handles from the volume
            # root through the manifest directory until the whole scan ends.
            root_lease = _RootLease(root, windows_pin=manifest_opened)
            manifest_opened = None
        else:
            root_fd = _open_posix_directory(root)
            root_lease = _RootLease(root, posix_fd=root_fd)
            manifest_bytes, manifest_opened = _read_manifest(
                root, relative_manifest, root_lease=root_lease
            )
            manifest_opened.close()
            manifest_opened = None

        report = _check_manifest_bytes(root, report, manifest_bytes, root_lease)
        if report["current_validation"]["status"] != "not_run":
            try:
                root_lease.verify_current()
            except _CheckError as exc:
                report["current_validation"]["status"] = "unsupported"
                report["current_validation"]["root_identity_status"] = "changed_or_unverified"
                _error(report, exc.code)
            else:
                report["current_validation"]["root_identity_status"] = root_lease.stable_status
        return report
    except _CheckError as exc:
        _error(report, exc.code)
        if (
            "unsupported" in exc.code
            or "reparse" in exc.code
            or "outside_repository" in exc.code
            or "identity_unresolved" in exc.code
            or "root_identity" in exc.code
        ):
            integrity = "unsupported"
        elif exc.code == "manifest_too_large":
            integrity = "invalid"
        else:
            integrity = "unavailable"
        report["historical_manifest"]["integrity"] = integrity
        return report
    finally:
        if manifest_opened is not None:
            manifest_opened.close()
        if root_lease is not None:
            root_lease.close()


def _check_manifest_bytes(
    root: Path,
    report: dict[str, Any],
    manifest_bytes: bytes,
    root_lease: _RootLease,
) -> dict[str, Any]:
    report["historical_manifest"]["sha256"] = _manifest_sha256(manifest_bytes)
    try:
        manifest_text = manifest_bytes.decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        _error(report, "manifest_invalid_json")
        return report
    if _json_exceeds_nesting_limit(manifest_text):
        _error(report, "manifest_nesting_limit_exceeded")
        return report
    try:
        record = json.loads(manifest_text, object_pairs_hook=_reject_duplicate_keys)
    except _DuplicateJsonKey:
        _error(report, "duplicate_json_key")
        return report
    except RecursionError:
        _error(report, "manifest_nesting_limit_exceeded")
        return report
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError):
        _error(report, "manifest_invalid_json")
        return report

    entries, validation_errors, declared_count, computed_aggregate = _validate_manifest_record(record)
    report["historical_manifest"]["recorded_file_count"] = declared_count
    report["historical_manifest"]["recomputed_recorded_aggregate_sha256"] = computed_aggregate
    if validation_errors:
        report["errors"] = validation_errors
        return report
    report["historical_manifest"]["integrity"] = "internally_consistent"

    file_results: list[dict[str, Any]] = []
    current_errors: list[str] = []
    budget = _ReadBudget(MAX_TOTAL_FILE_BYTES)
    matched = 0
    mismatched = 0
    unsupported = 0
    for entry in sorted(entries or [], key=lambda item: item["path"]):
        result: dict[str, Any] = {
            "path": entry["path"],
            "status": None,
            "historical_sha256": entry["sha256"],
            "current_sha256": None,
            "historical_bytes": entry.get("bytes"),
            "current_bytes": None,
        }
        file_read_start = budget.consumed
        try:
            if "bytes" in entry and entry["bytes"] > MAX_FILE_SIZE_BYTES:
                raise _CheckError("file_size_limit_exceeded")
            current_hash, current_bytes = _hash_current_file(root, entry, budget, root_lease)
            result["current_sha256"] = current_hash
            result["current_bytes"] = current_bytes
            if current_hash == entry["sha256"] and (
                "bytes" not in entry or current_bytes == entry["bytes"]
            ):
                result["status"] = "matches_historical"
                matched += 1
            else:
                result["status"] = "changed_since_historical"
                mismatched += 1
        except _CheckError as exc:
            bytes_this_file = budget.consumed - file_read_start
            if bytes_this_file:
                result["current_bytes"] = bytes_this_file
            if exc.code == "current_file_missing":
                result["status"] = "missing_from_current"
                mismatched += 1
            else:
                result["status"] = "unsupported"
                unsupported += 1
                current_errors.append(exc.code)
        file_results.append(result)

    report["errors"] = sorted(set(current_errors))
    if unsupported:
        current_status = "unsupported"
    elif mismatched:
        current_status = "mismatch"
    else:
        current_status = "match"
    report["current_validation"] = {
        "status": current_status,
        "consistency": "per_file_non_atomic",
        "atomic_snapshot": False,
        "release_or_slice_completion_claimed": False,
        "root_identity_status": "not_checked",
        "checked_count": matched + mismatched,
        "matched_count": matched,
        "mismatch_count": mismatched,
        "unsupported_count": unsupported,
        "bytes_read": budget.consumed,
        "bytes_limit": budget.limit,
        "files": file_results,
    }
    return report


def _exit_code(report: dict[str, Any]) -> int:
    integrity = report["historical_manifest"]["integrity"]
    current = report["current_validation"]["status"]
    if integrity != "internally_consistent" or current in {"not_run", "unsupported"}:
        return 2
    return 0 if current == "match" else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Read-only bounded comparison of a TLCX historical manifest to the current worktree."
    )
    parser.add_argument("--repo", required=True, help="requested repository root")
    parser.add_argument("--manifest", required=True, help="repository-relative historical manifest path")
    args = parser.parse_args(argv)
    report = check_manifest(args.repo, args.manifest)
    json.dump(report, sys.stdout, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    sys.stdout.write("\n")
    return _exit_code(report)


if __name__ == "__main__":
    raise SystemExit(main())
