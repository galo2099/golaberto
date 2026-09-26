package main

import (
	"log"
	"math"
	"math/rand"
	"os"
	"sort"
)

const matchedPointPoolSamples = 100000

func matchedPointPoolEnabled() bool {
	return os.Getenv("RARE_POSITION_MATCHED_POINT_POOL") == "1"
}

func sortedPointTotals(pmf map[int]float64) []int {
	totals := make([]int, 0, len(pmf))
	for total := range pmf {
		totals = append(totals, total)
	}
	sort.Ints(totals)
	return totals
}

// matchedPointPoolMatrix uses only actual finishes from the scout. The exact
// marginal point distribution supplies the mass at each target point total.
func matchedPointPoolMatrix(teams []int, scouts map[int]*TeamPointRankScout,
	current map[int]int, pmfs map[int]map[int]float64, bounds pointRankBounds) map[int]map[int]float64 {
	n := len(teams)
	groupCounts := make(map[int][]int)
	groupTotals := make(map[int]int)
	means := make(map[int]float64, n)
	for _, id := range teams {
		mean := float64(current[id])
		for _, added := range sortedPointTotals(pmfs[id]) {
			mass := pmfs[id][added]
			mean += float64(added) * mass
		}
		means[id] = mean
		for added, counts := range scouts[id].PointRankCounts {
			final := current[id] + added
			if groupCounts[final] == nil {
				groupCounts[final] = make([]int, n)
			}
			groupTotals[final] += scouts[id].PointCounts[added]
			for rank, count := range counts {
				groupCounts[final][rank] += count
			}
		}
	}
	raw := make(map[int]map[int]float64, n)
	for _, id := range teams {
		raw[id] = make(map[int]float64, n)
		matched := make([]float64, n)
		shrunk := make([]float64, n)
		own := scouts[id]
		for _, added := range sortedPointTotals(pmfs[id]) {
			mass := pmfs[id][added]
			if mass <= 0 {
				continue
			}
			final := current[id] + added
			groupTotal := groupTotals[final]
			if groupTotal == 0 {
				continue
			}
			weighted := make([]float64, n)
			weightTotal := 0.0
			for _, donor := range teams {
				delta := final - current[donor]
				count := scouts[donor].PointCounts[delta]
				if count == 0 {
					continue
				}
				weight := math.Exp(-math.Abs(means[donor]-means[id]) / 3)
				weightTotal += float64(count) * weight
				for rank, hits := range scouts[donor].PointRankCounts[delta] {
					weighted[rank] += float64(hits) * weight
				}
			}
			ownCount := own.PointCounts[added]
			ownRanks := own.PointRankCounts[added]
			for rank := 0; rank < n; rank++ {
				if !bounds.rankNotRuledOut(id, rank, added) {
					continue
				}
				groupQ := float64(groupCounts[final][rank]) / float64(groupTotal)
				if weightTotal > 0 {
					matched[rank] += mass * weighted[rank] / weightTotal
				}
				ownHits := 0
				if rank < len(ownRanks) {
					ownHits = ownRanks[rank]
				}
				shrunk[rank] += mass * (float64(ownHits) + 5*groupQ) / float64(ownCount+5)
			}
		}
		for rank := 0; rank < n; rank++ {
			if own.RankCounts[rank] <= 10 {
				raw[id][rank] = matched[rank]
			} else {
				raw[id][rank] = shrunk[rank]
			}
		}
	}
	return reconcileProbabilityMatrix(raw, teams)
}

func matchedPointPoolValid(matrix map[int]map[int]float64, teams []int) bool {
	for _, id := range teams {
		sum := 0.0
		for rank := range teams {
			p := matrix[id][rank]
			if math.IsNaN(p) || math.IsInf(p, 0) || p < 0 || p > 1 {
				return false
			}
			sum += p
		}
		if math.Abs(sum-1) > 1e-6 {
			return false
		}
	}
	for rank := range teams {
		sum := 0.0
		for _, id := range teams {
			sum += matrix[id][rank]
		}
		if math.Abs(sum-1) > 1e-6 {
			return false
		}
	}
	return true
}

func balanceMatchedPointPool(matrix map[int]map[int]float64, teams []int) bool {
	// Sparse support can need substantially more than the 100 screening rounds.
	// Continue to the production tolerance while preserving every structural zero.
	for iteration := 0; iteration < 20000; iteration++ {
		for _, id := range teams {
			sum := 0.0
			for rank := range teams {
				sum += matrix[id][rank]
			}
			if sum > 0 {
				for rank := range teams {
					matrix[id][rank] /= sum
				}
			}
		}
		for rank := range teams {
			sum := 0.0
			for _, id := range teams {
				sum += matrix[id][rank]
			}
			if sum > 0 {
				for _, id := range teams {
					matrix[id][rank] /= sum
				}
			}
		}
		if iteration%100 == 0 && matchedPointPoolValid(matrix, teams) {
			return true
		}
	}
	return matchedPointPoolValid(matrix, teams)
}

