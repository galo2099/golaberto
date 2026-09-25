package main

import (
	"fmt"
	"math"
	"math/rand"
	"sort"
)

// A points stratum fixes only the target team's remaining W/D/L pattern.
// Scores conditional on that pattern, and every other game, retain their
// original Poisson law. The pattern filter is necessary for a target rank,
// so its total P mass is a rigorous upper bound on that rank's probability.
type PointStratumPattern struct {
	Code         uint64
	Probability  float64
	Cumulative   float64
	AddedPoints  int
	CertainAhead int
}

type PointStratumUniverse struct {
	TargetTeam           int
	GameIndices          []int
	GameSlots            map[int]int
	GamePowers           []uint64
	OutcomeProbabilities [][3]float64 // target loss, draw, win
	OutcomeGains         [][3]int
	Patterns             []PointStratumPattern
	MinimumAddedPoints   int
}

type PointStratum struct {
	Universe           *PointStratumUniverse
	MaxRank            int
	MinimumAddedPoints int
	Mass               float64
	Patterns           []PointStratumPattern
	Allowed            map[uint64]bool
	Tail               *pointTailDP
}

type pointTailDP struct {
	Universe  *PointStratumUniverse
	Threshold int
	Memo      map[[2]int]float64
}

func poissonOutcomeMass(mean float64) []float64 {
	if mean < 0 || math.IsNaN(mean) || math.IsInf(mean, 0) {
		panic("invalid Poisson mean")
	}
	masses := []float64{math.Exp(-mean)}
	sum := masses[0]
	for k := 1; 1-sum > 1e-15 && k < 1000; k++ {
		masses = append(masses, masses[k-1]*mean/float64(k))
		sum += masses[k]
	}
	return masses
}

func targetOutcomeProbabilities(game *GameType) [3]float64 {
	home := poissonOutcomeMass(game.HomePower)
	away := poissonOutcomeMass(game.AwayPower)
	var homeOutcome [3]float64
	for hs, hp := range home {
		for as, ap := range away {
			outcome := 1
			if hs > as {
				outcome = 2
			} else if hs < as {
				outcome = 0
			}
			homeOutcome[outcome] += hp * ap
		}
	}
	sum := homeOutcome[0] + homeOutcome[1] + homeOutcome[2]
	for i := range homeOutcome {
		homeOutcome[i] /= sum
	}
	return homeOutcome
}

func targetOutcomeDigit(targetHome bool, homeScore, awayScore int) int {
	if homeScore == awayScore {
		return 1
	}
	if (homeScore > awayScore) == targetHome {
		return 2
	}
	return 0
}

func pointOutcomeUniverse(targetTeam int, campaign []*TeamCampaign, table *Table, games []*GameType, maxGames int) (*PointStratumUniverse, bool) {
	index := table.Query(uint32(targetTeam))
	if index < 0 || int(index) >= len(campaign) || campaign[index] == nil {
		return nil, false
	}
	universe := &PointStratumUniverse{TargetTeam: targetTeam, GameSlots: map[int]int{}}
	for _, c := range campaign {
		if c != nil && (c.points_win < 0 || c.points_draw < 0 || c.points_loss < 0) {
			return nil, false
		}
	}
	codePower := uint64(1)
	for i, game := range games {
		if !game.Played && (game.HomeId == targetTeam || game.AwayId == targetTeam) {
			if len(universe.GameIndices) >= maxGames {
				return nil, false
			}
			universe.GameSlots[i] = len(universe.GameIndices)
			universe.GameIndices = append(universe.GameIndices, i)
			universe.GamePowers = append(universe.GamePowers, codePower)
			codePower *= 3
			prob := targetOutcomeProbabilities(game)
			if game.AwayId == targetTeam {
				prob[0], prob[2] = prob[2], prob[0]
			}
			universe.OutcomeProbabilities = append(universe.OutcomeProbabilities, prob)
			universe.OutcomeGains = append(universe.OutcomeGains, [3]int{campaign[index].points_loss, campaign[index].points_draw, campaign[index].points_win})
		}
	}
	if len(universe.GameIndices) == 0 || len(universe.GameIndices) > maxGames {
		return nil, false
	}
	return universe, true
}

func pointStratumUniverse(targetTeam int, campaign []*TeamCampaign, table *Table, games []*GameType, maxGames int) (*PointStratumUniverse, bool) {
	universe, ok := pointOutcomeUniverse(targetTeam, campaign, table, games, maxGames)
	if !ok {
		return nil, false
	}
	index := table.Query(uint32(targetTeam))
	patternCount := uint64(1)
	for range universe.GameIndices {
		patternCount *= 3
	}
	target := campaign[index]
	otherPoints := make(map[int]int)
	for _, c := range campaign {
		if c != nil && c.id != targetTeam {
			otherPoints[c.id] = c.points
		}
	}
	minimum := math.MaxInt
	for code := uint64(0); code < patternCount; code++ {
		probability := 1.0
		added := 0
		forcedOther := make(map[int]int, len(universe.GameIndices))
		encoded := code
		for slot, gameIndex := range universe.GameIndices {
			digit := int(encoded % 3)
			encoded /= 3
			probability *= universe.OutcomeProbabilities[slot][digit]
			game := games[gameIndex]
			otherID := game.AwayId
			if otherID == targetTeam {
				otherID = game.HomeId
			}
			other := campaign[table.Query(uint32(otherID))]
			switch digit {
			case 0:
				added += target.points_loss
				forcedOther[otherID] += other.points_win
			case 1:
				added += target.points_draw
				forcedOther[otherID] += other.points_draw
			case 2:
				added += target.points_win
				forcedOther[otherID] += other.points_loss
			}
		}
		if probability == 0 {
			continue
		}
		certainAhead := 0
		for id, points := range otherPoints {
			if points+forcedOther[id] > target.points+added {
				certainAhead++
			}
		}
		universe.Patterns = append(universe.Patterns, PointStratumPattern{Code: code, Probability: probability, AddedPoints: added, CertainAhead: certainAhead})
		if added < minimum {
			minimum = added
		}
	}
	universe.MinimumAddedPoints = minimum
	return universe, true
}

