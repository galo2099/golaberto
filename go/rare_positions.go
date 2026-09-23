package main

import (
	"fmt"
	"log"
	"math"
	"math/rand"
	"os"
	"sort"
	"time"
)

const MinInterestingProbability = 1e-5

type PositionSearchStatus int

const (
	StatusUnexplored PositionSearchStatus = iota
	StatusObserved
	StatusFrontier
	StatusPromising
	StatusResolved
	StatusExhausted
	StatusProvenImpossible
	StatusBelowInterest
)

func (s PositionSearchStatus) String() string {
	switch s {
	case StatusUnexplored:
		return "Unexplored"
	case StatusObserved:
		return "Observed"
	case StatusFrontier:
		return "Frontier"
	case StatusPromising:
		return "Promising"
	case StatusResolved:
		return "Resolved"
	case StatusExhausted:
		return "Exhausted"
	case StatusProvenImpossible:
		return "ProvenImpossible"
	case StatusBelowInterest:
		return "BelowInterest"
	default:
		return fmt.Sprintf("Unknown(%d)", s)
	}
}

type SearchProposal struct {
	Name          string
	Direction     RareDirection
	StrengthLevel int
	TargetRank    int
	Components    []ProposalComponent
}

type ProductionEstimate struct {
	Probability float64
	StdErr      float64
	Samples     int
	Hits        int
	ESS         float64
	MeanWeight  float64
	Found       bool
	WorkSpent   int64
}

type PositionSearchState struct {
	Position            int
	Status              PositionSearchStatus
	Feasible            bool
	ObservedCount       int
	NormalProb          float64
	BestProposal        *SearchProposal
	BestCandidateHits   int
	BestOvershootHits   int
	BestScore           float64
	BestPilotRate       float64
	BestPilotSamples    int
	Difficult           bool
	ProductionEstimate  *ProductionEstimate
	SearchWorkSpent     int64
	ProductionWorkSpent int64
}

type TeamRareSearch struct {
	TeamID              int
	Positions           []*PositionSearchState
	NormalMeanRank      float64
	Has100PercentNormal bool
}

type FrontierCandidate struct {
	TeamID      int
	Position    int
	Direction   RareDirection
	Priority    int
	SearchState *PositionSearchState
}

type SearchResult struct {
	Proposal      SearchProposal
	Samples       int
	WorkSpent     int64
	CandidateHits int
	OvershootHits int
	RankHistogram []int
	CandidateRate float64
	OvershootRate float64
	Score         float64
}

func rarePositionSamplingEnabled() bool {
	return os.Getenv("RARE_POSITION_IMPORTANCE_SAMPLING") == "1"
}

func estimateSeasonWork(unplayedGames int, componentCount int, teamCount int) int64 {
	if unplayedGames <= 0 {
		return int64(teamCount + 1)
	}
	if componentCount <= 0 {
		componentCount = 1
	}
	return int64(unplayedGames*componentCount + teamCount)
}

func calculateMaxRareWork(unplayedGames int, teamCount int) int64 {
	return 100000 * estimateSeasonWork(unplayedGames, 1, teamCount)
}

func initializeTeamRareSearch(
	teamID int,
	normalCounts []int,
	normalProbs []float64,
	campaign []*TeamCampaign,
	teamGroups []TeamType,
	games []*GameType,
	table *Table,
	sortOrder []SortType,
) *TeamRareSearch {
	numPositions := len(normalCounts)
	positions := make([]*PositionSearchState, numPositions)

	normalMeanRank := 0.0
	has100Percent := false

	for pos := 0; pos < numPositions; pos++ {
		prob := normalProbs[pos]
		count := normalCounts[pos]

		normalMeanRank += float64(pos) * prob
		if prob >= 1.0-1e-9 {
			has100Percent = true
		}

		state := &PositionSearchState{
			Position:      pos,
			ObservedCount: count,
			NormalProb:    prob,
		}

		if count > 0 {
			state.Status = StatusObserved
			state.Feasible = true
		} else {
			feasible := possiblePositionByPointsBounds(
				teamID, pos, campaign, teamGroups, games, table, sortOrder,
			)
			state.Feasible = feasible
			if feasible {
				state.Status = StatusUnexplored
			} else {
				state.Status = StatusProvenImpossible
			}
		}

		positions[pos] = state
	}

	return &TeamRareSearch{
		TeamID:              teamID,
		Positions:           positions,
		NormalMeanRank:      normalMeanRank,
		Has100PercentNormal: has100Percent,
	}
}

func discoverFrontier(teamSearch *TeamRareSearch) []*FrontierCandidate {
	numPositions := len(teamSearch.Positions)

	knownSet := make(map[int]bool)
	minKnown := numPositions
	maxKnown := -1

	for pos, st := range teamSearch.Positions {
		if st.Status == StatusObserved || st.Status == StatusPromising || st.Status == StatusResolved {
			knownSet[pos] = true
			if pos < minKnown {
				minKnown = pos
			}
			if pos > maxKnown {
				maxKnown = pos
			}
		}
	}

	if len(knownSet) == 0 {
		return nil
	}

	var candidates []*FrontierCandidate

	for pos, st := range teamSearch.Positions {
		if st.Status != StatusUnexplored || !st.Feasible {
			continue
		}

		isAdjacent := (pos > 0 && knownSet[pos-1]) || (pos < numPositions-1 && knownSet[pos+1])
		isGap := pos > minKnown && pos < maxKnown

		if isAdjacent || isGap {
			st.Status = StatusFrontier

			dir := RareBetter
			if float64(pos) > teamSearch.NormalMeanRank {
				dir = RareWorse
			}

			priority := 10
			if teamSearch.Has100PercentNormal {
				priority += 100
			}
			if isGap {
				priority += 50
			}
			if isAdjacent {
				priority += 30
			}

			candidates = append(candidates, &FrontierCandidate{
				TeamID:      teamSearch.TeamID,
				Position:    pos,
				Direction:   dir,
				Priority:    priority,
				SearchState: st,
			})
		}
	}

	sort.Slice(candidates, func(i, j int) bool {
		return candidates[i].Priority > candidates[j].Priority
	})

	return candidates
}

