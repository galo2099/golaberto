package main

import (
	"fmt"
	"log"
	"math"
	"math/rand"
	"sort"
)

const (
	CEMBatchSamples                = 300
	CEMMaxIterations               = 4
	CEMEliteFraction               = 0.15
	CEMSmoothing                   = 0.5
	CEMMaxKL                       = 3.0
	CEMExactEventThreshold         = 5
	CEMValidationSamples           = 500
	MaxCEMWorkFraction             = 0.10
	MaxCEMValidationWorkFraction   = 0.02
	MaxCEMPlainEquivalentSamples   = 5000
	CEMMaxChangedGames             = 12
	CEMMinExactHitsForValidation   = 2
	CEMNearTargetRateForValidation = 0.05
	CEMStrongNearTargetRate        = 0.10
	CEMRetentionBonus              = 0.25
	CEMMinBatchSamples             = 100
	CEMMinRelativeProgress         = 0.02
	CEMMaxStalledIterations        = 2
)

type CEMProposal struct {
	Means         []GameProposalMeans
	Iteration     int
	KL            float64
	ChangedGames  int
	SelectedGames []int
	Signals       []CEMGameSignal
	EliteESS      float64
}

type CEMGameSignal struct {
	GameIndex     int
	HomeEliteMean float64
	AwayEliteMean float64
	HomeESS       float64
	AwayESS       float64
	HomeSE        float64
	AwaySE        float64
	HomeZ         float64
	AwayZ         float64
	GameScore     float64
	Selected      bool
}

type CEMScore struct{ Home, Away int }

type CEMSeason struct {
	Rank      int
	Scores    []CEMScore
	LogWeight float64 // log(P / Q_current), never a probability estimate
}

type CEMBatchStats struct {
	ExactHits         int
	EliteCount        int
	MeanRank          float64
	BestRank          int
	EliteMeanDistance float64
	ExactEventESS     float64
	ExactRate         float64
	NearTargetHits    int
	NearTargetRate    float64
	EliteESS          float64
	UsedExactElites   bool
}

type CEMRoundResult struct {
	Eligible          []*FrontierCandidate
	CEMWork           int64
	ValidationWork    int64
	TargetsAttempted  int
	Iterations        int
	TargetsAnyExact   int
	TargetsExactElite int
	TargetsValidated  int
	SelectedGames     int
	MaxSelectedGames  int
	ValidatedESS      float64
}

func cemPoissonKL(mu, original float64) float64 {
	if math.IsNaN(mu) || math.IsNaN(original) || mu < 0 || original < 0 {
		return math.Inf(1)
	}
	if original == 0 {
		if mu == 0 {
			return 0
		}
		return math.Inf(1)
	}
	if mu == 0 {
		return original
	}
	return mu*math.Log(mu/original) - mu + original
}

func cemTotalKL(means, original []GameProposalMeans, games []*GameType) float64 {
	total := 0.0
	for i, game := range games {
		if game.Played {
			continue
		}
		total += cemPoissonKL(means[i].Home, original[i].Home)
		total += cemPoissonKL(means[i].Away, original[i].Away)
	}
	return total
}

func cemWeightedMean(scores []float64, weights []float64) float64 {
	weighted, total := 0.0, 0.0
	for i, score := range scores {
		weighted += weights[i] * score
		total += weights[i]
	}
	if total <= 0 {
		return 0
	}
	return weighted / total
}

func cemSmoothMean(current, estimate float64) float64 {
	if math.IsNaN(current) || math.IsInf(current, 0) || current < 0 {
		current = 0
	}
	if math.IsNaN(estimate) || math.IsInf(estimate, 0) || estimate < 0 {
		estimate = 0
	}
	return (1-CEMSmoothing)*current + CEMSmoothing*estimate
}

func cemInterpolatedMeans(original, proposed []GameProposalMeans, games []*GameType, fraction float64) []GameProposalMeans {
	result := append([]GameProposalMeans(nil), original...)
	if fraction < 0 {
		fraction = 0
	}
	if fraction > 1 {
		fraction = 1
	}
	for i, game := range games {
		if game.Played {
			continue
		}
		result[i].Home = cemInterpolateMean(original[i].Home, proposed[i].Home, fraction)
		result[i].Away = cemInterpolateMean(original[i].Away, proposed[i].Away, fraction)
	}
	return result
}

