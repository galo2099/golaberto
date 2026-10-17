"""Confidence accounting for the optional three-stream hybrid sampler."""
from __future__ import annotations

import math

from confidence import EpochStats, threshold_mean_bounds


def _outward_sum(lower: float, upper: float, other_lower: float, other_upper: float):
    return (max(0.0, math.nextafter(lower + other_lower, 0.0)),
            min(1.0, math.nextafter(upper + other_upper, math.inf)))


class HybridEpochStats:
    """Transactional accumulator for core, residual, and baseline score streams.

    ``n`` and the legacy empirical fields refer to the baseline stream, which
    samples one score for each pair of physical hybrid draws.
    """

    STREAMS = ("core", "residual", "baseline")

    def __init__(self, epoch: int, clip_fraction: float = 1 / 1024):
        if not math.isfinite(clip_fraction) or not 0 < clip_fraction <= 1:
            raise ValueError("clip_fraction must be finite and in (0, 1]")
        self.epoch = int(epoch)
        self.clip_fraction = float(clip_fraction)
        self.streams = {name: EpochStats(epoch) for name in self.STREAMS}
        self.conflict = False
        self.H = None

    def reset(self, epoch: int):
        self.__init__(epoch, self.clip_fraction)

    @property
    def baseline(self): return self.streams["baseline"]
    @property
    def n(self): return self.baseline.n
    @property
    def cap(self): return self.baseline.cap
    @property
    def lower(self): return self.baseline.lower
    @property
    def upper(self): return self.baseline.upper
    @property
    def sum_lower(self): return self.baseline.sum_lower
    @property
    def sum_upper(self): return self.baseline.sum_upper
    @property
    def positive_draws(self): return self.baseline.positive_draws
    @property
    def sum_lower_sq(self): return self.baseline.sum_lower_sq
    @property
    def sum_upper_sq(self): return self.baseline.sum_upper_sq

    def consume(self, receipt, expected_n: int):
        if expected_n < 2 or expected_n % 2:
            raise ValueError("hybrid physical draw count must be even and at least two")
        if int(receipt.get("n", -1)) != expected_n or int(receipt.get("epoch", -1)) != self.epoch:
            raise ValueError("hybrid sample receipt is incomplete or belongs to another epoch")
        if receipt.get("mode", "hybrid") != "hybrid":
            raise ValueError("sample receipt is not in hybrid mode")
        if "clip_fraction" in receipt and float(receipt["clip_fraction"]) != self.clip_fraction:
            raise ValueError("sample receipt clip fraction changed")
        streams = receipt.get("streams")
        if not isinstance(streams, dict):
            streams = {name: receipt.get(name) for name in self.STREAMS}
        if any(not isinstance(streams.get(name), dict) for name in self.STREAMS):
            raise ValueError("hybrid receipt must include all three stream receipts")
        pair_n = expected_n // 2
        staged = {}
        for name in self.STREAMS:
            rec = streams[name]
            if int(rec.get("n", -1)) != pair_n or int(rec.get("epoch", -1)) != self.epoch:
                raise ValueError("hybrid stream sample size or epoch is invalid")
            obj = EpochStats(self.epoch)
            obj.consume(rec, pair_n)
            staged[name] = obj
        caps = {name: staged[name].cap for name in self.STREAMS}
        B = float(receipt.get("B", caps["baseline"]))
        H = float(receipt.get("H", caps["core"]))
        want_H = B * self.clip_fraction
        if not math.isfinite(B) or B <= 0 or not math.isfinite(H) or H <= 0:
            raise ValueError("hybrid receipt has invalid B or H")
        if not math.isclose(H, want_H, rel_tol=1e-12, abs_tol=0.0):
            raise ValueError("hybrid clip cap does not match configured fraction")
        if caps["baseline"] != B or caps["residual"] != B or caps["core"] != H:
            raise ValueError("hybrid stream caps do not match B and H")
        if self.H is not None and (self.H != H or any(self.streams[k].cap != caps[k] for k in self.STREAMS)):
            raise ValueError("hybrid caps changed within one proposal epoch")
        for name in self.STREAMS:
            old = self.streams[name]
            obj = staged[name]
            # Stage cumulative addition in fresh objects; no stream commits alone.
            merged = EpochStats(self.epoch)
            merged.consume({"n": old.n + pair_n, "epoch": self.epoch,
                            "B": caps[name], "sum_lower": old.sum_lower + obj.sum_lower,
                            "sum_upper": old.sum_upper + obj.sum_upper,
                            "positive_draws": old.positive_draws + obj.positive_draws,
                            "sum_lower_sq": old.sum_lower_sq + obj.sum_lower_sq,
                            "sum_upper_sq": old.sum_upper_sq + obj.sum_upper_sq,
                            "lower_exceedances": [a+b for a,b in zip(old.lower,obj.lower)],
                            "upper_exceedances": [a+b for a,b in zip(old.upper,obj.upper)]}, old.n + pair_n)
            staged[name] = merged
        self.streams = staged
        self.H = H
        return self.n

    def bounds(self, alpha: float, analytic_upper: float):
        if self.n <= 0:
            raise ValueError("no hybrid samples available")
        if not math.isfinite(analytic_upper) or not 0 <= analytic_upper <= 1:
            raise ValueError("invalid analytic event upper bound")
        intervals = {}
        for name in self.STREAMS:
            stats = self.streams[name]
            lo, hi = threshold_mean_bounds(stats.lower, stats.upper, stats.n, alpha / 3)
            intervals[name] = (math.nextafter(stats.cap * lo, 0.0),
                               min(1.0, math.nextafter(stats.cap * hi, math.inf)))
        dec_lo, dec_hi = _outward_sum(*intervals["core"], *intervals["residual"])
        base_lo, base_hi = intervals["baseline"]
        lo, hi = max(dec_lo, base_lo), min(dec_hi, base_hi)
        self.conflict = self.conflict or lo > hi
        if lo > hi:
            lo, hi = min(dec_lo, base_lo), max(dec_hi, base_hi)
        if lo > analytic_upper:
            self.conflict = True
            return 0.0, float(analytic_upper)
        return max(0.0, lo), min(float(analytic_upper), hi)

    def diagnostic(self):
        return {name: {"n": s.n, "B": s.cap, "sum_lower": s.sum_lower,
                       "sum_upper": s.sum_upper, "sum_lower_sq": s.sum_lower_sq,
                       "sum_upper_sq": s.sum_upper_sq, "positive_draws": s.positive_draws,
                       "lower_exceedances": s.lower, "upper_exceedances": s.upper}
                for name, s in self.streams.items()}
