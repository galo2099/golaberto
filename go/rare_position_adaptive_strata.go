package main

import (
	"fmt"
	"log"
	"math"
	"math/rand"
	"os"
	"sort"
	"strconv"
)

func (dp *pointSetDP) probability(slot, currentPoints int) float64 {
	if slot >= len(dp.Universe.GameIndices) {
		if dp.AllowedPoints[currentPoints] {
			return 1.0
		}
		return 0.0
	}
	key := [2]int{slot, currentPoints}
	if val, ok := dp.Memo[key]; ok {
		return val
	}
	val := 0.0
	probs := dp.Universe.OutcomeProbabilities[slot]
	gains := dp.Universe.OutcomeGains[slot]
	for digit := 0; digit < 3; digit++ {
		p := probs[digit]
		if p > 0 {
			val += p * dp.probability(slot+1, currentPoints+gains[digit])
		}
	}
	dp.Memo[key] = val
	return val
}

func (dp *pointSetDP) samplePattern(rng *rand.Rand) uint64 {
	currentPoints := 0
	code := uint64(0)
	for slot := range dp.Universe.GameIndices {
		probs := dp.Universe.OutcomeProbabilities[slot]
		gains := dp.Universe.OutcomeGains[slot]
		weights := [3]float64{}
		total := 0.0
		for digit := 0; digit < 3; digit++ {
			p := probs[digit]
			if p > 0 {
				w := p * dp.probability(slot+1, currentPoints+gains[digit])
				weights[digit] = w
				total += w
			}
		}
		if total <= 0 {
			panic("point-set conditional path has zero probability")
		}
		u := rng.Float64() * total
		chosenDigit := 2
		for digit := 0; digit < 2; digit++ {
			if u < weights[digit] {
				chosenDigit = digit
				break
			}
			u -= weights[digit]
		}
		code += uint64(chosenDigit) * dp.Universe.GamePowers[slot]
		currentPoints += gains[chosenDigit]
	}
	return code
}

func makePointSetStratum(universe *PointStratumUniverse, allowed []int) (*PointStratum, bool) {
	if universe == nil || len(allowed) == 0 {
		return nil, false
	}
	allowedMap := make(map[int]bool, len(allowed))
	for _, pts := range allowed {
		allowedMap[pts] = true
	}
	dp := &pointSetDP{
		Universe:      universe,
		AllowedPoints: allowedMap,
		Memo:          make(map[[2]int]float64),
	}
	mass := dp.probability(0, 0)
	if mass <= 0 || math.IsNaN(mass) || math.IsInf(mass, 0) {
		return nil, false
	}
	return &PointStratum{
		Universe:           universe,
		MaxRank:            len(universe.GameIndices),
		MinimumAddedPoints: 0,
		Mass:               mass,
		Allowed:            make(map[uint64]bool),
		PointSetDP:         dp,
	}, true
}

type TeamPointRankScout struct {
	Samples         int           `json:"samples"`
	RankCounts      []int         `json:"rank_counts"`
	PointRankCounts map[int][]int `json:"point_rank_counts"`
	PointCounts     map[int]int   `json:"point_counts"`
}

type PointRankProfile struct {
	AddedPoints int       `json:"added_points"`
	PointMass   float64   `json:"point_mass"`
	ScoutCount  int       `json:"scout_count"`
	RankCounts  []int     `json:"rank_counts"`
	RankProb    []float64 `json:"rank_prob"`
}

func pointRankProfileDistance(a, b []float64) float64 {
	n := len(a)
	if len(b) < n {
		n = len(b)
	}
	tvd := 0.0
	for i := 0; i < n; i++ {
		tvd += math.Abs(a[i] - b[i])
	}
	return 0.5 * tvd
}

func computePointRankProfile(
	addedPoints int,
	pmf map[int]float64,
	scout *TeamPointRankScout,
	numPositions int,
	bandwidth float64,
) PointRankProfile {
	if bandwidth <= 0 {
		if raw := os.Getenv("RARE_POSITION_ADAPTIVE_POINT_BANDWIDTH"); raw != "" {
			if parsed, err := strconv.ParseFloat(raw, 64); err == nil && parsed > 0 {
				bandwidth = parsed
			}
		}
		if bandwidth <= 0 {
			bandwidth = 1.5
		}
	}
	mass := pmf[addedPoints]
	rawCount := 0
	rawRankCounts := make([]int, numPositions)

	if scout != nil {
		rawCount = scout.PointCounts[addedPoints]
		if hits, ok := scout.PointRankCounts[addedPoints]; ok {
			copy(rawRankCounts, hits)
		}
	}

	rankProb := make([]float64, numPositions)
	weightedTotal := 0.0

	if scout != nil && len(scout.PointCounts) > 0 {
		for sNeighbor, count := range scout.PointCounts {
			if count <= 0 {
				continue
			}
			w := math.Exp(-math.Abs(float64(sNeighbor-addedPoints)) / bandwidth)
			weightedTotal += float64(count) * w
			hits := scout.PointRankCounts[sNeighbor]
			for rank := 0; rank < numPositions && rank < len(hits); rank++ {
				rankProb[rank] += float64(hits[rank]) * w
			}
		}
	}

	if weightedTotal > 0 {
		for r := 0; r < numPositions; r++ {
			rankProb[r] /= weightedTotal
		}
	} else {
		for r := 0; r < numPositions; r++ {
			rankProb[r] = 1.0 / float64(numPositions)
		}
	}

	probSum := 0.0
	for r := 0; r < numPositions; r++ {
		probSum += rankProb[r]
	}
	if probSum > 0 {
		for r := 0; r < numPositions; r++ {
			rankProb[r] /= probSum
		}
	}

	return PointRankProfile{
		AddedPoints: addedPoints,
		PointMass:   mass,
		ScoutCount:  rawCount,
		RankCounts:  rawRankCounts,
		RankProb:    rankProb,
	}
}

type AdaptiveCellAnalysis struct {
	Team                int     `json:"team"`
	Position            int     `json:"position"`
	ScoutHits           int     `json:"scout_hits"`
	ScoutProbability    float64 `json:"scout_probability"`
	ProvenImpossible    bool    `json:"proven_impossible"`
	PossiblePointTotals []int   `json:"possible_point_totals"`
	HardUpperBound      float64 `json:"hard_upper_bound"`
	PriorityEstimate    float64 `json:"priority_estimate"`
	PointGapEstimate    float64 `json:"point_gap_estimate"`
	PointGapSupport     float64 `json:"point_gap_support"`
	Decision            string  `json:"decision"`
}

type PointProfileGroup struct {
	AllowedPoints          []int     `json:"allowed_points"`
	Mass                   float64   `json:"mass"`
	ConditionalRankProfile []float64 `json:"conditional_rank_profile"`
}

