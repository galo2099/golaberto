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
	conditionedPointTiltPilotSamples             = 1000
	conditionedPointTiltFinalSamples             = 15000
	conditionedPointTiltLowEvidenceSamples       = 30000
	conditionedPointTiltFinalCells               = 4
	conditionedPointTiltPilotCells               = 12
	conditionedPointTiltUndecidedFinalCells      = 3
	conditionedPointTiltGapSamples               = 60000
	conditionedPointTiltGapCells                 = 2
	conditionedPointTiltUndecidedGapSamples      = 15000
	conditionedPointTiltUndecidedGapPilotSamples = 1000
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

func conditionedPointTiltGapEnabled() bool {
	return os.Getenv("RARE_POSITION_POINT_TILT_GAP_RESCUE") != "0"
}

func conditionedPointTiltUndecidedGapEnabled() bool {
	return conditionedPointTiltUndecidedEnabled() &&
		os.Getenv("RARE_POSITION_POINT_TILT_UNDECIDED_GAP") != "0"
}

func conditionedPointTiltResultValid(result conditionedZeroResult) bool {
	return result.weighted && result.probability > 0 && result.hits >= 30 && result.ess >= 8 &&
		result.stdErr/result.probability <= 0.35 && result.maxWeightShare <= 0.25 && result.batchGap <= 1
}

func applyConditionedPointTiltEstimate(est ProductionEstimate, result conditionedZeroResult, design string) ProductionEstimate {
	est.Probability = result.probability
	est.StdErr = result.stdErr
	est.RelativeSE = relativeSEPointer(result.stdErr / result.probability)
	est.ESS = result.ess
	est.MaxEventWeightShare = result.maxWeightShare
	est.ConditionalMass = result.mass
	est.ConditionalSamples = result.samples
	est.ConditionalHits = result.hits
	est.Design = design
	est.MeetsPrecisionGoal = estimateMeetsPrecisionGoal(result.ess, result.stdErr/result.probability)
	est.Reachability = "witness"
	est.ZeroHitUpper95 = 0
	return est
}

