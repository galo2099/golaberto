package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"math"
	"os"
	"slices"
	"testing"
)

func TestPointTiltConfirmationSpilloverPreservesQuotas(t *testing.T) {
	tests := []struct {
		name             string
		candidateCount   int
		proved           []int
		includeUndecided bool
		want             []int
	}{
		{
			name:             "full quotas do not change",
			candidateCount:   9,
			proved:           []int{2, 4, 6, 8, 9},
			includeUndecided: true,
			want:             []int{1, 2, 3, 4, 5, 6, 8},
		},
		{
			name:             "unused proved slot goes to next undecided pilot",
			candidateCount:   9,
			proved:           []int{6, 7, 8},
			includeUndecided: true,
			want:             []int{1, 2, 3, 6, 7, 8, 4},
		},
		{
			name:             "unused undecided slot goes to next proved pilot",
			candidateCount:   7,
			proved:           []int{1, 2, 3, 4, 5},
			includeUndecided: true,
			want:             []int{1, 2, 3, 4, 6, 7, 5},
		},
		{
			name:             "undecided opt-out is respected",
			candidateCount:   6,
			proved:           []int{2, 4, 6},
			includeUndecided: false,
			want:             []int{2, 4, 6},
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			proved := make(map[int]bool, len(test.proved))
			for _, index := range test.proved {
				proved[index] = true
			}
			candidates := make([]conditionedPointTiltCandidate, test.candidateCount)
			for i := range candidates {
				candidates[i] = conditionedPointTiltCandidate{index: i + 1, proved: proved[i+1]}
			}
			selected := selectConditionedPointTiltFinalists(candidates, test.includeUndecided)
			got := make([]int, len(selected))
			for i, candidate := range selected {
				got[i] = candidate.index
			}
			if !slices.Equal(got, test.want) {
				t.Fatalf("selected %v, want %v", got, test.want)
			}
		})
	}
}

func TestPointTiltSpilloverSavedGroup16653(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to the saved group 16653 request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if fmt.Sprintf("%x", sha256.Sum256(data)) != "2d1c1d6f70a64d5b86f5129b32cbf77bedc4c42e464cd849011ca0b3018b3d87" {
		t.Skip("request differs from the saved group 16653 input")
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	t.Setenv("RARE_POSITION_RANDOM_SEED", "809")
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "0")
	estimates := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	for _, cell := range []conditionedZeroCell{{68, 0}, {95, 3}} {
		est := estimates[cell.id][cell.rank]
		if est.Probability <= 0 || est.Reachability != "witness" {
			t.Errorf("team=%d rank=%d: expected retained or spillover estimate, got %+v",
				cell.id, cell.rank+1, est)
		}
	}
}

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

