package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"math"
	"os"
	"testing"
	"time"
)

func TestConditionedWinAwareRankMatchesFullSampling(t *testing.T) {
	group, campaign, table, _, _ := createTestGroupForDiversified()
	order := []SortType{PT, W, GD, GF, BIAS}
	campaign[0].wins, campaign[1].wins, campaign[2].wins = 4, 3, 1
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	samplers := newConditionedScoreSamplers(group.Games)
	t.Setenv("RARE_POSITION_WIN_AWARE_RANK", "1")
	for rank := range group.Team_groups {
		var event *conditionedPointEvent
		target := 0
		for _, team := range group.Team_groups {
			candidate, ok := buildConditionedPointEvent(team.Team_id, rank, nil,
				group, campaign, table, bounds)
			if ok && candidate.mass > 0 {
				event, target = candidate, team.Team_id
				break
			}
		}
		if event == nil {
			t.Fatalf("rank=%d: missing event", rank)
		}
		const draws = 40000
		full := sampleConditionedZeroCell(event, target, rank, group, campaign, table,
			order, samplers, draws, 701+int64(rank))
		fast := sampleConditionedZeroCellFast(event, target, rank, group, campaign, table,
			order, samplers, draws, 907+int64(rank))
		weighted, ok := sampleConditionedZeroRankLookahead(event, target, rank, group,
			campaign, table, order, bounds, samplers, draws, 1103+int64(rank))
		if !ok {
			t.Fatalf("rank=%d: win-aware lookahead unavailable", rank)
		}
		want := event.mass * float64(full.hits) / draws
		got := event.mass * float64(fast.hits) / draws
		if math.Abs(got-want) > 0.02 {
			t.Errorf("rank=%d: fast=%g full=%g", rank, got, want)
		}
		if math.Abs(weighted.probability-want) > 0.02 {
			t.Errorf("rank=%d: lookahead=%g full=%g", rank, weighted.probability, want)
		}
	}
}

func TestWinAwareRankDefaultAndOverride(t *testing.T) {
	group, campaign, table, _, _ := createTestGroupForDiversified()
	order := []SortType{PT, W, GD, GF, BIAS}
	t.Setenv("RARE_POSITION_WIN_AWARE_RANK", "")
	if stride := conditionedRankWinStride(order, group, campaign, table, "lookahead"); stride <= 1 {
		t.Fatalf("default lookahead stride=%d", stride)
	}
	if stride := conditionedRankWinStride(order, group, campaign, table, "screen"); stride != 1 {
		t.Fatalf("default screen stride=%d", stride)
	}
	t.Setenv("RARE_POSITION_WIN_AWARE_RANK", "0")
	if stride := conditionedRankWinStride(order, group, campaign, table, "lookahead"); stride != 1 {
		t.Fatalf("disabled lookahead stride=%d", stride)
	}
	t.Setenv("RARE_POSITION_WIN_AWARE_RANK", "1")
	if stride := conditionedRankWinStride(order, group, campaign, table, "screen"); stride <= 1 {
		t.Fatalf("enabled screen stride=%d", stride)
	}
	order = []SortType{PT, GD, W, GF, BIAS}
	if stride := conditionedRankWinStride(order, group, campaign, table, "lookahead"); stride != 1 {
		t.Fatalf("non-wins sort stride=%d", stride)
	}
}

func TestPairedWinScreenPreservesRankHits(t *testing.T) {
	group, campaign, table, _, _ := createTestGroupForDiversified()
	order := []SortType{PT, W, GD, GF, BIAS}
	campaign[0].wins, campaign[1].wins, campaign[2].wins = 4, 3, 1
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	samplers := newConditionedScoreSamplers(group.Games)
	t.Setenv("RARE_POSITION_PAIRED_SCREEN_RNG", "1")
	for _, team := range group.Team_groups {
		for rank := range group.Team_groups {
			event, ok := buildConditionedPointEvent(team.Team_id, rank, nil,
				group, campaign, table, bounds)
			if !ok || event.mass <= 0 {
				continue
			}
			t.Setenv("RARE_POSITION_WIN_AWARE_RANK", "0")
			point := sampleConditionedZeroCellFast(event, team.Team_id, rank, group,
				campaign, table, order, samplers, 5000, 881)
			t.Setenv("RARE_POSITION_WIN_AWARE_RANK", "screen")
			wins := sampleConditionedZeroCellFast(event, team.Team_id, rank, group,
				campaign, table, order, samplers, 5000, 881)
			if point.hits != wins.hits {
				t.Fatalf("team=%d rank=%d point=%d wins=%d", team.Team_id, rank, point.hits, wins.hits)
			}
		}
	}
}

