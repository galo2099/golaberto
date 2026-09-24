package main

import (
	"fmt"
	"log"
	"math"
	"math/rand"
	"os"
	"sort"
	"strconv"
	"time"
)

const (
	DefaultRarePositionMinInterestingProbability = 1e-5
	ExperimentalRarePositionMinProbability       = 1e-8
	SequentialPruningCalibrationSamples          = 500
	MaxSequentialPruningCalibrationSamples       = 5000
	DefaultSequentialPruningTargetSampleCap      = 100000
	SequentialPruningProductionBudgetFraction    = 0.20
)

func sequentialPruningEnabled() bool {
	return os.Getenv("RARE_POSITION_SEQUENTIAL_PRUNING") == "1"
}

func sequentialPruningCalibrationSampleCount() int {
	value := SequentialPruningCalibrationSamples
	if raw := os.Getenv("RARE_POSITION_SEQUENTIAL_PRUNING_CALIBRATION_SAMPLES"); raw != "" {
		parsed, err := strconv.Atoi(raw)
		if err == nil && parsed > 0 {
			value = parsed
		}
	}
	return minInt(value, MaxSequentialPruningCalibrationSamples)
}

func sequentialPruningTargetSampleCap() int {
	value := DefaultSequentialPruningTargetSampleCap
	if raw := os.Getenv("RARE_POSITION_SEQUENTIAL_PRUNING_MAX_TARGET_SAMPLES"); raw != "" {
		parsed, err := strconv.Atoi(raw)
		if err == nil && parsed > 0 {
			value = parsed
		}
	}
	return value
}

type rarePositionProbabilityClass string

const (
	ProbabilityAtOrAboveThreshold   rarePositionProbabilityClass = "estimate_at_or_above_threshold"
	ProbabilityUncertainAtThreshold rarePositionProbabilityClass = "estimate_below_threshold_uncertainty_overlaps_threshold"
	ProbabilityBelowThreshold       rarePositionProbabilityClass = "upper_confidence_bound_below_threshold"
)

func rarePositionMinInterestingProbability() float64 {
	if raw := os.Getenv("RARE_POSITION_MIN_INTERESTING_PROBABILITY"); raw != "" {
		if value, err := strconv.ParseFloat(raw, 64); err == nil && value > 0 && !math.IsNaN(value) && !math.IsInf(value, 0) {
			return value
		}
	}
	return MinInterestingProbability
}

func classifyRareProbability(probability, stdErr, threshold float64) rarePositionProbabilityClass {
	if probability >= threshold {
		return ProbabilityAtOrAboveThreshold
	}
	if probability+1.96*stdErr < threshold {
		return ProbabilityBelowThreshold
	}
	return ProbabilityUncertainAtThreshold
}

func classifyRareProbabilityWithUpper(probability, upper95, threshold float64) rarePositionProbabilityClass {
	if probability >= threshold {
		return ProbabilityAtOrAboveThreshold
	}
	if upper95 < threshold {
		return ProbabilityBelowThreshold
	}
	return ProbabilityUncertainAtThreshold
}

func estimateMayMeetInterestingThreshold(estimate RarePositionEstimate, threshold float64) bool {
	if estimate.Probability >= threshold {
		return true
	}
	upper := estimate.Probability + 1.96*estimate.StdErr
	if estimate.Hits == 0 && estimate.ZeroHitUpper95 > upper {
		upper = estimate.ZeroHitUpper95
	}
	return upper >= threshold
}

// incrementalPositionBounds maintains the unplayed-game counts while the
// sampled season advances. Its rank bounds intentionally use only points,
// matching conservativePositionBounds exactly.
type incrementalPositionBounds struct {
	remainingByTeam map[int]int
	remainingGames  int
	teams           []TeamType
}

func newIncrementalPositionBounds(games []*GameType, teams []TeamType) *incrementalPositionBounds {
	state := &incrementalPositionBounds{
		remainingByTeam: make(map[int]int, len(teams)),
		teams:           teams,
	}
	for _, game := range games {
		if game.Played {
			continue
		}
		state.remainingGames++
		state.remainingByTeam[game.HomeId]++
		state.remainingByTeam[game.AwayId]++
	}
	return state
}

func (state *incrementalPositionBounds) gameCompleted(game *GameType) {
	state.remainingGames--
	state.remainingByTeam[game.HomeId]--
	state.remainingByTeam[game.AwayId]--
}

