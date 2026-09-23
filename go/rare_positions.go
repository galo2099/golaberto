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

const (
	MinInterestingProbability = 1e-5
	ScoutIterations           = 20000
)

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
	Probability         float64  `json:"probability"`
	StdErr              float64  `json:"std_err"`
	Samples             int      `json:"samples"`
	Hits                int      `json:"hits"`
	ESS                 float64  `json:"ess"`
	MeanWeight          float64  `json:"mean_weight"`
	WorkSpent           int64    `json:"work_spent"`
	Available           bool     `json:"available"`
	MeetsPrecisionGoal  bool     `json:"meets_precision_goal"`
	RelativeSE          *float64 `json:"relative_se"`
	MaxEventWeightShare float64  `json:"max_event_weight_share"`
	ZeroHitUpper95      float64  `json:"zero_hit_upper_95"`
	Design              string   `json:"design"`
}

type ProductionDesign struct {
	Kind                 string
	Components           []ProposalComponent
	TargetTeam           int
	TargetPosition       int
	Samples              int
	Work                 int64
	SelectedFromSnapshot int
}

type PositionSearchState struct {
	Position                      int
	Status                        PositionSearchStatus
	Feasible                      bool
	ObservedCount                 int
	NormalProb                    float64
	BestProposal                  *SearchProposal
	BestPilot                     *WeightedPilotResult
	Pilots                        []*WeightedPilotResult
	ProductionEstimate            *ProductionEstimate
	SearchWorkSpent               int64
	ProductionWorkSpent           int64
	CEMFailedConfirmationProposal *CEMProposal // legacy-only state
	CEMEverHadExactHit            bool
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
		if st.Status == StatusObserved || st.Status == StatusResolved {
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
	return buildSearchProposalForSpec(games, SparseProposalSpec{
		TargetTeamID: targetTeamID, TargetRank: targetRank, Direction: direction,
		StrengthLevel: level, TargetGameLimit: 2, BoundaryCompetitorLimit: 0,
	}, normalMeanRanks, originalMeans)
}

func buildSearchProposalForSpec(
	games []*GameType, spec SparseProposalSpec, normalMeanRanks map[int]float64,
	originalMeans []GameProposalMeans,
) SearchProposal {
	strengths := getStrengthDefinitions(spec.Direction)
	sMap := make(map[int]StrengthDefinition, len(strengths))
	for _, s := range strengths {
		sMap[s.Level] = s
	}

	level := spec.StrengthLevel
	bestDef := sMap[level]
	bestCfg := buildSparseProposalConfig(games, spec, bestDef, normalMeanRanks)

	comps := []ProposalComponent{
		{
			Name:          "original",
			Weight:        OriginalMixtureWeight,
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
			neighbor := spec
			neighbor.StrengthLevel = lvl
			cfg = buildSparseProposalConfig(games, neighbor, sMap[lvl], normalMeanRanks)
		}
		comps = append(comps, ProposalComponent{
			Name:          cfg.Name,
			Weight:        weights[i],
			Means:         cfg.Means,
			RelevantTeams: cfg.RelevantTeams,
			TargetRank:    spec.TargetRank,
		})
	}

	validateProposalMixture(comps, len(games))

	return SearchProposal{
		Name:          bestCfg.Name,
		Direction:     spec.Direction,
		StrengthLevel: level,
		TargetRank:    spec.TargetRank,
		Components:    comps,
	}
}

const (
	MinPilotHitsForProduction  = 3
	MinPilotESSForProduction   = 2.0
	TargetProductionESS        = 15.0
	ProductionWorkSafetyFactor = 1.25
)

type WeightedPilotResult struct {
	Proposal            SearchProposal
	Samples             int
	WorkSpent           int64
	Hits                int
	SumY                float64
	SumY2               float64
	MaxEventWeight      float64
	Probability         float64
	StdErr              float64
	RelSE               float64
	ESS                 float64
	MaxEventWeightShare float64
	ESSPerWork          float64
	Refined             bool
	RankHistogram       []int
	ComponentSamples    []int
	ComponentRankHists  [][]int
}

func updateWeightedPilotStats(pilot *WeightedPilotResult) {
	pilot.Probability, pilot.StdErr, pilot.ESS = eventEstimateStats(pilot.SumY, pilot.SumY2, pilot.Samples)
	pilot.RelSE = math.Inf(1)
	if pilot.Probability > 0 {
		pilot.RelSE = pilot.StdErr / pilot.Probability
		pilot.MaxEventWeightShare = pilot.MaxEventWeight / pilot.SumY
	}
	if pilot.WorkSpent > 0 {
		pilot.ESSPerWork = pilot.ESS / float64(pilot.WorkSpent)
	}
}

func eventEstimateStats(sumY, sumY2 float64, samples int) (probability, stdErr, ess float64) {
	if samples <= 0 {
		return 0, 0, 0
	}
	n := float64(samples)
	probability = sumY / n
	if samples > 1 {
		variance := (sumY2 - n*probability*probability) / (n - 1)
		if variance < 0 {
			variance = 0
		}
		stdErr = math.Sqrt(variance / n)
	}
	if sumY2 > 0 {
		ess = sumY * sumY / sumY2
	}
	return probability, stdErr, ess
}

// A refinement appends fresh draws from the same fixed mixture to its pilot.
func evaluateWeightedPilot(
	pilot *WeightedPilotResult,
	baseCampaign []*TeamCampaign,
	games []*GameType,
	originalMeans []GameProposalMeans,
	table *Table,
	sortOrder []SortType,
	teamGroups []TeamType,
	teamID, position, samples int,
	workPerSample int64,
	rng *rand.Rand,
	groupID int,
) {
	if samples <= 0 {
		return
	}
	components := pilot.Proposal.Components
	if pilot.RankHistogram == nil {
		pilot.RankHistogram = make([]int, len(teamGroups))
		pilot.ComponentSamples = make([]int, len(components))
		pilot.ComponentRankHists = make([][]int, len(components))
		for i := range pilot.ComponentRankHists {
			pilot.ComponentRankHists[i] = make([]int, len(teamGroups))
		}
	}
	simCampaign := make([]*TeamCampaign, len(baseCampaign))
	teamSlice := make([]*TeamCampaign, len(teamGroups))
	logQ := make([]float64, len(components))
	componentWeights := make([]float64, len(components))
	for i := 0; i < samples; i++ {
		rank, weight, chosen := simulateTargetTeamRankAndWeightMulti(
			baseCampaign, simCampaign, teamSlice, games, originalMeans, components,
			table, sortOrder, teamGroups, teamID, rng, logQ, componentWeights,
		)
		pilot.ComponentSamples[chosen]++
		if rank >= 0 && rank < len(teamGroups) {
			pilot.RankHistogram[rank]++
			pilot.ComponentRankHists[chosen][rank]++
		}
		if rank == position {
			pilot.Hits++
			pilot.SumY += weight
			pilot.SumY2 += weight * weight
			if weight > pilot.MaxEventWeight {
				pilot.MaxEventWeight = weight
			}
		}
	}
	pilot.Samples += samples
	pilot.WorkSpent += int64(samples) * workPerSample
	updateWeightedPilotStats(pilot)
	log.Printf("rare-position-weighted-pilot: group=%d team=%d pos=%d center_level=%d samples=%d hits=%d hit_rate=%.4f p=%.3e ess=%.2f relSE=%.3f max_weight_share=%.3f work=%d ess_per_million_work=%.3f refined=%t hist=%v",
		groupID, teamID, position, pilot.Proposal.StrengthLevel, pilot.Samples, pilot.Hits,
		float64(pilot.Hits)/float64(pilot.Samples), pilot.Probability, pilot.ESS, pilot.RelSE,
		pilot.MaxEventWeightShare, pilot.WorkSpent, pilot.ESSPerWork*1e6, pilot.Refined, pilot.RankHistogram)
	for k, component := range components {
		log.Printf("rare-position-pilot-component-hist: group=%d team=%d pos=%d center_level=%d component=%s samples=%d hist=%v",
			groupID, teamID, position, pilot.Proposal.StrengthLevel,
			component.Name, pilot.ComponentSamples[k], pilot.ComponentRankHists[k])
	}
}

func pilotBetter(a, b *WeightedPilotResult) bool {
	if a.ESSPerWork != b.ESSPerWork {
		return a.ESSPerWork > b.ESSPerWork
	}
	if a.MaxEventWeightShare != b.MaxEventWeightShare {
		return a.MaxEventWeightShare < b.MaxEventWeightShare
	}
	return a.Proposal.StrengthLevel < b.Proposal.StrengthLevel
}

func selectPilotMixture(results []*WeightedPilotResult) *WeightedPilotResult {
	var best *WeightedPilotResult
	for _, result := range results {
		if !result.Refined || result.Hits < MinPilotHitsForProduction || result.ESS < MinPilotESSForProduction {
			continue
		}
		if best == nil || pilotBetter(result, best) {
			best = result
		}
	}
	return best
}

func projectedWorkForESS(pilot *WeightedPilotResult) int64 {
	if pilot == nil || pilot.ESS <= 0 || pilot.WorkSpent <= 0 {
		return 0
	}
	return int64(math.Ceil(float64(pilot.WorkSpent) * TargetProductionESS /
		pilot.ESS * ProductionWorkSafetyFactor))
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

func proposalMean(original, multiplier float64) float64 {
	if original == 0 || multiplier == 1 {
		return original
	}
	if original < 0 || math.IsNaN(original) || math.IsInf(original, 0) ||
		multiplier < 0 || math.IsNaN(multiplier) || math.IsInf(multiplier, 0) {
		panic("invalid proposal mean or multiplier")
	}
	proposed := original * multiplier
	if math.IsInf(proposed, 0) {
		proposed = math.MaxFloat64
	}
	if multiplier < 1 {
		if original < MinProposalMean {
			return proposed
		}
		return math.Max(MinProposalMean, proposed)
	}
	if original > MaxProposalMean {
		return proposed
	}
	return math.Min(MaxProposalMean, proposed)
}

type SparseProposalSpec struct {
	TargetTeamID            int
	TargetRank              int
	Direction               RareDirection
	StrengthLevel           int
	TargetGameLimit         int
	BoundaryCompetitorLimit int
	CompetitorGameLimit     int
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

func closestBoundaryCompetitors(targetTeamID, targetRank, limit int, direction RareDirection, normalMeanRanks map[int]float64) []int {
	if limit <= 0 {
		return nil
	}
	boundary := float64(targetRank)
	if direction == RareBetter {
		boundary += 0.5
	} else {
		boundary -= 0.5
	}
	ids := make([]int, 0, len(normalMeanRanks))
	for id := range normalMeanRanks {
		if id != targetTeamID {
			ids = append(ids, id)
		}
	}
	sort.Slice(ids, func(i, j int) bool {
		di := math.Abs(normalMeanRanks[ids[i]] - boundary)
		dj := math.Abs(normalMeanRanks[ids[j]] - boundary)
		if di != dj {
			return di < dj
		}
		return ids[i] < ids[j]
	})
	if limit < len(ids) {
		ids = ids[:limit]
	}
	return ids
}

func sparseGameLeverage(g *GameType, teamID int, multiplier float64) float64 {
	if g.HomeId == teamID {
		return math.Abs(proposalMean(g.HomePower, multiplier) - g.HomePower)
	}
	return math.Abs(proposalMean(g.AwayPower, multiplier) - g.AwayPower)
}

func rankedSparseGames(games []*GameType, teamID, targetID int, selected map[int]bool, multiplier float64) []int {
	var indexes []int
	for i, g := range games {
		if g.Played || (g.HomeId != teamID && g.AwayId != teamID) {
			continue
		}
		if teamID != targetID && selected[g.HomeId] && selected[g.AwayId] {
			continue // blocker-vs-blocker games cannot help the target boundary directly
		}
		indexes = append(indexes, i)
	}
	sort.Slice(indexes, func(i, j int) bool {
		a, b := games[indexes[i]], games[indexes[j]]
		aHead := a.HomeId == targetID && selected[a.AwayId] || a.AwayId == targetID && selected[a.HomeId]
		bHead := b.HomeId == targetID && selected[b.AwayId] || b.AwayId == targetID && selected[b.HomeId]
		if aHead != bHead {
			return aHead
		}
		la, lb := sparseGameLeverage(a, teamID, multiplier), sparseGameLeverage(b, teamID, multiplier)
		if la != lb {
			return la > lb
		}
		if a.Id != b.Id {
			return a.Id < b.Id
		}
		return indexes[i] < indexes[j]
	})
	return indexes
}

func buildSparseProposalConfig(games []*GameType, spec SparseProposalSpec, sDef StrengthDefinition, normalMeanRanks map[int]float64) ProposalConfig {
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

	compBlockers := closestBoundaryCompetitors(spec.TargetTeamID, spec.TargetRank,
		spec.BoundaryCompetitorLimit, spec.Direction, normalMeanRanks)
	blockerMap := make(map[int]bool, len(compBlockers))
	for _, id := range compBlockers {
		blockerMap[id] = true
	}

	compMeans := append([]GameProposalMeans(nil), originalMeans...)
	targetGames := rankedSparseGames(games, spec.TargetTeamID, spec.TargetTeamID, blockerMap, sDef.TargetMult)
	targetLimit := spec.TargetGameLimit
	if targetLimit == -1 {
		targetLimit = len(targetGames)
	}
	for i := 0; i < targetLimit && i < len(targetGames); i++ {
		index := targetGames[i]
		g := games[index]
		if g.HomeId == spec.TargetTeamID {
			compMeans[index].Home = proposalMean(g.HomePower, sDef.TargetMult)
		} else {
			compMeans[index].Away = proposalMean(g.AwayPower, sDef.TargetMult)
		}
	}
	for _, blocker := range compBlockers {
		blockerGames := rankedSparseGames(games, blocker, spec.TargetTeamID, blockerMap, sDef.BlockMult)
		for i := 0; i < spec.CompetitorGameLimit && i < len(blockerGames); i++ {
			index := blockerGames[i]
			g := games[index]
			if g.HomeId == blocker {
				compMeans[index].Home = proposalMean(g.HomePower, sDef.BlockMult)
			} else {
				compMeans[index].Away = proposalMean(g.AwayPower, sDef.BlockMult)
			}
		}
	}

	targetScope := fmt.Sprint(spec.TargetGameLimit)
	if spec.TargetGameLimit == -1 {
		targetScope = "all"
	}
	return ProposalConfig{
		Name:              fmt.Sprintf("target%s_comp%dx%d_%s_lvl%d_rank%d", targetScope, spec.BoundaryCompetitorLimit, spec.CompetitorGameLimit, sDef.Name, sDef.Level, spec.TargetRank),
		StrengthLevel:     sDef.Level,
		TargetRank:        spec.TargetRank,
		TargetMultiplier:  sDef.TargetMult,
		BlockerMultiplier: sDef.BlockMult,
		RelevantTeams:     compBlockers,
		Means:             compMeans,
	}
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

type ProductionPlan struct {
	Candidate        *FrontierCandidate
	Pilot            *WeightedPilotResult
	PriorityScore    float64
	ProjectedWork    int64
	ProjectedSamples int
	Critical100      bool
}

type ProductionAllocation struct {
	Plan         ProductionPlan
	Samples      int
	Work         int64
	PredictedESS float64
}

func buildProductionPlans(candidates []*FrontierCandidate, workPerSample int64, teamSearches map[int]*TeamRareSearch) []ProductionPlan {
	var plans []ProductionPlan
	if workPerSample <= 0 {
		return plans
	}
	for _, candidate := range candidates {
		st := candidate.SearchState
		if st.Status != StatusPromising || st.BestPilot == nil {
			continue
		}
		projectedWork := projectedWorkForESS(st.BestPilot)
		if projectedWork <= 0 {
			continue
		}
		samples := int((projectedWork + workPerSample - 1) / workPerSample)
		plans = append(plans, ProductionPlan{
			Candidate: candidate, Pilot: st.BestPilot,
			PriorityScore: cemValidationPriority(st.BestPilot),
			ProjectedWork: projectedWork, ProjectedSamples: samples,
			Critical100: teamSearches[candidate.TeamID].Has100PercentNormal,
		})
	}
	return plans
}

func planProductionAllocations(plans []ProductionPlan, remainingWork, workPerSample int64) []ProductionAllocation {
	if remainingWork <= 0 || workPerSample <= 0 {
		return nil
	}
	ordered := append([]ProductionPlan(nil), plans...)
	sort.Slice(ordered, func(i, j int) bool {
		a, b := ordered[i], ordered[j]
		if a.Critical100 != b.Critical100 {
			return a.Critical100
		}
		if a.PriorityScore != b.PriorityScore {
			return a.PriorityScore > b.PriorityScore
		}
		if a.ProjectedWork != b.ProjectedWork {
			return a.ProjectedWork < b.ProjectedWork
		}
		if a.Pilot.ESSPerWork != b.Pilot.ESSPerWork {
			return a.Pilot.ESSPerWork > b.Pilot.ESSPerWork
		}
		if a.Candidate.Priority != b.Candidate.Priority {
			return a.Candidate.Priority > b.Candidate.Priority
		}
		if a.Candidate.TeamID != b.Candidate.TeamID {
			return a.Candidate.TeamID < b.Candidate.TeamID
		}
		return a.Candidate.Position < b.Candidate.Position
	})
	var allocations []ProductionAllocation
	for _, plan := range ordered {
		samples := affordableSamples(plan.ProjectedSamples, remainingWork, workPerSample)
		if samples <= 0 {
			continue
		}
		work := int64(samples) * workPerSample
		predictedESS := plan.Pilot.ESS * float64(work) / float64(plan.Pilot.WorkSpent)
		if samples < plan.ProjectedSamples && predictedESS < MinUsableESS {
			continue
		}
		allocations = append(allocations, ProductionAllocation{
			Plan: plan, Samples: samples, Work: work, PredictedESS: predictedESS,
		})
		remainingWork -= work
	}
	return allocations
}
func collectFrontierCandidates(teamSearches map[int]*TeamRareSearch) []*FrontierCandidate {
	var candidates []*FrontierCandidate
	for _, search := range teamSearches {
		candidates = append(candidates, discoverFrontier(search)...)
	}
	sort.Slice(candidates, func(i, j int) bool {
		if candidates[i].Priority != candidates[j].Priority {
			return candidates[i].Priority > candidates[j].Priority
		}
		if candidates[i].TeamID != candidates[j].TeamID {
			return candidates[i].TeamID < candidates[j].TeamID
		}
		return candidates[i].Position < candidates[j].Position
	})
	return candidates
}

func planFallbackSamples(remainingWork, plainWorkPerSample int64) (int, int64) {
	if remainingWork <= 0 || plainWorkPerSample <= 0 {
		return 0, 0
	}
	samples := int(remainingWork / plainWorkPerSample)
	return samples, int64(samples) * plainWorkPerSample
}

// simulatePlainRankCounts draws a fresh ordinary-MC batch. In the active
// estimator path these counts are summarized on their own, never pooled with
// the adaptive scout histogram.
func simulatePlainRankCounts(baseCampaign []*TeamCampaign, games []*GameType,
	table *Table, sortOrder []SortType, teamGroups []TeamType, samples int,
	rng *rand.Rand) map[int][]int {
	counts := make(map[int][]int, len(teamGroups))
	for _, team := range teamGroups {
		counts[team.Team_id] = make([]int, len(teamGroups))
	}
	simCampaign := make([]*TeamCampaign, len(baseCampaign))
	teamSlice := make([]*TeamCampaign, len(teamGroups))
	for n := 0; n < samples; n++ {
		for k, campaign := range baseCampaign {
			simCampaign[k] = campaign.clone()
		}
		for _, g := range games {
			if g.Played {
				continue
			}
			homeScore := poissonRand(rng, g.HomePower)
			awayScore := poissonRand(rng, g.AwayPower)
			score := &GameType{g.Id, g.HomeId, g.AwayId, homeScore, awayScore, 0, 0, true,
				g.home_table_index, g.away_table_index}
			if simCampaign[g.home_table_index] != nil {
				simCampaign[g.home_table_index].add_game(score)
			}
			if simCampaign[g.away_table_index] != nil {
				simCampaign[g.away_table_index].add_game(score)
			}
		}
		for i, team := range teamGroups {
			teamSlice[i] = simCampaign[table.Query(uint32(team.Team_id))]
		}
		sort.Sort(TeamCampaignSorted{teamSlice, sortOrder})
		for rank, team := range teamSlice {
			counts[team.id][rank]++
		}
	}
	return counts
}

// combineNormalPositionCounts is retained for legacy-only tests; active
// search/production orchestration does not call it.
func combineNormalPositionCounts(initial, fallback map[int][]int, initialSamples, fallbackSamples int,
	teamOdds []OddsType, table *Table) (newNonzeroCells, brokenHundreds int) {
	combinedSamples := initialSamples + fallbackSamples
	if combinedSamples <= 0 {
		return 0, 0
	}
	for teamID, counts := range initial {
		additional := fallback[teamID]
		odds := teamOdds[table.Query(uint32(teamID))].team
		for pos := range counts {
			wasZero := counts[pos] == 0
			wasCertain := counts[pos] == initialSamples
			if pos < len(additional) {
				counts[pos] += additional[pos]
			}
			if wasZero && counts[pos] > 0 {
				newNonzeroCells++
			}
			if wasCertain && counts[pos] < combinedSamples {
				brokenHundreds++
			}
			odds.Pos[pos] = float64(counts[pos]) / float64(combinedSamples)
		}
	}
	return newNonzeroCells, brokenHundreds
}

func searchAndMergeRarePositions(
	group *GroupType,
	campaign []*TeamCampaign,
	table *Table,
	sortOrder []SortType,
	normalPositionCounts map[int][]int,
	teamOdds []OddsType,
	normalSamples int,
) map[int]map[int]ProductionEstimate {
	return runRarePositionSearchEvaluationProduction(group, campaign, table, sortOrder,
		normalPositionCounts, teamOdds, normalSamples)
}

func cloneProposalComponents(components []ProposalComponent) []ProposalComponent {
	cloned := make([]ProposalComponent, len(components))
	for i, component := range components {
		cloned[i] = component
		cloned[i].Means = append([]GameProposalMeans(nil), component.Means...)
		cloned[i].RelevantTeams = append([]int(nil), component.RelevantTeams...)
	}
	return cloned
}

func estimateMeetsPrecisionGoal(ess, relativeSE float64) bool {
	return ess >= MinUsableESS && relativeSE <= MaxUsableRelSE
}

func relativeSEPointer(relativeSE float64) *float64 {
	if math.IsNaN(relativeSE) || math.IsInf(relativeSE, 0) {
		return nil
	}
	value := relativeSE
	return &value
}

func relativeSEValue(relativeSE *float64) float64 {
	if relativeSE == nil {
		return math.Inf(1)
	}
	return *relativeSE
}

func zeroHitUpper95(samples int) float64 {
	if samples <= 0 {
		return 1
	}
	return math.Min(1, -math.Expm1(math.Log(0.05)/float64(samples)))
}

func weightedZeroHitUpper95(samples int, maximumWeight float64) float64 {
	if samples <= 0 || maximumWeight <= 0 {
		return 1
	}
	return math.Min(1, maximumWeight*math.Sqrt(-math.Log(0.05)/(2*float64(samples))))
}

func freezeProductionDesign(selected *CEMProposalEvaluation, original []GameProposalMeans,
	remainingWork, plainWorkPerSample, isWorkPerSample int64) ProductionDesign {
	design := ProductionDesign{Kind: "plain_mc", TargetPosition: -1,
		Components: []ProposalComponent{{Name: "P", Weight: 1,
			Means: append([]GameProposalMeans(nil), original...)}}}
	workPerSample := plainWorkPerSample
	if selected != nil {
		design.Kind = "importance_sampling"
		design.TargetTeam = selected.Snapshot.CandidateTeam
		design.TargetPosition = selected.Snapshot.CandidatePosition
		design.SelectedFromSnapshot = selected.Snapshot.SourceIteration
		design.Components = cloneProposalComponents(cemEvaluationMixture(original, selected.Snapshot))
		workPerSample = isWorkPerSample
	}
	if workPerSample > 0 && remainingWork > 0 {
		design.Samples = int(remainingWork / workPerSample)
		design.Work = int64(design.Samples) * workPerSample
	}
	return design
}

func summarizePlainProductionCounts(counts map[int][]int, samples int, work int64) map[int]map[int]ProductionEstimate {
	estimates := make(map[int]map[int]ProductionEstimate, len(counts))
	if samples <= 0 {
		return estimates
	}
	for teamID, rankCounts := range counts {
		estimates[teamID] = make(map[int]ProductionEstimate, len(rankCounts))
		for position, hits := range rankCounts {
			p := float64(hits) / float64(samples)
			se := 0.0
			if samples > 1 {
				se = math.Sqrt(p * (1 - p) / float64(samples-1))
			}
			relSE := math.Inf(1)
			if p > 0 {
				relSE = se / p
			}
			share := 0.0
			if hits > 0 {
				share = 1 / float64(hits)
			}
			upper := 0.0
			if hits == 0 {
				upper = zeroHitUpper95(samples)
			}
			estimates[teamID][position] = ProductionEstimate{
				Probability: p, StdErr: se, Samples: samples, Hits: hits,
				ESS: float64(hits), MeanWeight: 1, WorkSpent: work,
				Available: true, MeetsPrecisionGoal: estimateMeetsPrecisionGoal(float64(hits), relSE),
				RelativeSE: relativeSEPointer(relSE), MaxEventWeightShare: share,
				ZeroHitUpper95: upper, Design: "plain_mc",
			}
		}
	}
	return estimates
}

func runRarePositionSearchEvaluationProduction(group *GroupType, campaign []*TeamCampaign,
	table *Table, sortOrder []SortType, normalPositionCounts map[int][]int,
	teamOdds []OddsType, normalSamples int) map[int]map[int]ProductionEstimate {
	rng := rand.New(rand.NewSource(time.Now().UnixNano()))
	unplayedGames := 0
	for _, game := range group.Games {
		if !game.Played {
			unplayedGames++
		}
	}
	numTeams := len(group.Team_groups)
	totalWorkLimit := calculateMaxRareWork(unplayedGames, numTeams)
	plainWorkPerSample := estimateSeasonWork(unplayedGames, 1, numTeams)
	scoutWork := int64(normalSamples) * plainWorkPerSample
	remainingWork := totalWorkLimit - scoutWork
	if remainingWork < 0 {
		panic("rare-position scout work exceeded total work budget")
	}
	cemWorkLimit := int64(float64(totalWorkLimit) * MaxCEMWorkFraction)
	if cap := int64(MaxCEMPlainEquivalentSamples) * plainWorkPerSample; cemWorkLimit > cap {
		cemWorkLimit = cap
	}
	cemWorkRemaining := cemWorkLimit
	explorationWorkRemaining := int64(float64(cemWorkLimit) * CEMMaxExplorationFraction)
	adaptWorkPerSample := plainWorkPerSample
	evaluationWorkLimit := int64(float64(totalWorkLimit) * MaxCEMEvaluationWorkFraction)
	evaluationWorkRemaining := evaluationWorkLimit
	evaluationWorkPerSample := estimateSeasonWork(unplayedGames, 2, numTeams)
	productionISWorkPerSample := evaluationWorkPerSample

	originalMeans := make([]GameProposalMeans, len(group.Games))
	for i, game := range group.Games {
		originalMeans[i] = GameProposalMeans{Home: game.HomePower, Away: game.AwayPower}
	}
	teamSearches := make(map[int]*TeamRareSearch, numTeams)
	scoutResolvedCells := 0
	for _, team := range group.Team_groups {
		teamID := team.Team_id
		index := table.Query(uint32(teamID))
		search := initializeTeamRareSearch(teamID, normalPositionCounts[teamID],
			teamOdds[index].team.Pos, campaign, group.Team_groups, group.Games, table, sortOrder)
		teamSearches[teamID] = search
		for _, position := range search.Positions {
			if position.ObservedCount > 0 {
				scoutResolvedCells++
			}
		}
	}

	candidates := collectFrontierCandidates(teamSearches)
	var cemRound CEMRoundResult
	if len(candidates) > 0 && remainingWork > 0 && cemWorkRemaining > 0 {
		cemRound = runCEMAdaptationRound(candidates, teamSearches, group, campaign,
			table, sortOrder, originalMeans, &cemWorkRemaining, &remainingWork,
			&explorationWorkRemaining, adaptWorkPerSample, rng)
	}
	if remainingWork < 0 || cemWorkRemaining < 0 || explorationWorkRemaining < 0 {
		panic("rare-position adaptation work budget exceeded")
	}
	adaptationWork := cemRound.CEMWork

	evaluationCandidates := selectCEMEvaluationSnapshots(cemRound.Snapshots, CEMMaxEvaluationSnapshots)
	evaluations := make([]CEMProposalEvaluation, 0, len(evaluationCandidates))
	for _, snapshot := range evaluationCandidates {
		work := int64(CEMEvaluationSamplesPerSnapshot) * evaluationWorkPerSample
		if evaluationWorkRemaining < work || remainingWork < work {
			break
		}
		evaluation := evaluateCEMProposalSnapshot(snapshot, originalMeans, campaign,
			group.Games, table, sortOrder, group.Team_groups,
			CEMEvaluationSamplesPerSnapshot, evaluationWorkPerSample, rng, group.Id)
		evaluationWorkRemaining -= evaluation.Work
		remainingWork -= evaluation.Work
		if evaluation.Work != work {
			panic("CEM evaluation did not consume its frozen sample count")
		}
		evaluations = append(evaluations, evaluation)
	}
	evaluationWork := evaluationWorkLimit - evaluationWorkRemaining
	selectedEvaluation := selectCEMProductionEvaluation(evaluations)
	selectionReason := "no_evaluated_snapshot_with_exact_event"
	if selectedEvaluation != nil {
		selectionReason = "highest_evaluated_event_ess_per_work"
	}
	remainingBeforeProduction := remainingWork
	design := freezeProductionDesign(selectedEvaluation, originalMeans, remainingWork,
		plainWorkPerSample, productionISWorkPerSample)
	componentsLog := "P"
	if design.Kind == "importance_sampling" {
		componentsLog = fmt.Sprintf("%s:%.2f,%s:%.2f",
			design.Components[0].Name, design.Components[0].Weight,
			design.Components[1].Name, design.Components[1].Weight)
	}
	log.Printf("rare-position-design-selection: group=%d selected=%s team=%d position=%d snapshot_iteration=%d evaluation_ess=%.3f evaluation_ess_per_work=%.8g evaluation_hits=%d reason=%s",
		group.Id, design.Kind, design.TargetTeam, design.TargetPosition,
		design.SelectedFromSnapshot, evaluationESS(selectedEvaluation),
		evaluationESSPerWork(selectedEvaluation), evaluationHits(selectedEvaluation), selectionReason)
	log.Printf("rare-position-design-freeze: group=%d design=%s target_team=%d target_position=%d selected_snapshot_iteration=%d components=%s production_samples=%d production_work=%d remaining_work_before_production=%d",
		group.Id, design.Kind, design.TargetTeam, design.TargetPosition,
		design.SelectedFromSnapshot, componentsLog, design.Samples, design.Work, remainingBeforeProduction)

	productionEstimates := make(map[int]map[int]ProductionEstimate)
	if design.Samples > 0 {
		if design.Kind == "importance_sampling" {
			job := &RareSimulationJob{TeamID: design.TargetTeam,
				Direction: RareBetter, CandidatePositions: []int{design.TargetPosition},
				Components: cloneProposalComponents(design.Components), Iterations: design.Samples}
			raw, _ := estimateRarePositionsForJob(campaign, group.Games, originalMeans,
				table, sortOrder, group.Team_groups, job, rng, group.Id)
			estimate := raw[design.TargetPosition]
			estimate.WorkSpent = design.Work
			productionEstimates[design.TargetTeam] = map[int]ProductionEstimate{
				design.TargetPosition: {
					Probability: estimate.Probability, StdErr: estimate.StdErr,
					Samples: estimate.Samples, Hits: estimate.Hits, ESS: estimate.ESS,
					MeanWeight: estimate.MeanWeight, WorkSpent: estimate.WorkSpent,
					Available:           estimate.Available,
					MeetsPrecisionGoal:  estimate.MeetsPrecisionGoal,
					RelativeSE:          relativeSEPointer(estimate.RelativeSE),
					MaxEventWeightShare: estimate.MaxEventWeightShare,
					ZeroHitUpper95:      estimate.ZeroHitUpper95, Design: design.Kind,
				},
			}
		} else {
			counts := simulatePlainRankCounts(campaign, group.Games, table, sortOrder,
				group.Team_groups, design.Samples, rng)
			for teamID, rankCounts := range counts {
				log.Printf("rare-position-production-rank-hist: group=%d design=plain_mc team=%d samples=%d hist=%v",
					group.Id, teamID, design.Samples, rankCounts)
			}
			productionEstimates = summarizePlainProductionCounts(counts, design.Samples, design.Work)
		}
		remainingWork -= design.Work
	}
	productionWork := design.Work
	if remainingWork < 0 {
		panic("rare-position production work budget exceeded")
	}
	for teamID, positions := range productionEstimates {
		for position, estimate := range positions {
			if !estimate.Available {
				continue
			}
			if search := teamSearches[teamID]; search != nil && position >= 0 && position < len(search.Positions) {
				copyEstimate := estimate
				search.Positions[position].ProductionEstimate = &copyEstimate
				search.Positions[position].ProductionWorkSpent = estimate.WorkSpent
				search.Positions[position].Status = StatusResolved
			}
			log.Printf("rare-position-production: group=%d design=%s team=%d position=%d samples=%d hits=%d p=%.8g se=%.3g relSE=%.3f ess=%.3f max_event_weight_share=%.3f mean_weight=%.6g work=%d meets_precision_goal=%t zero_hit_upper95=%.6g",
				group.Id, estimate.Design, teamID, position, estimate.Samples,
				estimate.Hits, estimate.Probability, estimate.StdErr, relativeSEValue(estimate.RelativeSE),
				estimate.ESS, estimate.MaxEventWeightShare, estimate.MeanWeight,
				estimate.WorkSpent, estimate.MeetsPrecisionGoal, estimate.ZeroHitUpper95)
		}
	}

	workSpent := scoutWork + adaptationWork + evaluationWork + productionWork
	if workSpent > totalWorkLimit || remainingWork < 0 || workSpent+remainingWork != totalWorkLimit {
		panic("rare-position total work budget exceeded or phase accounting is inconsistent")
	}
	searchOverhead := scoutWork + adaptationWork + evaluationWork
	log.Printf("rare-position-summary: parameterization=team_level group=%d scout_samples=%d scout_work=%d adaptation_work=%d adaptation_plain_mc_equiv=%.1f evaluation_work=%d evaluation_plain_mc_equiv=%.1f snapshots_retained=%d snapshots_evaluated=%d production_design=%s production_samples=%d production_work=%d fresh_final_estimator_samples=%d search_overhead_plain_mc_equiv=%.1f fresh_production_plain_mc_equiv=%.1f total_work_limit=%d work_spent=%d unused_work=%d scout_resolved_cells=%d cem_batches=%d candidates_admitted=%d exact_hit_targets=%d plain_P_selected=%t",
		group.Id, normalSamples, scoutWork, adaptationWork, float64(adaptationWork)/float64(plainWorkPerSample),
		evaluationWork, float64(evaluationWork)/float64(plainWorkPerSample), len(cemRound.Snapshots),
		len(evaluations), design.Kind, design.Samples, productionWork, design.Samples,
		float64(searchOverhead)/float64(plainWorkPerSample),
		float64(productionWork)/float64(plainWorkPerSample), totalWorkLimit,
		workSpent, remainingWork, scoutResolvedCells, cemRound.Iterations,
		cemRound.CandidatesAdmitted, cemRound.TargetsAnyExact, design.Kind == "plain_mc")
	return productionEstimates
}

func evaluationESS(evaluation *CEMProposalEvaluation) float64 {
	if evaluation == nil {
		return 0
	}
	return evaluation.ESS
}

func evaluationESSPerWork(evaluation *CEMProposalEvaluation) float64 {
	if evaluation == nil {
		return 0
	}
	return evaluation.ESSPerWork
}

func evaluationHits(evaluation *CEMProposalEvaluation) int {
	if evaluation == nil {
		return 0
	}
	return evaluation.Hits
}

func searchAndMergeRarePositionsLegacy(
	group *GroupType,
	campaign []*TeamCampaign,
	table *Table,
	sortOrder []SortType,
	normalPositionCounts map[int][]int,
	teamOdds []OddsType,
	normalSamples int,
) {
	rng := rand.New(rand.NewSource(time.Now().UnixNano()))
	unplayedGames := 0
	for _, game := range group.Games {
		if !game.Played {
			unplayedGames++
		}
	}
	numTeams := len(group.Team_groups)
	totalWorkLimit := calculateMaxRareWork(unplayedGames, numTeams)
	plainWorkPerSample := estimateSeasonWork(unplayedGames, 1, numTeams)
	scoutWork := int64(normalSamples) * plainWorkPerSample
	remainingWork := totalWorkLimit - scoutWork
	if remainingWork < 0 {
		panic("rare-position scout work exceeded total work budget")
	}
	cemWorkRemaining := int64(float64(totalWorkLimit) * MaxCEMWorkFraction)
	if cap := int64(MaxCEMPlainEquivalentSamples) * estimateSeasonWork(unplayedGames, 1, numTeams); cemWorkRemaining > cap {
		cemWorkRemaining = cap
	}
	cemExplorationWorkRemaining := int64(float64(cemWorkRemaining) * CEMMaxExplorationFraction)
	validationWorkRemaining := int64(float64(totalWorkLimit) * MaxCEMValidationWorkFraction)
	adaptWorkPerSample := estimateSeasonWork(unplayedGames, 1, numTeams)
	confirmationWorkRemaining := int64(float64(totalWorkLimit) * MaxCEMConfirmationWorkFraction)
	confirmationWorkPerSample := estimateSeasonWork(unplayedGames, 1, numTeams)
	workPerSample := estimateSeasonWork(unplayedGames, 2, numTeams)

	originalMeans := make([]GameProposalMeans, len(group.Games))
	for i, game := range group.Games {
		originalMeans[i] = GameProposalMeans{Home: game.HomePower, Away: game.AwayPower}
	}
	teamSearches := make(map[int]*TeamRareSearch, numTeams)
	scoutResolvedCells := 0
	for _, tg := range group.Team_groups {
		teamID := tg.Team_id
		index := table.Query(uint32(teamID))
		ts := initializeTeamRareSearch(teamID, normalPositionCounts[teamID],
			teamOdds[index].team.Pos, campaign, group.Team_groups, group.Games, table, sortOrder)
		teamSearches[teamID] = ts
		for _, state := range ts.Positions {
			if state.ObservedCount > 0 {
				scoutResolvedCells++
			}
		}
	}

	teamRareEstimates := make(map[int]map[int]RarePositionEstimate)
	var cemWork, confirmationWork, validationWork, productionWork, expansionWork int64
	var cemTargets, cemIterations, targetsAnyExact, targetsExactElite, targetsValidated int
	exactHitCandidateKeys := make(map[[2]int]bool)
	var cemCandidatesTotal, cemCandidatesAdmitted, cemCandidatesNotAdmitted int
	var cemExplorationSamples, cemAdaptiveSamples int
	var confirmationAttempts, confirmationSuccesses, confirmationFailures int
	var adaptationExactHitBatches, confirmationReadyCandidates int
	var confirmationSingleHitSuccesses, confirmationMultiHitSuccesses int
	var confirmationFailedResumed, confirmationFailedExhausted, confirmationHits int
	var confirmationChunks, confirmationSamples, confirmationInconclusiveBudget int
	var validationAttempts, validationSuccesses int
	var changedTeams, maxChangedTeams int
	var absThetaSum, maxAbsTheta, thetaDeltaL2Sum float64
	var thetaUpdates, thetaParameterCount int
	var schedulerBatches, candidatesOneBatch, candidatesMultiBatch, maxBatchesSingleCandidate int
	var stopValidationReady, stopStalled, stopLowEliteESS, stopRegression, stopPerCandidateCap, stopGlobalBudget int
	var bestCandidateTeam, bestCandidatePosition, bestCandidateBatches int
	var bestCandidateDistanceImprovement, bestCandidateNearTargetRate float64
	var highestNearTeam, highestNearPosition, highestNearBatches int
	var highestNearRate float64
	var validatedESS float64
	productionJobs := 0
	resolvedInitial := 0
	for round := 0; round <= 1; round++ {
		if round == 1 && (resolvedInitial == 0 ||
			cemWorkRemaining < int64(CEMMinBatchSamples)*adaptWorkPerSample) {
			break
		}
		// Discover once per round. Only observed and successfully resolved
		// positions seed the next frontier.
		candidates := collectFrontierCandidates(teamSearches)
		if len(candidates) == 0 {
			break
		}
		cemRound := runCEMRound(candidates, teamSearches, group, campaign, table,
			sortOrder, originalMeans, &cemWorkRemaining, &confirmationWorkRemaining,
			&validationWorkRemaining, &remainingWork, &cemExplorationWorkRemaining, adaptWorkPerSample,
			confirmationWorkPerSample, workPerSample, totalWorkLimit, rng)
		if remainingWork < 0 || cemWorkRemaining < 0 || confirmationWorkRemaining < 0 || validationWorkRemaining < 0 {
			panic("rare-position CEM, confirmation, or validation work budget exceeded")
		}
		cemWork += cemRound.CEMWork
		confirmationWork += cemRound.ConfirmationWork
		validationWork += cemRound.ValidationWork
		cemCandidatesTotal += cemRound.CandidatesTotal
		cemCandidatesAdmitted += cemRound.CandidatesAdmitted
		cemCandidatesNotAdmitted += cemRound.CandidatesNotAdmitted
		cemExplorationSamples += cemRound.ExplorationSamples
		cemAdaptiveSamples += cemRound.AdaptiveSamples
		confirmationAttempts += cemRound.ConfirmationAttempts
		confirmationSuccesses += cemRound.ConfirmationSuccesses
		confirmationFailures += cemRound.ConfirmationFailures
		adaptationExactHitBatches += cemRound.AdaptationExactHitBatches
		confirmationReadyCandidates += cemRound.ConfirmationReadyCandidates
		confirmationSingleHitSuccesses += cemRound.ConfirmationSingleHitSuccesses
		confirmationMultiHitSuccesses += cemRound.ConfirmationMultiHitSuccesses
		confirmationFailedResumed += cemRound.ConfirmationFailedResumed
		confirmationFailedExhausted += cemRound.ConfirmationFailedExhausted
		confirmationHits += cemRound.ConfirmationHits
		confirmationChunks += cemRound.ConfirmationChunks
		confirmationSamples += cemRound.ConfirmationSamples
		confirmationInconclusiveBudget += cemRound.ConfirmationInconclusiveBudget
		validationAttempts += cemRound.ValidationAttempts
		validationSuccesses += cemRound.ValidationSuccesses
		cemTargets += cemRound.TargetsAttempted
		cemIterations += cemRound.Iterations
		for _, state := range cemRound.Candidates {
			if state.EverHadExactHit {
				exactHitCandidateKeys[[2]int{state.Candidate.TeamID, state.Candidate.Position}] = true
			}
		}
		targetsAnyExact = len(exactHitCandidateKeys)
		targetsExactElite += cemRound.TargetsExactElite
		targetsValidated += cemRound.TargetsValidated
		changedTeams += cemRound.ChangedTeams
		if cemRound.MaxChangedTeams > maxChangedTeams {
			maxChangedTeams = cemRound.MaxChangedTeams
		}
		absThetaSum += cemRound.AbsThetaSum
		if cemRound.MaxAbsTheta > maxAbsTheta {
			maxAbsTheta = cemRound.MaxAbsTheta
		}
		thetaDeltaL2Sum += cemRound.ThetaDeltaL2Sum
		thetaUpdates += cemRound.ThetaUpdates
		thetaParameterCount += cemRound.ThetaParameterCount
		schedulerBatches += cemRound.SchedulerBatches
		candidatesOneBatch += cemRound.OneBatchCandidates
		candidatesMultiBatch += cemRound.MultiBatchCandidates
		if cemRound.MaxBatchesPerCandidate > maxBatchesSingleCandidate {
			maxBatchesSingleCandidate = cemRound.MaxBatchesPerCandidate
		}
		stopValidationReady += cemRound.StopValidationReady
		stopStalled += cemRound.StopStalled
		stopLowEliteESS += cemRound.StopLowEliteESS
		stopRegression += cemRound.StopRegression
		stopPerCandidateCap += cemRound.StopPerCandidateCap
		stopGlobalBudget += cemRound.StopGlobalBudget
		if cemRound.BestCandidateTeam != 0 &&
			cemRound.BestCandidateDistanceImprovement > bestCandidateDistanceImprovement {
			bestCandidateTeam = cemRound.BestCandidateTeam
			bestCandidatePosition = cemRound.BestCandidatePosition
			bestCandidateBatches = cemRound.BestCandidateBatches
			bestCandidateDistanceImprovement = cemRound.BestCandidateDistanceImprovement
			bestCandidateNearTargetRate = cemRound.BestCandidateNearTargetRate
		}
		if cemRound.HighestNearTeam != 0 && cemRound.HighestNearRate > highestNearRate {
			highestNearTeam = cemRound.HighestNearTeam
			highestNearPosition = cemRound.HighestNearPosition
			highestNearBatches = cemRound.HighestNearBatches
			highestNearRate = cemRound.HighestNearRate
		}
		validatedESS += cemRound.ValidatedESS
		if round == 1 {
			expansionWork += cemRound.CEMWork + cemRound.ValidationWork
		}

		eligible := cemRound.Eligible
		plans := buildProductionPlans(eligible, workPerSample, teamSearches)
		allocations := planProductionAllocations(plans, remainingWork, workPerSample)
		for _, allocation := range allocations {
			candidate := allocation.Plan.Candidate
			st := candidate.SearchState
			if st.Status != StatusPromising {
				continue
			}
			samples := affordableSamples(allocation.Samples, remainingWork, workPerSample)
			if samples <= 0 {
				break
			}
			work := int64(samples) * workPerSample
			predictedESS := allocation.Plan.Pilot.ESS * float64(work) /
				float64(allocation.Plan.Pilot.WorkSpent)
			if samples < allocation.Samples && predictedESS < MinUsableESS {
				continue
			}

			// Share the fixed run with other eligible positions of this team
			// in the current frontier, while keeping pilots out of estimates.
			var positions []int
			for _, other := range eligible {
				if other.TeamID == candidate.TeamID && other.SearchState.Status == StatusPromising {
					positions = append(positions, other.Position)
				}
			}
			if len(positions) == 0 {
				positions = []int{candidate.Position}
			}
			log.Printf("rare-position-production-plan: group=%d team=%d position=%d priority=%d critical100=%t projected_work=%d allocated_work=%d samples=%d predicted_ess=%.2f",
				group.Id, candidate.TeamID, candidate.Position, candidate.Priority,
				allocation.Plan.Critical100, allocation.Plan.ProjectedWork, work, samples, predictedESS)
			job := &RareSimulationJob{
				TeamID: candidate.TeamID, Direction: candidate.Direction,
				CandidatePositions: positions, Components: st.BestProposal.Components,
				Iterations: samples,
			}
			jobEstimates, _ := estimateRarePositionsForJob(
				campaign, group.Games, originalMeans, table, sortOrder, group.Team_groups,
				job, rng, group.Id,
			)
			remainingWork -= work
			productionJobs++
			if remainingWork < 0 {
				panic("rare-position production work budget exceeded")
			}
			productionWork += work
			if round == 1 {
				expansionWork += work
			}
			st.ProductionWorkSpent += work
			for position, est := range jobEstimates {
				positionState := teamSearches[candidate.TeamID].Positions[position]
				if est.Available {
					if est.Probability >= MinInterestingProbability {
						positionState.Status = StatusResolved
						positionState.ProductionEstimate = &ProductionEstimate{
							Probability: est.Probability, StdErr: est.StdErr,
							Samples: est.Samples, Hits: est.Hits, ESS: est.ESS,
							Available: true, MeetsPrecisionGoal: estimateMeetsPrecisionGoal(est.ESS, est.RelativeSE),
							RelativeSE: relativeSEPointer(est.RelativeSE), MaxEventWeightShare: est.MaxEventWeightShare,
							MeanWeight: est.MeanWeight, WorkSpent: work, Design: "importance_sampling",
						}
						if teamRareEstimates[candidate.TeamID] == nil {
							teamRareEstimates[candidate.TeamID] = make(map[int]RarePositionEstimate)
						}
						teamRareEstimates[candidate.TeamID][position] = est
						if round == 0 {
							resolvedInitial++
						}
					} else {
						positionState.Status = StatusBelowInterest
					}
				} else if position == candidate.Position {
					positionState.Status = StatusExhausted
				}
			}
		}
	}

	fallbackSamples, fallbackWork := planFallbackSamples(remainingWork, plainWorkPerSample)
	var newNonzeroCells, brokenHundreds int
	if fallbackSamples > 0 {
		fallbackCounts := simulatePlainRankCounts(campaign, group.Games, table, sortOrder,
			group.Team_groups, fallbackSamples, rng)
		newNonzeroCells, brokenHundreds = combineNormalPositionCounts(normalPositionCounts,
			fallbackCounts, normalSamples, fallbackSamples, teamOdds, table)
		remainingWork -= fallbackWork
	}
	if remainingWork < 0 {
		panic("rare-position fallback work budget exceeded")
	}
	log.Printf("rare-position-fallback-mc: group=%d samples=%d work=%d combined_normal_samples=%d new_nonzero_cells=%d broken_100_percent_results=%d",
		group.Id, fallbackSamples, fallbackWork, normalSamples+fallbackSamples,
		newNonzeroCells, brokenHundreds)
	for _, tg := range group.Team_groups {
		teamID := tg.Team_id
		if estimates := teamRareEstimates[teamID]; len(estimates) > 0 {
			index := table.Query(uint32(teamID))
			odds := teamOdds[index].team
			final := mergeRarePositionEstimates(odds.Pos, normalPositionCounts[teamID], estimates)
			copy(odds.Pos, final)
		}
	}
	resolvedPositions := 0
	for _, search := range teamSearches {
		for _, state := range search.Positions {
			if state.Status == StatusResolved {
				resolvedPositions++
			}
		}
	}
	workSpent := totalWorkLimit - remainingWork
	if workSpent > totalWorkLimit {
		panic("rare-position total work budget exceeded")
	}
	accountedWork := scoutWork + cemWork + confirmationWork + validationWork + productionWork + fallbackWork
	if accountedWork > totalWorkLimit || workSpent < accountedWork {
		panic("rare-position phase work accounting exceeded total budget or omitted tracked work")
	}
	combinedCEMValidationEquivalent := float64(cemWork+validationWork) / float64(plainWorkPerSample)
	validatedESSPerPlainEquivalent := 0.0
	if validationWork > 0 {
		validatedESSPerPlainEquivalent = validatedESS /
			(float64(validationWork) / float64(plainWorkPerSample))
	}
	averageChangedTeams := 0.0
	averageThetaL1Norm, averageAbsTeamTheta := cemThetaSummary(absThetaSum, thetaUpdates, thetaParameterCount)
	averageThetaDeltaL2 := 0.0
	if cemIterations > 0 {
		averageChangedTeams = float64(changedTeams) / float64(cemIterations)
		averageThetaDeltaL2 = thetaDeltaL2Sum / float64(cemIterations)
	}
	averageBatchesPerCandidate := 0.0
	if cemTargets > 0 {
		averageBatchesPerCandidate = float64(cemIterations) / float64(cemTargets)
	}
	log.Printf("rare-position-summary: parameterization=team_level group=%d scout_sims=%d scout_work=%d fallback_normal_sims=%d normal_sims_total=%d total_work_limit=%d cem_work=%d validation_work=%d production_work=%d expansion_work=%d fallback_work=%d unused_work=%d work_spent=%d resolved_positions_is=%d scout_resolved_cells=%d new_cells_from_fallback=%d cem_targets_attempted=%d cem_iterations=%d cem_scheduler_batches=%d cem_candidates_receiving_1_batch=%d cem_candidates_receiving_2plus_batches=%d max_batches_single_candidate=%d average_batches_per_candidate=%.2f best_candidate_team=%d best_candidate_position=%d best_candidate_batches=%d best_candidate_distance_improvement=%.3f best_candidate_near_target_rate=%.4f highest_near_team=%d highest_near_position=%d highest_near_batches=%d highest_near_target_rate=%.4f stop_validation_ready=%d stop_stalled=%d stop_low_elite_ess=%d stop_regression=%d stop_per_candidate_cap=%d stop_global_budget_exhausted=%d average_changed_teams=%.2f max_changed_teams=%d average_theta_l1_norm=%.4f average_abs_team_theta=%.4f max_abs_team_theta=%.4f average_theta_delta_l2=%.4f targets_with_any_exact_hit=%d targets_reaching_exact_elite_threshold=%d targets_passing_validation=%d production_jobs=%d cem_plain_mc_equiv=%.1f validation_plain_mc_equiv=%.1f cem_and_validation_plain_mc_equiv=%.1f validated_ess_per_plain_mc_equiv=%.4f production_plain_mc_equiv=%.1f heuristic_baseline_group16982_plain_mc_equiv=8499.4",
		group.Id, normalSamples, scoutWork, fallbackSamples, normalSamples+fallbackSamples, totalWorkLimit,
		cemWork, validationWork, productionWork, expansionWork, fallbackWork, remainingWork,
		workSpent, resolvedPositions, scoutResolvedCells, newNonzeroCells, cemTargets, cemIterations,
		schedulerBatches, candidatesOneBatch, candidatesMultiBatch, maxBatchesSingleCandidate,
		averageBatchesPerCandidate, bestCandidateTeam, bestCandidatePosition, bestCandidateBatches,
		bestCandidateDistanceImprovement, bestCandidateNearTargetRate, highestNearTeam,
		highestNearPosition, highestNearBatches, highestNearRate, stopValidationReady,
		stopStalled, stopLowEliteESS, stopRegression, stopPerCandidateCap, stopGlobalBudget,
		averageChangedTeams, maxChangedTeams, averageThetaL1Norm, averageAbsTeamTheta,
		maxAbsTheta, averageThetaDeltaL2,
		targetsAnyExact, targetsExactElite,
		targetsValidated, productionJobs, float64(cemWork)/float64(plainWorkPerSample),
		float64(validationWork)/float64(plainWorkPerSample),
		combinedCEMValidationEquivalent, validatedESSPerPlainEquivalent,
		float64(productionWork)/float64(plainWorkPerSample))
	log.Printf("rare-position-cem-summary: group=%d cem_candidates_total=%d cem_candidates_admitted=%d cem_candidates_not_admitted=%d cem_exploration_samples=%d cem_adaptive_samples=%d max_batches_single_candidate=%d adaptation_exact_hit_batches=%d confirmation_ready_candidates=%d confirmation_attempts=%d confirmation_chunks=%d confirmation_samples=%d confirmation_hits=%d confirmation_successes=%d confirmation_single_hit_successes=%d confirmation_multi_hit_successes=%d confirmation_failures=%d confirmation_failed_resumed=%d confirmation_failed_exhausted=%d confirmation_inconclusive_budget=%d confirmation_work=%d confirmation_plain_mc_equiv=%.1f validation_attempts=%d validation_successes=%d validation_work=%d cem_confirmation_validation_plain_mc_equiv=%.1f confirmation_work_per_sample=%d validation_work_per_sample=%d total_work_limit=%d work_spent=%d",
		group.Id, cemCandidatesTotal, cemCandidatesAdmitted, cemCandidatesNotAdmitted,
		cemExplorationSamples, cemAdaptiveSamples, maxBatchesSingleCandidate,
		adaptationExactHitBatches, confirmationReadyCandidates, confirmationAttempts,
		confirmationChunks, confirmationSamples, confirmationHits, confirmationSuccesses, confirmationSingleHitSuccesses,
		confirmationMultiHitSuccesses, confirmationFailures, confirmationFailedResumed,
		confirmationFailedExhausted, confirmationInconclusiveBudget,
		confirmationWork, float64(confirmationWork)/float64(plainWorkPerSample),
		validationAttempts, validationSuccesses, validationWork,
		float64(cemWork+confirmationWork+validationWork)/float64(plainWorkPerSample),
		confirmationWorkPerSample, workPerSample, totalWorkLimit, workSpent)
}

type RarePositionEstimate struct {
	Probability         float64
	StdErr              float64
	Samples             int
	Hits                int
	ESS                 float64
	MeanWeight          float64
	Available           bool
	MeetsPrecisionGoal  bool
	RelativeSE          float64
	MaxEventWeightShare float64
	ZeroHitUpper95      float64
	WorkSpent           int64
	Found               bool // legacy quality gate; never used by active production path
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
	for pos, est := range rareEstimates {
		if pos >= 0 && pos < len(normalCounts) && normalCounts[pos] == 0 &&
			est.Found && est.Probability >= MinInterestingProbability {
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
		if normalCounts[pos] == 0 && isRare && est.Found && est.Probability >= MinInterestingProbability {
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
		est.Probability <= 0 ||
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
	sumWeight := make(map[int]float64)
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
		sumWeight[job.TeamID] += w

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

		pHat, stdErr, ess := eventEstimateStats(sY, sY2, totalN)

		est := RarePositionEstimate{
			Probability:    pHat,
			StdErr:         stdErr,
			Samples:        totalN,
			Hits:           h,
			ESS:            ess,
			Found:          false,
			Available:      true,
			MeanWeight:     sumWeight[job.TeamID] / N,
			ZeroHitUpper95: 0,
		}
		est.RelativeSE = math.Inf(1)
		if pHat > 0 {
			est.RelativeSE = stdErr / pHat
		}

		if h > 0 {
			posWeights := eventWeights[pos]
			sort.Float64s(posWeights)
			minW := posWeights[0]
			maxW := posWeights[len(posWeights)-1]
			medW := posWeights[len(posWeights)/2]
			p90W := posWeights[int(float64(len(posWeights))*0.90)]
			p99W := posWeights[int(float64(len(posWeights))*0.99)]
			maxShare := 0.0
			if sY > 0 {
				maxShare = maxW / sY
			}
			est.MaxEventWeightShare = maxShare

			log.Printf("rare-position-weights: group=%d team=%d pos=%d hits=%d min_w=%.3e median_w=%.3e p90_w=%.3e p99_w=%.3e max_w=%.3e max_event_weight_share=%.3f",
				groupID, job.TeamID, pos, h, minW, medW, p90W, p99W, maxW, maxShare)
		} else {
			est.ZeroHitUpper95 = weightedZeroHitUpper95(totalN, 1/OriginalMixtureWeight)
		}
		est.MeetsPrecisionGoal = estimateMeetsPrecisionGoal(ess, est.RelativeSE)
		est.Found = rareEstimateUsable(est) // legacy compatibility only

		results[pos] = est
		relSE := math.Inf(1)
		if pHat > 0 {
			relSE = stdErr / pHat
		}
		naiveEquivalentSamples := 0.0
		if est.Available && pHat > 0 {
			naiveEquivalentSamples = ess / pHat
		}
		log.Printf("rare-position-job: group=%d team=%d pos=%d direction=%d samples=%d hits=%d p=%.7g se=%.3e relSE=%.3f ess=%.1f mean_weight=%.6g naive_equiv_sims=%.0f available=%t meets_precision_goal=%t",
			groupID, job.TeamID, pos, job.Direction, totalN, h, pHat, stdErr, relSE, ess,
			est.MeanWeight, naiveEquivalentSamples, est.Available, est.MeetsPrecisionGoal)
	}

	for i, comp := range job.Components {
		log.Printf("rare-position-component-hist: group=%d team=%d component=%s weight=%.2f target_rank=%d samples=%d hist=%v",
			groupID, job.TeamID, comp.Name, comp.Weight, comp.TargetRank,
			componentSampleCounts[i], componentRankHists[i])
	}

	return results, totalN
}
