package main

import (
	"fmt"
	"log"
	"math"
	"math/rand"
	"os"
	"runtime"
	"sort"
	"sync"
)

// The scout's zero cells are the only candidates. Conditioning is exact for
// points: the event contains every finish at the requested rank, including
// finishes decided by a score or head-to-head tiebreaker.
const (
	conditionedZeroMaxStates   = 20000
	conditionedZeroDeepStates  = 250000
	conditionedZeroDeepRuns    = 1000000
	conditionedZeroSamples     = 2000
	conditionedZeroExtraRuns   = 50000
	conditionedZeroExtraBudget = 200000
	conditionedZeroExtraStates = 120000
	conditionedZeroExtraFloor  = 1e-12
	conditionedZeroBatch       = 1000
)

type conditionedPointState [6]uint8

type conditionedPointGame struct {
	index  int
	prob   [3]float64 // home loss, draw, home win
	deltas [3]conditionedPointState
}

type conditionedPointTerminal struct {
	state      conditionedPointState
	cumulative float64
}

type conditionedPointEvent struct {
	teams    []int
	games    []conditionedPointGame
	forward  []map[conditionedPointState]float64
	terminal []conditionedPointTerminal
	mass     float64
}

type conditionedScore struct{ home, away int }

type conditionedScoreSampler struct {
	home, away poissonScoreSampler
	scores     [3][]conditionedScore
	cdf        [3][]float64
}

type conditionedZeroResult struct {
	mass            float64
	samples         int
	hits            int
	blockers        int
	work            int64
	weighted        bool
	probability     float64
	stdErr          float64
	ess             float64
	maxWeightShare  float64
	batchGap        float64
	witnessOutcomes []uint8
}

type conditionedZeroSearchResult struct {
	result          conditionedZeroResult
	impossible      bool
	event           *conditionedPointEvent
	witnessOutcomes []uint8
}

type conditionedZeroCell struct{ id, rank int }

type conditionedExtraCell struct {
	index int
	event *conditionedPointEvent
	gain  float64
}

func conditionedRankBlockerNeed(rank, teams int) int {
	needed := rank + 1
	if opposite := teams - rank; opposite < needed {
		needed = opposite
	}
	return needed
}

func conditionedZeroEnabled() bool {
	return os.Getenv("RARE_POSITION_CONDITIONED_ZERO") != "0"
}

func conditionedZeroDeepEnabled() bool {
	return os.Getenv("RARE_POSITION_CONDITIONED_ZERO_DEEP") == "1"
}

func conditionedZeroLookaheadEnabled() bool {
	return os.Getenv("RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD") != "0"
}

func conditionedZeroBlockers(target, rank int, teams []int, current map[int]int,
	pmfs map[int]map[int]float64, games []*GameType, count int) []int {
	if count <= 0 {
		return nil
	}
	type candidate struct {
		id    int
		score float64
	}
	others := make([]candidate, 0, len(teams)-1)
	top := rank+1 <= len(teams)-rank
	for _, id := range teams {
		if id == target {
			continue
		}
		mean := float64(current[id])
		for added, p := range pmfs[id] {
			mean += float64(added) * p
		}
		score := mean + 0.04*float64(current[id])
		if !top {
			score = -mean - 0.04*float64(current[id])
		}
		others = append(others, candidate{id, score})
	}
	if count > len(others) {
		count = len(others)
	}
	blockers := make([]int, 0, count)
	used := make([]bool, len(others))
	for len(blockers) < count {
		best, bestScore := -1, math.Inf(-1)
		for i, candidate := range others {
			if used[i] {
				continue
			}
			score := candidate.score
			// Rival-vs-rival fixtures make their simultaneous extremes
			// less useful as a conditioning event.
			if len(blockers) >= 2 {
				for _, game := range games {
					if game.Played {
						continue
					}
					for _, selected := range blockers {
						if (game.HomeId == candidate.id && game.AwayId == selected) ||
							(game.AwayId == candidate.id && game.HomeId == selected) {
							score -= 1
						}
					}
				}
			}
			if score > bestScore || score == bestScore && (best < 0 || candidate.id < others[best].id) {
				best, bestScore = i, score
			}
		}
		blockers = append(blockers, others[best].id)
		used[best] = true
	}
	return blockers
}