func buildSearchProposalForLevel(
	games []*GameType,
	targetTeamID int,
	direction RareDirection,
	targetRank int,
	level int,
	normalMeanRanks map[int]float64,
	originalMeans []GameProposalMeans,
) SearchProposal {
	strengths := getStrengthDefinitions(direction)
	sMap := make(map[int]StrengthDefinition, len(strengths))
	for _, s := range strengths {
		sMap[s.Level] = s
	}

	bestDef := sMap[level]
	bestCfg := buildSingleProposalConfig(games, targetTeamID, direction, targetRank, bestDef, normalMeanRanks)

	comps := []ProposalComponent{
		{
			Name:          "original",
			Weight:        0.05,
			Means:         originalMeans,
			RelevantTeams: nil,
			TargetRank:    -1,
		},
	}
	levels := []int{level - 1, level, level + 1}
	weights := []float64{0.20, 0.50, 0.25}
	if level == 1 {
		levels = []int{1, 2, 3}
		weights = []float64{0.50, 0.25, 0.20}
	} else if level == 5 {
		levels = []int{3, 4, 5}
		weights = []float64{0.20, 0.25, 0.50}
	}
	for i, lvl := range levels {
		cfg := bestCfg
		if lvl != level {
			cfg = buildSingleProposalConfig(games, targetTeamID, direction, targetRank, sMap[lvl], normalMeanRanks)
		}
		comps = append(comps, ProposalComponent{
			Name:          cfg.Name,
			Weight:        weights[i],
			Means:         cfg.Means,
			RelevantTeams: cfg.RelevantTeams,
			TargetRank:    targetRank,
		})
	}

	validateProposalMixture(comps, len(games))

	return SearchProposal{
		Name:          fmt.Sprintf("%s_lvl%d_rank%d", bestDef.Name, level, targetRank),
		Direction:     direction,
		StrengthLevel: level,
		TargetRank:    targetRank,
		Components:    comps,
	}
}

func evaluateFrontierCandidate(
	candidate *FrontierCandidate,
	campaign []*TeamCampaign,
	games []*GameType,
	table *Table,
	sortOrder []SortType,
	teamGroups []TeamType,
	originalMeans []GameProposalMeans,
	normalMeanRanks map[int]float64,
	rng *rand.Rand,
	groupID int,
	unplayedGames int,
	samplesPerLevel int,
) int64 {
	st := candidate.SearchState
	targetTeamID := candidate.TeamID
	pos := candidate.Position
	direction := candidate.Direction

	singleRunWork := estimateSeasonWork(unplayedGames, 1, len(teamGroups))

	strengths := getStrengthDefinitions(direction)
	var testedConfigs []ProposalConfig

	for _, sDef := range strengths {
		if sDef.Level == 0 {
			continue
		}
		cfg := buildSingleProposalConfig(games, targetTeamID, direction, pos, sDef, normalMeanRanks)
		testedConfigs = append(testedConfigs, cfg)
	}

	pilotResults := runRareProposalPilot(
		campaign, games, table, sortOrder, teamGroups,
		targetTeamID, direction, []int{pos}, testedConfigs,
		samplesPerLevel, rng, groupID,
	)

	workSpent := int64(len(testedConfigs)*samplesPerLevel) * singleRunWork
	st.SearchWorkSpent += workSpent

	bestIdx := selectWeakestUsefulPilot(pilotResults)
	for i, res := range pilotResults {
		reason := "not_selected"
		if i == bestIdx {
			if res.CandidateRate >= MinUsefulPilotRate {
				reason = "first_useful_rate"
			} else {
				reason = "best_below_threshold"
			}
		} else if bestIdx < 0 {
			reason = "no_hits"
		}
		log.Printf("rare-position-pilot: group=%d team=%d pos=%d level=%d samples=%d hits=%d rate=%.4f selected=%t reason=%s",
			groupID, targetTeamID, pos, res.Config.StrengthLevel, res.Samples, res.CandidateHits,
			res.CandidateRate, i == bestIdx, reason)
	}

	if bestIdx >= 0 {
		bestRes := pilotResults[bestIdx]
		bestLevel := bestRes.Config.StrengthLevel

		proposal := buildSearchProposalForLevel(
			games, targetTeamID, direction, pos, bestLevel, normalMeanRanks, originalMeans,
		)

		st.Status = StatusPromising
		st.BestProposal = &proposal
		st.BestCandidateHits = bestRes.CandidateHits
		st.BestOvershootHits = bestRes.OvershootHits
		st.BestScore = bestRes.Score
		st.BestPilotRate = bestRes.CandidateRate
		st.BestPilotSamples = bestRes.Samples
		st.Difficult = bestRes.CandidateRate < MinUsefulPilotRate
	} else {
		st.Status = StatusExhausted
	}

	return workSpent
}

const (
	MinUsefulPilotRate   = 0.005
	MaxUsefulPilotRate   = 0.03
	PilotSamplesPerLevel = 200
)

// The first level that produces enough target events keeps the proposal close
// to the original distribution. If none does, retain the most productive
// nonzero pilot, breaking ties in favor of the weaker level.
func selectWeakestUsefulPilot(results []PilotProposalResult) int {
	bestBelow := -1
	for i, result := range results {
		if result.CandidateHits == 0 {
			continue
		}
		if result.CandidateRate >= MinUsefulPilotRate {
			return i
		}
		if bestBelow < 0 || result.CandidateRate > results[bestBelow].CandidateRate {
			bestBelow = i
		}
	}
	return bestBelow
}

func poissonRand(rng *rand.Rand, mean float64) int {
	if mean <= 0 {
		return 0
	}
	var em int
	t := 0.0
	for {
		t += rng.ExpFloat64()
		if t >= mean {
			break
		}
		em++
	}
	return em
}

func logPoissonQOverP(score int, originalMean, proposalMean float64) float64 {
	if originalMean < 0 || proposalMean < 0 {
		return math.NaN()
	}

	if originalMean == 0 {
		if score == 0 {
			return -proposalMean
		}
		return math.Inf(1) // Q/P = +Inf => P/Q = 0 => weight = 0
	}

	if proposalMean == 0 {
		if score == 0 {
			return originalMean
		}
		return math.Inf(-1)
	}

	return originalMean - proposalMean + float64(score)*math.Log(proposalMean/originalMean)
}