func subtractPointRankBatch(full, batch map[int]*TeamPointRankScout, teams []int) map[int]*TeamPointRankScout {
	result := make(map[int]*TeamPointRankScout, len(teams))
	for _, id := range teams {
		a, b := full[id], batch[id]
		copyScout := &TeamPointRankScout{Samples: a.Samples - b.Samples,
			RankCounts: make([]int, len(a.RankCounts)), PointCounts: make(map[int]int),
			PointRankCounts: make(map[int][]int)}
		for rank, count := range a.RankCounts {
			copyScout.RankCounts[rank] = count - b.RankCounts[rank]
		}
		for added, count := range a.PointCounts {
			copyScout.PointCounts[added] = count - b.PointCounts[added]
			if copyScout.PointCounts[added] == 0 {
				delete(copyScout.PointCounts, added)
				continue
			}
			copyScout.PointRankCounts[added] = make([]int, len(a.RankCounts))
			for rank, hits := range a.PointRankCounts[added] {
				batchHits := 0
				if rank < len(b.PointRankCounts[added]) {
					batchHits = b.PointRankCounts[added][rank]
				}
				copyScout.PointRankCounts[added][rank] = hits - batchHits
			}
		}
		result[id] = copyScout
	}
	return result
}

func runMatchedPointPoolProduction(group *GroupType, campaign []*TeamCampaign,
	table *Table, sortOrder []SortType, seed int64) map[int]map[int]ProductionEstimate {
	teams := teamIDsFromGroups(group.Team_groups)
	pmfs := make(map[int]map[int]float64, len(teams))
	current := make(map[int]int, len(teams))
	supported := len(sortOrder) > 0 && sortOrder[0] == PT
	if supported {
		for _, id := range teams {
			universe, ok := pointOutcomeUniverse(id, campaign, table, group.Games, 39)
			if !ok {
				supported = false
				break
			}
			pmfs[id] = additionalPointsPMF(universe)
			current[id] = campaign[table.Query(uint32(id))].points
		}
	}
	unplayed := 0
	for _, game := range group.Games {
		if !game.Played {
			unplayed++
		}
	}
	work := estimateSeasonWork(unplayed, 1, len(teams)) * matchedPointPoolSamples
	rng := rand.New(rand.NewSource(deriveRarePositionSeed(seed, "pooled-point-scout")))
	if !supported {
		log.Printf("rare-position-matched-pool: group=%d unsupported standings/fixtures; using plain MC", group.Id)
		counts := simulatePlainRankCounts(campaign, group.Games, table, sortOrder, group.Team_groups, matchedPointPoolSamples, rng)
		return summarizePlainProductionCounts(counts, matchedPointPoolSamples, work)
	}
	scout := runPlainMCScoutWithJointPointsBatched(campaign, group.Games, table, sortOrder,
		group.Team_groups, matchedPointPoolSamples, work/matchedPointPoolSamples, rng, 10, false)
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	matrix := matchedPointPoolMatrix(teams, scout.TeamScout, current, pmfs, bounds)
	if !balanceMatchedPointPool(matrix, teams) {
		log.Printf("rare-position-matched-pool: group=%d invalid reconciled matrix; using plain MC", group.Id)
		return summarizePlainProductionCounts(scout.TeamCounts, matchedPointPoolSamples, work)
	}
	leaveout := make([]map[int]map[int]float64, len(scout.PointRankBatches))
	for i, batch := range scout.PointRankBatches {
		leaveout[i] = matchedPointPoolMatrix(teams,
			subtractPointRankBatch(scout.TeamScout, batch, teams), current, pmfs, bounds)
		if !balanceMatchedPointPool(leaveout[i], teams) {
			log.Printf("rare-position-matched-pool: group=%d invalid leave-one-batch matrix; using plain MC", group.Id)
			return summarizePlainProductionCounts(scout.TeamCounts, matchedPointPoolSamples, work)
		}
	}
	estimates := make(map[int]map[int]ProductionEstimate, len(teams))
	for _, id := range teams {
		estimates[id] = make(map[int]ProductionEstimate, len(teams))
		for rank := range teams {
			p := matrix[id][rank]
			mean := 0.0
			for _, partial := range leaveout {
				mean += partial[id][rank]
			}
			mean /= float64(len(leaveout))
			variance := 0.0
			for _, partial := range leaveout {
				delta := partial[id][rank] - mean
				variance += delta * delta
			}
			se := math.Sqrt(float64(len(leaveout)-1) / float64(len(leaveout)) * variance)
			hits := scout.TeamScout[id].RankCounts[rank]
			upper := 0.0
			if hits == 0 {
				upper = zeroHitUpper95(matchedPointPoolSamples)
				hardBound, _, _ := computeHardCellUpperBoundWithBounds(id, rank, pmfs[id], bounds)
				upper = math.Min(upper, hardBound)
			}
			rel := math.Inf(1)
			if p > 0 {
				rel = se / p
			}
			estimates[id][rank] = ProductionEstimate{
				Probability: p, StdErr: se, Samples: matchedPointPoolSamples, Hits: hits,
				MeanWeight: 1, WorkSpent: work, Available: true, MeetsPrecisionGoal: false,
				RelativeSE: relativeSEPointer(rel), ZeroHitUpper95: upper,
				Design: "matched_point_pool",
			}
		}
	}
	log.Printf("rare-position-matched-pool: group=%d samples=%d work=%d", group.Id, matchedPointPoolSamples, work)
	return estimates
}
