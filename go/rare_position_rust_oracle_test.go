package main

import (
	"encoding/json"
	"fmt"
	"math/rand"
	"os"
	"testing"
	"time"
)

// Offline differential oracle; the Rust binary never invokes Go.
func TestRustEstimatorOracle(t *testing.T) {
	path, output := os.Getenv("RUST_ODDS_ORACLE_REQUEST"), os.Getenv("RUST_ODDS_ORACLE_OUTPUT")
	if path == "" || output == "" {
		t.Skip("optional Rust differential oracle")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	campaign, table, order := diversifiedBenchmarkSetup(&input)
	seed := int64(808)
	rng := rand.New(rand.NewSource(seed))
	floats := make([]float64, 1000)
	for i := range floats {
		floats[i] = rng.Float64()
	}
	scout := runPlainMCScoutWithJointPointsBatched(campaign, input.Games, table, order, input.Team_groups,
		1000, 1, rand.New(rand.NewSource(seed)), 0, false)
	pmfs := make(map[int]map[int]float64)
	for _, team := range input.Team_groups {
		universe, ok := pointOutcomeUniverse(team.Team_id, campaign, table, input.Games, 39)
		if ok {
			pmfs[team.Team_id] = additionalPointsPMF(universe)
		}
	}
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO", "0")
	estimates := runMatchedPointPoolProduction(&input, campaign, table, order, seed)
	bounds := buildPointRankBounds(campaign, input.Team_groups, input.Games, table)
	kernels := make(map[string]interface{})
	for _, team := range input.Team_groups {
		for _, rank := range []int{0, 1, 2, 17, 18, 19} {
			if rank >= len(input.Team_groups) {
				continue
			}
			event, ok := buildConditionedPointEvent(team.Team_id, rank, nil, &input, campaign, table, bounds)
			if !ok || event.mass <= 0 {
				continue
			}
			samplers := newConditionedScoreSamplers(input.Games)
			for _, force := range []bool{false, true} {
				for _, domains := range []bool{false, true} {
					result, _ := sampleConditionedZeroRankLookaheadPolicy(event, team.Team_id, rank, &input, campaign, table, order, bounds, samplers, 200, 808, 3, 0.5, force, domains)
					key := fmt.Sprintf("%d-%d-%t-%t", team.Team_id, rank, force, domains)
					kernels[key] = map[string]interface{}{"mass": result.mass, "hits": result.hits, "samples": result.samples, "probability": result.probability, "std_err": result.stdErr, "ess": result.ess, "max_share": result.maxWeightShare, "batch_gap": result.batchGap, "omitted_draws": result.omittedDraws}
				}
			}
		}
	}
	encoded, err := json.Marshal(map[string]interface{}{"floats": floats, "scout": scout.TeamScout, "pmfs": pmfs, "rare_position_estimates": estimates, "kernels": kernels})
	if err != nil {
		t.Fatal(err)
	}
	if err = os.WriteFile(output, encoded, 0600); err != nil {
		t.Fatal(err)
	}
}

func TestRustEstimatorFullBaseline(t *testing.T) {
	path, output := os.Getenv("RUST_ODDS_ORACLE_REQUEST"), os.Getenv("RUST_ODDS_ORACLE_OUTPUT")
	if path == "" || output == "" {
		t.Skip("optional Rust full-request baseline")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var input GroupType
	if err = json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "0")
	t.Setenv("RARE_POSITION_BENCHMARK_ITERATIONS", "20000")
	if os.Getenv("RARE_POSITION_RANDOM_SEED") == "" {
		t.Setenv("RARE_POSITION_RANDOM_SEED", "808")
	}
	start := time.Now()
	result := input.calculate_odds()
	elapsed := time.Since(start).Seconds() * 1000
	encoded, err := json.Marshal(result)
	if err != nil {
		t.Fatal(err)
	}
	if err = os.WriteFile(output, encoded, 0600); err != nil {
		t.Fatal(err)
	}
	fmt.Printf("rust-comparison-go elapsed_ms=%f\n", elapsed)
}