func buildConditionedPointEvent(target, rank int, blockers []int, group *GroupType,
	campaign []*TeamCampaign, table *Table, bounds pointRankBounds) (*conditionedPointEvent, bool) {
	return buildConditionedPointEventWithLimit(target, rank, blockers, group,
		campaign, table, bounds, conditionedZeroMaxStates)
}

func buildConditionedPointEventWithLimit(target, rank int, blockers []int, group *GroupType,
	campaign []*TeamCampaign, table *Table, bounds pointRankBounds, maxStates int) (*conditionedPointEvent, bool) {
	selected := append([]int{target}, blockers...)
	indices := make(map[int]int, len(selected))
	for i, id := range selected {
		indices[id] = i
	}
	event := &conditionedPointEvent{teams: selected}
	targetMaximum := bounds.maximum[target]
	rivalCurrent := make([]int, len(blockers))
	for i, id := range blockers {
		rivalCurrent[i] = bounds.current[id]
	}
	for gameIndex, game := range group.Games {
		if game.Played {
			continue
		}
		homeSlot, homeSelected := indices[game.HomeId]
		awaySlot, awaySelected := indices[game.AwayId]
		if !homeSelected && !awaySelected {
			continue
		}
		pointGame := conditionedPointGame{index: gameIndex, prob: targetOutcomeProbabilities(game)}
		for outcome := 0; outcome < 3; outcome++ {
			if homeSelected {
				home := campaign[table.Query(uint32(game.HomeId))]
				if home == nil {
					return nil, false
				}
				gain := home.points_draw
				if outcome == 0 {
					gain = home.points_loss
				} else if outcome == 2 {
					gain = home.points_win
				}
				if gain < 0 || gain > 255 {
					return nil, false
				}
				pointGame.deltas[outcome][homeSlot] = uint8(gain)
			}
			if awaySelected {
				away := campaign[table.Query(uint32(game.AwayId))]
				if away == nil {
					return nil, false
				}
				gain := away.points_draw
				if outcome == 0 {
					gain = away.points_win
				} else if outcome == 2 {
					gain = away.points_loss
				}
				if gain < 0 || gain > 255 {
					return nil, false
				}
				pointGame.deltas[outcome][awaySlot] = uint8(gain)
			}
		}
		event.games = append(event.games, pointGame)
	}
	event.forward = []map[conditionedPointState]float64{{{}: 1}}
	for _, game := range event.games {
		previous := event.forward[len(event.forward)-1]
		next := make(map[conditionedPointState]float64, len(previous)*2)
		for state, mass := range previous {
			for outcome, p := range game.prob {
				if p <= 0 {
					continue
				}
				newState := state
				valid := true
				for i := range selected {
					if int(state[i])+int(game.deltas[outcome][i]) > 255 {
						valid = false
						break
					}
					newState[i] += game.deltas[outcome][i]
				}
				if valid && len(selected) > 1 {
					forcedAbove := 0
					for slot, current := range rivalCurrent {
						if current+int(newState[slot+1]) > targetMaximum {
							forcedAbove++
						}
					}
					if forcedAbove > rank {
						valid = false
					}
				}
				if valid {
					next[newState] += mass * p
				}
			}
		}
		if len(next) > maxStates {
			return nil, false
		}
		event.forward = append(event.forward, next)
	}
	for state, p := range event.forward[len(event.forward)-1] {
		targetPoints := bounds.current[target] + int(state[0])
		if !bounds.rankNotRuledOut(target, rank, int(state[0])) {
			continue
		}
		above, below := 0, 0
		for slot, id := range selected[1:] {
			points := bounds.current[id] + int(state[slot+1])
			if points > targetPoints {
				above++
			} else if points < targetPoints {
				below++
			}
		}
		if above > rank || below > len(bounds.teams)-1-rank {
			continue
		}
		event.mass += p
		event.terminal = append(event.terminal, conditionedPointTerminal{state, event.mass})
	}
	if event.mass <= 0 {
		return event, true
	}
	// Stable terminal order makes seeded results reproducible across Go map layouts.
	sort.Slice(event.terminal, func(i, j int) bool {
		for slot := range event.teams {
			if event.terminal[i].state[slot] != event.terminal[j].state[slot] {
				return event.terminal[i].state[slot] < event.terminal[j].state[slot]
			}
		}
		return false
	})
	event.mass = 0
	last := event.forward[len(event.forward)-1]
	for i := range event.terminal {
		event.mass += last[event.terminal[i].state]
		event.terminal[i].cumulative = event.mass
	}
	return event, true
}

