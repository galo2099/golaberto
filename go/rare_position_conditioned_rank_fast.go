package main

import (
	"math/rand"
	"os"
)

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
	stride := conditionedRankWinStride(sortOrder, group, campaign, table, "screen")
	for i, team := range campaign {
		if team == nil {
			continue
		}
		if team.points_win < 0 || team.points_draw < 0 || team.points_loss < 0 {
			return fallback()
		}
		basePoints[i] = team.points * stride
		if stride > 1 {
			basePoints[i] += team.wins
		}
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
			homeGain: [3]int{home.points_loss * stride, home.points_draw * stride, home.points_win * stride},
			awayGain: [3]int{away.points_win * stride, away.points_draw * stride, away.points_loss * stride},
		})
		if stride > 1 {
			pointGames[len(pointGames)-1].homeGain[2]++
			pointGames[len(pointGames)-1].awayGain[0]++
		}
	}
	points := make([]int, len(campaign))
	outcomes := make([]uint8, len(group.Games))
	scoreContext := newConditionedRankScoreContext(group, campaign, table, sortOrder, samplers)
	rng := rand.New(rand.NewSource(seed))
	pairedScores := os.Getenv("RARE_POSITION_PAIRED_SCREEN_RNG") == "1"
	var scoreRNG *rand.Rand
	if pairedScores {
		scoreRNG = rand.New(rand.NewSource(seed))
	}
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
		hit := above <= rank && above+tied >= rank
		if hit && tied > 0 {
			rankRNG := rng
			if pairedScores {
				scoreRNG.Seed(seed + int64(n)*6364136223846793005)
				rankRNG = scoreRNG
			}
			hit = scoreContext.finishesAtRank(target, rank, outcomes, rankRNG)
		}
		if hit {
			result.hits++
			if result.witnessOutcomes == nil {
				result.witnessOutcomes = append([]uint8(nil), outcomes...)
			}
		}
		result.samples++
	}
	result.work = int64(result.samples) * estimateSeasonWork(
		len(group.Games)-countPlayedGames(group.Games), 1, len(group.Team_groups))
	return result
}