func cemInterpolateMean(original, proposed, fraction float64) float64 {
	if original <= 0 || math.IsNaN(original) || math.IsInf(original, 0) {
		return 0
	}
	if proposed <= 0 || math.IsNaN(proposed) || math.IsInf(proposed, 0) {
		return original
	}
	result := original * math.Exp(fraction*math.Log(proposed/original))
	if math.IsNaN(result) || math.IsInf(result, 0) || result < 0 {
		return original
	}
	return result
}

// Scale the entire log-mean update toward P; never clip individual dimensions.
func cemTrustRegion(original, proposed []GameProposalMeans, games []*GameType, maxKL float64) ([]GameProposalMeans, float64) {
	kl := cemTotalKL(proposed, original, games)
	if kl <= maxKL {
		return proposed, kl
	}
	lo, hi := 0.0, 1.0
	for i := 0; i < 55; i++ {
		mid := (lo + hi) / 2
		candidate := cemInterpolatedMeans(original, proposed, games, mid)
		if cemTotalKL(candidate, original, games) <= maxKL {
			lo = mid
		} else {
			hi = mid
		}
	}
	result := cemInterpolatedMeans(original, proposed, games, lo)
	return result, cemTotalKL(result, original, games)
}

func cemRankDistance(rank, target int) int {
	return absInt(rank - target)
}

func cemDirectionalPenalty(rank, target int, direction RareDirection) int {
	if direction == RareBetter && rank <= target {
		return 0
	}
	if direction == RareWorse && rank >= target {
		return 0
	}
	return 1
}

func cemEliteIndices(seasons []CEMSeason, target int, direction RareDirection) ([]int, bool) {
	var exact []int
	for i, season := range seasons {
		if season.Rank == target {
			exact = append(exact, i)
		}
	}
	if len(exact) >= CEMExactEventThreshold {
		return exact, true
	}
	indexes := make([]int, len(seasons))
	for i := range indexes {
		indexes[i] = i
	}
	sort.Slice(indexes, func(i, j int) bool {
		a, b := seasons[indexes[i]].Rank, seasons[indexes[j]].Rank
		da, db := cemRankDistance(a, target), cemRankDistance(b, target)
		if da != db {
			return da < db
		}
		pa, pb := cemDirectionalPenalty(a, target, direction), cemDirectionalPenalty(b, target, direction)
		if pa != pb {
			return pa < pb
		}
		return indexes[i] < indexes[j]
	})
	count := int(math.Ceil(float64(len(indexes)) * CEMEliteFraction))
	if count < 1 && len(indexes) > 0 {
		count = 1
	}
	return indexes[:count], false
}

func absInt(x int) int {
	if x < 0 {
		return -x
	}
	return x
}

func cemEventESS(seasons []CEMSeason, target int) float64 {
	maxLog := math.Inf(-1)
	for _, season := range seasons {
		if season.Rank == target && season.LogWeight > maxLog {
			maxLog = season.LogWeight
		}
	}
	if math.IsInf(maxLog, -1) {
		return 0
	}
	sum, sum2 := 0.0, 0.0
	for _, season := range seasons {
		if season.Rank == target {
			w := math.Exp(season.LogWeight - maxLog)
			sum += w
			sum2 += w * w
		}
	}
	return sum * sum / sum2
}

func cemEliteWeights(seasons []CEMSeason, elite []int) ([]float64, float64) {
	if len(elite) == 0 {
		return nil, 0
	}
	maxLog := math.Inf(-1)
	for _, index := range elite {
		if seasons[index].LogWeight > maxLog {
			maxLog = seasons[index].LogWeight
		}
	}
	weights := make([]float64, len(elite))
	sum, sum2 := 0.0, 0.0
	for j, index := range elite {
		weights[j] = math.Exp(seasons[index].LogWeight - maxLog)
		sum += weights[j]
		sum2 += weights[j] * weights[j]
	}
	if sum2 == 0 {
		return weights, 0
	}
	return weights, sum * sum / sum2
}