func (event *conditionedPointEvent) sampleOutcomes(rng *rand.Rand, outcomes []uint8) {
	u := rng.Float64() * event.mass
	terminal := sort.Search(len(event.terminal), func(i int) bool { return event.terminal[i].cumulative >= u })
	if terminal == len(event.terminal) {
		terminal--
	}
	event.sampleOutcomesFromTerminal(rng, outcomes, terminal)
}

func (event *conditionedPointEvent) sampleOutcomesFromTerminal(rng *rand.Rand, outcomes []uint8, terminal int) {
	state := event.terminal[terminal].state
	for i := len(event.games) - 1; i >= 0; i-- {
		game := event.games[i]
		var weights [3]float64
		var previous [3]conditionedPointState
		for outcome := 0; outcome < 3; outcome++ {
			valid := true
			previous[outcome] = state
			for slot := range event.teams {
				if previous[outcome][slot] < game.deltas[outcome][slot] {
					valid = false
					break
				}
				previous[outcome][slot] -= game.deltas[outcome][slot]
			}
			if valid {
				weights[outcome] = event.forward[i][previous[outcome]] * game.prob[outcome]
			}
		}
		u := rng.Float64() * (weights[0] + weights[1] + weights[2])
		outcome := 2
		if u < weights[0] {
			outcome = 0
		} else if u < weights[0]+weights[1] {
			outcome = 1
		}
		outcomes[game.index] = uint8(outcome)
		state = previous[outcome]
	}
}

func newConditionedScoreSamplers(games []*GameType) []conditionedScoreSampler {
	samplers := make([]conditionedScoreSampler, len(games))
	for i, game := range games {
		if game.Played {
			continue
		}
		sampler := &samplers[i]
		sampler.home = newPoissonScoreSampler(game.HomePower)
		sampler.away = newPoissonScoreSampler(game.AwayPower)
		homeMass := poissonOutcomeMass(game.HomePower)
		awayMass := poissonOutcomeMass(game.AwayPower)
		for home, hp := range homeMass {
			for away, ap := range awayMass {
				outcome := targetOutcomeDigit(true, home, away)
				sampler.scores[outcome] = append(sampler.scores[outcome], conditionedScore{home, away})
				cdf := sampler.cdf[outcome]
				cumulative := hp * ap
				if len(cdf) > 0 {
					cumulative += cdf[len(cdf)-1]
				}
				sampler.cdf[outcome] = append(cdf, cumulative)
			}
		}
		for outcome := range sampler.cdf {
			cdf := sampler.cdf[outcome]
			if len(cdf) > 0 {
				for j := range cdf {
					cdf[j] /= cdf[len(cdf)-1]
				}
				cdf[len(cdf)-1] = 1
			}
		}
	}
	return samplers
}

func (sampler *conditionedScoreSampler) sample(rng *rand.Rand, outcome uint8) (int, int) {
	cdf := sampler.cdf[outcome]
	if len(cdf) == 0 {
		return sampler.home.sample(rng), sampler.away.sample(rng)
	}
	u := rng.Float64()
	i := sort.Search(len(cdf), func(i int) bool { return cdf[i] >= u })
	if i == len(cdf) {
		i--
	}
	score := sampler.scores[outcome][i]
	return score.home, score.away
}

