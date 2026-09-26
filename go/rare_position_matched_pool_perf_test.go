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
