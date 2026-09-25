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
			total = 1.0
			weights[0] = 1.0
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

type AdaptiveCellAnalysis struct {
	Team                int     `json:"team"`
	Position            int     `json:"position"`
	ScoutHits           int     `json:"scout_hits"`
	ScoutProbability    float64 `json:"scout_probability"`
	ProvenImpossible    bool    `json:"proven_impossible"`
	PossiblePointTotals []int   `json:"possible_point_totals"`
	HardUpperBound      float64 `json:"hard_upper_bound"`
	PriorityEstimate    float64 `json:"priority_estimate"`
	Decision            string  `json:"decision"`
}

type AdaptivePointProposal struct {
	ID                string        `json:"id"`
	Team              int           `json:"team"`
	AllowedPoints     []int         `json:"allowed_points"`
	Mass              float64       `json:"mass"`
	TargetCells       [][2]int      `json:"target_cells"`
	ValidationSamples int           `json:"validation_samples"`
	PredictedUtility  float64       `json:"predicted_utility"`
	Stratum           *PointStratum `json:"-"`
}

type AdaptiveStratumAdmitted struct {
	TeamID         int     `json:"team_id"`
	MaxRank        int     `json:"max_rank"`
	Threshold      int     `json:"threshold"`
	Mass           float64 `json:"mass"`
	ValidationHits int     `json:"validation_hits"`
	Samples        int     `json:"samples"`
	Work           int64   `json:"work"`
}

type AdaptiveStratumDiagnostics struct {
	ScoutWork        int64                           `json:"scout_work"`
	DiscoveryWork    int64                           `json:"discovery_work"`
	ValidationWork   int64                           `json:"validation_work"`
	ProductionWork   int64                           `json:"production_work"`
	TotalWork        int64                           `json:"total_work"`
	TotalWorkLimit   int64                           `json:"total_work_limit"`
	PlainSamples     int                             `json:"plain_samples"`
	Feasibility      map[[2]int]string               `json:"feasibility,omitempty"`
	UpperBounds      map[[2]int]float64              `json:"upper_bounds,omitempty"`
	CellAnalysis     map[[2]int]AdaptiveCellAnalysis `json:"cell_analysis,omitempty"`
	AdmittedStrata   []AdaptiveStratumAdmitted       `json:"admitted_strata,omitempty"`
	ReconciledMatrix map[int]map[int]float64         `json:"reconciled_matrix,omitempty"`
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

// computePriorityEstimate calculates a heuristic priority score for cell (team, rank)
// using exact PMF and smoothed scout rank distribution P(rank | S = s).
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
		totalObsS := 0
		weightedHits := 0.0

		for ds := -3; ds <= 3; ds++ {
			sNeighbor := s + ds
			wPoint := math.Exp(-math.Abs(float64(ds)) / 3.0)
			obsCount := scout.PointCounts[sNeighbor]
			if obsCount > 0 {
				totalObsS += obsCount
				for rOther := 0; rOther < numPositions; rOther++ {
					hits := scout.PointRankCounts[sNeighbor][rOther]
					if hits > 0 {
						wRank := math.Exp(-math.Abs(float64(rOther-rank)) / 1.5)
						weightedHits += float64(hits) * wPoint * wRank
					}
				}
			}
		}

		pRankGivenS := 0.0
		if totalObsS > 0 {
			pRankGivenS = weightedHits / float64(totalObsS)
		} else {
			pRankGivenS = 1.0 / float64(numPositions)
		}

		score += probS * pRankGivenS
	}
	return score
}

