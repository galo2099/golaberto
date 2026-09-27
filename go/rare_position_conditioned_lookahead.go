package main

import (
	"math"
	"math/rand"
)

const (
	conditionedZeroLookaheadStates  = 30000
	conditionedZeroLookaheadSamples = 20000
	conditionedZeroLookaheadMaxCap  = 128
)

// conditionedFirstSuffix holds each team's marginal chance of earning at most
// cap additional points from the remaining games. It guides the proposal;
// importance weights correct for correlations between teams.
func conditionedFirstSuffix(games []conditionedFirstPointGame, teamCount, maxCap int) [][][]float64 {
	suffix := make([][][]float64, len(games)+1)
	suffix[len(games)] = make([][]float64, teamCount)
	for team := 0; team < teamCount; team++ {
		cdf := make([]float64, maxCap+1)
		for cap := range cdf {
			cdf[cap] = 1
		}
		suffix[len(games)][team] = cdf
	}
	for step := len(games) - 1; step >= 0; step-- {
		suffix[step] = make([][]float64, teamCount)
		copy(suffix[step], suffix[step+1])
		game := games[step]
		for _, side := range []struct {
			team int32
			gain [3]int
		}{{game.home, game.homeGain}, {game.away, game.awayGain}} {
			cdf := make([]float64, maxCap+1)
			for cap := range cdf {
				for outcome, p := range game.prob {
					remaining := cap - side.gain[outcome]
					if remaining >= 0 {
						cdf[cap] += p * suffix[step+1][side.team][remaining]
					}
				}
			}
			suffix[step][side.team] = cdf
		}
	}
	return suffix
}

func sampleConditionedZeroFirstLookahead(event *conditionedPointEvent, target int,
	group *GroupType, campaign []*TeamCampaign, table *Table, sortOrder []SortType,
	bounds pointRankBounds, samplers []conditionedScoreSampler, samples int,
	seed int64) (conditionedZeroResult, bool) {
	result := conditionedZeroResult{mass: event.mass}
	if event.mass <= 0 || samples <= 0 {
		return result, false
	}
	for _, team := range campaign {
		if team != nil && (team.points_win < 0 || team.points_draw < 0 || team.points_loss < 0) {
			return result, false
		}
	}
	targetIndex := table.Query(uint32(target))
	ranked := make([]bool, len(campaign))
	for _, team := range group.Team_groups {
		ranked[table.Query(uint32(team.Team_id))] = true
	}
	selected := make([]bool, len(group.Games))
	for _, game := range event.games {
		selected[game.index] = true
	}
	var selectedGames, remaining []conditionedFirstPointGame
	for index, game := range group.Games {
		if game.Played {
			continue
		}
		if !ranked[game.home_table_index] || !ranked[game.away_table_index] ||
			campaign[game.home_table_index] == nil || campaign[game.away_table_index] == nil {
			return result, false
		}
		if game.home_table_index == game.away_table_index ||
			(!selected[index] && (game.home_table_index == targetIndex || game.away_table_index == targetIndex)) {
			return result, false
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
			selectedGames = append(selectedGames, pointGame)
		} else {
			remaining = append(remaining, pointGame)
		}
	}
	maxCap := 0
	for _, team := range campaign {
		if team != nil && bounds.maximum[target]-team.points > maxCap {
			maxCap = bounds.maximum[target] - team.points
		}
	}
	if maxCap > conditionedZeroLookaheadMaxCap {
		return result, false
	}
	suffix := conditionedFirstSuffix(remaining, len(campaign), maxCap)
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
	var sumY, sumY2, maxY float64
	var batchY [2]float64
	for draw := 0; draw < samples; draw++ {
		event.sampleOutcomes(rng, outcomes)
		copy(points, basePoints)
		for _, game := range selectedGames {
			outcome := outcomes[game.index]
			points[game.home] += game.homeGain[outcome]
			points[game.away] += game.awayGain[outcome]
		}
		targetPoints := points[targetIndex]
		weight := 1.0
		for _, team := range group.Team_groups {
			if team.Team_id != target && points[table.Query(uint32(team.Team_id))] > targetPoints {
				weight = 0
				break
			}
		}
		if weight > 0 {
			for step, game := range remaining {
				var score [3]float64
				total := 0.0
				for outcome, p := range game.prob {
					homeCap := targetPoints - points[game.home] - game.homeGain[outcome]
					awayCap := targetPoints - points[game.away] - game.awayGain[outcome]
					if homeCap < 0 || awayCap < 0 {
						continue
					}
					if homeCap > maxCap {
						homeCap = maxCap
					}
					if awayCap > maxCap {
						awayCap = maxCap
					}
					score[outcome] = p * suffix[step+1][game.home][homeCap] *
						suffix[step+1][game.away][awayCap]
					total += score[outcome]
				}
				if total <= 0 {
					weight = 0
					break
				}
				u := rng.Float64() * total
				outcome := 2
				if u < score[0] {
					outcome = 0
				} else if u < score[0]+score[1] {
					outcome = 1
				}
				q := score[outcome] / total
				if q <= 0 {
					weight = 0
					break
				}
				weight *= game.prob[outcome] / q
				outcomes[game.index] = uint8(outcome)
				points[game.home] += game.homeGain[outcome]
				points[game.away] += game.awayGain[outcome]
			}
		}
		result.samples++
		if weight <= 0 {
			continue
		}
		tied := false
		for _, team := range group.Team_groups {
			if team.Team_id != target && points[table.Query(uint32(team.Team_id))] == targetPoints {
				tied = true
				break
			}
		}
		hit := !tied
		if tied {
			hit = scoreContext.finishesFirst(target, outcomes, rng)
		}
		if hit {
			result.hits++
			sumY += weight
			sumY2 += weight * weight
			if weight > maxY {
				maxY = weight
			}
			batch := 0
			if draw >= samples/2 {
				batch = 1
			}
			batchY[batch] += weight
		}
	}
	result.work = int64(result.samples) * estimateSeasonWork(
		len(group.Games)-countPlayedGames(group.Games), 1, len(group.Team_groups))
	if sumY <= 0 || sumY2 <= 0 || math.IsNaN(sumY) || math.IsInf(sumY, 0) {
		return result, false
	}
	mean := sumY / float64(samples)
	variance := (sumY2/float64(samples) - mean*mean) / float64(samples-1)
	if variance < 0 {
		variance = 0
	}
	result.weighted = true
	result.probability = event.mass * mean
	result.stdErr = event.mass * math.Sqrt(variance)
	result.ess = sumY * sumY / sumY2
	result.maxWeightShare = maxY / sumY
	left, right := batchY[0]/float64(samples/2), batchY[1]/float64(samples-samples/2)
	batchGap := math.Abs(left-right) / mean
	accepted := result.hits >= 100 && result.ess >= 200 &&
		result.stdErr/result.probability <= 0.1 && result.maxWeightShare <= 0.03 && batchGap <= 0.2
	return result, accepted
}