func TestWinAwareDefaultMatchesExplicitFullRequest(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	previous := log.Writer()
	log.SetOutput(io.Discard)
	defer log.SetOutput(previous)
	t.Setenv("RARE_POSITION_RANDOM_SEED", "811")
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_POINT_TILT", "1")
	t.Setenv("RARE_POSITION_POINT_TILT_UNDECIDED", "1")
	t.Setenv("RARE_POSITION_PAIRED_SCREEN_RNG", "0")
	t.Setenv("RARE_POSITION_WIN_AWARE_RANK", "")
	t.Setenv("RARE_POSITION_WIN_AWARE_STABLE_SELECTION", "")
	t.Setenv("RARE_POSITION_WIN_AWARE_FALLBACK", "")
	defaults := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	t.Setenv("RARE_POSITION_WIN_AWARE_RANK", "lookahead")
	t.Setenv("RARE_POSITION_WIN_AWARE_STABLE_SELECTION", "1")
	t.Setenv("RARE_POSITION_WIN_AWARE_FALLBACK", "1")
	explicit := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	for id, ranks := range defaults {
		for rank, estimate := range ranks {
			other := explicit[id][rank]
			if math.Abs(estimate.Probability-other.Probability) >
				1e-12*math.Max(estimate.Probability, other.Probability) ||
				estimate.WorkSpent != other.WorkSpent {
				t.Fatalf("team=%d rank=%d: default=%g/%d explicit=%g/%d", id, rank+1,
					estimate.Probability, estimate.WorkSpent, other.Probability, other.WorkSpent)
			}
		}
	}
}

// Optional full-request experiment on a saved request. Run once with a fixed
// seed and compare the opt-in win-aware estimator against its current path.
func TestWinAwareRankFullRequestExperiment(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	requestHash := fmt.Sprintf("%x", sha256.Sum256(data))
	if os.Getenv("RARE_POSITION_TRACE_SELECTION") != "1" {
		previous := log.Writer()
		log.SetOutput(io.Discard)
		defer log.SetOutput(previous)
	}
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_POINT_TILT", "1")
	t.Setenv("RARE_POSITION_POINT_TILT_UNDECIDED", "1")
	results := make([]map[int]map[int]ProductionEstimate, 2)
	durations := make([]time.Duration, 2)
	candidateMode := os.Getenv("RARE_POSITION_WIN_AWARE_CANDIDATE")
	if candidateMode == "" {
		candidateMode = "1"
	}
	paired := os.Getenv("RARE_POSITION_PAIRED_SCREEN_CANDIDATE") == "1"
	stable := os.Getenv("RARE_POSITION_STABLE_SELECTION_CANDIDATE") == "1"
	fallback := os.Getenv("RARE_POSITION_WIN_FALLBACK_CANDIDATE") == "1"
	if paired {
		t.Setenv("RARE_POSITION_PAIRED_SCREEN_RNG", "1")
	}
	for index, enabled := range []string{"0", candidateMode} {
		t.Setenv("RARE_POSITION_WIN_AWARE_RANK", enabled)
		if stable && index == 1 {
			t.Setenv("RARE_POSITION_WIN_AWARE_STABLE_SELECTION", "1")
		} else {
			t.Setenv("RARE_POSITION_WIN_AWARE_STABLE_SELECTION", "0")
		}
		if fallback && index == 1 {
			t.Setenv("RARE_POSITION_WIN_AWARE_FALLBACK", "1")
		} else {
			t.Setenv("RARE_POSITION_WIN_AWARE_FALLBACK", "0")
		}
		start := time.Now()
		results[index] = cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
		durations[index] = time.Since(start)
	}
	gained, lost, retained := 0, 0, 0
	for id, ranks := range results[0] {
		for rank, before := range ranks {
			after := results[1][id][rank]
			switch {
			case before.Probability == 0 && after.Probability > 0:
				gained++
				t.Logf("gained team=%d rank=%d p=%g design=%s ess=%g", id, rank+1, after.Probability, after.Design, after.ESS)
			case before.Probability > 0 && after.Probability == 0:
				lost++
				t.Logf("lost team=%d rank=%d p=%g design=%s", id, rank+1, before.Probability, before.Design)
			case before.Probability > 0 && after.Probability > 0:
				retained++
			}
		}
	}
	work := [2]int64{}
	for index, result := range results {
		for _, ranks := range result {
			for _, est := range ranks {
				work[index] = est.WorkSpent
				break
			}
			break
		}
	}
	if path := os.Getenv("RARE_POSITION_BENCHMARK_REFERENCE_JSON"); path != "" {
		data, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		var reference diversifiedReference
		if err := json.Unmarshal(data, &reference); err != nil {
			t.Fatal(err)
		}
		if reference.InputSHA256 != requestHash {
			t.Fatalf("reference input hash %s differs from request %s", reference.InputSHA256, requestHash)
		}
		var nearSquared [2]float64
		nearCount := 0
		for _, cell := range reference.Cells {
			if cell.P < 5e-7 || cell.P >= 2e-6 {
				continue
			}
			for index := range results {
				difference := results[index][cell.Team][cell.Position].Probability - cell.P
				nearSquared[index] += difference * difference
			}
			nearCount++
		}
		if nearCount > 0 {
			t.Logf("near_1e-6_cells=%d rmse_baseline=%g rmse_candidate=%g", nearCount,
				math.Sqrt(nearSquared[0]/float64(nearCount)), math.Sqrt(nearSquared[1]/float64(nearCount)))
		}
	}
	t.Logf("group=%d mode=%s paired=%t stable=%t fallback=%t baseline=%s win_aware=%s work_baseline=%d work_candidate=%d gained=%d lost=%d retained=%d",
		input.Id, candidateMode, paired, stable, fallback,
		durations[0], durations[1], work[0], work[1], gained, lost, retained)
}