func (state *incrementalPositionBounds) ranks(targetTeamID int, campaign []*TeamCampaign, table *Table) (bestRank, worstRank int, hasUnplayed bool) {
	if state.remainingGames == 0 {
		return 0, 0, false
	}
	target := campaign[table.Query(uint32(targetTeamID))]
	targetUnplayed := state.remainingByTeam[targetTeamID]
	targetMin, targetMax := 0, 0
	if target != nil {
		minGain, maxGain := target.points_loss, target.points_loss
		if target.points_draw < minGain {
			minGain = target.points_draw
		}
		if target.points_win < minGain {
			minGain = target.points_win
		}
		if target.points_draw > maxGain {
			maxGain = target.points_draw
		}
		if target.points_win > maxGain {
			maxGain = target.points_win
		}
		targetMin = target.points + targetUnplayed*minGain
		targetMax = target.points + targetUnplayed*maxGain
	}
	strictlyBetter, strictlyWorse := 0, 0
	for _, team := range state.teams {
		id := team.Team_id
		if id == targetTeamID {
			continue
		}
		campaignTeam := campaign[table.Query(uint32(id))]
		minPoints, maxPoints := 0, 0 // map lookup defaults in conservativePositionBounds
		if campaignTeam != nil {
			remaining := state.remainingByTeam[id]
			minGain, maxGain := campaignTeam.points_loss, campaignTeam.points_loss
			if campaignTeam.points_draw < minGain {
				minGain = campaignTeam.points_draw
			}
			if campaignTeam.points_win < minGain {
				minGain = campaignTeam.points_win
			}
			if campaignTeam.points_draw > maxGain {
				maxGain = campaignTeam.points_draw
			}
			if campaignTeam.points_win > maxGain {
				maxGain = campaignTeam.points_win
			}
			minPoints = campaignTeam.points + remaining*minGain
			maxPoints = campaignTeam.points + remaining*maxGain
		}
		if minPoints > targetMax {
			strictlyBetter++
		}
		if maxPoints < targetMin {
			strictlyWorse++
		}
	}
	return strictlyBetter, (len(state.teams) - 1) - strictlyWorse, true
}

type SequentialPruningStats struct {
	Pruned               bool          `json:"pruned"`
	PruneGameIndex       int           `json:"prune_game_index"`
	GamesSimulated       int           `json:"games_simulated"`
	SolverChecks         int           `json:"solver_checks"`
	SolverDuration       time.Duration `json:"-"`
	SampleDuration       time.Duration `json:"-"`
	ChosenComponent      int           `json:"chosen_component"`
	CompletedRank        int           `json:"completed_rank"`
	ImportanceWeight     float64       `json:"importance_weight"`
	MeanWeightDiagnostic float64       `json:"-"`
	ScorelineSignature   uint64        `json:"-"`
}

type SequentialGameOrdering struct {
	Name        string
	GameIndexes []int
}

func buildSequentialGameOrderings(
	games []*GameType,
	targetTeamID int,
	targetPosition int,
	normalMeanRanks map[int]float64,
) []SequentialGameOrdering {
	var unplayed []int
	for i, g := range games {
		if !g.Played {
			unplayed = append(unplayed, i)
		}
	}

	originalIndexes := append([]int(nil), unplayed...)

	var targetGames []int
	var nonTargetGames []int
	for _, idx := range unplayed {
		g := games[idx]
		if g.HomeId == targetTeamID || g.AwayId == targetTeamID {
			targetGames = append(targetGames, idx)
		} else {
			nonTargetGames = append(nonTargetGames, idx)
		}
	}
	targetFirstIndexes := make([]int, 0, len(unplayed))
	targetFirstIndexes = append(targetFirstIndexes, targetGames...)
	targetFirstIndexes = append(targetFirstIndexes, nonTargetGames...)

	inCorridor := func(teamID int) bool {
		if normalMeanRanks == nil {
			return false
		}
		meanRank, ok := normalMeanRanks[teamID]
		if !ok {
			return false
		}
		return math.Abs(meanRank-float64(targetPosition)) <= 3.0
	}

	var tier1 []int
	var tier2a []int
	var tier2b []int
	var tier3 []int

	for _, idx := range unplayed {
		g := games[idx]
		if g.HomeId == targetTeamID || g.AwayId == targetTeamID {
			tier1 = append(tier1, idx)
		} else {
			corridorCount := 0
			if inCorridor(g.HomeId) {
				corridorCount++
			}
			if inCorridor(g.AwayId) {
				corridorCount++
			}
			if corridorCount == 2 {
				tier2a = append(tier2a, idx)
			} else if corridorCount == 1 {
				tier2b = append(tier2b, idx)
			} else {
				tier3 = append(tier3, idx)
			}
		}
	}

	targetBoundaryIndexes := make([]int, 0, len(unplayed))
	targetBoundaryIndexes = append(targetBoundaryIndexes, tier1...)
	targetBoundaryIndexes = append(targetBoundaryIndexes, tier2a...)
	targetBoundaryIndexes = append(targetBoundaryIndexes, tier2b...)
	targetBoundaryIndexes = append(targetBoundaryIndexes, tier3...)

	return []SequentialGameOrdering{
		{Name: "original", GameIndexes: originalIndexes},
		{Name: "target_first", GameIndexes: targetFirstIndexes},
		{Name: "target_boundary", GameIndexes: targetBoundaryIndexes},
	}
}