func TestPointTiltLowEvidenceFinalReferenceGroup(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to the saved live group 16653 request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if fmt.Sprintf("%x", sha256.Sum256(data)) != "71d4fea8502c3086b8787c764a94b5f8a5caa6549300f2a7687ead8897e11aec" {
		t.Skip("request differs from the saved live group 16653 input")
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	t.Setenv("RARE_POSITION_RANDOM_SEED", "1790644074581708000")
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_POINT_TILT", "1")
	t.Setenv("RARE_POSITION_POINT_TILT_UNDECIDED", "1")
	estimates := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	avaiSecond := estimates[68][1]
	if avaiSecond.Probability <= 0 || avaiSecond.ConditionalSamples != conditionedPointTiltLowEvidenceSamples ||
		avaiSecond.Design != "matched_point_pool_conditioned_point_tilt" || avaiSecond.Reachability != "witness" {
		t.Fatalf("Avaí second-place estimate: %+v", avaiSecond)
	}
}

func TestPointTiltProvedNeighborGapReferenceGroup(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to a saved group 16653 request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var seed string
	var rank int
	switch fmt.Sprintf("%x", sha256.Sum256(data)) {
	case "71d4fea8502c3086b8787c764a94b5f8a5caa6549300f2a7687ead8897e11aec":
		seed, rank = "816", 1 // Avaí second on the live request.
	case "2d1c1d6f70a64d5b86f5129b32cbf77bedc4c42e464cd849011ca0b3018b3d87":
		seed, rank = "810", 2 // Avaí third on the earlier reference request.
	default:
		t.Skip("request differs from the saved group 16653 inputs")
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	t.Setenv("RARE_POSITION_RANDOM_SEED", seed)
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_POINT_TILT", "1")
	t.Setenv("RARE_POSITION_POINT_TILT_UNDECIDED", "1")
	t.Setenv("RARE_POSITION_POINT_TILT_GAP_RESCUE", "0")
	baseline := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	if baseline[68][rank].Probability != 0 || baseline[68][rank].Reachability != "reachable_by_construction" ||
		baseline[68][rank-1].Probability <= 0 || baseline[68][rank+1].Probability <= 0 {
		t.Fatalf("expected a proved Avaí gap at rank %d: %+v", rank+1, baseline[68])
	}
	t.Setenv("RARE_POSITION_POINT_TILT_GAP_RESCUE", "1")
	rescued := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	est := rescued[68][rank]
	if est.Probability <= 0 || est.Design != "matched_point_pool_conditioned_point_tilt_gap" ||
		est.ConditionalSamples < conditionedPointTiltFinalSamples ||
		est.ConditionalSamples > conditionedPointTiltGapSamples || est.Reachability != "witness" ||
		est.ESS < 8 || est.MaxEventWeightShare > 0.25 {
		t.Fatalf("Avaí rank %d gap rescue: %+v", rank+1, est)
	}
}

func TestPointTiltSharesProvedGapBudgetOnSavedRequests(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to a saved group request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var seed string
	var gained conditionedZeroCell
	var retained []conditionedZeroCell
	switch fmt.Sprintf("%x", sha256.Sum256(data)) {
	case "2d1c1d6f70a64d5b86f5129b32cbf77bedc4c42e464cd849011ca0b3018b3d87":
		seed = "816"
		gained = conditionedZeroCell{68, 1} // Avaí second, after team 457 second.
		retained = []conditionedZeroCell{{457, 1}}
	case "b37c82f317d5f0e412b2642101929cca778857c2792dc89c0fbb830e702e701e":
		seed = "825"
		gained = conditionedZeroCell{74, 18}
	default:
		t.Skip("request differs from the saved group inputs")
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	t.Setenv("RARE_POSITION_RANDOM_SEED", seed)
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_POINT_TILT", "1")
	t.Setenv("RARE_POSITION_POINT_TILT_UNDECIDED", "1")
	t.Setenv("RARE_POSITION_POINT_TILT_GAP_RESCUE", "1")
	t.Setenv("RARE_POSITION_POINT_TILT_UNDECIDED_GAP", "1")
	estimates := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	for _, cell := range append(retained, gained) {
		est := estimates[cell.id][cell.rank]
		if est.Probability <= 0 || est.Reachability != "witness" ||
			est.Design != "matched_point_pool_conditioned_point_tilt_gap" ||
			est.ConditionalSamples < conditionedPointTiltFinalSamples ||
			est.ConditionalSamples > conditionedPointTiltGapSamples {
			t.Errorf("team=%d rank=%d: %+v", cell.id, cell.rank+1, est)
		}
	}
}

func TestPointTiltUndecidedNeighborGapNeedsSampledWitness(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to the saved group 16498 request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if fmt.Sprintf("%x", sha256.Sum256(data)) != "b37c82f317d5f0e412b2642101929cca778857c2792dc89c0fbb830e702e701e" {
		t.Skip("request differs from the saved group 16498 input")
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	t.Setenv("RARE_POSITION_RANDOM_SEED", "1790644074581708000")
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_POINT_TILT", "1")
	t.Setenv("RARE_POSITION_POINT_TILT_GAP_RESCUE", "1")
	t.Setenv("RARE_POSITION_POINT_TILT_UNDECIDED_GAP", "0")
	baseline := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	if baseline[110][2].Probability <= 0 || baseline[110][3].Probability != 0 ||
		baseline[110][4].Probability <= 0 || baseline[110][3].Reachability != "undecided" {
		t.Fatalf("expected undecided fourth-place gap: %+v", baseline[110][3])
	}
	t.Setenv("RARE_POSITION_POINT_TILT_UNDECIDED_GAP", "")
	rescued := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	est := rescued[110][3]
	if est.Probability <= 0 || est.Reachability != "witness" ||
		est.Design != "matched_point_pool_conditioned_point_tilt_undecided_gap" ||
		est.ConditionalSamples != conditionedPointTiltUndecidedGapSamples || est.ESS < 50 {
		t.Fatalf("team 110 fourth-place gap rescue: %+v", est)
	}
	for id, ranks := range baseline {
		for rank, before := range ranks {
			if before.Probability > 0 && rescued[id][rank].Probability == 0 {
				t.Errorf("team=%d rank=%d lost a positive estimate", id, rank+1)
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
		order, 808, bounds, samplers, []conditionedZeroCell{cell}, disabled, 0)
	if witnesses != 0 || work != 0 || disabled[2][1].Probability != 0 {
		t.Fatalf("undecided opt-out sampled the cell: witnesses=%d work=%d estimate=%+v",
			witnesses, work, disabled[2][1])
	}
	t.Setenv("RARE_POSITION_POINT_TILT_UNDECIDED", "1")
	enabled := newEstimates()
	witnesses, work = runConditionedPointTiltSearch(group, campaign, table,
		order, 808, bounds, samplers, []conditionedZeroCell{cell}, enabled, 0)
	est := enabled[2][1]
	if witnesses != 1 || work <= 0 || est.Probability <= 0 ||
		est.Reachability != "witness" || est.Design != "matched_point_pool_conditioned_point_tilt" {
		t.Fatalf("undecided cell did not receive a full-cell estimate: witnesses=%d work=%d estimate=%+v",
			witnesses, work, est)
	}
	t.Setenv("RARE_POSITION_CONDITIONED_POINT_TILT", "0")
	allDisabled := newEstimates()
	witnesses, work = runConditionedPointTiltSearch(group, campaign, table,
		order, 808, bounds, samplers, []conditionedZeroCell{cell}, allDisabled, 0)
	if witnesses != 0 || work != 0 || allDisabled[2][1].Probability != 0 {
		t.Fatalf("point-tilt opt-out sampled the cell: witnesses=%d work=%d estimate=%+v",
			witnesses, work, allDisabled[2][1])
	}
}

func TestPointTiltRecycledProofBudgetPreservesSpilloverEstimate(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to the saved live group 16653 request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if fmt.Sprintf("%x", sha256.Sum256(data)) != "b87207393a283ae4b7cc986c49a60fdd0943057295872ddcda27926d9d6597dd" {
		t.Skip("request differs from the saved live group 16653 input")
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	t.Setenv("RARE_POSITION_RANDOM_SEED", "808")
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "0")
	t.Setenv("RARE_POSITION_RECYCLE_PROOF_WORK", "0")
	baseline := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	t.Setenv("RARE_POSITION_RECYCLE_PROOF_WORK", "1")
	recycled := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	before, cell := baseline[12][16], recycled[12][16]
	if before.Probability <= 0 || cell.Probability <= 0 ||
		cell.Reachability != "witness" || before.Design != "matched_point_pool_conditioned_point_tilt" ||
		cell.Design != before.Design || cell.ConditionalSamples != conditionedPointTiltFinalSamples || cell.ESS < 8 {
		t.Fatalf("team 12 finishing 17th: baseline=%+v recycled=%+v", baseline[12][16], cell)
	}
	for id, ranks := range baseline {
		for rank, before := range ranks {
			if before.Probability > 0 && recycled[id][rank].Probability == 0 {
				t.Errorf("team=%d rank=%d: lost baseline estimate", id, rank+1)
			}
		}
	}
	if got := recycled[12][16].WorkSpent - baseline[12][16].WorkSpent; got != 2000*105 {
		t.Fatalf("unexpected recycled work: %d", got)
	}
}
