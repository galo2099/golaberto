#!/usr/bin/env python3
"""Run a bounded residual certificate audit for one prepared frontier region."""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import time
from pathlib import Path

from driver import HERE, Protocol, sha

DEFAULT_BINARY = HERE.parents[2] / "odds-rust/frontier-recovery/target/release/frontier-recovery"
DEFAULT_REQUEST = HERE.parent / "reference/2026-10-04-two-results/inputs/group-16498-two-results.json"
DEFAULT_REGION_ID = 5
HISTORICAL_REQUEST_SHA256 = "cc1e7144a7e50aa3abc54a4c9b64a641bde7f5db0328143bef7e7bf8009071ee"


def source_manifest(binary: Path) -> tuple[str, dict[str, str]]:
    crate = HERE.parents[2] / "odds-rust/frontier-recovery"
    paths = sorted(HERE.glob("*.py")) + [binary, crate / "Cargo.toml"]
    paths += sorted(crate.rglob("*.rs"))
    if (crate / "Cargo.lock").exists():
        paths.append(crate / "Cargo.lock")
    files = {str(path): sha(path) for path in sorted(paths) if path.exists()}
    digest = hashlib.sha256()
    for filename, file_hash in sorted(files.items()):
        digest.update(filename.encode())
        digest.update(bytes.fromhex(file_hash))
    return digest.hexdigest(), files


def played_game_counts(request: dict) -> tuple[int, int]:
    games = request.get("games", [])
    played = sum(bool(game.get("played")) for game in games)
    return played, len(games) - played


def canonical_request_sha256(request: dict) -> str:
    canonical = json.dumps(request, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode()).hexdigest()


def suffix_options(args) -> dict:
    """Validate and return the optional suffix-bound request configuration."""
    options = {
        "enabled": bool(getattr(args, "suffix_bounds", False)),
        "state_cap": getattr(args, "suffix_state_cap", 1_000_000),
        "max_remaining": getattr(args, "suffix_max_remaining", 24),
        "probe_nodes": getattr(args, "suffix_probe_nodes", 25_000),
        "probe_ms": getattr(args, "suffix_probe_ms", 25),
        "budget_fraction": getattr(args, "suffix_budget_fraction", .15),
    }
    for name in ("state_cap", "max_remaining", "probe_nodes", "probe_ms"):
        value = options[name]
        if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
            raise ValueError(f"suffix {name.replace('_', '-')} must be a positive integer")
    fraction = options["budget_fraction"]
    if not isinstance(fraction, (int, float)) or isinstance(fraction, bool) or not math.isfinite(fraction) or not 0 < fraction <= 1:
        raise ValueError("suffix budget-fraction must be finite and in (0, 1]")
    options["budget_fraction"] = float(fraction)
    return options