func selectGameOrdering(orderings []SequentialGameOrdering, name string) SequentialGameOrdering {
	for _, ord := range orderings {
		if ord.Name == name {
			return ord
		}
	}
	if len(orderings) > 0 {
		return orderings[0]
	}
	return SequentialGameOrdering{Name: "original"}
}

type SequentialPruningCalibrationSummary struct {
	Method           string  `json:"method"`
	Ordering         string  `json:"ordering"`
	Stride           int     `json:"stride"`
	Samples          int     `json:"samples"`
	ElapsedSeconds   float64 `json:"elapsed_seconds"`
	SamplesPerSecond float64 `json:"samples_per_second"`
	AverageGames     float64 `json:"average_games_per_sample"`
	SolverChecks     int64   `json:"solver_checks"`
	SolverSeconds    float64 `json:"solver_seconds"`
	SolverFraction   float64 `json:"solver_fraction"`
	PrunedFraction   float64 `json:"pruned_fraction"`
}

type SequentialPruningProductionPlan struct {
	Enabled                  bool
	SelectedOrdering         string
	SelectedStride           int
	CalibrationSamples       int
	CalibrationWork          int64
	AllRankSamples           int
	AllRankWork              int64
	TargetSamples            int
	TargetWork               int64
	TotalWork                int64
	TargetWorkPerSample      int64
	BaselineSecondsPerSample float64
	PrunedSecondsPerSample   float64
	Speedup                  float64
	RuntimeFractionTarget    float64
	ExpectedAllRankSeconds   float64
	ExpectedTargetSeconds    float64
	ExpectedTotalSeconds     float64
}

func planSequentialPruningProduction(remainingWork, allRankWorkPerSample int64,
	desiredCalibrationSamples int, baselineSamplesPerSecond, prunedSamplesPerSecond,
	averageGamesPerPrunedSample float64, candidateCount, unplayedGames, teamCount, componentCount,
	targetSampleCap int, calibrationSeconds float64) SequentialPruningProductionPlan {
	plan := SequentialPruningProductionPlan{}
	if candidateCount <= 0 {
		candidateCount = 7
	}
	if remainingWork <= 0 || allRankWorkPerSample <= 0 || desiredCalibrationSamples <= 0 ||
		baselineSamplesPerSecond <= 0 || prunedSamplesPerSecond <= baselineSamplesPerSecond ||
		math.IsNaN(baselineSamplesPerSecond) || math.IsNaN(prunedSamplesPerSecond) ||
		math.IsInf(baselineSamplesPerSecond, 0) || math.IsInf(prunedSamplesPerSecond, 0) {
		return plan
	}
	// Calibration compares baseline plus ordering × stride combinations.
	maxCalibrationSamples := int(remainingWork / (int64(candidateCount) * 5 * allRankWorkPerSample))
	calibrationSamples := minInt(desiredCalibrationSamples, maxCalibrationSamples)
	if calibrationSamples <= 0 {
		return plan
	}
	plan.CalibrationSamples = calibrationSamples
	plan.CalibrationWork = int64(calibrationSamples*candidateCount) * allRankWorkPerSample
	remainingAfterCalibration := remainingWork - plan.CalibrationWork
	if unplayedGames <= 0 || teamCount < 0 || componentCount <= 0 || targetSampleCap <= 0 {
		return SequentialPruningProductionPlan{}
	}
	plan.BaselineSecondsPerSample = 1 / baselineSamplesPerSecond
	plan.PrunedSecondsPerSample = 1 / prunedSamplesPerSecond
	plan.Speedup = prunedSamplesPerSecond / baselineSamplesPerSecond
	plan.RuntimeFractionTarget = SequentialPruningProductionBudgetFraction
	productionRuntimeBudget := float64(remainingAfterCalibration) / float64(allRankWorkPerSample) * plan.BaselineSecondsPerSample
	plan.ExpectedAllRankSeconds = productionRuntimeBudget * (1 - plan.RuntimeFractionTarget)
	plan.ExpectedTargetSeconds = productionRuntimeBudget * plan.RuntimeFractionTarget
	plan.AllRankSamples, plan.TargetSamples = runtimeDerivedSampleCounts(productionRuntimeBudget,
		plan.RuntimeFractionTarget, baselineSamplesPerSecond, prunedSamplesPerSecond, targetSampleCap)
	plan.AllRankSamples = minInt(plan.AllRankSamples, int(remainingAfterCalibration/allRankWorkPerSample))
	if plan.AllRankSamples < 1 {
		return SequentialPruningProductionPlan{}
	}
	plan.AllRankWork = int64(plan.AllRankSamples) * allRankWorkPerSample
	targetWorkPerSample := int64(math.Ceil(averageGamesPerPrunedSample*float64(componentCount) + float64(teamCount)))
	if targetWorkPerSample <= 0 {
		targetWorkPerSample = allRankWorkPerSample
	}
	plan.TargetWorkPerSample = targetWorkPerSample
	remainingForTarget := remainingAfterCalibration - plan.AllRankWork
	affordableTargetSamples := int(remainingForTarget / targetWorkPerSample)
	plan.TargetSamples = minInt(plan.TargetSamples, affordableTargetSamples)
	if plan.TargetSamples <= 0 {
		return SequentialPruningProductionPlan{}
	}
	plan.TargetWork = int64(plan.TargetSamples) * targetWorkPerSample
	plan.ExpectedAllRankSeconds = float64(plan.AllRankSamples) * plan.BaselineSecondsPerSample
	plan.ExpectedTargetSeconds = float64(plan.TargetSamples) * plan.PrunedSecondsPerSample
	plan.ExpectedTotalSeconds = calibrationSeconds + plan.ExpectedAllRankSeconds + plan.ExpectedTargetSeconds
	plan.TotalWork = plan.CalibrationWork + plan.AllRankWork + plan.TargetWork
	plan.Enabled = plan.AllRankSamples > 0 && plan.TargetSamples > 0 && plan.TotalWork <= remainingWork
	if !plan.Enabled {
		return SequentialPruningProductionPlan{}
	}
	return plan
}

