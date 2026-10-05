#!/usr/bin/env python3
"""Rebuild the curated R53 evidence tarball from the scratch outputs."""
from __future__ import annotations
import argparse
import gzip
import hashlib
import io
import tarfile
from pathlib import Path

RUNS = (
    "equal-3win", "best-3win-1", "best-3win-4", "share-3win-1", "share-3win-4",
    "equal-full-1", "equal-full-2", "best-full-1", "share-full-1",
)
AUX = (
    "invariance.json", "equal-parity.json", "equal-version-parity.json",
    "share-invariance.json", "share-summary.json", "test.log", "test-upper-share.log",
)

def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("scratch", nargs="?", type=Path, default=Path("/private/tmp/golaberto-r53"))
    ap.add_argument("--artifact-dir", type=Path, default=Path(__file__).resolve().parent)
    args = ap.parse_args()
    scratch = args.scratch.resolve()
    artifact = args.artifact_dir.resolve()
    archive = artifact / "raw-evidence.tar.gz"
    r52 = Path("/private/tmp/golaberto-r52")
    files: dict[str, bytes] = {}
    for name in ("request-16498.json", "certified-paths.json"):
        candidates = (scratch / name, r52 / name, r52 / ("certified-paths.json" if name == "certified-paths.json" else "request-16498.json"))
        source = next((p for p in candidates if p.is_file()), None)
        if source is None:
            raise SystemExit(f"missing required input {name}; checked {candidates}")
        files[f"raw/{name}"] = source.read_bytes()
    for stem in RUNS:
        for suffix in (".json", ".stdout", ".time"):
            source = scratch / f"{stem}{suffix}"
            if source.is_file():
                files[f"raw/{source.name}"] = source.read_bytes()
        if f"raw/{stem}.json" not in files:
            raise SystemExit(f"missing run output {stem}.json")
    for name in AUX:
        source = scratch / name
        if source.is_file():
            files[f"raw/{name}"] = source.read_bytes()
    metadata = artifact / "commands-metadata.json"
    if not metadata.is_file():
        raise SystemExit(f"missing root metadata {metadata}")
    files["commands-metadata.json"] = metadata.read_bytes()
    manifest = "".join(f"{hashlib.sha256(data).hexdigest()}  {name}\n" for name, data in sorted(files.items())).encode()
    archive.parent.mkdir(parents=True, exist_ok=True)
    with archive.open("wb") as raw, gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w") as tf:
            for name, data in sorted(files.items()):
                info = tarfile.TarInfo(name)
                info.size = len(data)
                info.mtime = 0
                tf.addfile(info, io.BytesIO(data))
            info = tarfile.TarInfo("SHA256SUMS")
            info.size = len(manifest)
            info.mtime = 0
            tf.addfile(info, io.BytesIO(manifest))
    print(f"members={len(files)} archive_bytes={archive.stat().st_size}")

if __name__ == "__main__":
    main()