// rankNotRuledOutAtAddedPoints returns false if exact rank targetRank is
// impossible for targetTeam when it earns exactly addedPoints additional points.
func rankNotRuledOutAtAddedPoints(
	targetTeamID int, targetRank int, addedPoints int,
	campaign []*TeamCampaign, teamGroups []TeamType, games []*GameType, table *Table,
) bool {
	numTeams := len(teamGroups)
	targetIdx := table.Query(uint32(targetTeamID))
	if targetIdx < 0 || int(targetIdx) >= len(campaign) || campaign[targetIdx] == nil {
		return false
	}
	targetCampaign := campaign[targetIdx]
	targetFinalPoints := targetCampaign.points + addedPoints

	unplayedPerTeam := make(map[int]int, numTeams)
	for _, g := range games {
		if !g.Played {
			unplayedPerTeam[g.HomeId]++
			unplayedPerTeam[g.AwayId]++
		}
	}

	minPoints := make(map[int]int, numTeams)
	maxPoints := make(map[int]int, numTeams)

	for _, tg := range teamGroups {
		id := tg.Team_id
		c := campaign[table.Query(uint32(id))]
		if c == nil {
			continue
		}
		if id == targetTeamID {
			minPoints[id] = targetFinalPoints
			maxPoints[id] = targetFinalPoints
			continue
		}

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

		minPoints[id] = c.points + unplayed*minGain
		maxPoints[id] = c.points + unplayed*maxGain
	}

	strictlyBetter := 0
	strictlyWorse := 0

	for _, tg := range teamGroups {
		id := tg.Team_id
		if id == targetTeamID {
			continue
		}
		if minPoints[id] > targetFinalPoints {
			strictlyBetter++
		}
		if maxPoints[id] < targetFinalPoints {
			strictlyWorse++
		}
	}

	bestPossibleRank := strictlyBetter
	worstPossibleRank := (numTeams - 1) - strictlyWorse

	return targetRank >= bestPossibleRank && targetRank <= worstPossibleRank
}