func runtimeDerivedSampleCounts(runtimeBudgetSeconds, targetRuntimeFraction,
	baselineSamplesPerSecond, prunedSamplesPerSecond float64, targetSampleCap int) (allRankSamples, targetSamples int) {
	if runtimeBudgetSeconds <= 0 || targetRuntimeFraction <= 0 || targetRuntimeFraction >= 1 ||
		baselineSamplesPerSecond <= 0 || prunedSamplesPerSecond <= 0 || targetSampleCap <= 0 {
		return 0, 0
	}
	allRankFloat := math.Floor(runtimeBudgetSeconds * (1 - targetRuntimeFraction) * baselineSamplesPerSecond)
	targetFloat := math.Floor(runtimeBudgetSeconds * targetRuntimeFraction * prunedSamplesPerSecond)
	maxIntValue := int(^uint(0) >> 1)
	if allRankFloat >= float64(maxIntValue) {
		allRankSamples = maxIntValue
	} else {
		allRankSamples = int(allRankFloat)
	}
	if targetFloat >= float64(targetSampleCap) {
		targetSamples = targetSampleCap
	} else {
		targetSamples = int(targetFloat)
	}
	return allRankSamples, targetSamples
}

func calibrateSequentialPruning(
	baseCampaign []*TeamCampaign, games []*GameType,
	originalMeans []GameProposalMeans, components []ProposalComponent, table *Table,
	sortOrder []SortType, teamGroups []TeamType, targetTeamID, targetPosition int,
	normalMeanRanks map[int]float64, samples int, masterSeed int64,
) (string, int, []SequentialPruningCalibrationSummary) {
	unplayed := 0
	for _, game := range games {
		if !game.Played {
			unplayed++
		}
	}
	orderings := buildSequentialGameOrderings(games, targetTeamID, targetPosition, normalMeanRanks)

	type candidateSpec struct {
		method   string
		ordering string
		stride   int
	}

	candidates := []candidateSpec{
		{method: "baseline", ordering: "baseline", stride: 0},
		{method: "pruned", ordering: "original", stride: 4},
		{method: "pruned", ordering: "original", stride: 8},
		{method: "pruned", ordering: "target_first", stride: 4},
		{method: "pruned", ordering: "target_first", stride: 8},
		{method: "pruned", ordering: "target_boundary", stride: 4},
		{method: "pruned", ordering: "target_boundary", stride: 8},
	}

	results := make([]SequentialPruningCalibrationSummary, 0, len(candidates))

	run := func(spec candidateSpec) SequentialPruningCalibrationSummary {
		result := SequentialPruningCalibrationSummary{
			Method:   spec.method,
			Ordering: spec.ordering,
			Stride:   spec.stride,
			Samples:  samples,
		}
		stream := deriveRarePositionSeed(masterSeed, fmt.Sprintf("runtime-calibration-%s-%d", spec.ordering, spec.stride))
		start := time.Now()
		var gamesSimulated int64
		var pruned int
		var sim []*TeamCampaign
		var teamSlice []*TeamCampaign
		var logs, weights []float64
		var allRanks *weightedRankAccumulator
		var gameIndexes []int
		if spec.method != "baseline" {
			gameIndexes = selectGameOrdering(orderings, spec.ordering).GameIndexes
		} else {
			sim = make([]*TeamCampaign, len(baseCampaign))
			teamSlice = make([]*TeamCampaign, len(teamGroups))
			logs, weights = make([]float64, len(components)), make([]float64, len(components))
			allRanks = newWeightedRankAccumulator(teamGroups, len(teamGroups))
		}
		for i := 0; i < samples; i++ {
			rng := rand.New(rand.NewSource(targetSampleSeed(stream, i)))
			if spec.method == "baseline" {
				_, _, _ = simulateTargetTeamRankAndWeightMulti(baseCampaign, sim, teamSlice,
					games, originalMeans, components, table, sortOrder, teamGroups,
					targetTeamID, rng, logs, weights, allRanks)
				gamesSimulated += int64(unplayed)
			} else {
				_, _, stats := simulateTargetSequential(baseCampaign, games, gameIndexes, originalMeans, components,
					table, sortOrder, teamGroups, targetTeamID, targetPosition, spec.stride, false, rng)
				gamesSimulated += int64(stats.GamesSimulated)
				result.SolverSeconds += stats.SolverDuration.Seconds()
				result.SolverChecks += int64(stats.SolverChecks)
				if stats.Pruned {
					pruned++
				}
			}
		}
		elapsed := time.Since(start)
		result.ElapsedSeconds = elapsed.Seconds()
		if elapsed > 0 {
			result.SamplesPerSecond = float64(samples) / elapsed.Seconds()
			result.SolverFraction = result.SolverSeconds / elapsed.Seconds()
		}
		if samples > 0 {
			result.AverageGames = float64(gamesSimulated) / float64(samples)
			result.PrunedFraction = float64(pruned) / float64(samples)
		}
		log.Printf("rare-position-calibration-candidate: ordering=%s stride=%d samples=%d elapsed_seconds=%.6f samples_per_second=%.2f average_games=%.1f pruned_fraction=%.4f solver_checks=%d solver_seconds=%.6f solver_fraction=%.4f",
			result.Ordering, result.Stride, result.Samples, result.ElapsedSeconds, result.SamplesPerSecond,
			result.AverageGames, result.PrunedFraction, result.SolverChecks, result.SolverSeconds, result.SolverFraction)
		return result
	}

	var baselineRate float64
	var bestOrdering string
	bestStride := 0
	bestRate := -1.0

	for _, spec := range candidates {
		res := run(spec)
		results = append(results, res)
		if spec.method == "baseline" {
			baselineRate = res.SamplesPerSecond
		} else {
			if res.SamplesPerSecond > bestRate {
				bestRate = res.SamplesPerSecond
				bestOrdering = res.Ordering
				bestStride = res.Stride
			}
		}
	}

	if bestRate >= baselineRate*1.03 {
		return bestOrdering, bestStride, results
	}

	return "", 0, results
}

