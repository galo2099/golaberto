#!/usr/bin/env python3
"""Focused tests for R71 diagnostic parsing and matrix validation."""
from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("analyze_matrix.py")
SPEC = importlib.util.spec_from_file_location("r71_analyze_matrix", SCRIPT)
analyzer = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(analyzer)


class AnalyzerTests(unittest.TestCase):
    def test_nested_attempts_and_no_selection_are_counted_once(self):
        events = [{
            "event": "rust_odds_rare_tail_branches_skip",
            "team": 17,
            "rank": 13,
            "reason": "no event evidence after structural discovery re-pilot",
            "messages": {
                "mode": "rank",
                "attempts": [
                    {"index": 4, "unseen": True, "work_delta": 21, "rejection": None},
                    {"index": 5, "unseen": False, "work_delta": 3, "rejection": "guide_bound"},
                ],
                "selected": [],
                "work": 24,
                "guide_predicted_work": 19,
                "remaining_grant": 30,
            },
            # This tree field must not be misread as a second message record.
            "setup": {"attempts": 19, "selected": {"count": 2}},
        }]
        found = analyzer.message_diagnostics(events, 17, 13)
        self.assertEqual(len(found), 1)
        self.assertEqual(found[0]["attempt_count"], 2)
        self.assertEqual(found[0]["successful_attempt_count"], 1)
        self.assertEqual(found[0]["unseen_successful_attempt_count"], 1)
        self.assertEqual(found[0]["selected_success_count"], 0)
        self.assertEqual(found[0]["selected_unseen_success_count"], 0)
        self.assertEqual(found[0]["rejection_labels"], {"guide_bound": 1})
        self.assertIn({"path": "", "field": "guide_predicted_work", "value": 19}, found[0]["predicted_guide_charges"])

    def test_successful_zero_hit_selected_attempt_is_detected(self):
        event = {
            "event": "rust_odds_rare_tail_branches",
            "team": 16,
            "rank": 15,
            "messages": {
                "attempts": [{"index": 2, "unseen": True, "rejection": None}],
                "selected": [2],
                "work": 12,
                "work_grant": 18,
            },
        }
        found = analyzer.message_diagnostics([event], 16, 15)
        self.assertEqual(len(found), 1)
        self.assertEqual(found[0]["selected_unseen_success_count"], 1)
        self.assertEqual(found[0]["message_grants"][0]["value"], 18)

    def test_missing_rejection_is_not_assumed_successful(self):
        event = {"event": "rust_odds_rare_tail_branches", "team": 17, "rank": 13,
                 "messages": {"attempts": [{"index": 1, "unseen": True}], "selected": [1]}}
        found = analyzer.message_diagnostics([event], 17, 13)
        self.assertEqual(found[0]["attempt_count"], 0)
        self.assertEqual(found[0]["selected_unseen_success_count"], 0)

    def test_default_parity_comparison_ignores_only_work_spent(self):
        left = {"rare_position_estimates": {"17": {"12": {"probability": 0.1, "work_spent": 10}}}}
        right = {"rare_position_estimates": {"17": {"12": {"probability": 0.1, "work_spent": 99}}}}
        self.assertEqual(analyzer.without_work_spent(left), analyzer.without_work_spent(right))
        changed = {"rare_position_estimates": {"17": {"12": {"probability": 0.2, "work_spent": 99}}}}
        self.assertNotEqual(analyzer.without_work_spent(left), analyzer.without_work_spent(changed))

    def test_budget_overrun_is_reported(self):
        with tempfile.TemporaryDirectory() as temp:
            export = Path(temp) / "export.json"
            export.write_text(json.dumps({"rare_position_estimates": {}}))
            stdout = Path(temp) / "stdout.log"
            stderr = Path(temp) / "stderr.log"
            stdout.write_text(json.dumps({"event": "budget", "grant": 4, "actual": 5}) + "\n")
            stderr.write_text("")
            record = {"export_data": {"rare_position_estimates": {}}, "stdout": str(stdout),
                      "stderr": str(stderr), "stdout_sha256": analyzer.file_sha256(stdout),
                      "stderr_sha256": analyzer.file_sha256(stderr)}
            work = analyzer.modeled_work(record, analyzer.record_events(record))
            self.assertEqual(len(work["overruns_or_within_grant_failures"]), 1)

    def test_branch_draw_audit_failure_is_reported(self):
        with tempfile.TemporaryDirectory() as temp:
            stdout = Path(temp) / "stdout.log"
            stderr = Path(temp) / "stderr.log"
            stdout.write_text(json.dumps({
                "event": "rust_odds_rare_tail_branches",
                "branch_draw_audit_valid": True,
                "branch_draw_reserved_units": 10,
                "branch_draw_actual_units": 8,
                "branch_draw_already_released_units": 3,
            }) + "\n")
            stderr.write_text("")
            record = {"export_data": {"rare_position_estimates": {}}, "stdout": str(stdout),
                      "stderr": str(stderr), "stdout_sha256": analyzer.file_sha256(stdout),
                      "stderr_sha256": analyzer.file_sha256(stderr)}
            work = analyzer.modeled_work(record, analyzer.record_events(record))
            self.assertEqual(work["overruns_or_within_grant_failures"][0]["field"], "branch_draw_actual_units")

    def test_incomplete_matrix_fails_clearly(self):
        with tempfile.TemporaryDirectory() as temp:
            run_dir = Path(temp)
            config = {
                "fixtures": {"fixture": {"path": "/missing/request.json", "sha256": "x"}},
                "seeds": [808], "repeats": 1,
                "arms": [{"name": "control", "binary": "/missing/binary", "binary_sha256": "x", "flags": {}}],
            }
            (run_dir / "run_manifest.json").write_text(json.dumps({"config": config, "config_hash": "x"}))
            report = analyzer.analyze(run_dir, 17, 16)
            self.assertFalse(report["valid"])
            self.assertEqual(report["validation_failures"][0]["kind"], "incomplete_or_invalid_matrix")


if __name__ == "__main__":
    unittest.main()