func groupPointRankProfiles(
	profiles []PointRankProfile,
	numPositions int,
	mode string,
	tvdThreshold float64,
) []PointProfileGroup {
	if len(profiles) == 0 {
		return nil
	}

	if mode == "" {
		if envMode := os.Getenv("RARE_POSITION_ADAPTIVE_POINT_GROUP_MODE"); envMode != "" {
			mode = envMode
		} else {
			mode = "profile"
		}
	}

	if tvdThreshold <= 0 {
		if raw := os.Getenv("RARE_POSITION_ADAPTIVE_POINT_GROUP_TVD"); raw != "" {
			if parsed, err := strconv.ParseFloat(raw, 64); err == nil && parsed > 0 {
				tvdThreshold = parsed
			}
		}
		if tvdThreshold <= 0 {
			tvdThreshold = 0.20
		}
	}

	sortedProfiles := make([]PointRankProfile, len(profiles))
	copy(sortedProfiles, profiles)
	sort.Slice(sortedProfiles, func(i, j int) bool {
		return sortedProfiles[i].AddedPoints < sortedProfiles[j].AddedPoints
	})

	var rawGroups [][]PointRankProfile

	if mode == "exact" {
		for _, p := range sortedProfiles {
			rawGroups = append(rawGroups, []PointRankProfile{p})
		}
	} else {
		currentGroup := []PointRankProfile{sortedProfiles[0]}
		for i := 1; i < len(sortedProfiles); i++ {
			prev := currentGroup[len(currentGroup)-1]
			curr := sortedProfiles[i]
			dist := pointRankProfileDistance(prev.RankProb, curr.RankProb)
			if dist <= tvdThreshold {
				currentGroup = append(currentGroup, curr)
			} else {
				rawGroups = append(rawGroups, currentGroup)
				currentGroup = []PointRankProfile{curr}
			}
		}
		if len(currentGroup) > 0 {
			rawGroups = append(rawGroups, currentGroup)
		}
	}

	groups := make([]PointProfileGroup, 0, len(rawGroups))
	for _, rg := range rawGroups {
		allowed := make([]int, len(rg))
		totalMass := 0.0
		rankProfile := make([]float64, numPositions)

		for idx, p := range rg {
			allowed[idx] = p.AddedPoints
			totalMass += p.PointMass
			for r := 0; r < numPositions && r < len(p.RankProb); r++ {
				rankProfile[r] += p.PointMass * p.RankProb[r]
			}
		}

		if totalMass > 0 {
			for r := 0; r < numPositions; r++ {
				rankProfile[r] /= totalMass
			}
		} else {
			for r := 0; r < numPositions; r++ {
				rankProfile[r] = 1.0 / float64(numPositions)
			}
		}

		groups = append(groups, PointProfileGroup{
			AllowedPoints:          allowed,
			Mass:                   totalMass,
			ConditionalRankProfile: rankProfile,
		})
	}

	if len(groups) >= 2 {
		numBase := len(groups)
		for i := 0; i < numBase-1; i++ {
			g1 := groups[i]
			g2 := groups[i+1]
			combinedAllowed := append(append([]int{}, g1.AllowedPoints...), g2.AllowedPoints...)
			combinedMass := g1.Mass + g2.Mass
			combinedProfile := make([]float64, numPositions)
			if combinedMass > 0 {
				for r := 0; r < numPositions; r++ {
					combinedProfile[r] = (g1.Mass*g1.ConditionalRankProfile[r] + g2.Mass*g2.ConditionalRankProfile[r]) / combinedMass
				}
			} else {
				for r := 0; r < numPositions; r++ {
					combinedProfile[r] = 1.0 / float64(numPositions)
				}
			}
			groups = append(groups, PointProfileGroup{
				AllowedPoints:          combinedAllowed,
				Mass:                   combinedMass,
				ConditionalRankProfile: combinedProfile,
			})
		}
	}

	return groups
}

// pointTailProfileGroups adds small, exact-mass tail events. Broad adjacent
// profile groups tend to cover almost the entire points distribution and
// cannot concentrate the rare ranks that motivated conditional sampling.
func pointTailProfileGroups(profiles []PointRankProfile, numPositions int) []PointProfileGroup {
	ordered := append([]PointRankProfile(nil), profiles...)
	sort.Slice(ordered, func(i, j int) bool { return ordered[i].AddedPoints < ordered[j].AddedPoints })
	groups := make([]PointProfileGroup, 0, 6)
	for _, targetMass := range []float64{0.0015, 0.005, 0.02} {
		for _, high := range []bool{false, true} {
			group := PointProfileGroup{ConditionalRankProfile: make([]float64, numPositions)}
			for i := 0; i < len(ordered); i++ {
				profile := ordered[i]
				if high {
					profile = ordered[len(ordered)-1-i]
				}
				group.AllowedPoints = append(group.AllowedPoints, profile.AddedPoints)
				group.Mass += profile.PointMass
				for rank := range group.ConditionalRankProfile {
					group.ConditionalRankProfile[rank] += profile.PointMass * profile.RankProb[rank]
				}
				if group.Mass >= targetMass {
					break
				}
			}
			if group.Mass <= 0 || group.Mass >= 0.25 {
				continue
			}
			for rank := range group.ConditionalRankProfile {
				group.ConditionalRankProfile[rank] /= group.Mass
			}
			groups = append(groups, group)
		}
	}
	return groups
}

// Only candidate ordering borrows a small amount of nearby-rank evidence.
// Conditional validation, never this heuristic, decides whether a cell can
// use the stratum in production.
func proposalConditionalRankScore(profile []float64, rank int) float64 {
	score := profile[rank]
	for other, probability := range profile {
		if other != rank {
			score += 0.3 * probability * math.Exp(-math.Abs(float64(rank-other))/2)
		}
	}
	return score
}

type AdaptivePointProposal struct {
	ID                     string        `json:"id"`
	Team                   int           `json:"team"`
	AllowedPoints          []int         `json:"allowed_points"`
	Mass                   float64       `json:"mass"`
	TargetCells            [][2]int      `json:"target_cells"`
	ConditionalRankProfile []float64     `json:"conditional_rank_profile,omitempty"`
	ValidationSamples      int           `json:"validation_samples"`
	PredictedUtility       float64       `json:"predicted_utility"`
	Stratum                *PointStratum `json:"-"`
}

type AdaptiveStratumAdmitted struct {
	ID             string  `json:"id"`
	TeamID         int     `json:"team_id"`
	MaxRank        int     `json:"max_rank"`
	Threshold      int     `json:"threshold"`
	Mass           float64 `json:"mass"`
	ValidationHits int     `json:"validation_hits"`
	Samples        int     `json:"samples"`
	Work           int64   `json:"work"`
}

type AdaptiveStratumDiagnostics struct {
	ScoutWork         int64                           `json:"scout_work"`
	DiscoveryWork     int64                           `json:"discovery_work"`
	ValidationWork    int64                           `json:"validation_work"`
	ProductionWork    int64                           `json:"production_work"`
	TotalWork         int64                           `json:"total_work"`
	TotalWorkLimit    int64                           `json:"total_work_limit"`
	PlainSamples      int                             `json:"plain_samples"`
	Feasibility       map[[2]int]string               `json:"feasibility,omitempty"`
	UpperBounds       map[[2]int]float64              `json:"upper_bounds,omitempty"`
	CellAnalysis      map[[2]int]AdaptiveCellAnalysis `json:"cell_analysis,omitempty"`
	AdmittedStrata    []AdaptiveStratumAdmitted       `json:"admitted_strata,omitempty"`
	Candidates        int                             `json:"candidates"`
	Validated         int                             `json:"validated"`
	Shortlist         []DiversifiedProposal           `json:"shortlist,omitempty"`
	ValidationSamples map[string]int                  `json:"validation_samples,omitempty"`
	ValidationCells   []DiversifiedValidationCell     `json:"validation_cells,omitempty"`
	ReconciledMatrix  map[int]map[int]float64         `json:"reconciled_matrix,omitempty"`
}

func adaptiveESSUtility(ess float64) float64 {
	first := math.Min(ess, 10.0)
	middle := 0.3 * math.Min(math.Max(ess-10.0, 0.0), 15.0)
	tail := 0.05 * math.Min(math.Max(ess-25.0, 0.0), 75.0)
	return first + middle + tail
}

// additionalPointsPMF computes the exact probability distribution P(S = s) of
// additional points earned by targetTeam in remaining fixtures using DP.
func additionalPointsPMF(universe *PointStratumUniverse) map[int]float64 {
	dp := map[int]float64{0: 1.0}
	if universe == nil {
		return dp
	}
	for slot := range universe.GameIndices {
		nextDP := make(map[int]float64, len(dp)*3)
		probs := universe.OutcomeProbabilities[slot]
		gains := universe.OutcomeGains[slot]
		for pts, p := range dp {
			if p <= 0 {
				continue
			}
			for digit := 0; digit < 3; digit++ {
				probDigit := probs[digit]
				if probDigit <= 0 {
					continue
				}
				gainDigit := gains[digit]
				nextDP[pts+gainDigit] += p * probDigit
			}
		}
		dp = nextDP
	}
	return dp
}