func cemGameSignals(current []GameProposalMeans, games []*GameType,
	seasons []CEMSeason, elite []int, weights []float64, eliteESS float64) []CEMGameSignal {
	signals := make([]CEMGameSignal, 0, len(games))
	if len(elite) == 0 || eliteESS <= 0 {
		return signals
	}
	for i, game := range games {
		if game.Played || current[i].Home <= 0 && current[i].Away <= 0 {
			continue
		}
		homeScores, awayScores := make([]float64, len(elite)), make([]float64, len(elite))
		for j, index := range elite {
			homeScores[j] = float64(seasons[index].Scores[i].Home)
			awayScores[j] = float64(seasons[index].Scores[i].Away)
		}
		homeMean := cemWeightedMean(homeScores, weights)
		awayMean := cemWeightedMean(awayScores, weights)
		homeSE, awaySE := 0.0, 0.0
		homeZ, awayZ := 0.0, 0.0
		if current[i].Home > 0 {
			homeSE = math.Sqrt(math.Max(current[i].Home, 1e-9) / math.Max(eliteESS, 1e-9))
			homeZ = (homeMean - current[i].Home) / homeSE
		}
		if current[i].Away > 0 {
			awaySE = math.Sqrt(math.Max(current[i].Away, 1e-9) / math.Max(eliteESS, 1e-9))
			awayZ = (awayMean - current[i].Away) / awaySE
		}
		signals = append(signals, CEMGameSignal{
			GameIndex: i, HomeEliteMean: homeMean, AwayEliteMean: awayMean,
			HomeESS: eliteESS, AwayESS: eliteESS, HomeSE: homeSE, AwaySE: awaySE,
			HomeZ: homeZ, AwayZ: awayZ,
			GameScore: math.Hypot(homeZ, awayZ),
		})
	}
	return signals
}

func selectCEMGames(signals []CEMGameSignal, previous []int) []CEMGameSignal {
	previousSet := make(map[int]bool, len(previous))
	for _, index := range previous {
		previousSet[index] = true
	}
	candidates := make([]CEMGameSignal, 0, len(signals))
	for _, signal := range signals {
		if signal.GameScore > 1e-12 || previousSet[signal.GameIndex] {
			signal.Selected = false
			candidates = append(candidates, signal)
		}
	}
	sort.Slice(candidates, func(i, j int) bool {
		a, b := candidates[i], candidates[j]
		aScore := a.GameScore
		bScore := b.GameScore
		if previousSet[a.GameIndex] {
			aScore += CEMRetentionBonus
		}
		if previousSet[b.GameIndex] {
			bScore += CEMRetentionBonus
		}
		if aScore != bScore {
			return aScore > bScore
		}
		return a.GameIndex < b.GameIndex
	})
	if len(candidates) > CEMMaxChangedGames {
		candidates = candidates[:CEMMaxChangedGames]
	}
	for i := range candidates {
		candidates[i].Selected = true
	}
	return candidates
}

func cemUpdateSparse(current, original []GameProposalMeans, games []*GameType,
	seasons []CEMSeason, elite []int, previous []int) CEMProposal {
	proposed := append([]GameProposalMeans(nil), original...)
	weights, eliteESS := cemEliteWeights(seasons, elite)
	if len(elite) == 0 || eliteESS <= 0 {
		return CEMProposal{Means: proposed, KL: cemTotalKL(proposed, original, games)}
	}
	signals := cemGameSignals(current, games, seasons, elite, weights, eliteESS)
	selected := selectCEMGames(signals, previous)
	selectedIndexes := make([]int, len(selected))
	for j, signal := range selected {
		selectedIndexes[j] = signal.GameIndex
		if original[signal.GameIndex].Home > 0 {
			proposed[signal.GameIndex].Home = cemSmoothMean(current[signal.GameIndex].Home, signal.HomeEliteMean)
		}
		if original[signal.GameIndex].Away > 0 {
			proposed[signal.GameIndex].Away = cemSmoothMean(current[signal.GameIndex].Away, signal.AwayEliteMean)
		}
	}
	trusted, kl := cemTrustRegion(original, proposed, games, CEMMaxKL)
	changed := 0
	for _, signal := range selected {
		if cemGameChange(trusted[signal.GameIndex], original[signal.GameIndex]) > 0.05 {
			changed++
		}
	}
	return CEMProposal{Means: trusted, KL: kl, ChangedGames: changed,
		SelectedGames: selectedIndexes, Signals: selected, EliteESS: eliteESS}
}