func calibrateSequentialPruningStride(baseCampaign []*TeamCampaign, games []*GameType,
	originalMeans []GameProposalMeans, components []ProposalComponent, table *Table,
	sortOrder []SortType, teamGroups []TeamType, targetTeamID, targetPosition int,
	samples int, masterSeed int64) (int, []SequentialPruningCalibrationSummary) {
	selectedOrdering, selectedStride, summaries := calibrateSequentialPruning(
		baseCampaign, games, originalMeans, components, table, sortOrder, teamGroups,
		targetTeamID, targetPosition, nil, samples, masterSeed,
	)
	_ = selectedOrdering
	return selectedStride, summaries
}

// simulateTargetSequential uses the same component draw, game order, Poisson
// draws, full-season rank, and full-mixture likelihood as the baseline. It can
// only terminate when the existing conservative points bounds exclude the
// requested exact rank. Such a sample contributes exactly zero.
func simulateTargetSequential(
	baseCampaign []*TeamCampaign,
	games []*GameType,
	orderedGameIndexes []int,
	originalMeans []GameProposalMeans,
	components []ProposalComponent,
	table *Table,
	sortOrder []SortType,
	teamGroups []TeamType,
	targetTeamID, targetPosition, stride int, trackScoreline bool,
	rng *rand.Rand,
) (rank int, weight float64, stats SequentialPruningStats) {
	started := time.Now()
	stats.PruneGameIndex = -1
	simCampaign := make([]*TeamCampaign, len(baseCampaign))
	for i, campaign := range baseCampaign {
		simCampaign[i] = campaign.clone()
	}
	logQOverP := make([]float64, len(components))
	componentWeights := make([]float64, len(components))
	for i, component := range components {
		componentWeights[i] = component.Weight
	}
	r := rng.Float64()
	cumulative := 0.0
	chosen := len(components) - 1
	for i, component := range components {
		cumulative += component.Weight
		if r <= cumulative {
			chosen = i
			break
		}
	}
	stats.ChosenComponent = chosen
	state := newIncrementalPositionBounds(games, teamGroups)
	completedUnplayed := 0
	if stride < 1 {
		stride = 1
	}
	if len(orderedGameIndexes) == 0 {
		for i, g := range games {
			if !g.Played {
				orderedGameIndexes = append(orderedGameIndexes, i)
			}
		}
	}
	for _, gameIndex := range orderedGameIndexes {
		game := games[gameIndex]
		if game.Played {
			continue
		}
		homeScore := poissonRand(rng, components[chosen].Means[gameIndex].Home)
		awayScore := poissonRand(rng, components[chosen].Means[gameIndex].Away)
		if trackScoreline {
			stats.ScorelineSignature = (stats.ScorelineSignature ^ uint64(uint32(game.Id))*0x9e3779b185ebca87 ^
				uint64(uint32(homeScore))<<32 ^ uint64(uint32(awayScore))) * 0x100000001b3
		}
		for k, component := range components {
			means := component.Means[gameIndex]
			logQOverP[k] += logPoissonQOverP(homeScore, originalMeans[gameIndex].Home, means.Home)
			logQOverP[k] += logPoissonQOverP(awayScore, originalMeans[gameIndex].Away, means.Away)
		}
		completed := &GameType{game.Id, game.HomeId, game.AwayId, homeScore, awayScore, 0, 0, true,
			game.home_table_index, game.away_table_index}
		if idx := game.home_table_index; simCampaign[idx] != nil {
			simCampaign[idx].add_game(completed)
		}
		if idx := game.away_table_index; simCampaign[idx] != nil {
			simCampaign[idx].add_game(completed)
		}
		state.gameCompleted(game)
		completedUnplayed++
		stats.GamesSimulated++
		if completedUnplayed%stride == 0 && state.remainingGames > 0 {
			solverStarted := time.Now()
			best, worst, _ := state.ranks(targetTeamID, simCampaign, table)
			stats.SolverDuration += time.Since(solverStarted)
			stats.SolverChecks++
			if targetPosition < best || targetPosition > worst {
				stats.Pruned = true
				stats.PruneGameIndex = completedUnplayed - 1
				// The unplayed suffix integrates to one under every proposal
				// component, so the prefix mixture ratio is a valid weight diagnostic.
				stats.MeanWeightDiagnostic = mixtureImportanceWeightMulti(logQOverP, componentWeights)
				stats.SampleDuration = time.Since(started)
				return -1, 0, stats
			}
		}
	}
	// The last completion is always resolved by a normal full-season rank,
	// never by early acceptance or a partial likelihood ratio.
	teamSlice := make([]*TeamCampaign, 0, len(teamGroups))
	for _, team := range teamGroups {
		if c := simCampaign[table.Query(uint32(team.Team_id))]; c != nil {
			teamSlice = append(teamSlice, c)
		}
	}
	sort.Sort(TeamCampaignSorted{t: teamSlice, sort: sortOrder, rng: rng})
	rank = -1
	for pos, team := range teamSlice {
		if team.id == targetTeamID {
			rank = pos
			break
		}
	}
	weight = mixtureImportanceWeightMulti(logQOverP, componentWeights)
	stats.CompletedRank = rank
	stats.ImportanceWeight = weight
	stats.MeanWeightDiagnostic = weight
	stats.SampleDuration = time.Since(started)
	return rank, weight, stats
}

