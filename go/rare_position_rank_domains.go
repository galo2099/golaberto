package main

import (
	"math/bits"
	"os"
)

const conditionedRankDomainCacheLimit = 64

func compactForcedFixturesEnabled() bool {
	return os.Getenv("RARE_POSITION_COMPACT_FORCED_FIXTURES") != "0"
}

func compactZeroGuideEnabled() bool {
	return os.Getenv("RARE_POSITION_COMPACT_ZERO_GUIDE") != "0"
}

type conditionedRankLoopStep struct {
	step                                               int
	skippedBefore                                      int
	homeForced, awayForced                             int
	game                                               *conditionedPointOutcomeGame
	homeCDF, awayCDF                                   []float64
	domain                                             uint8
	homeMinimum, homeMaximum, awayMinimum, awayMaximum int
	homeLower, homeUpper, awayLower, awayUpper         int
}

type conditionedRankForcedOutcome struct {
	index   int
	outcome uint8
}

// These are necessary rank constraints, not a witness's chosen results. Equal
// points/wins remain possible on either side until the full sorter resolves
// later tiebreakers. Discarding a domain therefore cannot discard a rank hit.
type conditionedRankDomains struct {
	domains                         []uint8
	lower, upper                    []int
	minimumSuffix                   [][]int
	maximumSuffix                   [][]int
	feasible                        bool
	compactBase                     []int
	forced                          []conditionedRankForcedOutcome
	variable                        []conditionedRankLoopStep
	forcedAfter                     int
	forcedMass                      float64
	zeroGuideChecked, zeroGuideSafe bool
}

// A zero guide can give a forced outcome zero proposal score. Check every
// feasible prefix once per cached target assignment, including floating-point
// cancellation and underflow, before skipping those original rejection sites.
func (domains *conditionedRankDomains) canCompactZeroGuide(games []conditionedPointOutcomeGame,
	points []int, targetPoints int, suffix [][][]float64, aboveWeight, belowWeight float64) bool {
	if domains.zeroGuideChecked {
		return domains.zeroGuideSafe
	}
	domains.zeroGuideChecked = true
	minimum, maximum := append([]int(nil), points...), append([]int(nil), points...)
	for step, game := range games {
		domain := domains.domains[step]
		if bits.OnesCount8(domain) == 1 {
			outcome := bits.TrailingZeros8(domain)
			factors := [2]float64{}
			for side, pair := range []struct {
				team int32
				gain int
			}{
				{game.home, game.homeGain[outcome]}, {game.away, game.awayGain[outcome]},
			} {
				team := pair.team
				lo := max(minimum[team], domains.lower[team]-domains.maximumSuffix[step][team])
				hi := min(maximum[team], domains.upper[team]-domains.minimumSuffix[step][team])
				if lo > hi {
					return false
				}
				for prefix := lo; prefix <= hi; prefix++ {
					factor := conditionedRankSideFactor(suffix[step+1][team], targetPoints-prefix-pair.gain, belowWeight, aboveWeight)
					if !(factor > 0) {
						return false
					}
					if prefix == lo || factor < factors[side] {
						factors[side] = factor
					}
				}
			}
			if !(game.prob[outcome]*factors[0]*factors[1] > 0) {
				return false
			}
		}
		minimum[game.home] += minJointPointGain(domain, game.homeGain)
		minimum[game.away] += minJointPointGain(domain, game.awayGain)
		maximum[game.home] += maxJointPointGain(domain, game.homeGain)
		maximum[game.away] += maxJointPointGain(domain, game.awayGain)
	}
	domains.zeroGuideSafe = true
	return true
}

type conditionedRankDomainCache struct {
	games            []conditionedPointOutcomeGame
	suffix           [][][]float64
	selected         []conditionedPointOutcomeGame
	rivals           []int32
	rank             int
	minimum, maximum []int
	entries          map[uint64]*conditionedRankDomains
}

func newConditionedRankDomainCache(games, selected []conditionedPointOutcomeGame,
	rivals []int32, rank, teams int) *conditionedRankDomainCache {
	if len(selected) > 32 {
		return nil
	}
	cache := &conditionedRankDomainCache{games: games, selected: selected, rivals: rivals,
		rank: rank, minimum: make([]int, teams), maximum: make([]int, teams),
		entries: make(map[uint64]*conditionedRankDomains)}
	for _, game := range games {
		domain := uint8(0)
		for outcome, p := range game.prob {
			if p > 0 {
				domain |= 1 << outcome
			}
		}
		cache.minimum[game.home] += minJointPointGain(domain, game.homeGain)
		cache.minimum[game.away] += minJointPointGain(domain, game.awayGain)
		cache.maximum[game.home] += maxJointPointGain(domain, game.homeGain)
		cache.maximum[game.away] += maxJointPointGain(domain, game.awayGain)
	}
	return cache
}

