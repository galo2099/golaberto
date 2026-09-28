package main

import (
	"math/rand"
	"sort"
)

type conditionedPointOutcomeGame struct {
	index              int
	home, away         int32
	prob               [3]float64
	homeGain, awayGain [3]int
}

type conditionedRankScoreContext struct {
	group       *GroupType
	campaign    []*TeamCampaign
	table       *Table
	sortOrder   []SortType
	samplers    []conditionedScoreSampler
	simCampaign []*TeamCampaign
	teamSlice   []*TeamCampaign
	simGames    []GameType
}

func newConditionedRankScoreContext(group *GroupType, campaign []*TeamCampaign,
	table *Table, sortOrder []SortType, samplers []conditionedScoreSampler) *conditionedRankScoreContext {
	return &conditionedRankScoreContext{
		group: group, campaign: campaign, table: table, sortOrder: sortOrder, samplers: samplers,
		simCampaign: make([]*TeamCampaign, len(campaign)),
		teamSlice:   make([]*TeamCampaign, len(group.Team_groups)),
		simGames:    make([]GameType, len(group.Games)),
	}
}

func (context *conditionedRankScoreContext) finishesAtRank(target, rank int,
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