// The proof searches identify some zero cells; unresolved cells can also
// receive a bounded pilot without a witness. Sampling the target-only points
// event includes every feasible final point total, and both proposal tilts
// are corrected by likelihood ratios. Pilots select a proposal but never
// enter its reported estimate.
func runConditionedPointTiltSearch(group *GroupType, campaign []*TeamCampaign,
	table *Table, sortOrder []SortType, seed int64, bounds pointRankBounds,
	samplers []conditionedScoreSampler, cells []conditionedZeroCell,
	estimates map[int]map[int]ProductionEstimate, recycledSamples int) (int, int64) {
	if os.Getenv("RARE_POSITION_CONDITIONED_POINT_TILT") == "0" {
		return 0, 0
	}
	stableSelection := os.Getenv("RARE_POSITION_WIN_AWARE_STABLE_SELECTION") != "0"
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
	type pilotOutcome struct {
		candidate conditionedPointTiltCandidate
		work      int64
		draws     int
		built     bool
		eligible  bool
	}
	pilotResults := make([]pilotOutcome, len(pilotCells))
	runPilot := func(index int) pilotOutcome {
		outcome := pilotOutcome{}
		cell := cells[index]
		event, ok := buildConditionedPointEvent(cell.id, cell.rank, nil,
			group, campaign, table, bounds)
		if !ok || event.mass <= 0 {
			return outcome
		}
		outcome.built = true
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
			pilot, _ := sampleConditionedZeroRankLookaheadWithPointTiltMode(event,
				cell.id, cell.rank, group, campaign, table, sortOrder, bounds,
				samplers, conditionedPointTiltPilotSamples, pilotSeed,
				config[0], pointTilt, stableSelection)
			outcome.work += pilot.work
			outcome.draws += pilot.samples
			// A handful of pilot hits cannot distinguish high-tilt tails.
			// Prefer the first successful, gentler proposal until a pilot
			// has enough effective hits to compare reliably.
			if best.hits == 0 && pilot.hits > 0 || pilot.ess >= 10 && pilot.ess > best.ess {
				best.tilt, best.pointTilt = config[0], pointTilt
				best.ess, best.hits = pilot.ess, pilot.hits
			}
		}
		if best.hits > 0 && !math.IsNaN(best.ess) && !math.IsInf(best.ess, 0) {
			outcome.candidate = best
			outcome.eligible = true
		}
		return outcome
	}
	pilotQueue := make(chan int, len(pilotCells))
	var pilotWait sync.WaitGroup
	for worker := 0; worker < conditionedGuidedWorkers(len(pilotCells)); worker++ {
		pilotWait.Add(1)
		go func() {
			defer pilotWait.Done()
			for position := range pilotQueue {
				pilotResults[position] = runPilot(pilotCells[position])
			}
		}()
	}
	for position := range pilotCells {
		pilotQueue <- position
	}
	close(pilotQueue)
	pilotWait.Wait()
	for _, outcome := range pilotResults {
		totalWork += outcome.work
		pilotDraws += outcome.draws
		if outcome.built {
			built++
		}
		if outcome.eligible {
			candidates = append(candidates, outcome.candidate)
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
	rankedCandidates := candidates
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
		draws  int
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
			valid := conditionedPointTiltResultValid(result)
			draws := result.samples
			if !valid && os.Getenv("RARE_POSITION_WIN_AWARE_FALLBACK") != "0" &&
				conditionedRankWinStride(sortOrder, group, campaign, table, "lookahead") > 1 {
				pointResult, _ := sampleConditionedZeroRankLookaheadWithPointTiltMode(
					candidate.event, cell.id, cell.rank, group, campaign, table,
					sortOrder, bounds, samplers, samples,
					freshSeed, candidate.tilt, candidate.pointTilt, true)
				pointResult.work += result.work
				draws += pointResult.samples
				result = pointResult
				valid = conditionedPointTiltResultValid(result)
			}
			confirmations[i] = confirmed{result, valid, draws}
		}(i, candidate)
	}
	wait.Wait()
	witnesses := 0
	for i, candidate := range candidates {
		confirmation := confirmations[i]
		totalWork += confirmation.result.work
		finalDraws += confirmation.draws
		if !confirmation.valid {
			continue
		}
		cell := cells[candidate.index]
		result := confirmation.result
		estimates[cell.id][cell.rank] = applyConditionedPointTiltEstimate(
			estimates[cell.id][cell.rank], result, "matched_point_pool_conditioned_point_tilt")
		witnesses++
	}
	gapAttempts, gapAccepted, gapDraws := 0, 0, 0
	if conditionedPointTiltGapEnabled() {
		var gapCells []conditionedZeroCell
		for _, cell := range cells {
			if cell.rank == 0 || cell.rank+1 >= len(group.Team_groups) {
				continue
			}
			row := estimates[cell.id]
			if row[cell.rank].Probability == 0 && row[cell.rank].Reachability == "reachable_by_construction" &&
				row[cell.rank-1].Probability > 0 && row[cell.rank+1].Probability > 0 {
				gapCells = append(gapCells, cell)
			}
		}
		sort.Slice(gapCells, func(i, j int) bool {
			a, b := gapCells[i], gapCells[j]
			upperA, upperB := estimates[a.id][a.rank].ZeroHitUpper95, estimates[b.id][b.rank].ZeroHitUpper95
			if upperA != upperB {
				return upperA > upperB
			}
			if a.id != b.id {
				return a.id < b.id
			}
			return a.rank < b.rank
		})
		// Share the former one-cell confirmation budget across at most two
		// proved gaps. Pilots and confirmations both count against the cap.
		budget := conditionedPointTiltGapSamples
		for _, cell := range gapCells {
			if gapAttempts >= conditionedPointTiltGapCells ||
				budget < 2*conditionedPointTiltPilotSamples+conditionedPointTiltFinalSamples {
				break
			}
			event, ok := buildConditionedPointEvent(cell.id, cell.rank, nil, group, campaign, table, bounds)
			if !ok || event.mass <= 0 {
				continue
			}
			pointDirection := 0.5
			if cell.rank > len(group.Team_groups)/2 {
				pointDirection = -0.5
			}
			pilot := func(name string, tilt, pointTilt float64) conditionedZeroResult {
				pilotSeed := deriveRarePositionSeed(seed, fmt.Sprintf("split-gap-%s-pilot-%d-%d", name, cell.id, cell.rank))
				result, _ := sampleConditionedZeroRankLookaheadWithPointTiltMode(event, cell.id, cell.rank,
					group, campaign, table, sortOrder, bounds, samplers,
					conditionedPointTiltPilotSamples, pilotSeed, tilt, pointTilt, stableSelection)
				budget -= result.samples
				pilotDraws += result.samples
				gapDraws += result.samples
				totalWork += result.work
				return result
			}
			gentle := pilot("gentle", 3, pointDirection)
			moderate := pilot("moderate", 8, 2*pointDirection)
			// Only prefer the stronger tilt when the gentle pilot almost
			// never reaches the rank. High hit counts alone are unreliable
			// when a proposal has very uneven event weights.
			moderateFirst := gentle.hits <= 2 && moderate.hits >= 10 && moderate.ess >= 1.5
			firstName, firstTilt, firstPoint := "gentle", 3.0, pointDirection
			secondName, secondTilt, secondPoint := "moderate", 8.0, 2*pointDirection
			if moderateFirst {
				firstName, secondName = secondName, firstName
				firstTilt, secondTilt = secondTilt, firstTilt
				firstPoint, secondPoint = secondPoint, firstPoint
			}
			confirm := func(name string, tilt, pointTilt float64) (conditionedZeroResult, bool) {
				freshSeed := deriveRarePositionSeed(seed, fmt.Sprintf("split-gap-%s-final-%d-%d", name, cell.id, cell.rank))
				result, _ := sampleConditionedZeroRankLookaheadWithPointTilt(event, cell.id, cell.rank,
					group, campaign, table, sortOrder, bounds, samplers,
					conditionedPointTiltFinalSamples, freshSeed, tilt, pointTilt)
				budget -= result.samples
				finalDraws += result.samples
				gapDraws += result.samples
				totalWork += result.work
				return result, conditionedPointTiltResultValid(result)
			}
			gapAttempts++
			result, accepted := confirm(firstName, firstTilt, firstPoint)
			if !accepted && budget >= conditionedPointTiltFinalSamples {
				result, accepted = confirm(secondName, secondTilt, secondPoint)
			}
			if accepted {
				estimates[cell.id][cell.rank] = applyConditionedPointTiltEstimate(
					estimates[cell.id][cell.rank], result, "matched_point_pool_conditioned_point_tilt_gap")
				witnesses++
				gapAccepted++
			}
		}
	}
	undecidedGapAttempts, undecidedGapAccepted, undecidedGapDraws := 0, 0, 0
	if conditionedPointTiltUndecidedGapEnabled() {
		var gapCells []conditionedZeroCell
		for _, cell := range cells {
			if cell.rank == 0 || cell.rank+1 >= len(group.Team_groups) {
				continue
			}
			row := estimates[cell.id]
			if row[cell.rank].Probability == 0 && row[cell.rank].Reachability == "undecided" &&
				row[cell.rank-1].Probability > 0 && row[cell.rank+1].Probability > 0 {
				gapCells = append(gapCells, cell)
			}
		}
		sort.Slice(gapCells, func(i, j int) bool {
			a, b := gapCells[i], gapCells[j]
			upperA, upperB := estimates[a.id][a.rank].ZeroHitUpper95, estimates[b.id][b.rank].ZeroHitUpper95
			if upperA != upperB {
				return upperA > upperB
			}
			if a.id != b.id {
				return a.id < b.id
			}
			return a.rank < b.rank
		})
		for _, cell := range gapCells {
			event, ok := buildConditionedPointEvent(cell.id, cell.rank, nil, group, campaign, table, bounds)
			if !ok || event.mass <= 0 {
				continue
			}
			pointDirection := 0.5
			if cell.rank > len(group.Team_groups)/2 {
				pointDirection = -0.5
			}
			// Short, independent pilots choose the first proposal. Confirmation
			// uses fresh draws so pilot outcomes cannot bias the estimate.
			pilot := func(name string, tilt, pointTilt float64) conditionedZeroResult {
				pilotSeed := deriveRarePositionSeed(seed, fmt.Sprintf(
					"conditioned-point-tilt-undecided-gap-%s-pilot-%d-%d", name, cell.id, cell.rank))
				result, _ := sampleConditionedZeroRankLookaheadWithPointTiltMode(event,
					cell.id, cell.rank, group, campaign, table, sortOrder, bounds,
					samplers, conditionedPointTiltUndecidedGapPilotSamples,
					pilotSeed, tilt, pointTilt, stableSelection)
				totalWork += result.work
				pilotDraws += result.samples
				undecidedGapDraws += result.samples
				return result
			}
			gentlePilot := pilot("gentle", 3, pointDirection)
			moderatePilot := pilot("moderate", 8, 2*pointDirection)
			moderateFirst := gentlePilot.hits <= 2 && moderatePilot.hits >= 25 && moderatePilot.ess >= 3
			confirm := func(name string, tilt, pointTilt float64) (conditionedZeroResult, bool) {
				freshSeed := deriveRarePositionSeed(seed, fmt.Sprintf(
					"conditioned-point-tilt-undecided-gap-%s-final-%d-%d", name, cell.id, cell.rank))
				result, _ := sampleConditionedZeroRankLookaheadWithPointTilt(event,
					cell.id, cell.rank, group, campaign, table, sortOrder, bounds,
					samplers, conditionedPointTiltUndecidedGapSamples,
					freshSeed, tilt, pointTilt)
				totalWork += result.work
				finalDraws += result.samples
				undecidedGapDraws += result.samples
				valid := conditionedPointTiltResultValid(result)
				if name == "moderate" {
					valid = valid && result.ess >= 50 && result.maxWeightShare <= 0.05 && result.batchGap <= 0.5
				}
				return result, valid
			}
			firstName, firstTilt, firstPoint := "gentle", 3.0, pointDirection
			secondName, secondTilt, secondPoint := "moderate", 8.0, 2*pointDirection
			if moderateFirst {
				firstName, secondName = secondName, firstName
				firstTilt, secondTilt = secondTilt, firstTilt
				firstPoint, secondPoint = secondPoint, firstPoint
			}
			undecidedGapAttempts++
			result, accepted := confirm(firstName, firstTilt, firstPoint)
			if !accepted {
				result, accepted = confirm(secondName, secondTilt, secondPoint)
			}
			if accepted {
				estimates[cell.id][cell.rank] = applyConditionedPointTiltEstimate(
					estimates[cell.id][cell.rank], result,
					"matched_point_pool_conditioned_point_tilt_undecided_gap")
				witnesses++
				undecidedGapAccepted++
			}
			break
		}
	}
	// An early impossibility proof can free enough targeted work for one
	// independent, short confirmation of a pilot-positive cell that missed
	// the regular final slots. A failed confirmation does not enter the odds.
	recycledDraws, recycledAccepted := 0, 0
	if recycledSamples > 0 {
		selected := make(map[int]bool, len(candidates))
		for _, candidate := range candidates {
			selected[candidate.index] = true
		}
		for _, candidate := range rankedCandidates {
			if selected[candidate.index] {
				continue
			}
			cell := cells[candidate.index]
			if estimates[cell.id][cell.rank].Probability > 0 {
				continue
			}
			freshSeed := deriveRarePositionSeed(seed, fmt.Sprintf(
				"conditioned-point-tilt-final-%d-%d", cell.id, cell.rank))
			result, _ := sampleConditionedZeroRankLookaheadWithPointTilt(
				candidate.event, cell.id, cell.rank, group, campaign, table,
				sortOrder, bounds, samplers, recycledSamples, freshSeed,
				candidate.tilt, candidate.pointTilt)
			recycledDraws = result.samples
			totalWork += result.work
			finalDraws += result.samples
			if conditionedPointTiltResultValid(result) {
				estimates[cell.id][cell.rank] = applyConditionedPointTiltEstimate(
					estimates[cell.id][cell.rank], result,
					"matched_point_pool_conditioned_point_tilt_recycled")
				witnesses++
				recycledAccepted = 1
			}
			break
		}
	}
	if len(pilotCells) > 0 {
		log.Printf("rare-position-point-tilt: group=%d attempted=%d built=%d pilot_draws=%d final_draws=%d proved_final=%d undecided_final=%d recycled_draws=%d recycled_accepted=%d gap_attempted=%d gap_accepted=%d gap_draws=%d undecided_gap_attempted=%d undecided_gap_accepted=%d undecided_gap_draws=%d accepted=%d work=%d elapsed=%s",
			group.Id, len(pilotCells), built, pilotDraws, finalDraws,
			provedFinals, undecidedFinals, recycledDraws, recycledAccepted, gapAttempts, gapAccepted, gapDraws,
			undecidedGapAttempts, undecidedGapAccepted, undecidedGapDraws,
			witnesses, totalWork, time.Since(start))
	}
	return witnesses, totalWork
}
