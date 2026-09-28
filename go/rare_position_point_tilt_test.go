package main

import (
	"math"
	"testing"
)

func TestConditionedPointTiltCoversTerminalsAndMatchesDirectSampling(t *testing.T) {
	group, campaign, table, order, _ := createTestGroupForDiversified()
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	event, ok := buildConditionedPointEvent(2, 1, nil, group, campaign, table, bounds)
	if !ok || event.mass <= 0 {
		t.Fatal("could not construct the target's point event")
	}
	cdf, ratios := conditionedPointTiltTerminals(event, 0.8)
	var reweighted float64
	for i, terminal := range event.terminal {
		q := cdf[i]
		p := terminal.cumulative
		if i > 0 {
			q -= cdf[i-1]
			p -= event.terminal[i-1].cumulative
		}
		if p > 0 && (q <= 0 || ratios[i] <= 0) {
			t.Fatalf("terminal %d lost support: p=%g q=%g ratio=%g", i, p, q, ratios[i])
		}
		if math.Abs(q*ratios[i]-p/event.mass) > 1e-12 {
			t.Fatalf("terminal %d has incorrect likelihood ratio", i)
		}
		reweighted += q * ratios[i]
	}
	if math.Abs(reweighted-1) > 1e-12 {
		t.Fatalf("reweighted terminal mass=%g", reweighted)
	}
	samplers := newConditionedScoreSamplers(group.Games)
	const samples = 100000
	direct := sampleConditionedZeroCellFast(event, 2, 1, group, campaign,
		table, order, samplers, samples, 2701)
	weighted, _ := sampleConditionedZeroRankLookaheadWithPointTilt(event, 2, 1,
		group, campaign, table, order, bounds, samplers, samples, 3812, 3, 0.8)
	directP := event.mass * float64(direct.hits) / samples
	directSE := event.mass * math.Sqrt(float64(direct.hits)/samples*(1-float64(direct.hits)/samples)/samples)
	if !weighted.weighted || math.Abs(weighted.probability-directP) >
		5*math.Hypot(weighted.stdErr, directSE) {
		t.Fatalf("point tilt=%g ± %g, direct=%g ± %g",
			weighted.probability, weighted.stdErr, directP, directSE)
	}
}