// computeRankRelevance calculates SmoothedRankScore / RankRelevance for (rank, addedPoints)
// using weighted numerator over weighted denominator from joint scout observations.
func computeRankRelevance(
	rank int, addedPoints int, scout *TeamPointRankScout, numPositions int,
) float64 {
	if scout == nil || scout.Samples <= 0 {
		return 1.0 / float64(numPositions)
	}
	weightedNumerator := 0.0
	weightedDenominator := 0.0

	for sNeighbor, count := range scout.PointCounts {
		if count <= 0 {
			continue
		}
		wPoint := math.Exp(-math.Abs(float64(sNeighbor-addedPoints)) / 3.0)
		weightedDenominator += float64(count) * wPoint

		for rOther := 0; rOther < numPositions; rOther++ {
			hits := scout.PointRankCounts[sNeighbor][rOther]
			if hits > 0 {
				wRank := math.Exp(-math.Abs(float64(rOther-rank)) / 1.5)
				weightedNumerator += float64(hits) * wPoint * wRank
			}
		}
	}

	if weightedDenominator > 0 {
		return weightedNumerator / weightedDenominator
	}
	return 1.0 / float64(numPositions)
}

// computePriorityEstimate calculates a heuristic priority score for cell (team, rank)
// using exact PMF and smoothed scout rank relevance P(rank | S = s).
// This is used ONLY for candidate ordering and proposal construction.
func computePriorityEstimate(
	teamID int, rank int, pmf map[int]float64, scout *TeamPointRankScout, numPositions int,
) float64 {
	if scout == nil || scout.Samples <= 0 {
		return 0.0
	}
	score := 0.0
	for s, probS := range pmf {
		if probS <= 0 {
			continue
		}
		rel := computeRankRelevance(rank, s, scout, numPositions)
		score += probS * rel
	}
	return score
}

// pointRankBounds caches the independent minimum and maximum points each team
// can finish with. It gives conservative rank feasibility bounds.
type pointRankBounds struct {
	current map[int]int
	minimum map[int]int
	maximum map[int]int
	teams   []TeamType
}

func buildPointRankBounds(campaign []*TeamCampaign, teamGroups []TeamType, games []*GameType, table *Table) pointRankBounds {
	bounds := pointRankBounds{current: make(map[int]int), minimum: make(map[int]int), maximum: make(map[int]int), teams: teamGroups}
	unplayedPerTeam := make(map[int]int, len(teamGroups))
	for _, g := range games {
		if !g.Played {
			unplayedPerTeam[g.HomeId]++
			unplayedPerTeam[g.AwayId]++
		}
	}
	for _, tg := range teamGroups {
		id := tg.Team_id
		c := campaign[table.Query(uint32(id))]
		if c == nil {
			continue
		}
		bounds.current[id] = c.points
		unplayed := unplayedPerTeam[id]
		pWin := c.points_win
		if pWin < 0 {
			pWin = 3
		}
		pLoss := c.points_loss
		if pLoss < 0 {
			pLoss = 0
		}
		pDraw := c.points_draw
		if pDraw < 0 {
			pDraw = 1
		}

		maxGain := pWin
		if pDraw > maxGain {
			maxGain = pDraw
		}
		if pLoss > maxGain {
			maxGain = pLoss
		}

		minGain := pLoss
		if pDraw < minGain {
			minGain = pDraw
		}
		if pWin < minGain {
			minGain = pWin
		}

		bounds.minimum[id] = c.points + unplayed*minGain
		bounds.maximum[id] = c.points + unplayed*maxGain
	}
	return bounds
}

func (bounds pointRankBounds) rankNotRuledOut(targetTeamID, targetRank, addedPoints int) bool {
	current, found := bounds.current[targetTeamID]
	if !found {
		return false
	}
	targetFinalPoints := current + addedPoints
	numTeams := len(bounds.teams)

	strictlyBetter := 0
	strictlyWorse := 0

	for _, tg := range bounds.teams {
		id := tg.Team_id
		if id == targetTeamID {
			continue
		}
		if bounds.minimum[id] > targetFinalPoints {
			strictlyBetter++
		}
		if bounds.maximum[id] < targetFinalPoints {
			strictlyWorse++
		}
	}

	bestPossibleRank := strictlyBetter
	worstPossibleRank := (numTeams - 1) - strictlyWorse

	return targetRank >= bestPossibleRank && targetRank <= worstPossibleRank
}

// rankNotRuledOutAtAddedPoints returns false if exact rank targetRank is
// impossible for targetTeam when it earns exactly addedPoints additional points.
func rankNotRuledOutAtAddedPoints(
	targetTeamID int, targetRank int, addedPoints int,
	campaign []*TeamCampaign, teamGroups []TeamType, games []*GameType, table *Table,
) bool {
	bounds := buildPointRankBounds(campaign, teamGroups, games, table)
	return bounds.rankNotRuledOut(targetTeamID, targetRank, addedPoints)
}

// computeHardCellUpperBound calculates HardUpperBound(team, rank) as
// sum_{s in PossiblePointTotals} P(S = s).
func computeHardCellUpperBound(
	targetTeamID int, targetRank int,
	pmf map[int]float64,
	campaign []*TeamCampaign, teamGroups []TeamType, games []*GameType, table *Table,
) (hardBound float64, possibleTotals []int, provenImpossible bool) {
	bounds := buildPointRankBounds(campaign, teamGroups, games, table)
	return computeHardCellUpperBoundWithBounds(targetTeamID, targetRank, pmf, bounds)
}

func computeHardCellUpperBoundWithBounds(
	targetTeamID int, targetRank int, pmf map[int]float64, bounds pointRankBounds,
) (hardBound float64, possibleTotals []int, provenImpossible bool) {
	possibleTotals = make([]int, 0, len(pmf))
	hardBound = 0.0

	for s, p := range pmf {
		if p <= 0 {
			continue
		}
		if bounds.rankNotRuledOut(targetTeamID, targetRank, s) {
			possibleTotals = append(possibleTotals, s)
			hardBound += p
		}
	}

	sort.Ints(possibleTotals)
	provenImpossible = (len(possibleTotals) == 0 || hardBound <= 0)
	if provenImpossible {
		hardBound = 0.0
	}
	return hardBound, possibleTotals, provenImpossible
}

func runPlainMCScoutWithJointPoints(
	baseCampaign []*TeamCampaign,
	games []*GameType,
	table *Table,
	sortOrder []SortType,
	teamGroups []TeamType,
	scoutSamples int,
	plainWorkPerSample int64,
	rng *rand.Rand,
) ScoutData {
	return runPlainMCScoutWithJointPointsBatched(baseCampaign, games, table, sortOrder,
		teamGroups, scoutSamples, plainWorkPerSample, rng, 0, true)
}

