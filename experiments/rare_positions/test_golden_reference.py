import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from build_golden_reference import canonical_bytes, make_case, normalized_request, probability_reference
from compare_golden_reference import compare


class GoldenReferenceTest(unittest.TestCase):
    def test_builder_preserves_source_phase_without_changing_estimator_input(self):
        raw = {"id": 16982, "phase": {"id": 4529, "sort": "pt,gd,gf",
               "championship": {"point_win": 3, "point_draw": 1, "point_loss": 0}},
               "team_groups": [{"team_id": 1}], "games": []}
        raw["dataset_transform"] = {"method": "unplay_latest_played_games", "removed_game_ids": []}
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            (output / "inputs").mkdir()
            source = output / "source.json"
            source.write_bytes(canonical_bytes(raw))
            run = {"input": str(source), "input_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                   "group": 16982, "seed": 801, "cells": [],
                   "baseline": {"1": {"0": {"probability": 1., "hits": 100}}},
                   **{key: 0 for key in ("baseline_ms", "extra_ms", "baseline_work", "extra_work", "work_limit")}}
            proof = {"cells": [{"team": 1, "position": 1, "impossible": False}],
                     "elapsed_ms": 0, "nodes_per_case": 200000}
            mc = {"cells": [{"team": 1, "position": 1, "count": 5000000}],
                  "samples": 5000000, "seed": 947117}
            case = make_case([run], proof, mc, output, "test")
            self.assertEqual(case["phase"], 4529)
            self.assertEqual(case["dataset_transform"], raw["dataset_transform"])
            self.assertEqual(json.loads((output / case["request"]).read_text()), normalized_request(raw))
            self.assertNotIn("id", normalized_request(raw)["phase"])

    def test_phase_4529_is_fingerprinted_and_scored_in_the_golden_set(self):
        root = Path(__file__).parent / "reference/2026-09-30-hundredfold"
        reference = json.loads((root / "reference.json").read_text())
        cases = [case for case in reference["cases"] if case.get("phase") == 4529]
        self.assertEqual(len(cases), 1)
        case = cases[0]
        self.assertEqual(case["group"], 16982)
        request = json.loads((root / case["request"]).read_text())
        self.assertEqual(hashlib.sha256(canonical_bytes(normalized_request(request))).hexdigest(),
                         case["semantic_input_sha256"])
        self.assertEqual(len(request["team_groups"]), 20)
        self.assertEqual((len(request["games"]), sum(game["played"] for game in request["games"])), (380, 50))
        counts = {(team["team_id"], rank): 0 for team in request["team_groups"] for rank in range(1, 21)}
        seen = set()
        matrix = {str(team["team_id"]): {} for team in request["team_groups"]}
        for cell in case["cells"]:
            key = cell["team"], cell["position"]
            self.assertIn(key, counts)
            self.assertNotIn(key, seen, "duplicate team/rank cell")
            seen.add(key)
            counts[key] = cell["plain_mc"]["hits"]
            self.assertEqual(cell["plain_mc"]["samples"], 5000000)
            matrix[str(cell["team"])][str(cell["position"] - 1)] = {"probability": cell["probability"] or 0.}
        self.assertEqual(len(case["cells"]), 400)
        self.assertEqual(seen, set(counts))
        for team in matrix:
            self.assertEqual(sum(counts[int(team), rank] for rank in range(1, 21)), 5000000)
        for rank in range(1, 21):
            self.assertEqual(sum(counts[int(team), rank] for team in matrix), 5000000)
        report = compare(case, matrix)
        self.assertEqual((report["reference_positive_cells"], report["covered"], report["excluded_cells"]), (368, 368, 32))
        self.assertEqual(report["mc_probability_rmse"], 0.)
        self.assertEqual(report["missing"], [])
        self.assertEqual(report["impossible_positive_errors"], [])

    def test_expanded_suite_removes_old_snapshot_and_preserves_reopened_games(self):
        root = Path(__file__).parent / "reference/2026-09-30-hundredfold"
        cases = json.loads((root / "reference.json").read_text())["cases"]
        self.assertEqual(len(cases), 6)
        self.assertEqual([case["case"] for case in cases if case["group"] == 16653],
                         ["group-16653-71d4fea8"])
        active_requests = {case["request"] for case in cases}
        self.assertEqual(active_requests, {str(path.relative_to(root)) for path in (root / "inputs").glob("*.json")})
        expected = {4392: (15902, 20, 380, 180, 348), 4451: (16413, 18, 306, 105, 276)}
        found = set()
        for case in cases:
            if case.get("phase") not in expected:
                continue
            phase = case["phase"]
            self.assertNotIn(phase, found)
            found.add(phase)
            group, teams, games, played, scorable = expected[phase]
            request = json.loads((root / case["request"]).read_text())
            self.assertEqual(case["group"], group)
            self.assertEqual((len(request["team_groups"]), len(request["games"])), (teams, games))
            self.assertEqual(sum(game["played"] for game in request["games"]), played)
            self.assertEqual(hashlib.sha256(canonical_bytes(normalized_request(request))).hexdigest(),
                             case["semantic_input_sha256"])
            transform = case["dataset_transform"]
            self.assertEqual(transform["method"], "unplay_latest_played_games")
            self.assertEqual(transform["removed_played_games"], 200)
            removed = set(transform["removed_game_ids"])
            self.assertEqual(len(removed), 200)
            reopened = [game for game in request["games"] if game["id"] in removed]
            self.assertEqual(len(reopened), 200)
            self.assertTrue(all(not game["played"] and game["home_score"] == game["away_score"] == 0 for game in reopened))
            self.assertEqual(transform["source_played_games"] - played, 200)
            self.assertEqual(transform["remaining_games"], games - played)
            self.assertEqual(len(case["cells"]), teams * teams)
            self.assertTrue(all(cell["plain_mc"]["samples"] == 5000000 for cell in case["cells"]))
            for team in request["team_groups"]:
                self.assertEqual(sum(cell["plain_mc"]["hits"] for cell in case["cells"] if cell["team"] == team["team_id"]), 5000000)
            for rank in range(1, teams + 1):
                self.assertEqual(sum(cell["plain_mc"]["hits"] for cell in case["cells"] if cell["position"] == rank), 5000000)
            self.assertEqual(sum(cell["scorable"] for cell in case["cells"]), scorable)
        self.assertEqual(found, set(expected))

    def test_independent_agreement_and_provisional_values(self):
        self.assertEqual(probability_reference([]), (None, None, None, 0))
        sample = lambda p, accepted=True: {"p": p, "accepted": accepted}
        status, _, _, count = probability_reference([sample(1e-20), sample(2e-20)])
        self.assertEqual((status, count), ("reference_is", 2))
        status, _, _, count = probability_reference([sample(1e-20), sample(1e-17)])
        self.assertEqual((status, count), ("provisional_is", 2))
        status, _, _, count = probability_reference([sample(1e-20), sample(0, False)])
        self.assertEqual((status, count), ("provisional_is", 1))

    def test_compare_excludes_unknowns_and_reports_missing_tails(self):
        def cell(position, status, p, scorable):
            return {"team": 1, "position": position, "status": status,
                    "probability": p, "scorable": scorable}
        case = {"case": "toy", "cells": [
            cell(1, "reference_is", 1e-20, True),
            cell(2, "impossible", 0, True),
            cell(3, "undecided", None, False),
            cell(4, "provisional_is", 1e-25, False),
        ]}
        matrix = {"1": {str(i): {"probability": p} for i, p in enumerate([0, 1e-10, 0, 0])}}
        report = compare(case, matrix)
        self.assertEqual(report["reference_positive_cells"], 1)
        self.assertEqual(report["covered"], 0)
        self.assertEqual(report["missing"][0]["position"], 1)
        self.assertEqual(report["impossible_positive_errors"][0]["position"], 2)
        self.assertEqual(report["excluded_cells"], 2)


if __name__ == "__main__":
    unittest.main()
