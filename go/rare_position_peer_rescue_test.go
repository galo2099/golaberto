package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
	"testing"
)

func TestDirectionalPeerSupportsBothDirections(t *testing.T) {
	if !directionalPeerSupports(5, 4, 18) {
		t.Fatal("a currently higher peer should flag a lower finishing rank")
	}
	if !directionalPeerSupports(5, 6, 0) {
		t.Fatal("a currently lower peer should flag a higher finishing rank")
	}
	if directionalPeerSupports(5, 6, 18) || directionalPeerSupports(5, 4, 0) ||
		directionalPeerSupports(5, 4, 5) {
		t.Fatal("peers in the wrong direction should not flag the target")
	}
}

func TestDirectionalPeerRescueSavedGroup16498(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to the saved group 16498 request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if fmt.Sprintf("%x", sha256.Sum256(data)) != "44eabb47ee55477c77025c3de4e839c6ea913b8569dd29694d7178bdb8bcdc89" {
		t.Skip("request differs from the saved group 16498 input")
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	t.Setenv("RARE_POSITION_RANDOM_SEED", "804")
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "0")
	t.Setenv("RARE_POSITION_BENCHMARK_ITERATIONS", "20000")
	t.Setenv("RARE_POSITION_DIRECTIONAL_PEER_RESCUE", "0")
	baseline := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	t.Setenv("RARE_POSITION_DIRECTIONAL_PEER_RESCUE", "1")
	rescued := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	if baseline[74][18].Probability <= 0 || baseline[15][18].Probability != 0 {
		t.Fatalf("unexpected starting cells: Bahia=%g Cruzeiro=%g",
			baseline[74][18].Probability, baseline[15][18].Probability)
	}
	if p := rescued[15][18].Probability; p < 1e-13 || p > 1e-11 ||
		rescued[15][18].Design != "matched_point_pool_conditioned_point_tilt_peer" {
		t.Fatalf("Cruzeiro 19th rescue=%+v", rescued[15][18])
	}
	for team, row := range baseline {
		for rank, before := range row {
			if before.Probability > 0 && rescued[team][rank].Probability <= 0 {
				t.Errorf("team=%d rank=%d lost an estimate", team, rank+1)
			}
		}
	}
}