func targetSampleSeed(masterSeed int64, sampleIndex int) int64 {
	return deriveRarePositionSeed(masterSeed, "sequential-target-sample:"+strconv.Itoa(sampleIndex))
}

type SequentialRareEstimate struct {
	TeamID                 int                          `json:"team_id"`
	Position               int                          `json:"position"`
	Seed                   int64                        `json:"seed"`
	Method                 string                       `json:"method"`
	Ordering               string                       `json:"ordering"`
	Stride                 int                          `json:"check_stride"`
	Samples                int                          `json:"samples_completed"`
	GamesSimulated         int64                        `json:"games_simulated"`
	Estimate               float64                      `json:"estimate"`
	StdErr                 float64                      `json:"standard_error"`
	RelativeSE             *float64                     `json:"relative_standard_error"`
	ESS                    float64                      `json:"event_ess"`
	ESSPerSecond           float64                      `json:"event_ess_per_second"`
	RawHits                int                          `json:"raw_exact_hits"`
	ESSPerRawHit           float64                      `json:"ess_per_raw_hit"`
	ElapsedSeconds         float64                      `json:"elapsed_seconds"`
	SamplesPerSecond       float64                      `json:"samples_per_second"`
	AverageGamesPerSample  float64                      `json:"average_games_per_sample"`
	Pruned                 int                          `json:"pruned_samples"`
	PrunedFraction         float64                      `json:"pruned_fraction"`
	MeanGameIndexAtPrune   float64                      `json:"mean_game_index_at_prune"`
	PrunesBySeasonQuartile [4]int                       `json:"prunes_by_season_quartile"`
	PruneP10               float64                      `json:"prune_p10"`
	PruneP25               float64                      `json:"prune_p25"`
	PruneP50               float64                      `json:"prune_p50"`
	PruneP75               float64                      `json:"prune_p75"`
	PruneP90               float64                      `json:"prune_p90"`
	PrunedBefore25Pct      int                          `json:"pruned_before_25pct"`
	PrunedBefore50Pct      int                          `json:"pruned_before_50pct"`
	PrunedBefore75Pct      int                          `json:"pruned_before_75pct"`
	SolverChecks           int64                        `json:"solver_checks"`
	SolverDuration         time.Duration                `json:"-"`
	SolverSeconds          float64                      `json:"solver_seconds"`
	SolverFraction         float64                      `json:"solver_fraction_of_runtime"`
	MaxEventWeightShare    float64                      `json:"maximum_event_weight_share"`
	MeanProductionWeight   float64                      `json:"mean_production_weight"`
	UpperConfidence95      float64                      `json:"upper_confidence_bound_95"`
	ProbabilityClass       rarePositionProbabilityClass `json:"probability_threshold_class"`
	SufficientStats        EventSufficientStats         `json:"-"`
}