func logAddExp(logA, logB float64) float64 {
	if math.IsInf(logA, -1) {
		return logB
	}
	if math.IsInf(logB, -1) {
		return logA
	}
	if logA > logB {
		return logA + math.Log1p(math.Exp(logB-logA))
	}
	return logB + math.Log1p(math.Exp(logA-logB))
}

func mixtureImportanceWeightMulti(logQOverP []float64, weights []float64) float64 {
	for i, logR := range logQOverP {
		if weights[i] > 0 && math.IsInf(logR, 1) {
			return 0.0
		}
	}

	logDenom := math.Inf(-1)
	for i, logR := range logQOverP {
		if weights[i] <= 0 {
			continue
		}
		if math.IsInf(logR, -1) {
			continue
		}
		term := math.Log(weights[i]) + logR
		logDenom = logAddExp(logDenom, term)
	}

	if math.IsInf(logDenom, -1) {
		return 0.0
	}
	return math.Exp(-logDenom)
}

func mixtureImportanceWeight(logQOverP float64, originalMixtureWeight float64) float64 {
	return mixtureImportanceWeightMulti([]float64{0.0, logQOverP}, []float64{originalMixtureWeight, 1.0 - originalMixtureWeight})
}

func conservativePositionBounds(
	targetTeamID int,
	campaign []*TeamCampaign,
	teamGroups []TeamType,
	games []*GameType,
	table *Table,
) (bestRank int, worstRank int, hasUnplayed bool) {
	numTeams := len(teamGroups)

	unplayedPerTeam := make(map[int]int)
	unplayedCount := 0
	for _, g := range games {
		if !g.Played {
			unplayedCount++
			unplayedPerTeam[g.HomeId]++
			unplayedPerTeam[g.AwayId]++
		}
	}

	if unplayedCount == 0 {
		return 0, 0, false
	}

	minPoints := make(map[int]int)
	maxPoints := make(map[int]int)

	for _, tg := range teamGroups {
		id := tg.Team_id
		c := campaign[table.Query(uint32(id))]
		if c == nil {
			continue
		}
		unplayed := unplayedPerTeam[id]
		pWin := c.points_win
		pLoss := c.points_loss
		pDraw := c.points_draw

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

	targetMax := maxPoints[targetTeamID]
	targetMin := minPoints[targetTeamID]

	strictlyBetter := 0
	strictlyWorse := 0

	for _, tg := range teamGroups {
		id := tg.Team_id
		if id == targetTeamID {
			continue
		}
		if minPoints[id] > targetMax {
			strictlyBetter++
		}
		if maxPoints[id] < targetMin {
			strictlyWorse++
		}
	}

	bestRank = strictlyBetter
	worstRank = (numTeams - 1) - strictlyWorse
	return bestRank, worstRank, true
}

func possiblePositionByPointsBounds(
	targetTeamID int,
	targetPosition int,
	campaign []*TeamCampaign,
	teamGroups []TeamType,
	games []*GameType,
	table *Table,
	sortOrder []SortType,
) bool {
	bestRank, worstRank, hasUnplayed := conservativePositionBounds(targetTeamID, campaign, teamGroups, games, table)

	if !hasUnplayed {
		teamSlice := make([]*TeamCampaign, 0, len(teamGroups))
		for _, tg := range teamGroups {
			c := campaign[table.Query(uint32(tg.Team_id))]
			if c != nil {
				teamSlice = append(teamSlice, c.clone())
			}
		}
		sortedTeams := TeamCampaignSorted{teamSlice, sortOrder}
		sort.Sort(sortedTeams)

		for pos, t := range sortedTeams.t {
			if t.id == targetTeamID {
				return pos == targetPosition
			}
		}
		return false
	}

	return targetPosition >= bestRank && targetPosition <= worstRank
}

type GameProposalMeans struct {
	Home float64
	Away float64
}

type RareDirection int

const (
	RareBetter RareDirection = iota
	RareWorse
)

const (
	MinProposalMean = 0.05
	MaxProposalMean = 8.0
)

func clampMean(m float64) float64 {
	if m < MinProposalMean {
		return MinProposalMean
	}
	if m > MaxProposalMean {
		return MaxProposalMean
	}
	return m
}

type ProposalConfig struct {
	Name              string
	StrengthLevel     int
	TargetRank        int
	TargetMultiplier  float64
	BlockerMultiplier float64
	RelevantTeams     []int
	Means             []GameProposalMeans
}

type ProposalComponent struct {
	Name          string
	Weight        float64
	Means         []GameProposalMeans
	RelevantTeams []int
	TargetRank    int
}

type StrengthDefinition struct {
	Level      int
	Name       string
	TargetMult float64
	BlockMult  float64
}

func getStrengthDefinitions(direction RareDirection) []StrengthDefinition {
	if direction == RareBetter {
		return []StrengthDefinition{
			{Level: 0, Name: "p0", TargetMult: 1.00, BlockMult: 1.00},
			{Level: 1, Name: "v_mild", TargetMult: 1.10, BlockMult: 0.93},
			{Level: 2, Name: "mild", TargetMult: 1.20, BlockMult: 0.87},
			{Level: 3, Name: "medium", TargetMult: 1.40, BlockMult: 0.78},
			{Level: 4, Name: "strong", TargetMult: 1.75, BlockMult: 0.65},
			{Level: 5, Name: "v_strong", TargetMult: 2.20, BlockMult: 0.50},
		}
	}
	return []StrengthDefinition{
		{Level: 0, Name: "p0", TargetMult: 1.00, BlockMult: 1.00},
		{Level: 1, Name: "v_mild", TargetMult: 0.93, BlockMult: 1.10},
		{Level: 2, Name: "mild", TargetMult: 0.87, BlockMult: 1.20},
		{Level: 3, Name: "medium", TargetMult: 0.78, BlockMult: 1.40},
		{Level: 4, Name: "strong", TargetMult: 0.65, BlockMult: 1.75},
		{Level: 5, Name: "v_strong", TargetMult: 0.50, BlockMult: 2.20},
	}
}

type PilotProposalResult struct {
	Config        ProposalConfig
	Samples       int
	RankHistogram []int
	CandidateHits int
	CandidateRate float64
	OvershootHits int
	OvershootRate float64
	Score         float64
}

func scorePilotProposal(candidateRate, overshootRate float64, strengthLevel int) float64 {
	return candidateRate - 0.5*overshootRate - 0.02*float64(strengthLevel)
}

func buildSingleProposalConfig(
	games []*GameType,
	targetTeamID int,
	direction RareDirection,
	depthRank int,
	sDef StrengthDefinition,
	normalMeanRanks map[int]float64,
) ProposalConfig {
	originalMeans := make([]GameProposalMeans, len(games))
	for i, g := range games {
		originalMeans[i] = GameProposalMeans{Home: g.HomePower, Away: g.AwayPower}
	}

	if sDef.Level == 0 {
		return ProposalConfig{
			Name:              "original",
			StrengthLevel:     0,
			TargetRank:        -1,
			TargetMultiplier:  1.0,
			BlockerMultiplier: 1.0,
			RelevantTeams:     nil,
			Means:             originalMeans,
		}
	}

	compBlockers := findRelevantCompetitorsForDepth(targetTeamID, normalMeanRanks, float64(depthRank), direction)
	blockerMap := make(map[int]bool, len(compBlockers))
	for _, id := range compBlockers {
		blockerMap[id] = true
	}

	compMeans := make([]GameProposalMeans, len(games))
	for i, g := range games {
		if g.Played {
			compMeans[i] = GameProposalMeans{Home: g.HomePower, Away: g.AwayPower}
			continue
		}

		hMult := 1.0
		aMult := 1.0

		if g.HomeId == targetTeamID {
			hMult = sDef.TargetMult
		} else if blockerMap[g.HomeId] {
			hMult = sDef.BlockMult
		}

		if g.AwayId == targetTeamID {
			aMult = sDef.TargetMult
		} else if blockerMap[g.AwayId] {
			aMult = sDef.BlockMult
		}

		hPower := g.HomePower
		aPower := g.AwayPower

		if hMult != 1.0 && hPower > 0 {
			hPower = clampMean(hPower * hMult)
		}
		if aMult != 1.0 && aPower > 0 {
			aPower = clampMean(aPower * aMult)
		}

		compMeans[i] = GameProposalMeans{
			Home: hPower,
			Away: aPower,
		}
	}

	return ProposalConfig{
		Name:              fmt.Sprintf("%s_lvl%d_rank%d", sDef.Name, sDef.Level, depthRank),
		StrengthLevel:     sDef.Level,
		TargetRank:        depthRank,
		TargetMultiplier:  sDef.TargetMult,
		BlockerMultiplier: sDef.BlockMult,
		RelevantTeams:     compBlockers,
		Means:             compMeans,
	}
}

func buildPilotProposalConfigs(
	games []*GameType,
	targetTeamID int,
	direction RareDirection,
	candidatePositions []int,
	normalMeanRanks map[int]float64,
) []ProposalConfig {
	normalMeanRank := normalMeanRanks[targetTeamID]
	mildRank, mediumRank, strongRank := proposalTargetRanks(candidatePositions, normalMeanRank, direction)

	strengths := getStrengthDefinitions(direction)
	sMap := make(map[int]StrengthDefinition, len(strengths))
	for _, s := range strengths {
		sMap[s.Level] = s
	}

	type depthLevelPair struct {
		depthRank int
		level     int
	}

	pairs := []depthLevelPair{
		{mildRank, 1},
		{mildRank, 2},
		{mildRank, 3},
		{mediumRank, 2},
		{mediumRank, 3},
		{mediumRank, 4},
		{strongRank, 3},
		{strongRank, 4},
		{strongRank, 5},
	}

	seen := make(map[string]bool)
	var configs []ProposalConfig

	for _, p := range pairs {
		sDef := sMap[p.level]
		cfg := buildSingleProposalConfig(games, targetTeamID, direction, p.depthRank, sDef, normalMeanRanks)
		if !seen[cfg.Name] {
			seen[cfg.Name] = true
			configs = append(configs, cfg)
		}
	}

	return configs
}

func proposalTargetRanks(
	candidatePositions []int,
	normalMeanRank float64,
	direction RareDirection,
) (mild int, medium int, strong int) {
	if len(candidatePositions) == 0 {
		return 0, 0, 0
	}

	sortedCandidates := make([]int, len(candidatePositions))
	copy(sortedCandidates, candidatePositions)
	sort.Ints(sortedCandidates)

	n := len(sortedCandidates)
	if n == 1 {
		val := sortedCandidates[0]
		return val, val, val
	}

	if direction == RareBetter {
		mild = sortedCandidates[n-1]
		strong = sortedCandidates[0]
		medium = sortedCandidates[n/2]
	} else {
		mild = sortedCandidates[0]
		strong = sortedCandidates[n-1]
		medium = sortedCandidates[n/2]
	}

	return mild, medium, strong
}

func calculateNormalMeanRanks(normalOdds map[int]*TeamOdds) map[int]float64 {
	ranks := make(map[int]float64, len(normalOdds))
	for teamID, odds := range normalOdds {
		meanRank := 0.0
		for pos, prob := range odds.Pos {
			meanRank += float64(pos) * prob
		}
		ranks[teamID] = meanRank
	}
	return ranks
}

func findRelevantCompetitorsForDepth(
	targetTeamID int,
	normalMeanRanks map[int]float64,
	targetDepthRank float64,
	direction RareDirection,
) []int {
	targetRank := normalMeanRanks[targetTeamID]
	var competitors []int

	for teamID, rank := range normalMeanRanks {
		if teamID == targetTeamID {
			continue
		}
		if direction == RareBetter {
			if rank >= targetDepthRank-1.5 && rank <= targetRank+0.5 {
				competitors = append(competitors, teamID)
			}
		} else {
			if rank >= targetRank-0.5 && rank <= targetDepthRank+1.5 {
				competitors = append(competitors, teamID)
			}
		}
	}
	return competitors
}

func findRelevantCompetitors(
	targetTeamID int,
	normalMeanRanks map[int]float64,
	candidatePositions []int,
	direction RareDirection,
) []int {
	if len(candidatePositions) == 0 {
		return nil
	}
	extremePos := candidatePositions[0]
	if direction == RareBetter {
		for _, p := range candidatePositions {
			if p < extremePos {
				extremePos = p
			}
		}
	} else {
		for _, p := range candidatePositions {
			if p > extremePos {
				extremePos = p
			}
		}
	}
	return findRelevantCompetitorsForDepth(targetTeamID, normalMeanRanks, float64(extremePos), direction)
}

func runRareProposalPilot(
	baseCampaign []*TeamCampaign,
	games []*GameType,
	table *Table,
	sortOrder []SortType,
	teamGroups []TeamType,
	targetTeamID int,
	direction RareDirection,
	candidatePositions []int,
	pilotConfigs []ProposalConfig,
	samplesPerConfig int,
	rng *rand.Rand,
	groupID int,
) []PilotProposalResult {
	numPositions := len(teamGroups)
	results := make([]PilotProposalResult, len(pilotConfigs))

	candidateSet := make(map[int]bool, len(candidatePositions))
	for _, pos := range candidatePositions {
		candidateSet[pos] = true
	}

	minCandidate := candidatePositions[0]
	maxCandidate := candidatePositions[0]
	for _, p := range candidatePositions {
		if p < minCandidate {
			minCandidate = p
		}
		if p > maxCandidate {
			maxCandidate = p
		}
	}

	simCampaign := make([]*TeamCampaign, len(baseCampaign))
	teamSlice := make([]*TeamCampaign, len(teamGroups))

	for cfgIdx, cfg := range pilotConfigs {
		rankHist := make([]int, numPositions)
		candidateHits := 0
		overshootHits := 0

		for s := 0; s < samplesPerConfig; s++ {
			rank, _ := simulateSingleProposalRank(
				baseCampaign, simCampaign, teamSlice, games,
				cfg.Means, table, sortOrder, teamGroups, targetTeamID, rng,
			)

			if rank >= 0 && rank < numPositions {
				rankHist[rank]++
			}

			if candidateSet[rank] {
				candidateHits++
			}

			if direction == RareBetter {
				if rank < minCandidate {
					overshootHits++
				}
			} else {
				if rank > maxCandidate {
					overshootHits++
				}
			}
		}

		cRate := float64(candidateHits) / float64(samplesPerConfig)
		oRate := float64(overshootHits) / float64(samplesPerConfig)
		sc := scorePilotProposal(cRate, oRate, cfg.StrengthLevel)

		res := PilotProposalResult{
			Config:        cfg,
			Samples:       samplesPerConfig,
			RankHistogram: rankHist,
			CandidateHits: candidateHits,
			CandidateRate: cRate,
			OvershootHits: overshootHits,
			OvershootRate: oRate,
			Score:         sc,
		}
		results[cfgIdx] = res

	}

	return results
}

func simulateSingleProposalRank(
	baseCampaign []*TeamCampaign,
	simCampaign []*TeamCampaign,
	teamSlice []*TeamCampaign,
	games []*GameType,
	means []GameProposalMeans,
	table *Table,
	sortOrder []SortType,
	teamGroups []TeamType,
	targetTeamID int,
	rng *rand.Rand,
) (rank int, logQOverP float64) {
	for k, v := range baseCampaign {
		if v != nil {
			simCampaign[k] = v.clone()
		} else {
			simCampaign[k] = nil
		}
	}

	for i, g := range games {
		if !g.Played {
			hMean := means[i].Home
			aMean := means[i].Away

			hScore := poissonRand(rng, hMean)
			aScore := poissonRand(rng, aMean)

			home := g.home_table_index
			away := g.away_table_index

			if simCampaign[home] != nil {
				simCampaign[home].add_game(&GameType{g.Id, g.HomeId, g.AwayId, hScore, aScore, 0.0, 0.0, true, home, away})
			}
			if simCampaign[away] != nil {
				simCampaign[away].add_game(&GameType{g.Id, g.HomeId, g.AwayId, hScore, aScore, 0.0, 0.0, true, home, away})
			}
		}
	}

	idx := 0
	for _, tg := range teamGroups {
		c := simCampaign[table.Query(uint32(tg.Team_id))]
		if c != nil {
			teamSlice[idx] = c
			idx++
		}
	}

	sortedTeams := TeamCampaignSorted{teamSlice[:idx], sortOrder}
	sort.Sort(sortedTeams)

	rank = -1
	for pos, t := range sortedTeams.t {
		if t.id == targetTeamID {
			rank = pos
			break
		}
	}

	return rank, 0.0
}

func validateProposalMixture(components []ProposalComponent, expectedGames int) {
	if len(components) == 0 {
		log.Fatalf("invalid proposal mixture: empty components")
	}

	totalWeight := 0.0
	for _, c := range components {
		if c.Weight < 0 {
			log.Fatalf("invalid proposal mixture: negative weight %f in component %s", c.Weight, c.Name)
		}
		if len(c.Means) != expectedGames {
			log.Fatalf("invalid proposal mixture: component %s has %d means, expected %d", c.Name, len(c.Means), expectedGames)
		}
		totalWeight += c.Weight
	}

	if math.Abs(totalWeight-1.0) > 1e-6 {
		log.Fatalf("invalid proposal mixture: weights sum to %f, expected 1.0", totalWeight)
	}
}

func buildProposalComponents(
	games []*GameType,
	targetTeamID int,
	direction RareDirection,
	candidatePositions []int,
	normalMeanRanks map[int]float64,
) []ProposalComponent {
	originalMeans := make([]GameProposalMeans, len(games))
	for i, g := range games {
		originalMeans[i] = GameProposalMeans{Home: g.HomePower, Away: g.AwayPower}
	}

	if len(candidatePositions) == 0 {
		return []ProposalComponent{
			{Name: "original", Weight: 1.0, Means: originalMeans},
		}
	}

	return buildSearchProposalForLevel(games, targetTeamID, direction, candidatePositions[0], 3, normalMeanRanks, originalMeans).Components
}

func buildDirectionalProposal(
	games []*GameType,
	targetTeamID int,
	direction RareDirection,
) []GameProposalMeans {
	comps := buildProposalComponents(games, targetTeamID, direction, nil, nil)
	return comps[len(comps)-1].Means
}

func affordableSamples(requested int, remainingWork, workPerSample int64) int {
	if requested <= 0 || remainingWork <= 0 || workPerSample <= 0 {
		return 0
	}
	maxSamples := remainingWork / workPerSample
	if maxSamples < int64(requested) {
		return int(maxSamples)
	}
	return requested
}

func productionPriority(candidate *FrontierCandidate) float64 {
	st := candidate.SearchState
	if st.BestProposal == nil {
		return math.Inf(-1)
	}
	priority := 200.0 - 30.0*float64(st.BestProposal.StrengthLevel) + float64(candidate.Priority)/100.0
	if st.Difficult {
		priority -= 70
	} else if st.BestPilotRate <= MaxUsefulPilotRate {
		priority += 20
	} else {
		priority -= 100 * (st.BestPilotRate - MaxUsefulPilotRate)
	}
	if st.BestPilotSamples > 0 {
		priority -= 10 * float64(st.BestOvershootHits) / float64(st.BestPilotSamples)
	}
	return priority
}

type ProductionAllocation struct {
	Candidate *FrontierCandidate
	Samples   int
}

// Plan only after the whole frontier has completed its initial pilot. Each
// allocation is fixed before its production estimator starts.
func planProductionAllocations(candidates []*FrontierCandidate, remainingWork, workPerSample int64) []ProductionAllocation {
	for _, candidate := range candidates {
		if candidate.SearchState.Status == StatusFrontier {
			return nil
		}
	}
	var promising []*FrontierCandidate
	for _, candidate := range candidates {
		if candidate.SearchState.Status == StatusPromising && candidate.SearchState.BestProposal != nil {
			promising = append(promising, candidate)
		}
	}
	sort.Slice(promising, func(i, j int) bool {
		pi, pj := productionPriority(promising[i]), productionPriority(promising[j])
		if pi != pj {
			return pi > pj
		}
		if promising[i].TeamID != promising[j].TeamID {
			return promising[i].TeamID < promising[j].TeamID
		}
		return promising[i].Position < promising[j].Position
	})
	if workPerSample <= 0 || remainingWork <= 0 {
		return nil
	}
	totalSamples := remainingWork / workPerSample
	n := len(promising)
	if n > int(totalSamples/2000) {
		n = int(totalSamples / 2000)
	}
	if n == 0 {
		return nil
	}
	allocations := make([]ProductionAllocation, n)
	for i := range allocations {
		allocations[i] = ProductionAllocation{Candidate: promising[i], Samples: 2000}
	}
	extra := int(totalSamples) - 2000*n
	weightSum := 0
	for i := range allocations {
		if !allocations[i].Candidate.SearchState.Difficult {
			weightSum += n - i
		}
	}
	if weightSum > 0 {
		initialExtra := extra
		for i := range allocations {
			if allocations[i].Candidate.SearchState.Difficult {
				continue
			}
			share := initialExtra * (n - i) / weightSum
			if share > 20000-allocations[i].Samples {
				share = 20000 - allocations[i].Samples
			}
			allocations[i].Samples += share
			extra -= share
		}
		for i := range allocations {
			if extra == 0 {
				break
			}
			if allocations[i].Candidate.SearchState.Difficult {
				continue
			}
			share := 20000 - allocations[i].Samples
			if share > extra {
				share = extra
			}
			allocations[i].Samples += share
			extra -= share
		}
	}
	return allocations
}

func searchAndMergeRarePositions(
	group *GroupType,
	campaign []*TeamCampaign,
	table *Table,
	sortOrder []SortType,
	normalPositionCounts map[int][]int,
	teamOdds []OddsType,
) {
	rng := rand.New(rand.NewSource(time.Now().UnixNano()))

	unplayedGames := 0
	for _, g := range group.Games {
		if !g.Played {
			unplayedGames++
		}
	}

	numTeams := len(group.Team_groups)
	totalWorkLimit := calculateMaxRareWork(unplayedGames, numTeams)
	remainingWork := totalWorkLimit
	pilotWorkRemaining := totalWorkLimit / 4

	originalMeans := make([]GameProposalMeans, len(group.Games))
	for i, g := range group.Games {
		originalMeans[i] = GameProposalMeans{Home: g.HomePower, Away: g.AwayPower}
	}

	normalMeanRanks := make(map[int]float64, numTeams)
	teamSearches := make(map[int]*TeamRareSearch, numTeams)

	for _, tg := range group.Team_groups {
		teamID := tg.Team_id
		index := table.Query(uint32(teamID))
		tOdds := teamOdds[index].team
		counts := normalPositionCounts[teamID]

		ts := initializeTeamRareSearch(teamID, counts, tOdds.Pos, campaign, group.Team_groups, group.Games, table, sortOrder)
		teamSearches[teamID] = ts
		normalMeanRanks[teamID] = ts.NormalMeanRank
	}

	teamRareEstimates := make(map[int]map[int]RarePositionEstimate)
	prodWorkPerSeason := estimateSeasonWork(unplayedGames, 4, numTeams)

	for remainingWork > 0 && pilotWorkRemaining > 0 {
		var activeCandidates []*FrontierCandidate
		for _, ts := range teamSearches {
			cands := discoverFrontier(ts)
			activeCandidates = append(activeCandidates, cands...)
		}

		if len(activeCandidates) == 0 {
			break
		}

		sort.Slice(activeCandidates, func(i, j int) bool {
			if activeCandidates[i].Priority != activeCandidates[j].Priority {
				return activeCandidates[i].Priority > activeCandidates[j].Priority
			}
			if activeCandidates[i].TeamID != activeCandidates[j].TeamID {
				return activeCandidates[i].TeamID < activeCandidates[j].TeamID
			}
			return activeCandidates[i].Position < activeCandidates[j].Position
		})

		pilotWorkPerSample := estimateSeasonWork(unplayedGames, 1, numTeams)
		pilotSamples := affordableSamples(PilotSamplesPerLevel, pilotWorkRemaining,
			int64(len(activeCandidates)*5)*pilotWorkPerSample)
		pilotSamples = affordableSamples(pilotSamples, remainingWork,
			int64(len(activeCandidates)*5)*pilotWorkPerSample)
		if pilotSamples == 0 {
			break
		}

		for _, cand := range activeCandidates {
			searchWorkSpent := evaluateFrontierCandidate(
				cand, campaign, group.Games, table, sortOrder, group.Team_groups,
				originalMeans, normalMeanRanks, rng, group.Id, unplayedGames, pilotSamples,
			)

			remainingWork -= searchWorkSpent
			pilotWorkRemaining -= searchWorkSpent
			st := cand.SearchState
			if st.BestProposal != nil {
				log.Printf("rare-position-pilot-selection: group=%d team=%d position=%d selected_level=%d selected_rate=%.4f priority=%.2f difficult=%t",
					group.Id, cand.TeamID, cand.Position, st.BestProposal.StrengthLevel,
					st.BestPilotRate, productionPriority(cand), st.Difficult)
			}
		}

		productionWork := remainingWork - pilotWorkRemaining
		allocations := planProductionAllocations(activeCandidates, productionWork, prodWorkPerSeason)
		for _, allocation := range allocations {
			cand := allocation.Candidate
			st := cand.SearchState
			targetSamples := affordableSamples(allocation.Samples, remainingWork, prodWorkPerSeason)
			if targetSamples <= 0 {
				break
			}

			job := &RareSimulationJob{
				TeamID:             cand.TeamID,
				Direction:          cand.Direction,
				CandidatePositions: []int{cand.Position},
				Components:         st.BestProposal.Components,
				Iterations:         targetSamples,
			}

			jobEstimates, _ := estimateRarePositionsForJob(
				campaign, group.Games, originalMeans, table, sortOrder, group.Team_groups,
				job, rng, group.Id,
			)

			prodWorkSpent := int64(targetSamples) * prodWorkPerSeason
			remainingWork -= prodWorkSpent
			st.ProductionWorkSpent += prodWorkSpent

			if est, ok := jobEstimates[cand.Position]; ok && est.Found && est.Probability >= MinInterestingProbability {
				st.Status = StatusResolved
				st.ProductionEstimate = &ProductionEstimate{
					Probability: est.Probability,
					StdErr:      est.StdErr,
					Samples:     est.Samples,
					Hits:        est.Hits,
					ESS:         est.ESS,
					Found:       true,
					WorkSpent:   prodWorkSpent,
				}

				if teamRareEstimates[cand.TeamID] == nil {
					teamRareEstimates[cand.TeamID] = make(map[int]RarePositionEstimate)
				}
				teamRareEstimates[cand.TeamID][cand.Position] = est
			} else if est, ok := jobEstimates[cand.Position]; ok && est.Found && est.Probability < MinInterestingProbability {
				st.Status = StatusBelowInterest
			} else {
				st.Status = StatusExhausted
			}
		}
	}

	for _, tg := range group.Team_groups {
		teamID := tg.Team_id
		if rareEsts, ok := teamRareEstimates[teamID]; ok && len(rareEsts) > 0 {
			index := table.Query(uint32(teamID))
			tOdds := teamOdds[index].team
			counts := normalPositionCounts[teamID]

			finalProbs := mergeRarePositionEstimates(tOdds.Pos, counts, rareEsts)
			copy(tOdds.Pos, finalProbs)
		}
	}

	workSpent := totalWorkLimit - remainingWork
	log.Printf("rare-position-summary: group=%d normal_sims=%d total_work_limit=%d remaining_work=%d work_spent=%d teams=%d",
		group.Id, NormalIterations, totalWorkLimit, remainingWork, workSpent, numTeams)
}

type RarePositionEstimate struct {
	Probability float64
	StdErr      float64
	Samples     int
	Hits        int
	ESS         float64
	Found       bool
}

const OriginalMixtureWeight = 0.05

func mergeRarePositionEstimates(
	normalProbs []float64,
	normalCounts []int,
	rareEstimates map[int]RarePositionEstimate,
) []float64 {
	numPositions := len(normalProbs)
	final := make([]float64, numPositions)

	rareMass := 0.0
	for _, est := range rareEstimates {
		if est.Found && est.Probability >= MinInterestingProbability {
			rareMass += est.Probability
		}
	}

	observedNormalMass := 0.0
	for pos := 0; pos < numPositions; pos++ {
		if normalCounts[pos] > 0 {
			observedNormalMass += normalProbs[pos]
		}
	}

	if rareMass >= 1.0 || math.IsNaN(rareMass) || math.IsInf(rareMass, 0) {
		log.Printf("WARNING: rareMass (%f) >= 1.0 or invalid, rejecting rare position adjustments", rareMass)
		copy(final, normalProbs)
		return final
	}

	for pos := 0; pos < numPositions; pos++ {
		est, isRare := rareEstimates[pos]
		if isRare && est.Found && est.Probability >= MinInterestingProbability {
			final[pos] = est.Probability
		} else if normalCounts[pos] > 0 {
			if observedNormalMass > 0 {
				final[pos] = (normalProbs[pos] / observedNormalMass) * (1.0 - rareMass)
			} else {
				final[pos] = normalProbs[pos]
			}
		} else {
			final[pos] = 0.0
		}
	}

	return final
}

type RareSimulationJob struct {
	TeamID               int
	Direction            RareDirection
	CandidatePositions   []int
	PilotConfigs         []ProposalConfig
	Components           []ProposalComponent
	PilotIterations      int
	ProductionIterations int
	Iterations           int
	Priority             int
}

const (
	NormalIterations    = 10000
	MaxRareIterations   = 100000
	MinIterationsPerJob = 2500
	MaxIterationsPerJob = 7500
)

func simulateTargetTeamRankAndWeightMulti(
	baseCampaign []*TeamCampaign,
	simCampaign []*TeamCampaign,
	teamSlice []*TeamCampaign,
	games []*GameType,
	originalMeans []GameProposalMeans,
	components []ProposalComponent,
	table *Table,
	sortOrder []SortType,
	teamGroups []TeamType,
	targetTeamID int,
	rng *rand.Rand,
	logQOverPBuf []float64,
	compWeightsBuf []float64,
) (rank int, weight float64, chosenComponent int) {
	for k, v := range baseCampaign {
		if v != nil {
			simCampaign[k] = v.clone()
		} else {
			simCampaign[k] = nil
		}
	}

	numComps := len(components)
	for i := 0; i < numComps; i++ {
		logQOverPBuf[i] = 0.0
		compWeightsBuf[i] = components[i].Weight
	}

	rVal := rng.Float64()
	cum := 0.0
	chosenK := 0
	for i := 0; i < numComps; i++ {
		cum += components[i].Weight
		if rVal <= cum || i == numComps-1 {
			chosenK = i
			break
		}
	}

	activeMeans := components[chosenK].Means

	for i, g := range games {
		if !g.Played {
			origH := originalMeans[i].Home
			origA := originalMeans[i].Away

			hScore := poissonRand(rng, activeMeans[i].Home)
			aScore := poissonRand(rng, activeMeans[i].Away)

			for k := 0; k < numComps; k++ {
				cMeans := components[k].Means[i]
				logQOverPBuf[k] += logPoissonQOverP(hScore, origH, cMeans.Home)
				logQOverPBuf[k] += logPoissonQOverP(aScore, origA, cMeans.Away)
			}

			home := g.home_table_index
			away := g.away_table_index

			if simCampaign[home] != nil {
				simCampaign[home].add_game(&GameType{g.Id, g.HomeId, g.AwayId, hScore, aScore, 0.0, 0.0, true, home, away})
			}
			if simCampaign[away] != nil {
				simCampaign[away].add_game(&GameType{g.Id, g.HomeId, g.AwayId, hScore, aScore, 0.0, 0.0, true, home, away})
			}
		}
	}

	weight = mixtureImportanceWeightMulti(logQOverPBuf, compWeightsBuf)

	idx := 0
	for _, tg := range teamGroups {
		c := simCampaign[table.Query(uint32(tg.Team_id))]
		if c != nil {
			teamSlice[idx] = c
			idx++
		}
	}

	sortedTeams := TeamCampaignSorted{teamSlice[:idx], sortOrder}
	sort.Sort(sortedTeams)

	rank = -1
	for pos, t := range sortedTeams.t {
		if t.id == targetTeamID {
			rank = pos
			break
		}
	}

	return rank, weight, chosenK
}

const (
	MinUsableESS   = 10.0
	MaxUsableRelSE = 0.50
)

func rareEstimateUsable(est RarePositionEstimate) bool {
	if est.Hits == 0 ||
		est.Probability < MinInterestingProbability ||
		math.IsNaN(est.Probability) ||
		math.IsInf(est.Probability, 0) {
		return false
	}

	relSE := est.StdErr / est.Probability

	return est.ESS >= MinUsableESS && relSE <= MaxUsableRelSE
}

func estimateRarePositionsForJob(
	baseCampaign []*TeamCampaign,
	games []*GameType,
	originalMeans []GameProposalMeans,
	table *Table,
	sortOrder []SortType,
	teamGroups []TeamType,
	job *RareSimulationJob,
	rng *rand.Rand,
	groupID int,
) (map[int]RarePositionEstimate, int) {
	results := make(map[int]RarePositionEstimate)
	if job.Iterations <= 0 || len(job.CandidatePositions) == 0 || len(job.Components) == 0 {
		return results, 0
	}

	simCampaign := make([]*TeamCampaign, len(baseCampaign))
	teamSlice := make([]*TeamCampaign, len(teamGroups))

	numPositions := len(teamGroups)
	rankHist := make([]int, numPositions)

	sumY := make(map[int]float64)
	sumY2 := make(map[int]float64)
	hits := make(map[int]int)
	eventWeights := make(map[int][]float64)

	candidateMap := make(map[int]bool)
	for _, pos := range job.CandidatePositions {
		candidateMap[pos] = true
	}

	numComps := len(job.Components)
	logQBuf := make([]float64, numComps)
	compWeightsBuf := make([]float64, numComps)
	componentSampleCounts := make([]int, numComps)
	componentRankHists := make([][]int, numComps)
	for i := range componentRankHists {
		componentRankHists[i] = make([]int, numPositions)
	}

	totalN := job.Iterations
	for i := 0; i < totalN; i++ {
		rank, w, chosenComponent := simulateTargetTeamRankAndWeightMulti(
			baseCampaign, simCampaign, teamSlice, games,
			originalMeans, job.Components, table, sortOrder, teamGroups,
			job.TeamID, rng, logQBuf, compWeightsBuf,
		)
		componentSampleCounts[chosenComponent]++

		if rank >= 0 && rank < numPositions {
			rankHist[rank]++
			componentRankHists[chosenComponent][rank]++
		}

		if candidateMap[rank] {
			sumY[rank] += w
			sumY2[rank] += w * w
			hits[rank]++
			eventWeights[rank] = append(eventWeights[rank], w)
		}
	}

	log.Printf("rare-position-rank-hist: group=%d team=%d direction=%d samples=%d hist=%v",
		groupID, job.TeamID, job.Direction, totalN, rankHist)

	N := float64(totalN)
	for _, pos := range job.CandidatePositions {
		sY := sumY[pos]
		sY2 := sumY2[pos]
		h := hits[pos]

		pHat := sY / N
		sampleVar := (sY2 - N*pHat*pHat) / (N - 1.0)
		if sampleVar < 0 {
			sampleVar = 0
		}
		stdErr := math.Sqrt(sampleVar / N)
		ess := 0.0
		if sY2 > 0 {
			ess = (sY * sY) / sY2
		}

		est := RarePositionEstimate{
			Probability: pHat,
			StdErr:      stdErr,
			Samples:     totalN,
			Hits:        h,
			ESS:         ess,
			Found:       false,
		}

		if rareEstimateUsable(est) {
			est.Found = true
		}

		if h > 0 {
			posWeights := eventWeights[pos]
			sort.Float64s(posWeights)
			minW := posWeights[0]
			maxW := posWeights[len(posWeights)-1]
			medW := posWeights[len(posWeights)/2]
			p90W := posWeights[int(float64(len(posWeights))*0.90)]
			p99W := posWeights[int(float64(len(posWeights))*0.99)]

			log.Printf("rare-position-weights: group=%d team=%d pos=%d hits=%d min_w=%.6f med_w=%.6f p90_w=%.6f p99_w=%.6f max_w=%.6f",
				groupID, job.TeamID, pos, h, minW, medW, p90W, p99W, maxW)
		}

		results[pos] = est
		relSE := math.Inf(1)
		if pHat > 0 {
			relSE = stdErr / pHat
		}
		log.Printf("rare-position-job: group=%d team=%d pos=%d direction=%d samples=%d hits=%d p=%.7f se=%.7f relSE=%.3f ess=%.1f found=%t",
			groupID, job.TeamID, pos, job.Direction, totalN, h, pHat, stdErr, relSE, ess, est.Found)
	}

	for i, comp := range job.Components {
		log.Printf("rare-position-component-hist: group=%d team=%d component=%s weight=%.2f target_rank=%d samples=%d hist=%v",
			groupID, job.TeamID, comp.Name, comp.Weight, comp.TargetRank,
			componentSampleCounts[i], componentRankHists[i])
	}

	return results, totalN
}
