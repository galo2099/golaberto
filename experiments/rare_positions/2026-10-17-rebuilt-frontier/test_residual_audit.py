import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import residual_audit


class Clock:
    def __init__(self):
        self.value = 50.0

    def __call__(self):
        return self.value

    def advance(self, seconds):
        self.value += seconds


class FakeEngine:
    def __init__(self, binary, request, team, position, deadline, clock, target=None):
        self.deadline = deadline
        self.clock = clock
        self.calls = []
        self.target = target or {"team_id": team, "position": position}
        self.clock.advance(.4)  # process startup is part of the shared wall budget

    def call(self, **request):
        self.calls.append(dict(request))
        self.clock.advance(.1)
        if request["op"] == "describe":
            return {"target": self.target, "canonical": "fixture-canonical",
                    "regions": [{"id": 5, "root_id": 0, "upper": .25,
                                 "plan": {"k": 9, "states": 100}}]}
        if request["op"] == "prepare":
            return {"accepted": True, "states": 100, "B": .25,
                    "selected_rival_ids": list(range(9)), "mode": "joint"}
        if request["op"] == "regime":
            return {"accepted": True, "states": 200, "B": .2,
                    "selected_rival_ids": list(range(9)), "mode": "regime"}
        if request["op"] == "residual_certificate":
            return {"mode": "residual_certificate", "complete": False,
                    "expectation_upper": .01, "max_residual_cap": .2,
                    "expanded": 7, "prune_count": 3}
        return {}

    def close(self):
        self.calls.append({"op": "close"})
        self.clock.advance(.05)


class ResidualAuditTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.request = root / "request.json"
        self.request.write_text(json.dumps({"games": [
            *({"played": True} for _ in range(279)),
            *({"played": False} for _ in range(101)),
        ]}))
        self.binary = root / "fake-engine"
        self.binary.write_text("fake binary")
        self.args = SimpleNamespace(binary=str(self.binary), request=str(self.request),
                                     team_id=16, position=16, region_id=5, root_id=0, k=9,
                                     state_cap=250000, budget=1.5,
                                     cleanup_reserve=.1, node_limit=250000,
                                     clip_fraction=1/1024, search_ms=200)
        self.input_hash = hashlib.sha256(self.request.read_bytes()).hexdigest()

    def tearDown(self):
        self.tmp.cleanup()

    def test_setup_cost_is_charged_before_certificate_and_known_bound_survives(self):
        clock = Clock()
        engines = []

        def factory(*args):
            engine = FakeEngine(*args, clock=clock)
            engines.append(engine)
            return engine

        with patch.object(residual_audit, "HISTORICAL_REQUEST_SHA256", self.input_hash):
            result = residual_audit.run_audit(self.args, factory, clock)

        engine = engines[0]
        self.assertEqual([call["op"] for call in engine.calls],
                         ["describe", "prepare", "regime", "residual_certificate", "close"])
        self.assertLess(engine.calls[1]["time_ms"], 1000)
        self.assertLess(engine.calls[2]["time_ms"], 1000)
        self.assertEqual(engine.calls[3]["time_ms"], 200)
        self.assertNotIn("suffix", engine.calls[3])
        self.assertAlmostEqual(engine.deadline, 51.5)
        self.assertEqual(result["game_counts"], {"played": 279, "remaining": 101})
        self.assertEqual(result["known_analytic_upper"], .25)
        self.assertEqual(result["root_id"], 0)
        self.assertEqual(result["status"], "incomplete")
        self.assertEqual(result["certificate"]["expanded"], 7)
        self.assertGreater(result["timing_ms"]["wall_elapsed"], 0)
        self.assertEqual(result["overrun_ms"], 0)
        self.assertFalse(result["suffix_bounds_enabled"])
        self.assertFalse(result["suffix_request_sent"])
        self.assertEqual(result["suffix_settings"], {
            "state_cap": 1_000_000, "max_remaining": 24,
            "probe_nodes": 25_000, "probe_ms": 25,
            "budget_fraction": .15, "canonicalize_certain": False,
            "joint_ratio": False,
        })
        self.assertTrue(result["hashes_unchanged"])

    def test_describe_target_mismatch_stops_before_prepare(self):
        clock = Clock()
        engines = []

        def factory(*args):
            engine = FakeEngine(*args, clock=clock,
                                target={"team_id": 99, "position": 16})
            engines.append(engine)
            return engine

        with patch.object(residual_audit, "HISTORICAL_REQUEST_SHA256", self.input_hash):
            result = residual_audit.run_audit(self.args, factory, clock)
        self.assertEqual(result["status"], "error")
        self.assertIn("does not match", result["errors"][0])
        self.assertEqual([call["op"] for call in engines[0].calls], ["describe", "close"])
        self.assertIsNone(result["known_analytic_upper"])

    def test_describe_plan_over_cap_stops_and_keeps_analytic_bound(self):
        clock = Clock()
        engines = []

        def factory(*args):
            engine = FakeEngine(*args, clock=clock)
            engines.append(engine)
            engine.call = lambda **request: (
                {"target": {"team_id": 16, "position": 16},
                 "regions": [{"id": 5, "root_id": 0, "upper": .25,
                              "plan": {"k": 9, "states": 300000}}]}
                if request["op"] == "describe" else {}
            )
            return engine

        self.args.state_cap = 250000
        with patch.object(residual_audit, "HISTORICAL_REQUEST_SHA256", self.input_hash):
            result = residual_audit.run_audit(self.args, factory, clock)
        self.assertEqual(result["status"], "error")
        self.assertIn("exceeds state cap", result["errors"][0])
        self.assertEqual(result["known_analytic_upper"], .25)
        self.assertEqual([call["op"] for call in engines[0].calls], ["close"])

    def test_certificate_error_preserves_known_analytic_upper(self):
        clock = Clock()
        engines = []

        def factory(*args):
            engine = FakeEngine(*args, clock=clock)
            engines.append(engine)
            original_call = engine.call

            def fail_certificate(**request):
                if request["op"] == "residual_certificate":
                    raise RuntimeError("audit deadline")
                return original_call(**request)

            engine.call = fail_certificate
            return engine

        with patch.object(residual_audit, "HISTORICAL_REQUEST_SHA256", self.input_hash):
            result = residual_audit.run_audit(self.args, factory, clock)
        self.assertEqual(result["status"], "error")
        self.assertEqual(result["known_analytic_upper"], .25)
        self.assertIsNone(result["certificate"])

    def test_suffix_opt_in_is_forwarded_as_one_optional_object(self):
        clock = Clock()
        engines = []
        self.args.suffix_bounds = True
        self.args.suffix_state_cap = 123456
        self.args.suffix_max_remaining = 18
        self.args.suffix_probe_nodes = 4321
        self.args.suffix_probe_ms = 17
        self.args.suffix_budget_fraction = .2
        self.args.suffix_canonicalize_certain = True
        self.args.suffix_joint_ratio = True

        def factory(*args):
            engine = FakeEngine(*args, clock=clock)
            engines.append(engine)
            return engine

        with patch.object(residual_audit, "HISTORICAL_REQUEST_SHA256", self.input_hash):
            result = residual_audit.run_audit(self.args, factory, clock)
        request = engines[0].calls[3]
        self.assertEqual(request["suffix"], {
            "state_cap": 123456, "max_remaining": 18,
            "probe_nodes": 4321, "probe_ms": 17,
            "budget_fraction": .2, "canonicalize_certain": True,
            "joint_ratio": True,
        })
        self.assertTrue(result["suffix_bounds_enabled"])
        self.assertTrue(result["suffix_request_sent"])
        self.assertTrue(result["hashes_unchanged"])

    def test_suffix_options_reject_nonpositive_integers_and_invalid_fraction(self):
        for name in ("suffix_state_cap", "suffix_max_remaining", "suffix_probe_nodes", "suffix_probe_ms"):
            setattr(self.args, name, 0)
            with self.subTest(option=name), self.assertRaises(ValueError):
                residual_audit.suffix_options(self.args)
            delattr(self.args, name)
        for fraction in (0.0, -0.1, 1.1, float("nan"), float("inf")):
            self.args.suffix_budget_fraction = fraction
            with self.subTest(fraction=fraction), self.assertRaises(ValueError):
                residual_audit.suffix_options(self.args)
        del self.args.suffix_budget_fraction

    def test_canonicalize_certain_requires_suffix_bounds_and_boolean_value(self):
        self.args.suffix_canonicalize_certain = True
        with self.assertRaisesRegex(ValueError, "requires --suffix-bounds"):
            residual_audit.suffix_options(self.args)
        self.args.suffix_bounds = True
        self.args.suffix_canonicalize_certain = "true"
        with self.assertRaisesRegex(ValueError, "must be boolean"):
            residual_audit.suffix_options(self.args)

    def test_joint_ratio_requires_bounds_and_is_forwarded(self):
        self.args.suffix_joint_ratio = True
        with self.assertRaisesRegex(ValueError, "requires --suffix-bounds"):
            residual_audit.suffix_options(self.args)
        self.args.suffix_bounds = True
        self.args.suffix_joint_ratio = "true"
        with self.assertRaisesRegex(ValueError, "must be boolean"):
            residual_audit.suffix_options(self.args)


if __name__ == "__main__":
    unittest.main()