func calculatePercentile(sortedValues []int, percentile float64) float64 {
	if len(sortedValues) == 0 {
		return 0
	}
	if len(sortedValues) == 1 {
		return float64(sortedValues[0])
	}
	index := percentile * float64(len(sortedValues)-1)
	lower := int(index)
	upper := lower + 1
	if upper >= len(sortedValues) {
		return float64(sortedValues[lower])
	}
	weight := index - float64(lower)
	return float64(sortedValues[lower])*(1-weight) + float64(sortedValues[upper])*weight
}

func runSequentialRareEstimate(
	baseCampaign []*TeamCampaign, games []*GameType, originalMeans []GameProposalMeans,
	components []ProposalComponent, table *Table, sortOrder []SortType, teamGroups []TeamType,
	targetTeamID, targetPosition int, orderingName string, stride int, samples int, masterSeed int64,
	method string, normalMeanRanks map[int]float64,
) SequentialRareEstimate {
	if orderingName == "" {
		orderingName = "original"
	}
	result := SequentialRareEstimate{TeamID: targetTeamID, Position: targetPosition, Seed: masterSeed,
		Method: method, Ordering: orderingName, Stride: stride, Samples: samples}
	if method == "baseline" {
		result.Ordering = "baseline"
		result.Stride = 0
	}
	if samples <= 0 {
		return result
	}
	unplayed := 0
	for _, game := range games {
		if !game.Played {
			unplayed++
		}
	}
	orderings := buildSequentialGameOrderings(games, targetTeamID, targetPosition, normalMeanRanks)
	selectedOrdering := selectGameOrdering(orderings, orderingName)

	var sumCompletedWeight, pruneIndexSum float64
	var completedWeights int
	pruneGames := make([]int, 0, samples)
	started := time.Now()
	for i := 0; i < samples; i++ {
		rng := rand.New(rand.NewSource(targetSampleSeed(masterSeed, i)))
		var rank int
		var weight float64
		var stats SequentialPruningStats
		if method == "baseline" {
			sim := make([]*TeamCampaign, len(baseCampaign))
			teamSlice := make([]*TeamCampaign, len(teamGroups))
			logs := make([]float64, len(components))
			weights := make([]float64, len(components))
			var chosen int
			rank, weight, chosen = simulateTargetTeamRankAndWeightMulti(baseCampaign, sim, teamSlice,
				games, originalMeans, components, table, sortOrder, teamGroups, targetTeamID, rng,
				logs, weights, nil)
			stats.ChosenComponent = chosen
			stats.GamesSimulated = unplayed
			stats.CompletedRank = rank
			stats.ImportanceWeight = weight
			stats.MeanWeightDiagnostic = weight
		} else {
			rank, weight, stats = simulateTargetSequential(baseCampaign, games, selectedOrdering.GameIndexes,
				originalMeans, components, table, sortOrder, teamGroups, targetTeamID, targetPosition, stride, false, rng)
		}
		result.GamesSimulated += int64(stats.GamesSimulated)
		result.SolverChecks += int64(stats.SolverChecks)
		result.SolverDuration += stats.SolverDuration
		if stats.Pruned {
			result.Pruned++
			result.SufficientStats.observeZero()
			pruneIndexSum += float64(stats.PruneGameIndex)
			pruneGames = append(pruneGames, stats.GamesSimulated)
			quartile := 0
			if unplayed > 0 {
				quartile = minInt(3, stats.PruneGameIndex*4/unplayed)
			}
			result.PrunesBySeasonQuartile[quartile]++
			continue // exact zero contribution; no likelihood ratio is needed
		}
		result.SufficientStats.observe(weight, rank == targetPosition)
		sumCompletedWeight += stats.MeanWeightDiagnostic
		completedWeights++
		if rank == targetPosition {
			result.RawHits++
		}
	}
	elapsed := time.Since(started)
	result.ElapsedSeconds = elapsed.Seconds()
	result.SolverSeconds = result.SolverDuration.Seconds()
	result.Estimate, result.StdErr, result.ESS = eventEstimateStats(
		result.SufficientStats.SumY, result.SufficientStats.SumY2, result.SufficientStats.Samples)
	if result.Estimate > 0 {
		relativeSE := result.StdErr / result.Estimate
		result.RelativeSE = &relativeSE
	}
	result.UpperConfidence95 = result.Estimate + 1.96*result.StdErr
	if result.RawHits == 0 {
		result.UpperConfidence95 = weightedZeroHitUpper95(samples, 1/OriginalMixtureWeight)
	}
	if elapsed > 0 {
		seconds := elapsed.Seconds()
		result.SamplesPerSecond = float64(samples) / seconds
		result.ESSPerSecond = result.ESS / seconds
		result.SolverFraction = result.SolverSeconds / seconds
	}
	result.AverageGamesPerSample = float64(result.GamesSimulated) / float64(samples)
	result.PrunedFraction = float64(result.Pruned) / float64(samples)
	if result.Pruned > 0 {
		result.MeanGameIndexAtPrune = pruneIndexSum / float64(result.Pruned)
		sort.Ints(pruneGames)
		result.PruneP10 = calculatePercentile(pruneGames, 0.10)
		result.PruneP25 = calculatePercentile(pruneGames, 0.25)
		result.PruneP50 = calculatePercentile(pruneGames, 0.50)
		result.PruneP75 = calculatePercentile(pruneGames, 0.75)
		result.PruneP90 = calculatePercentile(pruneGames, 0.90)
		p25Limit := int(math.Ceil(float64(unplayed) * 0.25))
		p50Limit := int(math.Ceil(float64(unplayed) * 0.50))
		p75Limit := int(math.Ceil(float64(unplayed) * 0.75))
		for _, g := range pruneGames {
			if g <= p25Limit {
				result.PrunedBefore25Pct++
			}
			if g <= p50Limit {
				result.PrunedBefore50Pct++
			}
			if g <= p75Limit {
				result.PrunedBefore75Pct++
			}
		}
	}
	if result.RawHits > 0 {
		result.ESSPerRawHit = result.ESS / float64(result.RawHits)
		if result.SufficientStats.SumY > 0 {
			result.MaxEventWeightShare = result.SufficientStats.MaxEventWeight / result.SufficientStats.SumY
		}
	}
	result.Samples = result.SufficientStats.Samples
	if completedWeights > 0 {
		result.MeanProductionWeight = sumCompletedWeight / float64(completedWeights)
	}
	result.ProbabilityClass = classifyRareProbabilityWithUpper(result.Estimate, result.UpperConfidence95,
		rarePositionMinInterestingProbability())
	return result
}

func productionEstimateFromSequential(estimate SequentialRareEstimate, work int64) ProductionEstimate {
	return productionEstimateFromSufficientStats(estimate.SufficientStats, work,
		"importance_sampling_sequential_pruning")
}

func minInt(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func (stats SequentialPruningStats) String() string {
	return fmt.Sprintf("pruned=%t checks=%d games=%d", stats.Pruned, stats.SolverChecks, stats.GamesSimulated)
}
