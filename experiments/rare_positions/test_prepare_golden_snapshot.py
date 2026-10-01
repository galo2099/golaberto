import copy
import unittest

from prepare_golden_snapshot import unplay_latest_games


class PrepareGoldenSnapshotTest(unittest.TestCase):
    def test_latest_results_are_removed_without_losing_fixtures_or_powers(self):
        games = [
            {"id": 2, "date": "2026-09-29T12:00:00Z", "played": True, "home_score": 2, "away_score": 1},
            {"id": 1, "date": "2026-09-29T12:00:00Z", "played": True, "home_score": 3, "away_score": 0},
            {"id": 3, "date": "2026-09-30T12:00:00Z", "played": False, "home_score": 0, "away_score": 0},
            {"id": 4, "date": "2026-09-30T12:00:00Z", "played": True, "home_score": 1, "away_score": 1},
        ]
        for game in games:
            game.update(home_power=1.2, away_power=0.8)
        raw = {"id": 7, "phase": {"id": 99}, "games": games}
        original = copy.deepcopy(raw)
        result = unplay_latest_games(raw, 2)
        self.assertEqual(raw, original, "transformation must not mutate its source")
        self.assertEqual([g["id"] for g in result["games"]], [2, 1, 3, 4])
        self.assertEqual(result["dataset_transform"]["removed_game_ids"], [4, 2])
        self.assertEqual(result["dataset_transform"]["remaining_games"], 3)
        for before, after in zip(games, result["games"]):
            self.assertEqual(after["home_power"], before["home_power"])
            self.assertEqual(after["away_power"], before["away_power"])
            if after["id"] in (2, 4):
                self.assertEqual((after["played"], after["home_score"], after["away_score"]), (False, 0, 0))
            else:
                self.assertEqual(after, before)

    def test_invalid_removal_counts_and_missing_dates_are_rejected(self):
        raw = {"games": [{"id": 1, "date": "2026-09-30", "played": True}]}
        for count in (-1, 2):
            with self.assertRaises(ValueError):
                unplay_latest_games(raw, count)
        with self.assertRaises(ValueError):
            unplay_latest_games({"games": [{"id": 1, "played": True}]}, 1)
        with self.assertRaises(ValueError):
            unplay_latest_games({"games": raw["games"] * 2}, 1)


if __name__ == "__main__":
    unittest.main()
