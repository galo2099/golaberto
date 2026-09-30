package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"math/rand"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"testing"
	"time"
)

// Independent plain Monte Carlo for the broad portion of the reference. The
// same four-core ceiling applies; streams and storage are private per worker.
func TestRarePositionReferencePlainMC(t *testing.T) {
	paths, output := os.Getenv("RARE_POSITION_BUDGET_EXPERIMENT_REQUESTS"), os.Getenv("RARE_POSITION_REFERENCE_MC_OUTPUT")
	if paths == "" || output == "" {
		t.Skip("set requests and absolute MC output directory")
	}
	if !filepath.IsAbs(output) {
		t.Fatal("absolute output required")
	}
	if err := os.MkdirAll(output, 0700); err != nil {
		t.Fatal(err)
	}
	previous := runtime.GOMAXPROCS(4)
	defer runtime.GOMAXPROCS(previous)
	n := diversifiedBenchmarkIntEnv(t, "RARE_POSITION_REFERENCE_SAMPLES", 5000000)
	for _, path := range strings.Split(paths, ",") {
		data, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		var input GroupType
		if err := json.Unmarshal(data, &input); err != nil {
			t.Fatal(err)
		}
		hash := fmt.Sprintf("%x", sha256.Sum256(data))
		started := time.Now()
		partials := make([]map[int][]int, 4)
		var wait sync.WaitGroup
		for worker := 0; worker < 4; worker++ {
			wait.Add(1)
			go func(worker int) {
				defer wait.Done()
				group := cloneGroupForBenchmark(input)
				campaign, table, order := diversifiedBenchmarkSetup(group)
				samples := n / 4
				if worker < n%4 {
					samples++
				}
				stream := deriveRarePositionSeed(947117, fmt.Sprintf("reference-mc-%s-worker-%d", hash, worker))
				partials[worker] = simulatePlainRankCounts(campaign, group.Games, table, order, group.Team_groups, samples, rand.New(rand.NewSource(stream)))
			}(worker)
		}
		wait.Wait()
		ref := diversifiedReference{GroupID: input.Id, InputSHA256: hash, Samples: n, Seed: 947117}
		for _, tg := range input.Team_groups {
			for rank := range input.Team_groups {
				count := 0
				for _, part := range partials {
					count += part[tg.Team_id][rank]
				}
				ref.Cells = append(ref.Cells, diversifiedRefCell{tg.Team_id, rank + 1, count, float64(count) / float64(n)})
			}
		}
		encoded, err := json.MarshalIndent(ref, "", "  ")
		if err != nil {
			t.Fatal(err)
		}
		name := fmt.Sprintf("mc-%d-%s.json", input.Id, hash[:8])
		if err := os.WriteFile(filepath.Join(output, name), encoded, 0600); err != nil {
			t.Fatal(err)
		}
		t.Logf("group=%d sha=%s samples=%d elapsed=%s", input.Id, hash[:8], n, time.Since(started))
	}
}