func cemUpdate(current, original []GameProposalMeans, games []*GameType,
	seasons []CEMSeason, elite []int) CEMProposal {
	return cemUpdateSparse(current, original, games, seasons, elite, nil)
}

func cemGameChange(proposal, original GameProposalMeans) float64 {
	delta := 0.0
	if original.Home > 0 && proposal.Home > 0 {
		delta += math.Abs(math.Log(proposal.Home / original.Home))
	}
	if original.Away > 0 && proposal.Away > 0 {
		delta += math.Abs(math.Log(proposal.Away / original.Away))
	}
	return delta
}

func simulateCEMBatch(base []*TeamCampaign, games []*GameType, original, proposal []GameProposalMeans,
	table *Table, order []SortType, groups []TeamType, targetTeam, samples int, rng *rand.Rand) []CEMSeason {
	batch := make([]CEMSeason, 0, samples)
	simCampaign := make([]*TeamCampaign, len(base))
	teamSlice := make([]*TeamCampaign, len(groups))
	for n := 0; n < samples; n++ {
		for i, campaign := range base {
			simCampaign[i] = campaign.clone()
		}
		season := CEMSeason{Scores: make([]CEMScore, len(games))}
		for i, game := range games {
			if game.Played {
				continue
			}
			home := poissonRand(rng, proposal[i].Home)
			away := poissonRand(rng, proposal[i].Away)
			season.Scores[i] = CEMScore{home, away}
			season.LogWeight -= logPoissonQOverP(home, original[i].Home, proposal[i].Home)
			season.LogWeight -= logPoissonQOverP(away, original[i].Away, proposal[i].Away)
			score := &GameType{game.Id, game.HomeId, game.AwayId, home, away, 0, 0, true,
				game.home_table_index, game.away_table_index}
			if simCampaign[game.home_table_index] != nil {
				simCampaign[game.home_table_index].add_game(score)
			}
			if simCampaign[game.away_table_index] != nil {
				simCampaign[game.away_table_index].add_game(score)
			}
		}
		for i, team := range groups {
			teamSlice[i] = simCampaign[table.Query(uint32(team.Team_id))]
		}
		sort.Sort(TeamCampaignSorted{teamSlice, order})
		season.Rank = -1
		for rank, team := range teamSlice {
			if team.id == targetTeam {
				season.Rank = rank
				break
			}
		}
		batch = append(batch, season)
	}
	return batch
}

func cemBatchSummary(seasons []CEMSeason, elite []int, target int, exact bool) CEMBatchStats {
	stats := CEMBatchStats{EliteCount: len(elite), BestRank: -1, UsedExactElites: exact}
	bestDistance := int(^uint(0) >> 1)
	for _, season := range seasons {
		stats.MeanRank += float64(season.Rank)
		if season.Rank == target {
			stats.ExactHits++
		}
		if absInt(season.Rank-target) <= 1 {
			stats.NearTargetHits++
		}
		if distance := absInt(season.Rank - target); distance < bestDistance {
			bestDistance, stats.BestRank = distance, season.Rank
		}
	}
	if len(seasons) > 0 {
		stats.MeanRank /= float64(len(seasons))
		stats.ExactRate = float64(stats.ExactHits) / float64(len(seasons))
		stats.NearTargetRate = float64(stats.NearTargetHits) / float64(len(seasons))
	}
	for _, index := range elite {
		stats.EliteMeanDistance += float64(absInt(seasons[index].Rank - target))
	}
	if len(elite) > 0 {
		stats.EliteMeanDistance /= float64(len(elite))
	}
	stats.ExactEventESS = cemEventESS(seasons, target)
	_, stats.EliteESS = cemEliteWeights(seasons, elite)
	return stats
}

