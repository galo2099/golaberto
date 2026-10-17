"""Coverage and input-integrity checks for the Set 4 partition."""
from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
import unittest


HERE = Path(__file__).resolve().parent
ARTIFACT = HERE.parents[1]
INPUTS = ARTIFACT / "inputs"
SPEC = importlib.util.spec_from_file_location("set4_gd_experiment", HERE / "experiment.py")
experiment = importlib.util.module_from_spec(SPEC)
assert SPEC is not None and SPEC.loader is not None
SPEC.loader.exec_module(experiment)


class Set4PartitionTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.request = json.loads((INPUTS / "request.json").read_text())
        cls.saved = json.loads((INPUTS / "target-16-rank-15-separator-saved.json").read_text())
        cls.before = copy.deepcopy(cls.saved)
        cls.audit = experiment.load_audit()
        cls.prepared, cls.receipt = experiment.partition(cls.saved, cls.request, cls.audit)
        cls.data = cls.audit.load_data()

    def test_covers_all_and_only_allowed_parent_pw_patterns(self):
        self.assertEqual(self.receipt["pw_valid_parent_patterns"], 11)
        self.assertEqual(len(self.prepared["cohorts"]), 118)
        labels = [row["gd_partition"]["label"] for row in self.prepared["cohorts"][4:5]
                  + self.prepared["cohorts"][108:118]]
        self.assertEqual(labels.count("10W"), 1)
        self.assertEqual(labels.count("9W1D"), 10)
        self.assertEqual(len({row["gd_partition"]["draw_fixture_id"]
                              for row in self.prepared["cohorts"][108:118]}), 10)
        self.assertEqual(self.prepared["cohorts"][4]["gd_partition"]["label"], "10W")

    def test_masks_use_native_home_perspective_and_children_keep_parent_bounds(self):
        fixtures = self.data[5]
        original = self.saved["cohorts"][4]
        child_rows = [self.prepared["cohorts"][4], *self.prepared["cohorts"][108:118]]
        for row in child_rows:
            results = []
            for fi in self.receipt["incident_fixture_indices"]:
                mask = row["fixture_masks"][fi]
                self.assertEqual(mask & (mask - 1), 0)
                outcome = mask.bit_length() - 1
                self.assertTrue(original["fixture_masks"][fi] & mask)
                results.append(experiment.team_outcome(fixtures[fi], experiment.TEAM_ID, outcome))
            if row["gd_partition"]["label"] == "10W":
                self.assertEqual(results, [2] * 10)
            else:
                self.assertEqual(results.count(2), 9)
                self.assertEqual(results.count(1), 1)
                self.assertTrue(row["gd_partition"]["gd_expected"])
            parent_prior = original["joint_points_diagnostic"]["cohort_event_prior_upper"]
            self.assertEqual(row["lower"], 0.0)
            self.assertLessEqual(row["upper"], parent_prior)

    def test_only_set4_and_root0_prepared_count_change(self):
        self.assertEqual(len(self.prepared["root_points"]), 486)
        self.assertEqual(self.prepared["setup"]["candidate_slots_before_integer_pruning"], 2425)
        for index, row in enumerate(self.saved["cohorts"]):
            if index != 4:
                self.assertEqual(self.prepared["cohorts"][index], row)
        old_counts = {row["root_path_index"]: row["prepared"] for row in self.saved["root_cohort_counts"]}
        new_counts = {row["root_path_index"]: row["prepared"] for row in self.prepared["root_cohort_counts"]}
        self.assertEqual(new_counts[0], old_counts[0] + 10)
        self.assertEqual({k: v for k, v in new_counts.items() if k != 0},
                         {k: v for k, v in old_counts.items() if k != 0})

    def test_source_input_is_not_mutated(self):
        self.assertEqual(self.saved, self.before)


if __name__ == "__main__":
    unittest.main()
