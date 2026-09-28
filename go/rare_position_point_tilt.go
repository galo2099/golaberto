package main

import (
	"fmt"
	"math"
	"os"
	"sort"
	"sync"
)

const (
	conditionedPointTiltPilotSamples = 1000
	conditionedPointTiltFinalSamples = 15000
	conditionedPointTiltFinalCells   = 4
	conditionedPointTiltPilotCells   = 12
)

type conditionedPointTiltCandidate struct {
	index     int
	event     *conditionedPointEvent
	tilt      float64
	pointTilt float64
	ess       float64
	hits      int
}

// The proof searches identify which zero cells are worth spending a short
// independent estimate on. Sampling the target-only points event includes
// every feasible final point total, and both proposal tilts are corrected by
// likelihood ratios. Pilot draws select a proposal but never enter its estimate.
func runConditionedPointTiltSearch(group *GroupType, campaign []*TeamCampaign,
	table *Table, sortOrder []SortType, seed int64, bounds pointRankBounds,
	samplers []conditionedScoreSampler, cells []conditionedZeroCell,
	estimates map[int]map[int]ProductionEstimate) (int, int64) {
	if os.Getenv("RARE_POSITION_CONDITIONED_POINT_TILT") == "0" {
		return 0, 0
	}
	var candidates []conditionedPointTiltCandidate
	var totalWork int64
	var proved []int
	for index, cell := range cells {
		est := estimates[cell.id][cell.rank]
		if est.Probability == 0 && est.Reachability == "reachable_by_construction" {
			proved = append(proved, index)
		}
	}
	sort.Slice(proved, func(i, j int) bool {
		a, b := cells[proved[i]], cells[proved[j]]
		upperA := estimates[a.id][a.rank].ZeroHitUpper95
		upperB := estimates[b.id][b.rank].ZeroHitUpper95
		if upperA != upperB {
			return upperA > upperB
		}
		if a.id != b.id {
			return a.id < b.id
		}
		return a.rank < b.rank
	})
	if len(proved) > conditionedPointTiltPilotCells {
		proved = proved[:conditionedPointTiltPilotCells]
	}
	for _, index := range proved {
		cell := cells[index]
		event, ok := buildConditionedPointEvent(cell.id, cell.rank, nil,
			group, campaign, table, bounds)
		if !ok || event.mass <= 0 {
			continue
		}
		direction := 1.0
		if cell.rank > len(group.Team_groups)/2 {
			direction = -1
		}
		best := conditionedPointTiltCandidate{index: index, event: event}
		for _, config := range [][2]float64{{3, 0.5}, {8, 1}, {12, 1}} {
			pointTilt := direction * config[1]
			pilotSeed := deriveRarePositionSeed(seed, fmt.Sprintf(
				"conditioned-point-tilt-pilot-%d-%d-%g-%g",
				cell.id, cell.rank, config[0], pointTilt))
			pilot, _ := sampleConditionedZeroRankLookaheadWithPointTilt(event,
				cell.id, cell.rank, group, campaign, table, sortOrder, bounds,
				samplers, conditionedPointTiltPilotSamples, pilotSeed,
				config[0], pointTilt)
			totalWork += pilot.work
			// A handful of pilot hits cannot distinguish high-tilt tails.
			// Prefer the first successful, gentler proposal until a pilot
			// has enough effective hits to compare reliably.
			if best.hits == 0 && pilot.hits > 0 || pilot.ess >= 10 && pilot.ess > best.ess {
				best.tilt, best.pointTilt = config[0], pointTilt
				best.ess, best.hits = pilot.ess, pilot.hits
			}
		}
		if best.hits > 0 && !math.IsNaN(best.ess) && !math.IsInf(best.ess, 0) {
			candidates = append(candidates, best)
		}
	}
	sort.Slice(candidates, func(i, j int) bool {
		if candidates[i].ess != candidates[j].ess {
			return candidates[i].ess > candidates[j].ess
		}
		return candidates[i].hits > candidates[j].hits
	})
	if len(candidates) > conditionedPointTiltFinalCells {
		candidates = candidates[:conditionedPointTiltFinalCells]
	}
	type confirmed struct {
		result conditionedZeroResult
		valid  bool
	}
	confirmations := make([]confirmed, len(candidates))
	var wait sync.WaitGroup
	for i, candidate := range candidates {
		wait.Add(1)
		go func(i int, candidate conditionedPointTiltCandidate) {
			defer wait.Done()
			cell := cells[candidate.index]
			freshSeed := deriveRarePositionSeed(seed, fmt.Sprintf(
				"conditioned-point-tilt-final-%d-%d", cell.id, cell.rank))
			result, _ := sampleConditionedZeroRankLookaheadWithPointTilt(
				candidate.event, cell.id, cell.rank, group, campaign, table,
				sortOrder, bounds, samplers, conditionedPointTiltFinalSamples,
				freshSeed, candidate.tilt, candidate.pointTilt)
			valid := result.weighted && result.hits >= 30 && result.ess >= 8 &&
				result.stdErr/result.probability <= 0.35 &&
				result.maxWeightShare <= 0.25 && result.batchGap <= 1
			confirmations[i] = confirmed{result, valid}
		}(i, candidate)
	}
	wait.Wait()
	witnesses := 0
	for i, candidate := range candidates {
		confirmation := confirmations[i]
		totalWork += confirmation.result.work
		if !confirmation.valid {
			continue
		}
		cell := cells[candidate.index]
		est := estimates[cell.id][cell.rank]
		result := confirmation.result
		est.Probability = result.probability
		est.StdErr = result.stdErr
		est.RelativeSE = relativeSEPointer(result.stdErr / result.probability)
		est.ESS = result.ess
		est.MaxEventWeightShare = result.maxWeightShare
		est.ConditionalMass = result.mass
		est.ConditionalSamples = result.samples
		est.ConditionalHits = result.hits
		est.Design = "matched_point_pool_conditioned_point_tilt"
		est.MeetsPrecisionGoal = estimateMeetsPrecisionGoal(result.ess,
			result.stdErr/result.probability)
		est.Reachability = "witness"
		est.ZeroHitUpper95 = 0
		estimates[cell.id][cell.rank] = est
		witnesses++
	}
	return witnesses, totalWork
}
