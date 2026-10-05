import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parents[4]
base = root / "experiments/rare_positions/2026-10-05-tight-guide-bound/screen-v1/runs"
current = root / "experiments/rare_positions/2026-10-05-experiment-merge/validation/default-controls/runs"


def load(path):
    return json.loads(path.read_text())


def strip_work_spent(value):
    if isinstance(value, dict):
        return {k: strip_work_spent(v) for k, v in value.items() if k != "work_spent"}
    if isinstance(value, list):
        return [strip_work_spent(v) for v in value]
    return value


def sha(value):
    data = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(data).hexdigest()


for fixture in ("original-16498", "updated-16498"):
    old_path = base / fixture / "seed-808/repeat-0/control/export.json"
    new_path = current / fixture / "seed-808/repeat-0/control/export.json"
    old, new = load(old_path), load(new_path)
    old_norm, new_norm = strip_work_spent(old), strip_work_spent(new)
    print(json.dumps({
        "fixture": fixture,
        "seed": 808,
        "baseline_binary": "R71 screen-v1 control",
        "baseline_sha256": hashlib.sha256(old_path.read_bytes()).hexdigest(),
        "current_binary": str(root / "odds-rust/target/release/golaberto-odds"),
        "current_binary_sha256": hashlib.sha256((root / "odds-rust/target/release/golaberto-odds").read_bytes()).hexdigest(),
        "current_sha256": hashlib.sha256(new_path.read_bytes()).hexdigest(),
        "equal_except_work_spent": old_norm == new_norm,
        "baseline_normalized_sha256": sha(old_norm),
        "current_normalized_sha256": sha(new_norm),
    }, sort_keys=True))