func sampleConditionedZeroCell(event *conditionedPointEvent, target, rank int,
	group *GroupType, campaign []*TeamCampaign, table *Table, sortOrder []SortType,
	samplers []conditionedScoreSampler, samples int, seed int64) conditionedZeroResult {
	result := conditionedZeroResult{mass: event.mass}
	if event.mass <= 0 || samples <= 0 {
		return result
	}
	rng := rand.New(rand.NewSource(seed))
	slots := make([]bool, len(group.Games))
	for _, game := range event.games {
		slots[game.index] = true
	}
	outcomes := make([]uint8, len(group.Games))
	simCampaign := make([]*TeamCampaign, len(campaign))
	teamSlice := make([]*TeamCampaign, len(group.Team_groups))
	simGames := make([]GameType, len(group.Games))
	for n := 0; n < samples; n++ {
		event.sampleOutcomes(rng, outcomes)
		for index, team := range campaign {
			simCampaign[index] = cloneCampaignInto(simCampaign[index], team)
		}
		for index, game := range group.Games {
			if game.Played {
				continue
			}
			var home, away int
			if slots[index] {
				home, away = samplers[index].sample(rng, outcomes[index])
			} else {
				home = samplers[index].home.sample(rng)
				away = samplers[index].away.sample(rng)
				if home < away {
					outcomes[index] = 0
				} else if home == away {
					outcomes[index] = 1
				} else {
					outcomes[index] = 2
				}
			}
			score := &simGames[index]
			*score = GameType{Id: game.Id, HomeId: game.HomeId, AwayId: game.AwayId,
				HomeScore: home, AwayScore: away, Played: true,
				home_table_index: game.home_table_index, away_table_index: game.away_table_index}
			addSimulatedGame(simCampaign[game.home_table_index], simCampaign[game.away_table_index], score)
		}
		for i, team := range group.Team_groups {
			teamSlice[i] = simCampaign[table.Query(uint32(team.Team_id))]
		}
		sort.Sort(TeamCampaignSorted{t: teamSlice, sort: sortOrder, rng: rng})
		if teamSlice[rank].id == target {
			result.hits++
			if result.witnessOutcomes == nil {
				result.witnessOutcomes = append([]uint8(nil), outcomes...)
			}
		}
		result.samples++
	}
	result.work = int64(result.samples) * estimateSeasonWork(len(group.Games)-countPlayedGames(group.Games), 1, len(group.Team_groups))
	return result
}

func countPlayedGames(games []*GameType) int {
	count := 0
	for _, game := range games {
		if game.Played {
			count++
		}
	}
	return count
}

func searchConditionedZeroCell(target, rank int, group *GroupType,
	campaign []*TeamCampaign, table *Table, sortOrder []SortType, seed int64,
	teams []int, current map[int]int, pmfs map[int]map[int]float64,
	bounds pointRankBounds, samplers []conditionedScoreSampler) conditionedZeroSearchResult {
	var event *conditionedPointEvent
	var ok bool
	// The fewer rivals needed to overfill one side of the rank, the more
	// useful it is to condition on them now. Defer deeper events to the
	// extra-search planner when the narrower side needs three or more.
	needed := conditionedRankBlockerNeed(rank, len(teams))
	blockerCount := 0
	if needed < 3 {
		blockerCount = 4 - needed
	}
	for ; blockerCount >= 0; blockerCount-- {
		blockers := conditionedZeroBlockers(target, rank, teams, current, pmfs, group.Games, blockerCount)
		event, ok = buildConditionedPointEvent(target, rank, blockers,
			group, campaign, table, bounds)
		if ok {
			break
		}
	}
	if !ok {
		return conditionedZeroSearchResult{}
	}
	if event.mass <= 0 {
		return conditionedZeroSearchResult{impossible: len(event.terminal) == 0, event: event}
	}
	samples := conditionedZeroSamples
	if event.mass < 1e-7 || event.mass > 1e-3 {
		samples = conditionedZeroBatch
	}
	cellSeed := deriveRarePositionSeed(seed, fmt.Sprintf("conditioned-zero-%d-%d", target, rank))
	result := sampleConditionedZeroCellFast(event, target, rank,
		group, campaign, table, sortOrder, samplers, samples, cellSeed)
	result.blockers = len(event.teams) - 1
	return conditionedZeroSearchResult{result: result, event: event, witnessOutcomes: result.witnessOutcomes}
}

// Gain measures how many log units of a cell's 95% zero-hit upper bound a
// fixed extra batch could remove. The floor stops already tiny bounds from
// consuming the request's search budget.
func conditionedExtraGain(currentUpper, eventMass float64) float64 {
	if currentUpper <= conditionedZeroExtraFloor || eventMass <= 0 {
		return 0
	}
	newUpper := eventMass * zeroHitUpper95(conditionedZeroExtraRuns)
	if newUpper >= currentUpper {
		return 0
	}
	if newUpper < conditionedZeroExtraFloor {
		newUpper = conditionedZeroExtraFloor
	}
	return math.Log(currentUpper / newUpper)
}

