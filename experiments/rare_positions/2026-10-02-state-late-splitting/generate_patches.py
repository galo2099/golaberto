#!/usr/bin/env python3
"""Generate self-contained per-arm Rust source patches and SHA256 metadata."""

import argparse
import difflib
import hashlib
import json
from pathlib import Path

BASELINE_COMMIT = "4e812fd5"
BASELINE_SOURCE = "ac92a8ab88656c2e20a24b37c1f3c3d3dcfdb4cd"
ARTIFACT_DIR = Path(__file__).resolve().parent


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def source_root(directory: Path) -> Path:
    resolved = directory.resolve(strict=True)
    nested = resolved / "odds-rust" / "src"
    return nested if nested.is_dir() else resolved


def read_sources(root: Path) -> dict[str, bytes]:
    return {
        path.relative_to(root).as_posix(): path.read_bytes()
        for path in sorted(root.rglob("*.rs"))
        if path.is_file()
    }


def unified_patch(before: dict[str, bytes], after: dict[str, bytes]) -> tuple[str, list[dict]]:
    chunks = []
    changes = []
    for relative in sorted(before.keys() | after.keys()):
        old, new = before.get(relative), after.get(relative)
        if old == new:
            continue
        old_name = f"a/odds-rust/src/{relative}" if old is not None else "/dev/null"
        new_name = f"b/odds-rust/src/{relative}" if new is not None else "/dev/null"
        old_lines = old.decode("utf-8").splitlines(keepends=True) if old is not None else []
        new_lines = new.decode("utf-8").splitlines(keepends=True) if new is not None else []
        chunks.extend(difflib.unified_diff(old_lines, new_lines, old_name, new_name, lineterm="\n"))
        changes.append({
            "filename": f"odds-rust/src/{relative}",
            "original_sha256": digest(old) if old is not None else None,
            "candidate_sha256": digest(new) if new is not None else None,
            "change": "added" if old is None else "deleted" if new is None else "modified",
        })
    return "".join(chunks), changes


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path, help="/tmp/r48/base source tree")
    parser.add_argument("--arm", action="append", required=True, metavar="NAME=DIR", help="candidate source tree; repeat per arm")
    parser.add_argument("--output-dir", type=Path, default=ARTIFACT_DIR)
    parser.add_argument("--baseline-commit", default=BASELINE_COMMIT)
    parser.add_argument("--baseline-source", default=BASELINE_SOURCE)
    args = parser.parse_args()

    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    base = read_sources(source_root(args.baseline))
    arms = {}
    for entry in args.arm:
        if "=" not in entry:
            raise SystemExit(f"--arm must be NAME=DIR: {entry!r}")
        name, directory = entry.split("=", 1)
        candidate = read_sources(source_root(Path(directory)))
        patch_text, changes = unified_patch(base, candidate)
        patch_path = output / f"{name}.patch"
        patch_path.write_text(patch_text)
        arms[name] = {
            "candidate_directory": str(Path(directory).resolve(strict=True)),
            "patch": patch_path.name,
            "patch_sha256": digest(patch_text.encode()),
            "changed_files": changes,
            "candidate_source_sha256": {
                f"odds-rust/src/{path}": digest(content)
                for path, content in sorted(candidate.items())
            },
        }
        if not changes:
            raise RuntimeError(f"candidate arm {name!r} is identical to the baseline")

    metadata = {
        "baseline_commit": args.baseline_commit,
        "baseline_source_revision": args.baseline_source,
        "baseline_directory": str(Path(args.baseline).resolve(strict=True)),
        "baseline_source_sha256": {
            f"odds-rust/src/{path}": digest(content)
            for path, content in sorted(base.items())
        },
        "arms": arms,
    }
    metadata_path = output / "source-patches.json"
    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n")
    print(json.dumps({
        "baseline_commit": metadata["baseline_commit"],
        "baseline_source_revision": metadata["baseline_source_revision"],
        "arms": {
            name: {"patch": arm["patch"], "patch_sha256": arm["patch_sha256"], "changed_file_count": len(arm["changed_files"])}
            for name, arm in arms.items()
        },
        "metadata": str(metadata_path),
    }, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