func maxJointPointGain(domain uint8, gain [3]int) int {
	maximum := -int(^uint(0)>>1) - 1
	for outcome := 0; outcome < 3; outcome++ {
		if domain&(1<<outcome) != 0 && gain[outcome] > maximum {
			maximum = gain[outcome]
		}
	}
	return maximum
}

func (cache *conditionedRankDomainCache) get(points []int, targetPoints int, outcomes []uint8) *conditionedRankDomains {
	if cache == nil {
		return nil
	}
	var key uint64
	for index, game := range cache.selected {
		key |= uint64(outcomes[game.index]) << (2 * index)
	}
	if result, found := cache.entries[key]; found {
		return result
	}
	if len(cache.entries) >= conditionedRankDomainCacheLimit {
		// Falling back to the unrestricted proposal preserves support and
		// bounds preprocessing for target assignments with many patterns.
		return nil
	}
	above, below := 0, 0
	for _, team := range cache.rivals {
		if points[team]+cache.minimum[team] > targetPoints {
			above++
		}
		if points[team]+cache.maximum[team] < targetPoints {
			below++
		}
	}
	var result *conditionedRankDomains
	if above >= cache.rank || below >= len(cache.rivals)-cache.rank {
		result = propagateConditionedRankDomains(cache.games, points, cache.rivals, cache.rank, targetPoints)
		if result.feasible && cache.suffix != nil {
			for index := range result.variable {
				entry := &result.variable[index]
				game, next := entry.game, entry.step+1
				entry.homeCDF, entry.awayCDF = cache.suffix[next][game.home], cache.suffix[next][game.away]
				entry.domain = result.domains[entry.step]
				entry.homeMinimum, entry.homeMaximum = result.minimumSuffix[next][game.home], result.maximumSuffix[next][game.home]
				entry.awayMinimum, entry.awayMaximum = result.minimumSuffix[next][game.away], result.maximumSuffix[next][game.away]
				entry.homeLower, entry.homeUpper = result.lower[game.home], result.upper[game.home]
				entry.awayLower, entry.awayUpper = result.lower[game.away], result.upper[game.away]
			}
		}
	}
	cache.entries[key] = result
	return result
}

// Transfer cap/floor propagation to a particular sampled target assignment.
// Once all above (or below) rank slots are necessarily occupied, every other
// rival must stay at or below (or at or above) the target's ordered score.
func propagateConditionedRankDomains(games []conditionedPointOutcomeGame, points []int,
	rivals []int32, rank, targetPoints int) *conditionedRankDomains {
	result := &conditionedRankDomains{domains: make([]uint8, len(games)),
		lower: make([]int, len(points)), upper: make([]int, len(points)), feasible: true}
	const infinity = int(^uint(0)>>1) / 4
	for team := range points {
		result.lower[team], result.upper[team] = -infinity, infinity
	}
	for index, game := range games {
		for outcome, p := range game.prob {
			if p > 0 {
				result.domains[index] |= 1 << outcome
			}
		}
	}
	minimum, maximum := make([]int, len(points)), make([]int, len(points))
	homeMin, homeMax := make([]int, len(games)), make([]int, len(games))
	awayMin, awayMax := make([]int, len(games)), make([]int, len(games))
	for {
		copy(minimum, points)
		copy(maximum, points)
		for index, game := range games {
			domain := result.domains[index]
			if domain == 0 {
				result.feasible = false
				return result
			}
			homeMin[index], homeMax[index] = minJointPointGain(domain, game.homeGain), maxJointPointGain(domain, game.homeGain)
			awayMin[index], awayMax[index] = minJointPointGain(domain, game.awayGain), maxJointPointGain(domain, game.awayGain)
			minimum[game.home] += homeMin[index]
			minimum[game.away] += awayMin[index]
			maximum[game.home] += homeMax[index]
			maximum[game.away] += awayMax[index]
		}
		above, below := 0, 0
		for _, team := range rivals {
			if minimum[team] > targetPoints {
				above++
			}
			if maximum[team] < targetPoints {
				below++
			}
		}
		if above > rank || below > len(rivals)-rank {
			result.feasible = false
			return result
		}
		for _, team := range rivals {
			if above == rank && minimum[team] <= targetPoints {
				result.upper[team] = min(result.upper[team], targetPoints)
			}
			if below == len(rivals)-rank && maximum[team] >= targetPoints {
				result.lower[team] = max(result.lower[team], targetPoints)
			}
			if minimum[team] > result.upper[team] || maximum[team] < result.lower[team] {
				result.feasible = false
				return result
			}
		}
		changed := false
		for index, game := range games {
			domain := result.domains[index]
			for outcome := 0; outcome < 3; outcome++ {
				if domain&(1<<outcome) == 0 {
					continue
				}
				if minimum[game.home]-homeMin[index]+game.homeGain[outcome] > result.upper[game.home] ||
					maximum[game.home]-homeMax[index]+game.homeGain[outcome] < result.lower[game.home] ||
					minimum[game.away]-awayMin[index]+game.awayGain[outcome] > result.upper[game.away] ||
					maximum[game.away]-awayMax[index]+game.awayGain[outcome] < result.lower[game.away] {
					domain &^= 1 << outcome
					changed = true
				}
			}
			result.domains[index] = domain
		}
		if !changed {
			break
		}
	}
	// These cheap suffix bounds enforce each necessary team threshold after
	// every sampled fixture, rather than waiting for a rejected full season.
	result.minimumSuffix = make([][]int, len(games)+1)
	result.maximumSuffix = make([][]int, len(games)+1)
	result.minimumSuffix[len(games)] = make([]int, len(points))
	result.maximumSuffix[len(games)] = make([]int, len(points))
	for step := len(games) - 1; step >= 0; step-- {
		game, domain := games[step], result.domains[step]
		result.minimumSuffix[step] = append([]int(nil), result.minimumSuffix[step+1]...)
		result.maximumSuffix[step] = append([]int(nil), result.maximumSuffix[step+1]...)
		result.minimumSuffix[step][game.home] += minJointPointGain(domain, game.homeGain)
		result.minimumSuffix[step][game.away] += minJointPointGain(domain, game.awayGain)
		result.maximumSuffix[step][game.home] += maxJointPointGain(domain, game.homeGain)
		result.maximumSuffix[step][game.away] += maxJointPointGain(domain, game.awayGain)
	}
	result.compact(games, points)
	return result
}