func makePointStratum(universe *PointStratumUniverse, maxRank, minimumAddedPoints int) (*PointStratum, bool) {
	stratum := &PointStratum{Universe: universe, MaxRank: maxRank, MinimumAddedPoints: minimumAddedPoints, Allowed: map[uint64]bool{}}
	for _, pattern := range universe.Patterns {
		if pattern.CertainAhead > maxRank || pattern.AddedPoints < minimumAddedPoints {
			continue
		}
		stratum.Mass += pattern.Probability
		pattern.Cumulative = stratum.Mass
		stratum.Patterns = append(stratum.Patterns, pattern)
		stratum.Allowed[pattern.Code] = true
	}
	return stratum, stratum.Mass > 0
}

func (stratum *PointStratum) samplePattern(rng *rand.Rand) uint64 {
	if stratum.Tail != nil {
		return stratum.Tail.samplePattern(rng)
	}
	u := rng.Float64() * stratum.Mass
	i := sort.Search(len(stratum.Patterns), func(i int) bool { return stratum.Patterns[i].Cumulative >= u })
	if i >= len(stratum.Patterns) {
		i = len(stratum.Patterns) - 1
	}
	return stratum.Patterns[i].Code
}

func (stratum *PointStratum) contains(code uint64) bool {
	if stratum.Tail == nil {
		return stratum.Allowed[code]
	}
	added := 0
	for slot, gains := range stratum.Universe.OutcomeGains {
		added += gains[pointStratumDigit(code, slot)]
	}
	return added >= stratum.Tail.Threshold
}

func (dp *pointTailDP) probability(slot, needed int) float64 {
	if needed <= 0 {
		return 1
	}
	if slot >= len(dp.Universe.GameIndices) {
		return 0
	}
	key := [2]int{slot, needed}
	if value, ok := dp.Memo[key]; ok {
		return value
	}
	value := 0.0
	for digit, probability := range dp.Universe.OutcomeProbabilities[slot] {
		value += probability * dp.probability(slot+1, needed-dp.Universe.OutcomeGains[slot][digit])
	}
	dp.Memo[key] = value
	return value
}

func (dp *pointTailDP) samplePattern(rng *rand.Rand) uint64 {
	needed := dp.Threshold
	code := uint64(0)
	for slot, probabilities := range dp.Universe.OutcomeProbabilities {
		weights := [3]float64{}
		total := 0.0
		for digit, probability := range probabilities {
			weights[digit] = probability * dp.probability(slot+1, needed-dp.Universe.OutcomeGains[slot][digit])
			total += weights[digit]
		}
		u := rng.Float64() * total
		digit := 2
		for candidate := 0; candidate < 2; candidate++ {
			if u < weights[candidate] {
				digit = candidate
				break
			}
			u -= weights[candidate]
		}
		code += uint64(digit) * dp.Universe.GamePowers[slot]
		needed -= dp.Universe.OutcomeGains[slot][digit]
	}
	return code
}

func makePointTailStratum(universe *PointStratumUniverse, threshold int) (*PointStratum, bool) {
	dp := &pointTailDP{Universe: universe, Threshold: threshold, Memo: make(map[[2]int]float64)}
	mass := dp.probability(0, threshold)
	return &PointStratum{Universe: universe, MinimumAddedPoints: threshold, Mass: mass, Tail: dp}, mass > 0
}

// Existing points are lower bounds on final points when future game rewards
// are nonnegative. To reach rank maxRank or better, the target must at least
// tie the (maxRank+1)th highest opponent's current points.
func minimumPointsForRank(targetTeam, maxRank int, campaign []*TeamCampaign) (int, bool) {
	current := 0
	found := false
	others := make([]int, 0, len(campaign)-1)
	for _, c := range campaign {
		if c == nil {
			continue
		}
		if c.id == targetTeam {
			current, found = c.points, true
		} else {
			others = append(others, c.points)
		}
	}
	if !found || maxRank < 0 || maxRank > len(others) {
		return 0, false
	}
	if maxRank == len(others) {
		return 0, true
	}
	sort.Sort(sort.Reverse(sort.IntSlice(others)))
	minimum := others[maxRank] - current
	if minimum < 0 {
		minimum = 0
	}
	return minimum, true
}

func pointStratumDigit(code uint64, slot int) int {
	for range slot {
		code /= 3
	}
	return int(code % 3)
}

func samplePointStratumScore(rng *rand.Rand, game *GameType, means GameProposalMeans, targetTeam, desiredOutcome int) (int, int) {
	for {
		hs := poissonRand(rng, means.Home)
		as := poissonRand(rng, means.Away)
		if targetOutcomeDigit(game.HomeId == targetTeam, hs, as) == desiredOutcome {
			return hs, as
		}
	}
}

func pointStratumProposalID(targetTeam, maxRank, minAdded int) string {
	return fmt.Sprintf("team%d_top%d_points%d", targetTeam, maxRank+1, minAdded)
}
