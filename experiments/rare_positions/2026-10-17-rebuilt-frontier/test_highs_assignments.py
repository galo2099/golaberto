import unittest

from audit_highs_assignments import enumerate_prefix_coverage, verify_witness, wdl_for_prefix


class AssignmentAuditTests(unittest.TestCase):
    def setUp(self):
        self.model = {
            "team_ids": [10, 20], "base_packed": [0, 0],
            "lower_packed": [0, 0], "upper_packed": [20, 20],
            "fixtures": [{"home_id": 10, "away_id": 20, "mask": 7,
                          "outcomes": [{"home": 0, "away": 10},
                                       {"home": 3, "away": 3},
                                       {"home": 10, "away": 0}]},
                         {"home_id": 20, "away_id": 10, "mask": 7,
                          "outcomes": [{"home": 0, "away": 10},
                                       {"home": 3, "away": 3},
                                       {"home": 10, "away": 0}]}]
        }
        self.prefix_fixtures = [{"fixture_index": 0, "home": 10, "away": 20},
                                {"fixture_index": 1, "home": 20, "away": 10}]

    def test_mixed_home_away_native_outcome_orientation(self):
        prefix = [0, 2]
        wdl, loss_opponent = wdl_for_prefix(prefix, self.prefix_fixtures,
                                            self.model, pivot_id=10)
        self.assertEqual(wdl, {"W": 0, "D": 0, "L": 2})
        self.assertEqual(loss_opponent, 20)
        prefix = [2, 0]
        wdl, loss_opponent = wdl_for_prefix(prefix, self.prefix_fixtures,
                                            self.model, pivot_id=10)
        self.assertEqual(wdl, {"W": 2, "D": 0, "L": 0})
        self.assertIsNone(loss_opponent)

    def test_replay_rejects_prefix_outcome_conflicting_with_native_mask(self):
        self.model["fixtures"][0]["mask"] = 1 << 2
        ok, why = verify_witness(self.model, [0], [0], {"outcome_indices": [0, 1]})
        self.assertFalse(ok)
        self.assertIn("mask", why)

    def test_replay_rejects_solver_assignment_violating_packed_bound(self):
        self.model["upper_packed"] = [0, 20]
        ok, why = verify_witness(self.model, [0], [2], {"outcome_indices": [2, 1]})
        self.assertFalse(ok)
        self.assertIn("packed bound", why)

    def test_coverage_enumeration_matches_prefix_set(self):
        audit = {"pivot_id": 10, "prefix_fixtures": self.prefix_fixtures,
                 "prefixes": [{"prefix": [a, b]} for a in range(3) for b in range(3)],
                 "pattern_count": 9}
        indices, assignments = enumerate_prefix_coverage(self.model, audit, team_id=10)
        self.assertEqual(indices, [0, 1])
        self.assertEqual(len(assignments), 9)


if __name__ == "__main__":
    unittest.main()
