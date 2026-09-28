package main

import "math/rand"

// sampleConditionedZeroCellFast resolves ranks from points and only generates
// scorelines when teams tied on points can affect the target's rank.
func sampleConditionedZeroCellFast(event *conditionedPointEvent, target, rank int,
	group *GroupType, campaign []*TeamCampaign, table *Table, sortOrder []SortType,
	samplers []conditionedScoreSampler, samples int, seed int64) conditionedZeroResult {
	result := conditionedZeroResult{mass: event.mass}
	if event.mass <= 0 || samples <= 0 {
		return result
	}
	fallback := func() conditionedZeroResult {
		return sampleConditionedZeroCell(event, target, rank, group, campaign, table,
			sortOrder, samplers, samples, seed)
	}
	if len(sortOrder) == 0 || sortOrder[0] != PT {
		return fallback()
	}
	selected := make([]bool, len(group.Games))
	for _, game := range event.games {
		selected[game.index] = true
	}
	targetIndex := table.Query(uint32(target))
	ranked := make([]int32, len(group.Team_groups))
	inGroup := make([]bool, len(campaign))
	for i, team := range group.Team_groups {
		index := table.Query(uint32(team.Team_id))
		ranked[i] = index
		inGroup[index] = true
	}
	basePoints := make([]int, len(campaign))
	for i, team := range campaign {
		if team == nil {
			continue
		}
		if team.points_win < 0 || team.points_draw < 0 || team.points_loss < 0 {
			return fallback()
		}
		basePoints[i] = team.points
	}
	pointGames := make([]conditionedPointOutcomeGame, 0, len(group.Games))
	for index, game := range group.Games {
		if game.Played {
			continue
		}
		homeIndex, awayIndex := game.home_table_index, game.away_table_index
		if !inGroup[homeIndex] || !inGroup[awayIndex] || homeIndex == awayIndex ||
			campaign[homeIndex] == nil || campaign[awayIndex] == nil {
			return fallback()
		}
		home, away := campaign[homeIndex], campaign[awayIndex]
		pointGames = append(pointGames, conditionedPointOutcomeGame{
			index: index, home: homeIndex, away: awayIndex,
			prob:     targetOutcomeProbabilities(game),
			homeGain: [3]int{home.points_loss, home.points_draw, home.points_win},
			awayGain: [3]int{away.points_win, away.points_draw, away.points_loss},
		})
	}
	points := make([]int, len(campaign))
	outcomes := make([]uint8, len(group.Games))
	scoreContext := newConditionedRankScoreContext(group, campaign, table, sortOrder, samplers)
	rng := rand.New(rand.NewSource(seed))
	for n := 0; n < samples; n++ {
		event.sampleOutcomes(rng, outcomes)
		copy(points, basePoints)
		for _, game := range pointGames {
			outcome := outcomes[game.index]
			if !selected[game.index] {
				u := rng.Float64()
				outcome = 2
				if u < game.prob[0] {
					outcome = 0
				} else if u < game.prob[0]+game.prob[1] {
					outcome = 1
				}
				outcomes[game.index] = outcome
			}
			points[game.home] += game.homeGain[outcome]
			points[game.away] += game.awayGain[outcome]
		}
		targetPoints := points[targetIndex]
		above, tied := 0, 0
		for _, index := range ranked {
			if index == targetIndex {
				continue
			}
			if points[index] > targetPoints {
				above++
			} else if points[index] == targetPoints {
				tied++
			}
		}
		if above <= rank && above+tied >= rank &&
			(tied == 0 || scoreContext.finishesAtRank(target, rank, outcomes, rng)) {
			result.hits++
		}
		result.samples++
	}
	result.work = int64(result.samples) * estimateSeasonWork(
		len(group.Games)-countPlayedGames(group.Games), 1, len(group.Team_groups))
	return result
}
