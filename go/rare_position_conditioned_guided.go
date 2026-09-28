package main

import (
	"fmt"
	"math"
	"runtime"
	"sort"
	"sync"
)

const (
	conditionedGuidedPilotSamples   = 200
	conditionedGuidedEstimateBudget = 3 * conditionedZeroLookaheadSamples
	conditionedGuidedDeepBudget     = 2 * conditionedZeroDeepRuns
)

type conditionedGuidedCandidate struct {
	index int
	hits  int
	ess   float64
}

func conditionedGuidedWorkers(jobs int) int {
	workers := runtime.GOMAXPROCS(0)
	if workers > 4 {
		workers = 4
	}
	if workers > jobs {
		workers = jobs
	}
	return workers
}

// runConditionedGuidedSearch gives every unresolved cell a small pilot. Only
// cells with observed, reasonably distributed importance weight receive a
// fresh production estimate. Rank enters the point feasibility calculation,
// never a position-specific search policy.
func runConditionedGuidedSearch(group *GroupType, campaign []*TeamCampaign,
	table *Table, sortOrder []SortType, seed int64, teams []int,
	current map[int]int, pmfs map[int]map[int]float64, bounds pointRankBounds,
	samplers []conditionedScoreSampler, cells []conditionedZeroCell,
	results []conditionedZeroSearchResult) {
	if !conditionedZeroLookaheadEnabled() && !conditionedZeroDeepEnabled() {
		return
	}
	candidates := make([]conditionedGuidedCandidate, len(cells))
	eligible := make([]bool, len(cells))
	queue := make(chan int, len(cells))
	var wait sync.WaitGroup
	for worker := 0; worker < conditionedGuidedWorkers(len(cells)); worker++ {
		wait.Add(1)
		go func() {
			defer wait.Done()
			for index := range queue {
				cell := cells[index]
				search := results[index]
				if search.impossible || search.result.samples == 0 || search.result.hits > 0 || search.event == nil {
					continue
				}
				event := search.event
				if event.mass < 1e-9 {
					blockers := conditionedZeroBlockers(cell.id, cell.rank, teams, current,
						pmfs, group.Games, 4)
					if deeper, ok := buildConditionedPointEventWithLimit(cell.id, cell.rank,
						blockers, group, campaign, table, bounds,
						conditionedZeroLookaheadStates); ok {
						if deeper.mass <= 0 {
							search.impossible = len(deeper.terminal) == 0
							search.event = deeper
							results[index] = search
							continue
						}
						event = deeper
					}
				}
				pilotSeed := deriveRarePositionSeed(seed,
					fmt.Sprintf("conditioned-guided-pilot-%d-%d", cell.id, cell.rank))
				pilot, _ := sampleConditionedZeroRankLookahead(event, cell.id, cell.rank,
					group, campaign, table, sortOrder, bounds, samplers,
					conditionedGuidedPilotSamples, pilotSeed)
				search.result.work += pilot.work
				search.event = event
				results[index] = search
				// The pilot only selects cells. The fresh production draw has
				// its own precision checks, so a noisy 200-draw weight must
				// not veto an otherwise well discovered cell.
				if pilot.weighted && pilot.hits >= 20 &&
					!math.IsNaN(pilot.ess) && !math.IsInf(pilot.ess, 0) {
					eligible[index] = true
					candidates[index] = conditionedGuidedCandidate{index, pilot.hits, pilot.ess}
				}
			}
		}()
	}
	for index := range cells {
		queue <- index
	}
	close(queue)
	wait.Wait()
	selected := make([]conditionedGuidedCandidate, 0, len(cells))
	for index := range cells {
		if eligible[index] {
			selected = append(selected, candidates[index])
		}
	}
	sort.Slice(selected, func(i, j int) bool {
		if selected[i].hits != selected[j].hits {
			return selected[i].hits > selected[j].hits
		}
		if selected[i].ess != selected[j].ess {
			return selected[i].ess > selected[j].ess
		}
		a, b := cells[selected[i].index], cells[selected[j].index]
		if a.id != b.id {
			return a.id < b.id
		}
		return a.rank < b.rank
	})
	if conditionedZeroLookaheadEnabled() {
		limit := conditionedGuidedEstimateBudget / conditionedZeroLookaheadSamples
		if limit > len(selected) {
			limit = len(selected)
		}
		var productionWait sync.WaitGroup
		for _, candidate := range selected[:limit] {
			productionWait.Add(1)
			go func(index int) {
				defer productionWait.Done()
				cell := cells[index]
				search := results[index]
				productionSeed := deriveRarePositionSeed(seed,
					fmt.Sprintf("conditioned-guided-estimate-%d-%d", cell.id, cell.rank))
				weighted, accepted := sampleConditionedZeroRankLookahead(search.event,
					cell.id, cell.rank, group, campaign, table, sortOrder, bounds,
					samplers, conditionedZeroLookaheadSamples, productionSeed)
				if accepted && weighted.weighted && weighted.hits > 0 {
					weighted.work += search.result.work
					weighted.blockers = len(search.event.teams) - 1
					search.result = weighted
				} else {
					search.result.work += weighted.work
				}
				results[index] = search
			}(candidate.index)
		}
		productionWait.Wait()
	}
	if conditionedZeroDeepEnabled() {
		runConditionedGuidedDeep(group, campaign, table, sortOrder, seed,
			teams, current, pmfs, bounds, samplers, cells, results, selected)
	}
}

func runConditionedGuidedDeep(group *GroupType, campaign []*TeamCampaign,
	table *Table, sortOrder []SortType, seed int64, teams []int,
	current map[int]int, pmfs map[int]map[int]float64, bounds pointRankBounds,
	samplers []conditionedScoreSampler, cells []conditionedZeroCell,
	results []conditionedZeroSearchResult, candidates []conditionedGuidedCandidate) {
	remaining := make([]conditionedGuidedCandidate, 0, len(candidates))
	for _, candidate := range candidates {
		if results[candidate.index].result.hits == 0 {
			remaining = append(remaining, candidate)
		}
	}
	sort.Slice(remaining, func(i, j int) bool {
		a := results[remaining[i].index].event.mass
		b := results[remaining[j].index].event.mass
		if a != b {
			return a < b
		}
		return remaining[i].ess > remaining[j].ess
	})
	limit := conditionedGuidedDeepBudget / conditionedZeroDeepRuns
	if limit > len(remaining) {
		limit = len(remaining)
	}
	var wait sync.WaitGroup
	for _, candidate := range remaining[:limit] {
		wait.Add(1)
		go func(index int) {
			defer wait.Done()
			cell := cells[index]
			search := results[index]
			event := search.event
			if event.mass < 1e-9 {
				for count := 5; count >= 4; count-- {
					blockers := conditionedZeroBlockers(cell.id, cell.rank, teams,
						current, pmfs, group.Games, count)
					if deeper, built := buildConditionedPointEventWithLimit(cell.id,
						cell.rank, blockers, group, campaign, table, bounds,
						conditionedZeroDeepStates); built && deeper.mass > 0 {
						event = deeper
						break
					}
				}
			}
			deepSeed := deriveRarePositionSeed(seed,
				fmt.Sprintf("conditioned-guided-deep-%d-%d", cell.id, cell.rank))
			deep := sampleConditionedZeroCellFast(event, cell.id, cell.rank,
				group, campaign, table, sortOrder, samplers, conditionedZeroDeepRuns, deepSeed)
			deep.work += search.result.work
			deep.blockers = len(event.teams) - 1
			search.result = deep
			search.event = event
			results[index] = search
		}(candidate.index)
	}
	wait.Wait()
}