func logCEMGameSignals(groupID, teamID, position, iteration int, games []*GameType,
	original []GameProposalMeans, proposal CEMProposal) float64 {
	maxAbs := 0.0
	for _, signal := range proposal.Signals {
		if !signal.Selected {
			continue
		}
		game := games[signal.GameIndex]
		if math.Abs(signal.HomeZ) > maxAbs {
			maxAbs = math.Abs(signal.HomeZ)
		}
		if math.Abs(signal.AwayZ) > maxAbs {
			maxAbs = math.Abs(signal.AwayZ)
		}
		log.Printf("rare-position-cem-game: group=%d team=%d position=%d iteration=%d game_id=%d home_team=%d away_team=%d home_ess=%.2f away_ess=%.2f home_se=%.5g away_se=%.5g home_z=%.4f away_z=%.4f game_score=%.4f elite_home_mean=%.5g elite_away_mean=%.5g original_home=%.5g proposal_home=%.5g original_away=%.5g proposal_away=%.5g",
			groupID, teamID, position, iteration, game.Id, game.HomeId, game.AwayId,
			signal.HomeESS, signal.AwayESS, signal.HomeSE, signal.AwaySE,
			signal.HomeZ, signal.AwayZ, signal.GameScore, signal.HomeEliteMean,
			signal.AwayEliteMean, original[signal.GameIndex].Home,
			proposal.Means[signal.GameIndex].Home, original[signal.GameIndex].Away,
			proposal.Means[signal.GameIndex].Away)
	}
	return maxAbs
}

func buildCEMMixture(original, learned []GameProposalMeans) []ProposalComponent {
	return []ProposalComponent{
		{Name: "original", Weight: OriginalMixtureWeight, Means: original, TargetRank: -1},
		{Name: "cem_learned", Weight: 1 - OriginalMixtureWeight, Means: learned},
	}
}

// Rank validation proposals by useful event frequency and overlap quality.
// The broad middle band is preferred over a proposal that merely generates
// many raw hits; ESS/work remains the deterministic tie-breaker.
func cemValidationPriority(pilot *WeightedPilotResult) float64 {
	if pilot == nil || pilot.Samples <= 0 || pilot.Hits <= 0 {
		return math.Inf(-1)
	}
	rate := float64(pilot.Hits) / float64(pilot.Samples)
	priority := -math.Abs(math.Log(rate / 0.015))
	if rate >= 0.005 && rate <= 0.03 {
		priority += 10
	} else if rate > 0.03 {
		priority -= 2 * math.Log(rate/0.03)
	} else {
		priority -= 2 * math.Log(0.005/rate)
	}
	priority += math.Log1p(math.Max(0, pilot.ESSPerWork) * 1e6)
	return priority
}

func cemValidationEvidence(last CEMBatchStats, maxExactHits int) (bool, string) {
	if maxExactHits >= CEMMinExactHitsForValidation {
		return true, "two_exact_hits_in_adaptation"
	}
	if last.ExactHits >= 1 && last.NearTargetRate >= CEMNearTargetRateForValidation {
		return true, "exact_hit_with_neighborhood"
	}
	if last.ExactHits == 0 && last.NearTargetRate >= CEMStrongNearTargetRate {
		return true, "strong_neighborhood"
	}
	return false, "insufficient_target_evidence"
}

func cemCandidatePriority(candidate *FrontierCandidate, searches map[int]*TeamRareSearch) int {
	search := searches[candidate.TeamID]
	score := 0
	if search.Has100PercentNormal {
		score += 1000
	}
	position := candidate.Position
	if position > 0 && search.Positions[position-1].Status == StatusObserved ||
		position+1 < len(search.Positions) && search.Positions[position+1].Status == StatusObserved {
		score += 200
	} else {
		score += 100 // internal non-contiguous gap
	}
	score -= int(math.Abs(float64(position)-search.NormalMeanRank) * 10)
	return score
}

