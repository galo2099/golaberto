package main

import (
	"encoding/json"
	"fmt"
	"math"
	"math/rand"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestMatchedPointPoolBatchesPreserveScout(t *testing.T) {
	t.Setenv("RARE_POSITION_POINT_GAP", "0")
	group, campaign, table, order, _ := createTestGroupForDiversified()
	a := runPlainMCScoutWithJointPoints(campaign, group.Games, table, order,
		group.Team_groups, 1000, 6, rand.New(rand.NewSource(17)))
	b := runPlainMCScoutWithJointPointsBatched(campaign, group.Games, table, order,
		group.Team_groups, 1000, 6, rand.New(rand.NewSource(17)), 10, false)
	if !reflect.DeepEqual(a.TeamScout, b.TeamScout) {
		t.Fatal("batch recording changed the scout")
	}
	for _, team := range group.Team_groups {
		id := team.Team_id
		count := 0
		for _, batch := range b.PointRankBatches {
			count += batch[id].Samples
		}
		if count != 1000 {
			t.Fatalf("team %d: batch samples=%d", id, count)
		}
		for rank, full := range b.TeamScout[id].RankCounts {
			sum := 0
			for _, batch := range b.PointRankBatches {
				sum += batch[id].RankCounts[rank]
			}
			if sum != full {
				t.Fatalf("team %d rank %d: batches=%d full=%d", id, rank, sum, full)
			}
		}
	}
}

func TestMatchedPointPoolParallelBatchesPreserveCounts(t *testing.T) {
	group, campaign, table, order, _ := createTestGroupForDiversified()
	const samples = 1003
	scout := runMatchedPointPoolScoutBatches(campaign, group.Games, table, order,
		group.Team_groups, samples, 6, rand.New(rand.NewSource(17)), 4)
	if scout.Samples != samples || len(scout.PointRankBatches) != 10 {
		t.Fatalf("parallel scout samples=%d batches=%d", scout.Samples, len(scout.PointRankBatches))
	}
	for _, team := range group.Team_groups {
		id := team.Team_id
		full := scout.TeamScout[id]
		if full.Samples != samples || !reflect.DeepEqual(full.RankCounts, scout.TeamCounts[id]) {
			t.Fatalf("team %d: incomplete aggregate scout", id)
		}
		for rank, count := range full.RankCounts {
			sum := 0
			for _, batch := range scout.PointRankBatches {
				sum += batch[id].RankCounts[rank]
			}
			if sum != count {
				t.Fatalf("team %d rank %d: batches=%d aggregate=%d", id, rank, sum, count)
			}
		}
		for added, count := range full.PointCounts {
			sum := 0
			for _, batch := range scout.PointRankBatches {
				sum += batch[id].PointCounts[added]
			}
			if sum != count {
				t.Fatalf("team %d points %d: batches=%d aggregate=%d", id, added, sum, count)
			}
			for rank, hits := range full.PointRankCounts[added] {
				rankSum := 0
				for _, batch := range scout.PointRankBatches {
					if ranks := batch[id].PointRankCounts[added]; rank < len(ranks) {
						rankSum += ranks[rank]
					}
				}
				if rankSum != hits {
					t.Fatalf("team %d points %d rank %d: batches=%d aggregate=%d", id, added, rank, rankSum, hits)
				}
			}
		}
	}
}

func TestMatchedPointPoolProductionMatrix(t *testing.T) {
	t.Setenv("RARE_POSITION_POINT_GAP", "0")
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	group, campaign, table, order, _ := createTestGroupForDiversified()
	estimates := runMatchedPointPoolProduction(group, campaign, table, order, 1001)
	if len(estimates) != 3 {
		t.Fatalf("teams=%d", len(estimates))
	}
	for _, team := range group.Team_groups {
		id := team.Team_id
		row := 0.0
		for rank := range group.Team_groups {
			est := estimates[id][rank]
			if est.Design != "matched_point_pool" || !est.Available || est.MeetsPrecisionGoal || est.Samples != matchedPointPoolSamples {
				t.Fatalf("team=%d rank=%d estimate=%+v", id, rank, est)
			}
			if est.StdErr < 0 || math.IsNaN(est.StdErr) {
				t.Fatalf("invalid std err: %+v", est)
			}
			row += est.Probability
		}
		if math.Abs(row-1) > 1e-6 {
			t.Fatalf("team %d row sum=%g", id, row)
		}
	}
	for rank := range group.Team_groups {
		column := 0.0
		for _, team := range group.Team_groups {
			column += estimates[team.Team_id][rank].Probability
		}
		if math.Abs(column-1) > 1e-6 {
			t.Fatalf("rank %d column sum=%g", rank, column)
		}
	}
	if estimates[3][0].Probability != 0 {
		t.Fatal("points-impossible rank received probability")
	}
	if estimates[3][0].ZeroHitUpper95 != 0 {
		t.Fatal("points-impossible rank retained a nonzero upper bound")
	}
}

func TestMatchedPointPoolUnsupportedStandingsFallBack(t *testing.T) {
	group, campaign, table, _, _ := createTestGroupForDiversified()
	estimates := runMatchedPointPoolProduction(group, campaign, table, []SortType{GF, PT, BIAS}, 1001)
	for _, cells := range estimates {
		for _, est := range cells {
			if est.Design != "plain_mc" || est.Samples != matchedPointPoolSamples {
				t.Fatalf("unsupported standings estimate=%+v", est)
			}
		}
	}
}

func TestMatchedPointPoolReplacesScoutMatrix(t *testing.T) {
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_RANDOM_SEED", "1001")
	group, campaign, table, order, _ := createTestGroupForDiversified()
	teamOdds := make([]OddsType, len(group.Team_groups))
	counts := make(map[int][]int)
	for _, team := range group.Team_groups {
		teamOdds[table.Query(uint32(team.Team_id))] = OddsType{team: &TeamOdds{Pos: []float64{1, 0, 0}}}
		counts[team.Team_id] = []int{1000, 0, 0}
	}
	estimates := searchAndMergeRarePositions(group, campaign, table, order, counts, teamOdds, 1000)
	for _, team := range group.Team_groups {
		id := team.Team_id
		for rank, got := range teamOdds[table.Query(uint32(id))].team.Pos {
			if got != estimates[id][rank].Probability {
				t.Fatalf("team %d rank %d: merged=%g estimate=%g", id, rank, got, estimates[id][rank].Probability)
			}
		}
	}
}

// Optional real-fixture regression check. The reference files are never used
// by the estimator; the selection reference fixes the cells before scoring.
func TestMatchedPointPoolReferenceBenchmark(t *testing.T) {
	paths, _ := diversifiedBenchmarkPaths(t)
	selectionDir := os.Getenv("RARE_POSITION_MATCHED_SELECTION_REFERENCE_DIR")
	holdoutDir := os.Getenv("RARE_POSITION_MATCHED_HOLDOUT_REFERENCE_DIR")
	if selectionDir == "" || holdoutDir == "" {
		t.Skip("set matched selection and holdout reference directories")
	}
	t.Setenv("RARE_POSITION_POINT_GAP", "0")
	seeds := diversifiedBenchmarkIntEnv(t, "RARE_POSITION_BENCHMARK_SEEDS", 3)
	var nearPlain, nearPooled, allPlain, allPooled float64
	var nearCount, allCount, fallback int
	for _, path := range paths {
		group, hash := diversifiedBenchmarkInput(t, path)
		campaign, table, order := diversifiedBenchmarkSetup(&group)
		readReference := func(dir string) diversifiedReference {
			t.Helper()
			data, err := os.ReadFile(filepath.Join(dir, fmt.Sprintf("group-%d.reference.json", group.Id)))
			if err != nil {
				t.Fatal(err)
			}
			var ref diversifiedReference
			if err := json.Unmarshal(data, &ref); err != nil {
				t.Fatal(err)
			}
			if ref.InputSHA256 != hash {
				t.Fatal("stale reference input")
			}
			return ref
		}
		selection := readReference(selectionDir)
		holdout := readReference(holdoutDir)
		selected := make(map[[2]int]bool)
		for _, cell := range selection.Cells {
			if cell.P >= 5e-7 && cell.P < 2e-6 {
				selected[[2]int{cell.Team, cell.Position}] = true
			}
		}
		for i := 0; i < seeds; i++ {
			seed := int64(1001 + i)
			estimates := runMatchedPointPoolProduction(&group, campaign, table, order, seed)
			for _, cell := range holdout.Cells {
				key := [2]int{cell.Team, cell.Position}
				got := estimates[cell.Team][cell.Position].Probability
				if estimates[cell.Team][cell.Position].Design != "matched_point_pool" {
					fallback++
				}
				plain := float64(estimates[cell.Team][cell.Position].Hits) / matchedPointPoolSamples
				allPlain += math.Pow(plain-cell.P, 2)
				allPooled += math.Pow(got-cell.P, 2)
				allCount++
				if selected[key] {
					nearPlain += math.Pow(plain-cell.P, 2)
					nearPooled += math.Pow(got-cell.P, 2)
					nearCount++
				}
			}
		}
	}
	if nearCount == 0 || allCount == 0 || fallback != 0 {
		t.Fatalf("near=%d all=%d fallback=%d", nearCount, allCount, fallback)
	}
	t.Logf("near RMSE plain=%g pooled=%g cells=%d; all RMSE plain=%g pooled=%g cells=%d",
		math.Sqrt(nearPlain/float64(nearCount)), math.Sqrt(nearPooled/float64(nearCount)), nearCount,
		math.Sqrt(allPlain/float64(allCount)), math.Sqrt(allPooled/float64(allCount)), allCount)
	if nearPooled >= nearPlain || allPooled >= allPlain {
		t.Fatal("pooled production path regressed against equal-sample plain MC")
	}
}
