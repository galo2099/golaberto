package main

import (
	"fmt"
	"log"
	"math/rand"
	"os"
	"sort"
	"strings"
	"sync"
	"time"
)

const (
	conditionedDomainPilotCells = 24
	conditionedDomainPilotDraws = 1000
	conditionedDomainFinalCells = 3
	conditionedDomainFinalDraws = 15000
	conditionedDomainCheckDraws = 5000
)

func conditionedDomainRescueEnabled() bool {
	return os.Getenv("RARE_POSITION_PROPAGATED_DOMAINS") != "0"
}

// Preserve the existing search and spend a bounded extra budget only on its
// remaining zeros. Team IDs and ranks are never special-cased.
func runConditionedDomainRescue(group *GroupType, campaign []*TeamCampaign, table *Table,
	order []SortType, seed int64, bounds pointRankBounds, samplers []conditionedScoreSampler,
	estimates map[int]map[int]ProductionEstimate) (int, int64) {
	if !conditionedDomainRescueEnabled() || len(order) == 0 || order[0] != PT {
		return 0, 0
	}
	started := time.Now()
	standing := make([]*TeamCampaign, 0, len(group.Team_groups))
	for _, team := range group.Team_groups {
		standing = append(standing, campaign[table.Query(uint32(team.Team_id))])
	}
	sort.Sort(TeamCampaignSorted{t: standing, sort: order, rng: rand.New(rand.NewSource(1))})
	current := make(map[int]int, len(standing))
	for rank, team := range standing {
		current[team.id] = rank
	}
	type candidate struct {
		cell      conditionedZeroCell
		event     *conditionedPointEvent
		pointTilt float64
		pilot     conditionedZeroResult
	}
	var candidates []candidate
	for _, team := range group.Team_groups {
		for rank := range group.Team_groups {
			est := estimates[team.Team_id][rank]
			if est.Probability != 0 || strings.HasPrefix(est.Reachability, "impossible") {
				continue
			}
			candidates = append(candidates, candidate{cell: conditionedZeroCell{team.Team_id, rank}})
		}
	}
	sort.Slice(candidates, func(i, j int) bool {
		a, b := candidates[i].cell, candidates[j].cell
		ua, ub := estimates[a.id][a.rank].ZeroHitUpper95, estimates[b.id][b.rank].ZeroHitUpper95
		if ua != ub {
			return ua > ub
		}
		if a.id != b.id {
			return a.id < b.id
		}
		return a.rank < b.rank
	})
	if len(candidates) > conditionedDomainPilotCells {
		candidates = candidates[:conditionedDomainPilotCells]
	}
	if len(candidates) == 0 {
		return 0, 0
	}
	parallel := func(count int, run func(int)) {
		jobs := make(chan int, count)
		var wait sync.WaitGroup
		for worker := 0; worker < conditionedGuidedWorkers(count); worker++ {
			wait.Add(1)
			go func() {
				defer wait.Done()
				for index := range jobs {
					run(index)
				}
			}()
		}
		for index := 0; index < count; index++ {
			jobs <- index
		}
		close(jobs)
		wait.Wait()
	}
	parallel(len(candidates), func(index int) {
		c := &candidates[index]
		var ok bool
		c.event, ok = buildConditionedPointEvent(c.cell.id, c.cell.rank, nil, group, campaign, table, bounds)
		if !ok || c.event.mass <= 0 {
			return
		}
		if !conditionedDomainUniformSide(c.event, c.cell, group, campaign, table, order, bounds) {
			return
		}
		c.pointTilt = 0.5
		if c.cell.rank > current[c.cell.id] {
			c.pointTilt = -0.5
		}
		stream := deriveRarePositionSeed(seed, fmt.Sprintf("rank-domain-pilot-%d-%d", c.cell.id, c.cell.rank))
		c.pilot, _ = sampleConditionedZeroRankWithDomains(c.event, c.cell.id, c.cell.rank, group, campaign, table, order, bounds, samplers, conditionedDomainPilotDraws, stream, 6, c.pointTilt)
	})
	var work int64
	var finalists []candidate
	for _, c := range candidates {
		work += c.pilot.work
		if c.pilot.hits > 0 && estimates[c.cell.id][c.cell.rank].Reachability == "undecided" {
			est := estimates[c.cell.id][c.cell.rank]
			est.Reachability = "reachable_by_construction"
			estimates[c.cell.id][c.cell.rank] = est
		}
		if c.pilot.hits >= 3 && c.pilot.ess >= 3 {
			finalists = append(finalists, c)
		}
	}
	sort.Slice(finalists, func(i, j int) bool {
		if finalists[i].pilot.ess != finalists[j].pilot.ess {
			return finalists[i].pilot.ess > finalists[j].pilot.ess
		}
		if finalists[i].cell.id != finalists[j].cell.id {
			return finalists[i].cell.id < finalists[j].cell.id
		}
		return finalists[i].cell.rank < finalists[j].cell.rank
	})
	if len(finalists) > conditionedDomainFinalCells {
		finalists = finalists[:conditionedDomainFinalCells]
	}
	type confirmation struct{ selected, check conditionedZeroResult }
	confirmed := make([]confirmation, len(finalists))
	parallel(len(finalists), func(index int) {
		c := finalists[index]
		stream := deriveRarePositionSeed(seed, fmt.Sprintf("rank-domain-final-%d-%d", c.cell.id, c.cell.rank))
		confirmed[index].selected, _ = sampleConditionedZeroRankWithDomains(c.event, c.cell.id, c.cell.rank, group, campaign, table, order, bounds, samplers, conditionedDomainFinalDraws, stream, 6, c.pointTilt)
		stream = deriveRarePositionSeed(seed, fmt.Sprintf("rank-domain-check-%d-%d", c.cell.id, c.cell.rank))
		confirmed[index].check, _ = sampleConditionedZeroRankWithDomains(c.event, c.cell.id, c.cell.rank, group, campaign, table, order, bounds, samplers, conditionedDomainCheckDraws, stream, 3, c.pointTilt)
	})
	accepted := 0
	for index, c := range finalists {
		result, check := confirmed[index].selected, confirmed[index].check
		work += result.work + check.work
		result, valid := conditionedDomainConfirmation(result, check)
		if valid {
			estimates[c.cell.id][c.cell.rank] = applyConditionedPointTiltEstimate(estimates[c.cell.id][c.cell.rank], result, "matched_point_pool_conditioned_rank_domains")
			accepted++
		}
		log.Printf("rare-position-rank-domains-cell: group=%d team=%d rank=%d hits=%d ess=%.1f p=%g check_hits=%d check_p=%g accepted=%t", group.Id, c.cell.id, c.cell.rank+1, result.hits, result.ess, result.probability, check.hits, check.probability, valid)
	}
	log.Printf("rare-position-rank-domains: group=%d pilots=%d finals=%d accepted=%d work=%d elapsed=%s", group.Id, len(candidates), len(finalists), accepted, work, time.Since(started))
	return accepted, work
}

