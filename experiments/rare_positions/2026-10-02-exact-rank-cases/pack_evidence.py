#!/usr/bin/env python3
"""Pack R52 top-level run evidence; excludes source trees and compiled binaries."""
from __future__ import annotations
import argparse
import hashlib
import io
import tarfile
from pathlib import Path

SUFFIXES = {".json", ".stdout", ".time", ".log", ".txt", ".md"}
EXACT = {"certified-paths.json", "baseline-808.json"}

def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()

def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("evidence_root", type=Path)
    ap.add_argument("output", type=Path)
    args = ap.parse_args()
    root = args.evidence_root.resolve()
    files = sorted(p for p in root.iterdir() if p.is_file() and (p.suffix in SUFFIXES or p.name in EXACT))
    entries = [(p, p.read_bytes()) for p in files]
    metadata = args.output.parent / "commands-metadata.txt"
    if metadata.exists():
        entries.append((metadata, metadata.read_bytes()))
    manifest = "".join(f"{sha256(data)}  {'commands-metadata.txt' if p == metadata else p.name}\n" for p, data in entries).encode()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(args.output, "w:gz") as tf:
        for p, data in entries:
            arcname = "commands-metadata.txt" if p == metadata else p.name
            info = tarfile.TarInfo(arcname)
            info.size = len(data)
            info.mtime = int(p.stat().st_mtime)
            tf.addfile(info, io.BytesIO(data))
        info = tarfile.TarInfo("SHA256SUMS")
        info.size = len(manifest)
        info.mtime = 0
        tf.addfile(info, io.BytesIO(manifest))
    print(f"members={len(entries)} manifest=SHA256SUMS bytes={args.output.stat().st_size}")

if __name__ == "__main__":
    main()
