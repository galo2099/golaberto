#!/usr/bin/env python3
"""Package selected raw logs and exports, then verify every archive item by SHA256."""

import argparse
import gzip
import hashlib
import json
from pathlib import Path, PurePosixPath
import tarfile


RAW_SUFFIXES = {".json", ".log", ".time", ".txt"}
EXCLUDED_JSON = {"summary.json", "analysis.json", "invariants.json", "source-patches.json", "raw-evidence.manifest.json"}


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_sources(entries: list[str]) -> list[tuple[str, Path]]:
    sources = []
    for entry in entries:
        if "=" not in entry:
            raise ValueError(f"--source must be PREFIX=DIR: {entry!r}")
        prefix, directory = entry.split("=", 1)
        path = PurePosixPath(prefix)
        if not prefix or path.is_absolute() or ".." in path.parts or len(path.parts) != 1:
            raise ValueError(f"unsafe archive prefix: {prefix!r}")
        sources.append((prefix, Path(directory).resolve(strict=True)))
    if len({prefix for prefix, _ in sources}) != len(sources):
        raise ValueError("archive prefixes must be unique")
    return sources


def selected_files(sources: list[tuple[str, Path]]) -> dict[str, Path]:
    selected = {}
    for prefix, root in sources:
        if not root.is_dir():
            raise ValueError(f"source must be a directory: {root}")
        for source in sorted(root.rglob("*")):
            if source.is_symlink() or not source.is_file() or source.suffix not in RAW_SUFFIXES:
                continue
            if source.name in EXCLUDED_JSON or source.name.endswith(".command.json"):
                continue
            relative = source.relative_to(root)
            archive_name = PurePosixPath(prefix, *relative.parts).as_posix()
            selected[archive_name] = source
    return dict(sorted(selected.items()))


def manifest_rows(files: dict[str, Path]) -> list[dict]:
    return [
        {"path": name, "bytes": path.stat().st_size, "sha256": sha256_file(path)}
        for name, path in files.items()
    ]


def write_tar(files: dict[str, Path], destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open("wb") as raw:
        with gzip.GzipFile(fileobj=raw, mode="wb", mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
                for name, source in files.items():
                    info = archive.gettarinfo(str(source), arcname=name)
                    info.uid = info.gid = 0
                    info.uname = info.gname = ""
                    info.mtime = 0
                    with source.open("rb") as stream:
                        archive.addfile(info, stream)


def verify_archive(archive_path: Path, rows: list[dict]) -> None:
    expected = {row["path"]: row for row in rows}
    seen = set()
    with tarfile.open(archive_path, "r:gz") as archive:
        for member in archive.getmembers():
            if not member.isfile() or member.name not in expected:
                raise RuntimeError(f"unexpected archive item: {member.name}")
            if member.name in seen:
                raise RuntimeError(f"duplicate archive item: {member.name}")
            stream = archive.extractfile(member)
            if stream is None:
                raise RuntimeError(f"cannot read archive item: {member.name}")
            data = stream.read()
            row = expected[member.name]
            if len(data) != row["bytes"] or sha256_bytes(data) != row["sha256"]:
                raise RuntimeError(f"archive content mismatch: {member.name}")
            seen.add(member.name)
    if seen != set(expected):
        missing = sorted(set(expected) - seen)
        raise RuntimeError(f"archive omitted manifest items: {missing}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", action="append", required=True, metavar="PREFIX=DIR", help="raw output directory; repeat per arm/run")
    parser.add_argument("--output-dir", type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument("--archive-name", default="raw-evidence.tar.gz")
    args = parser.parse_args()

    output = args.output_dir.resolve()
    files = selected_files(parse_sources(args.source))
    if not files:
        raise RuntimeError("no raw JSON exports or logs selected")
    rows = manifest_rows(files)
    manifest_sha = sha256_bytes(json.dumps(rows, sort_keys=True, separators=(",", ":")).encode())
    archive_path = output / args.archive_name
    manifest_path = output / "raw-evidence.manifest.json"
    write_tar(files, archive_path)
    verify_archive(archive_path, rows)
    document = {
        "archive": archive_path.name,
        "archive_bytes": archive_path.stat().st_size,
        "archive_sha256": sha256_file(archive_path),
        "manifest_sha256": manifest_sha,
        "verified": True,
        "member_count": len(rows),
        "members": rows,
    }
    output.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(document, indent=2) + "\n")
    print(json.dumps({key: value for key, value in document.items() if key != "members"}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