// computeHardCellUpperBound calculates HardUpperBound(team, rank) as
// sum_{s in PossiblePointTotals} P(S = s).
func computeHardCellUpperBound(
	targetTeamID int, targetRank int,
	pmf map[int]float64,
	campaign []*TeamCampaign, teamGroups []TeamType, games []*GameType, table *Table,
) (hardBound float64, possibleTotals []int, provenImpossible bool) {
	possibleTotals = make([]int, 0, len(pmf))
	hardBound = 0.0

	for s, p := range pmf {
		if p <= 0 {
			continue
		}
		if rankNotRuledOutAtAddedPoints(targetTeamID, targetRank, s, campaign, teamGroups, games, table) {
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
	if rng == nil {
		rng = rand.New(rand.NewSource(1))
	}
	seed := rng.Int63()
	data := runPlainMCScout(baseCampaign, games, table, sortOrder, teamGroups, scoutSamples, plainWorkPerSample, rand.New(rand.NewSource(seed)))
	data.TeamScout = make(map[int]*TeamPointRankScout, len(teamGroups))

	numPositions := len(teamGroups)
	for _, team := range teamGroups {
		id := team.Team_id
		data.TeamScout[id] = &TeamPointRankScout{
			Samples:         scoutSamples,
			RankCounts:      append([]int(nil), data.TeamCounts[id]...),
			PointRankCounts: make(map[int][]int),
			PointCounts:     make(map[int]int),
		}
	}

	simCampaign := make([]*TeamCampaign, len(baseCampaign))
	teamSlice := make([]*TeamCampaign, len(teamGroups))

	rngJoint := rand.New(rand.NewSource(deriveRarePositionSeed(seed, "joint-scout")))

	for s := 0; s < scoutSamples; s++ {
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
			hs := poissonRand(rngJoint, game.HomePower)
			as := poissonRand(rngJoint, game.AwayPower)
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
		sort.Sort(TeamCampaignSorted{t: teamSlice[:idx], sort: sortOrder, rng: rngJoint})

		for rank, c := range teamSlice[:idx] {
			teamID := c.id
			baseC := baseCampaign[table.Query(uint32(teamID))]
			added := c.points - baseC.points

			ts := data.TeamScout[teamID]
			if ts.PointRankCounts[added] == nil {
				ts.PointRankCounts[added] = make([]int, numPositions)
			}
			ts.PointRankCounts[added][rank]++
			ts.PointCounts[added]++
		}
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

// buildAdaptivePointProposal constructs a cell-driven AdaptivePointProposal
// for a team with unresolved target cells.
func buildAdaptivePointProposal(
	teamID int,
	targetCells [][2]int,
	universe *PointStratumUniverse,
	pmf map[int]float64,
	cellAnalysis map[[2]int]AdaptiveCellAnalysis,
) (AdaptivePointProposal, bool) {
	if universe == nil || len(targetCells) == 0 {
		return AdaptivePointProposal{}, false
	}

	pointSetMap := make(map[int]bool)
	maxPriority := 0.0

	for _, cell := range targetCells {
		analysis := cellAnalysis[cell]
		for _, pts := range analysis.PossiblePointTotals {
			pointSetMap[pts] = true
		}
		if analysis.PriorityEstimate > maxPriority {
			maxPriority = analysis.PriorityEstimate
		}
	}

	allowedPoints := make([]int, 0, len(pointSetMap))
	for pts := range pointSetMap {
		allowedPoints = append(allowedPoints, pts)
	}
	sort.Ints(allowedPoints)

	if len(allowedPoints) == 0 {
		return AdaptivePointProposal{}, false
	}

	var stratum *PointStratum
	var ok bool

	isTail := true
	minPts := allowedPoints[0]
	maxPts := allowedPoints[len(allowedPoints)-1]

	for pts := minPts; pts <= maxPts; pts++ {
		if pmf[pts] > 0 && !pointSetMap[pts] {
			isTail = false
			break
		}
	}

	if isTail {
		stratum, ok = makePointTailStratum(universe, minPts)
	} else {
		stratum, ok = makePointSetStratum(universe, allowedPoints)
	}

	if !ok || stratum == nil || stratum.Mass <= 0 {
		return AdaptivePointProposal{}, false
	}

	propID := fmt.Sprintf("team%d_point_set", teamID)
	return AdaptivePointProposal{
		ID:               propID,
		Team:             teamID,
		AllowedPoints:    allowedPoints,
		Mass:             stratum.Mass,
		TargetCells:      targetCells,
		PredictedUtility: maxPriority,
		Stratum:          stratum,
	}, true
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
	scoutRequested := 15000
	if raw := os.Getenv("RARE_POSITION_DIVERSIFIED_SCOUT_SAMPLES"); raw != "" {
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

	// 2 & 3 & 4. Points/rank analysis, exact PMF, priority estimates, and hard upper bounds
	candidateProposals := make([]AdaptivePointProposal, 0, numTeams)
	discoveryWorkTotal := int64(0)

	for _, team := range group.Team_groups {
		teamID := team.Team_id
		universe, ok := pointOutcomeUniverse(teamID, base, table, group.Games, 39)
		if !ok || universe == nil {
			continue
		}
		discoveryWorkTotal += int64(len(universe.GameIndices) * 1000)

		pmf := additionalPointsPMF(universe)
		teamScout := scout.TeamScout[teamID]

		unresolvedTargetCells := make([][2]int, 0, numTeams)

		for pos := 0; pos < numTeams; pos++ {
			cell := [2]int{teamID, pos}

			hardUB, possibleTotals, provenImp := computeHardCellUpperBound(teamID, pos, pmf, base, group.Team_groups, group.Games, table)
			scoutHits := scout.TeamCounts[teamID][pos]
			scoutProb := float64(scoutHits) / float64(scoutSamples)
			priorityEst := computePriorityEstimate(teamID, pos, pmf, teamScout, numTeams)

			decision := "candidate"
			if provenImp {
				decision = "proven_impossible"
			} else if scoutHits > 0 {
				decision = "observed"
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
				Decision:            decision,
			}
			diag.CellAnalysis[cell] = analysis
			diag.UpperBounds[cell] = hardUB

			log.Printf("rare-position-adaptive-cell: team=%d rank=%d scout_hits=%d hard_upper_bound=%.6g priority=%.6g possible_points=%v decision=%s",
				teamID, pos, scoutHits, hardUB, priorityEst, possibleTotals, decision)

			if !provenImp && scoutHits == 0 && hardUB >= simulateThreshold {
				unresolvedTargetCells = append(unresolvedTargetCells, cell)
			}
		}

		if len(unresolvedTargetCells) > 0 {
			prop, ok := buildAdaptivePointProposal(teamID, unresolvedTargetCells, universe, pmf, diag.CellAnalysis)
			if ok {
				candidateProposals = append(candidateProposals, prop)
			}
		}
	}

	sort.Slice(candidateProposals, func(i, j int) bool {
		return candidateProposals[i].PredictedUtility > candidateProposals[j].PredictedUtility
	})

	diag.DiscoveryWork = discoveryWorkTotal

	// 5. Validation probes on shortlisted candidate proposals (Design)
	valSamplesPerStratum := 200
	if raw := os.Getenv("RARE_POSITION_ADAPTIVE_VALIDATION_INITIAL_SAMPLES"); raw != "" {
		if parsed, err := strconv.Atoi(raw); err == nil && parsed > 0 {
			valSamplesPerStratum = parsed
		}
	}

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

	valWorkTotal := int64(len(candidateProposals) * valSamplesPerStratum) * stratumCost
	if valWorkTotal > valWorkCap {
		valWorkTotal = valWorkCap
	}

	diag.ValidationWork = valWorkTotal

	type validationResult struct {
		prop               AdaptivePointProposal
		targetHits         int
		validationSamples  int
		targetHitRate      float64
		pCondHit           map[[2]int]float64
		varCondPerSample   map[[2]int]float64
	}

	valResults := make([]validationResult, 0, len(candidateProposals))

	if valSamplesPerStratum > 0 && valWorkTotal > 0 {
		for _, prop := range candidateProposals {
			valSeed := deriveRarePositionSeed(masterSeed, fmt.Sprintf("adaptive-val-team%d", prop.Team))
			valCounts, _ := simulatePointHybridSeasons(base, group.Games, table, order, group.Team_groups,
				prop.Stratum, true, valSamplesPerStratum, valSeed)

			targetHits := 0
			pCondHit := make(map[[2]int]float64, len(prop.TargetCells))
			varCondPerSample := make(map[[2]int]float64, len(prop.TargetCells))

			for _, cell := range prop.TargetCells {
				hits := valCounts[cell]
				targetHits += hits
				pCond := float64(hits) / float64(valSamplesPerStratum)
				pCondHit[cell] = pCond
				varCondPerSample[cell] = prop.Stratum.Mass * prop.Stratum.Mass * pCond * (1.0 - pCond)
			}

			targetHitRate := float64(targetHits) / float64(valSamplesPerStratum*len(prop.TargetCells))

			log.Printf("rare-position-adaptive-validation: proposal=%s samples=%d work=%d target_hits=%d decision=%s",
				prop.ID, valSamplesPerStratum, int64(valSamplesPerStratum)*stratumCost, targetHits,
				map[bool]string{true: "admitted", false: "rejected"}[targetHits >= 2])

			if targetHits >= 2 {
				valResults = append(valResults, validationResult{
					prop:               prop,
					targetHits:         targetHits,
					validationSamples:  valSamplesPerStratum,
					targetHitRate:      targetHitRate,
					pCondHit:           pCondHit,
					varCondPerSample:   varCondPerSample,
				})
			}
		}
	}

	// 6. Freeze work allocation using greedy ESS utility scheduler and 80% plain production floor
	remainingWork := workLimit - diag.ScoutWork - diag.DiscoveryWork - diag.ValidationWork
	if remainingWork <= 0 {
		return nil, diag, false
	}

	minPlainFraction := 0.80
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

	// Utility-driven production scheduler: allocate conditional budget in chunks
	if len(valResults) > 0 && maxCondWork >= stratumCost {
		chunkWork := int64(100000)
		if chunkWork < stratumCost {
			chunkWork = stratumCost
		}

		allocatedSamples := make([]int, len(valResults))
		allocatedWork := make([]int64, len(valResults))

		initialPlainSamplesEstimate := int(remainingWork / plainCost)
		currentCellESS := make(map[[2]int]float64, numTeams*numTeams)

		for _, team := range group.Team_groups {
			for pos := 0; pos < numTeams; pos++ {
				cell := [2]int{team.Team_id, pos}
				scoutHits := scout.TeamCounts[team.Team_id][pos]
				if scoutHits > 0 {
					p := float64(scoutHits) / float64(scoutSamples)
					currentCellESS[cell] = p * float64(initialPlainSamplesEstimate)
				}
			}
		}

		remainingCondWork := maxCondWork

		for remainingCondWork >= chunkWork {
			bestValIdx := -1
			bestUtilityGain := 0.0

			chunkSamples := int(chunkWork / stratumCost)
			if chunkSamples <= 0 {
				break
			}

			for idx, vr := range valResults {
				utilityGain := 0.0

				for _, cell := range vr.prop.TargetCells {
					vCond := vr.varCondPerSample[cell]
					currentESS := currentCellESS[cell]

					deltaESS := 0.0
					if vCond > 0 {
						deltaESS = (vr.prop.Mass * vr.prop.Mass) * float64(chunkSamples) / vCond
					} else if vr.targetHits > 0 {
						deltaESS = float64(chunkSamples)
					}

					newESS := currentESS + deltaESS
					gain := adaptiveESSUtility(newESS) - adaptiveESSUtility(currentESS)
					utilityGain += gain
				}

				if utilityGain > bestUtilityGain {
					bestUtilityGain = utilityGain
					bestValIdx = idx
				}
			}

			if bestValIdx < 0 || bestUtilityGain <= 0 {
				break
			}

			allocatedWork[bestValIdx] += chunkWork
			allocatedSamples[bestValIdx] += chunkSamples
			remainingCondWork -= chunkWork
			totalStratumWork += chunkWork

			bestVR := valResults[bestValIdx]
			for _, cell := range bestVR.prop.TargetCells {
				vCond := bestVR.varCondPerSample[cell]
				deltaESS := 0.0
				if vCond > 0 {
					deltaESS = (bestVR.prop.Mass * bestVR.prop.Mass) * float64(chunkSamples) / vCond
				} else if bestVR.targetHits > 0 {
					deltaESS = float64(chunkSamples)
				}
				currentCellESS[cell] += deltaESS
			}
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

	if diag.ScoutWork+diag.DiscoveryWork+diag.ValidationWork+diag.ProductionWork > workLimit {
		panic("adaptive stratum work budget exceeded")
	}

	for _, ps := range prodStreams {
		maxR := 0
		for _, cell := range ps.prop.TargetCells {
			if cell[1] > maxR {
				maxR = cell[1]
			}
		}
		diag.AdmittedStrata = append(diag.AdmittedStrata, AdaptiveStratumAdmitted{
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

		targetSet := make(map[int]bool)
		if hasStratum {
			for _, cell := range ps.prop.TargetCells {
				targetSet[cell[1]] = true
			}
		}

		for pos := 0; pos < numTeams; pos++ {
			cell := [2]int{id, pos}
			plainHits := plainCounts[cell]
			pPlain := float64(plainHits) / float64(plainSamples)

			p := pPlain
			varCell := pPlain * (1 - pPlain) / float64(plainSamples)
			hits := plainHits
			samples := plainSamples
			designName := "plain_mc"

			if hasStratum && targetSet[pos] {
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

				// Frozen cell-level selection: use hybrid if predicted variance is better
				if varHybrid <= varCell || pPlain == 0 {
					p = pHybrid
					varCell = varHybrid
					hits = outsideHits + insideHits
					samples = plainSamples + ps.samples
				} else {
					designName = "plain_mc"
				}
			}

			est := pointHybridEstimate(p, varCell, hits, samples, diag.ProductionWork, designName)

			if hasStratum && targetSet[pos] && hits == 0 {
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