func allocateConditionedZeroExtra(group *GroupType, campaign []*TeamCampaign,
	table *Table, sortOrder []SortType, seed int64, teams []int,
	current map[int]int, pmfs map[int]map[int]float64, bounds pointRankBounds,
	samplers []conditionedScoreSampler, cells []conditionedZeroCell,
	estimates map[int]map[int]ProductionEstimate, results []conditionedZeroSearchResult) {
	planned := make([]conditionedExtraCell, len(cells))
	eligible := make([]bool, len(cells))
	impossible := make([]bool, len(cells))
	planCell := func(index int) {
		cell := cells[index]
		result := results[index].result
		if results[index].impossible || result.samples == 0 || result.hits > 0 || result.mass <= 0 {
			return
		}
		upper := result.mass * zeroHitUpper95(result.samples)
		if prior := estimates[cell.id][cell.rank].ZeroHitUpper95; prior > 0 && prior < upper {
			upper = prior
		}
		if upper <= 10*conditionedZeroExtraFloor {
			return
		}
		mass := result.mass
		var refined *conditionedPointEvent
		// Three tracked rivals can constrain a rank only when three of them
		// could already put the target above or below that rank.
		needed := conditionedRankBlockerNeed(cell.rank, len(teams))
		if len(teams) > 3 && needed <= 3 && result.blockers < 3 && mass <= 1e-3 {
			trial := conditionedZeroBlockers(cell.id, cell.rank, teams, current,
				pmfs, group.Games, 3)
			if deeper, ok := buildConditionedPointEventWithLimit(cell.id, cell.rank,
				trial, group, campaign, table, bounds, conditionedZeroExtraStates); ok {
				if deeper.mass <= 0 {
					impossible[index] = true
					return
				}
				if deeper.mass < mass*(1-1e-8) {
					mass = deeper.mass
					refined = deeper
				}
			}
		}
		if gain := conditionedExtraGain(upper, mass); gain > 0 {
			planned[index] = conditionedExtraCell{index, refined, gain}
			eligible[index] = true
		}
	}
	planningWorkers := runtime.GOMAXPROCS(0)
	if planningWorkers > 4 {
		planningWorkers = 4
	}
	if planningWorkers > len(cells) {
		planningWorkers = len(cells)
	}
	planQueue := make(chan int, len(cells))
	var planWait sync.WaitGroup
	for worker := 0; worker < planningWorkers; worker++ {
		planWait.Add(1)
		go func() {
			defer planWait.Done()
			for index := range planQueue {
				planCell(index)
			}
		}()
	}
	for index := range cells {
		planQueue <- index
	}
	close(planQueue)
	planWait.Wait()
	candidates := make([]conditionedExtraCell, 0, len(cells))
	for index := range cells {
		if impossible[index] {
			results[index].impossible = true
		}
		if eligible[index] {
			candidates = append(candidates, planned[index])
		}
	}
	sort.Slice(candidates, func(i, j int) bool {
		if candidates[i].gain != candidates[j].gain {
			return candidates[i].gain > candidates[j].gain
		}
		a, b := cells[candidates[i].index], cells[candidates[j].index]
		if a.id != b.id {
			return a.id < b.id
		}
		return a.rank < b.rank
	})
	budget := conditionedZeroExtraBudget
	type extraJob struct {
		index int
		cell  conditionedZeroCell
		event *conditionedPointEvent
	}
	jobs := make([]extraJob, 0, budget/conditionedZeroExtraRuns)
	for _, candidate := range candidates {
		if budget < conditionedZeroExtraRuns {
			break
		}
		cell := cells[candidate.index]
		event, ok := candidate.event, candidate.event != nil
		if !ok {
			event, ok = buildConditionedPointEventWithLimit(cell.id, cell.rank,
				nil, group, campaign, table, bounds, conditionedZeroExtraStates)
		}
		if !ok || event.mass <= 0 {
			continue
		}
		jobs = append(jobs, extraJob{candidate.index, cell, event})
		budget -= conditionedZeroExtraRuns
	}
	if len(jobs) == 0 {
		return
	}
	extras := make([]conditionedZeroResult, len(jobs))
	workers := runtime.GOMAXPROCS(0)
	if workers > 4 {
		workers = 4
	}
	if workers > len(jobs) {
		workers = len(jobs)
	}
	queue := make(chan int, len(jobs))
	var wait sync.WaitGroup
	for worker := 0; worker < workers; worker++ {
		wait.Add(1)
		go func() {
			defer wait.Done()
			for index := range queue {
				job := jobs[index]
				extraSeed := deriveRarePositionSeed(seed,
					fmt.Sprintf("conditioned-zero-extra-%d-%d", job.cell.id, job.cell.rank))
				extras[index] = sampleConditionedZeroCellFast(job.event, job.cell.id, job.cell.rank,
					group, campaign, table, sortOrder, samplers, conditionedZeroExtraRuns, extraSeed)
			}
		}()
	}
	for index := range jobs {
		queue <- index
	}
	close(queue)
	wait.Wait()
	for index, job := range jobs {
		extra := extras[index]
		extra.blockers = len(job.event.teams) - 1
		extra.work += results[job.index].result.work
		results[job.index].result = extra
		if results[job.index].witnessOutcomes == nil {
			results[job.index].witnessOutcomes = extra.witnessOutcomes
		}
	}
}

