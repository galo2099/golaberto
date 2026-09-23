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
	NormalIterations          = 10000
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
	Spec          SparseProposalSpec
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
	BestPilot           *WeightedPilotResult
	Pilots              []*WeightedPilotResult
	ProductionEstimate  *ProductionEstimate
	SearchWorkSpent     int64
	ProductionWorkSpent int64
	AdaptiveSteps       int
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
		Spec:          spec,
		Components:    comps,
	}
}

const (
	RefinedPilotSamplesPerLevel = 800
	MinPilotSamples             = 100
	AdaptivePilotChunk          = 100
	MinAdaptivePilotSamples     = 50
	RankProgressWindow          = 2
	MinPilotHitsForProduction   = 3
	MinPilotESSForProduction    = 2.0
	TargetProductionESS         = 15.0
	ProductionWorkSafetyFactor  = 1.25
	MaxInitialPilotWorkFraction = 0.10
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

func shortlistPilotMixtures(results []*WeightedPilotResult) []*WeightedPilotResult {
	var shortlist []*WeightedPilotResult
	for _, result := range results {
		if result.SumY > 0 {
			shortlist = append(shortlist, result)
		}
	}
	sort.Slice(shortlist, func(i, j int) bool { return pilotBetter(shortlist[i], shortlist[j]) })
	if len(shortlist) > 2 {
		shortlist = shortlist[:2]
	}
	return shortlist
}

type ProposalKey struct {
	TargetGameLimit         int
	BoundaryCompetitorLimit int
	CompetitorGameLimit     int
	StrengthLevel           int
}

func proposalKey(spec SparseProposalSpec) ProposalKey {
	return ProposalKey{spec.TargetGameLimit, spec.BoundaryCompetitorLimit,
		spec.CompetitorGameLimit, spec.StrengthLevel}
}

type PilotProgress struct {
	MeanRank             float64
	TargetDistance       float64
	DirectionalTailRate  float64
	ProgressFromBaseline float64
	ProgressPerWork      float64
}

func rankSummary(hist []int, targetRank int, direction RareDirection) (mean, tail float64) {
	total := 0
	for rank, count := range hist {
		total += count
		mean += float64(rank * count)
		if direction == RareBetter && rank <= targetRank+RankProgressWindow ||
			direction == RareWorse && rank >= targetRank-RankProgressWindow {
			tail += float64(count)
		}
	}
	if total > 0 {
		mean /= float64(total)
		tail /= float64(total)
	}
	return mean, tail
}

func calculatePilotProgress(pilot *WeightedPilotResult, baselineHist []int,
	targetRank int, direction RareDirection) PilotProgress {
	mean, tail := rankSummary(pilot.RankHistogram, targetRank, direction)
	baselineMean, baselineTail := rankSummary(baselineHist, targetRank, direction)
	directionalMovement := baselineMean - mean
	if direction == RareWorse {
		directionalMovement = mean - baselineMean
	}
	// Rank mean drives zero-hit search; a bounded tail bonus captures movement
	// in multimodal rank distributions. Neither quantity estimates event P.
	progress := directionalMovement + 2*(tail-baselineTail)
	result := PilotProgress{MeanRank: mean, TargetDistance: math.Abs(mean - float64(targetRank)),
		DirectionalTailRate: tail, ProgressFromBaseline: progress}
	if pilot.WorkSpent > 0 {
		result.ProgressPerWork = progress / float64(pilot.WorkSpent)
	}
	return result
}

func adaptivePilotBetter(a, b *WeightedPilotResult, baselineHist []int,
	targetRank int, direction RareDirection) bool {
	if a.Hits > 0 || b.Hits > 0 {
		if a.Hits == 0 || b.Hits == 0 {
			return a.Hits > 0
		}
		if pilotBetter(a, b) {
			return true
		}
		if pilotBetter(b, a) {
			return false
		}
	} else {
		ap := calculatePilotProgress(a, baselineHist, targetRank, direction)
		bp := calculatePilotProgress(b, baselineHist, targetRank, direction)
		if ap.ProgressPerWork != bp.ProgressPerWork {
			return ap.ProgressPerWork > bp.ProgressPerWork
		}
		if ap.DirectionalTailRate != bp.DirectionalTailRate {
			return ap.DirectionalTailRate > bp.DirectionalTailRate
		}
	}
	return a.Proposal.Name < b.Proposal.Name
}

func adaptiveNeighborImproves(next, current *WeightedPilotResult, baselineHist []int,
	targetRank int, direction RareDirection) bool {
	if next.Hits > 0 || current.Hits > 0 {
		return next.Hits > 0 && (current.Hits == 0 || pilotBetter(next, current))
	}
	nextProgress := calculatePilotProgress(next, baselineHist, targetRank, direction)
	currentProgress := calculatePilotProgress(current, baselineHist, targetRank, direction)
	return nextProgress.ProgressPerWork > currentProgress.ProgressPerWork+1e-12 &&
		nextProgress.ProgressFromBaseline > currentProgress.ProgressFromBaseline
}

func adaptiveNeighbors(spec SparseProposalSpec, remainingTargetGames int) []SparseProposalSpec {
	var neighbors []SparseProposalSpec
	if spec.TargetGameLimit != -1 && spec.TargetGameLimit < remainingTargetGames {
		next := spec
		switch spec.TargetGameLimit {
		case 1:
			next.TargetGameLimit = 2
		case 2:
			next.TargetGameLimit = 4
		case 4:
			next.TargetGameLimit = 8
		default:
			next.TargetGameLimit = -1
		}
		neighbors = append(neighbors, next)
	}
	if spec.StrengthLevel < 5 {
		next := spec
		next.StrengthLevel++
		neighbors = append(neighbors, next)
	}
	next := spec
	switch {
	case spec.BoundaryCompetitorLimit == 0:
		next.BoundaryCompetitorLimit, next.CompetitorGameLimit = 1, 1
	case spec.BoundaryCompetitorLimit == 1:
		next.BoundaryCompetitorLimit = 2
	case spec.CompetitorGameLimit == 1:
		next.CompetitorGameLimit = 2
	case spec.CompetitorGameLimit == 2:
		next.CompetitorGameLimit = 4
	}
	if proposalKey(next) != proposalKey(spec) {
		neighbors = append(neighbors, next)
	}
	return neighbors
}

func unseenAdaptiveNeighbors(spec SparseProposalSpec, remainingTargetGames int,
	seen map[ProposalKey]bool) []SparseProposalSpec {
	var unseen []SparseProposalSpec
	for _, neighbor := range adaptiveNeighbors(spec, remainingTargetGames) {
		if !seen[proposalKey(neighbor)] {
			unseen = append(unseen, neighbor)
		}
	}
	return unseen
}

func selectAdaptiveNeighbor(current *WeightedPilotResult, neighbors []*WeightedPilotResult,
	baselineHist []int, targetRank int, direction RareDirection) *WeightedPilotResult {
	var best *WeightedPilotResult
	for _, neighbor := range neighbors {
		if best == nil || adaptivePilotBetter(neighbor, best, baselineHist, targetRank, direction) {
			best = neighbor
		}
	}
	if best == nil || !adaptiveNeighborImproves(best, current, baselineHist, targetRank, direction) {
		return nil
	}
	return best
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

func runWeightedPilotRound(
	candidates []*FrontierCandidate,
	group *GroupType,
	campaign []*TeamCampaign,
	table *Table,
	sortOrder []SortType,
	originalMeans []GameProposalMeans,
	normalMeanRanks map[int]float64,
	normalPositionCounts map[int][]int,
	workPerSample int64,
	remainingWork, pilotWorkRemaining *int64,
	rng *rand.Rand,
) ([]*FrontierCandidate, int64) {
	if len(candidates) == 0 || workPerSample <= 0 {
		return nil, 0
	}
	pilotBudget := *pilotWorkRemaining
	if *remainingWork < pilotBudget {
		pilotBudget = *remainingWork
	}
	if pilotBudget < int64(MinAdaptivePilotSamples)*workPerSample {
		return nil, 0
	}
	// The first sweep is one baseline per cell. Cap it at 55% of pilot work,
	// leaving explicit room for escalation and weighted refinement (15%).
	initialBudget := pilotBudget * 55 / 100
	refinementReserve := pilotBudget * 15 / 100
	baselineSamples := affordableSamples(AdaptivePilotChunk, initialBudget,
		int64(len(candidates))*workPerSample)
	if baselineSamples < MinAdaptivePilotSamples {
		baselineSamples = MinAdaptivePilotSamples
		slots := int(initialBudget / (int64(baselineSamples) * workPerSample))
		if slots < len(candidates) {
			for _, skipped := range candidates[slots:] {
				skipped.SearchState.Status = StatusUnexplored
			}
			candidates = candidates[:slots]
		}
	}
	if len(candidates) == 0 {
		return nil, 0
	}
	type adaptiveState struct {
		candidate   *FrontierCandidate
		current     *WeightedPilotResult
		seen        map[ProposalKey]bool
		active      bool
		targetGames int
	}
	results := make(map[*FrontierCandidate][]*WeightedPilotResult, len(candidates))
	states := make([]*adaptiveState, 0, len(candidates))
	var roundWork int64
	runPilot := func(candidate *FrontierCandidate, spec SparseProposalSpec, samples int) *WeightedPilotResult {
		pilot := &WeightedPilotResult{Proposal: buildSearchProposalForSpec(
			group.Games, spec, normalMeanRanks, originalMeans,
		)}
		evaluateWeightedPilot(pilot, campaign, group.Games, originalMeans, table, sortOrder,
			group.Team_groups, candidate.TeamID, candidate.Position, samples,
			workPerSample, rng, group.Id)
		*remainingWork -= pilot.WorkSpent
		*pilotWorkRemaining -= pilot.WorkSpent
		roundWork += pilot.WorkSpent
		candidate.SearchState.SearchWorkSpent += pilot.WorkSpent
		results[candidate] = append(results[candidate], pilot)
		progress := calculatePilotProgress(pilot, normalPositionCounts[candidate.TeamID],
			candidate.Position, candidate.Direction)
		log.Printf("rare-position-adaptive-step: group=%d team=%d position=%d step=%d spec=%s samples=%d hits=%d mean_rank=%.3f target_distance=%.3f tail_rate=%.4f progress=%.4f progress_per_million_work=%.4f ess=%.2f",
			group.Id, candidate.TeamID, candidate.Position, candidate.SearchState.AdaptiveSteps,
			pilot.Proposal.Name, samples, pilot.Hits, progress.MeanRank, progress.TargetDistance,
			progress.DirectionalTailRate, progress.ProgressFromBaseline,
			progress.ProgressPerWork*1e6, pilot.ESS)
		return pilot
	}
	// Fair baseline before any adaptive work or production allocation.
	for _, candidate := range candidates {
		spec := SparseProposalSpec{TargetTeamID: candidate.TeamID, TargetRank: candidate.Position,
			Direction: candidate.Direction, StrengthLevel: 2, TargetGameLimit: 2}
		pilot := runPilot(candidate, spec, baselineSamples)
		targetGames := 0
		for _, game := range group.Games {
			if !game.Played && (game.HomeId == candidate.TeamID || game.AwayId == candidate.TeamID) {
				targetGames++
			}
		}
		states = append(states, &adaptiveState{candidate: candidate, current: pilot,
			seen: map[ProposalKey]bool{proposalKey(spec): true}, active: pilot.Hits == 0,
			targetGames: targetGames})
	}
	// Equal baselines followed by bounded, globally prioritized local walks.
	explorationRemaining := pilotBudget - refinementReserve - roundWork
	perCandidateCap := 4 * pilotBudget / int64(len(candidates))
	if perCandidateCap > pilotBudget*60/100 {
		perCandidateCap = pilotBudget * 60 / 100
	}
	log.Printf("rare-position-adaptive-budget: group=%d candidates=%d pilot_budget=%d initial_work_limit=%d initial_samples_per_candidate=%d exploration_work_available=%d refinement_reserve=%d per_candidate_exploration_cap=%d",
		group.Id, len(candidates), pilotBudget, initialBudget, baselineSamples,
		explorationRemaining, refinementReserve, perCandidateCap)
	for explorationRemaining >= int64(MinAdaptivePilotSamples)*workPerSample {
		var chosen *adaptiveState
		for _, state := range states {
			if !state.active || state.candidate.SearchState.SearchWorkSpent+
				int64(MinAdaptivePilotSamples)*workPerSample > perCandidateCap {
				continue
			}
			if chosen == nil {
				chosen = state
				continue
			}
			a, b := state.candidate, chosen.candidate
			criticalA, criticalB := a.Priority >= 100, b.Priority >= 100
			if criticalA != criticalB {
				if criticalA {
					chosen = state
				}
				continue
			}
			progressA := calculatePilotProgress(state.current, normalPositionCounts[a.TeamID],
				a.Position, a.Direction)
			progressB := calculatePilotProgress(chosen.current, normalPositionCounts[b.TeamID],
				b.Position, b.Direction)
			if progressA.ProgressPerWork > progressB.ProgressPerWork ||
				(progressA.ProgressPerWork == progressB.ProgressPerWork && a.Priority > b.Priority) {
				chosen = state
			}
		}
		if chosen == nil {
			break
		}
		candidate := chosen.candidate
		current := chosen.current
		var neighbors []*WeightedPilotResult
		for _, spec := range unseenAdaptiveNeighbors(current.Proposal.Spec, chosen.targetGames, chosen.seen) {
			key := proposalKey(spec)
			available := explorationRemaining
			if capLeft := perCandidateCap - candidate.SearchState.SearchWorkSpent; capLeft < available {
				available = capLeft
			}
			samples := affordableSamples(AdaptivePilotChunk, available, workPerSample)
			samples = affordableSamples(samples, *remainingWork, workPerSample)
			if samples < MinAdaptivePilotSamples {
				break
			}
			chosen.seen[key] = true
			pilot := runPilot(candidate, spec, samples)
			neighbors = append(neighbors, pilot)
			explorationRemaining -= pilot.WorkSpent
			progress := calculatePilotProgress(pilot, normalPositionCounts[candidate.TeamID],
				candidate.Position, candidate.Direction)
			log.Printf("rare-position-adaptive-neighbor: group=%d team=%d position=%d from=%s candidate=%s hits=%d progress_per_million_work=%.4f ess_per_million_work=%.4f",
				group.Id, candidate.TeamID, candidate.Position, current.Proposal.Name,
				pilot.Proposal.Name, pilot.Hits, progress.ProgressPerWork*1e6,
				pilot.ESSPerWork*1e6)
		}
		bestNeighbor := selectAdaptiveNeighbor(current, neighbors,
			normalPositionCounts[candidate.TeamID], candidate.Position, candidate.Direction)
		if len(neighbors) == 0 {
			chosen.active = false
			continue
		}
		candidate.SearchState.AdaptiveSteps++
		if bestNeighbor == nil {
			chosen.active = false
			log.Printf("rare-position-adaptive-select: group=%d team=%d position=%d selected=%s reason=no_improving_neighbor",
				group.Id, candidate.TeamID, candidate.Position, current.Proposal.Name)
			continue
		}
		chosen.current = bestNeighbor
		reason := "best_rank_progress"
		if bestNeighbor.Hits > 0 {
			reason = "weighted_ess_per_work"
			chosen.active = false
		}
		log.Printf("rare-position-adaptive-select: group=%d team=%d position=%d selected=%s reason=%s",
			group.Id, candidate.TeamID, candidate.Position, bestNeighbor.Proposal.Name, reason)
	}

	type refinement struct {
		candidate *FrontierCandidate
		pilot     *WeightedPilotResult
	}
	var refinements []refinement
	for _, candidate := range candidates {
		candidate.SearchState.Pilots = results[candidate]
		if len(results[candidate]) == 0 {
			candidate.SearchState.Status = StatusExhausted
			continue
		}
		for _, pilot := range shortlistPilotMixtures(results[candidate]) {
			refinements = append(refinements, refinement{candidate, pilot})
		}
	}
	sort.Slice(refinements, func(i, j int) bool {
		a, b := refinements[i], refinements[j]
		if pilotBetter(a.pilot, b.pilot) {
			return true
		}
		if pilotBetter(b.pilot, a.pilot) {
			return false
		}
		if a.candidate.Priority != b.candidate.Priority {
			return a.candidate.Priority > b.candidate.Priority
		}
		if a.candidate.TeamID != b.candidate.TeamID {
			return a.candidate.TeamID < b.candidate.TeamID
		}
		return a.candidate.Position < b.candidate.Position
	})
	for _, item := range refinements {
		requested := RefinedPilotSamplesPerLevel - item.pilot.Samples
		samples := affordableSamples(requested, *remainingWork, workPerSample)
		samples = affordableSamples(samples, *pilotWorkRemaining, workPerSample)
		if samples < MinPilotSamples {
			break
		}
		before := item.pilot.WorkSpent
		item.pilot.Refined = true
		evaluateWeightedPilot(item.pilot, campaign, group.Games, originalMeans, table, sortOrder,
			group.Team_groups, item.candidate.TeamID, item.candidate.Position, samples,
			workPerSample, rng, group.Id)
		spent := item.pilot.WorkSpent - before
		*remainingWork -= spent
		*pilotWorkRemaining -= spent
		roundWork += spent
		item.candidate.SearchState.SearchWorkSpent += spent
	}

	var eligible []*FrontierCandidate
	for _, candidate := range candidates {
		best := selectPilotMixture(results[candidate])
		if best == nil {
			candidate.SearchState.Status = StatusExhausted
			log.Printf("rare-position-pilot-selection: group=%d team=%d position=%d selected_spec=none reason=insufficient_weighted_evidence search_work=%d",
				group.Id, candidate.TeamID, candidate.Position, candidate.SearchState.SearchWorkSpent)
			continue
		}
		candidate.SearchState.Status = StatusPromising
		candidate.SearchState.BestProposal = &best.Proposal
		candidate.SearchState.BestPilot = best
		eligible = append(eligible, candidate)
		projected := projectedWorkForESS(best)
		log.Printf("rare-position-pilot-selection: group=%d team=%d position=%d selected_spec=%s selected_level=%d pilot_samples=%d pilot_hits=%d pilot_ess=%.2f pilot_ess_per_million_work=%.3f max_weight_share=%.3f search_work=%d projected_work_ess15=%d projected_samples=%d",
			group.Id, candidate.TeamID, candidate.Position, best.Proposal.Name, best.Proposal.StrengthLevel,
			best.Samples, best.Hits, best.ESS, best.ESSPerWork*1e6, best.MaxEventWeightShare,
			candidate.SearchState.SearchWorkSpent, projected, (projected+workPerSample-1)/workPerSample)
	}
	return eligible, roundWork
}

func shouldRunFrontierRound(round, resolvedInitial int, pilotWork, remainingWork, workPerSample int64) bool {
	if round == 0 {
		return true
	}
	// A new round needs at least one baseline and some room for escalation.
	minimumRoundWork := int64(2*AdaptivePilotChunk) * workPerSample
	return round == 1 && resolvedInitial > 0 &&
		pilotWork >= minimumRoundWork && remainingWork >= minimumRoundWork
}

func planFallbackSamples(remainingWork, plainWorkPerSample int64) (int, int64) {
	if remainingWork <= 0 || plainWorkPerSample <= 0 {
		return 0, 0
	}
	samples := int(remainingWork / plainWorkPerSample)
	return samples, int64(samples) * plainWorkPerSample
}

// Unlike IS, these are independent draws directly from P and can be added
// count-for-count to the initial ordinary Monte Carlo histogram.
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
	remainingWork := totalWorkLimit
	pilotWorkRemaining := int64(float64(totalWorkLimit) * MaxInitialPilotWorkFraction)
	workPerSample := estimateSeasonWork(unplayedGames, 4, numTeams)

	originalMeans := make([]GameProposalMeans, len(group.Games))
	for i, game := range group.Games {
		originalMeans[i] = GameProposalMeans{Home: game.HomePower, Away: game.AwayPower}
	}
	normalMeanRanks := make(map[int]float64, numTeams)
	teamSearches := make(map[int]*TeamRareSearch, numTeams)
	for _, tg := range group.Team_groups {
		teamID := tg.Team_id
		index := table.Query(uint32(teamID))
		ts := initializeTeamRareSearch(teamID, normalPositionCounts[teamID],
			teamOdds[index].team.Pos, campaign, group.Team_groups, group.Games, table, sortOrder)
		teamSearches[teamID] = ts
		normalMeanRanks[teamID] = ts.NormalMeanRank
	}

	teamRareEstimates := make(map[int]map[int]RarePositionEstimate)
	var pilotWork, productionWork, expansionWork int64
	productionJobs := 0
	resolvedInitial := 0
	for round := 0; shouldRunFrontierRound(round, resolvedInitial,
		pilotWorkRemaining, remainingWork, workPerSample); round++ {
		// Discover once per round. Only observed and successfully resolved
		// positions seed the next frontier.
		candidates := collectFrontierCandidates(teamSearches)
		if len(candidates) == 0 {
			break
		}
		eligible, roundPilotWork := runWeightedPilotRound(
			candidates, group, campaign, table, sortOrder, originalMeans,
			normalMeanRanks, normalPositionCounts, workPerSample, &remainingWork, &pilotWorkRemaining, rng,
		)
		if remainingWork < 0 || pilotWorkRemaining < 0 {
			panic("rare-position pilot work budget exceeded")
		}
		pilotWork += roundPilotWork
		if round == 1 {
			expansionWork += roundPilotWork
		}

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
				if est.Found {
					if est.Probability >= MinInterestingProbability {
						positionState.Status = StatusResolved
						positionState.ProductionEstimate = &ProductionEstimate{
							Probability: est.Probability, StdErr: est.StdErr,
							Samples: est.Samples, Hits: est.Hits, ESS: est.ESS,
							Found: true, WorkSpent: work,
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

	plainWorkPerSample := estimateSeasonWork(unplayedGames, 1, numTeams)
	fallbackSamples, fallbackWork := planFallbackSamples(remainingWork, plainWorkPerSample)
	var newNonzeroCells, brokenHundreds int
	if fallbackSamples > 0 {
		fallbackCounts := simulatePlainRankCounts(campaign, group.Games, table, sortOrder,
			group.Team_groups, fallbackSamples, rng)
		newNonzeroCells, brokenHundreds = combineNormalPositionCounts(normalPositionCounts,
			fallbackCounts, NormalIterations, fallbackSamples, teamOdds, table)
		remainingWork -= fallbackWork
	}
	if remainingWork < 0 {
		panic("rare-position fallback work budget exceeded")
	}
	log.Printf("rare-position-fallback-mc: group=%d samples=%d work=%d combined_normal_samples=%d new_nonzero_cells=%d broken_100_percent_results=%d",
		group.Id, fallbackSamples, fallbackWork, NormalIterations+fallbackSamples,
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
	adaptiveCandidates, adaptiveSteps, reachingTarget, reachingPilotESS := 0, 0, 0, 0
	for _, search := range teamSearches {
		for _, state := range search.Positions {
			if state.Status == StatusResolved {
				resolvedPositions++
			}
			if len(state.Pilots) > 0 {
				adaptiveCandidates++
				adaptiveSteps += state.AdaptiveSteps
				hit, enoughESS := false, false
				for _, pilot := range state.Pilots {
					hit = hit || pilot.Hits > 0
					enoughESS = enoughESS || pilot.ESS >= MinPilotESSForProduction
				}
				if hit {
					reachingTarget++
				}
				if enoughESS {
					reachingPilotESS++
				}
			}
		}
	}
	workSpent := totalWorkLimit - remainingWork
	if workSpent > totalWorkLimit {
		panic("rare-position total work budget exceeded")
	}
	log.Printf("rare-position-summary: group=%d normal_sims_initial=%d fallback_normal_sims=%d normal_sims_total=%d total_work_limit=%d pilot_work=%d production_work=%d expansion_work=%d fallback_work=%d unused_work=%d work_spent=%d resolved_positions_is=%d new_cells_from_fallback=%d adaptive_candidates=%d adaptive_steps=%d candidates_reaching_target=%d candidates_reaching_min_pilot_ess=%d production_jobs=%d pilot_plain_mc_equiv=%.1f production_plain_mc_equiv=%.1f",
		group.Id, NormalIterations, fallbackSamples, NormalIterations+fallbackSamples, totalWorkLimit,
		pilotWork, productionWork, expansionWork, fallbackWork, remainingWork, workSpent,
		resolvedPositions, newNonzeroCells, adaptiveCandidates, adaptiveSteps, reachingTarget,
		reachingPilotESS, productionJobs, float64(pilotWork)/float64(plainWorkPerSample),
		float64(productionWork)/float64(plainWorkPerSample))
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
			maxShare := 0.0
			if sY > 0 {
				maxShare = maxW / sY
			}

			log.Printf("rare-position-weights: group=%d team=%d pos=%d hits=%d min_w=%.3e median_w=%.3e p90_w=%.3e p99_w=%.3e max_w=%.3e max_event_weight_share=%.3f",
				groupID, job.TeamID, pos, h, minW, medW, p90W, p99W, maxW, maxShare)
		}

		results[pos] = est
		relSE := math.Inf(1)
		if pHat > 0 {
			relSE = stdErr / pHat
		}
		naiveEquivalentSamples := 0.0
		if est.Found && pHat > 0 {
			naiveEquivalentSamples = ess / pHat
		}
		log.Printf("rare-position-job: group=%d team=%d pos=%d direction=%d samples=%d hits=%d p=%.7g se=%.3e relSE=%.3f ess=%.1f naive_equiv_sims=%.0f found=%t",
			groupID, job.TeamID, pos, job.Direction, totalN, h, pHat, stdErr, relSE, ess, naiveEquivalentSamples, est.Found)
	}

	for i, comp := range job.Components {
		log.Printf("rare-position-component-hist: group=%d team=%d component=%s weight=%.2f target_rank=%d samples=%d hist=%v",
			groupID, job.TeamID, comp.Name, comp.Weight, comp.TargetRank,
			componentSampleCounts[i], componentRankHists[i])
	}

	return results, totalN
}
