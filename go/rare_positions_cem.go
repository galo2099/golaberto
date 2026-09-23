package main

import (
	"fmt"
	"log"
	"math"
	"math/rand"
	"sort"
)

const (
	CEMBatchSamples                     = 300
	CEMMaxIterationsPerCandidate        = 12
	CEMEliteFraction                    = 0.15
	CEMSmoothing                        = 0.5
	CEMMaxKL                            = 3.0
	CEMExactEventThreshold              = 5
	CEMValidationSamples                = 500
	CEMConfirmationSamples              = 300
	CEMMinAdaptationHitsForConfirmation = 1
	CEMMinConfirmationHits              = 1
	CEMMaxInitialCandidates             = 6
	CEMConfirmationMaxEventShare        = 0.95
	CEMMaxExplorationFraction           = 0.40
	MaxCEMWorkFraction                  = 0.10
	MaxCEMValidationWorkFraction        = 0.02
	MaxCEMConfirmationWorkFraction      = 0.03
	MaxCEMPlainEquivalentSamples        = 5000
	CEMMinEliteESSForUpdate             = 8.0
	CEMMeaningfulTeamLogShift           = 0.03
	CEMThetaStabilityThreshold          = 0.02
	CEMMinMultiplier                    = 1e-12
	CEMMinBatchSamples                  = 100
	CEMMinRelativeProgress              = 0.02
	CEMMaxStalledIterations             = 2
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
	Eligible                         []*FrontierCandidate
	CEMWork                          int64
	ValidationWork                   int64
	ConfirmationWork                 int64
	ConfirmationAttempts             int
	ConfirmationSuccesses            int
	ConfirmationFailures             int
	AdaptationExactHitBatches        int
	ConfirmationReadyCandidates      int
	ConfirmationSingleHitSuccesses   int
	ConfirmationMultiHitSuccesses    int
	ConfirmationFailedResumed        int
	ConfirmationFailedExhausted      int
	ConfirmationHits                 int
	ValidationAttempts               int
	ValidationSuccesses              int
	CandidatesTotal                  int
	CandidatesAdmitted               int
	CandidatesNotAdmitted            int
	ExplorationSamples               int
	AdaptiveSamples                  int
	TargetsAttempted                 int
	Iterations                       int
	TargetsAnyExact                  int
	TargetsExactElite                int
	TargetsValidated                 int
	ChangedTeams                     int
	MaxChangedTeams                  int
	AbsThetaSum                      float64
	ThetaUpdates                     int
	ThetaParameterCount              int
	MaxAbsTheta                      float64
	ThetaDeltaL2Sum                  float64
	ValidatedESS                     float64
	SchedulerBatches                 int
	OneBatchCandidates               int
	MultiBatchCandidates             int
	MaxBatchesPerCandidate           int
	AverageBatchesPerCandidate       float64
	BestCandidateTeam                int
	BestCandidatePosition            int
	BestCandidateBatches             int
	BestCandidateDistanceImprovement float64
	BestCandidateNearTargetRate      float64
	HighestNearTeam                  int
	HighestNearPosition              int
	HighestNearBatches               int
	HighestNearRate                  float64
	StopValidationReady              int
	StopStalled                      int
	StopLowEliteESS                  int
	StopRegression                   int
	StopPerCandidateCap              int
	StopGlobalBudget                 int
	Candidates                       []CEMCandidateState
}

type CEMCandidateState struct {
	Candidate                      *FrontierCandidate
	Proposal                       CEMProposal
	BestProposal                   CEMProposal
	Iterations                     int
	Samples                        int
	WorkSpent                      int64
	FirstStats                     CEMBatchStats
	LastStats                      CEMBatchStats
	BestStats                      CEMBatchStats
	BestBatch                      int
	BestEliteDistance              float64
	BestNearTargetRate             float64
	PreviousStats                  CEMBatchStats
	HasStats                       bool
	HasBest                        bool
	ConfirmationAccepted           bool
	MaxExactHits                   int
	AnyExactHit                    bool
	ExactEliteSeen                 bool
	StalledIterations              int
	RegressionIterations           int
	StableIterations               int
	Active                         bool
	StopReason                     string
	ProgressScore                  float64
	Confirmation                   CEMConfirmationStats
	ConfirmationProposal           CEMProposal
	ConfirmationBatchStats         CEMBatchStats
	ConfirmationSourceBatch        int
	HasConfirmationCandidate       bool
	LastFailedConfirmationProposal CEMProposal
	HasFailedConfirmationProposal  bool
	AdmissionReason                string
}

