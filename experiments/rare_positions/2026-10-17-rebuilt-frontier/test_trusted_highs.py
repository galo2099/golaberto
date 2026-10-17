import unittest
from types import SimpleNamespace

from trusted_highs import prescreen


def model(lower, upper, mask=7):
    return {"stride": 3, "team_ids": [10, 20], "base_packed": [0, 0],
            "lower_packed": lower, "upper_packed": upper,
            "fixtures": [{"home_id": 10, "away_id": 20, "mask": mask,
                          "outcomes": [{"home": 0, "away": 10},
                                       {"home": 3, "away": 3},
                                       {"home": 10, "away": 0}]}]}


class TrustedHighsTests(unittest.TestCase):
    def run_model(self, data, **kwargs):
        import time
        return prescreen(data, .2, time.monotonic() + 2, **kwargs)

    def test_tied_packed_bounds_are_feasible(self):
        r = self.run_model(model([3, 3], [3, 3]))
        self.assertEqual(r["status"], 0)
        self.assertFalse(r["pruned"])
        self.assertEqual(r["feasible_witness"]["outcome_indices"], [1])

    def test_single_shared_fixture_cannot_make_both_teams_win(self):
        r = self.run_model(model([10, 10], [10, 10]))
        self.assertEqual(r["status"], 2)
        self.assertTrue(r["pruned"])

    def test_wdl_mask_orientation_home_and_away_wins(self):
        home = self.run_model(model([10, 0], [10, 0], mask=4))
        away = self.run_model(model([0, 10], [0, 10], mask=1))
        self.assertEqual(home["status"], 0)
        self.assertEqual(home["feasible_witness"]["outcome_indices"], [2])
        self.assertEqual(away["status"], 0)
        self.assertEqual(away["feasible_witness"]["outcome_indices"], [0])

    def test_only_status_two_prunes(self):
        for status in (1, 3, 4, "error"):
            fake = lambda *args, _status=status, **kw: SimpleNamespace(status=_status, message="mock")
            r = self.run_model(model([0, 0], [99, 99]), milp=fake)
            self.assertFalse(r["pruned"], status)


if __name__ == "__main__":
    unittest.main()
