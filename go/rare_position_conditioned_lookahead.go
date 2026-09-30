package main

import (
	"math"
	"math/rand"
	"sort"
)

const (
	conditionedZeroLookaheadStates  = 30000
	conditionedZeroLookaheadSamples = 20000
	conditionedZeroLookaheadMaxCap  = 128
)

// conditionedRankSuffix holds each team's marginal chance of earning at most
// cap additional points from the remaining games. It guides the proposal;
// importance weights correct for correlations between teams.
func conditionedRankSuffix(games []conditionedPointOutcomeGame, teamCount, maxCap int) [][][]float64 {
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

func conditionedRankCDFAt(cdf []float64, limit int) float64 {
	if limit < 0 {
		return 0
	}
	if limit >= len(cdf) {
		return 1
	}
	return cdf[limit]
}

func conditionedRankSideFactor(cdf []float64, cap int, belowWeight, aboveWeight float64) float64 {
	belowProb := conditionedRankCDFAt(cdf, cap-1)
	atMostProb := conditionedRankCDFAt(cdf, cap)
	return belowWeight*belowProb + (atMostProb - belowProb) + aboveWeight*(1-atMostProb)
}

func sampleConditionedZeroRankLookahead(event *conditionedPointEvent, target, rank int,
	group *GroupType, campaign []*TeamCampaign, table *Table, sortOrder []SortType,
	bounds pointRankBounds, samplers []conditionedScoreSampler, samples int,
	seed int64) (conditionedZeroResult, bool) {
	return sampleConditionedZeroRankLookaheadWithTilt(event, target, rank, group,
		campaign, table, sortOrder, bounds, samplers, samples, seed, 1)
}

func sampleConditionedZeroRankLookaheadWithTilt(event *conditionedPointEvent, target, rank int,
	group *GroupType, campaign []*TeamCampaign, table *Table, sortOrder []SortType,
	bounds pointRankBounds, samplers []conditionedScoreSampler, samples int,
	seed int64, tilt float64) (conditionedZeroResult, bool) {
	return sampleConditionedZeroRankLookaheadWithPointTilt(event, target, rank,
		group, campaign, table, sortOrder, bounds, samplers, samples, seed, tilt, 0)
}

// pointTilt changes the distribution of final target points while preserving
// positive probability for every feasible terminal state. The returned
// likelihood ratio covers the complete cell, not one selected point total.
func sampleConditionedZeroRankLookaheadWithPointTilt(event *conditionedPointEvent, target, rank int,
	group *GroupType, campaign []*TeamCampaign, table *Table, sortOrder []SortType,
	bounds pointRankBounds, samplers []conditionedScoreSampler, samples int,
	seed int64, tilt, pointTilt float64) (conditionedZeroResult, bool) {
	return sampleConditionedZeroRankLookaheadWithPointTiltMode(event, target, rank,
		group, campaign, table, sortOrder, bounds, samplers, samples, seed,
		tilt, pointTilt, false)
}

func sampleConditionedZeroRankLookaheadWithPointTiltMode(event *conditionedPointEvent, target, rank int,
	group *GroupType, campaign []*TeamCampaign, table *Table, sortOrder []SortType,
	bounds pointRankBounds, samplers []conditionedScoreSampler, samples int,
	seed int64, tilt, pointTilt float64, forcePoints bool) (conditionedZeroResult, bool) {
	return sampleConditionedZeroRankLookaheadPolicy(event, target, rank, group, campaign,
		table, sortOrder, bounds, samplers, samples, seed, tilt, pointTilt, forcePoints,
		false)
}

func sampleConditionedZeroRankWithDomains(event *conditionedPointEvent, target, rank int,
	group *GroupType, campaign []*TeamCampaign, table *Table, sortOrder []SortType,
	bounds pointRankBounds, samplers []conditionedScoreSampler, samples int,
	seed int64, tilt, pointTilt float64) (conditionedZeroResult, bool) {
	return sampleConditionedZeroRankLookaheadPolicy(event, target, rank, group, campaign,
		table, sortOrder, bounds, samplers, samples, seed, tilt, pointTilt, false, true)
}

func sampleConditionedZeroRankLookaheadPolicy(event *conditionedPointEvent, target, rank int,
	group *GroupType, campaign []*TeamCampaign, table *Table, sortOrder []SortType,
	bounds pointRankBounds, samplers []conditionedScoreSampler, samples int,
	seed int64, tilt, pointTilt float64, forcePoints, propagate bool) (conditionedZeroResult, bool) {
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
	rankedIndices := make([]int32, 0, len(group.Team_groups)-1)
	for _, team := range group.Team_groups {
		index := table.Query(uint32(team.Team_id))
		ranked[index] = true
		if index != targetIndex {
			rankedIndices = append(rankedIndices, index)
		}
	}
	selected := make([]bool, len(group.Games))
	stride := 1
	if !forcePoints {
		stride = conditionedRankWinStride(sortOrder, group, campaign, table, "lookahead")
	}
	for _, game := range event.games {
		selected[game.index] = true
	}
	var selectedGames, remaining []conditionedPointOutcomeGame
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
		pointGame := conditionedPointOutcomeGame{
			index: index, home: game.home_table_index, away: game.away_table_index,
			prob:     targetOutcomeProbabilities(game),
			homeGain: [3]int{home.points_loss * stride, home.points_draw * stride, home.points_win * stride},
			awayGain: [3]int{away.points_win * stride, away.points_draw * stride, away.points_loss * stride},
		}
		if stride > 1 {
			pointGame.homeGain[2]++
			pointGame.awayGain[0]++
		}
		if selected[index] {
			selectedGames = append(selectedGames, pointGame)
		} else {
			remaining = append(remaining, pointGame)
		}
	}
	maxCap := 0
	for _, team := range campaign {
		if team != nil {
			current := team.points * stride
			if stride > 1 {
				current += team.wins
			}
			cap := bounds.maximum[target]*stride + stride - 1 - current
			if stride == 1 {
				cap = bounds.maximum[target] - team.points
			}
			if cap > maxCap {
				maxCap = cap
			}
		}
	}
	if maxCap > conditionedZeroLookaheadMaxCap*stride+stride-1 {
		return result, false
	}
	suffix := conditionedRankSuffix(remaining, len(campaign), maxCap)
	var domainCache *conditionedRankDomainCache
	if propagate && len(sortOrder) > 0 && sortOrder[0] == PT {
		domainCache = newConditionedRankDomainCache(remaining, selectedGames, rankedIndices, rank, len(campaign))
	}
	basePoints := make([]int, len(campaign))
	for index, team := range campaign {
		if team != nil {
			basePoints[index] = team.points * stride
			if stride > 1 {
				basePoints[index] += team.wins
			}
		}
	}
	points := make([]int, len(campaign))
	outcomes := make([]uint8, len(group.Games))
	scoreContext := newConditionedRankScoreContext(group, campaign, table, sortOrder, samplers)
	rng := rand.New(rand.NewSource(seed))
	backwardSampler := newConditionedPointBackwardSampler(event)
	terminalCDF, terminalRatio := conditionedPointTiltTerminals(event, pointTilt)
	var sumY, sumY2, maxY float64
	var batchY [2]float64
	for draw := 0; draw < samples; draw++ {
		terminalWeight := 1.0
		if pointTilt == 0 {
			backwardSampler.sampleOutcomes(rng, outcomes)
		} else {
			u := rng.Float64()
			terminal := sort.SearchFloat64s(terminalCDF, u)
			if terminal == len(terminalCDF) {
				terminal--
			}
			backwardSampler.sampleOutcomesFromTerminal(rng, outcomes, terminal)
			terminalWeight = terminalRatio[terminal]
		}
		copy(points, basePoints)
		for _, game := range selectedGames {
			outcome := outcomes[game.index]
			points[game.home] += game.homeGain[outcome]
			points[game.away] += game.awayGain[outcome]
		}
		targetPoints := points[targetIndex]
		domains := domainCache.get(points, targetPoints, outcomes)
		if domains != nil && !domains.feasible {
			result.samples++
			continue
		}
		// Use the expected counts of rivals above and below the target to
		// decide how strongly each side needs to be tilted. At either edge,
		// one coefficient becomes zero without a position-specific proposal.
		expectedAbove, expectedBelow := 0.0, 0.0
		above := 0
		for _, index := range rankedIndices {
			cap := targetPoints - points[index]
			cdf := suffix[0][index]
			expectedAbove += 1 - conditionedRankCDFAt(cdf, cap)
			expectedBelow += conditionedRankCDFAt(cdf, cap-1)
			if points[index] > targetPoints {
				above++
			}
		}
		aboveWeight, belowWeight := 1.0, 1.0
		if expectedAbove > 0 {
			aboveWeight = math.Min(1, float64(rank)/expectedAbove)
		}
		if expectedBelow > 0 {
			belowWeight = math.Min(1, float64(len(group.Team_groups)-1-rank)/expectedBelow)
		}
		if tilt != 1 {
			aboveWeight = math.Pow(aboveWeight, tilt)
			belowWeight = math.Pow(belowWeight, tilt)
		}
		weight := terminalWeight
		if above > rank {
			weight = 0
		}
		if weight > 0 {
			for step, game := range remaining {
				var score [3]float64
				total := 0.0
				homeCDF := suffix[step+1][game.home]
				awayCDF := suffix[step+1][game.away]
				homeCap := targetPoints - points[game.home]
				awayCap := targetPoints - points[game.away]
				for outcome, p := range game.prob {
					if domains != nil && !domains.allows(step, game, outcome, points) {
						continue
					}
					homeFactor := conditionedRankSideFactor(homeCDF,
						homeCap-game.homeGain[outcome], belowWeight, aboveWeight)
					awayFactor := conditionedRankSideFactor(awayCDF,
						awayCap-game.awayGain[outcome], belowWeight, aboveWeight)
					score[outcome] = p * homeFactor * awayFactor
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
		above, below := 0, 0
		for _, index := range rankedIndices {
			p := points[index]
			if p > targetPoints {
				above++
			} else if p < targetPoints {
				below++
			}
		}
		if above > rank || below > len(group.Team_groups)-1-rank {
			continue
		}
		hit := above == rank && above+below == len(group.Team_groups)-1
		if !hit && above <= rank && rank <= len(group.Team_groups)-1-below {
			hit = scoreContext.finishesAtRank(target, rank, outcomes, rng)
		}
		if hit {
			result.hits++
			if result.witnessOutcomes == nil {
				result.witnessOutcomes = append([]uint8(nil), outcomes...)
			}
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
	result.batchGap = batchGap
	accepted := result.hits >= 100 && result.ess >= 200 &&
		result.stdErr/result.probability <= 0.1 && result.maxWeightShare <= 0.03 && batchGap <= 0.2
	return result, accepted
}

func conditionedPointTiltTerminals(event *conditionedPointEvent, pointTilt float64) ([]float64, []float64) {
	if pointTilt == 0 || event.mass <= 0 {
		return nil, nil
	}
	terminalCDF := make([]float64, len(event.terminal))
	terminalRatio := make([]float64, len(event.terminal))
	maxExponent := math.Inf(-1)
	for i, terminal := range event.terminal {
		p := terminal.cumulative
		if i > 0 {
			p -= event.terminal[i-1].cumulative
		}
		if p > 0 {
			maxExponent = math.Max(maxExponent, pointTilt*float64(terminal.state[0]))
		}
	}
	tiltedWeights := make([]float64, len(event.terminal))
	var tiltedMass float64
	for i, terminal := range event.terminal {
		p := terminal.cumulative
		if i > 0 {
			p -= event.terminal[i-1].cumulative
		}
		tiltedWeights[i] = p * math.Exp(pointTilt*float64(terminal.state[0])-maxExponent)
		tiltedMass += tiltedWeights[i]
	}
	var cumulative float64
	for i, terminal := range event.terminal {
		p := terminal.cumulative
		if i > 0 {
			p -= event.terminal[i-1].cumulative
		}
		q := 0.02*p/event.mass + 0.98*tiltedWeights[i]/tiltedMass
		if q > 0 {
			terminalRatio[i] = (p / event.mass) / q
		}
		cumulative += q
		terminalCDF[i] = cumulative
	}
	terminalCDF[len(terminalCDF)-1] = 1
	return terminalCDF, terminalRatio
}
