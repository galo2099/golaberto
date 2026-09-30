package main

import (
	"math/rand"
	"os"
	"sort"
)

type conditionedPointOutcomeGame struct {
	index              int
	home, away         int32
	prob               [3]float64
	homeGain, awayGain [3]int
}

func conditionedWinAwareRankMode() string {
	mode := os.Getenv("RARE_POSITION_WIN_AWARE_RANK")
	if mode == "" {
		return "lookahead"
	}
	return mode
}

// A points/wins prefix can be represented as one ordered integer. The stride
// exceeds every attainable win total, so later tiebreakers only matter when
// the encoded values are equal.
func conditionedRankWinStride(sortOrder []SortType, group *GroupType, campaign []*TeamCampaign,
	table *Table, stage string) int {
	mode := conditionedWinAwareRankMode()
	if (mode != "1" && mode != stage) ||
		len(sortOrder) < 2 || sortOrder[0] != PT || sortOrder[1] != W {
		return 1
	}
	remaining := make(map[int]int, len(group.Team_groups))
	for _, game := range group.Games {
		if !game.Played {
			remaining[game.HomeId]++
			remaining[game.AwayId]++
		}
	}
	maximum := 0
	for _, team := range group.Team_groups {
		c := campaign[table.Query(uint32(team.Team_id))]
		if c == nil || c.wins < 0 {
			return 1
		}
		if total := c.wins + remaining[team.Team_id]; total > maximum {
			maximum = total
		}
	}
	return maximum + 1
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
	return context.finishesAtRankReduced(target, rank, outcomes, rng, nil)
}

func (context *conditionedRankScoreContext) finishesAtRankReduced(target, rank int,
	outcomes []uint8, rng *rand.Rand, omitted []bool) bool {
	for index, team := range context.campaign {
		context.simCampaign[index] = cloneCampaignInto(context.simCampaign[index], team)
	}
	for index, game := range context.group.Games {
		if game.Played {
			continue
		}
		var home, away int
		if index < len(omitted) && omitted[index] {
			// Neither side can tie the target's ordered prefix. These goals
			// cannot affect its rank; retain a legal representative score.
			if outcomes[index] == 0 {
				away = 1
			} else if outcomes[index] == 2 {
				home = 1
			}
		} else {
			home, away = context.samplers[index].sample(rng, outcomes[index])
		}
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
