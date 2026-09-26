package main

import (
	"encoding/json"
	"math/rand"
	"os"
	"path/filepath"
	"testing"
)

type pooledBenchmarkTeam struct {
	ID              int             `json:"id"`
	CurrentPoints   int             `json:"current_points"`
	AdditionalPMF   map[int]float64 `json:"additional_pmf"`
	RankCounts      []int           `json:"rank_counts"`
	PointCounts     map[int]int     `json:"point_counts"`
	PointRankCounts map[int][]int   `json:"point_rank_counts"`
}

type pooledBenchmarkRun struct {
	Group        int                   `json:"group"`
	InputSHA256  string                `json:"input_sha256"`
	Seed         int64                 `json:"seed"`
	ScoutSamples int                   `json:"scout_samples"`
	Teams        []pooledBenchmarkTeam `json:"teams"`
}

// Saves sufficient scout statistics for offline point/rank estimators. The
// independent reference never enters the scout or candidate construction.
func TestPooledPointEstimatorBenchmark(t *testing.T) {
	t.Setenv("RARE_POSITION_POINT_GAP", "0")
	paths, out := diversifiedBenchmarkPaths(t)
	seeds := diversifiedBenchmarkIntEnv(t, "RARE_POSITION_BENCHMARK_SEEDS", 5)
	samples := diversifiedBenchmarkIntEnv(t, "RARE_POSITION_POOLED_SCOUT_SAMPLES", 100000)
	firstSeed := diversifiedBenchmarkIntEnv(t, "RARE_POSITION_BENCHMARK_FIRST_SEED", 1001)
	file, err := os.OpenFile(filepath.Join(out, "pooled-scouts.jsonl"), os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0644)
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	encoder := json.NewEncoder(file)
	for _, path := range paths {
		group, hash := diversifiedBenchmarkInput(t, path)
		campaign, table, order := diversifiedBenchmarkSetup(&group)
		unplayed := 0
		for _, game := range group.Games {
			if !game.Played {
				unplayed++
			}
		}
		plainCost := estimateSeasonWork(unplayed, 1, len(group.Team_groups))
		pmfs := make(map[int]map[int]float64, len(group.Team_groups))
		current := make(map[int]int, len(group.Team_groups))
		for _, team := range group.Team_groups {
			universe, ok := pointOutcomeUniverse(team.Team_id, campaign, table, group.Games, 39)
			if !ok {
				t.Fatalf("point universe unavailable for team %d", team.Team_id)
			}
			pmfs[team.Team_id] = additionalPointsPMF(universe)
			current[team.Team_id] = campaign[table.Query(uint32(team.Team_id))].points
		}
		for i := 0; i < seeds; i++ {
			seed := int64(firstSeed + i)
			rng := rand.New(rand.NewSource(deriveRarePositionSeed(seed, "pooled-point-scout")))
			scout := runPlainMCScoutWithJointPoints(campaign, group.Games, table, order,
				group.Team_groups, samples, plainCost, rng)
			run := pooledBenchmarkRun{Group: group.Id, InputSHA256: hash,
				Seed: seed, ScoutSamples: samples}
			for _, team := range group.Team_groups {
				id := team.Team_id
				data := scout.TeamScout[id]
				run.Teams = append(run.Teams, pooledBenchmarkTeam{
					ID: id, CurrentPoints: current[id], AdditionalPMF: pmfs[id],
					RankCounts: data.RankCounts, PointCounts: data.PointCounts,
					PointRankCounts: data.PointRankCounts,
				})
			}
			if err := encoder.Encode(run); err != nil {
				t.Fatal(err)
			}
		}
	}
}
