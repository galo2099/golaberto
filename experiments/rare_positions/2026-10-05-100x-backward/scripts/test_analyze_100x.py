import importlib.util
import json
from pathlib import Path
import hashlib
import contextlib
import io
from unittest.mock import patch
import tempfile
import unittest

MODULE_PATH = Path(__file__).with_name("analyze_100x.py")
SPEC = importlib.util.spec_from_file_location("analyze_100x", MODULE_PATH)
analyzer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(analyzer)


class Analyze100xTests(unittest.TestCase):
    def request(self):
        return {"team_groups": [{"team_id": 10}, {"team_id": 16}, {"team_id": 17}],
                "games": [{"id": 1, "home_id": 10, "away_id": 16, "played": False},
                          {"id": 2, "home_id": 16, "away_id": 17, "played": False}]}

    def witness(self, first, second, teams):
        return {"fixtures": [{"fixture_id": 1, "outcome": first}, {"fixture_id": 2, "outcome": second}],
                "teams": teams}

    def test_branch_probability_fractions_ignore_asymmetric_normalizers(self):
        result = {"seed": 60013, "main": {"probability": 1.0}}
        branches = [{"main": {"probability": 0.2}}, {"main": {"probability": 0.8}}]
        probabilities, fractions = analyzer.result_contributions(result, branches, "main")
        self.assertEqual(probabilities, [0.2, 0.8])
        self.assertEqual(fractions, [0.2, 0.8])
        self.assertAlmostEqual(sum(fractions), 1.0)

    def test_away_win_code_and_pal16_final_rival_classification(self):
        witness = self.witness(0, 2, [
            {"team_id": 10, "points": 0, "wins": 0},
            {"team_id": 16, "points": 3, "wins": 1},
            {"team_id": 17, "points": 4, "wins": 1},
        ])
        summary = analyzer.witness_summary(witness, 16, self.request())
        self.assertEqual(summary["target_wdl"], {"wins": 2, "draws": 0, "losses": 0})
        self.assertEqual(summary["target_remaining_wdl"], summary["target_wdl"])
        self.assertEqual(summary["rival_classes"]["above"], [17])
        self.assertEqual(summary["rival_classes"]["below"], [10])

    def test_main_and_check_contribution_conserve_per_semantic_root(self):
        req = self.request()
        with tempfile.TemporaryDirectory() as tmp:
            request_path = Path(tmp) / "request.json"
            request_path.write_text(json.dumps(req))
            branches = []
            for i, (main_p, check_p, outcomes) in enumerate(((0.25, 0.6, [0, 2]), (0.75, 0.4, [2, 0]))):
                teams = [{"team_id": 10, "points": 0, "wins": 0},
                         {"team_id": 16, "points": 3, "wins": 1},
                         {"team_id": 17, "points": 3, "wins": 1}]
                witness = self.witness(*outcomes, teams)
                branches.append({"index": i, "pilot": {"hits": 0}, "main": {"probability": main_p},
                                 "check": {"probability": check_p}, "main_outcome_witness": witness,
                                 "check_outcome_witness": witness})
            result = {"seed": 60013, "pilot_seed": 808, "main": {"probability": 1.0},
                      "check": {"probability": 1.0}, "branches": branches}
            run = {"path": tmp, "record": {"request": str(request_path), "arguments": {"team": 16}},
                   "setup": {"proposal": {"team": 16, "tree": {"selected": [
                       {"selected": [16], "strict": [17]}, {"selected": [16], "strict": [10]}]},
                       "strata": [{"target_outcomes": [0, 2], "proposal_normalizer": 0.01},
                                  {"target_outcomes": [2, 0], "proposal_normalizer": 0.99}]}},
                   "results": [result]}
            analyzed = analyzer.analyze_run(run)
            self.assertEqual([b["normalized_branch_contribution"] for b in analyzed["branches"]], [0.25, 0.75])
            self.assertEqual([b["normalized_check_contribution"] for b in analyzed["branches"]], [0.6, 0.4])
            self.assertAlmostEqual(sum(b["normalized_branch_contribution"] for b in analyzed["branches"]), 1.0)
            self.assertAlmostEqual(sum(b["normalized_check_contribution"] for b in analyzed["branches"]), 1.0)
            self.assertTrue(all(b["strict_signature"]["selected_metadata_available"] for b in analyzed["branches"]))

    def test_expected_seed_completeness_and_duplicate_failures(self):
        with self.assertRaisesRegex(ValueError, "exactly one result"):
            analyzer.validate_results([{"seed": 60013}], [60013, 60017])
        with self.assertRaisesRegex(ValueError, "exactly one result"):
            analyzer.validate_results([{"seed": 60013}, {"seed": 60013}], [60013, 60017])
        analyzer.validate_results([{"seed": 60013}, {"seed": 60017}], [60013, 60017])

    def test_main_writes_compact_twelve_panel_three_seed_summaries(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            binary = root / "branch-stratification"
            binary.write_bytes(b"synthetic binary")
            digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
            requests, request_paths = {}, {}
            for phase in ("before", "after"):
                request_paths[phase] = root / f"{phase}-request.json"
                requests[phase] = {"scenario": phase, "team_groups": [{"team_id": 10}, {"team_id": 16}, {"team_id": 17}],
                                   "games": [{"id": 1, "home_id": 10, "away_id": 16, "played": False},
                                             {"id": 2, "home_id": 10, "away_id": 17, "played": False}]}
                request_paths[phase].write_text(json.dumps(requests[phase]))
            for panel in analyzer.PANELS:
                out = root / panel
                out.mkdir()
                phase, cell = panel.split("-", 2)[:2]
                team, rank = analyzer.EXPECTED_CELL[cell]
                pilot = 60031 if panel.endswith("-pilot60031") else 808
                setup = {"event": "strata_setup", "proposal": {"team": team, "rank": rank,
                    "tree": {"selected": [{"selected": [16], "strict": [10]}]},
                    "strata": [{"target_outcomes": [0], "proposal_normalizer": 0.7,
                                "direction": "above", "exceptions": [10]}]}}
                rows = [setup]
                teams = [{"team_id": 10, "points": 0, "wins": 0}, {"team_id": 16, "points": 3, "wins": 1},
                         {"team_id": 17, "points": 3, "wins": 1}]
                witness = {"fixtures": [{"fixture_id": 1, "outcome": 0}, {"fixture_id": 2, "outcome": 0}], "teams": teams}
                for seed in analyzer.SEEDS:
                    rows.append({"event": "strata_result", "seed": seed, "pilot_seed": pilot, "accepted": True,
                        "main": {"probability": 1.0, "ess": 4.0, "max_share": 0.2},
                        "check": {"probability": 1.0, "ess": 3.0, "max_share": 0.3},
                        "messages": {"selected": []}, "branches": [{"index": 0, "draws": 10,
                            "pilot": {"hits": 1}, "main": {"probability": 1.0, "ess": 4.0, "max_share": 0.2},
                            "check": {"probability": 1.0, "ess": 3.0, "max_share": 0.3},
                            "main_outcome_witness": witness, "check_outcome_witness": witness}]})
                stdout = out / "stdout.jsonl"
                stdout.write_text("".join(json.dumps(row) + "\n" for row in rows))
                stderr = out / "stderr.txt"
                stderr.write_text("")
                baseline_messages = 3 if cell == "pal15" and pilot == 60031 else 0
                record = {"exit_code": 0, "arguments": {"team": team, "rank": rank, "pilot_seed": pilot,
                            "draws": 600000, "seeds": ",".join(map(str, analyzer.SEEDS)), "messages": baseline_messages},
                          "binary": str(binary), "binary_sha256": digest(binary),
                          "request": str(request_paths[phase]), "request_sha256": digest(request_paths[phase]),
                          "environment_overrides": {"TREE_MESSAGES": str(baseline_messages)} if baseline_messages else {},
                          "wall_seconds": 1.0, "child_cpu_seconds": 0.5,
                          "output_files": {"stdout.jsonl": digest(stdout), "stderr.txt": digest(stderr)}}
                (out / "record.json").write_text(json.dumps(record))
            for phase in ("before", "after"):
                base_panel = f"{phase}-pal15-100x-pilot60031"
                source = root / base_panel
                extra = root / f"{base_panel}-messages4"
                extra.mkdir()
                for name in ("stdout.jsonl", "stderr.txt"):
                    (extra / name).write_bytes((source / name).read_bytes())
                record = json.loads((source / "record.json").read_text())
                record["arguments"]["messages"] = 4
                record["environment_overrides"]["TREE_MESSAGES"] = "4"
                record["output_files"] = {name: digest(extra / name) for name in ("stdout.jsonl", "stderr.txt")}
                (extra / "record.json").write_text(json.dumps(record))
            output_path = root / "analysis-100x.json"
            expected_requests = {phase: digest(path) for phase, path in request_paths.items()}
            with patch.object(analyzer, "FROZEN_BINARY_SHA256", digest(binary)), \
                    patch.object(analyzer, "EXPECTED_REQUEST_SHA256", expected_requests), \
                    patch("sys.argv", ["analyze_100x.py", "--run-dir", str(root)]), contextlib.redirect_stdout(io.StringIO()):
                analyzer.main()
            payload = json.loads(output_path.read_text())
            self.assertEqual(len(payload["panel_summary_across_final_seeds"]), 12)
            self.assertEqual(payload["summary"]["before-pal16-100x/60013"]["result_count"], 1)
            self.assertEqual(payload["summary"]["before-pal16-100x/60013"]["branch_concentration"]["top1_mass"]["mean"], 1.0)
            self.assertEqual(payload["runs"]["before-pal16-100x"]["60013"]["roots"][0]["first_hit_rival_ids_by_points_wins_class"]["main"]["above"], 0)
            self.assertIn("top_main_semantic_root_contribution_deltas", payload["pilot_seed_paired_comparison"]["before-pal16-100x"])
            self.assertEqual(payload["messages4_counterfactual"]["before-pal15-100x-pilot60031-messages4"]["by_seed"]["60013"]["baseline_messages3"]["estimate"]["main_probability"], 1.0)


if __name__ == "__main__":
    unittest.main()
