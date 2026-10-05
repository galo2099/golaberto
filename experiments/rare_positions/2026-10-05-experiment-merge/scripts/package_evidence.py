#!/usr/bin/env python3
"""Package untracked Oct 2–5 rare-position evidence into verified archives."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tarfile
from pathlib import Path, PurePosixPath

REPO = Path(__file__).resolve().parents[4]
RARE = REPO / "experiments/rare_positions"
MERGE_NAME = "2026-10-05-experiment-merge"
SUMMARY = RARE / MERGE_NAME / "integration-summary.json"
ARCHIVE_NAME = "merge-evidence.tar.gz"
MANIFEST_NAME = "merge-evidence.manifest.json"
DAY_DIR = re.compile(r"^2026-10-0[2-5]-.+")
KEEP_EXT = {".py", ".rb", ".sh", ".rs", ".md", ".markdown"}
ARCHIVE_EXT = (".tar.gz", ".tgz", ".zip")
CACHES = {"__pycache__", ".pytest_cache", ".mypy_cache", ".ruff_cache", ".cache", "node_modules", "target"}


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def safe_rel(value: str) -> PurePosixPath:
    rel = PurePosixPath(value)
    if rel.is_absolute() or not rel.parts or any(p in ("", ".", "..") for p in rel.parts):
        raise ValueError(f"unsafe relative path: {value!r}")
    if "\\" in value or "\x00" in value:
        raise ValueError(f"unsafe path encoding: {value!r}")
    return rel


def source_path(value: str) -> Path:
    rel = safe_rel(value)
    path = REPO.joinpath(*rel.parts)
    cur = REPO
    for part in rel.parts:
        cur = cur / part
        info = cur.lstat()
        if stat.S_ISLNK(info.st_mode):
            raise ValueError(f"symlink in snapshot path: {value}")
    if not stat.S_ISREG(path.lstat().st_mode):
        raise ValueError(f"snapshot member is not a regular file: {value}")
    path.relative_to(REPO)
    return path


def snapshot() -> list[str]:
    proc = subprocess.run(
        ["git", "ls-files", "--others", "--exclude-standard", "--", "experiments/rare_positions"],
        cwd=REPO, check=True, capture_output=True, text=True,
    )
    values = sorted(set(line for line in proc.stdout.splitlines() if line))
    for value in values:
        parts = safe_rel(value).parts
        if len(parts) < 3 or parts[2] != MERGE_NAME:
            source_path(value)
    return values


def exp_root(value: str) -> str | None:
    rel = safe_rel(value)
    if len(rel.parts) < 3 or rel.parts[:2] != ("experiments", "rare_positions"):
        return None
    name = rel.parts[2]
    # This merge directory is owned by the integration workflow and excluded.
    if name == MERGE_NAME or not DAY_DIR.match(name):
        return None
    root = RARE / name
    return name if root.is_dir() and not root.is_symlink() else None


def cache_path(value: str) -> bool:
    return any(part in CACHES or part.endswith(".pyc") for part in safe_rel(value).parts)


def top_level_pairs(root: str, paths: list[str]) -> set[str]:
    prefix = f"experiments/rare_positions/{root}/"
    top = {p[len(prefix):]: p for p in paths if p.startswith(prefix) and "/" not in p[len(prefix):]}
    keep: set[str] = set()
    for name, global_path in top.items():
        if not name.endswith(ARCHIVE_EXT):
            continue
        stem = name
        for suffix in ARCHIVE_EXT:
            if stem.endswith(suffix):
                stem = stem[:-len(suffix)]
                break
        companion_names = {stem + ".manifest.json", name + ".manifest.json"}
        companions = companion_names.intersection(top)
        if companions:
            keep.add(global_path)
            keep.update(top[item] for item in companions)
    return keep


def readable(value: str, root: str, pairs: set[str]) -> bool:
    rel = safe_rel(value)
    prefix_len = len(PurePosixPath("experiments/rare_positions", root).parts)
    within = rel.parts[prefix_len:]
    if Path(rel.name).suffix.lower() in KEEP_EXT:
        return True
    if within and within[0] == "reference":
        return True
    return len(within) == 1 and value in pairs


def item_metadata(path: Path, local: str) -> dict:
    info = path.stat()
    return {
        "path": local,
        "size": info.st_size,
        "sha256": digest(path),
        "mode": stat.S_IMODE(info.st_mode),
        "executable": bool(info.st_mode & 0o111),
    }


def verify_tar(archive: Path, manifest: dict, exp_dir: Path, check_sources: bool = True) -> None:
    if digest(archive) != manifest["archive_sha256"]:
        raise ValueError(f"archive hash mismatch: {archive}")
    expected = {entry["path"]: entry for entry in manifest["members"]}
    observed: set[str] = set()
    with tarfile.open(archive, "r:gz") as tar:
        for member in tar.getmembers():
            rel = safe_rel(member.name)
            name = rel.as_posix()
            if name not in expected or name in observed:
                raise ValueError(f"unexpected or duplicate tar member: {name}")
            if not member.isfile() or member.issym() or member.islnk() or member.isdev():
                raise ValueError(f"unsafe tar member: {name}")
            record = expected[name]
            if member.size != record["size"] or stat.S_IMODE(member.mode) != record["mode"]:
                raise ValueError(f"tar metadata mismatch: {name}")
            stream = tar.extractfile(member)
            if stream is None:
                raise ValueError(f"unreadable tar member: {name}")
            h, size = hashlib.sha256(), 0
            with stream:
                for block in iter(lambda: stream.read(1 << 20), b""):
                    h.update(block)
                    size += len(block)
            if size != record["size"] or h.hexdigest() != record["sha256"]:
                raise ValueError(f"tar member hash mismatch: {name}")
            if check_sources:
                original = exp_dir.joinpath(*rel.parts)
                original = source_path(str(original.relative_to(REPO)))
                info = original.lstat()
                if info.st_size != record["size"] or stat.S_IMODE(info.st_mode) != record["mode"] or digest(original) != record["sha256"]:
                    raise ValueError(f"original changed after packaging: {original}")
            observed.add(name)
    if observed != set(expected):
        raise ValueError(f"tar member set mismatch: {archive}")


def create_archive(root: str, members: list[str]) -> dict:
    exp_dir = RARE / root
    archive, manifest_path = exp_dir / ARCHIVE_NAME, exp_dir / MANIFEST_NAME
    temporary = exp_dir / ".merge-evidence.tar.gz.building"
    if archive.exists() or archive.is_symlink() or manifest_path.exists() or manifest_path.is_symlink():
        raise FileExistsError(f"refusing to overwrite existing packaging output in {exp_dir}")
    if temporary.exists() or temporary.is_symlink():
        raise FileExistsError(f"refusing to overwrite an interrupted packaging file: {temporary}")
    prefix = f"experiments/rare_positions/{root}/"
    records = [
        item_metadata(source_path(path), path[len(prefix):])
        for path in sorted(members)
    ]
    with tarfile.open(temporary, "x:gz", format=tarfile.PAX_FORMAT) as tar:
        for record in records:
            src = exp_dir.joinpath(*safe_rel(record["path"]).parts)
            tar.add(src, arcname=record["path"], recursive=False)
    if temporary.stat().st_size >= 90 * 1024 * 1024:
        size = temporary.stat().st_size
        temporary.unlink()
        raise ValueError(f"archive is at least 90 MiB; split before integration: {exp_dir} ({size} bytes)")
    temporary.replace(archive)
    manifest = {
        "schema_version": 1,
        "experiment": root,
        "archive": ARCHIVE_NAME,
        "archive_size": archive.stat().st_size,
        "archive_sha256": digest(archive),
        "members": records,
    }
    with manifest_path.open("x", encoding="utf-8") as stream:
        json.dump(manifest, stream, indent=2, sort_keys=True)
        stream.write("\n")
    persisted = json.loads(manifest_path.read_text(encoding="utf-8"))
    if persisted != manifest:
        raise ValueError(f"written manifest differs from in-memory manifest: {manifest_path}")
    verify_tar(archive, persisted, exp_dir)
    return {
        "experiment": root,
        "archive": str(archive.relative_to(REPO)),
        "manifest": str(manifest_path.relative_to(REPO)),
        "archive_size": archive.stat().st_size,
        "archive_sha256": manifest["archive_sha256"],
        "member_count": len(records),
        "members": records,
    }


def build() -> dict:
    if SUMMARY.exists() or SUMMARY.is_symlink():
        raise FileExistsError(f"refusing to overwrite integration summary: {SUMMARY}")
    full_snapshot = snapshot()
    # This directory contains merger-owned tooling and validation evidence;
    # preserve it as-is without feeding any of it into the campaign archives.
    paths = [
        value for value in full_snapshot
        if not (len(safe_rel(value).parts) >= 3 and safe_rel(value).parts[2] == MERGE_NAME)
    ]
    grouped: dict[str, list[str]] = {}
    retained, archived, excluded, out_of_scope = [], [], [], []
    for value in paths:
        root = exp_root(value)
        if root is None:
            parts = safe_rel(value).parts
            # Root-level dated reports are retained; merge directory is excluded above.
            if len(parts) == 3 and parts[:2] == ("experiments", "rare_positions") and Path(value).suffix.lower() in KEEP_EXT:
                retained.append(value)
            else:
                out_of_scope.append(value)
            continue
        grouped.setdefault(root, []).append(value)
    for root, root_paths in grouped.items():
        pairs = top_level_pairs(root, root_paths)
        for value in root_paths:
            if cache_path(value):
                excluded.append(value)
            elif readable(value, root, pairs):
                retained.append(value)
            else:
                archived.append(value)
    roots = sorted({exp_root(value) for value in archived} - {None})
    # Preflight outputs before writing any archive.
    for root in roots:
        for out in (RARE / root / ARCHIVE_NAME, RARE / root / MANIFEST_NAME,
                    RARE / root / ".merge-evidence.tar.gz.building"):
            if out.exists() or out.is_symlink():
                raise FileExistsError(f"refusing to overwrite existing output: {out}")
    archive_records = []
    for root in roots:
        members = [value for value in archived if exp_root(value) == root]
        record = create_archive(root, members)
        if record:
            archive_records.append(record)
    retained, archived, excluded, out_of_scope = map(
        lambda xs: sorted(set(xs)), (retained, archived, excluded, out_of_scope)
    )
    total = sum(item["size"] for archive in archive_records for item in archive["members"])
    summary = {
        "schema_version": 1,
        "purpose": "verified compact packaging of untracked Oct 2–5 rare-position experiment evidence",
        "snapshot_command": "git ls-files --others --exclude-standard -- experiments/rare_positions",
        "snapshot_file_count": len(full_snapshot),
        "campaign_snapshot_file_count": len(paths),
        "experiment_directories": sorted(grouped),
        "retained_count": len(retained),
        "retained_paths": retained,
        "archived_count": len(archived),
        "archived_paths": archived,
        "excluded_cache_count": len(excluded),
        "excluded_cache_paths": excluded,
        "out_of_scope_count": len(out_of_scope),
        "out_of_scope_paths": out_of_scope,
        "archive_count": len(archive_records),
        "archived_original_bytes": total,
        "archives": archive_records,
        "verified": "archive hash, every member hash/size/mode, safe member path and original content checked after write",
        "original_files_removed": False,
    }
    with SUMMARY.open("x", encoding="utf-8") as stream:
        json.dump(summary, stream, indent=2, sort_keys=True)
        stream.write("\n")
    return summary


def prune() -> dict:
    if not SUMMARY.is_file() or SUMMARY.is_symlink():
        raise FileNotFoundError(f"prune requires a prior integration summary: {SUMMARY}")
    summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
    temp = SUMMARY.with_suffix(".json.prune.tmp")
    merge_dir = SUMMARY.parent
    verification_log = merge_dir / "prune-verification.json"
    prune_log = merge_dir / "prune-log.json"
    for output in (temp, verification_log, prune_log):
        if output.exists() or output.is_symlink():
            raise FileExistsError(f"refusing prune because output already exists: {output}")
    checked: list[tuple[Path, dict]] = []
    if summary.get("original_files_removed"):
        raise ValueError("summary already records a prune; refusing to repeat")
    # Verify every archive and all originals before unlinking any.
    for record in summary["archives"]:
        root = record["experiment"]
        root_rel = safe_rel(root)
        if len(root_rel.parts) != 1 or DAY_DIR.fullmatch(root) is None or root == MERGE_NAME:
            raise ValueError(f"unsafe experiment name in summary: {root}")
        exp_dir = RARE / root
        manifest_path = exp_dir / MANIFEST_NAME
        manifest_path = source_path(str(manifest_path.relative_to(REPO)))
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        archive_rel = safe_rel(manifest["archive"])
        if len(archive_rel.parts) != 1 or archive_rel.name != ARCHIVE_NAME:
            raise ValueError(f"unsafe archive name in manifest: {manifest.get('archive')!r}")
        archive = source_path(str((exp_dir / archive_rel.name).relative_to(REPO)))
        verify_tar(archive, manifest, exp_dir, check_sources=True)
        if manifest["archive_sha256"] != record["archive_sha256"]:
            raise ValueError(f"archive differs from summary: {archive}")
        manifest_members = manifest.get("members", [])
        if manifest_members != record.get("members", []):
            raise ValueError(f"manifest member list differs from integration summary: {manifest_path}")
        prefix = f"experiments/rare_positions/{root}/"
        expected_global = {prefix + item["path"] for item in manifest_members}
        summary_global = {
            value for value in summary.get("archived_paths", [])
            if value.startswith(prefix)
        }
        if expected_global != summary_global:
            raise ValueError(f"manifest paths differ from summary archived path set: {manifest_path}")
        for item in manifest["members"]:
            rel = safe_rel(item["path"])
            original = exp_dir.joinpath(*rel.parts)
            checked.append((source_path(str(original.relative_to(REPO))), item))
    verification = {
        "status": "all archives, manifests, member sets and original hashes verified before prune",
        "archive_count": len(summary["archives"]),
        "member_count": len(checked),
        "archives": [{
            "experiment": record["experiment"],
            "archive_sha256": record["archive_sha256"],
            "member_count": len(record["members"]),
        } for record in summary["archives"]],
    }
    with verification_log.open("x", encoding="utf-8") as stream:
        json.dump(verification, stream, indent=2, sort_keys=True)
        stream.write("\n")
    with prune_log.open("x", encoding="utf-8") as stream:
        json.dump({"status": "prune_started", "expected_remove_count": len(checked)}, stream, indent=2)
        stream.write("\n")
    staged = dict(summary)
    staged["prune"] = {"status": "prune_started", "expected_remove_count": len(checked)}
    staged["original_files_removed"] = False
    with temp.open("x", encoding="utf-8") as stream:
        json.dump(staged, stream, indent=2, sort_keys=True)
        stream.write("\n")
    removed = []
    for original, item in checked:
        original = source_path(str(original.relative_to(REPO)))
        info = original.lstat()
        if info.st_size != item["size"] or stat.S_IMODE(info.st_mode) != item["mode"] or digest(original) != item["sha256"]:
            raise ValueError(f"original differs from verified archive: {original}")
        original.unlink()
        removed.append(str(original.relative_to(REPO)))
    summary["prune"] = {"removed_count": len(removed), "removed_paths": removed}
    summary["original_files_removed"] = True
    actual = subprocess.run(
        ["git", "ls-files", "--others", "--exclude-standard", "--", "experiments/rare_positions"],
        cwd=REPO, check=True, capture_output=True, text=True,
    )
    remaining = sorted(line for line in actual.stdout.splitlines() if line)
    expected = set(summary.get("retained_paths", [])) | set(summary.get("out_of_scope_paths", []))
    for record in summary["archives"]:
        expected.add(record["archive"])
        expected.add(record["manifest"])
    merge_prefix = f"experiments/rare_positions/{MERGE_NAME}/"
    unexpected = [path for path in remaining if path not in expected and not path.startswith(merge_prefix)]
    missing_retained = [
        path for path in summary.get("retained_paths", [])
        if not (REPO / path).is_file()
    ]
    summary["remaining_untracked_inventory"] = {
        "count": len(remaining),
        "unexpected_count": len(unexpected),
        "unexpected_paths": unexpected,
        "missing_retained_count": len(missing_retained),
        "missing_retained_paths": missing_retained,
    }
    with prune_log.open("w", encoding="utf-8") as stream:
        json.dump({
            "status": "prune_complete",
            "removed_count": len(removed),
            "verified_archive_count": len(summary["archives"]),
            "verified_member_count": len(checked),
            "remaining_untracked_count": len(remaining),
            "unexpected_untracked_count": len(unexpected),
            "unexpected_untracked_paths": unexpected,
            "missing_retained_count": len(missing_retained),
        }, stream, indent=2, sort_keys=True)
        stream.write("\n")
    with temp.open("w", encoding="utf-8") as stream:
        json.dump(summary, stream, indent=2, sort_keys=True)
        stream.write("\n")
    temp.replace(SUMMARY)
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prune", action="store_true", help="explicitly remove only hash-matching archived originals")
    args = parser.parse_args()
    report = prune() if args.prune else build()
    print(json.dumps({
        "summary": str(SUMMARY),
        "snapshot_file_count": report.get("snapshot_file_count"),
        "retained_count": report.get("retained_count"),
        "archived_count": report.get("archived_count"),
        "archive_count": report.get("archive_count"),
        "archived_original_bytes": report.get("archived_original_bytes"),
        "removed_count": report.get("prune", {}).get("removed_count", 0),
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"package_evidence: {exc}", file=sys.stderr)
        raise
