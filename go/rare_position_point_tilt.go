package main

import (
	"fmt"
	"log"
	"math"
	"os"
	"sort"
	"sync"
	"time"
)

const (
	conditionedPointTiltPilotSamples        = 1000
	conditionedPointTiltFinalSamples        = 15000
	conditionedPointTiltLowEvidenceSamples  = 30000
	conditionedPointTiltFinalCells          = 4
	conditionedPointTiltPilotCells          = 12
	conditionedPointTiltUndecidedFinalCells = 3
)

type conditionedPointTiltCandidate struct {
	index     int
	event     *conditionedPointEvent
	tilt      float64
	pointTilt float64
	ess       float64
	hits      int
	proved    bool
}

func conditionedPointTiltUndecidedEnabled() bool {
	return os.Getenv("RARE_POSITION_CONDITIONED_POINT_TILT") != "0" &&
		os.Getenv("RARE_POSITION_POINT_TILT_UNDECIDED") != "0"
}

// The proof searches identify some zero cells; unresolved cells can also
// receive a bounded pilot without a witness. Sampling the target-only points
// event includes every feasible final point total, and both proposal tilts
// are corrected by likelihood ratios. Pilots select a proposal but never
// enter its reported estimate.
func runConditionedPointTiltSearch(group *GroupType, campaign []*TeamCampaign,
	table *Table, sortOrder []SortType, seed int64, bounds pointRankBounds,
	samplers []conditionedScoreSampler, cells []conditionedZeroCell,
	estimates map[int]map[int]ProductionEstimate) (int, int64) {
	if os.Getenv("RARE_POSITION_CONDITIONED_POINT_TILT") == "0" {
		return 0, 0
	}
	start := time.Now()
	var candidates []conditionedPointTiltCandidate
	var totalWork int64
	pilotDraws, finalDraws, built := 0, 0, 0
	var proved []int
	var undecided []int
	includeUndecided := conditionedPointTiltUndecidedEnabled()
	for index, cell := range cells {
		est := estimates[cell.id][cell.rank]
		if est.Probability == 0 && est.Reachability == "reachable_by_construction" {
			proved = append(proved, index)
		} else if includeUndecided && est.Probability == 0 && est.Reachability == "undecided" {
			undecided = append(undecided, index)
		}
	}
	sortByUpper := func(indices []int) {
		sort.Slice(indices, func(i, j int) bool {
			a, b := cells[indices[i]], cells[indices[j]]
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
	}
	sortByUpper(proved)
	sortByUpper(undecided)
	pilotCells := append(proved, undecided...)
	if len(pilotCells) > conditionedPointTiltPilotCells {
		pilotCells = pilotCells[:conditionedPointTiltPilotCells]
	}
	for _, index := range pilotCells {
		cell := cells[index]
		event, ok := buildConditionedPointEvent(cell.id, cell.rank, nil,
			group, campaign, table, bounds)
		if !ok || event.mass <= 0 {
			continue
		}
		built++
		direction := 1.0
		if cell.rank > len(group.Team_groups)/2 {
			direction = -1
		}
		best := conditionedPointTiltCandidate{index: index, event: event,
			proved: estimates[cell.id][cell.rank].Reachability == "reachable_by_construction"}
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
			pilotDraws += pilot.samples
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
		if candidates[i].hits != candidates[j].hits {
			return candidates[i].hits > candidates[j].hits
		}
		a, b := cells[candidates[i].index], cells[candidates[j].index]
		if a.id != b.id {
			return a.id < b.id
		}
		return a.rank < b.rank
	})
	// Preserve the previous four proved-cell slots. Undecided cells can
	// use three additional fresh estimates without displacing those cells.
	selected := make([]conditionedPointTiltCandidate, 0,
		conditionedPointTiltFinalCells+conditionedPointTiltUndecidedFinalCells)
	provedFinals, undecidedFinals := 0, 0
	for _, candidate := range candidates {
		if candidate.proved && provedFinals < conditionedPointTiltFinalCells {
			selected = append(selected, candidate)
			provedFinals++
		} else if includeUndecided && !candidate.proved &&
			undecidedFinals < conditionedPointTiltUndecidedFinalCells {
			selected = append(selected, candidate)
			undecidedFinals++
		}
	}
	candidates = selected
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
			samples := conditionedPointTiltFinalSamples
			// A few pilot hits can identify a useful proposal while leaving
			// its confirmation vulnerable to one dominating event weight.
			if candidate.proved && candidate.ess < 10 && candidate.hits >= 3 {
				samples = conditionedPointTiltLowEvidenceSamples
			}
			result, _ := sampleConditionedZeroRankLookaheadWithPointTilt(
				candidate.event, cell.id, cell.rank, group, campaign, table,
				sortOrder, bounds, samplers, samples,
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
		finalDraws += confirmation.result.samples
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
	if len(pilotCells) > 0 {
		log.Printf("rare-position-point-tilt: group=%d attempted=%d built=%d pilot_draws=%d final_draws=%d proved_final=%d undecided_final=%d accepted=%d work=%d elapsed=%s",
			group.Id, len(pilotCells), built, pilotDraws, finalDraws,
			provedFinals, undecidedFinals, witnesses, totalWork, time.Since(start))
	}
	return witnesses, totalWork
}