// runConditionedZeroSearch is called after the pool and its uncertainty
// estimates are complete. It only changes point estimates for cells with a
// verified simulated finish, then rebalances the full matrix.
func runConditionedZeroSearch(group *GroupType, campaign []*TeamCampaign, table *Table,
	sortOrder []SortType, seed int64, teams []int, current map[int]int,
	pmfs map[int]map[int]float64, bounds pointRankBounds,
	estimates map[int]map[int]ProductionEstimate) (int, int64) {
	if !conditionedZeroEnabled() {
		return 0, 0
	}
	baseline := make(map[int]map[int]ProductionEstimate, len(teams))
	for _, id := range teams {
		baseline[id] = make(map[int]ProductionEstimate, len(teams))
		for rank := range teams {
			baseline[id][rank] = estimates[id][rank]
		}
	}
	var cells []conditionedZeroCell
	for _, id := range teams {
		for rank := range teams {
			if estimates[id][rank].Probability != 0 {
				continue
			}
			bound, _, impossible := computeHardCellUpperBoundWithBounds(id, rank, pmfs[id], bounds)
			est := estimates[id][rank]
			if impossible || bound <= 0 {
				est.Reachability = "impossible_by_points"
			} else {
				est.Reachability = "undecided"
				cells = append(cells, conditionedZeroCell{id, rank})
			}
			estimates[id][rank] = est
		}
	}
	if len(cells) == 0 {
		return 0, 0
	}
	// Edge positions have the most selective necessary points events.
	sort.Slice(cells, func(i, j int) bool {
		a := cells[i].rank
		b := cells[j].rank
		if len(teams)-1-a < a {
			a = len(teams) - 1 - a
		}
		if len(teams)-1-b < b {
			b = len(teams) - 1 - b
		}
		if a != b {
			return a < b
		}
		if cells[i].id != cells[j].id {
			return cells[i].id < cells[j].id
		}
		return cells[i].rank < cells[j].rank
	})
	totalWork, witnesses := int64(0), 0
	samplers := newConditionedScoreSamplers(group.Games)
	results := make([]conditionedZeroSearchResult, len(cells))
	workers := runtime.GOMAXPROCS(0)
	if workers > 4 {
		workers = 4
	}
	if workers > len(cells) {
		workers = len(cells)
	}
	jobs := make(chan int, len(cells))
	var wait sync.WaitGroup
	for worker := 0; worker < workers; worker++ {
		wait.Add(1)
		go func() {
			defer wait.Done()
			for index := range jobs {
				target := cells[index]
				results[index] = searchConditionedZeroCell(target.id, target.rank,
					group, campaign, table, sortOrder, seed, teams, current, pmfs, bounds, samplers)
			}
		}()
	}
	for index := range cells {
		jobs <- index
	}
	close(jobs)
	wait.Wait()
	runConditionedGuidedSearch(group, campaign, table, sortOrder, seed,
		teams, current, pmfs, bounds, samplers, cells, results)
	allocateConditionedZeroExtra(group, campaign, table, sortOrder, seed,
		teams, current, pmfs, bounds, samplers, cells, estimates, results)
	for index, target := range cells {
		search := results[index]
		if search.impossible {
			totalWork += search.result.work
			est := estimates[target.id][target.rank]
			est.Reachability = "impossible_by_joint_points"
			est.ZeroHitUpper95 = 0
			estimates[target.id][target.rank] = est
			continue
		}
		result := search.result
		if result.samples == 0 {
			continue
		}
		totalWork += result.work
		est := estimates[target.id][target.rank]
		est.ConditionalMass = result.mass
		est.ConditionalSamples = result.samples
		est.ConditionalHits = result.hits
		if result.hits > 0 {
			if result.weighted {
				est.Probability = result.probability
				est.StdErr = result.stdErr
				est.ESS = result.ess
				est.MaxEventWeightShare = result.maxWeightShare
				est.Design = "matched_point_pool_conditioned_lookahead"
				est.MeetsPrecisionGoal = estimateMeetsPrecisionGoal(result.ess,
					result.stdErr/result.probability)
			} else {
				est.Probability = result.mass * float64(result.hits) / float64(result.samples)
				frequency := float64(result.hits) / float64(result.samples)
				est.StdErr = result.mass * math.Sqrt(frequency*(1-frequency)/float64(result.samples))
				est.Design = "matched_point_pool_conditioned"
			}
			est.RelativeSE = relativeSEPointer(est.StdErr / est.Probability)
			est.Reachability = "witness"
			est.ZeroHitUpper95 = 0
			witnesses++
		} else {
			upper := result.mass * zeroHitUpper95(result.samples)
			if upper < est.ZeroHitUpper95 {
				est.ZeroHitUpper95 = upper
			}
		}
		estimates[target.id][target.rank] = est
	}
	if reachabilityNeighborEnabled() {
		proofs, attempts := searchReachabilityNeighbors(group, campaign, table,
			sortOrder, cells, results, estimates, reachabilityNeighborBudget())
		for _, proof := range proofs {
			est := estimates[proof.cell.id][proof.cell.rank]
			est.Reachability = "reachable_by_construction"
			estimates[proof.cell.id][proof.cell.rank] = est
			log.Printf("rare-position-neighborhood-proof: group=%d team=%d rank=%d",
				group.Id, proof.cell.id, proof.cell.rank+1)
		}
		log.Printf("rare-position-neighborhood: group=%d search_cells=%d candidates=%d proofs=%d",
			group.Id, len(results), attempts, len(proofs))
	}
	if jointPointCapEnabled() {
		runJointPointCapProofs(group, campaign, table, sortOrder, cells, estimates)
	}
	if jointPointWitnessEnabled() {
		runJointPointWitnessSearch(group, campaign, table, sortOrder, cells, estimates)
	}
	pointTiltWitnesses, pointTiltWork := runConditionedPointTiltSearch(group,
		campaign, table, sortOrder, seed, bounds, samplers, cells, estimates)
	witnesses += pointTiltWitnesses
	totalWork += pointTiltWork
	if witnesses == 0 {
		return 0, totalWork
	}
	matrix := make(map[int]map[int]float64, len(teams))
	for _, id := range teams {
		matrix[id] = make(map[int]float64, len(teams))
		for rank := range teams {
			matrix[id][rank] = estimates[id][rank].Probability
		}
	}
	if !balanceMatchedPointPool(matrix, teams) {
		log.Printf("rare-position-conditioned-zero: group=%d reconciliation failed", group.Id)
		for _, id := range teams {
			for rank := range teams {
				estimates[id][rank] = baseline[id][rank]
			}
		}
		return 0, totalWork
	}
	for _, id := range teams {
		for rank := range teams {
			est := estimates[id][rank]
			if est.Reachability == "witness" && est.Probability > 0 {
				est.StdErr *= matrix[id][rank] / est.Probability
				est.RelativeSE = relativeSEPointer(est.StdErr / matrix[id][rank])
			}
			est.Probability = matrix[id][rank]
			estimates[id][rank] = est
		}
	}
	return witnesses, totalWork
}
