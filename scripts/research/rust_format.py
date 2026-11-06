"""Run the same Cargo target roots through rustfmt without Windows argv overflow."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys


def target_roots(metadata: dict) -> list[tuple[str, str]]:
    members = set(metadata["workspace_members"])
    packages = [p for p in metadata["packages"] if p["id"] in members]
    if {p["id"] for p in packages} != members or not packages:
        raise ValueError("incomplete or empty Cargo workspace metadata")
    manifests = {str(Path(p["manifest_path"]).parent.resolve()) for p in packages}
    roots = set()
    for package in packages:
        for dependency in package["dependencies"]:
            if dependency.get("path") and str(Path(dependency["path"]).resolve()) not in manifests:
                raise ValueError("local dependency outside workspace must be enumerated explicitly")
        for target in package["targets"]:
            path = Path(target["src_path"])
            if not path.is_file():
                raise ValueError(f"missing Cargo format target: {path}")
            roots.add((target["edition"], str(path)))
    if not roots:
        raise ValueError("no Rust target roots found")
    return sorted(roots)


def batches(roots: list[tuple[str, str]], limit: int = 6000):
    """Bound command text, not target count; preserve edition and every root."""
    edition = None
    group: list[str] = []
    size = 0
    for next_edition, path in roots:
        cost = len(path.encode("utf-16-le")) // 2 + 3
        if cost > limit:
            raise ValueError("individual rustfmt path exceeds argv budget")
        if group and (next_edition != edition or size + cost > limit):
            yield edition, group
            group, size = [], 0
        edition = next_edition
        group.append(path)
        size += cost
    if group:
        yield edition, group


def format_roots(roots, toolchain: str, check: bool, run=subprocess.run) -> int:
    failed = False
    for edition, paths in batches(roots):
        command = ["rustup", "run", toolchain, "rustfmt", "--edition", edition]
        if check:
            command.append("--check")
        # Out-of-line modules are still traversed, just as cargo fmt does.
        result = run([*command, *paths], check=False)
        failed |= result.returncode != 0
    return 1 if failed else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain", default="1.98.0")
    parser.add_argument("--write", action="store_true", help="apply formatting; default is check-only")
    args = parser.parse_args()
    manifest = Path(__file__).resolve().parents[1] / "lc631" / "Cargo.toml"
    try:
        result = subprocess.run(
            ["cargo", f"+{args.toolchain}", "metadata", "--no-deps", "--locked", "--offline",
             "--format-version", "1", "--manifest-path", str(manifest)],
            check=True, capture_output=True, text=True, encoding="utf-8",
        )
        roots = target_roots(json.loads(result.stdout))
        print(f"rustfmt: {len(roots)} Cargo target roots; check={not args.write}", flush=True)
        return format_roots(roots, args.toolchain, not args.write)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f"Rust format check failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
