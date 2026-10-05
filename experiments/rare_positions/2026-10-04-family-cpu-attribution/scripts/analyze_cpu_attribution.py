"""Summarize paired CPU attribution runs without summing nested scopes."""
import argparse
import hashlib
import json
import statistics
from collections import defaultdict
from pathlib import Path

ARMS = ("production-baseline", "frozen-r60-flag-off", "diagnostic-r60-flag-off", "diagnostic-r60-family-on", "frozen-r60-family-on")
DIAG_OFF = "diagnostic-r60-flag-off"
DIAG_ON = "diagnostic-r60-family-on"
FAMILY_FLAGS = {
    "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK": "1",
    "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_SCOPE": "all",
    "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_MODE": "family",
    "RUST_ODDS_EXPERIMENT_FAMILY_FALLBACK_TRAINING_CAP": "3000",
    "RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING": "1",
    "RUST_ODDS_EXPERIMENT_FAMILY_NATIVE_TRAINING_FUNDING": "stage",
}


def sha(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def read(path):
    return json.loads(Path(path).read_text())


def validate_binary_mapping(binary_map):
    if not isinstance(binary_map, dict) or set(binary_map) != set(ARMS):
        raise ValueError(f"binary metadata must have exactly one entry for each arm: {ARMS}")
    for arm in ARMS:
        item = binary_map[arm]
        if not isinstance(item, dict) or not item.get("path") or not item.get("sha256"):
            raise ValueError(f"binary metadata missing path/hash for arm {arm}")
    if binary_map["frozen-r60-flag-off"]["path"] != binary_map["frozen-r60-family-on"]["path"]:
        raise ValueError("frozen R60 arms must resolve to the same binary")
    if binary_map[DIAG_OFF]["path"] != binary_map[DIAG_ON]["path"]:
        raise ValueError("diagnostic R60 arms must resolve to the same binary")
    if binary_map[DIAG_OFF]["path"] == binary_map["frozen-r60-flag-off"]["path"]:
        raise ValueError("diagnostic and frozen R60 binaries must remain distinct")
    return {arm: binary_map[arm] for arm in ARMS}


def verify_run(run, request):
    if Path(run["request"]).resolve() != request or run["request_sha256"] != sha(request):
        raise ValueError(f"request provenance mismatch: {run['arm']} seed {run['seed']}")
    if not Path(run["binary"]).is_file() or sha(run["binary"]) != run["binary_sha256"]:
        raise ValueError(f"binary provenance mismatch: {run['arm']} seed {run['seed']}")
    for key in ("export", "stdout", "stderr"):
        path = Path(run[key])
        if not path.is_file() or not run.get(f"{key}_sha256") or sha(path) != run[f"{key}_sha256"]:
            raise ValueError(f"{key} hash mismatch: {run['arm']} seed {run['seed']}")
    if not run.get("events") or not any(e.get("event") == "rust_odds_complete" for e in run["events"]):
        raise ValueError(f"missing completion marker: {run['arm']} seed {run['seed']}")
    parsed = []
    for line in Path(run["stderr"]).read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(event, dict) and isinstance(event.get("event"), str):
            parsed.append(event)
    if parsed != run["events"]:
        raise ValueError(f"recorded event list differs from stderr: {run['arm']} seed {run['seed']}")
    scopes = [e for e in parsed if e.get("event") == "rust_odds_cpu_scope"]
    if scopes != run.get("cpu_scope_events"):
        raise ValueError(f"recorded CPU scopes differ from stderr: {run['arm']} seed {run['seed']}")


def scope_key(event):
    team, rank = event.get("team_id"), event.get("rank")
    return (str(team), int(rank)) if team is not None and rank is not None else None


def scope_rollup(events):
    values = defaultdict(lambda: {"cpu_ms": 0.0, "user_ms": 0.0, "system_ms": 0.0, "wall_ms": 0.0, "count": 0, "per_cell": {}})
    for event in events:
        if event.get("event") != "rust_odds_cpu_scope":
            continue
        label = event.get("label")
        if not isinstance(label, str):
            raise ValueError("CPU scope event lacks label")
        row = values[label]
        for field in ("cpu_ms", "user_ms", "system_ms", "wall_ms"):
            amount = event.get(field)
            if not isinstance(amount, (int, float)) or amount < 0:
                raise ValueError(f"invalid {field} in {label} scope")
            row[field] += float(amount)
        row["count"] += 1
        cell = scope_key(event)
        if cell is not None:
            cell_key = f"{cell[0]}:{cell[1]}"
            cell_row = row["per_cell"].setdefault(cell_key, {field: 0.0 for field in ("cpu_ms", "user_ms", "system_ms", "wall_ms")})
            for field in ("cpu_ms", "user_ms", "system_ms", "wall_ms"):
                cell_row[field] += float(event[field])
    return dict(values)


def fallback_outcomes(events):
    attempts = [e.get("fallback", {}) for e in events if e.get("event") == "rust_odds_family_fallback"]
    late = [e for e in events if e.get("event") == "rust_odds_family_fallback_late_summary"]
    return {
        "attempt_records": len(attempts),
        "published_attempts": sum(1 for item in attempts if item.get("published") is True),
        "failed_attempts": sum(1 for item in attempts if item.get("published") is False),
        "failed_attempt_actual_work": sum(int(item.get("added_actual_work", 0)) for item in attempts if item.get("published") is False),
        "all_attempt_actual_work": sum(int(item.get("added_actual_work", 0)) for item in attempts),
        "late_summary": late[0] if len(late) == 1 else None,
        "late_summary_count": len(late),
    }


def mean(values):
    return statistics.mean(values) if values else None


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--runs", required=True, type=lambda s: Path(s).resolve(strict=True))
    p.add_argument("--out", required=True, type=lambda s: Path(s).resolve())
    if __name__ == "__main__":
        args = p.parse_args()
    else:
        return
    if args.out.exists():
        p.error(f"refusing existing output file: {args.out}")
    summary = read(args.runs / "summary.json")
    request = Path(summary["request"]).resolve(strict=True)
    if summary.get("workers") != 4 or summary.get("serialized_processes") is not True or summary.get("warmups_excluded_from_timing") is not True:
        raise ValueError("run summary does not establish serialized four-worker timing protocol")
    records = summary.get("runs", [])
    screen_summary_path = Path(summary["reference_screen_summary"]).resolve(strict=True)
    screen = read(screen_summary_path)
    if Path(screen["request"]).resolve() != request or screen.get("request_sha256") != sha(request):
        raise ValueError("reference screen request provenance mismatch")
    binary_map = validate_binary_mapping(summary.get("binaries"))
    seen = set()
    for record in records:
        identity = (record["arm"], record["seed"], record["repeat"])
        if identity in seen:
            raise ValueError(f"duplicate timing record {identity}")
        seen.add(identity)
        verify_run(record, request)
        expected_env = {"RUST_ODDS_LOG": "1"}
        if record["arm"].startswith("diagnostic-"):
            expected_env["RUST_ODDS_DIAGNOSTIC_CPU"] = "1"
        if record["arm"].endswith("family-on"):
            expected_env.update(FAMILY_FLAGS)
        if record.get("explicit_environment") != expected_env:
            raise ValueError(f"explicit experiment flags mismatch for {identity}")
        binary_meta = binary_map[record["arm"]]
        if Path(record["binary"]).resolve() != Path(binary_meta.get("path", "")).resolve() or record["binary_sha256"] != binary_meta.get("sha256"):
            raise ValueError(f"run binary differs from summary for {identity}")
        family_state = "family_on" if record["arm"].endswith("family-on") else "family_off"
        expected_hash = summary.get("expected_screen_export_sha256", {}).get(str(record["seed"]), {}).get(family_state)
        if not expected_hash or record.get("expected_export_sha256") != expected_hash or record.get("export_sha256") != expected_hash:
            raise ValueError(f"export does not match frozen screen for {identity}")
    seeds, repeats = summary["seeds"], summary["repeats"]
    expected = {(arm, seed, repeat) for arm in ARMS for seed in seeds for repeat in range(repeats)}
    if seen != expected:
        raise ValueError(f"timing record grid incomplete; missing={sorted(expected-seen)} extra={sorted(seen-expected)}")
    rows = {(r["arm"], r["seed"], r["repeat"]): r for r in records}
    per_run = []
    for key in sorted(rows, key=lambda x: (x[1], x[2], ARMS.index(x[0]))):
        record = rows[key]
        per_run.append({
            "arm": record["arm"], "seed": record["seed"], "repeat": record["repeat"],
            "wall_seconds": record["wall_seconds"], "child_user_seconds": record["child_user_seconds"],
            "child_system_seconds": record["child_system_seconds"], "child_cpu_seconds": record["child_cpu_seconds"],
            "cpu_scopes_ms": scope_rollup(record.get("cpu_scope_events", record["events"])),
            "fallback_outcomes": fallback_outcomes(record["events"]),
        })
    by_arm = {}
    for arm in ARMS:
        arm_runs = [r for r in per_run if r["arm"] == arm]
        by_arm[arm] = {field: mean([r[field] for r in arm_runs]) for field in ("wall_seconds", "child_user_seconds", "child_system_seconds", "child_cpu_seconds")}
        by_arm[arm]["sample_count"] = len(arm_runs)
    by_seed_arm = {}
    for seed in seeds:
        by_seed_arm[str(seed)] = {}
        for arm in ARMS:
            arm_runs = [r for r in per_run if r["arm"] == arm and r["seed"] == seed]
            by_seed_arm[str(seed)][arm] = {field: mean([r[field] for r in arm_runs]) for field in ("wall_seconds", "child_user_seconds", "child_system_seconds", "child_cpu_seconds")}
            by_seed_arm[str(seed)][arm]["sample_count"] = len(arm_runs)
            if arm.startswith("diagnostic-"):
                arm_raw = [rows[(arm, seed, repeat)] for repeat in range(repeats)]
                labels = sorted({event.get("label") for row in arm_raw for event in row.get("cpu_scope_events", [])})
                by_seed_arm[str(seed)][arm]["scope_cpu_ms_mean"] = {
                    label: mean([scope_rollup(row["cpu_scope_events"]).get(label, {}).get("cpu_ms", 0.0) for row in arm_raw])
                    for label in labels
                }
                by_seed_arm[str(seed)][arm]["fallback_failed_attempts_mean"] = mean([fallback_outcomes(row["events"])["failed_attempts"] for row in arm_raw])
    paired = []
    scope_labels = sorted({label for r in per_run if r["arm"] in (DIAG_OFF, DIAG_ON) for label in r["cpu_scopes_ms"]})
    for seed in seeds:
        for repeat in range(repeats):
            off = rows[(DIAG_OFF, seed, repeat)]
            on = rows[(DIAG_ON, seed, repeat)]
            paired.append({
                "seed": seed, "repeat": repeat,
                "diagnostic_family_on_minus_off_wall_ms": 1000 * (on["wall_seconds"] - off["wall_seconds"]),
                "diagnostic_family_on_minus_off_cpu_ms": 1000 * (on["child_cpu_seconds"] - off["child_cpu_seconds"]),
                "scope_cpu_delta_ms": {label: scope_rollup(on["cpu_scope_events"]).get(label, {}).get("cpu_ms", 0.0) - scope_rollup(off["cpu_scope_events"]).get(label, {}).get("cpu_ms", 0.0) for label in scope_labels},
            })
    contrasts = []
    for seed in seeds:
        for repeat in range(repeats):
            paired_rows = {arm: rows[(arm, seed, repeat)] for arm in ARMS}
            cpu = {arm: paired_rows[arm]["child_cpu_seconds"] * 1000 for arm in ARMS}
            wall = {arm: paired_rows[arm]["wall_seconds"] * 1000 for arm in ARMS}
            contrasts.append({
                "seed": seed, "repeat": repeat,
                "frozen_r60_vs_production_baseline_cpu_ms": cpu["frozen-r60-flag-off"] - cpu["production-baseline"],
                "diagnostic_build_vs_frozen_r60_cpu_ms": cpu[DIAG_OFF] - cpu["frozen-r60-flag-off"],
                "family_flags_effect_frozen_r60_cpu_ms": cpu["frozen-r60-family-on"] - cpu["frozen-r60-flag-off"],
                "family_flags_effect_diagnostic_r60_cpu_ms": cpu[DIAG_ON] - cpu[DIAG_OFF],
                "diagnostic_family_effect_minus_frozen_family_effect_cpu_ms": (cpu[DIAG_ON] - cpu[DIAG_OFF]) - (cpu["frozen-r60-family-on"] - cpu["frozen-r60-flag-off"]),
                "frozen_r60_vs_production_baseline_wall_ms": wall["frozen-r60-flag-off"] - wall["production-baseline"],
                "diagnostic_build_vs_frozen_r60_wall_ms": wall[DIAG_OFF] - wall["frozen-r60-flag-off"],
                "family_flags_effect_frozen_r60_wall_ms": wall["frozen-r60-family-on"] - wall["frozen-r60-flag-off"],
                "family_flags_effect_diagnostic_r60_wall_ms": wall[DIAG_ON] - wall[DIAG_OFF],
                "frozen_r60_vs_production_baseline_cpu_percent": 100 * (cpu["frozen-r60-flag-off"] - cpu["production-baseline"]) / cpu["production-baseline"] if cpu["production-baseline"] else None,
                "diagnostic_build_vs_frozen_r60_cpu_percent": 100 * (cpu[DIAG_OFF] - cpu["frozen-r60-flag-off"]) / cpu["frozen-r60-flag-off"] if cpu["frozen-r60-flag-off"] else None,
                "family_flags_effect_frozen_r60_cpu_percent": 100 * (cpu["frozen-r60-family-on"] - cpu["frozen-r60-flag-off"]) / cpu["frozen-r60-flag-off"] if cpu["frozen-r60-flag-off"] else None,
                "family_flags_effect_diagnostic_r60_cpu_percent": 100 * (cpu[DIAG_ON] - cpu[DIAG_OFF]) / cpu[DIAG_OFF] if cpu[DIAG_OFF] else None,
            })
    scope_means = {}
    for label in scope_labels:
        off_values, on_values = [], []
        for seed in seeds:
            for repeat in range(repeats):
                off_scope = scope_rollup(rows[(DIAG_OFF, seed, repeat)]["cpu_scope_events"]).get(label)
                on_scope = scope_rollup(rows[(DIAG_ON, seed, repeat)]["cpu_scope_events"]).get(label)
                off_values.append(off_scope["cpu_ms"] if off_scope else 0.0)
                on_values.append(on_scope["cpu_ms"] if on_scope else 0.0)
        scope_means[label] = {"diagnostic_off_mean_cpu_ms": mean(off_values), "diagnostic_on_mean_cpu_ms": mean(on_values), "paired_mean_delta_cpu_ms": mean([b - a for a, b in zip(off_values, on_values)])}
    completed = {
        "cpu_scope_events_per_run": {r["arm"] + f":seed{r['seed']}:r{r['repeat']}": len(rows[(r["arm"], r["seed"], r["repeat"])].get("cpu_scope_events", [])) for r in per_run if r["arm"].startswith("diagnostic-")},
        "fallback_failures": {r["arm"] + f":seed{r['seed']}:r{r['repeat']}": r["fallback_outcomes"]["failed_attempts"] for r in per_run if r["arm"].endswith("family-on")},
    }
    result = {
        "protocol": {"seeds": seeds, "repeats": repeats, "workers": 4, "serialized": True, "warmups_excluded": True},
        "arms_mean_process_resources": by_arm,
        "per_seed_arm_mean_resources_and_scope_cpu": by_seed_arm,
        "paired_total_cpu_and_wall_contrasts_ms": {field: {"mean": mean([r[field] for r in contrasts]), "pairs": [{"seed": r["seed"], "repeat": r["repeat"], "value": r[field]} for r in contrasts]} for field in contrasts[0] if field not in ("seed", "repeat")},
        "paired_diagnostic_family_effect": {"meaning": "diagnostic-R60 family-on minus diagnostic-R60 flag-off, paired by seed and repeat", "mean_cpu_ms": mean([r["diagnostic_family_on_minus_off_cpu_ms"] for r in paired]), "mean_wall_ms": mean([r["diagnostic_family_on_minus_off_wall_ms"] for r in paired]), "pairs": paired},
        "diagnostic_scopes_paired_mean_cpu_ms": scope_means,
        "scope_hierarchy_warning": "Scope labels can nest; compare each label separately and never sum parent and child scope CPU values.",
        "fallback_failure_and_work_counts": completed,
        "runs": per_run,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
