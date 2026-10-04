"""Small output-classification and native-schema checks for the adoption driver."""
import importlib.util
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("run_adoption.py")
SPEC = importlib.util.spec_from_file_location("adoption_driver", SCRIPT)
DRIVER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DRIVER)


class AdoptionHelpersTest(unittest.TestCase):
    def test_positive_metadata_ignores_only_request_work(self):
        before = {"rare_position_estimates": {"1": {"2": {"probability": 0.25, "hits": 4, "work_spent": 10}}}}
        after = {"rare_position_estimates": {"1": {"2": {"probability": 0.25, "hits": 4, "work_spent": 22}}}}
        self.assertEqual(DRIVER.positive_differences(before, after), [])
        after["rare_position_estimates"]["1"]["2"]["hits"] = 5
        self.assertEqual(DRIVER.positive_differences(before, after)[0]["team_id"], "1")
        self.assertEqual(DRIVER.work_counter_deltas(before, after)[0]["delta"], 12)

    def test_zero_to_positive_is_gain_and_positive_to_zero_is_loss(self):
        before = {"rare_position_estimates": {"1": {"1": {"probability": 0.0}, "2": {"probability": 0.3}}}}
        after = {"rare_position_estimates": {"1": {"1": {"probability": 0.1}, "2": {"probability": 0.0}}}}
        gains, losses = DRIVER.gain_loss(before, after)
        self.assertEqual([(x["team_id"], x["rank_index"]) for x in gains], [("1", "1")])
        self.assertEqual([(x["team_id"], x["rank_index"]) for x in losses], [("1", "2")])

    def test_native_comparison_requires_cell_identity_and_core_metrics(self):
        event = {"event": "rust_odds_rare_tail_confirm_more", "team": 95, "rank": 3}
        with self.assertRaisesRegex(ValueError, "missing essential fields"):
            DRIVER.native_cells([event])

    def test_production_late_summary_aliases_match_r60_semantics_and_ledger(self):
        r60 = {"event": "rust_odds_family_fallback_late_summary", "attempted": 1,
               "skipped_superseded_or_impossible": 0, "published": 1,
               "added_actual_work": 10, "granted_work": 12,
               "settled_fallback_work": 10, "unused_grant_work": 2,
               "published_charged_work": 10, "handoff_fee": 3}
        production = {"event": "rust_odds_family_fallback_late_summary", "attempts": 1,
                      "skips": 0, "added": 1, "added_actual_work": 10,
                      "granted_work": 12, "settled_fallback_work": 10,
                      "unused_grant_work": 2, "published_charged_work": 10,
                      "handoff_fee": 3}
        components = {"training_operation_work": 1, "family_collection_work": 1,
                      "clone_work": 1, "fit_work": 1, "validation_work": 1, "final_work": 5}
        fallback = {"base_fallback_grant": 12, "fallback_grant": 12,
                    "added_actual_work": 10, "unused_fallback_grant": 2,
                    "settlement_within_grant": True, "overrun": False,
                    "published": True, "native_training_funding": "stage",
                    "native_training_observer_bound": 1, "native_training_observer_work": 0,
                    "added_actual_work_including_stage_observer": 10,
                    "native_plus_added_work": 15,
                    "training_draws": 1, "training_replayed_draws": 1,
                    "training_new_draws": 0, "training_cap": 3000,
                    "training_hits": 0, "training_ess": 0.0,
                    "validation_draws": 1, "validation_hits": 0,
                    "validation_ess": 0.0, "final_draws_per_stream": 1,
                    **components}
        per_cell = {"event": "rust_odds_family_fallback", "team_id": 95, "rank": 3, "fallback": fallback}
        self.assertEqual(DRIVER.family_late_signature([r60]), DRIVER.family_late_signature([production]))
        audit = DRIVER.fallback_ledger([production, per_cell])
        self.assertTrue(audit["schema_complete"])
        self.assertTrue(audit["all_checks_pass"], audit)


if __name__ == "__main__":
    unittest.main()