func runPlainMCScoutWithJointPointsBatched(
	baseCampaign []*TeamCampaign,
	games []*GameType,
	table *Table,
	sortOrder []SortType,
	teamGroups []TeamType,
	scoutSamples int,
	plainWorkPerSample int64,
	rng *rand.Rand,
	batchCount int,
	capturePointGaps bool,
) ScoutData {
	if rng == nil {
		rng = rand.New(rand.NewSource(1))
	}
	numPositions := len(teamGroups)

	counts := make(map[int][]int, len(teamGroups))
	teamScoutMap := make(map[int]*TeamPointRankScout, len(teamGroups))

	for _, team := range teamGroups {
		id := team.Team_id
		counts[id] = make([]int, numPositions)
		teamScoutMap[id] = &TeamPointRankScout{
			Samples:         scoutSamples,
			RankCounts:      counts[id],
			PointRankCounts: make(map[int][]int),
			PointCounts:     make(map[int]int),
		}
	}
	var batches []map[int]*TeamPointRankScout
	if batchCount > 0 && scoutSamples > 0 {
		if batchCount > scoutSamples {
			batchCount = scoutSamples
		}
		batches = make([]map[int]*TeamPointRankScout, batchCount)
		for batch := range batches {
			batches[batch] = make(map[int]*TeamPointRankScout, len(teamGroups))
			for _, team := range teamGroups {
				batches[batch][team.Team_id] = &TeamPointRankScout{
					RankCounts:      make([]int, numPositions),
					PointRankCounts: make(map[int][]int),
					PointCounts:     make(map[int]int),
				}
			}
		}
	}

	simCampaign := make([]*TeamCampaign, len(baseCampaign))
	teamSlice := make([]*TeamCampaign, len(teamGroups))
	var pointGaps *PointGapScout
	if capturePointGaps && os.Getenv("RARE_POSITION_POINT_GAP") == "1" && len(sortOrder) > 0 && sortOrder[0] == PT {
		bounds := buildPointRankBounds(baseCampaign, teamGroups, games, table)
		minPoints, maxPoints := math.MaxInt, math.MinInt
		for _, team := range teamGroups {
			if minimum, ok := bounds.minimum[team.Team_id]; ok && minimum < minPoints {
				minPoints = minimum
			}
			if maximum, ok := bounds.maximum[team.Team_id]; ok && maximum > maxPoints {
				maxPoints = maximum
			}
		}
		if minPoints <= maxPoints {
			pointGaps = newPointGapScout(minPoints, maxPoints, numPositions)
		}
	}

	for s := 0; s < scoutSamples; s++ {
		batchIndex := 0
		if len(batches) > 0 {
			batchIndex = s * len(batches) / scoutSamples
		}
		for i, c := range baseCampaign {
			if c != nil {
				simCampaign[i] = c.clone()
			} else {
				simCampaign[i] = nil
			}
		}

		for _, game := range games {
			if game.Played {
				continue
			}
			hs := poissonRand(rng, game.HomePower)
			as := poissonRand(rng, game.AwayPower)
			home, away := game.home_table_index, game.away_table_index
			played := &GameType{game.Id, game.HomeId, game.AwayId, hs, as, 0, 0, true, home, away}
			if simCampaign[home] != nil {
				simCampaign[home].add_game(played)
			}
			if simCampaign[away] != nil {
				simCampaign[away].add_game(played)
			}
		}

		idx := 0
		for _, team := range teamGroups {
			if c := simCampaign[table.Query(uint32(team.Team_id))]; c != nil {
				teamSlice[idx] = c
				idx++
			}
		}
		sort.Sort(TeamCampaignSorted{t: teamSlice[:idx], sort: sortOrder, rng: rng})
		if pointGaps != nil {
			pointGaps.addTable(teamSlice[:idx])
		}

		for rank, c := range teamSlice[:idx] {
			teamID := c.id
			baseC := baseCampaign[table.Query(uint32(teamID))]
			added := c.points - baseC.points

			counts[teamID][rank]++

			ts := teamScoutMap[teamID]
			if ts.PointRankCounts[added] == nil {
				ts.PointRankCounts[added] = make([]int, numPositions)
			}
			ts.PointRankCounts[added][rank]++
			ts.PointCounts[added]++
			if len(batches) > 0 {
				bs := batches[batchIndex][teamID]
				bs.Samples++
				bs.RankCounts[rank]++
				if bs.PointRankCounts[added] == nil {
					bs.PointRankCounts[added] = make([]int, numPositions)
				}
				bs.PointRankCounts[added][rank]++
				bs.PointCounts[added]++
			}
		}
	}

	data := ScoutData{
		Samples:          scoutSamples,
		Work:             int64(scoutSamples) * plainWorkPerSample,
		TeamCounts:       counts,
		TeamProbs:        make(map[int][]float64, len(teamGroups)),
		TeamMeanRanks:    make(map[int]float64, len(teamGroups)),
		MinObservedRank:  make(map[int]int, len(teamGroups)),
		MaxObservedRank:  make(map[int]int, len(teamGroups)),
		Feasibility:      make(map[[2]int]string),
		TeamScout:        teamScoutMap,
		PointRankBatches: batches,
		PointGaps:        pointGaps,
	}

	for _, team := range teamGroups {
		id := team.Team_id
		teamCounts := counts[id]
		probs := make([]float64, numPositions)
		meanRank := 0.0
		minObserved := numPositions
		maxObserved := -1

		for pos, c := range teamCounts {
			p := float64(c) / float64(scoutSamples)
			probs[pos] = p
			meanRank += float64(pos) * p
			if c > 0 {
				if pos < minObserved {
					minObserved = pos
				}
				if pos > maxObserved {
					maxObserved = pos
				}
				data.Feasibility[[2]int{id, pos}] = "observed"
			} else {
				feasible := possiblePositionByPointsBoundsWithRNG(id, pos, baseCampaign, teamGroups, games, table, sortOrder, rng)
				if feasible {
					data.Feasibility[[2]int{id, pos}] = "feasible_unseen"
				} else {
					data.Feasibility[[2]int{id, pos}] = "proven_impossible"
				}
			}
		}

		data.TeamProbs[id] = probs
		data.TeamMeanRanks[id] = meanRank
		data.MinObservedRank[id] = minObserved
		data.MaxObservedRank[id] = maxObserved
	}

	return data
}

func simulateAdaptivePlainSeasons(
	base []*TeamCampaign, games []*GameType, table *Table, order []SortType,
	teamGroups []TeamType, strata map[int]*PointStratum,
	samples int, seed int64,
) (plainCounts map[[2]int]int, outsideCounts map[[2]int]int) {
	rng := rand.New(rand.NewSource(seed))
	plainCounts = make(map[[2]int]int, len(teamGroups)*len(teamGroups))
	outsideCounts = make(map[[2]int]int, len(teamGroups)*len(teamGroups))

	type teamUniverseInfo struct {
		universe *PointStratumUniverse
		stratum  *PointStratum
	}
	infoByTeam := make(map[int]teamUniverseInfo, len(strata))
	for teamID, stratum := range strata {
		if stratum != nil && stratum.Universe != nil {
			infoByTeam[teamID] = teamUniverseInfo{
				universe: stratum.Universe,
				stratum:  stratum,
			}
		}
	}

	campaign := make([]*TeamCampaign, len(base))
	teamSlice := make([]*TeamCampaign, len(teamGroups))

	for s := 0; s < samples; s++ {
		for i, c := range base {
			if c != nil {
				campaign[i] = c.clone()
			} else {
				campaign[i] = nil
			}
		}

		observedCodes := make(map[int]uint64, len(infoByTeam))

		for i, game := range games {
			if game.Played {
				continue
			}
			hs := poissonRand(rng, game.HomePower)
			as := poissonRand(rng, game.AwayPower)

			for teamID, info := range infoByTeam {
				if slot, ok := info.universe.GameSlots[i]; ok {
					digit := targetOutcomeDigit(game.HomeId == teamID, hs, as)
					observedCodes[teamID] += uint64(digit) * info.universe.GamePowers[slot]
				}
			}

			home, away := game.home_table_index, game.away_table_index
			played := &GameType{game.Id, game.HomeId, game.AwayId, hs, as, 0, 0, true, home, away}
			if campaign[home] != nil {
				campaign[home].add_game(played)
			}
			if campaign[away] != nil {
				campaign[away].add_game(played)
			}
		}

		idx := 0
		for _, team := range teamGroups {
			if c := campaign[table.Query(uint32(team.Team_id))]; c != nil {
				teamSlice[idx] = c
				idx++
			}
		}
		sort.Sort(TeamCampaignSorted{t: teamSlice[:idx], sort: order, rng: rng})

		for rank, c := range teamSlice[:idx] {
			cell := [2]int{c.id, rank}
			plainCounts[cell]++
			if info, ok := infoByTeam[c.id]; ok {
				code := observedCodes[c.id]
				if !info.stratum.contains(code) {
					outsideCounts[cell]++
				}
			}
		}
	}

	return plainCounts, outsideCounts
}

type pointContribution struct {
	points int
	score  float64
}

// buildAdaptivePointProposal constructs a localized AdaptivePointProposal
// for a team with under-resolved target cells based on point contribution scores.
func directScoutOutsideHits(teamID int, rank int, allowedPoints []int, scout *TeamPointRankScout) int {
	if scout == nil || scout.PointRankCounts == nil {
		return 0
	}
	allowedMap := make(map[int]bool, len(allowedPoints))
	for _, pts := range allowedPoints {
		allowedMap[pts] = true
	}
	outsideHits := 0
	for s, hits := range scout.PointRankCounts {
		if !allowedMap[s] && rank < len(hits) {
			outsideHits += hits[rank]
		}
	}
	return outsideHits
}

