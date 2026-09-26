package main

// PointGapScout pools final tables rather than requiring a team to have
// actually finished with each queried point total. For each table and queried
// total, it moves each team to that total while holding the other teams fixed.
// The actual sorted order breaks hypothetical ties; tie-sensitive predictions
// remain a model-based approximation.
type PointGapScout struct {
	PointRankCounts         map[int][]int
	PointCounts             map[int]int
	ObservedPointRankCounts map[int][]int
	ObservedPointCounts     map[int]int
	Observations            int64
	Work                    int64
	MinPoints               int
	MaxPoints               int
	NumPositions            int
}

func newPointGapScout(minPoints, maxPoints, numPositions int) *PointGapScout {
	return &PointGapScout{
		PointRankCounts:         make(map[int][]int),
		PointCounts:             make(map[int]int),
		ObservedPointRankCounts: make(map[int][]int),
		ObservedPointCounts:     make(map[int]int),
		MinPoints:               minPoints,
		MaxPoints:               maxPoints,
		NumPositions:            numPositions,
	}
}

func (gap *PointGapScout) add(points, rank int) {
	if points < gap.MinPoints || points > gap.MaxPoints {
		return
	}
	if gap.PointRankCounts[points] == nil {
		gap.PointRankCounts[points] = make([]int, gap.NumPositions)
	}
	gap.PointRankCounts[points][rank]++
	gap.PointCounts[points]++
	gap.Observations++
}

func (gap *PointGapScout) addTable(sorted []*TeamCampaign) {
	counts := make(map[int]int, len(sorted))
	for rank, team := range sorted {
		counts[team.points]++
		if gap.ObservedPointRankCounts[team.points] == nil {
			gap.ObservedPointRankCounts[team.points] = make([]int, gap.NumPositions)
		}
		gap.ObservedPointRankCounts[team.points][rank]++
		gap.ObservedPointCounts[team.points]++
	}
	greater := len(sorted)
	for points := gap.MinPoints; points <= gap.MaxPoints; points++ {
		equal := counts[points]
		greater -= equal
		below := len(sorted) - greater - equal
		ranks := gap.PointRankCounts[points]
		if ranks == nil {
			ranks = make([]int, gap.NumPositions)
			gap.PointRankCounts[points] = ranks
		}
		if greater > 0 {
			ranks[greater-1] += greater
		}
		for tied := 0; tied < equal; tied++ {
			ranks[greater+tied]++
		}
		if below > 0 {
			ranks[greater+equal] += below
		}
		gap.PointCounts[points] += len(sorted)
		gap.Observations += int64(len(sorted))
		gap.Work += int64(1 + equal)
	}
}

// pointGapSmoothedEstimate uses actual point/rank outcomes plus a small number
// of hypothetical standings placements as prior observations. The gap curve
// supplies a value for point totals absent from the scout.
func pointGapSmoothedEstimate(pmf map[int]float64, currentPoints, rank int, gap *PointGapScout, priorStrength float64) (estimate, supportedMass float64) {
	if gap == nil || rank < 0 || rank >= gap.NumPositions || priorStrength < 0 {
		return 0, 0
	}
	for additional, mass := range pmf {
		if mass <= 0 {
			continue
		}
		finalPoints := currentPoints + additional
		observedCount := gap.ObservedPointCounts[finalPoints]
		gapCount := gap.PointCounts[finalPoints]
		if observedCount == 0 && gapCount == 0 {
			continue
		}
		gapProbability := 0.0
		if gapCount > 0 {
			gapProbability = float64(gap.PointRankCounts[finalPoints][rank]) / float64(gapCount)
		}
		if observedCount > 0 {
			if priorStrength > 0 && gapCount > 0 {
				estimate += mass * (float64(gap.ObservedPointRankCounts[finalPoints][rank]) + priorStrength*gapProbability) /
					(float64(observedCount) + priorStrength)
			} else {
				estimate += mass * float64(gap.ObservedPointRankCounts[finalPoints][rank]) / float64(observedCount)
			}
		} else {
			estimate += mass * gapProbability
		}
		supportedMass += mass
	}
	return estimate, supportedMass
}

func pointGapConditionalEstimate(pmf map[int]float64, allowed []int, currentPoints, rank int, gap *PointGapScout) (estimate, supportedMass float64) {
	if gap == nil || rank < 0 || rank >= gap.NumPositions {
		return 0, 0
	}
	for _, additional := range allowed {
		mass := pmf[additional]
		if mass <= 0 {
			continue
		}
		finalPoints := currentPoints + additional
		observedCount := gap.ObservedPointCounts[finalPoints]
		if observedCount > 0 {
			estimate += mass * float64(gap.ObservedPointRankCounts[finalPoints][rank]) / float64(observedCount)
			supportedMass += mass
		} else if gapCount := gap.PointCounts[finalPoints]; gapCount > 0 {
			estimate += mass * float64(gap.PointRankCounts[finalPoints][rank]) / float64(gapCount)
			supportedMass += mass
		}
	}
	if supportedMass > 0 {
		estimate /= supportedMass
	}
	return estimate, supportedMass
}

// pointGapEstimate reweights the pooled standings-gap rank curve by a team's
// exact points distribution. This is a model-based proxy: changing a team's
// fixture results would also change some opponents' points.
func pointGapEstimate(pmf map[int]float64, currentPoints, rank int, gap *PointGapScout) (estimate, supportedMass float64) {
	if gap == nil || rank < 0 || rank >= gap.NumPositions {
		return 0, 0
	}
	for additional, mass := range pmf {
		if mass <= 0 {
			continue
		}
		finalPoints := currentPoints + additional
		count := gap.PointCounts[finalPoints]
		if count <= 0 {
			continue
		}
		estimate += mass * float64(gap.PointRankCounts[finalPoints][rank]) / float64(count)
		supportedMass += mass
	}
	return estimate, supportedMass
}
