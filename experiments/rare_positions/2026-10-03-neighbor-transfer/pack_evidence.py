#!/usr/bin/env python3
"""Package selected R54 run/invariant evidence with SHA256 verification."""
from __future__ import annotations
import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import tarfile

RAW_SUFFIXES = {".json", ".log", ".csv", ".stdout", ".stderr", ".time", ".txt"}
EXCLUDED_NAMES = {"analysis.json", "summary.json", "raw-evidence.manifest.json"}


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def parse_roots(values: list[str]) -> list[tuple[str, Path]]:
    roots = []
    for entry in values:
        if "=" not in entry:
            raise ValueError(f"--source must be PREFIX=DIR: {entry!r}")
        prefix, directory = entry.split("=", 1)
        p = PurePosixPath(prefix)
        if not prefix or p.is_absolute() or ".." in p.parts or len(p.parts) != 1:
            raise ValueError(f"invalid archive prefix {prefix!r}")
        root = Path(directory).resolve(strict=True)
        if not root.is_dir():
            raise ValueError(f"source is not a directory: {root}")
        roots.append((prefix, root))
    if len({prefix for prefix, _ in roots}) != len(roots):
        raise ValueError("source prefixes must be unique")
    return roots


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("artifact_dir", type=Path)
    ap.add_argument("--request", type=Path, required=True)
    ap.add_argument("--source", action="append", default=[], metavar="PREFIX=DIR",
                    help="include raw files from a run/invariant directory")
    ap.add_argument("--file", action="append", default=[], metavar="ARCHIVE_PATH=FILE",
                    help="include a specific candidate patch/test artifact")
    args = ap.parse_args()
    artifact = args.artifact_dir.resolve()
    artifact.mkdir(parents=True, exist_ok=True)
    request = args.request.resolve(strict=True)
    roots = parse_roots(args.source)
    sources: dict[str, Path] = {"raw/input/request-16498.json": request}
    for prefix, root in roots:
        for path in sorted(root.rglob("*")):
            if path.is_symlink() or not path.is_file() or path.suffix not in RAW_SUFFIXES:
                continue
            if path.name in EXCLUDED_NAMES:
                continue
            rel = path.relative_to(root)
            archive_path = PurePosixPath("raw", prefix, *rel.parts).as_posix()
            if archive_path in sources:
                raise ValueError(f"duplicate archive path {archive_path}")
            sources[archive_path] = path
    for entry in args.file:
        if "=" not in entry:
            raise ValueError(f"--file must be ARCHIVE_PATH=FILE: {entry!r}")
        name, filename = entry.split("=", 1)
        member = PurePosixPath(name)
        if member.is_absolute() or ".." in member.parts or not name:
            raise ValueError(f"invalid archive path {name!r}")
        path = Path(filename).resolve(strict=True)
        if not path.is_file():
            raise ValueError(f"not a file: {path}")
        if name in sources:
            raise ValueError(f"duplicate archive path {name}")
        sources[name] = path
    rows = [{"path": name, "bytes": path.stat().st_size, "sha256": sha256_bytes(path.read_bytes())}
            for name, path in sorted(sources.items())]
    manifest = {"members": rows, "count": len(rows)}
    manifest_path = artifact / "raw-evidence.manifest.json"
    archive_path = artifact / "raw-evidence.tar.gz"
    artifact.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    manifest_bytes = "".join(f"{row['sha256']}  {row['path']}\n" for row in rows).encode()
    with archive_path.open("wb") as raw:
        with gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
                for name, source in sorted(sources.items()):
                    data = source.read_bytes()
                    info = tarfile.TarInfo(name)
                    info.size = len(data); info.mtime = info.uid = info.gid = 0
                    info.uname = info.gname = ""
                    archive.addfile(info, io.BytesIO(data))
                info = tarfile.TarInfo("SHA256SUMS")
                info.size = len(manifest_bytes); info.mtime = info.uid = info.gid = 0
                info.uname = info.gname = ""
                archive.addfile(info, io.BytesIO(manifest_bytes))
    verify(archive_path, rows)
    print(json.dumps({"archive": str(archive_path), "bytes": archive_path.stat().st_size,
                      "members": len(rows), "sha256": sha256_bytes(archive_path.read_bytes())}, indent=2))
    return 0


def verify(path: Path, rows: list[dict]) -> None:
    expected = {row["path"]: row for row in rows}
    seen = set()
    with tarfile.open(path, "r:gz") as archive:
        for member in archive.getmembers():
            if not member.isfile() or member.name == "SHA256SUMS":
                continue
            if member.name not in expected or member.name in seen:
                raise RuntimeError(f"unexpected/duplicate evidence member {member.name}")
            f = archive.extractfile(member)
            if f is None:
                raise RuntimeError(f"cannot read {member.name}")
            data = f.read(); row = expected[member.name]
            if len(data) != row["bytes"] or sha256_bytes(data) != row["sha256"]:
                raise RuntimeError(f"checksum mismatch for {member.name}")
            seen.add(member.name)
    if seen != set(expected):
        raise RuntimeError(f"archive missing members: {sorted(set(expected)-seen)}")

if __name__ == "__main__":
    raise SystemExit(main())