func buildAdaptivePointProposalsForTeam(
	teamID int,
	currentPoints int,
	targetCells [][2]int,
	universe *PointStratumUniverse,
	pmf map[int]float64,
	scout *TeamPointRankScout,
	pointGaps *PointGapScout,
	numPositions int,
	cellAnalysis map[[2]int]AdaptiveCellAnalysis,
) []AdaptivePointProposal {
	if universe == nil || len(targetCells) == 0 {
		return nil
	}

	profiles := make([]PointRankProfile, 0, len(pmf))
	for s, p := range pmf {
		if p > 0 {
			prof := computePointRankProfile(s, pmf, scout, numPositions, 0)
			profiles = append(profiles, prof)
			if os.Getenv("RARE_POSITION_ADAPTIVE_DEBUG") == "1" {
				log.Printf("rare-position-point-profile: team=%d points=%d pmf_mass=%.6g scout_count=%d rank_profile=%v",
					teamID, s, p, prof.ScoutCount, prof.RankProb)
			}
		}
	}

	if len(profiles) == 0 {
		return nil
	}

	tailGroups := pointTailProfileGroups(profiles, numPositions)
	groups := tailGroups
	if os.Getenv("RARE_POSITION_ADAPTIVE_TAIL_ONLY") == "0" {
		groups = append(groupPointRankProfiles(profiles, numPositions, "", 0), tailGroups...)
	}
	if len(groups) == 0 {
		return nil
	}

	maxCandidatesPerTeam := 3
	if raw := os.Getenv("RARE_POSITION_ADAPTIVE_MAX_CANDIDATES_PER_TEAM"); raw != "" {
		if parsed, err := strconv.Atoi(raw); err == nil && parsed > 0 {
			maxCandidatesPerTeam = parsed
		}
	}

	proposals := make([]AdaptivePointProposal, 0, len(groups))

	for idx, g := range groups {
		if g.Mass <= 0 || g.Mass >= 0.25 {
			continue
		}

		targetUtility := 0.0
		usefulCells := make([][2]int, 0, len(targetCells))

		for _, cell := range targetCells {
			r := cell[1]
			pCond := proposalConditionalRankScore(g.ConditionalRankProfile, r)
			if os.Getenv("RARE_POSITION_POINT_GAP_USE_PRIORITY") == "1" && pointGaps != nil {
				gapCond, supportedMass := pointGapConditionalEstimate(pmf, g.AllowedPoints, currentPoints, r, pointGaps)
				if supportedMass >= 0.95*g.Mass {
					pCond = 0.5*pCond + 0.5*gapCond
				}
			}
			if pCond <= 0 {
				continue
			}
			usefulCells = append(usefulCells, cell)
			inside := g.Mass * pCond
			outside := float64(directScoutOutsideHits(teamID, r, g.AllowedPoints, scout)) / float64(scout.Samples)
			pPred := outside + inside
			const predictedPlainSamples = 100000.0
			const predictedConditionalSamples = 1000.0
			plainVariance := pPred * (1 - pPred) / predictedPlainSamples
			hybridVariance := outside*(1-outside)/predictedPlainSamples +
				g.Mass*g.Mass*pCond*(1-pCond)/predictedConditionalSamples
			if hybridVariance > 0 && plainVariance > 0 {
				plainESS := pPred * pPred / plainVariance
				hybridESS := pPred * pPred / hybridVariance
				gain := math.Min(hybridESS, 10) - math.Min(plainESS, 10)
				if gain > 0 {
					targetUtility += gain
				}
			}
		}

		if len(usefulCells) == 0 || targetUtility <= 0 {
			continue
		}

		sort.Ints(g.AllowedPoints)
		minAllowed := g.AllowedPoints[0]
		allowedMap := make(map[int]bool, len(g.AllowedPoints))
		for _, pts := range g.AllowedPoints {
			allowedMap[pts] = true
		}

		isTrueTail := true
		for s, p := range pmf {
			if p > 0 && s >= minAllowed {
				if !allowedMap[s] {
					isTrueTail = false
					break
				}
			}
		}

		var stratum *PointStratum
		var ok bool
		if isTrueTail {
			stratum, ok = makePointTailStratum(universe, minAllowed)
		} else {
			stratum, ok = makePointSetStratum(universe, g.AllowedPoints)
		}

		if !ok || stratum == nil || stratum.Mass <= 0 {
			continue
		}

		propID := fmt.Sprintf("team%d_grp%d_pts%v", teamID, idx, g.AllowedPoints)
		prop := AdaptivePointProposal{
			ID:                     propID,
			Team:                   teamID,
			AllowedPoints:          g.AllowedPoints,
			Mass:                   stratum.Mass,
			TargetCells:            usefulCells,
			ConditionalRankProfile: g.ConditionalRankProfile,
			PredictedUtility:       targetUtility,
			Stratum:                stratum,
		}
		proposals = append(proposals, prop)

		if os.Getenv("RARE_POSITION_ADAPTIVE_DEBUG") == "1" {
			log.Printf("rare-position-adaptive-proposal: proposal=%s team=%d points=%v mass=%.6g target_cells=%v predicted_rank_probs=%v predicted_utility=%.6g",
				prop.ID, prop.Team, prop.AllowedPoints, prop.Mass, prop.TargetCells, prop.ConditionalRankProfile, prop.PredictedUtility)
		}
	}

	sort.Slice(proposals, func(i, j int) bool {
		if proposals[i].PredictedUtility == proposals[j].PredictedUtility {
			return proposals[i].ID < proposals[j].ID
		}
		return proposals[i].PredictedUtility > proposals[j].PredictedUtility
	})

	if len(proposals) > maxCandidatesPerTeam {
		proposals = proposals[:maxCandidatesPerTeam]
	}

	return proposals
}

