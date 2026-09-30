#!/usr/bin/env python3
"""Score a saved estimator matrix against the usable part of the golden set."""

import argparse
import hashlib
import json
import math
import statistics
from pathlib import Path

from build_golden_reference import canonical_bytes, normalized_request


def compare(case, matrix):
    observed, missing, impossible_errors, mc_squared, log_errors = [], [], [], [], []
    for cell in case["cells"]:
        team, rank = str(cell["team"]), str(cell["position"] - 1)
        p = matrix[team][rank]["probability"]
        if not math.isfinite(p) or not 0 <= p <= 1:
            raise ValueError(f"invalid probability at {team}/{rank}")
        if not cell["scorable"]:
            continue
        key = {"team": cell["team"], "position": cell["position"]}
        if cell["status"] == "impossible":
            if p > 0:
                impossible_errors.append({**key, "estimated_probability": p})
            continue
        observed.append(cell)
        if p == 0:
            missing.append({**key, "reference_probability": cell["probability"], "status": cell["status"]})
        if cell["status"] == "reference_mc":
            mc_squared.append((p - cell["probability"]) ** 2)
        elif p > 0:
            log_errors.append(abs(math.log10(p) - math.log10(cell["probability"])))
    return {
        "case": case["case"], "reference_positive_cells": len(observed),
        "covered": len(observed) - len(missing), "missing": missing,
        "impossible_positive_errors": impossible_errors,
        "mc_probability_rmse": math.sqrt(statistics.mean(mc_squared)) if mc_squared else None,
        "is_mean_absolute_log10_error": statistics.mean(log_errors) if log_errors else None,
        "is_within_one_order_of_magnitude": sum(error <= 1 for error in log_errors),
        "is_nonzero_compared": len(log_errors),
        "excluded_cells": sum(not c["scorable"] for c in case["cells"]),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", required=True)
    parser.add_argument("--estimates", required=True)
    parser.add_argument("--matrix-key", default="rare_position_estimates")
    parser.add_argument("--request", required=True, help="Exact saved request, or its normalized portable counterpart")
    parser.add_argument("--output")
    args = parser.parse_args()
    reference = json.loads(Path(args.reference).read_text())
    request = normalized_request(json.loads(Path(args.request).read_text()))
    fingerprint = hashlib.sha256(canonical_bytes(request)).hexdigest()
    cases = [case for case in reference["cases"] if case["semantic_input_sha256"] == fingerprint]
    if len(cases) != 1:
        raise ValueError("request does not uniquely match a reference snapshot")
    export = json.loads(Path(args.estimates).read_text())
    report = compare(cases[0], export[args.matrix_key])
    encoded = json.dumps(report, indent=2, allow_nan=False) + "\n"
    if args.output:
        Path(args.output).write_text(encoded)
    print(encoded, end="")


if __name__ == "__main__":
    main()
