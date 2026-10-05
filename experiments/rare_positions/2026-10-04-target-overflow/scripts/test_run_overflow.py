"""Focused tests for target overflow export comparisons."""
import importlib.util
import argparse
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("run_overflow.py")
SPEC = importlib.util.spec_from_file_location("run_overflow", SCRIPT)
run_overflow = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(run_overflow)


def export(probability=0.0, *, work_spent=10, hits=0, reachability="impossible", games=None):
    return {
        "rare_position_estimates": {
            "16": {"14": {
                "probability": probability,
                "work_spent": work_spent,
                "hits": hits,
                "reachability": reachability,
            }}
        },
        "game_importance": {"game-1": 0.25} if games is None else games,
    }


class CompareExportsTest(unittest.TestCase):
    def test_reports_gain_from_zero(self):
        result = run_overflow.compare_exports(export(), export(0.03, reachability="reachable"))
        self.assertEqual(len(result["gains_from_zero"]), 1)
        self.assertEqual(result["gains_from_zero"][0]["team_id"], "16")
        self.assertEqual(result["lost_positives"], [])

    def test_reports_loss_of_positive_estimate(self):
        result = run_overflow.compare_exports(export(0.03, reachability="reachable"), export())
        self.assertEqual(result["gains_from_zero"], [])
        self.assertEqual(len(result["lost_positives"]), 1)

    def test_positive_metadata_ignores_only_work_spent(self):
        base = export(0.03, work_spent=10)
        same_except_work = export(0.03, work_spent=99)
        changed_metadata = export(0.03, work_spent=99, hits=4)
        self.assertEqual(run_overflow.positive_changes(base, same_except_work), [])
        changes = run_overflow.positive_changes(base, changed_metadata)
        self.assertEqual(len(changes), 1)
        self.assertEqual(changes[0]["after"]["hits"], 4)

    def test_reports_proof_classification_change(self):
        result = run_overflow.compare_exports(export(), export(reachability="undecided"))
        self.assertEqual(len(result["impossible_or_undecided_reachability_changes"]), 1)
        self.assertFalse(result["all_zero_classifications_equal"])
        self.assertEqual(len(result["all_zero_classification_changes"]), 1)

    def test_reports_game_importance_change(self):
        result = run_overflow.compare_exports(
            export(), export(games={"game-1": 0.3})
        )
        self.assertFalse(result["game_importance_equal"])

    def test_rejects_malformed_estimate_row(self):
        with self.assertRaisesRegex(ValueError, "malformed estimate row"):
            run_overflow.matrix({"rare_position_estimates": {"16": []}})


class BaselineHashArgumentTest(unittest.TestCase):
    def test_default_hash_remains_frozen_r61_value(self):
        self.assertEqual(
            run_overflow.BASELINE_SHA256,
            "87991033bdb81854cec91f3d29c32c66f5b089f51d2530590e48b2a0efc40085",
        )

    def test_accepts_valid_hash_and_normalizes_case(self):
        digest = "A" * 64
        self.assertEqual(run_overflow.parse_sha256(digest), digest.lower())

    def test_rejects_invalid_hashes(self):
        for value in ("a" * 63, "a" * 65, "g" * 64):
            with self.subTest(value=value), self.assertRaises(argparse.ArgumentTypeError):
                run_overflow.parse_sha256(value)


if __name__ == "__main__":
    unittest.main()