// runAdaptivePointStratifiedSearch automatically discovers team-position point
// strata across all teams and ranks, validates them in design, freezes work
// allocation, and performs fresh production simulation.
func runAdaptivePointStratifiedSearch(
	group *GroupType, base []*TeamCampaign, table *Table, order []SortType,
	workLimit, masterSeed int64,
) (map[int]map[int]ProductionEstimate, AdaptiveStratumDiagnostics, bool) {
	diag := AdaptiveStratumDiagnostics{
		TotalWorkLimit:   workLimit,
		Feasibility:      make(map[[2]int]string),
		UpperBounds:      make(map[[2]int]float64),
		CellAnalysis:     make(map[[2]int]AdaptiveCellAnalysis),
		ReconciledMatrix: make(map[int]map[int]float64),
	}

	numTeams := len(group.Team_groups)
	unplayed := 0
	for _, game := range group.Games {
		if !game.Played {
			unplayed++
		}
	}
	plainCost := estimateSeasonWork(unplayed, 1, numTeams)
	stratumCost := estimateSeasonWork(unplayed, 2, numTeams)
	if plainCost <= 0 || stratumCost <= 0 {
		return nil, diag, false
	}

	// 1. Initial plain-MC design scout run with joint points x rank tracking
	scoutRequested := 1000
	if raw := os.Getenv("RARE_POSITION_ADAPTIVE_SCOUT_SAMPLES"); raw != "" {
		if parsed, err := strconv.Atoi(raw); err == nil && parsed > 0 {
			scoutRequested = parsed
		}
	} else if raw := os.Getenv("RARE_POSITION_DIVERSIFIED_SCOUT_SAMPLES"); raw != "" {
		if parsed, err := strconv.Atoi(raw); err == nil && parsed > 0 {
			scoutRequested = parsed
		}
	}

	scoutSamples := affordableSamples(scoutRequested, workLimit, plainCost)
	if scoutSamples <= 0 {
		return nil, diag, false
	}
	diag.ScoutWork = int64(scoutSamples) * plainCost

	scoutRNG := rand.New(rand.NewSource(deriveRarePositionSeed(masterSeed, "adaptive-scout")))
	scout := runPlainMCScoutWithJointPoints(base, group.Games, table, order, group.Team_groups, scoutSamples, plainCost, scoutRNG)
	rankBounds := buildPointRankBounds(base, group.Team_groups, group.Games, table)

	for cell, status := range scout.Feasibility {
		diag.Feasibility[cell] = status
	}

	simulateThreshold := 1e-6
	if raw := os.Getenv("RARE_POSITION_ADAPTIVE_SIMULATE_THRESHOLD"); raw != "" {
		if parsed, err := strconv.ParseFloat(raw, 64); err == nil && parsed > 0 {
			simulateThreshold = parsed
		}
	}

	reportFloor := 1e-7
	if raw := os.Getenv("RARE_POSITION_ADAPTIVE_REPORT_FLOOR"); raw != "" {
		if parsed, err := strconv.ParseFloat(raw, 64); err == nil && parsed > 0 {
			reportFloor = parsed
		}
	}

	targetScoutESS := 25
	if raw := os.Getenv("RARE_POSITION_ADAPTIVE_TARGET_SCOUT_ESS"); raw != "" {
		if parsed, err := strconv.Atoi(raw); err == nil && parsed > 0 {
			targetScoutESS = parsed
		}
	}

	// 2 & 3 & 4. Points/rank analysis, exact PMF, priority estimates, and hard upper bounds
	candidateProposals := make([]AdaptivePointProposal, 0, numTeams)
	// Charge the points DP and repeated rank-bound scans as design work.
	// A work unit follows estimateSeasonWork's game/team operation convention.
	diag.DiscoveryWork = 0
	if scout.PointGaps != nil {
		diag.DiscoveryWork += scout.PointGaps.Work
	}

	for _, team := range group.Team_groups {
		teamID := team.Team_id
		universe, ok := pointOutcomeUniverse(teamID, base, table, group.Games, 39)
		if !ok || universe == nil {
			continue
		}

		pmf := additionalPointsPMF(universe)
		diag.DiscoveryWork += int64(len(universe.GameIndices)*len(pmf)*3 + numTeams*len(pmf)*numTeams)
		teamScout := scout.TeamScout[teamID]
		currentPoints := rankBounds.current[teamID]

		unresolvedTargetCells := make([][2]int, 0, numTeams)

		for pos := 0; pos < numTeams; pos++ {
			cell := [2]int{teamID, pos}

			hardUB, possibleTotals, provenImp := computeHardCellUpperBoundWithBounds(teamID, pos, pmf, rankBounds)
			scoutHits := scout.TeamCounts[teamID][pos]
			scoutProb := float64(scoutHits) / float64(scoutSamples)
			priorityEst := computePriorityEstimate(teamID, pos, pmf, teamScout, numTeams)
			gapEst, gapSupport := pointGapSmoothedEstimate(pmf, currentPoints, pos, scout.PointGaps, 0)
			if provenImp {
				gapEst = 0
			} else if gapEst > hardUB {
				gapEst = hardUB
			}

			decision := "under_resolved_candidate"
			if provenImp {
				decision = "proven_impossible"
			} else if scoutHits >= targetScoutESS {
				decision = "well_observed"
			} else if hardUB < reportFloor {
				decision = "bound_only_tiny"
			} else if hardUB < simulateThreshold {
				decision = "bound_only"
			}

			analysis := AdaptiveCellAnalysis{
				Team:                teamID,
				Position:            pos,
				ScoutHits:           scoutHits,
				ScoutProbability:    scoutProb,
				ProvenImpossible:    provenImp,
				PossiblePointTotals: possibleTotals,
				HardUpperBound:      hardUB,
				PriorityEstimate:    priorityEst,
				PointGapEstimate:    gapEst,
				PointGapSupport:     gapSupport,
				Decision:            decision,
			}
			diag.CellAnalysis[cell] = analysis
			diag.UpperBounds[cell] = hardUB

			if os.Getenv("RARE_POSITION_ADAPTIVE_DEBUG") == "1" {
				log.Printf("rare-position-adaptive-cell: team=%d rank=%d scout_hits=%d hard_upper_bound=%.6g priority=%.6g possible_points=%v decision=%s",
					teamID, pos, scoutHits, hardUB, priorityEst, possibleTotals, decision)
			}

			if !provenImp && scoutHits < targetScoutESS && hardUB >= simulateThreshold {
				unresolvedTargetCells = append(unresolvedTargetCells, cell)
			}
		}

		if len(unresolvedTargetCells) > 0 {
			teamProps := buildAdaptivePointProposalsForTeam(teamID, currentPoints, unresolvedTargetCells, universe, pmf, teamScout, scout.PointGaps, numTeams, diag.CellAnalysis)
			candidateProposals = append(candidateProposals, teamProps...)
		}
	}

	valSamplesPerStratum := 200
	if raw := os.Getenv("RARE_POSITION_ADAPTIVE_VALIDATION_INITIAL_SAMPLES"); raw != "" {
		if parsed, err := strconv.Atoi(raw); err == nil && parsed > 0 {
			valSamplesPerStratum = parsed
		}
	}

	sort.Slice(candidateProposals, func(i, j int) bool {
		if candidateProposals[i].PredictedUtility == candidateProposals[j].PredictedUtility {
			return candidateProposals[i].ID < candidateProposals[j].ID
		}
		return candidateProposals[i].PredictedUtility > candidateProposals[j].PredictedUtility
	})
	diag.Candidates = len(candidateProposals)

	// 5. Validation probes on shortlisted candidate proposals (Design)

	valWorkCapEq := int64(3000)
	if raw := os.Getenv("RARE_POSITION_ADAPTIVE_VALIDATION_WORK_CAP_EQ"); raw != "" {
		if parsed, err := strconv.ParseInt(raw, 10, 64); err == nil && parsed > 0 {
			valWorkCapEq = parsed
		}
	}
	valWorkCap := valWorkCapEq * plainCost

	maxValProposals := 8
	if raw := os.Getenv("RARE_POSITION_ADAPTIVE_MAX_VALIDATED_PROPOSALS"); raw != "" {
		if parsed, err := strconv.Atoi(raw); err == nil && parsed > 0 {
			maxValProposals = parsed
		}
	}

	if len(candidateProposals) > maxValProposals {
		candidateProposals = candidateProposals[:maxValProposals]
	}

	// Physical validation budget enforcement: cap candidate count so actual validation work <= valWorkCap
	singlePropValWork := int64(valSamplesPerStratum) * stratumCost
	if singlePropValWork > 0 {
		maxAffordable := int(valWorkCap / singlePropValWork)
		if len(candidateProposals) > maxAffordable {
			if maxAffordable <= 0 {
				candidateProposals = nil
			} else {
				candidateProposals = candidateProposals[:maxAffordable]
			}
		}
	}

	valWorkTotal := int64(len(candidateProposals)*valSamplesPerStratum) * stratumCost
	diag.ValidationWork = valWorkTotal
	if diag.ScoutWork+diag.DiscoveryWork+diag.ValidationWork+2*plainCost > workLimit {
		return nil, diag, false
	}
	diag.Validated = len(candidateProposals)
	diag.ValidationSamples = make(map[string]int, len(candidateProposals))
	for _, prop := range candidateProposals {
		diag.Shortlist = append(diag.Shortlist, DiversifiedProposal{ID: prop.ID, Kind: "point_outcome_stratum",
			TargetTeam: prop.Team, Strength: float64(prop.AllowedPoints[0]), KL: -math.Log(prop.Mass), WorkPerSample: stratumCost})
	}

	type validationResult struct {
		prop              AdaptivePointProposal
		targetHits        int
		valHitsPerCell    map[[2]int]int
		validationSamples int
		pCondHit          map[[2]int]float64
		varCondPerSample  map[[2]int]float64
	}

	valResults := make([]validationResult, 0, len(candidateProposals))

	if valSamplesPerStratum > 0 && valWorkTotal > 0 {
		for _, prop := range candidateProposals {
			diag.ValidationSamples[prop.ID] = valSamplesPerStratum
			valSeed := deriveRarePositionSeed(masterSeed, fmt.Sprintf("adaptive-val-team%d", prop.Team))
			valCounts, _ := simulatePointHybridSeasons(base, group.Games, table, order, group.Team_groups,
				prop.Stratum, true, valSamplesPerStratum, valSeed)

			targetHits := 0
			valHitsPerCell := make(map[[2]int]int, len(prop.TargetCells))
			pCondHit := make(map[[2]int]float64, len(prop.TargetCells))
			varCondPerSample := make(map[[2]int]float64, len(prop.TargetCells))

			targetHitsByRank := make(map[int]int, len(prop.TargetCells))
			for _, cell := range prop.TargetCells {
				hits := valCounts[cell]
				valHitsPerCell[cell] = hits
				targetHits += hits
				targetHitsByRank[cell[1]] = hits

				qReg := (float64(hits) + 0.5) / (float64(valSamplesPerStratum) + 1.0)
				pCondHit[cell] = qReg
				varCondPerSample[cell] = prop.Stratum.Mass * prop.Stratum.Mass * qReg * (1.0 - qReg)
				diag.ValidationCells = append(diag.ValidationCells, DiversifiedValidationCell{
					Proposal: prop.ID, Team: cell[0], Position: cell[1], Intended: true,
					ESS: float64(hits), Eligible: hits >= 2, VariancePerSample: varCondPerSample[cell],
				})
			}

			log.Printf("rare-position-adaptive-validation: proposal=%s team=%d points=%v mass=%.6g samples=%d work=%d rank_hits=%v target_hits=%d decision=%s",
				prop.ID, prop.Team, prop.AllowedPoints, prop.Mass, valSamplesPerStratum, int64(valSamplesPerStratum)*stratumCost,
				targetHitsByRank, targetHits,
				map[bool]string{true: "admitted", false: "rejected"}[targetHits >= 2])

			if targetHits >= 2 {
				valResults = append(valResults, validationResult{
					prop:              prop,
					targetHits:        targetHits,
					valHitsPerCell:    valHitsPerCell,
					validationSamples: valSamplesPerStratum,
					pCondHit:          pCondHit,
					varCondPerSample:  varCondPerSample,
				})
			}
		}
	}
	// Production supports one conditional stratum per team. Keep the first
	// independently admitted proposal in the frozen utility order.
	uniqueResults := valResults[:0]
	seenTeam := make(map[int]bool, len(valResults))
	for _, result := range valResults {
		if !seenTeam[result.prop.Team] {
			uniqueResults = append(uniqueResults, result)
			seenTeam[result.prop.Team] = true
		}
	}
	valResults = uniqueResults

	// 6. Freeze work allocation using greedy ESS utility and a plain-work floor.
	remainingWork := workLimit - diag.ScoutWork - diag.DiscoveryWork - diag.ValidationWork
	if remainingWork <= 0 {
		return nil, diag, false
	}

	minPlainFraction := 0.975
	if raw := os.Getenv("RARE_POSITION_ADAPTIVE_MIN_PLAIN_PRODUCTION_FRACTION"); raw != "" {
		if parsed, err := strconv.ParseFloat(raw, 64); err == nil && parsed >= 0 && parsed <= 1 {
			minPlainFraction = parsed
		}
	}

	maxCondWork := int64(float64(remainingWork) * (1.0 - minPlainFraction))

	type admittedProdStream struct {
		prop    AdaptivePointProposal
		samples int
		work    int64
		valHits int
	}

	prodStreams := make([]admittedProdStream, 0, len(valResults))
	totalStratumWork := int64(0)

	// Whole-matrix utility-driven production scheduler: SWAP conditional work against plain MC
	if len(valResults) > 0 && maxCondWork >= stratumCost {
		chunkRequested := int64(100000)
		chunkSamples := int(chunkRequested / stratumCost)
		if chunkSamples <= 0 {
			chunkSamples = 1
		}
		actualChunkWork := int64(chunkSamples) * stratumCost

		allocatedSamples := make([]int, len(valResults))
		allocatedWork := make([]int64, len(valResults))

		cellA := make(map[[2]int]float64, numTeams*numTeams)
		cellQ := make(map[[2]int]float64, numTeams*numTeams)
		cellP := make(map[[2]int]float64, numTeams*numTeams)
		cellPropIdx := make(map[[2]int]int, numTeams*numTeams)

		for _, team := range group.Team_groups {
			for pos := 0; pos < numTeams; pos++ {
				cell := [2]int{team.Team_id, pos}
				scoutHits := scout.TeamCounts[team.Team_id][pos]
				pScout := (float64(scoutHits) + 0.5) / (float64(scoutSamples) + 1.0)
				cellP[cell] = pScout
				cellA[cell] = pScout
				cellPropIdx[cell] = -1
			}
		}

		for idx, vr := range valResults {
			for _, cell := range vr.prop.TargetCells {
				if vr.valHitsPerCell[cell] >= 2 {
					cellPropIdx[cell] = idx
					q := vr.pCondHit[cell]
					cellQ[cell] = q
					scoutOutsideHits := directScoutOutsideHits(cell[0], cell[1], vr.prop.AllowedPoints, scout.TeamScout[cell[0]])
					inside := vr.prop.Mass * q
					cellA[cell] = float64(scoutOutsideHits) / float64(scoutSamples)
					cellP[cell] = cellA[cell] + inside
				}
			}
		}

		remainingCondWork := maxCondWork

		for remainingCondWork >= actualChunkWork {
			bestValIdx := -1
			bestUtilityDelta := 0.0

			currentBProd := remainingWork
			currentBQ := totalStratumWork
			currentBP := currentBProd - currentBQ
			currentNP := int(currentBP / plainCost)
			if currentNP <= 1 {
				break
			}

			currentWholeMatrixUtility := 0.0
			for _, team := range group.Team_groups {
				for pos := 0; pos < numTeams; pos++ {
					cell := [2]int{team.Team_id, pos}
					p := cellP[cell]
					propIdx := cellPropIdx[cell]

					vCell := p * (1.0 - p) / float64(currentNP)
					if propIdx >= 0 && allocatedSamples[propIdx] > 0 {
						a := cellA[cell]
						q := cellQ[cell]
						m := valResults[propIdx].prop.Mass
						nQ := allocatedSamples[propIdx]
						vCell = a*(1.0-a)/float64(currentNP) + m*m*q*(1.0-q)/float64(nQ)
					}

					essCell := 0.0
					if vCell > 0 {
						essCell = p * p / vCell
					}
					currentWholeMatrixUtility += adaptiveESSUtility(essCell)
				}
			}

			for idx := range valResults {
				tentativeBQ := currentBQ + actualChunkWork
				tentativeBP := currentBProd - tentativeBQ
				tentativeNP := int(tentativeBP / plainCost)
				if tentativeNP <= 1 {
					continue
				}

				tentativeNQ := allocatedSamples[idx] + chunkSamples
				tentativeWholeMatrixUtility := 0.0

				for _, team := range group.Team_groups {
					for pos := 0; pos < numTeams; pos++ {
						cell := [2]int{team.Team_id, pos}
						p := cellP[cell]
						propIdx := cellPropIdx[cell]

						vCell := p * (1.0 - p) / float64(tentativeNP)
						if propIdx >= 0 {
							nQ := allocatedSamples[propIdx]
							if propIdx == idx {
								nQ = tentativeNQ
							}
							if nQ > 0 {
								a := cellA[cell]
								q := cellQ[cell]
								m := valResults[propIdx].prop.Mass
								vCell = a*(1.0-a)/float64(tentativeNP) + m*m*q*(1.0-q)/float64(nQ)
							}
						}

						essCell := 0.0
						if vCell > 0 {
							essCell = p * p / vCell
						}
						tentativeWholeMatrixUtility += adaptiveESSUtility(essCell)
					}
				}

				deltaU := tentativeWholeMatrixUtility - currentWholeMatrixUtility
				if deltaU > bestUtilityDelta {
					bestUtilityDelta = deltaU
					bestValIdx = idx
				}
			}

			if bestValIdx < 0 || bestUtilityDelta <= 0 {
				break
			}

			allocatedWork[bestValIdx] += actualChunkWork
			allocatedSamples[bestValIdx] += chunkSamples
			remainingCondWork -= actualChunkWork
			totalStratumWork += actualChunkWork
		}

		for idx, vr := range valResults {
			if allocatedSamples[idx] > 0 {
				prodStreams = append(prodStreams, admittedProdStream{
					prop:    vr.prop,
					samples: allocatedSamples[idx],
					work:    allocatedWork[idx],
					valHits: vr.targetHits,
				})
			}
		}
	}

	plainSamples := int((remainingWork - totalStratumWork) / plainCost)
	if plainSamples <= 1 {
		prodStreams = nil
		totalStratumWork = 0
		plainSamples = int(remainingWork / plainCost)
		if plainSamples <= 1 {
			return nil, diag, false
		}
	}

	diag.PlainSamples = plainSamples
	diag.ProductionWork = int64(plainSamples)*plainCost + totalStratumWork
	diag.TotalWork = diag.ScoutWork + diag.DiscoveryWork + diag.ValidationWork + diag.ProductionWork

	if diag.TotalWork > workLimit {
		panic("adaptive stratum work budget exceeded")
	}

	// Freeze cell-level estimator decisions BEFORE fresh production based on design predicted variances
	frozenUseHybrid := make(map[[2]int]bool, numTeams*numTeams)

	for _, ps := range prodStreams {
		nQ := ps.samples
		if nQ <= 0 {
			continue
		}
		m := ps.prop.Mass
		var vrMatching *validationResult
		for i := range valResults {
			if valResults[i].prop.ID == ps.prop.ID {
				vrMatching = &valResults[i]
				break
			}
		}
		if vrMatching == nil {
			continue
		}

		for _, cell := range ps.prop.TargetCells {
			// Per-cell validation gate: MUST have >= 2 validation hits for this specific cell
			if vrMatching.valHitsPerCell[cell] < 2 {
				continue
			}

			scoutOutsideHits := directScoutOutsideHits(cell[0], cell[1], ps.prop.AllowedPoints, scout.TeamScout[cell[0]])
			qVal := vrMatching.pCondHit[cell]
			inside := m * qVal
			aScout := float64(scoutOutsideHits) / float64(scoutSamples)
			pDesign := aScout + inside

			varPlainPred := pDesign * (1.0 - pDesign) / float64(plainSamples)
			varHybridPred := aScout*(1.0-aScout)/float64(plainSamples) + m*m*qVal*(1.0-qVal)/float64(nQ)

			if varHybridPred <= varPlainPred {
				frozenUseHybrid[cell] = true
			}
		}
	}

	for _, ps := range prodStreams {
		maxR := 0
		for _, cell := range ps.prop.TargetCells {
			if cell[1] > maxR {
				maxR = cell[1]
			}
		}
		diag.AdmittedStrata = append(diag.AdmittedStrata, AdaptiveStratumAdmitted{
			ID:             ps.prop.ID,
			TeamID:         ps.prop.Team,
			MaxRank:        maxR,
			Threshold:      0,
			Mass:           ps.prop.Mass,
			ValidationHits: ps.valHits,
			Samples:        ps.samples,
			Work:           ps.work,
		})
	}

	log.Printf("rare-position-adaptive-freeze: scout_work=%d discovery_work=%d validation_work=%d production_work=%d plain_production_work=%d conditional_production_work=%d plain_fraction=%.4f",
		diag.ScoutWork, diag.DiscoveryWork, diag.ValidationWork, diag.ProductionWork,
		int64(plainSamples)*plainCost, totalStratumWork, float64(int64(plainSamples)*plainCost)/float64(diag.ProductionWork))

	// 7 & 8. Fresh production simulation and raw cell estimates
	strataByTeam := make(map[int]*PointStratum, len(prodStreams))
	for _, ps := range prodStreams {
		strataByTeam[ps.prop.Team] = ps.prop.Stratum
	}

	plainCounts, plainOutsideCounts := simulateAdaptivePlainSeasons(base, group.Games, table, order, group.Team_groups,
		strataByTeam, plainSamples, deriveRarePositionSeed(masterSeed, "adaptive-prod-P"))

	// Map targetTeamID -> admittedProdStream
	streamByTeam := make(map[int]admittedProdStream, len(prodStreams))
	condCountsByTeam := make(map[int]map[[2]int]int, len(prodStreams))

	for _, ps := range prodStreams {
		streamByTeam[ps.prop.Team] = ps
		qSeed := deriveRarePositionSeed(masterSeed, fmt.Sprintf("adaptive-prod-Q-team%d", ps.prop.Team))
		qCounts, _ := simulatePointHybridSeasons(base, group.Games, table, order, group.Team_groups,
			ps.prop.Stratum, true, ps.samples, qSeed)
		condCountsByTeam[ps.prop.Team] = qCounts
	}

	estimates := make(map[int]map[int]ProductionEstimate, numTeams)
	rawProbs := make(map[int]map[int]float64, numTeams)

	for _, team := range group.Team_groups {
		id := team.Team_id
		estimates[id] = make(map[int]ProductionEstimate, numTeams)
		rawProbs[id] = make(map[int]float64, numTeams)

		ps, hasStratum := streamByTeam[id]

		for pos := 0; pos < numTeams; pos++ {
			cell := [2]int{id, pos}
			plainHits := plainCounts[cell]
			pPlain := float64(plainHits) / float64(plainSamples)

			p := pPlain
			varCell := pPlain * (1 - pPlain) / float64(plainSamples)
			hits := plainHits
			samples := plainSamples
			designName := "plain_mc"

			if hasStratum && frozenUseHybrid[cell] {
				designName = "adaptive_point_stratum"
				outsideHits := plainOutsideCounts[cell]
				pOutside := float64(outsideHits) / float64(plainSamples)

				qCounts := condCountsByTeam[id]
				insideHits := qCounts[cell]
				pInside := float64(insideHits) / float64(ps.samples)

				stratumMass := ps.prop.Stratum.Mass
				pHybrid := pOutside + stratumMass*pInside
				varHybrid := pOutside*(1-pOutside)/float64(plainSamples) +
					stratumMass*stratumMass*pInside*(1-pInside)/float64(ps.samples)

				p = pHybrid
				varCell = varHybrid
				hits = outsideHits + insideHits
				samples = plainSamples + ps.samples
			}

			est := pointHybridEstimate(p, varCell, hits, samples, diag.ProductionWork, designName)

			if hasStratum && frozenUseHybrid[cell] && hits == 0 {
				stratumMass := ps.prop.Stratum.Mass
				est.ZeroHitUpper95 = math.Min(1, -math.Log(0.025)/float64(plainSamples)+stratumMass*(-math.Log(0.025))/float64(ps.samples))
			}

			estimates[id][pos] = est
			rawProbs[id][pos] = est.Probability
		}
	}

	// 9. Reporting-only reconciled matrix (deferred / optional)
	if os.Getenv("RARE_POSITION_ENABLE_RECONCILIATION") == "1" {
		teamIDs := teamIDsFromGroups(group.Team_groups)
		diag.ReconciledMatrix = reconcileProbabilityMatrix(rawProbs, teamIDs)
	}

	log.Printf("rare-position-adaptive-strata-summary: group=%d scout_work=%d discovery_work=%d validation_work=%d production_work=%d total_work=%d admitted_strata=%d",
		group.Id, diag.ScoutWork, diag.DiscoveryWork, diag.ValidationWork, diag.ProductionWork,
		diag.ScoutWork+diag.DiscoveryWork+diag.ValidationWork+diag.ProductionWork, len(diag.AdmittedStrata))

	return estimates, diag, true
}

// reconcileProbabilityMatrix applies Sinkhorn-Knopp doubly-stochastic matrix
// normalization to produce a separately labeled reconciled N x N matrix.
func reconcileProbabilityMatrix(raw map[int]map[int]float64, teams []int) map[int]map[int]float64 {
	n := len(teams)
	reconciled := make(map[int]map[int]float64, n)
	for _, t := range teams {
		reconciled[t] = make(map[int]float64, n)
		for r := 0; r < n; r++ {
			reconciled[t][r] = raw[t][r]
		}
	}

	for iter := 0; iter < 100; iter++ {
		for _, t := range teams {
			rowSum := 0.0
			for r := 0; r < n; r++ {
				rowSum += reconciled[t][r]
			}
			if rowSum > 0 {
				for r := 0; r < n; r++ {
					reconciled[t][r] /= rowSum
				}
			}
		}

		for r := 0; r < n; r++ {
			colSum := 0.0
			for _, t := range teams {
				colSum += reconciled[t][r]
			}
			if colSum > 0 {
				for _, t := range teams {
					reconciled[t][r] /= colSum
				}
			}
		}
	}

	return reconciled
}
