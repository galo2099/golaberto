import unittest
from unittest.mock import patch

from hybrid import HybridEpochStats


def stream(epoch=4, n=2, cap=1.0, low=0, high=1):
    return {"n": n, "epoch": epoch, "B": cap,
            "lower_exceedances": [low] * 241, "upper_exceedances": [high] * 241,
            "sum_lower": low * .25, "sum_upper": high * .5,
            "positive_draws": high, "sum_lower_sq": low * .0625,
            "sum_upper_sq": high * .25}


def receipt(epoch=4, core=None, residual=None, baseline=None, total=4, B=1., fraction=.25):
    return {"n": total, "epoch": epoch, "B": B, "H": B*fraction,
            "streams": {"core": core or stream(epoch, total//2, B*fraction),
                        "residual": residual or stream(epoch, total//2, B),
                        "baseline": baseline or stream(epoch, total//2, B)}}


class HybridStatsTests(unittest.TestCase):
    def test_accumulates_each_stream_at_half_physical_draw_count(self):
        stats = HybridEpochStats(4, .25)
        stats.consume(receipt(), 4)
        self.assertEqual(stats.n, 2)
        self.assertEqual(stats.streams["core"].lower[0], 0)
        self.assertEqual(stats.streams["residual"].upper[0], 1)
        self.assertEqual(stats.streams["baseline"].upper[0], 1)
        self.assertEqual(stats.diagnostic()["core"]["n"], 2)

    def test_confidence_alpha_is_split_three_ways(self):
        stats = HybridEpochStats(4, .25)
        stats.consume(receipt(), 4)
        with patch("hybrid.threshold_mean_bounds", return_value=(.1, .2)) as bounds:
            lo, hi = stats.bounds(.03, .9)
        self.assertEqual(bounds.call_count, 3)
        self.assertTrue(all(call.args[3] == .01 for call in bounds.call_args_list))
        self.assertLessEqual(lo, hi)

    def test_reset_clears_streams_and_keeps_clip_fraction(self):
        stats = HybridEpochStats(4, .25)
        stats.consume(receipt(), 4)
        stats.reset(5)
        self.assertEqual((stats.epoch, stats.n, stats.clip_fraction), (5, 0, .25))

    def test_cap_or_clip_mismatch_rejected_transactionally(self):
        stats = HybridEpochStats(4, .25)
        stats.consume(receipt(), 4)
        before = stats.diagnostic()
        bad = receipt(total=2)
        bad["streams"]["residual"]["B"] = .9
        with self.assertRaises(ValueError): stats.consume(bad, 2)
        self.assertEqual(stats.diagnostic(), before)
        bad = receipt(total=2); bad["H"] = .3
        with self.assertRaises(ValueError): stats.consume(bad, 2)
        self.assertEqual(stats.diagnostic(), before)

    def test_invalid_later_stream_does_not_commit_earlier_streams(self):
        stats = HybridEpochStats(4, .25)
        bad = receipt()
        bad["streams"]["baseline"]["upper_exceedances"] = [3] * 241
        with self.assertRaises(ValueError): stats.consume(bad, 4)
        self.assertEqual(stats.n, 0)


if __name__ == "__main__": unittest.main()
