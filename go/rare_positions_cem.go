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
	CEMMinEliteESSForUpdate        = 8.0
	CEMMeaningfulTeamLogShift      = 0.03
	CEMThetaStabilityThreshold     = 0.02
	CEMMinMultiplier               = 1e-12
	CEMMinExactHitsForValidation   = 2
	CEMNearTargetRateForValidation = 0.05
	CEMStrongNearTargetRate        = 0.10
	CEMMinBatchSamples             = 100
	CEMMinRelativeProgress         = 0.02
	CEMMaxStalledIterations        = 2
)

type CEMProposal struct {
	TeamLogMultipliers map[int]float64
	PreviousLogTheta   map[int]float64
	EliteTeamGoals     map[int]float64
	Means              []GameProposalMeans
	Iteration          int
	KL                 float64
	ChangedTeams       int
	MaxAbsTheta        float64
	ThetaDeltaL2       float64
	EliteESS           float64
	UpdateAllowed      bool
}

type CEMTeamSignal struct {
	TeamID        int
	OriginalGoals float64
	EliteGoals    float64
	OldMultiplier float64
	NewMultiplier float64
	Theta         float64
}

type CEMSeason struct {
	Rank      int
	TeamGoals []int
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
	ChangedTeams      int
	MaxChangedTeams   int
	AbsThetaSum       float64
	MaxAbsTheta       float64
	ThetaDeltaL2Sum   float64
	ValidatedESS      float64
}

func teamIDsFromGroups(groups []TeamType) []int {
	ids := make([]int, len(groups))
	for i, group := range groups {
		ids[i] = group.Team_id
	}
	sort.Ints(ids)
	return ids
}

func newCEMProposal(original []GameProposalMeans, games []*GameType, teamIDs []int) CEMProposal {
	theta := make(map[int]float64, len(teamIDs))
	for _, teamID := range teamIDs {
		theta[teamID] = 0
	}
	proposal := CEMProposal{TeamLogMultipliers: theta, UpdateAllowed: true}
	proposal.Means = materializeCEMProposal(proposal, original, games)
	return proposal
}

func materializeCEMProposal(proposal CEMProposal, original []GameProposalMeans, games []*GameType) []GameProposalMeans {
	means := append([]GameProposalMeans(nil), original...)
	for i, game := range games {
		if game.Played {
			continue
		}
		homeTheta := proposal.TeamLogMultipliers[game.HomeId]
		awayTheta := proposal.TeamLogMultipliers[game.AwayId]
		if original[i].Home > 0 {
			means[i].Home = original[i].Home * math.Exp(homeTheta)
		} else {
			means[i].Home = 0
		}
		if original[i].Away > 0 {
			means[i].Away = original[i].Away * math.Exp(awayTheta)
		} else {
			means[i].Away = 0
		}
	}
	return means
}