func runCEMRound(candidates []*FrontierCandidate, searches map[int]*TeamRareSearch,
	group *GroupType, campaign []*TeamCampaign, table *Table, order []SortType,
	original []GameProposalMeans, cemRemaining, validationRemaining, remainingWork *int64,
	adaptWorkPerSample, validationWorkPerSample int64, rng *rand.Rand) CEMRoundResult {
	result := CEMRoundResult{}
	ordered := append([]*FrontierCandidate(nil), candidates...)
	sort.Slice(ordered, func(i, j int) bool {
		a, b := ordered[i], ordered[j]
		pa, pb := cemCandidatePriority(a, searches), cemCandidatePriority(b, searches)
		if pa != pb {
			return pa > pb
		}
		if a.TeamID != b.TeamID {
			return a.TeamID < b.TeamID
		}
		return a.Position < b.Position
	})
	for _, candidate := range ordered {
		if affordableSamples(CEMBatchSamples, *cemRemaining, adaptWorkPerSample) < CEMMinBatchSamples ||
			affordableSamples(CEMBatchSamples, *remainingWork, adaptWorkPerSample) < CEMMinBatchSamples {
			candidate.SearchState.Status = StatusUnexplored
			continue
		}
		result.TargetsAttempted++
		proposal := append([]GameProposalMeans(nil), original...)
		prevDistance, stalled := math.Inf(1), 0
		prevNearRate, firstDistance, bestDistance := 0.0, math.Inf(1), math.Inf(1)
		exactEliteSeen := false
		eventObserved := false
		maxExactHits := 0
		lastStats := CEMBatchStats{}
		activeGames := []int(nil)
		for iteration := 1; iteration <= CEMMaxIterations; iteration++ {
			samples := affordableSamples(CEMBatchSamples, *cemRemaining, adaptWorkPerSample)
			samples = affordableSamples(samples, *remainingWork, adaptWorkPerSample)
			if samples < CEMMinBatchSamples {
				break
			}
			batch := simulateCEMBatch(campaign, group.Games, original, proposal,
				table, order, group.Team_groups, candidate.TeamID, samples, rng)
			work := int64(samples) * adaptWorkPerSample
			*cemRemaining -= work
			*remainingWork -= work
			result.CEMWork += work
			candidate.SearchState.SearchWorkSpent += work
			result.Iterations++
			elite, exact := cemEliteIndices(batch, candidate.Position, candidate.Direction)
			stats := cemBatchSummary(batch, elite, candidate.Position, exact)
			lastStats = stats
			if math.IsInf(firstDistance, 1) {
				firstDistance = stats.EliteMeanDistance
			}
			if stats.EliteMeanDistance < bestDistance {
				bestDistance = stats.EliteMeanDistance
			}
			eventObserved = eventObserved || stats.ExactHits > 0
			if stats.ExactHits > maxExactHits {
				maxExactHits = stats.ExactHits
			}
			exactEliteSeen = exactEliteSeen || exact
			updated := cemUpdateSparse(proposal, original, group.Games, batch, elite, activeGames)
			updated.Iteration = iteration
			proposal = updated.Means
			activeGames = updated.SelectedGames
			maxSignal := logCEMGameSignals(group.Id, candidate.TeamID, candidate.Position,
				iteration, group.Games, original, updated)
			result.SelectedGames += len(updated.SelectedGames)
			if len(updated.SelectedGames) > result.MaxSelectedGames {
				result.MaxSelectedGames = len(updated.SelectedGames)
			}
			log.Printf("rare-position-cem: group=%d team=%d position=%d iteration=%d samples=%d exact_hits=%d exact_hit_rate=%.4f near_target_rate=%.4f elite_count=%d elite_ess=%.2f exact_elites=%t mean_rank=%.3f best_rank=%d elite_mean_distance=%.3f kl=%.3f selected_games=%d effective_changed_games=%d max_abs_z=%.3f ess_exact_events=%.2f",
				group.Id, candidate.TeamID, candidate.Position, iteration, samples, stats.ExactHits,
				stats.ExactRate, stats.NearTargetRate, stats.EliteCount, stats.EliteESS,
				stats.UsedExactElites, stats.MeanRank, stats.BestRank,
				stats.EliteMeanDistance, updated.KL, len(updated.SelectedGames),
				updated.ChangedGames, maxSignal,
				stats.ExactEventESS)
			if exact || stats.ExactEventESS >= MinPilotESSForProduction {
				break
			}
			if prevDistance < math.Inf(1) {
				distanceImprovement := (prevDistance - stats.EliteMeanDistance) / math.Max(1, prevDistance)
				nearImprovement := stats.NearTargetRate - prevNearRate
				if distanceImprovement < CEMMinRelativeProgress && nearImprovement < CEMMinRelativeProgress {
					stalled++
				} else {
					stalled = 0
				}
				if stalled >= CEMMaxStalledIterations {
					break
				}
			}
			prevDistance = stats.EliteMeanDistance
			prevNearRate = stats.NearTargetRate
		}
		if eventObserved {
			result.TargetsAnyExact++
		}
		if exactEliteSeen {
			result.TargetsExactElite++
		}
		if firstDistance-bestDistance < CEMMinRelativeProgress*math.Max(1, firstDistance) &&
			lastStats.NearTargetRate < CEMStrongNearTargetRate && maxExactHits == 0 {
			candidate.SearchState.Status = StatusExhausted
			log.Printf("rare-position-cem-selection: group=%d team=%d position=%d reason=no_rank_progress cem_work=%d",
				group.Id, candidate.TeamID, candidate.Position, candidate.SearchState.SearchWorkSpent)
			continue
		}
		validate, reason := cemValidationEvidence(lastStats, maxExactHits)
		if !validate {
			candidate.SearchState.Status = StatusExhausted
			log.Printf("rare-position-cem-selection: group=%d team=%d position=%d reason=%s exact_hits=%d max_exact_hits=%d near_target_rate=%.4f",
				group.Id, candidate.TeamID, candidate.Position, reason, lastStats.ExactHits,
				maxExactHits, lastStats.NearTargetRate)
			continue
		}
		if *remainingWork < int64(CEMMinBatchSamples)*validationWorkPerSample ||
			*validationRemaining < int64(CEMMinBatchSamples)*validationWorkPerSample {
			candidate.SearchState.Status = StatusExhausted
			continue
		}
		samples := affordableSamples(CEMValidationSamples, *validationRemaining, validationWorkPerSample)
		samples = affordableSamples(samples, *remainingWork, validationWorkPerSample)
		mixture := buildCEMMixture(original, proposal)
		validateProposalMixture(mixture, len(group.Games))
		pilot := &WeightedPilotResult{Proposal: SearchProposal{
			Name:      fmt.Sprintf("cem_team%d_rank%d", candidate.TeamID, candidate.Position),
			Direction: candidate.Direction, TargetRank: candidate.Position, Components: mixture,
		}}
		// Independent draws: none of the CEM adaptation seasons enter this pilot.
		evaluateWeightedPilot(pilot, campaign, group.Games, original, table, order,
			group.Team_groups, candidate.TeamID, candidate.Position, samples,
			validationWorkPerSample, rng, group.Id)
		pilot.Refined = true
		work := int64(samples) * validationWorkPerSample
		*validationRemaining -= work
		*remainingWork -= work
		result.ValidationWork += work
		candidate.SearchState.SearchWorkSpent += work
		candidate.SearchState.Pilots = []*WeightedPilotResult{pilot}
		priority := cemValidationPriority(pilot)
		log.Printf("rare-position-cem-validation: group=%d team=%d position=%d reason=%s samples=%d hits=%d p=%.6g se=%.3g relSE=%.3f ess=%.2f ess_per_million_work=%.3f max_event_weight_share=%.3f priority=%.3f",
			group.Id, candidate.TeamID, candidate.Position, reason, pilot.Samples, pilot.Hits,
			pilot.Probability, pilot.StdErr, pilot.RelSE, pilot.ESS,
			pilot.ESSPerWork*1e6, pilot.MaxEventWeightShare, priority)
		if selectPilotMixture([]*WeightedPilotResult{pilot}) == nil {
			candidate.SearchState.Status = StatusExhausted
			continue
		}
		candidate.SearchState.Status = StatusPromising
		candidate.SearchState.BestProposal = &pilot.Proposal
		candidate.SearchState.BestPilot = pilot
		result.TargetsValidated++
		result.ValidatedESS += pilot.ESS
		result.Eligible = append(result.Eligible, candidate)
	}
	return result
}
