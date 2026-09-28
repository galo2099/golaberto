package main

import (
	"math/rand"
	"sort"
)

type conditionedFirstPointGame struct {
	index              int
	home, away         int32
	prob               [3]float64
	homeGain, awayGain [3]int
}

type conditionedFirstScoreContext struct {
	group       *GroupType
	campaign    []*TeamCampaign
	table       *Table
	sortOrder   []SortType
	samplers    []conditionedScoreSampler
	simCampaign []*TeamCampaign
	teamSlice   []*TeamCampaign
	simGames    []GameType
}

func newConditionedFirstScoreContext(group *GroupType, campaign []*TeamCampaign,
	table *Table, sortOrder []SortType, samplers []conditionedScoreSampler) *conditionedFirstScoreContext {
	return &conditionedFirstScoreContext{
		group: group, campaign: campaign, table: table, sortOrder: sortOrder, samplers: samplers,
		simCampaign: make([]*TeamCampaign, len(campaign)),
		teamSlice:   make([]*TeamCampaign, len(group.Team_groups)),
		simGames:    make([]GameType, len(group.Games)),
	}
}

func (context *conditionedFirstScoreContext) finishesFirst(target int, outcomes []uint8,
	rng *rand.Rand) bool {
	return context.finishesAtRank(target, 0, outcomes, rng)
}

func (context *conditionedFirstScoreContext) finishesAtRank(target, rank int,
	outcomes []uint8, rng *rand.Rand) bool {
	for index, team := range context.campaign {
		context.simCampaign[index] = cloneCampaignInto(context.simCampaign[index], team)
	}
	for index, game := range context.group.Games {
		if game.Played {
			continue
		}
		home, away := context.samplers[index].sample(rng, outcomes[index])
		score := &context.simGames[index]
		*score = GameType{Id: game.Id, HomeId: game.HomeId, AwayId: game.AwayId,
			HomeScore: home, AwayScore: away, Played: true,
			home_table_index: game.home_table_index, away_table_index: game.away_table_index}
		addSimulatedGame(context.simCampaign[game.home_table_index],
			context.simCampaign[game.away_table_index], score)
	}
	for index, team := range context.group.Team_groups {
		context.teamSlice[index] = context.simCampaign[context.table.Query(uint32(team.Team_id))]
	}
	sort.Sort(TeamCampaignSorted{t: context.teamSlice, sort: context.sortOrder, rng: rng})
	return context.teamSlice[rank].id == target
}

// sampleConditionedZeroFirstCellFast resolves most first-place draws using
// points. It only generates scorelines when the target ties another team on
// points; every target fixture belongs to the conditioned event.
func sampleConditionedZeroFirstCellFast(event *conditionedPointEvent, target int,
	group *GroupType, campaign []*TeamCampaign, table *Table, sortOrder []SortType,
	samplers []conditionedScoreSampler, samples int, seed int64) conditionedZeroResult {
	result := conditionedZeroResult{mass: event.mass}
	if event.mass <= 0 || samples <= 0 {
		return result
	}
	for _, team := range campaign {
		if team != nil && (team.points_win < 0 || team.points_draw < 0 || team.points_loss < 0) {
			return sampleConditionedZeroCell(event, target, 0, group, campaign, table,
				sortOrder, samplers, samples, seed)
		}
	}
	selected := make([]bool, len(group.Games))
	for _, game := range event.games {
		selected[game.index] = true
	}
	targetIndex := table.Query(uint32(target))
	ranked := make([]bool, len(campaign))
	for _, team := range group.Team_groups {
		ranked[table.Query(uint32(team.Team_id))] = true
	}
	var pointGames []conditionedFirstPointGame
	var remaining []conditionedFirstPointGame
	for index, game := range group.Games {
		if game.Played {
			continue
		}
		if !ranked[game.home_table_index] || !ranked[game.away_table_index] ||
			campaign[game.home_table_index] == nil || campaign[game.away_table_index] == nil ||
			game.home_table_index == game.away_table_index {
			return sampleConditionedZeroCell(event, target, 0, group, campaign, table,
				sortOrder, samplers, samples, seed)
		}
		if !selected[index] && (game.home_table_index == targetIndex || game.away_table_index == targetIndex) {
			return sampleConditionedZeroCell(event, target, 0, group, campaign, table,
				sortOrder, samplers, samples, seed)
		}
		home := campaign[game.home_table_index]
		away := campaign[game.away_table_index]
		pointGame := conditionedFirstPointGame{
			index: index, home: game.home_table_index, away: game.away_table_index,
			prob:     targetOutcomeProbabilities(game),
			homeGain: [3]int{home.points_loss, home.points_draw, home.points_win},
			awayGain: [3]int{away.points_win, away.points_draw, away.points_loss},
		}
		if selected[index] {
			pointGames = append(pointGames, pointGame)
		} else {
			remaining = append(remaining, pointGame)
		}
	}
	basePoints := make([]int, len(campaign))
	for index, team := range campaign {
		if team != nil {
			basePoints[index] = team.points
		}
	}
	points := make([]int, len(campaign))
	outcomes := make([]uint8, len(group.Games))
	scoreContext := newConditionedFirstScoreContext(group, campaign, table, sortOrder, samplers)
	rng := rand.New(rand.NewSource(seed))
	for n := 0; n < samples; n++ {
		event.sampleOutcomes(rng, outcomes)
		copy(points, basePoints)
		for _, game := range pointGames {
			outcome := outcomes[game.index]
			points[game.home] += game.homeGain[outcome]
			points[game.away] += game.awayGain[outcome]
		}
		targetPoints := points[targetIndex]
		rejected := false
		for _, team := range group.Team_groups {
			if team.Team_id != target && points[table.Query(uint32(team.Team_id))] > targetPoints {
				rejected = true
				break
			}
		}
		if !rejected {
			for _, game := range remaining {
				u := rng.Float64()
				outcome := uint8(2)
				if u < game.prob[0] {
					outcome = 0
				} else if u < game.prob[0]+game.prob[1] {
					outcome = 1
				}
				outcomes[game.index] = outcome
				points[game.home] += game.homeGain[outcome]
				points[game.away] += game.awayGain[outcome]
				if points[game.home] > targetPoints || points[game.away] > targetPoints {
					rejected = true
					break
				}
			}
		}
		result.samples++
		if rejected {
			continue
		}
		tied := false
		for _, team := range group.Team_groups {
			if team.Team_id != target && points[table.Query(uint32(team.Team_id))] == targetPoints {
				tied = true
				break
			}
		}
		if !tied {
			result.hits++
			continue
		}
		if scoreContext.finishesFirst(target, outcomes, rng) {
			result.hits++
		}
	}
	result.work = int64(result.samples) * estimateSeasonWork(
		len(group.Games)-countPlayedGames(group.Games), 1, len(group.Team_groups))
	return result
}