type CEMConfirmationStats struct {
	Samples             int
	Hits                int
	HitRate             float64
	NearTargetRate      float64
	Probability         float64
	StdErr              float64
	RelSE               float64
	EventESS            float64
	MaxEventWeightShare float64
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

func cemValidationMixture(original []GameProposalMeans, state *CEMCandidateState) []ProposalComponent {
	return buildCEMMixture(original, state.ConfirmationProposal.Means)
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

func cemThetaSummary(absThetaSum float64, updates, parameterCount int) (averageL1, averageAbs float64) {
	if updates > 0 {
		averageL1 = absThetaSum / float64(updates)
	}
	if parameterCount > 0 {
		averageAbs = absThetaSum / float64(parameterCount)
	}
	return averageL1, averageAbs
}

func cloneCEMProposal(proposal CEMProposal) CEMProposal {
	copy := proposal
	copy.TeamLogMultipliers = copyTheta(proposal.TeamLogMultipliers)
	copy.PreviousLogTheta = copyTheta(proposal.PreviousLogTheta)
	copy.EliteTeamGoals = make(map[int]float64, len(proposal.EliteTeamGoals))
	for teamID, goals := range proposal.EliteTeamGoals {
		copy.EliteTeamGoals[teamID] = goals
	}
	copy.Means = append([]GameProposalMeans(nil), proposal.Means...)
	return copy
}

func cemSnapshotBetter(stats CEMBatchStats, proposal CEMProposal, bestStats CEMBatchStats, bestProposal CEMProposal, hasBest bool) bool {
	if !hasBest {
		return true
	}
	if stats.ExactHits != bestStats.ExactHits {
		return stats.ExactHits > bestStats.ExactHits
	}
	if stats.NearTargetRate != bestStats.NearTargetRate {
		return stats.NearTargetRate > bestStats.NearTargetRate
	}
	if stats.EliteMeanDistance != bestStats.EliteMeanDistance {
		return stats.EliteMeanDistance < bestStats.EliteMeanDistance
	}
	if stats.EliteESS != bestStats.EliteESS {
		return stats.EliteESS > bestStats.EliteESS
	}
	return proposal.KL < bestProposal.KL
}

func cemValidationSnapshotBetter(a *CEMCandidateState, b *CEMCandidateState) bool {
	if a.MaxExactHits != b.MaxExactHits {
		return a.MaxExactHits > b.MaxExactHits
	}
	if a.ConfirmationBatchStats.NearTargetRate != b.ConfirmationBatchStats.NearTargetRate {
		return a.ConfirmationBatchStats.NearTargetRate > b.ConfirmationBatchStats.NearTargetRate
	}
	if a.ConfirmationBatchStats.ExactEventESS != b.ConfirmationBatchStats.ExactEventESS {
		return a.ConfirmationBatchStats.ExactEventESS > b.ConfirmationBatchStats.ExactEventESS
	}
	if a.ConfirmationBatchStats.EliteESS != b.ConfirmationBatchStats.EliteESS {
		return a.ConfirmationBatchStats.EliteESS > b.ConfirmationBatchStats.EliteESS
	}
	if a.ConfirmationProposal.KL != b.ConfirmationProposal.KL {
		return a.ConfirmationProposal.KL < b.ConfirmationProposal.KL
	}
	if a.Candidate.TeamID != b.Candidate.TeamID {
		return a.Candidate.TeamID < b.Candidate.TeamID
	}
	return a.Candidate.Position < b.Candidate.Position
}

func updateCEMProgressScore(state *CEMCandidateState) float64 {
	if !state.HasStats {
		return 0
	}
	// Prioritize any exact event (+1000), then near-target mass, normalized
	// best and recent distance progress; repeated stalls and regressions lower
	// the score. Scores are deterministic, with explicit candidate tie-breaks.
	initialDistance := math.Max(1, state.FirstStats.EliteMeanDistance)
	bestDistanceGain := (state.FirstStats.EliteMeanDistance - state.BestEliteDistance) / initialDistance
	recentDistanceGain := 0.0
	recentNearGain := 0.0
	if state.Iterations > 1 {
		recentDistanceGain = (state.PreviousStats.EliteMeanDistance - state.LastStats.EliteMeanDistance) / initialDistance
		recentNearGain = state.LastStats.NearTargetRate - state.PreviousStats.NearTargetRate
	}
	score := 5*bestDistanceGain + 8*recentDistanceGain + 20*state.BestNearTargetRate +
		10*state.LastStats.NearTargetRate + 2*math.Min(1, state.LastStats.EliteESS/30)
	if state.BestStats.BestRank >= 0 && cemRankDistance(state.BestStats.BestRank, state.Candidate.Position) <= 1 {
		score += 40
	}
	if state.AnyExactHit {
		score += 1000 + 10*float64(state.MaxExactHits)
	}
	if state.BestNearTargetRate == 0 && state.BestEliteDistance > 1 {
		score -= 8
	}
	if state.Iterations > 1 {
		score += 15 * recentNearGain
	}
	score -= 20 * float64(state.StalledIterations)
	score -= 12 * float64(state.RegressionIterations)
	return score
}

func cemCandidateStopReason(state *CEMCandidateState) string {
	switch {
	case state.HasConfirmationCandidate:
		return "confirmation_ready"
	case state.StalledIterations >= CEMMaxStalledIterations:
		return "stalled"
	case state.RegressionIterations >= CEMMaxStalledIterations:
		return "regression"
	case state.Iterations >= CEMMaxIterationsPerCandidate:
		return "per_candidate_cap"
	default:
		return ""
	}
}

func snapshotCEMConfirmationCandidate(state *CEMCandidateState, sampled CEMProposal,
	stats CEMBatchStats, sourceBatch int) bool {
	if stats.ExactHits < CEMMinAdaptationHitsForConfirmation {
		return false
	}
	if state.HasFailedConfirmationProposal && sameCEMProposal(sampled, state.LastFailedConfirmationProposal) {
		return false
	}
	state.ConfirmationProposal = cloneCEMProposal(sampled)
	state.ConfirmationBatchStats = stats
	state.ConfirmationSourceBatch = sourceBatch
	state.HasConfirmationCandidate = true
	state.Confirmation = CEMConfirmationStats{}
	state.ConfirmationAccepted = false
	state.Active = false
	state.StopReason = "confirmation_ready"
	return true
}

func sameCEMProposal(a, b CEMProposal) bool {
	if len(a.TeamLogMultipliers) != len(b.TeamLogMultipliers) {
		return false
	}
	for teamID, theta := range a.TeamLogMultipliers {
		if b.TeamLogMultipliers[teamID] != theta {
			return false
		}
	}
	return true
}

func updateCEMStallCounters(state *CEMCandidateState, stats CEMBatchStats) {
	if state.Iterations <= 1 {
		return
	}
	distanceImprovement := state.PreviousStats.EliteMeanDistance - stats.EliteMeanDistance
	nearImprovement := stats.NearTargetRate - state.PreviousStats.NearTargetRate
	if distanceImprovement < CEMMinRelativeProgress*math.Max(1, state.PreviousStats.EliteMeanDistance) &&
		nearImprovement < CEMMinRelativeProgress && stats.ExactHits == 0 {
		state.StalledIterations++
	} else {
		state.StalledIterations = 0
	}
	if state.HasBest && stats.EliteMeanDistance > state.BestEliteDistance+
		CEMMinRelativeProgress*math.Max(1, state.BestEliteDistance) &&
		stats.NearTargetRate < state.BestNearTargetRate && stats.ExactHits == 0 {
		state.RegressionIterations++
	} else if !state.HasBest || stats.EliteMeanDistance <= state.BestEliteDistance ||
		stats.NearTargetRate >= state.BestNearTargetRate || stats.ExactHits > 0 {
		state.RegressionIterations = 0
	}
}

func selectCEMCandidate(states []*CEMCandidateState, searches map[int]*TeamRareSearch) *CEMCandidateState {
	var best *CEMCandidateState
	for _, state := range states {
		if !state.Active || state.HasConfirmationCandidate {
			continue
		}
		if best == nil || state.ProgressScore > best.ProgressScore {
			best = state
			continue
		}
		if state.ProgressScore == best.ProgressScore {
			pa, pb := cemCandidatePriority(state.Candidate, searches), cemCandidatePriority(best.Candidate, searches)
			if pa > pb || (pa == pb && (state.Candidate.TeamID < best.Candidate.TeamID ||
				state.Candidate.TeamID == best.Candidate.TeamID && state.Candidate.Position < best.Candidate.Position)) {
				best = state
			}
		}
	}
	return best
}

func logCEMStop(groupID int, state *CEMCandidateState) {
	if state.StopReason == "" {
		return
	}
	bestDistance, bestNear := math.Inf(1), 0.0
	if state.HasStats {
		bestDistance = state.BestEliteDistance
		bestNear = state.BestNearTargetRate
	}
	log.Printf("rare-position-cem-stop: group=%d team=%d position=%d iterations=%d reason=%s best_elite_distance=%.3f best_near_target_rate=%.4f max_exact_hits=%d cem_work=%d",
		groupID, state.Candidate.TeamID, state.Candidate.Position, state.Iterations,
		state.StopReason, bestDistance, bestNear, state.MaxExactHits, state.WorkSpent)
}

func runCEMCandidateBatch(state *CEMCandidateState, group *GroupType, campaign []*TeamCampaign,
	table *Table, order []SortType, original []GameProposalMeans, teamIDs []int,
	cemRemaining, remainingWork, phaseRemaining *int64, workPerSample int64, rng *rand.Rand,
	result *CEMRoundResult, step int, reason string) bool {
	samples := affordableSamples(CEMBatchSamples, *cemRemaining, workPerSample)
	samples = affordableSamples(samples, *remainingWork, workPerSample)
	if phaseRemaining != nil {
		samples = affordableSamples(samples, *phaseRemaining, workPerSample)
	}
	if samples < CEMMinBatchSamples {
		return false
	}
	work := int64(samples) * workPerSample
	log.Printf("rare-position-cem-schedule: group=%d step=%d team=%d position=%d iteration=%d progress_score=%.4f initial_elite_distance=%.3f current_elite_distance=%.3f best_elite_distance=%.3f near_target_rate=%.4f best_near_target_rate=%.4f exact_hits=%d max_exact_hits=%d elite_ess=%.2f stalled_iterations=%d work_remaining=%d reason=%s",
		group.Id, step, state.Candidate.TeamID, state.Candidate.Position, state.Iterations+1,
		state.ProgressScore, state.FirstStats.EliteMeanDistance, state.LastStats.EliteMeanDistance,
		state.BestEliteDistance, state.LastStats.NearTargetRate, state.BestNearTargetRate,
		state.LastStats.ExactHits, state.MaxExactHits, state.LastStats.EliteESS,
		state.StalledIterations, *cemRemaining, reason)

	sampledProposal := cloneCEMProposal(state.Proposal)
	batch := simulateCEMBatchForTeams(campaign, group.Games, original, sampledProposal.Means,
		table, order, group.Team_groups, teamIDs, state.Candidate.TeamID, samples, rng)
	*cemRemaining -= work
	*remainingWork -= work
	if phaseRemaining != nil {
		*phaseRemaining -= work
		result.ExplorationSamples += samples
	} else {
		result.AdaptiveSamples += samples
	}
	result.CEMWork += work
	result.SchedulerBatches++
	result.Iterations++
	state.Samples += samples
	state.WorkSpent += work
	state.Candidate.SearchState.SearchWorkSpent += work
	state.Iterations++

	elite, exact := cemEliteIndices(batch, state.Candidate.Position, state.Candidate.Direction)
	stats := cemBatchSummary(batch, elite, state.Candidate.Position, exact)
	if state.Iterations == 1 {
		state.FirstStats = stats
		state.BestEliteDistance = stats.EliteMeanDistance
		state.BestNearTargetRate = stats.NearTargetRate
	} else {
		state.PreviousStats = state.LastStats
	}
	state.LastStats = stats
	state.HasStats = true
	state.AnyExactHit = state.AnyExactHit || stats.ExactHits > 0
	if stats.ExactHits > state.MaxExactHits {
		state.MaxExactHits = stats.ExactHits
	}
	state.ExactEliteSeen = state.ExactEliteSeen || exact
	if stats.ExactHits >= CEMMinAdaptationHitsForConfirmation {
		result.AdaptationExactHitBatches++
	}
	newConfirmationCandidate := snapshotCEMConfirmationCandidate(state, sampledProposal, stats, state.Iterations)
	if newConfirmationCandidate {
		log.Printf("rare-position-cem-confirmation-ready: group=%d team=%d position=%d source_iteration=%d source_samples=%d source_exact_hits=%d source_exact_rate=%.5f source_near_target_rate=%.5f source_elite_ess=%.3f source_kl=%.4f work_remaining=%d",
			group.Id, state.Candidate.TeamID, state.Candidate.Position, state.Iterations,
			samples, stats.ExactHits, stats.ExactRate, stats.NearTargetRate,
			stats.EliteESS, sampledProposal.KL, *remainingWork)
	}
	updated := cemUpdateTeam(sampledProposal, original, group.Games, teamIDs, batch, elite)
	updated.Iteration = state.Iterations
	if !updated.UpdateAllowed {
		state.Active = false
		state.StopReason = "low_elite_ess"
		if state.HasConfirmationCandidate {
			state.StopReason = "confirmation_ready"
		}
		state.Proposal.EliteESS = updated.EliteESS
		log.Printf("rare-position-cem-selection: group=%d team=%d position=%d reason=low_elite_ess elite_ess=%.2f threshold=%.2f",
			group.Id, state.Candidate.TeamID, state.Candidate.Position, updated.EliteESS, CEMMinEliteESSForUpdate)
		logCEMStop(group.Id, state)
		return true
	}
	if updated.ThetaDeltaL2 < CEMThetaStabilityThreshold {
		state.StableIterations++
	} else {
		state.StableIterations = 0
	}
	updateCEMStallCounters(state, stats)
	if stats.EliteMeanDistance < state.BestEliteDistance {
		state.BestEliteDistance = stats.EliteMeanDistance
	}
	if stats.NearTargetRate > state.BestNearTargetRate {
		state.BestNearTargetRate = stats.NearTargetRate
	}
	state.Proposal = cloneCEMProposal(updated)
	if cemSnapshotBetter(stats, sampledProposal, state.BestStats, state.BestProposal, state.HasBest) {
		state.BestStats = stats
		state.BestProposal = cloneCEMProposal(sampledProposal)
		state.BestBatch = state.Iterations
		state.HasBest = true
	}
	if state.HasStats {
		state.ProgressScore = updateCEMProgressScore(state)
	}
	result.ChangedTeams += updated.ChangedTeams
	if updated.ChangedTeams > result.MaxChangedTeams {
		result.MaxChangedTeams = updated.ChangedTeams
	}
	result.AbsThetaSum += sumAbsTheta(updated.TeamLogMultipliers)
	result.ThetaUpdates++
	result.ThetaParameterCount += len(updated.TeamLogMultipliers)
	if updated.MaxAbsTheta > result.MaxAbsTheta {
		result.MaxAbsTheta = updated.MaxAbsTheta
	}
	result.ThetaDeltaL2Sum += updated.ThetaDeltaL2
	logCEMTeamChanges(group.Id, state.Candidate.TeamID, state.Candidate.Position,
		state.Iterations, teamIDs, original, group.Games, updated)
	log.Printf("rare-position-cem: parameterization=team_level group=%d team=%d position=%d iteration=%d samples=%d exact_hits=%d exact_hit_rate=%.4f near_target_rate=%.4f elite_count=%d elite_ess=%.2f exact_elites=%t mean_rank=%.3f best_rank=%d elite_mean_distance=%.3f kl=%.3f changed_teams=%d max_abs_team_theta=%.4f theta_delta_l2=%.4f progress_score=%.4f",
		group.Id, state.Candidate.TeamID, state.Candidate.Position, state.Iterations, samples,
		stats.ExactHits, stats.ExactRate, stats.NearTargetRate, stats.EliteCount,
		stats.EliteESS, stats.UsedExactElites, stats.MeanRank, stats.BestRank,
		stats.EliteMeanDistance, updated.KL, updated.ChangedTeams, updated.MaxAbsTheta,
		updated.ThetaDeltaL2, state.ProgressScore)

	stopReason := cemCandidateStopReason(state)
	if stopReason != "" {
		state.Active = false
		state.StopReason = stopReason
		logCEMStop(group.Id, state)
	}
	return true
}

func runCEMAdaptiveSchedule(states []*CEMCandidateState, searches map[int]*TeamRareSearch,
	canAffordInitial, canAffordAdaptive func() bool,
	runBatch func(*CEMCandidateState, int, string) bool) {
	ordered := append([]*CEMCandidateState(nil), states...)
	sort.Slice(ordered, func(i, j int) bool {
		a, b := ordered[i].Candidate, ordered[j].Candidate
		pa, pb := cemCandidatePriority(a, searches), cemCandidatePriority(b, searches)
		if pa != pb {
			return pa > pb
		}
		if a.TeamID != b.TeamID {
			return a.TeamID < b.TeamID
		}
		return a.Position < b.Position
	})
	step := 0
	for _, state := range ordered {
		if !canAffordInitial() {
			state.Active = false
			state.StopReason = "global_budget_exhausted"
			state.Candidate.SearchState.Status = StatusExhausted
			continue
		}
		step++
		if !runBatch(state, step, "initial_fairness") {
			state.Active = false
			state.StopReason = "global_budget_exhausted"
			state.Candidate.SearchState.Status = StatusExhausted
		}
	}
	for canAffordAdaptive() {
		state := selectCEMCandidate(states, searches)
		if state == nil {
			return
		}
		step++
		reason := "highest_progress"
		if state.AnyExactHit {
			reason = "exact_event_priority"
		}
		if !runBatch(state, step, reason) {
			state.Active = false
			state.StopReason = "global_budget_exhausted"
			state.Candidate.SearchState.Status = StatusExhausted
		}
	}
	for _, state := range states {
		if state.Active {
			state.Active = false
			state.StopReason = "global_budget_exhausted"
			state.Candidate.SearchState.Status = StatusExhausted
		}
	}
}

func admitCEMCandidates(states []*CEMCandidateState, searches map[int]*TeamRareSearch,
	maxCandidates, explorationSamples int) (admitted, excluded []*CEMCandidateState) {
	ordered := append([]*CEMCandidateState(nil), states...)
	sort.Slice(ordered, func(i, j int) bool {
		a, b := ordered[i].Candidate, ordered[j].Candidate
		pa, pb := cemCandidatePriority(a, searches), cemCandidatePriority(b, searches)
		if pa != pb {
			return pa > pb
		}
		if a.TeamID != b.TeamID {
			return a.TeamID < b.TeamID
		}
		return a.Position < b.Position
	})
	if maxCandidates < 0 {
		maxCandidates = 0
	}
	if explorationSamples < maxCandidates*CEMBatchSamples {
		maxCandidates = explorationSamples / CEMBatchSamples
	}
	selected := make(map[*CEMCandidateState]bool)
	seenTeams := make(map[int]bool)
	for _, state := range ordered {
		if len(admitted) >= maxCandidates {
			break
		}
		if seenTeams[state.Candidate.TeamID] {
			continue
		}
		state.AdmissionReason = "top_initial_candidates"
		admitted = append(admitted, state)
		selected[state] = true
		seenTeams[state.Candidate.TeamID] = true
	}
	for _, state := range ordered {
		if len(admitted) >= maxCandidates {
			break
		}
		if selected[state] {
			continue
		}
		state.AdmissionReason = "team_diversity"
		admitted = append(admitted, state)
		selected[state] = true
	}
	for _, state := range ordered {
		if !selected[state] {
			excluded = append(excluded, state)
		}
	}
	return admitted, excluded
}

func summarizeCEMConfirmation(seasons []CEMSeason, target int) CEMConfirmationStats {
	stats := CEMConfirmationStats{Samples: len(seasons), RelSE: math.Inf(1)}
	maxLog := math.Inf(-1)
	for _, season := range seasons {
		if season.Rank == target {
			stats.Hits++
		}
		if absInt(season.Rank-target) <= 1 {
			stats.NearTargetRate++
		}
		if season.Rank == target && season.LogWeight > maxLog {
			maxLog = season.LogWeight
		}
	}
	if len(seasons) == 0 {
		return stats
	}
	stats.HitRate = float64(stats.Hits) / float64(len(seasons))
	stats.NearTargetRate /= float64(len(seasons))
	if stats.Hits == 0 {
		return stats
	}
	var sum, sumSquares float64
	for _, season := range seasons {
		if season.Rank != target {
			continue
		}
		weight := math.Exp(season.LogWeight - maxLog)
		sum += weight
		sumSquares += weight * weight
	}
	stats.EventESS = sum * sum / sumSquares
	maxScaled := 0.0
	for _, season := range seasons {
		if season.Rank == target {
			if weight := math.Exp(season.LogWeight - maxLog); weight > maxScaled {
				maxScaled = weight
			}
		}
	}
	stats.MaxEventWeightShare = maxScaled / sum
	meanScaled := sum / float64(len(seasons))
	stats.Probability = meanScaled * math.Exp(maxLog)
	var varianceScaled float64
	for _, season := range seasons {
		value := 0.0
		if season.Rank == target {
			value = math.Exp(season.LogWeight - maxLog)
		}
		delta := value - meanScaled
		varianceScaled += delta * delta
	}
	if len(seasons) > 1 {
		stats.StdErr = math.Sqrt(varianceScaled/float64(len(seasons)-1)/float64(len(seasons))) * math.Exp(maxLog)
	}
	if stats.Probability > 0 {
		stats.RelSE = stats.StdErr / stats.Probability
	}
	return stats
}

func cemConfirmationAccepted(stats CEMConfirmationStats) (bool, string) {
	if stats.Hits < CEMMinConfirmationHits {
		return false, "no_reproduced_exact_event"
	}
	if stats.Hits == 1 {
		return true, "single_independent_exact_event"
	}
	if stats.MaxEventWeightShare > CEMConfirmationMaxEventShare || stats.EventESS <= 1.05 {
		if stats.MaxEventWeightShare > CEMConfirmationMaxEventShare {
			return false, "pathological_event_weight_share"
		}
		return false, "pathological_event_ess"
	}
	return true, "reproducible_exact_events"
}

func runCEMConfirmation(state *CEMCandidateState, group *GroupType, campaign []*TeamCampaign,
	table *Table, order []SortType, original []GameProposalMeans, teamIDs []int,
	confirmationRemaining, remainingWork *int64, workPerSample int64, rng *rand.Rand,
	result *CEMRoundResult) bool {
	samples := affordableSamples(CEMConfirmationSamples, *confirmationRemaining, workPerSample)
	samples = affordableSamples(samples, *remainingWork, workPerSample)
	if samples < CEMConfirmationSamples {
		return false
	}
	proposal := cloneCEMProposal(state.ConfirmationProposal)
	thetaBefore := cloneCEMProposal(state.Proposal)
	work := int64(samples) * workPerSample
	*confirmationRemaining -= work
	*remainingWork -= work
	result.ConfirmationWork += work
	result.ConfirmationAttempts++
	state.Candidate.SearchState.SearchWorkSpent += work
	state.StopReason = "confirming"
	seasons := simulateCEMBatchForTeams(campaign, group.Games, original, proposal.Means,
		table, order, group.Team_groups, teamIDs, state.Candidate.TeamID, samples, rng)
	stats := summarizeCEMConfirmation(seasons, state.Candidate.Position)
	if !equalCEMTheta(thetaBefore.TeamLogMultipliers, state.Proposal.TeamLogMultipliers) {
		panic("CEM confirmation changed theta")
	}
	state.Confirmation = stats
	result.ConfirmationHits += stats.Hits
	accepted, reason := cemConfirmationAccepted(stats)
	log.Printf("rare-position-cem-confirmation: group=%d team=%d position=%d proposal_iteration=%d samples=%d hits=%d hit_rate=%.5f near_target_rate=%.5f weighted_p_hat=%.8g se=%.3g relSE=%.3f event_ess=%.3f max_event_weight_share=%.3f work=%d confirmed=%t reason=%s",
		group.Id, state.Candidate.TeamID, state.Candidate.Position, proposal.Iteration,
		samples, stats.Hits, stats.HitRate, stats.NearTargetRate, stats.Probability,
		stats.StdErr, stats.RelSE, stats.EventESS, stats.MaxEventWeightShare, work, accepted, reason)
	if accepted {
		state.StopReason = "validation_ready"
		state.ConfirmationAccepted = true
		state.Candidate.SearchState.Status = StatusPromising
		result.ConfirmationSuccesses++
		if stats.Hits == 1 {
			result.ConfirmationSingleHitSuccesses++
		} else {
			result.ConfirmationMultiHitSuccesses++
		}
	} else {
		state.StopReason = "confirmation_failed"
		state.LastFailedConfirmationProposal = cloneCEMProposal(state.ConfirmationProposal)
		state.HasFailedConfirmationProposal = true
		state.Candidate.SearchState.Status = StatusFrontier
		result.ConfirmationFailures++
	}
	return accepted
}

func equalCEMTheta(a, b map[int]float64) bool {
	if len(a) != len(b) {
		return false
	}
	for teamID, theta := range a {
		if b[teamID] != theta {
			return false
		}
	}
	return true
}

func runCEMRound(candidates []*FrontierCandidate, searches map[int]*TeamRareSearch,
	group *GroupType, campaign []*TeamCampaign, table *Table, order []SortType,
	original []GameProposalMeans, cemRemaining, confirmationRemaining, validationRemaining, remainingWork *int64,
	explorationRemaining *int64, adaptWorkPerSample, confirmationWorkPerSample, validationWorkPerSample int64,
	totalWorkLimit int64, rng *rand.Rand) CEMRoundResult {
	result := CEMRoundResult{CandidatesTotal: len(candidates)}
	teamIDs := teamIDsFromGroups(group.Team_groups)
	states := make([]*CEMCandidateState, 0, len(candidates))
	for _, candidate := range candidates {
		state := &CEMCandidateState{Candidate: candidate,
			Proposal: newCEMProposal(original, group.Games, teamIDs), Active: true}
		states = append(states, state)
	}
	explorationAvailable := min(*explorationRemaining, min(*cemRemaining, *remainingWork))
	explorationSamples := int(explorationAvailable / adaptWorkPerSample)
	initialStates, notAdmitted := admitCEMCandidates(states, searches,
		CEMMaxInitialCandidates, explorationSamples)
	result.CandidatesAdmitted = len(initialStates)
	result.CandidatesNotAdmitted = len(notAdmitted)
	for _, state := range initialStates {
		if state.AdmissionReason == "team_diversity" {
			log.Printf("rare-position-cem-admission: group=%d team=%d position=%d static_priority=%d admitted=true reason=team_diversity",
				group.Id, state.Candidate.TeamID, state.Candidate.Position,
				cemCandidatePriority(state.Candidate, searches))
		} else {
			log.Printf("rare-position-cem-admission: group=%d team=%d position=%d static_priority=%d admitted=true reason=top_initial_candidates",
				group.Id, state.Candidate.TeamID, state.Candidate.Position,
				cemCandidatePriority(state.Candidate, searches))
		}
	}
	for _, state := range notAdmitted {
		state.Active = false
		state.AdmissionReason = "candidate_limit"
		if explorationSamples < CEMMaxInitialCandidates*CEMBatchSamples {
			state.AdmissionReason = "exploration_budget_limit"
		}
		state.StopReason = "not_admitted_to_cem"
		log.Printf("rare-position-cem-admission: group=%d team=%d position=%d static_priority=%d admitted=false reason=%s",
			group.Id, state.Candidate.TeamID, state.Candidate.Position,
			cemCandidatePriority(state.Candidate, searches), state.AdmissionReason)
	}
	canAffordInitial := func() bool {
		return affordableSamples(CEMBatchSamples, *explorationRemaining, adaptWorkPerSample) >= CEMMinBatchSamples &&
			affordableSamples(CEMBatchSamples, *cemRemaining, adaptWorkPerSample) >= CEMMinBatchSamples &&
			affordableSamples(CEMBatchSamples, *remainingWork, adaptWorkPerSample) >= CEMMinBatchSamples
	}
	canAffordAdaptive := func() bool {
		return affordableSamples(CEMBatchSamples, *cemRemaining, adaptWorkPerSample) >= CEMMinBatchSamples &&
			affordableSamples(CEMBatchSamples, *remainingWork, adaptWorkPerSample) >= CEMMinBatchSamples
	}
	confirmed := make(map[*CEMCandidateState]bool)
	confirmState := func(state *CEMCandidateState) bool {
		if affordableSamples(CEMConfirmationSamples, *confirmationRemaining, confirmationWorkPerSample) < CEMConfirmationSamples ||
			affordableSamples(CEMConfirmationSamples, *remainingWork, confirmationWorkPerSample) < CEMConfirmationSamples {
			state.StopReason = "confirmation_budget_exhausted"
			state.Candidate.SearchState.Status = StatusFrontier
			log.Printf("rare-position-cem-confirmation-budget: group=%d team=%d position=%d proposal_iteration=%d required_work=%d confirmation_work_remaining=%d global_work_remaining=%d",
				group.Id, state.Candidate.TeamID, state.Candidate.Position,
				state.ConfirmationProposal.Iteration,
				int64(CEMConfirmationSamples)*confirmationWorkPerSample,
				*confirmationRemaining, *remainingWork)
			return false
		}
		return runCEMConfirmation(state, group, campaign, table, order, original,
			teamIDs, confirmationRemaining, remainingWork,
			confirmationWorkPerSample, rng, &result)
	}
	runCEMAdaptiveSchedule(initialStates, searches, canAffordInitial, canAffordAdaptive, func(state *CEMCandidateState, step int, reason string) bool {
		if reason == "initial_fairness" {
			result.TargetsAttempted++
		}
		var phaseBudget *int64
		if reason == "initial_fairness" {
			phaseBudget = explorationRemaining
		}
		ok := runCEMCandidateBatch(state, group, campaign, table, order, original,
			teamIDs, cemRemaining, remainingWork, phaseBudget, adaptWorkPerSample, rng,
			&result, step, reason)
		if ok && state.StopReason == "confirmation_ready" && state.HasConfirmationCandidate {
			result.ConfirmationReadyCandidates++
			if confirmState(state) {
				confirmed[state] = true
			} else if state.StopReason == "confirmation_budget_exhausted" {
				result.ConfirmationFailedExhausted++
				state.HasConfirmationCandidate = false
			} else if state.Confirmation.Hits == 0 &&
				state.ConfirmationBatchStats.EliteESS >= CEMMinEliteESSForUpdate && canAffordAdaptive() {
				failedProposal := cloneCEMProposal(state.ConfirmationProposal)
				failedHits := state.Confirmation.Hits
				sourceBatch := state.ConfirmationSourceBatch
				state.HasConfirmationCandidate = false
				state.Active = true
				state.StopReason = ""
				result.ConfirmationFailedResumed++
				log.Printf("rare-position-cem-confirmation-resume: group=%d team=%d position=%d failed_proposal_iteration=%d confirmation_hits=%d next_iteration=%d cem_work_remaining=%d",
					group.Id, state.Candidate.TeamID, state.Candidate.Position,
					failedProposal.Iteration, failedHits, sourceBatch+1, *cemRemaining)
			} else {
				result.ConfirmationFailedExhausted++
				state.HasConfirmationCandidate = false
				state.Active = false
				state.Candidate.SearchState.Status = StatusFrontier
			}
		}
		return ok
	})
	for _, state := range states {
		if state.StopReason == "global_budget_exhausted" {
			logCEMStop(group.Id, state)
		}
	}

	for _, state := range states {
		if state.Iterations == 1 {
			result.OneBatchCandidates++
		}
		if state.Iterations >= 2 {
			result.MultiBatchCandidates++
		}
		if state.Iterations > result.MaxBatchesPerCandidate {
			result.MaxBatchesPerCandidate = state.Iterations
		}
		if state.Iterations > 0 {
			result.AverageBatchesPerCandidate += float64(state.Iterations)
			if result.BestCandidateTeam == 0 || state.FirstStats.EliteMeanDistance-state.BestEliteDistance > result.BestCandidateDistanceImprovement {
				result.BestCandidateTeam = state.Candidate.TeamID
				result.BestCandidatePosition = state.Candidate.Position
				result.BestCandidateBatches = state.Iterations
				result.BestCandidateDistanceImprovement = state.FirstStats.EliteMeanDistance - state.BestEliteDistance
				result.BestCandidateNearTargetRate = state.BestNearTargetRate
			}
			if state.AnyExactHit {
				result.TargetsAnyExact++
			}
			if state.ExactEliteSeen {
				result.TargetsExactElite++
			}
			nearRate := state.BestNearTargetRate
			if result.HighestNearTeam == 0 || nearRate > result.HighestNearRate {
				result.HighestNearTeam = state.Candidate.TeamID
				result.HighestNearPosition = state.Candidate.Position
				result.HighestNearBatches = state.Iterations
				result.HighestNearRate = nearRate
			}
		}
		if state.ConfirmationAccepted {
			state.Candidate.SearchState.Status = StatusPromising
		} else if state.Iterations > 0 && state.StopReason != "not_admitted_to_cem" &&
			state.StopReason != "confirmation_budget_exhausted" && state.StopReason != "confirmation_failed" {
			state.Candidate.SearchState.Status = StatusExhausted
		}
		switch state.StopReason {
		case "confirmation_ready", "validation_ready":
			result.StopValidationReady++
		case "stalled":
			result.StopStalled++
		case "low_elite_ess":
			result.StopLowEliteESS++
		case "regression":
			result.StopRegression++
		case "per_candidate_cap":
			result.StopPerCandidateCap++
		case "global_budget_exhausted":
			result.StopGlobalBudget++
		}
	}
	if result.TargetsAttempted > 0 {
		result.AverageBatchesPerCandidate /= float64(result.TargetsAttempted)
	}

	for _, state := range states {
		if state.Iterations > result.MaxBatchesPerCandidate {
			result.MaxBatchesPerCandidate = state.Iterations
		}
	}

	validationQueue := make([]*CEMCandidateState, 0, len(states))
	for _, state := range states {
		if confirmed[state] && state.StopReason == "validation_ready" {
			validationQueue = append(validationQueue, state)
		}
	}
	sort.Slice(validationQueue, func(i, j int) bool { return cemValidationSnapshotBetter(validationQueue[i], validationQueue[j]) })
	for _, state := range validationQueue {
		if *remainingWork < int64(CEMMinBatchSamples)*validationWorkPerSample ||
			*validationRemaining < int64(CEMMinBatchSamples)*validationWorkPerSample {
			state.Candidate.SearchState.Status = StatusExhausted
			state.StopReason = "global_budget_exhausted"
			result.StopGlobalBudget++
			logCEMStop(group.Id, state)
			continue
		}
		samples := affordableSamples(CEMValidationSamples, *validationRemaining, validationWorkPerSample)
		samples = affordableSamples(samples, *remainingWork, validationWorkPerSample)
		mixture := cemValidationMixture(original, state)
		validateProposalMixture(mixture, len(group.Games))
		candidate := state.Candidate
		reason := "independently_confirmed_frozen_proposal"
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
		result.ValidationAttempts++
		priority := cemValidationPriority(pilot)
		log.Printf("rare-position-cem-validation: group=%d team=%d position=%d reason=%s best_iteration=%d samples=%d hits=%d p=%.6g se=%.3g relSE=%.3f ess=%.2f ess_per_million_work=%.3f max_event_weight_share=%.3f priority=%.3f",
			group.Id, candidate.TeamID, candidate.Position, reason, state.ConfirmationSourceBatch,
			pilot.Samples, pilot.Hits, pilot.Probability, pilot.StdErr, pilot.RelSE, pilot.ESS,
			pilot.ESSPerWork*1e6, pilot.MaxEventWeightShare, priority)
		if selectPilotMixture([]*WeightedPilotResult{pilot}) == nil {
			candidate.SearchState.Status = StatusExhausted
			continue
		}
		candidate.SearchState.Status = StatusPromising
		candidate.SearchState.BestProposal = &pilot.Proposal
		candidate.SearchState.BestPilot = pilot
		result.TargetsValidated++
		result.ValidationSuccesses++
		result.ValidatedESS += pilot.ESS
		result.Eligible = append(result.Eligible, candidate)
	}
	result.Candidates = make([]CEMCandidateState, len(states))
	for i, state := range states {
		result.Candidates[i] = *state
	}
	if result.ConfirmationWork > int64(float64(totalWorkLimit)*MaxCEMConfirmationWorkFraction) {
		panic("CEM confirmation work exceeded its global fraction")
	}
	return result
}
