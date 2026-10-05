"""Generate the v3 source patch and hashes against the frozen ac92 baseline."""

import argparse
import difflib
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile


DEFAULT_REVISION = "ac92a8ab88656c2e20a24b37c1f3c3d3dcfdb4cd"
DEFAULT_ARTIFACT = Path(__file__).resolve().parent


def baseline_sources(repository: Path, revision: str) -> dict[str, bytes]:
    with tempfile.TemporaryDirectory(prefix="golaberto-v3-baseline-") as temp:
        archive_path = Path(temp) / "baseline.tar"
        with archive_path.open("wb") as stream:
            subprocess.run(
                ["git", "-C", str(repository), "archive", revision, "odds-rust/src"],
                stdout=stream,
                check=True,
            )
        with tarfile.open(archive_path) as archive:
            return {
                member.name.removeprefix("odds-rust/src/"): archive.extractfile(member).read()
                for member in archive.getmembers()
                if member.isfile() and member.name.startswith("odds-rust/src/")
            }


def make_patch(old: dict[str, bytes], current_root: Path) -> tuple[str, dict[str, str]]:
    current = {
        path.relative_to(current_root).as_posix(): path.read_bytes()
        for path in current_root.rglob("*.rs")
        if path.is_file()
    }
    changed = sorted(path for path in old.keys() | current.keys() if old.get(path) != current.get(path))
    chunks = []
    hashes = {}
    for relative in changed:
        before = old.get(relative)
        after = current.get(relative)
        if after is not None:
            hashes[f"odds-rust/src/{relative}"] = hashlib.sha256(after).hexdigest()
        old_lines = before.decode("utf-8").splitlines(keepends=True) if before is not None else []
        new_lines = after.decode("utf-8").splitlines(keepends=True) if after is not None else []
        from_name = f"a/odds-rust/src/{relative}" if before is not None else "/dev/null"
        to_name = f"b/odds-rust/src/{relative}" if after is not None else "/dev/null"
        chunks.extend(
            difflib.unified_diff(
                old_lines,
                new_lines,
                fromfile=from_name,
                tofile=to_name,
                lineterm="\n",
            )
        )
    return "".join(chunks), hashes


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("repository", type=Path, help="Golaberto git repository")
    parser.add_argument("isolated_source", type=Path, help="isolated tree containing odds-rust/src")
    parser.add_argument("--revision", default=DEFAULT_REVISION)
    parser.add_argument("--artifact-dir", type=Path, default=DEFAULT_ARTIFACT)
    args = parser.parse_args()

    repository = args.repository.resolve(strict=True)
    source_root = (args.isolated_source.resolve(strict=True) / "odds-rust/src")
    if not source_root.is_dir():
        raise SystemExit(f"missing Rust source directory: {source_root}")
    artifact = args.artifact_dir.resolve(strict=True)
    old = baseline_sources(repository, args.revision)
    patch_text, v3_hashes = make_patch(old, source_root)
    if not patch_text:
        raise SystemExit("isolated source matches the baseline; refusing to write an empty v3 patch")

    hash_path = artifact / "source-hashes.json"
    hash_document = json.loads(hash_path.read_text())
    if "v1" not in hash_document or "v2" not in hash_document:
        raise RuntimeError("source-hashes.json is missing frozen v1/v2 entries")
    hash_document["v3"] = v3_hashes

    (artifact / "prototype-v3.patch").write_text(patch_text)
    hash_path.write_text(json.dumps(hash_document, indent=2) + "\n")
    print(
        json.dumps(
            {
                "revision": args.revision,
                "changed_sources": sorted(v3_hashes),
                "pair_file_added_from_dev_null": (
                    "odds-rust/src/joint_caps/propagated/pair.rs" in v3_hashes
                    and "--- /dev/null\n+++ b/odds-rust/src/joint_caps/propagated/pair.rs" in patch_text
                ),
                "patch": str(artifact / "prototype-v3.patch"),
                "hashes": str(hash_path),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
