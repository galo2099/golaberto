package main

import (
	"fmt"
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
)

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

// simulateTargetSequential uses the same component draw, game order, Poisson
// draws, full-season rank, and full-mixture likelihood as the baseline. It can
// only terminate when the existing conservative points bounds exclude the
// requested exact rank. Such a sample contributes exactly zero.
func simulateTargetSequential(
	baseCampaign []*TeamCampaign,
	games []*GameType,
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
	for gameIndex, game := range games {
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
	SolverChecks           int64                        `json:"solver_checks"`
	SolverDuration         time.Duration                `json:"-"`
	SolverSeconds          float64                      `json:"solver_seconds"`
	SolverFraction         float64                      `json:"solver_fraction_of_runtime"`
	MaxEventWeightShare    float64                      `json:"maximum_event_weight_share"`
	MeanProductionWeight   float64                      `json:"mean_production_weight"`
	UpperConfidence95      float64                      `json:"upper_confidence_bound_95"`
	ProbabilityClass       rarePositionProbabilityClass `json:"probability_threshold_class"`
}

func runSequentialRareEstimate(
	baseCampaign []*TeamCampaign, games []*GameType, originalMeans []GameProposalMeans,
	components []ProposalComponent, table *Table, sortOrder []SortType, teamGroups []TeamType,
	targetTeamID, targetPosition, stride int, samples int, masterSeed int64, method string,
) SequentialRareEstimate {
	result := SequentialRareEstimate{TeamID: targetTeamID, Position: targetPosition, Seed: masterSeed,
		Method: method, Stride: stride, Samples: samples}
	if samples <= 0 {
		return result
	}
	unplayed := 0
	for _, game := range games {
		if !game.Played {
			unplayed++
		}
	}
	var sumY, sumY2, sumEventWeight, maxEventWeight, sumCompletedWeight, pruneIndexSum float64
	var completedWeights int
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
			rank, weight, stats = simulateTargetSequential(baseCampaign, games, originalMeans, components,
				table, sortOrder, teamGroups, targetTeamID, targetPosition, stride, false, rng)
		}
		result.GamesSimulated += int64(stats.GamesSimulated)
		result.SolverChecks += int64(stats.SolverChecks)
		result.SolverDuration += stats.SolverDuration
		sumCompletedWeight += stats.MeanWeightDiagnostic
		completedWeights++
		if stats.Pruned {
			result.Pruned++
			pruneIndexSum += float64(stats.PruneGameIndex)
			quartile := 0
			if unplayed > 0 {
				quartile = minInt(3, stats.PruneGameIndex*4/unplayed)
			}
			result.PrunesBySeasonQuartile[quartile]++
			continue // exact zero contribution; no likelihood ratio is needed
		}
		if rank == targetPosition {
			result.RawHits++
			y := weight
			sumY += y
			sumY2 += y * y
			sumEventWeight += y
			if y > maxEventWeight {
				maxEventWeight = y
			}
		}
	}
	elapsed := time.Since(started)
	result.ElapsedSeconds = elapsed.Seconds()
	result.SolverSeconds = result.SolverDuration.Seconds()
	result.Estimate, result.StdErr, result.ESS = eventEstimateStats(sumY, sumY2, samples)
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
	}
	if result.RawHits > 0 {
		result.ESSPerRawHit = result.ESS / float64(result.RawHits)
		if sumEventWeight > 0 {
			result.MaxEventWeightShare = maxEventWeight / sumEventWeight
		}
	}
	if completedWeights > 0 {
		result.MeanProductionWeight = sumCompletedWeight / float64(completedWeights)
	}
	result.ProbabilityClass = classifyRareProbabilityWithUpper(result.Estimate, result.UpperConfidence95,
		rarePositionMinInterestingProbability())
	return result
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
