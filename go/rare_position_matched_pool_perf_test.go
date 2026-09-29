package main

import (
	"encoding/json"
	"io"
	"log"
	"os"
	"testing"
)

// BenchmarkMatchedPointPoolFullRequest measures the initial scout and the
// matched pool together using a request JSON captured from a real group.
func BenchmarkMatchedPointPoolFullRequest(b *testing.B) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		b.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to a saved group request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		b.Fatal(err)
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		b.Fatal(err)
	}
	b.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	b.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "0")
	b.Setenv("RARE_POSITION_BENCHMARK_ITERATIONS", "20000")
	b.Setenv("RARE_POSITION_RANDOM_SEED", "808")
	previous := log.Writer()
	log.SetOutput(io.Discard)
	defer log.SetOutput(previous)
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = cloneGroupForBenchmark(input).calculate_odds()
	}
}

// Optional fixed-seed snapshot for checking estimator output across
// performance changes. It uses the same configuration as the benchmark.
func TestMatchedPointPoolFullRequestSnapshot(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	output := os.Getenv("RARE_POSITION_BENCHMARK_SNAPSHOT_JSON")
	if path == "" || output == "" {
		t.Skip("set request and snapshot paths")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "0")
	t.Setenv("RARE_POSITION_BENCHMARK_ITERATIONS", "20000")
	if os.Getenv("RARE_POSITION_RANDOM_SEED") == "" {
		t.Setenv("RARE_POSITION_RANDOM_SEED", "808")
	}
	previous := log.Writer()
	log.SetOutput(io.Discard)
	defer log.SetOutput(previous)
	estimates := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"]
	encoded, err := json.Marshal(estimates)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(output, encoded, 0600); err != nil {
		t.Fatal(err)
	}
}
