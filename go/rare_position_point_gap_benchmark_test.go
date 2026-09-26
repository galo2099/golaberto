package main

import (
	"encoding/json"
	"fmt"
	"math/rand"
	"os"
	"path/filepath"
	"testing"
)

type pointGapBenchCell struct {
	Team             int                `json:"team"`
	Position         int                `json:"position"`
	ReferenceP       float64            `json:"reference_probability"`
	ReferenceCount   int                `json:"reference_count"`
	GapEstimate      float64            `json:"gap_estimate"`
	GapSupport       float64            `json:"gap_support"`
	ObservedEstimate float64            `json:"observed_estimate"`
	ObservedSupport  float64            `json:"observed_support"`
	TeamOnlyEstimate float64            `json:"team_only_estimate"`
	TeamOnlySupport  float64            `json:"team_only_support"`
	Smoothed         map[string]float64 `json:"smoothed"`
}

type pointGapBenchRun struct {
	Group            int                 `json:"group"`
	Seed             int64               `json:"seed"`
	ScoutSamples     int                 `json:"scout_samples"`
	GapObservations  int64               `json:"gap_observations"`
	ReferenceSamples int                 `json:"reference_samples"`
	Cells            []pointGapBenchCell `json:"cells"`
}

// Offline diagnostic: compare group-pooled standing gaps with exact-point
// pooling and team-only pooling against the independent 5M-season reference.
func TestPointGapScoutBenchmark(t *testing.T) {
	t.Setenv("RARE_POSITION_POINT_GAP", "1")
	paths, out := diversifiedBenchmarkPaths(t)
	seeds := diversifiedBenchmarkIntEnv(t, "RARE_POSITION_BENCHMARK_SEEDS", 5)
	samples := diversifiedBenchmarkIntEnv(t, "RARE_POSITION_GAP_SCOUT_SAMPLES", 1000)
	resultPath := filepath.Join(out, "point-gap-runs.jsonl")
	file, err := os.OpenFile(resultPath, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0644)
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	encoder := json.NewEncoder(file)
	for _, path := range paths {
		group, hash := diversifiedBenchmarkInput(t, path)
		data, err := os.ReadFile(diversifiedReferencePath(diversifiedReferenceDirectory(out), group.Id))
		if err != nil {
			t.Fatal(err)
		}
		var ref diversifiedReference
		if err := json.Unmarshal(data, &ref); err != nil {
			t.Fatal(err)
		}
		if ref.InputSHA256 != hash || ref.GroupID != group.Id || ref.Samples < 5000000 {
			t.Fatalf("missing or stale reference for group %d", group.Id)
		}
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
				t.Fatalf("point outcome universe unavailable for team %d", team.Team_id)
			}
			pmfs[team.Team_id] = additionalPointsPMF(universe)
			current[team.Team_id] = campaign[table.Query(uint32(team.Team_id))].points
		}
		for i := 0; i < seeds; i++ {
			seed := int64(1001 + i)
			scout := runPlainMCScoutWithJointPoints(campaign, group.Games, table, order, group.Team_groups,
				samples, plainCost, rand.New(rand.NewSource(deriveRarePositionSeed(seed, "adaptive-scout"))))
			if scout.PointGaps == nil {
				t.Fatalf("point-gap scout unavailable for group %d; points must lead the sort order", group.Id)
			}
			observed := newPointGapScout(scout.PointGaps.MinPoints, scout.PointGaps.MaxPoints, len(group.Team_groups))
			for _, team := range group.Team_groups {
				teamScout := scout.TeamScout[team.Team_id]
				for added, ranks := range teamScout.PointRankCounts {
					finalPoints := current[team.Team_id] + added
					for rank, count := range ranks {
						for hit := 0; hit < count; hit++ {
							observed.add(finalPoints, rank)
						}
					}
				}
			}
			run := pointGapBenchRun{Group: group.Id, Seed: seed, ScoutSamples: samples,
				GapObservations: scout.PointGaps.Observations, ReferenceSamples: ref.Samples}
			for _, cell := range ref.Cells {
				pmf := pmfs[cell.Team]
				currentPoints := current[cell.Team]
				gapEstimate, gapSupport := pointGapEstimate(pmf, currentPoints, cell.Position, scout.PointGaps)
				observedEstimate, observedSupport := pointGapEstimate(pmf, currentPoints, cell.Position, observed)
				teamOnlyEstimate, teamOnlySupport := 0.0, 0.0
				teamScout := scout.TeamScout[cell.Team]
				for added, mass := range pmf {
					count := teamScout.PointCounts[added]
					if mass <= 0 || count <= 0 {
						continue
					}
					teamOnlyEstimate += mass * float64(teamScout.PointRankCounts[added][cell.Position]) / float64(count)
					teamOnlySupport += mass
				}
				smoothed := make(map[string]float64)
				for _, strength := range []float64{0, 1, 5, 20, 100} {
					value, _ := pointGapSmoothedEstimate(pmf, currentPoints, cell.Position, scout.PointGaps, strength)
					smoothed[fmt.Sprint(strength)] = value
				}
				run.Cells = append(run.Cells, pointGapBenchCell{
					Team: cell.Team, Position: cell.Position, ReferenceP: cell.P, ReferenceCount: cell.Count,
					GapEstimate: gapEstimate, GapSupport: gapSupport, ObservedEstimate: observedEstimate,
					ObservedSupport: observedSupport, TeamOnlyEstimate: teamOnlyEstimate,
					TeamOnlySupport: teamOnlySupport, Smoothed: smoothed,
				})
			}
			if err := encoder.Encode(run); err != nil {
				t.Fatal(err)
			}
		}
		t.Logf("point-gap benchmark complete for group %d: %s", group.Id, fmt.Sprint(resultPath))
	}
}