func conditionedDomainConfirmation(result, check conditionedZeroResult) (conditionedZeroResult, bool) {
	if conditionedPointTiltResultValid(check) {
		return check, true
	}
	if !conditionedPointTiltResultValid(result) {
		return result, false
	}
	result, valid, _ := crossCheckExtremePointTilt(result, check)
	return result, valid
}

// This bounded rescue targets cells with a necessarily occupied rank side
// throughout their attainable target totals. Partly constrained cells can
// contain separate dominant modes that a short moderate-tilt run misses.
func conditionedDomainUniformSide(event *conditionedPointEvent, cell conditionedZeroCell,
	group *GroupType, campaign []*TeamCampaign, table *Table, order []SortType, bounds pointRankBounds) bool {
	stride := conditionedRankWinStride(order, group, campaign, table, "lookahead")
	baseWins := 0
	winRanges := map[int][2]int{0: {0, 0}}
	if stride > 1 {
		baseWins = campaign[table.Query(uint32(cell.id))].wins
		for _, game := range event.games {
			next := make(map[int][2]int)
			for points, wins := range winRanges {
				for outcome, p := range game.prob {
					if p <= 0 {
						continue
					}
					win := 0
					if group.Games[game.index].HomeId == cell.id && outcome == 2 || group.Games[game.index].AwayId == cell.id && outcome == 0 {
						win = 1
					}
					added := points + int(game.deltas[outcome][0])
					lo, hi := wins[0]+win, wins[1]+win
					if previous, found := next[added]; found {
						lo, hi = min(lo, previous[0]), max(hi, previous[1])
					}
					next[added] = [2]int{lo, hi}
				}
			}
			winRanges = next
		}
	}
	minimum, maximum := int(^uint(0)>>1), -int(^uint(0)>>1)
	for _, terminal := range event.terminal {
		added := int(terminal.state[0])
		lo, hi := 0, 0
		if stride > 1 {
			lo, hi = winRanges[added][0], winRanges[added][1]
		}
		points := bounds.current[cell.id] + added
		minimum = min(minimum, points*stride+baseWins+lo)
		maximum = max(maximum, points*stride+baseWins+hi)
	}
	remaining := make(map[int]int)
	if stride > 1 {
		for _, game := range group.Games {
			if !game.Played {
				remaining[game.HomeId]++
				remaining[game.AwayId]++
			}
		}
	}
	above, below := 0, 0
	for _, team := range group.Team_groups {
		id := team.Team_id
		if id == cell.id {
			continue
		}
		lo, hi := bounds.minimum[id]*stride, bounds.maximum[id]*stride
		if stride > 1 {
			wins := campaign[table.Query(uint32(id))].wins
			lo += wins
			hi += wins + remaining[id]
		}
		if lo > maximum {
			above++
		}
		if hi < minimum {
			below++
		}
	}
	return above == cell.rank || below == len(group.Team_groups)-1-cell.rank
}
