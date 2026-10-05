#!/usr/bin/env python3
"""Build and verify the self-contained source bundle for the hit audit."""

import argparse
import gzip
import hashlib
import json
import tarfile
from pathlib import Path


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def included_files(source: Path) -> list[tuple[Path, str]]:
    paths = [
        source / "odds-rust/Cargo.toml",
        source / "odds-rust/Cargo.lock",
        source / "odds-rust/src",
        source / "odds-rust/examples",
        source / "stats/core/Cargo.toml",
        source / "stats/core/src",
    ]
    files: list[tuple[Path, str]] = []
    for path in paths:
        if not path.exists():
            raise FileNotFoundError(path)
        if path.is_file():
            files.append((path, path.relative_to(source).as_posix()))
        else:
            for item in sorted(path.rglob("*")):
                if not item.is_file():
                    continue
                relative = item.relative_to(source)
                if any(part in {"target", ".git", ".codex", "artifacts", "artifact"} for part in relative.parts):
                    continue
                if item.name.startswith(".env") or item.name.endswith((".secret", ".pem", ".key")):
                    continue
                files.append((item, relative.as_posix()))
    return sorted(files, key=lambda pair: pair[1])


def add_file(archive: tarfile.TarFile, path: Path, arcname: str) -> None:
    info = archive.gettarinfo(str(path), arcname=arcname)
    info.uid = info.gid = 0
    info.uname = info.gname = ""
    info.mtime = 0
    info.mode &= 0o777
    with path.open("rb") as stream:
        archive.addfile(info, stream)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True, help="r56 workspace containing odds-rust and stats/core")
    parser.add_argument("--repo", type=Path, required=True, help="Golaberto checkout containing the audit exports")
    parser.add_argument("--out", type=Path, required=True, help="output .tar.gz path")
    args = parser.parse_args()
    source = args.source.resolve()
    repo = args.repo.resolve()
    out = args.out.resolve()
    out.parent.mkdir(parents=True, exist_ok=True)

    files = included_files(source)
    with out.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0, compresslevel=9) as zipped:
            with tarfile.open(fileobj=zipped, mode="w", format=tarfile.PAX_FORMAT) as archive:
                for path, name in files:
                    add_file(archive, path, name)

    seeds = []
    for seed in (808, 1669, 1993, 2281, 2293):
        if seed == 808:
            control = Path("/private/tmp/golaberto-r54/v4-native10-808.json")
            control_label = "R54 v4 native-10 seed-808 control"
        else:
            control = Path(
                f"/private/tmp/golaberto-r54/highbudget-10x-five-seed/"
                f"better-paired-seed{seed}/paired-baseline-seed{seed}-w4.json"
            )
            control_label = f"R54 better-paired seed-{seed} w4 baseline"
        audited = repo / f"experiments/rare_positions/2026-10-03-hit-patterns/seed{seed}-observer-on.json"
        equal = control.read_bytes() == audited.read_bytes()
        seeds.append({
            "seed": seed,
            "control_label": control_label,
            "control_path": str(control),
            "control_sha256": sha256(control),
            "audit_export": str(audited),
            "audit_export_sha256": sha256(audited),
            "byte_identical": equal,
        })

    summary = {
        "archive": str(out),
        "archive_sha256": sha256(out),
        "archive_size_bytes": out.stat().st_size,
        "members": [{"path": name, "sha256": sha256(path), "size_bytes": path.stat().st_size} for path, name in files],
        "excluded": ["target directories", ".git", ".env files", "local artifacts and secrets"],
        "control_comparisons": seeds,
        "all_five_exports_byte_identical": all(row["byte_identical"] for row in seeds),
    }
    summary_path = out.parent / "package-summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps({"archive_sha256": summary["archive_sha256"], "member_count": len(files), "all_five_exports_byte_identical": summary["all_five_exports_byte_identical"], "summary": str(summary_path)}, indent=2))
    if not summary["all_five_exports_byte_identical"]:
        raise SystemExit("one or more full exports differ from their specified controls")


if __name__ == "__main__":
    main()