func teamOriginalGoals(original []GameProposalMeans, games []*GameType, teamIDs []int) map[int]float64 {
	goals := make(map[int]float64, len(teamIDs))
	for _, teamID := range teamIDs {
		goals[teamID] = 0
	}
	for i, game := range games {
		if game.Played {
			continue
		}
		if _, ok := goals[game.HomeId]; ok {
			goals[game.HomeId] += original[i].Home
		}
		if _, ok := goals[game.AwayId]; ok {
			goals[game.AwayId] += original[i].Away
		}
	}
	return goals
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

func cemSmoothLogTheta(current, estimate float64) float64 {
	if math.IsNaN(current) || math.IsInf(current, 0) {
		current = 0
	}
	if math.IsNaN(estimate) || math.IsInf(estimate, 0) {
		estimate = 0
	}
	return (1-CEMSmoothing)*current + CEMSmoothing*estimate
}

func cemTrustRegionTeam(proposal CEMProposal, original []GameProposalMeans, games []*GameType, maxKL float64) CEMProposal {
	if proposal.KL <= maxKL {
		return proposal
	}
	lo, hi := 0.0, 1.0
	for i := 0; i < 55; i++ {
		mid := (lo + hi) / 2
		candidate := proposal
		candidate.TeamLogMultipliers = make(map[int]float64, len(proposal.TeamLogMultipliers))
		for teamID, theta := range proposal.TeamLogMultipliers {
			candidate.TeamLogMultipliers[teamID] = mid * theta
		}
		candidate.Means = materializeCEMProposal(candidate, original, games)
		candidate.KL = cemTotalKL(candidate.Means, original, games)
		if candidate.KL <= maxKL {
			lo = mid
		} else {
			hi = mid
		}
	}
	scaled := proposal
	scaled.TeamLogMultipliers = make(map[int]float64, len(proposal.TeamLogMultipliers))
	for teamID, theta := range proposal.TeamLogMultipliers {
		scaled.TeamLogMultipliers[teamID] = lo * theta
	}
	scaled.Means = materializeCEMProposal(scaled, original, games)
	scaled.KL = cemTotalKL(scaled.Means, original, games)
	return scaled
}

func cemRankDistance(rank, target int) int { return absInt(rank - target) }

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

func copyTheta(theta map[int]float64) map[int]float64 {
	copy := make(map[int]float64, len(theta))
	for teamID, value := range theta {
		copy[teamID] = value
	}
	return copy
}

func cemUpdateTeam(current CEMProposal, original []GameProposalMeans, games []*GameType,
	teamIDs []int, seasons []CEMSeason, elite []int) CEMProposal {
	weights, eliteESS := cemEliteWeights(seasons, elite)
	updated := current
	updated.TeamLogMultipliers = copyTheta(current.TeamLogMultipliers)
	updated.EliteESS = eliteESS
	if len(elite) == 0 || eliteESS < CEMMinEliteESSForUpdate {
		updated.UpdateAllowed = false
		return updated
	}
	lambda := teamOriginalGoals(original, games, teamIDs)
	oldTheta := copyTheta(current.TeamLogMultipliers)
	updated.PreviousLogTheta = oldTheta
	updated.EliteTeamGoals = make(map[int]float64, len(teamIDs))
	for index, teamID := range teamIDs {
		if index >= len(seasons[elite[0]].TeamGoals) || lambda[teamID] <= 0 {
			updated.TeamLogMultipliers[teamID] = 0
			updated.EliteTeamGoals[teamID] = 0
			continue
		}
		goals := make([]float64, len(elite))
		for j, seasonIndex := range elite {
			goals[j] = float64(seasons[seasonIndex].TeamGoals[index])
		}
		eliteGoals := cemWeightedMean(goals, weights)
		updated.EliteTeamGoals[teamID] = eliteGoals
		rawMultiplier := eliteGoals / lambda[teamID]
		if rawMultiplier < CEMMinMultiplier || math.IsNaN(rawMultiplier) || math.IsInf(rawMultiplier, 0) {
			rawMultiplier = CEMMinMultiplier
		}
		updated.TeamLogMultipliers[teamID] = cemSmoothLogTheta(oldTheta[teamID], math.Log(rawMultiplier))
	}
	updated.Means = materializeCEMProposal(updated, original, games)
	updated.KL = cemTotalKL(updated.Means, original, games)
	updated = cemTrustRegionTeam(updated, original, games, CEMMaxKL)
	updated.UpdateAllowed = true
	updated.ChangedTeams = 0
	updated.MaxAbsTheta = 0
	updated.ThetaDeltaL2 = 0
	for _, teamID := range teamIDs {
		theta := updated.TeamLogMultipliers[teamID]
		if math.Abs(theta) >= CEMMeaningfulTeamLogShift {
			updated.ChangedTeams++
		}
		if math.Abs(theta) > updated.MaxAbsTheta {
			updated.MaxAbsTheta = math.Abs(theta)
		}
		delta := theta - oldTheta[teamID]
		updated.ThetaDeltaL2 += delta * delta
	}
	updated.ThetaDeltaL2 = math.Sqrt(updated.ThetaDeltaL2)
	return updated
}

func simulateCEMBatchForTeams(base []*TeamCampaign, games []*GameType, original, proposal []GameProposalMeans,
	table *Table, order []SortType, groups []TeamType, teamIDs []int, targetTeam, samples int, rng *rand.Rand) []CEMSeason {
	teamIndex := make(map[int]int, len(teamIDs))
	for i, teamID := range teamIDs {
		teamIndex[teamID] = i
	}
	batch := make([]CEMSeason, 0, samples)
	simCampaign := make([]*TeamCampaign, len(base))
	teamSlice := make([]*TeamCampaign, len(groups))
	for n := 0; n < samples; n++ {
		for i, campaign := range base {
			simCampaign[i] = campaign.clone()
		}
		season := CEMSeason{TeamGoals: make([]int, len(teamIDs))}
		for i, game := range games {
			if game.Played {
				continue
			}
			home := poissonRand(rng, proposal[i].Home)
			away := poissonRand(rng, proposal[i].Away)
			if index, ok := teamIndex[game.HomeId]; ok {
				season.TeamGoals[index] += home
			}
			if index, ok := teamIndex[game.AwayId]; ok {
				season.TeamGoals[index] += away
			}
			// Likelihood remains the exact product of game-level P/Q ratios.
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

func logCEMTeamChanges(groupID, targetTeam, position, iteration int, teamIDs []int,
	original []GameProposalMeans, games []*GameType, proposal CEMProposal) {
	lambda := teamOriginalGoals(original, games, teamIDs)
	signals := make([]CEMTeamSignal, 0, len(teamIDs))
	for _, teamID := range teamIDs {
		theta := proposal.TeamLogMultipliers[teamID]
		oldTheta := proposal.PreviousLogTheta[teamID]
		eliteGoals, ok := proposal.EliteTeamGoals[teamID]
		if !ok {
			eliteGoals = lambda[teamID] * math.Exp(theta)
		}
		signals = append(signals, CEMTeamSignal{TeamID: teamID, OriginalGoals: lambda[teamID],
			EliteGoals: eliteGoals, OldMultiplier: math.Exp(oldTheta),
			NewMultiplier: math.Exp(theta), Theta: theta})
	}
	sort.Slice(signals, func(i, j int) bool {
		if math.Abs(signals[i].Theta) != math.Abs(signals[j].Theta) {
			return math.Abs(signals[i].Theta) > math.Abs(signals[j].Theta)
		}
		return signals[i].TeamID < signals[j].TeamID
	})
	for i := 0; i < len(signals) && i < 10; i++ {
		signal := signals[i]
		log.Printf("rare-position-cem-team: group=%d target_team=%d target_position=%d iteration=%d team_id=%d original_expected_remaining_goals=%.5g elite_expected_remaining_goals=%.5g old_multiplier=%.5g new_multiplier=%.5g theta=%.5g",
			groupID, targetTeam, position, iteration, signal.TeamID, signal.OriginalGoals,
			signal.EliteGoals, signal.OldMultiplier, signal.NewMultiplier, signal.Theta)
	}
}

func buildCEMMixture(original, learned []GameProposalMeans) []ProposalComponent {
	return []ProposalComponent{
		{Name: "original", Weight: OriginalMixtureWeight, Means: original, TargetRank: -1},
		{Name: "cem_team_level", Weight: 1 - OriginalMixtureWeight, Means: learned},
	}
}

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
		score += 100
	}
	score -= int(math.Abs(float64(position)-search.NormalMeanRank) * 10)
	return score
}

func sumAbsTheta(theta map[int]float64) float64 {
	total := 0.0
	for _, value := range theta {
		total += math.Abs(value)
	}
	return total
}

func runCEMRound(candidates []*FrontierCandidate, searches map[int]*TeamRareSearch,
	group *GroupType, campaign []*TeamCampaign, table *Table, order []SortType,
	original []GameProposalMeans, cemRemaining, validationRemaining, remainingWork *int64,
	adaptWorkPerSample, validationWorkPerSample int64, rng *rand.Rand) CEMRoundResult {
	result := CEMRoundResult{}
	teamIDs := teamIDsFromGroups(group.Team_groups)
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
		proposal := newCEMProposal(original, group.Games, teamIDs)
		prevDistance, prevNearRate, stalled := math.Inf(1), 0.0, 0
		firstDistance, bestDistance := math.Inf(1), math.Inf(1)
		exactEliteSeen, eventObserved := false, false
		maxExactHits := 0
		lastStats := CEMBatchStats{}
		stableIterations, lowESS := 0, false
		candidateIterations := 0
		for iteration := 1; iteration <= CEMMaxIterations; iteration++ {
			samples := affordableSamples(CEMBatchSamples, *cemRemaining, adaptWorkPerSample)
			samples = affordableSamples(samples, *remainingWork, adaptWorkPerSample)
			if samples < CEMMinBatchSamples {
				break
			}
			batch := simulateCEMBatchForTeams(campaign, group.Games, original, proposal.Means,
				table, order, group.Team_groups, teamIDs, candidate.TeamID, samples, rng)
			work := int64(samples) * adaptWorkPerSample
			*cemRemaining -= work
			*remainingWork -= work
			result.CEMWork += work
			candidate.SearchState.SearchWorkSpent += work
			result.Iterations++
			candidateIterations++
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
			updated := cemUpdateTeam(proposal, original, group.Games, teamIDs, batch, elite)
			updated.Iteration = iteration
			if !updated.UpdateAllowed {
				lowESS = true
				log.Printf("rare-position-cem-selection: group=%d team=%d position=%d reason=low_elite_ess elite_ess=%.2f threshold=%.2f",
					group.Id, candidate.TeamID, candidate.Position, updated.EliteESS, CEMMinEliteESSForUpdate)
				break
			}
			if updated.ThetaDeltaL2 < CEMThetaStabilityThreshold {
				stableIterations++
			} else {
				stableIterations = 0
			}
			proposal = updated
			result.ChangedTeams += updated.ChangedTeams
			if updated.ChangedTeams > result.MaxChangedTeams {
				result.MaxChangedTeams = updated.ChangedTeams
			}
			result.AbsThetaSum += sumAbsTheta(updated.TeamLogMultipliers)
			if updated.MaxAbsTheta > result.MaxAbsTheta {
				result.MaxAbsTheta = updated.MaxAbsTheta
			}
			result.ThetaDeltaL2Sum += updated.ThetaDeltaL2
			logCEMTeamChanges(group.Id, candidate.TeamID, candidate.Position, iteration,
				teamIDs, original, group.Games, updated)
			log.Printf("rare-position-cem: parameterization=team_level group=%d team=%d position=%d iteration=%d samples=%d exact_hits=%d exact_hit_rate=%.4f near_target_rate=%.4f elite_count=%d elite_ess=%.2f exact_elites=%t mean_rank=%.3f best_rank=%d elite_mean_distance=%.3f kl=%.3f changed_teams=%d max_abs_team_theta=%.4f theta_delta_l2=%.4f",
				group.Id, candidate.TeamID, candidate.Position, iteration, samples, stats.ExactHits,
				stats.ExactRate, stats.NearTargetRate, stats.EliteCount, stats.EliteESS,
				stats.UsedExactElites, stats.MeanRank, stats.BestRank, stats.EliteMeanDistance,
				updated.KL, updated.ChangedTeams, updated.MaxAbsTheta, updated.ThetaDeltaL2)
			if exact || stats.ExactEventESS >= MinPilotESSForProduction || stableIterations >= 2 {
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
			prevDistance, prevNearRate = stats.EliteMeanDistance, stats.NearTargetRate
		}
		if eventObserved {
			result.TargetsAnyExact++
		}
		if exactEliteSeen {
			result.TargetsExactElite++
		}
		if lowESS || candidateIterations == 0 {
			candidate.SearchState.Status = StatusExhausted
			continue
		}
		if firstDistance-bestDistance < CEMMinRelativeProgress*math.Max(1, firstDistance) &&
			lastStats.NearTargetRate < CEMStrongNearTargetRate && maxExactHits == 0 {
			candidate.SearchState.Status = StatusExhausted
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
		mixture := buildCEMMixture(original, proposal.Means)
		validateProposalMixture(mixture, len(group.Games))
		pilot := &WeightedPilotResult{Proposal: SearchProposal{
			Name:      fmt.Sprintf("cem_team_level_team%d_rank%d", candidate.TeamID, candidate.Position),
			Direction: candidate.Direction, TargetRank: candidate.Position, Components: mixture,
		}}
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