def run_audit(args, protocol_factory=Protocol, now=time.monotonic) -> dict:
    """Run setup and one bounded certificate call under a shared wall deadline."""
    started = now()
    deadline = started + args.budget
    audit_deadline = deadline - args.cleanup_reserve
    request_path = Path(args.request).resolve()
    binary_path = Path(args.binary).resolve()
    request_data = json.loads(request_path.read_text())
    request_hash = sha(request_path)
    source_hash, source_files = source_manifest(binary_path)
    binary_hash = sha(binary_path)
    played, remaining = played_game_counts(request_data)
    suffix = suffix_options(args)
    output = {
        "provenance": "bounded residual certificate audit; no sampling or confidence claim",
        "target": {"team_id": args.team_id, "position": args.position},
        "region_id": args.region_id,
        "request_sha256": request_hash,
        "canonical_request_sha256": canonical_request_sha256(request_data),
        "source_sha256": source_hash,
        "source_file_sha256": source_files,
        "binary_sha256": binary_hash,
        "canonical": None,
        "game_counts": {"played": played, "remaining": remaining},
        "known_analytic_upper": None,
        "prepared_plan": None,
        "regime_plan": None,
        "certificate": None,
        "status": "not_started",
        "errors": [],
        "timing_ms": {"wall_elapsed": 0.0, "prepare": None, "regime": None, "certificate": None},
        "overrun_ms": 0.0,
        "budget_seconds": args.budget,
        "cleanup_reserve_seconds": args.cleanup_reserve,
        "node_limit": args.node_limit,
        "clip_fraction": args.clip_fraction,
        "suffix_bounds_enabled": suffix["enabled"],
        "suffix_settings": {key: value for key, value in suffix.items() if key != "enabled"},
        "suffix_request_sent": False,
    }
    engine = None
    try:
        if request_hash != HISTORICAL_REQUEST_SHA256 or (played, remaining) != (279, 101):
            raise ValueError(
                "request must match the historical 279-played/101-remaining input "
                f"({HISTORICAL_REQUEST_SHA256})"
            )
        if now() >= audit_deadline:
            output["status"] = "setup_budget_exhausted"
            return output
        # Native request deadlines stop before the reserve, while the protocol
        # remains readable until the full wall deadline so final aggregation and
        # receipt serialization can finish during the reserved interval.
        engine = protocol_factory(binary_path, request_path, args.team_id, args.position, deadline)
        desc = engine.call(op="describe")
        output["canonical"] = desc.get("canonical")
        target = desc.get("target") or {}
        if int(target.get("team_id", -1)) != args.team_id:
            raise ValueError(f"describe target team_id {target.get('team_id')!r} does not match {args.team_id}")
        if int(target.get("position", -1)) != args.position:
            raise ValueError(f"describe target position {target.get('position')!r} does not match {args.position}")
        regions = {int(region["id"]): region for region in desc.get("regions", [])}
        if args.region_id not in regions:
            raise ValueError(f"describe has no region id {args.region_id}")
        region = regions[args.region_id]
        output["root_id"] = region.get("root_id")
        if int(region.get("root_id", -1)) != args.root_id:
            raise ValueError(f"region root_id {region.get('root_id')!r} does not match {args.root_id}")
        if region.get("impossible"):
            output["status"] = "region_impossible"
            output["known_analytic_upper"] = 0.0
            return output
        output["known_analytic_upper"] = float(region.get("upper", 1.0))
        plan = region.get("plan") or {}
        output["describe_plan"] = plan
        if int(plan.get("states", 0)) > args.state_cap:
            raise ValueError(f"describe plan exceeds state cap: {plan.get('states')} > {args.state_cap}")
        if int(plan.get("k", args.k)) != args.k:
            output["describe_plan_k"] = plan.get("k")
        setup_ms = min(1000, max(0, int((audit_deadline - now()) * 1000)))
        if setup_ms <= 0:
            output["status"] = "setup_budget_exhausted"
            return output
        call_started = now()
        prepared = engine.call(op="prepare", id=args.region_id, k=args.k,
                               state_cap=args.state_cap, time_ms=setup_ms)
        output["timing_ms"]["prepare"] = (now() - call_started) * 1000.0
        output["prepared_plan"] = prepared
        if not prepared.get("accepted"):
            output["status"] = "prepare_rejected"
            return output
        if int(prepared.get("states", 0)) > args.state_cap:
            raise ValueError(f"prepare receipt exceeds state cap: {prepared.get('states')} > {args.state_cap}")
        setup_ms = min(1000, max(0, int((audit_deadline - now()) * 1000)))
        if setup_ms <= 0:
            output["status"] = "setup_budget_exhausted"
            return output
        call_started = now()
        regime = engine.call(op="regime", id=args.region_id,
                             state_cap=args.state_cap, time_ms=setup_ms)
        output["timing_ms"]["regime"] = (now() - call_started) * 1000.0
        output["regime_plan"] = regime
        if not regime.get("accepted"):
            output["status"] = "regime_rejected"
            return output
        if int(regime.get("states", 0)) > args.state_cap:
            raise ValueError(f"regime receipt exceeds state cap: {regime.get('states')} > {args.state_cap}")
        remaining_ms = max(0, int((audit_deadline - now()) * 1000))
        search_ms = min(remaining_ms, args.search_ms) if args.search_ms is not None else remaining_ms
        if search_ms <= 0:
            output["status"] = "search_budget_exhausted"
            return output
        call_started = now()
        certificate_request = {"op": "residual_certificate", "id": args.region_id,
                               "clip_fraction": args.clip_fraction,
                               "node_limit": args.node_limit, "time_ms": search_ms}
        if suffix["enabled"]:
            certificate_request["suffix"] = {
                "state_cap": suffix["state_cap"],
                "max_remaining": suffix["max_remaining"],
                "probe_nodes": suffix["probe_nodes"],
                "probe_ms": suffix["probe_ms"],
                "budget_fraction": suffix["budget_fraction"],
            }
            output["suffix_request_sent"] = True
        certificate = engine.call(**certificate_request)
        output["timing_ms"]["certificate"] = (now() - call_started) * 1000.0
        output["certificate"] = certificate
        output["status"] = "complete" if certificate.get("complete") is True else "incomplete"
        return output
    except Exception as exc:
        output["status"] = "error"
        output["errors"].append(f"{type(exc).__name__}: {exc}")
        return output
    finally:
        if engine is not None:
            try:
                engine.close()
            except Exception as exc:
                output["errors"].append(f"close: {type(exc).__name__}: {exc}")
                if output["status"] == "complete":
                    output["status"] = "complete_with_close_error"
        elapsed = (now() - started) * 1000.0
        output["timing_ms"]["wall_elapsed"] = elapsed
        output["overrun_ms"] = max(0.0, elapsed - args.budget * 1000.0)
        output["hashes_unchanged"] = (
            sha(request_path) == request_hash and sha(binary_path) == binary_hash
            and all(sha(Path(path)) == digest for path, digest in source_files.items())
        )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default=str(DEFAULT_BINARY))
    parser.add_argument("--request", default=str(DEFAULT_REQUEST))
    parser.add_argument("--team-id", type=int, default=16)
    parser.add_argument("--position", type=int, default=16)
    parser.add_argument("--region-id", type=int, default=DEFAULT_REGION_ID)
    parser.add_argument("--root-id", type=int, default=0)
    parser.add_argument("--k", type=int, default=9)
    parser.add_argument("--state-cap", type=int, default=250000)
    parser.add_argument("--budget", type=float, default=180.0)
    parser.add_argument("--cleanup-reserve", type=float, default=1.0)
    parser.add_argument("--node-limit", type=int, default=250000)
    parser.add_argument("--clip-fraction", type=float, default=1 / 1024)
    parser.add_argument("--search-ms", type=int, help="optional certificate search cap for smoke runs")
    parser.add_argument("--suffix-bounds", action="store_true", help="enable optional cached suffix bounds")
    parser.add_argument("--suffix-state-cap", type=int, default=1_000_000)
    parser.add_argument("--suffix-max-remaining", type=int, default=24)
    parser.add_argument("--suffix-probe-nodes", type=int, default=25_000)
    parser.add_argument("--suffix-probe-ms", type=int, default=25)
    parser.add_argument("--suffix-budget-fraction", type=float, default=.15)
    parser.add_argument("--output")
    args = parser.parse_args()
    if not math.isfinite(args.budget) or args.budget <= 0:
        parser.error("--budget must be finite and positive")
    if not math.isfinite(args.cleanup_reserve) or not 0 <= args.cleanup_reserve < args.budget:
        parser.error("--cleanup-reserve must be nonnegative and less than --budget")
    if args.k < 1 or args.state_cap < 1 or args.node_limit < 1:
        parser.error("--k, --state-cap, and --node-limit must be positive")
    if args.search_ms is not None and args.search_ms < 1:
        parser.error("--search-ms must be positive")
    if not math.isfinite(args.clip_fraction) or not 0 < args.clip_fraction <= 1:
        parser.error("--clip-fraction must be finite and in (0, 1]")
    try:
        suffix_options(args)
    except ValueError as exc:
        parser.error(str(exc))
    result = run_audit(args)
    rendered = json.dumps(result, indent=2, sort_keys=True)
    if args.output:
        Path(args.output).write_text(rendered + "\n")
    print(rendered)


if __name__ == "__main__":
    main()