// Cache the fixed score increments and their probability mass. Future fixed
// increments are subtracted from proposal caps so the existing marginal
// lookahead and sequential feasibility checks see exactly the old prefix.
func (domains *conditionedRankDomains) compact(games []conditionedPointOutcomeGame, points []int) {
	domains.forcedMass = 1
	fixed := 0
	for _, domain := range domains.domains {
		if bits.OnesCount8(domain) == 1 {
			fixed++
		}
	}
	if fixed == 0 {
		return
	}
	domains.variable = make([]conditionedRankLoopStep, 0, len(games)-fixed)
	domains.forced = make([]conditionedRankForcedOutcome, 0, fixed)
	future := make([]int, len(points))
	for step := len(games) - 1; step >= 0; step-- {
		game, domain := games[step], domains.domains[step]
		if bits.OnesCount8(domain) == 1 {
			outcome := bits.TrailingZeros8(domain)
			future[game.home] += game.homeGain[outcome]
			future[game.away] += game.awayGain[outcome]
		} else {
			domains.variable = append(domains.variable, conditionedRankLoopStep{
				step: step, homeForced: future[game.home], awayForced: future[game.away], game: &games[step]})
		}
	}
	domains.compactBase = append([]int(nil), points...)
	for team, gain := range future {
		domains.compactBase[team] += gain
	}
	for left, right := 0, len(domains.variable)-1; left < right; left, right = left+1, right-1 {
		domains.variable[left], domains.variable[right] = domains.variable[right], domains.variable[left]
	}
	skipped, variable := 0, 0
	for step, game := range games {
		domain := domains.domains[step]
		if bits.OnesCount8(domain) == 1 {
			outcome := uint8(bits.TrailingZeros8(domain))
			domains.forced = append(domains.forced, conditionedRankForcedOutcome{game.index, outcome})
			domains.forcedMass *= game.prob[outcome]
			skipped++
		} else {
			domains.variable[variable].skippedBefore = skipped
			variable++
			skipped = 0
		}
	}
	domains.forcedAfter = skipped
}

func (domains *conditionedRankDomains) allows(step int, game conditionedPointOutcomeGame, outcome int, points []int) bool {
	if domains == nil {
		return true
	}
	if domains.domains[step]&(1<<outcome) == 0 {
		return false
	}
	home, away := points[game.home]+game.homeGain[outcome], points[game.away]+game.awayGain[outcome]
	return home+domains.minimumSuffix[step+1][game.home] <= domains.upper[game.home] &&
		home+domains.maximumSuffix[step+1][game.home] >= domains.lower[game.home] &&
		away+domains.minimumSuffix[step+1][game.away] <= domains.upper[game.away] &&
		away+domains.maximumSuffix[step+1][game.away] >= domains.lower[game.away]
}
