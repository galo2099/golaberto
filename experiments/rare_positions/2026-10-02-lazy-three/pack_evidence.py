"""Pack the selected R45-R47 raw evidence and verify its SHA256 manifest."""

import argparse
import hashlib
import json
from pathlib import Path
import tarfile


ALLOWED_SUFFIXES = {".json", ".log", ".txt", ".patch", ".py", ".md"}
TOP_LEVEL_FILES = {
    "controls.log",
    "marginal.log",
    "pair-cached.log",
    "warm-portfolio.log",
    "reconstruct_v1.py",
    "source-hashes.json",
}
EXPERIMENT_DIRS = ("controls", "portfolio", "pair", "pair-cached", "marginal", "warm-portfolio")
EXTRA_EVIDENCE_DIRS = (
    "controls-v3",
    "pair-bounded",
    "warm-pair-bounded",
    "warm-pair-cached",
    "warm-marginal",
)
ARTIFACT_FILES = (
    "analyze.py",
    "aggregate_summary.py",
    "generate_v3_patch.py",
    "invariants.py",
    "pack_evidence.py",
    "commands.txt",
    "final-summary.json",
    "prototype-v1.patch",
    "prototype-v2.patch",
    "prototype-v3.patch",
    "source-hashes.json",
)


def digest(path: Path) -> str:
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def add_file(files: dict[str, Path], source: Path, archive_name: str) -> None:
    if not source.is_file() or source.suffix not in ALLOWED_SUFFIXES:
        return
    files[archive_name] = source


def selected_files(root: Path, artifact: Path) -> dict[str, Path]:
    files: dict[str, Path] = {}
    for name in TOP_LEVEL_FILES:
        source = root / name
        if source.exists():
            add_file(files, source, f"golaberto-r45/{name}")
    # The benchmark root is dedicated to this work, so its top-level logs are
    # the complete arm/build/test progress record and are safe to collect.
    for source in root.glob("*.log"):
        add_file(files, source, f"golaberto-r45/{source.name}")
    for pattern in ("commands*.txt", "commands*.json"):
        for source in root.glob(pattern):
            add_file(files, source, f"golaberto-r45/commands/{source.name}")
    for pattern in ("*sol*review*.txt", "*sol*review*.md", "*sol*notes*.txt", "*sol*notes*.md"):
        for source in root.glob(pattern):
            add_file(files, source, f"golaberto-r45/review-notes/{source.name}")

    for source in root.glob("*-analysis.txt"):
        add_file(files, source, f"golaberto-r45/summaries/{source.name}")
    for source in root.glob("*analysis.json"):
        add_file(files, source, f"golaberto-r45/summaries/{source.name}")

    for directory in (*EXPERIMENT_DIRS, *EXTRA_EVIDENCE_DIRS):
        base = root / directory
        if not base.is_dir():
            continue
        for source in sorted(base.rglob("*")):
            if source.is_file():
                relative = source.relative_to(root)
                add_file(files, source, f"golaberto-r45/{relative.as_posix()}")

    invariants = artifact / "invariants"
    if invariants.is_dir():
        for source in sorted(invariants.rglob("*")):
            if source.is_file():
                relative = source.relative_to(artifact)
                add_file(files, source, f"reproducibility/{relative.as_posix()}")

    for name in ARTIFACT_FILES:
        source = artifact / name
        if source.exists():
            add_file(files, source, f"reproducibility/{name}")
    for pattern in ("*review*.txt", "*review*.md", "*review*.json", "sol-notes.txt", "sol-notes.md"):
        for source in artifact.glob(pattern):
            add_file(files, source, f"reproducibility/review-notes/{source.name}")
    for pattern in ("*summary*.json", "final-summary.txt", "final-summary.md"):
        for source in artifact.glob(pattern):
            add_file(files, source, f"reproducibility/summaries/{source.name}")

    # Include the exact serialized requests named by each paired-run summary.
    repository = artifact.parents[2]
    referenced_inputs: set[str] = set()
    for directory in (*EXPERIMENT_DIRS, *EXTRA_EVIDENCE_DIRS):
        summary_path = root / directory / "summary.json"
        if not summary_path.exists():
            continue
        summary = json.loads(summary_path.read_text())
        referenced_inputs.update((summary.get("inputs") or {}).keys())
    for relative in sorted(referenced_inputs):
        source = repository / relative
        if source.is_file() and source.suffix == ".json":
            add_file(files, source, f"inputs/{relative}")

    return dict(sorted(files.items()))


def source_manifest(files: dict[str, Path]) -> list[dict[str, object]]:
    return [
        {
            "path": archive_name,
            "bytes": source.stat().st_size,
            "sha256": digest(source),
        }
        for archive_name, source in files.items()
    ]


def write_archive(files: dict[str, Path], archive_path: Path) -> None:
    archive_path.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive_path, "w:gz", format=tarfile.PAX_FORMAT) as archive:
        for archive_name, source in files.items():
            info = archive.gettarinfo(str(source), arcname=archive_name)
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            with source.open("rb") as stream:
                archive.addfile(info, stream)

def verify(archive_path: Path, manifest: list[dict[str, object]]) -> None:
    expected = {entry["path"]: entry for entry in manifest}
    observed: set[str] = set()
    with tarfile.open(archive_path, "r:gz") as archive:
        for member in archive.getmembers():
            if member.name not in expected:
                raise RuntimeError(f"unlisted archive member: {member.name}")
            stream = archive.extractfile(member)
            if stream is None:
                raise RuntimeError(f"cannot verify archive member: {member.name}")
            data = stream.read()
            record = expected[member.name]
            if len(data) != record["bytes"]:
                raise RuntimeError(f"size mismatch for {member.name}")
            if hashlib.sha256(data).hexdigest() != record["sha256"]:
                raise RuntimeError(f"SHA256 mismatch for {member.name}")
            observed.add(member.name)
    if observed != set(expected):
        raise RuntimeError("archive members differ from the manifest")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("r45_root", type=Path, help="/private/tmp/golaberto-r45")
    parser.add_argument("destination", type=Path, help="artifact directory")
    args = parser.parse_args()

    root = args.r45_root.resolve(strict=True)
    destination = args.destination.resolve()
    artifact = Path(__file__).resolve().parent
    archive_path = destination / "raw-evidence.tar.gz"
    manifest_path = destination / "raw-evidence.manifest.json"
    files = selected_files(root, artifact)
    if not files:
        raise RuntimeError("no evidence files selected")

    manifest = source_manifest(files)
    write_archive(files, archive_path)
    verify(archive_path, manifest)
    document = {
        "archive": archive_path.name,
        "archive_sha256": digest(archive_path),
        "archive_bytes": archive_path.stat().st_size,
        "verified": True,
        "member_count": len(manifest),
        "members": manifest,
    }
    manifest_path.write_text(json.dumps(document, indent=2) + "\n")
    print(json.dumps({key: value for key, value in document.items() if key != "members"}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
