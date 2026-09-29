package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"math"
	"os"
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

// Optional fixture regression: saved requests are identified by hash
// so a later snapshot of the same group does not silently change this check.
func TestPointTiltUndecidedReferenceGroups(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to a saved reference-group request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	hash := fmt.Sprintf("%x", sha256.Sum256(data))
	var targetCells []conditionedZeroCell
	seed := "808"
	switch hash {
	case "2d1c1d6f70a64d5b86f5129b32cbf77bedc4c42e464cd849011ca0b3018b3d87":
		targetCells = []conditionedZeroCell{{12, 16}, {95, 4}}
	case "b37c82f317d5f0e412b2642101929cca778857c2792dc89c0fbb830e702e701e":
		targetCells = []conditionedZeroCell{{8, 17}, {74, 18}}
	case "71d4fea8502c3086b8787c764a94b5f8a5caa6549300f2a7687ead8897e11aec":
		// Criciúma's 18th place pilot ranks third among undecided cells at this seed.
		targetCells = []conditionedZeroCell{{73, 17}}
		seed = "817"
	default:
		t.Skip("request differs from the saved reference-group inputs")
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "0")
	t.Setenv("RARE_POSITION_CONDITIONED_POINT_TILT", "1")
	t.Setenv("RARE_POSITION_RANDOM_SEED", seed)
	t.Setenv("RARE_POSITION_POINT_TILT_UNDECIDED", "0")
	baseline := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	t.Setenv("RARE_POSITION_POINT_TILT_UNDECIDED", "1")
	candidate := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	for _, cell := range targetCells {
		before, after := baseline[cell.id][cell.rank], candidate[cell.id][cell.rank]
		if before.Probability != 0 || after.Probability <= 0 ||
			after.Reachability != "witness" || after.ESS < 8 {
			t.Errorf("team=%d rank=%d: baseline=%+v candidate=%+v",
				cell.id, cell.rank+1, before, after)
		}
	}
	for id, ranks := range baseline {
		for rank, before := range ranks {
			after := candidate[id][rank]
			if before.Probability > 0 && after.Probability == 0 {
				t.Errorf("team=%d rank=%d: lost baseline estimate", id, rank+1)
			}
			if before.Probability == 0 && after.Probability == 0 && before.ZeroHitUpper95 > 0 &&
				after.ZeroHitUpper95 > before.ZeroHitUpper95*(1+1e-12) {
				t.Errorf("team=%d rank=%d: upper bound grew from %g to %g",
					id, rank+1, before.ZeroHitUpper95, after.ZeroHitUpper95)
			}
		}
	}
}

func TestConditionedPointTiltCanEstimateUndecidedCell(t *testing.T) {
	group, campaign, table, order, _ := createTestGroupForDiversified()
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	samplers := newConditionedScoreSamplers(group.Games)
	cell := conditionedZeroCell{2, 1}
	newEstimates := func() map[int]map[int]ProductionEstimate {
		return map[int]map[int]ProductionEstimate{
			2: {1: {Reachability: "undecided", ZeroHitUpper95: 0.1}},
		}
	}
	t.Setenv("RARE_POSITION_CONDITIONED_POINT_TILT", "1")
	t.Setenv("RARE_POSITION_POINT_TILT_UNDECIDED", "0")
	disabled := newEstimates()
	witnesses, work := runConditionedPointTiltSearch(group, campaign, table,
		order, 808, bounds, samplers, []conditionedZeroCell{cell}, disabled)
	if witnesses != 0 || work != 0 || disabled[2][1].Probability != 0 {
		t.Fatalf("undecided opt-out sampled the cell: witnesses=%d work=%d estimate=%+v",
			witnesses, work, disabled[2][1])
	}
	t.Setenv("RARE_POSITION_POINT_TILT_UNDECIDED", "1")
	enabled := newEstimates()
	witnesses, work = runConditionedPointTiltSearch(group, campaign, table,
		order, 808, bounds, samplers, []conditionedZeroCell{cell}, enabled)
	est := enabled[2][1]
	if witnesses != 1 || work <= 0 || est.Probability <= 0 ||
		est.Reachability != "witness" || est.Design != "matched_point_pool_conditioned_point_tilt" {
		t.Fatalf("undecided cell did not receive a full-cell estimate: witnesses=%d work=%d estimate=%+v",
			witnesses, work, est)
	}
	t.Setenv("RARE_POSITION_CONDITIONED_POINT_TILT", "0")
	allDisabled := newEstimates()
	witnesses, work = runConditionedPointTiltSearch(group, campaign, table,
		order, 808, bounds, samplers, []conditionedZeroCell{cell}, allDisabled)
	if witnesses != 0 || work != 0 || allDisabled[2][1].Probability != 0 {
		t.Fatalf("point-tilt opt-out sampled the cell: witnesses=%d work=%d estimate=%+v",
			witnesses, work, allDisabled[2][1])
	}
}
