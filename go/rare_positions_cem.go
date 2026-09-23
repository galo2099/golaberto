package main

import (
	"fmt"
	"log"
	"math"
	"math/rand"
	"os"
	"sort"
)

const (
	CEMBatchSamples                     = 300
	CEMMaxIterationsPerCandidate        = 12
	CEMEliteFraction                    = 0.15
	CEMSmoothing                        = 0.5
	CEMMaxKL                            = 3.0
	CEMExactEventThreshold              = 5
	CEMValidationSamples                = 500 // legacy-only; not used by active orchestration
	CEMConfirmationChunkSamples         = 300 // legacy-only; not used by active orchestration
	CEMMaxConfirmationChunks            = 3   // legacy-only; not used by active orchestration
	CEMMinAdaptationHitsForConfirmation = 1   // legacy-only; not used by active orchestration
	CEMMinConfirmationHits              = 1   // legacy-only; not used by active orchestration
	CEMMaxInitialCandidates             = 6
	CEMConfirmationMaxEventShare        = 0.95 // legacy-only
	CEMMaxExplorationFraction           = 0.40
	MaxCEMWorkFraction                  = 0.10
	MaxCEMEvaluationWorkFraction        = 0.02
	MaxCEMValidationWorkFraction        = 0.02 // legacy-only
	MaxCEMConfirmationWorkFraction      = 0.03 // legacy-only
	MaxCEMPlainEquivalentSamples        = 5000
	CEMMaxSnapshotsPerCandidate         = 3
	CEMMaxGlobalSnapshots               = 12
	CEMMaxEvaluationSnapshots           = 3
	CEMEvaluationSamplesPerSnapshot     = 300
	CEMEvaluationChunkSamples           = 100
	CEMMaxEvaluationSamples             = 900
	CEMEvaluationMaxSamplesPerSnapshot  = 600
	CEMEvaluationEarlyAcceptESS         = 2.0
	CEMMinEliteESSForUpdate             = 8.0
	CEMMeaningfulTeamLogShift           = 0.03
	CEMThetaStabilityThreshold          = 0.02
	CEMProposalThetaTolerance           = 1e-4
	CEMProposalMeanTolerance            = 1e-6
	CEMMinRelativeDistanceImprovement   = 0.20
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
	SignAgreement     float64
	CosineSimilarity  float64
}

type CEMRoundResult struct {
	CEMWork                          int64
	AdaptationExactHitBatches        int
	Eligible                         []*FrontierCandidate // legacy-only
	ValidationWork                   int64                // legacy-only
	ConfirmationWork                 int64                // legacy-only
	ConfirmationAttempts             int                  // legacy-only
	ConfirmationChunks               int                  // legacy-only
	ConfirmationSamples              int                  // legacy-only
	ConfirmationSuccesses            int                  // legacy-only
	ConfirmationFailures             int                  // legacy-only
	ConfirmationReadyCandidates      int                  // legacy-only
	ConfirmationSingleHitSuccesses   int                  // legacy-only
	ConfirmationMultiHitSuccesses    int                  // legacy-only
	ConfirmationFailedResumed        int                  // legacy-only
	ConfirmationFailedExhausted      int                  // legacy-only
	ConfirmationInconclusiveBudget   int                  // legacy-only
	ConfirmationHits                 int                  // legacy-only
	ValidationAttempts               int                  // legacy-only
	ValidationSuccesses              int                  // legacy-only
	CandidatesTotal                  int
	CandidatesAdmitted               int
	CandidatesNotAdmitted            int
	ExplorationSamples               int
	AdaptiveSamples                  int
	TargetsAttempted                 int
	Iterations                       int
	TargetsAnyExact                  int
	TargetsExactElite                int
	TargetsValidated                 int // legacy-only
	Snapshots                        []CEMProposalSnapshot
	ChangedTeams                     int
	MaxChangedTeams                  int
	AbsThetaSum                      float64
	ThetaUpdates                     int
	ThetaParameterCount              int
	MaxAbsTheta                      float64
	ThetaDeltaL2Sum                  float64
	ValidatedESS                     float64 // legacy-only
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
	StopValidationReady              int // legacy-only
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
	InitialDirectionVector         map[int]float64
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
	MaxExactHits                   int
	EverHadExactHit                bool
	ExactEliteSeen                 bool
	StalledIterations              int
	RegressionIterations           int
	StableIterations               int
	Active                         bool
	StopReason                     string
	ProgressScore                  float64
	Snapshots                      []CEMProposalSnapshot
	AdmissionReason                string
	ConfirmationAccepted           bool // legacy-only; not used by active orchestration
	ExactHitPriorityActive         bool // legacy-only; not used by active orchestration
	Confirmation                   CEMConfirmationStats
	ConfirmationChunks             int
	ConfirmationSeasons            []CEMSeason
	ConfirmationProposal           CEMProposal
	ConfirmationBatchStats         CEMBatchStats
	ConfirmationSourceBatch        int
	HasConfirmationCandidate       bool
	LastFailedConfirmationProposal CEMProposal
	HasFailedConfirmationProposal  bool
}

// Legacy helpers remain available to old tests while orchestration migrates.
type CEMConfirmationStats struct {
	Samples             int
	Hits                int
	NearTargetHits      int
	HitRate             float64
	NearTargetRate      float64
	Probability         float64
	StdErr              float64
	RelSE               float64
	EventESS            float64
	MaxEventWeightShare float64
}

type CEMConfirmationOutcome struct {
	Confirmed    bool
	Failed       bool
	Inconclusive bool
}

type CEMProposalSnapshot struct {
	CandidateTeam        int
	CandidatePosition    int
	SourceIteration      int
	Proposal             CEMProposal
	Stats                CEMBatchStats
	InitialEliteDistance float64
	ExactHits            int
	SearchScore          float64
	Reason               string
}

type CEMProposalEvaluation struct {
	Snapshot            CEMProposalSnapshot
	Samples             int
	Hits                int
	SumY                float64
	SumY2               float64
	Probability         float64
	StdErr              float64
	RelSE               float64
	ESS                 float64
	ESSPerWork          float64
	SecondMoment        float64
	MaxEventWeightShare float64
	Work                int64
}

// WeightedEventStats is the cumulative held-out information for one event.
// In particular, SumY and SumY2 contain w*I(A) and its square, not Q hit
// rates.  The same type is used for the exact event and both search-only
// neighbourhood events so their accounting cannot drift apart.
type WeightedEventStats struct {
	Hits           int
	SumY           float64
	SumY2          float64
	Probability    float64
	StdErr         float64
	RelSE          float64
	ESS            float64
	MaxWeight      float64
	MaxWeightShare float64
}

type CEMEvaluationState struct {
	Snapshot CEMProposalSnapshot
	Stage    int
	Samples  int
	Work     int64
	Chunks   int

	Exact WeightedEventStats
	Near1 WeightedEventStats
	Near2 WeightedEventStats

	Near1EfficiencyRatio  float64
	Near2EfficiencyRatio  float64
	AdaptationPriority    float64
	ScoutNear1Probability float64
	ScoutNear2Probability float64

	Eliminated        bool
	EliminationReason string
}

func (s *WeightedEventStats) observe(hit bool, weight float64) {
	if !hit {
		return
	}
	s.Hits++
	s.SumY += weight
	s.SumY2 += weight * weight
	if weight > s.MaxWeight {
		s.MaxWeight = weight
	}
}

func (s *WeightedEventStats) summarize(samples int) {
	s.Probability, s.StdErr, s.ESS = eventEstimateStats(s.SumY, s.SumY2, samples)
	s.RelSE = math.Inf(1)
	s.MaxWeightShare = 0
	if s.Probability > 0 {
		s.RelSE = s.StdErr / s.Probability
		s.MaxWeightShare = s.MaxWeight / s.SumY
	}
}

func weightedESSPerWork(stats WeightedEventStats, work int64) float64 {
	if work <= 0 {
		return 0
	}
	return stats.ESS / float64(work)
}

func teamIDsFromGroups(groups []TeamType) []int {
	ids := make([]int, len(groups))
	for i, group := range groups {
		ids[i] = group.Team_id
	}
	sort.Ints(ids)
	return ids
}

const (
	CEMWarmStartCompetitorMass = 1.0
	CEMWarmStartKL             = 0.15
)

type CEMInitializationMode int

const (
	CEMInitZero CEMInitializationMode = iota
	CEMInitStandingsDirected
)

var CEMDefaultInitializationMode = CEMInitStandingsDirected

func newZeroCEMProposal(original []GameProposalMeans, games []*GameType, teamIDs []int) CEMProposal {
	return newCEMProposal(original, games, teamIDs)
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

func newCEMProposalWithMode(mode CEMInitializationMode, candidate *FrontierCandidate, searches map[int]*TeamRareSearch, original []GameProposalMeans, games []*GameType, teamIDs []int, groupID int) (CEMProposal, map[int]float64) {
	if mode == CEMInitStandingsDirected {
		return initializeDirectedCEMProposal(candidate, searches, original, games, teamIDs, groupID)
	}
	prop := newZeroCEMProposal(original, games, teamIDs)
	return prop, nil
}

func computeSignAgreementAndCosine(d map[int]float64, theta map[int]float64) (float64, float64) {
	if len(d) == 0 || len(theta) == 0 {
		return 0, 0
	}

	dot, normD, normTheta := 0.0, 0.0, 0.0
	matchedSigns, totalNonZeroD := 0, 0

	for teamID, dVal := range d {
		if dVal == 0 {
			continue
		}
		totalNonZeroD++
		thVal := theta[teamID]
		if (dVal > 0 && thVal > 0) || (dVal < 0 && thVal < 0) {
			matchedSigns++
		}
		dot += dVal * thVal
		normD += dVal * dVal
	}

	for _, thVal := range theta {
		normTheta += thVal * thVal
	}

	signAgreement := 0.0
	if totalNonZeroD > 0 {
		signAgreement = float64(matchedSigns) / float64(totalNonZeroD)
	}

	cosineSim := 0.0
	if normD > 0 && normTheta > 0 {
		cosineSim = dot / (math.Sqrt(normD) * math.Sqrt(normTheta))
	}

	return signAgreement, cosineSim
}

func initializeDirectedCEMProposal(
	candidate *FrontierCandidate,
	searches map[int]*TeamRareSearch,
	original []GameProposalMeans,
	games []*GameType,
	teamIDs []int,
	groupID int,
) (CEMProposal, map[int]float64) {
	targetTeamID := candidate.TeamID
	targetPosition := candidate.Position
	direction := candidate.Direction

	targetSearch, ok := searches[targetTeamID]
	if !ok || targetSearch == nil {
		log.Printf("rare-position-cem-init-fallback: group=%d team=%d position=%d reason=no_target_search",
			groupID, targetTeamID, targetPosition)
		return newCEMProposal(original, games, teamIDs), nil
	}

	rT := targetSearch.NormalMeanRank

	var corridor []int
	var overtakersBlockersRole string

	if direction == RareBetter {
		overtakersBlockersRole = "blocker"
		for teamID, search := range searches {
			if teamID == targetTeamID || search == nil {
				continue
			}
			rJ := search.NormalMeanRank
			if float64(targetPosition) <= rJ && rJ < rT {
				corridor = append(corridor, teamID)
			}
		}
	} else {
		overtakersBlockersRole = "overtaker"
		for teamID, search := range searches {
			if teamID == targetTeamID || search == nil {
				continue
			}
			rJ := search.NormalMeanRank
			if rT < rJ && rJ <= float64(targetPosition) {
				corridor = append(corridor, teamID)
			}
		}
	}

	if len(corridor) == 0 {
		for teamID, search := range searches {
			if teamID == targetTeamID || search == nil {
				continue
			}
			rJ := search.NormalMeanRank
			if math.Abs(rJ-float64(targetPosition)) <= 1.0 {
				corridor = append(corridor, teamID)
			}
		}
	}

	if len(corridor) == 0 {
		log.Printf("rare-position-cem-init-fallback: group=%d team=%d position=%d reason=no_corridor_competitors",
			groupID, targetTeamID, targetPosition)
		return newCEMProposal(original, games, teamIDs), nil
	}

	sort.Ints(corridor)

	rawWeights := make(map[int]float64, len(corridor))
	sumRaw := 0.0
	for _, teamID := range corridor {
		rJ := searches[teamID].NormalMeanRank
		dist := math.Abs(rJ - float64(targetPosition))
		w := 1.0 / (1.0 + dist)
		rawWeights[teamID] = w
		sumRaw += w
	}

	normWeights := make(map[int]float64, len(corridor))
	for _, teamID := range corridor {
		normWeights[teamID] = rawWeights[teamID] / sumRaw
	}

	unscaledTheta := make(map[int]float64, len(teamIDs))
	for _, teamID := range teamIDs {
		unscaledTheta[teamID] = 0.0
	}

	targetDir := 1.0
	if direction == RareWorse {
		targetDir = -1.0
	}
	unscaledTheta[targetTeamID] = targetDir

	for _, teamID := range corridor {
		if direction == RareBetter {
			unscaledTheta[teamID] = -CEMWarmStartCompetitorMass * normWeights[teamID]
		} else {
			unscaledTheta[teamID] = +CEMWarmStartCompetitorMass * normWeights[teamID]
		}
	}

	unscaledL1 := 0.0
	for _, th := range unscaledTheta {
		unscaledL1 += math.Abs(th)
	}

	unscaledProp := CEMProposal{TeamLogMultipliers: copyTheta(unscaledTheta), UpdateAllowed: true}
	unscaledProp.Means = materializeCEMProposal(unscaledProp, original, games)
	unscaledProp.KL = cemTotalKL(unscaledProp.Means, original, games)

	var scaledProp CEMProposal
	alpha := 0.0

	if unscaledProp.KL > 0 {
		scaledProp = cemTrustRegionTeam(unscaledProp, original, games, CEMWarmStartKL)
		if scaledProp.TeamLogMultipliers[targetTeamID] != 0 {
			alpha = scaledProp.TeamLogMultipliers[targetTeamID] / targetDir
		}
	} else {
		scaledProp = newCEMProposal(original, games, teamIDs)
	}

	dirName := "better"
	if direction == RareWorse {
		dirName = "worse"
	}

	log.Printf("rare-position-cem-init: group=%d team=%d position=%d mode=standings_directed direction=%s normal_mean_rank=%.2f target_position=%d competitor_count=%d competitor_mass=%.2f target_direction=%.1f unscaled_theta_l1=%.4f alpha=%.4f kl=%.4f",
		groupID, targetTeamID, targetPosition, dirName, rT, targetPosition,
		len(corridor), CEMWarmStartCompetitorMass, targetDir, unscaledL1, alpha, scaledProp.KL)

	log.Printf("rare-position-cem-init-team: group=%d target_team=%d target_position=%d team_id=%d role=target normal_mean_rank=%.2f rank_distance_to_boundary=%.2f raw_relevance=1.0000 normalized_relevance=1.0000 direction=%.1f initial_theta=%.6f initial_multiplier=%.6f",
		groupID, targetTeamID, targetPosition, targetTeamID, rT, math.Abs(rT-float64(targetPosition)),
		targetDir, scaledProp.TeamLogMultipliers[targetTeamID], math.Exp(scaledProp.TeamLogMultipliers[targetTeamID]))

	for _, teamID := range corridor {
		rJ := searches[teamID].NormalMeanRank
		dist := math.Abs(rJ - float64(targetPosition))
		compDir := -targetDir
		thetaVal := scaledProp.TeamLogMultipliers[teamID]

		log.Printf("rare-position-cem-init-team: group=%d target_team=%d target_position=%d team_id=%d role=%s normal_mean_rank=%.2f rank_distance_to_boundary=%.2f raw_relevance=%.4f normalized_relevance=%.4f direction=%.1f initial_theta=%.6f initial_multiplier=%.6f",
			groupID, targetTeamID, targetPosition, teamID, overtakersBlockersRole, rJ, dist,
			rawWeights[teamID], normWeights[teamID], compDir, thetaVal, math.Exp(thetaVal))
	}

	return scaledProp, unscaledTheta
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
		sort.Sort(TeamCampaignSorted{t: teamSlice, sort: order, rng: rng})
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

func buildCEMMixture(original, learned []GameProposalMeans, targetRank int) []ProposalComponent {
	return []ProposalComponent{
		{Name: "original", Weight: OriginalMixtureWeight, Means: original, TargetRank: -1},
		{Name: "cem_team_level", Weight: 1 - OriginalMixtureWeight, Means: learned, TargetRank: targetRank},
	}
}

func cemEvaluationMixture(original []GameProposalMeans, snapshot CEMProposalSnapshot) []ProposalComponent {
	return buildCEMMixture(original, snapshot.Proposal.Means, snapshot.CandidatePosition)
}

// Legacy-only helpers; the active production path uses held-out evaluations.
func cemValidationMixture(original []GameProposalMeans, state *CEMCandidateState) []ProposalComponent {
	return buildCEMMixture(original, state.ConfirmationProposal.Means, state.Candidate.Position)
}

func cemValidationPriority(pilot *WeightedPilotResult) float64 {
	if pilot == nil || pilot.Samples <= 0 || pilot.Hits <= 0 {
		return math.Inf(-1)
	}
	return pilot.ESSPerWork
}

func cemSnapshotBetterForEvaluation(a, b CEMProposalEvaluation) bool {
	if a.ESSPerWork != b.ESSPerWork {
		return a.ESSPerWork > b.ESSPerWork
	}
	if a.MaxEventWeightShare != b.MaxEventWeightShare {
		return a.MaxEventWeightShare < b.MaxEventWeightShare
	}
	if a.RelSE != b.RelSE {
		return a.RelSE < b.RelSE
	}
	if a.Snapshot.ExactHits != b.Snapshot.ExactHits {
		return a.Snapshot.ExactHits > b.Snapshot.ExactHits
	}
	if a.Snapshot.Proposal.KL != b.Snapshot.Proposal.KL {
		return a.Snapshot.Proposal.KL < b.Snapshot.Proposal.KL
	}
	if a.Snapshot.CandidateTeam != b.Snapshot.CandidateTeam {
		return a.Snapshot.CandidateTeam < b.Snapshot.CandidateTeam
	}
	if a.Snapshot.CandidatePosition != b.Snapshot.CandidatePosition {
		return a.Snapshot.CandidatePosition < b.Snapshot.CandidatePosition
	}
	return a.Snapshot.SourceIteration < b.Snapshot.SourceIteration
}

func cemSnapshotAdaptationBetter(a, b CEMProposalSnapshot) bool {
	if a.ExactHits != b.ExactHits {
		return a.ExactHits > b.ExactHits
	}
	if a.Stats.NearTargetRate != b.Stats.NearTargetRate {
		return a.Stats.NearTargetRate > b.Stats.NearTargetRate
	}
	if a.Stats.EliteMeanDistance != b.Stats.EliteMeanDistance {
		return a.Stats.EliteMeanDistance < b.Stats.EliteMeanDistance
	}
	if a.Stats.EliteESS != b.Stats.EliteESS {
		return a.Stats.EliteESS > b.Stats.EliteESS
	}
	if a.Proposal.KL != b.Proposal.KL {
		return a.Proposal.KL < b.Proposal.KL
	}
	return a.SourceIteration < b.SourceIteration
}

func thetaDistanceL2(a, b map[int]float64) float64 {
	sum := 0.0
	for teamID, theta := range a {
		delta := theta - b[teamID]
		sum += delta * delta
	}
	for teamID, theta := range b {
		if _, ok := a[teamID]; !ok {
			sum += theta * theta
		}
	}
	return math.Sqrt(sum)
}

func retainCEMProposalSnapshot(groupID int, state *CEMCandidateState, proposal CEMProposal,
	stats CEMBatchStats, iteration int, reason string) bool {
	snapshot := CEMProposalSnapshot{
		CandidateTeam: state.Candidate.TeamID, CandidatePosition: state.Candidate.Position,
		SourceIteration: iteration, Proposal: cloneCEMProposal(proposal), Stats: stats,
		InitialEliteDistance: state.FirstStats.EliteMeanDistance,
		ExactHits:            stats.ExactHits, Reason: reason,
		SearchScore: stats.NearTargetRate*100 - stats.EliteMeanDistance + float64(stats.ExactHits)*10,
	}
	nearestDistance := 0.0
	if len(state.Snapshots) > 0 {
		nearestDistance = math.Inf(1)
		for _, previous := range state.Snapshots {
			nearestDistance = math.Min(nearestDistance, thetaDistanceL2(
				snapshot.Proposal.TeamLogMultipliers, previous.Proposal.TeamLogMultipliers))
		}
	}
	for i := range state.Snapshots {
		if thetaDistanceL2(snapshot.Proposal.TeamLogMultipliers,
			state.Snapshots[i].Proposal.TeamLogMultipliers) < CEMThetaStabilityThreshold {
			if cemSnapshotAdaptationBetter(snapshot, state.Snapshots[i]) {
				state.Snapshots[i] = snapshot
				logCEMSnapshot(groupID, state, snapshot, nearestDistance)
				return true
			}
			return false
		}
	}
	state.Snapshots = append(state.Snapshots, snapshot)
	sort.Slice(state.Snapshots, func(i, j int) bool {
		return cemSnapshotAdaptationBetter(state.Snapshots[i], state.Snapshots[j])
	})
	if len(state.Snapshots) > CEMMaxSnapshotsPerCandidate {
		state.Snapshots = state.Snapshots[:CEMMaxSnapshotsPerCandidate]
	}
	for _, retained := range state.Snapshots {
		if retained.SourceIteration == snapshot.SourceIteration {
			logCEMSnapshot(groupID, state, snapshot, nearestDistance)
			return true
		}
	}
	return false
}

// snapshotCEMConfirmationCandidate is retained solely for legacy-only tests.
func snapshotCEMConfirmationCandidate(state *CEMCandidateState, sampled CEMProposal,
	stats CEMBatchStats, sourceBatch int) bool {
	if stats.ExactHits < CEMMinAdaptationHitsForConfirmation || state.HasConfirmationCandidate {
		return false
	}
	state.ConfirmationProposal = cloneCEMProposal(sampled)
	state.ConfirmationBatchStats = stats
	state.ConfirmationSourceBatch = sourceBatch
	state.HasConfirmationCandidate = true
	state.ConfirmationAccepted = false
	state.Active = false
	state.StopReason = "confirmation_ready"
	return true
}

func logCEMSnapshot(groupID int, state *CEMCandidateState, snapshot CEMProposalSnapshot, thetaL2 float64) {
	log.Printf("rare-position-cem-snapshot: group=%d team=%d position=%d source_iteration=%d reason=%s exact_hits=%d near_target_rate=%.5f elite_mean_distance=%.4f elite_ess=%.3f kl=%.4f theta_l2_from_previous_snapshot=%.5f snapshot_count_for_candidate=%d",
		groupID, snapshot.CandidateTeam, snapshot.CandidatePosition, snapshot.SourceIteration,
		snapshot.Reason, snapshot.ExactHits, snapshot.Stats.NearTargetRate,
		snapshot.Stats.EliteMeanDistance, snapshot.Stats.EliteESS, snapshot.Proposal.KL,
		thetaL2, len(state.Snapshots))
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
	// Score current near-target and distance progress. Exact hits inform retained
	// snapshots but do not monopolize adaptation scheduling.
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
		if !state.Active {
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
	if state.InitialDirectionVector != nil {
		signAgr, cosSim := computeSignAgreementAndCosine(state.InitialDirectionVector, state.Proposal.TeamLogMultipliers)
		stats.SignAgreement = signAgr
		stats.CosineSimilarity = cosSim
		log.Printf("rare-position-cem-init-agreement: group=%d team=%d position=%d iteration=%d sign_agreement=%.4f cosine_similarity=%.4f",
			group.Id, state.Candidate.TeamID, state.Candidate.Position, state.Iterations, signAgr, cosSim)
	}
	if state.Iterations == 1 {
		state.FirstStats = stats
		state.BestEliteDistance = stats.EliteMeanDistance
		state.BestNearTargetRate = stats.NearTargetRate
	} else {
		state.PreviousStats = state.LastStats
	}
	state.LastStats = stats
	state.HasStats = true
	state.EverHadExactHit = state.EverHadExactHit || stats.ExactHits > 0
	if stats.ExactHits > 0 {
		state.Candidate.SearchState.CEMEverHadExactHit = true
	}
	if stats.ExactHits > state.MaxExactHits {
		state.MaxExactHits = stats.ExactHits
	}
	state.ExactEliteSeen = state.ExactEliteSeen || exact
	if stats.ExactHits > 0 {
		result.AdaptationExactHitBatches++
	}
	snapshotReason := ""
	switch {
	case stats.ExactHits > 0:
		snapshotReason = "exact_hit"
	case !state.HasBest || cemSnapshotBetter(stats, sampledProposal, state.BestStats, state.BestProposal, state.HasBest):
		snapshotReason = "best_snapshot"
	case stats.NearTargetRate > state.BestNearTargetRate:
		snapshotReason = "best_near"
	case stats.EliteMeanDistance < state.BestEliteDistance:
		snapshotReason = "best_distance"
	}
	if snapshotReason != "" {
		retainCEMProposalSnapshot(group.Id, state, sampledProposal, stats, state.Iterations, snapshotReason)
	}
	updated := cemUpdateTeam(sampledProposal, original, group.Games, teamIDs, batch, elite)
	updated.Iteration = state.Iterations
	if !updated.UpdateAllowed {
		state.Active = false
		state.StopReason = "low_elite_ess"
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
		if !runBatch(state, step, "highest_progress") {
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
			stats.NearTargetHits++
		}
		if season.Rank == target && season.LogWeight > maxLog {
			maxLog = season.LogWeight
		}
	}
	if len(seasons) == 0 {
		return stats
	}
	stats.HitRate = float64(stats.Hits) / float64(len(seasons))
	stats.NearTargetRate = float64(stats.NearTargetHits) / float64(len(seasons))
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

func runCEMConfirmation(state *CEMCandidateState, groupID int,
	confirmationRemaining, remainingWork *int64, workPerSample int64,
	sampleChunk func(CEMProposal, int) []CEMSeason, result *CEMRoundResult) CEMConfirmationOutcome {
	proposal := cloneCEMProposal(state.ConfirmationProposal)
	thetaBefore := cloneCEMProposal(state.Proposal)
	state.ConfirmationChunks = 0
	state.ConfirmationSeasons = nil
	state.Confirmation = CEMConfirmationStats{}
	state.StopReason = "confirming"
	decision := CEMConfirmationOutcome{}
	reason := "confirmation_budget_exhausted"
	workForChunk := int64(CEMConfirmationChunkSamples) * workPerSample

	for chunk := 1; chunk <= CEMMaxConfirmationChunks; chunk++ {
		if *confirmationRemaining < workForChunk || *remainingWork < workForChunk {
			decision.Inconclusive = true
			result.ConfirmationInconclusiveBudget++
			break
		}
		*confirmationRemaining -= workForChunk
		*remainingWork -= workForChunk
		result.ConfirmationWork += workForChunk
		result.ConfirmationChunks++
		result.ConfirmationSamples += CEMConfirmationChunkSamples
		if chunk == 1 {
			result.ConfirmationAttempts++
		}
		state.Candidate.SearchState.SearchWorkSpent += workForChunk
		chunkSeasons := sampleChunk(cloneCEMProposal(proposal), CEMConfirmationChunkSamples)
		if len(chunkSeasons) != CEMConfirmationChunkSamples {
			panic("CEM confirmation sampler returned an unexpected chunk size")
		}
		chunkHits, chunkNear := 0, 0
		for _, season := range chunkSeasons {
			if season.Rank == state.Candidate.Position {
				chunkHits++
			}
			if absInt(season.Rank-state.Candidate.Position) <= 1 {
				chunkNear++
			}
			state.ConfirmationSeasons = append(state.ConfirmationSeasons,
				CEMSeason{Rank: season.Rank, LogWeight: season.LogWeight})
		}
		result.ConfirmationHits += chunkHits
		state.ConfirmationChunks = chunk
		stats := summarizeCEMConfirmation(state.ConfirmationSeasons, state.Candidate.Position)
		state.Confirmation = stats
		if !equalCEMTheta(thetaBefore.TeamLogMultipliers, state.Proposal.TeamLogMultipliers) {
			panic("CEM confirmation changed theta")
		}
		chunkDecision := "continue"
		if stats.Hits > 0 {
			accepted, acceptReason := cemConfirmationAccepted(stats)
			reason = acceptReason
			if accepted {
				decision.Confirmed = true
				chunkDecision = "confirmed"
			} else {
				decision.Failed = true
				chunkDecision = "failed"
			}
		} else if chunk == CEMMaxConfirmationChunks {
			decision.Failed = true
			reason = "no_exact_event_after_max_chunks"
			chunkDecision = "failed"
		} else if *confirmationRemaining < workForChunk || *remainingWork < workForChunk {
			decision.Inconclusive = true
			result.ConfirmationInconclusiveBudget++
			reason = "confirmation_budget_exhausted"
			chunkDecision = "budget_exhausted"
		}
		log.Printf("rare-position-cem-confirmation-chunk: group=%d team=%d position=%d proposal_iteration=%d chunk=%d chunk_samples=%d cumulative_samples=%d chunk_hits=%d cumulative_hits=%d chunk_near_target_rate=%.5f cumulative_near_target_rate=%.5f event_ess=%.3f max_event_weight_share=%.3f work=%d decision=%s",
			groupID, state.Candidate.TeamID, state.Candidate.Position, proposal.Iteration,
			chunk, CEMConfirmationChunkSamples, stats.Samples, chunkHits, stats.Hits,
			float64(chunkNear)/float64(CEMConfirmationChunkSamples), stats.NearTargetRate,
			stats.EventESS, stats.MaxEventWeightShare, workForChunk, chunkDecision)
		if decision.Confirmed || decision.Failed || decision.Inconclusive {
			break
		}
	}

	stats := state.Confirmation
	if decision.Confirmed {
		state.StopReason = "validation_ready"
		state.ConfirmationAccepted = true
		state.ExactHitPriorityActive = false
		state.Candidate.SearchState.Status = StatusPromising
		result.ConfirmationSuccesses++
		if stats.Hits == 1 {
			result.ConfirmationSingleHitSuccesses++
		} else {
			result.ConfirmationMultiHitSuccesses++
		}
	} else if decision.Failed {
		state.StopReason = "confirmation_failed"
		state.LastFailedConfirmationProposal = cloneCEMProposal(state.ConfirmationProposal)
		state.HasFailedConfirmationProposal = true
		failedProposal := cloneCEMProposal(state.ConfirmationProposal)
		state.Candidate.SearchState.CEMFailedConfirmationProposal = &failedProposal
		state.ExactHitPriorityActive = false
		state.ProgressScore = updateCEMProgressScore(state)
		state.Candidate.SearchState.Status = StatusFrontier
		result.ConfirmationFailures++
	} else {
		state.StopReason = "confirmation_budget_exhausted"
		state.ExactHitPriorityActive = false
		state.ProgressScore = updateCEMProgressScore(state)
		state.Candidate.SearchState.Status = StatusFrontier
	}
	log.Printf("rare-position-cem-confirmation: group=%d team=%d position=%d proposal_iteration=%d chunks=%d samples=%d hits=%d hit_rate=%.5f near_target_rate=%.5f weighted_p_hat=%.8g se=%.3g relSE=%.3f event_ess=%.3f max_event_weight_share=%.3f confirmed=%t reason=%s",
		groupID, state.Candidate.TeamID, state.Candidate.Position, proposal.Iteration,
		stats.Samples/CEMConfirmationChunkSamples, stats.Samples, stats.Hits, stats.HitRate,
		stats.NearTargetRate, stats.Probability, stats.StdErr, stats.RelSE,
		stats.EventESS, stats.MaxEventWeightShare, decision.Confirmed, reason)
	return decision
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
			Proposal: newCEMProposal(original, group.Games, teamIDs), Active: true,
			EverHadExactHit: candidate.SearchState.CEMEverHadExactHit}
		if candidate.SearchState.CEMFailedConfirmationProposal != nil {
			state.LastFailedConfirmationProposal = cloneCEMProposal(*candidate.SearchState.CEMFailedConfirmationProposal)
			state.HasFailedConfirmationProposal = true
		}
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
	confirmState := func(state *CEMCandidateState) CEMConfirmationOutcome {
		return runCEMConfirmation(state, group.Id, confirmationRemaining, remainingWork,
			confirmationWorkPerSample, func(proposal CEMProposal, samples int) []CEMSeason {
				return simulateCEMBatchForTeams(campaign, group.Games, original, proposal.Means,
					table, order, group.Team_groups, teamIDs, state.Candidate.TeamID, samples, rng)
			}, &result)
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
			outcome := confirmState(state)
			if outcome.Confirmed {
				confirmed[state] = true
			} else if outcome.Inconclusive {
				state.HasConfirmationCandidate = false
				state.Active = false
				state.Candidate.SearchState.Status = StatusFrontier
			} else if outcome.Failed && state.Confirmation.Hits == 0 &&
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
			} else if outcome.Failed {
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
			if state.EverHadExactHit {
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

// runCEMAdaptationRound is the active search-only CEM path. Exact events are
// retained as snapshots by runCEMCandidateBatch and never stop the scheduler.
func runCEMAdaptationRound(candidates []*FrontierCandidate, searches map[int]*TeamRareSearch,
	group *GroupType, campaign []*TeamCampaign, table *Table, order []SortType,
	original []GameProposalMeans, cemRemaining, remainingWork, explorationRemaining *int64,
	adaptWorkPerSample int64, rng *rand.Rand) CEMRoundResult {
	mode := CEMDefaultInitializationMode
	if envMode := os.Getenv("RARE_POSITION_CEM_INIT"); envMode == "zero" {
		mode = CEMInitZero
	} else if envMode == "directed" {
		mode = CEMInitStandingsDirected
	}
	return runCEMAdaptationRoundWithMode(mode, candidates, searches, group, campaign, table,
		order, original, cemRemaining, remainingWork, explorationRemaining, adaptWorkPerSample, rng)
}

func runCEMAdaptationRoundWithMode(mode CEMInitializationMode, candidates []*FrontierCandidate, searches map[int]*TeamRareSearch,
	group *GroupType, campaign []*TeamCampaign, table *Table, order []SortType,
	original []GameProposalMeans, cemRemaining, remainingWork, explorationRemaining *int64,
	adaptWorkPerSample int64, rng *rand.Rand) CEMRoundResult {
	result := CEMRoundResult{CandidatesTotal: len(candidates)}
	teamIDs := teamIDsFromGroups(group.Team_groups)
	states := make([]*CEMCandidateState, 0, len(candidates))
	for _, candidate := range candidates {
		prop, initDir := newCEMProposalWithMode(mode, candidate, searches, original, group.Games, teamIDs, group.Id)
		states = append(states, &CEMCandidateState{
			Candidate:              candidate,
			Proposal:               prop,
			InitialDirectionVector: initDir,
			Active:                 true,
		})
	}
	explorationAvailable := min(*explorationRemaining, min(*cemRemaining, *remainingWork))
	explorationSamples := int(explorationAvailable / adaptWorkPerSample)
	initialStates, notAdmitted := admitCEMCandidates(states, searches,
		CEMMaxInitialCandidates, explorationSamples)
	result.CandidatesAdmitted = len(initialStates)
	result.CandidatesNotAdmitted = len(notAdmitted)
	for _, state := range notAdmitted {
		state.Active = false
		state.AdmissionReason = "candidate_limit"
		if explorationSamples < CEMMaxInitialCandidates*CEMBatchSamples {
			state.AdmissionReason = "exploration_budget_limit"
		}
		state.StopReason = "not_admitted_to_cem"
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
	runCEMAdaptiveSchedule(initialStates, searches, canAffordInitial, canAffordAdaptive,
		func(state *CEMCandidateState, step int, reason string) bool {
			var phaseBudget *int64
			if reason == "initial_fairness" {
				phaseBudget = explorationRemaining
				result.TargetsAttempted++
			}
			return runCEMCandidateBatch(state, group, campaign, table, order, original,
				teamIDs, cemRemaining, remainingWork, phaseBudget, adaptWorkPerSample,
				rng, &result, step, reason)
		})

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
			if result.BestCandidateTeam == 0 ||
				state.FirstStats.EliteMeanDistance-state.BestEliteDistance > result.BestCandidateDistanceImprovement {
				result.BestCandidateTeam = state.Candidate.TeamID
				result.BestCandidatePosition = state.Candidate.Position
				result.BestCandidateBatches = state.Iterations
				result.BestCandidateDistanceImprovement = state.FirstStats.EliteMeanDistance - state.BestEliteDistance
				result.BestCandidateNearTargetRate = state.BestNearTargetRate
			}
			if state.EverHadExactHit {
				result.TargetsAnyExact++
			}
			if state.ExactEliteSeen {
				result.TargetsExactElite++
			}
			if result.HighestNearTeam == 0 || state.BestNearTargetRate > result.HighestNearRate {
				result.HighestNearTeam = state.Candidate.TeamID
				result.HighestNearPosition = state.Candidate.Position
				result.HighestNearBatches = state.Iterations
				result.HighestNearRate = state.BestNearTargetRate
			}
			if len(state.Snapshots) > 0 {
				state.Candidate.SearchState.Status = StatusPromising
			} else {
				state.Candidate.SearchState.Status = StatusExhausted
			}
		}
		switch state.StopReason {
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
		result.Snapshots = append(result.Snapshots, state.Snapshots...)
	}
	if result.TargetsAttempted > 0 {
		result.AverageBatchesPerCandidate /= float64(result.TargetsAttempted)
	}
	sort.Slice(result.Snapshots, func(i, j int) bool {
		return cemSnapshotAdaptationBetter(result.Snapshots[i], result.Snapshots[j])
	})
	if len(result.Snapshots) > CEMMaxGlobalSnapshots {
		result.Snapshots = result.Snapshots[:CEMMaxGlobalSnapshots]
	}
	globalSnapshotSet := make(map[[3]int]bool, len(result.Snapshots))
	for _, snapshot := range result.Snapshots {
		globalSnapshotSet[[3]int{snapshot.CandidateTeam, snapshot.CandidatePosition,
			snapshot.SourceIteration}] = true
	}
	result.Candidates = make([]CEMCandidateState, len(states))
	for i, state := range states {
		retained := state.Snapshots[:0]
		for _, snapshot := range state.Snapshots {
			if globalSnapshotSet[[3]int{snapshot.CandidateTeam, snapshot.CandidatePosition,
				snapshot.SourceIteration}] {
				retained = append(retained, snapshot)
			}
		}
		state.Snapshots = retained
		if state.Iterations > 0 {
			if len(retained) > 0 {
				state.Candidate.SearchState.Status = StatusPromising
			} else {
				state.Candidate.SearchState.Status = StatusExhausted
			}
		}
		result.Candidates[i] = *state
	}
	return result
}

const (
	CEMBestRankDistanceForEvaluation = 2
)

type CEMSnapshotEligibility struct {
	Eligible     bool
	Reason       string
	ThetaL2      float64
	DistanceGain float64
	Priority     float64
}

func cemProposalDiffersFromP(snapshot CEMProposalSnapshot, original []GameProposalMeans) (bool, float64) {
	thetaL2 := thetaDistanceL2(snapshot.Proposal.TeamLogMultipliers, nil)
	if thetaL2 > CEMProposalThetaTolerance {
		return true, thetaL2
	}
	maxMeanDelta := 0.0
	for i := 0; i < len(original) && i < len(snapshot.Proposal.Means); i++ {
		maxMeanDelta = math.Max(maxMeanDelta, math.Abs(original[i].Home-snapshot.Proposal.Means[i].Home))
		maxMeanDelta = math.Max(maxMeanDelta, math.Abs(original[i].Away-snapshot.Proposal.Means[i].Away))
	}
	return maxMeanDelta > CEMProposalMeanTolerance, thetaL2
}

func cemSnapshotEligibility(snapshot CEMProposalSnapshot, original []GameProposalMeans) CEMSnapshotEligibility {
	differs, thetaL2 := cemProposalDiffersFromP(snapshot, original)
	eligibility := CEMSnapshotEligibility{ThetaL2: thetaL2}
	if !differs {
		eligibility.Reason = "equivalent_to_P"
		return eligibility
	}
	initialDistance := math.Max(1, snapshot.InitialEliteDistance)
	eligibility.DistanceGain = (snapshot.InitialEliteDistance - snapshot.Stats.EliteMeanDistance) / initialDistance
	switch {
	case snapshot.ExactHits > 0 || snapshot.Stats.ExactHits > 0:
		eligibility.Eligible, eligibility.Reason = true, "exact_hit"
	case snapshot.Stats.NearTargetHits > 0:
		eligibility.Eligible, eligibility.Reason = true, "near_target_hit"
	case snapshot.Stats.BestRank >= 0 && absInt(snapshot.Stats.BestRank-snapshot.CandidatePosition) <= CEMBestRankDistanceForEvaluation:
		eligibility.Eligible, eligibility.Reason = true, "best_rank_close"
	case eligibility.DistanceGain >= CEMMinRelativeDistanceImprovement:
		eligibility.Eligible, eligibility.Reason = true, "distance_improvement"
	default:
		eligibility.Reason = "insufficient_progress"
	}
	// A simple deterministic diagnostic priority. Eligibility and selection do
	// not depend on the held-out results.
	eligibility.Priority = float64(snapshot.ExactHits)*1000 +
		snapshot.Stats.NearTargetRate*100 + eligibility.DistanceGain*10 +
		math.Min(snapshot.Stats.EliteESS, 100)/100 - snapshot.Proposal.KL*0.01
	return eligibility
}

func selectCEMEvaluationSnapshots(snapshots []CEMProposalSnapshot, limit int) []CEMProposalSnapshot {
	ordered := append([]CEMProposalSnapshot(nil), snapshots...)
	sort.Slice(ordered, func(i, j int) bool {
		return cemSnapshotAdaptationBetter(ordered[i], ordered[j])
	})
	if limit <= 0 {
		return nil
	}
	selected := make([]CEMProposalSnapshot, 0, limit)
	seenTargets := make(map[[2]int]bool)
	seenThetas := make([]map[int]float64, 0, limit)
	appendUnique := func(snapshot CEMProposalSnapshot) {
		for _, theta := range seenThetas {
			if thetaDistanceL2(theta, snapshot.Proposal.TeamLogMultipliers) < CEMThetaStabilityThreshold {
				return
			}
		}
		selected = append(selected, snapshot)
		seenTargets[[2]int{snapshot.CandidateTeam, snapshot.CandidatePosition}] = true
		seenThetas = append(seenThetas, copyTheta(snapshot.Proposal.TeamLogMultipliers))
	}
	// First pass gives each distinct target a chance to be evaluated.
	for _, snapshot := range ordered {
		key := [2]int{snapshot.CandidateTeam, snapshot.CandidatePosition}
		if seenTargets[key] {
			continue
		}
		appendUnique(snapshot)
		if len(selected) == limit {
			return selected
		}
	}
	// Then use any remaining slots for the next-best retained snapshots.
	for _, snapshot := range ordered {
		alreadySelected := false
		for _, current := range selected {
			if current.CandidateTeam == snapshot.CandidateTeam &&
				current.CandidatePosition == snapshot.CandidatePosition &&
				current.SourceIteration == snapshot.SourceIteration {
				alreadySelected = true
				break
			}
		}
		if alreadySelected {
			continue
		}
		appendUnique(snapshot)
		if len(selected) == limit {
			break
		}
	}
	return selected
}

func prepareCEMEvaluationSnapshots(groupID int, snapshots []CEMProposalSnapshot,
	original []GameProposalMeans, limit int) []CEMProposalSnapshot {
	eligible := make([]CEMProposalSnapshot, 0, len(snapshots))
	eligibilityByKey := make(map[[3]int]CEMSnapshotEligibility, len(snapshots))
	for _, snapshot := range snapshots {
		eligibility := cemSnapshotEligibility(snapshot, original)
		eligibilityByKey[[3]int{snapshot.CandidateTeam, snapshot.CandidatePosition, snapshot.SourceIteration}] = eligibility
		if eligibility.Eligible {
			eligible = append(eligible, snapshot)
		}
	}
	selected := selectCEMEvaluationSnapshots(eligible, limit)
	selectedByKey := make(map[[3]int]bool, len(selected))
	for _, snapshot := range selected {
		selectedByKey[[3]int{snapshot.CandidateTeam, snapshot.CandidatePosition, snapshot.SourceIteration}] = true
	}
	for _, snapshot := range snapshots {
		key := [3]int{snapshot.CandidateTeam, snapshot.CandidatePosition, snapshot.SourceIteration}
		eligibility := eligibilityByKey[key]
		selectedThis := selectedByKey[key]
		reason := eligibility.Reason
		if eligibility.Eligible && !selectedThis {
			reason = "evaluation_limit"
			for _, chosen := range selected {
				if thetaDistanceL2(snapshot.Proposal.TeamLogMultipliers, chosen.Proposal.TeamLogMultipliers) < CEMThetaStabilityThreshold {
					reason = "duplicate_snapshot"
					break
				}
			}
		}
		log.Printf("rare-position-pilot-selection: group=%d team=%d position=%d source_iteration=%d eligible=%t selected=%t reason=%s eligible_reason=%s kl=%.6g theta_l2=%.6g exact_hits=%d near_target_hits=%d best_rank=%d snapshot_distance=%.6g initial_distance=%.6g relative_distance_improvement=%.6g priority=%.6g",
			groupID, snapshot.CandidateTeam, snapshot.CandidatePosition, snapshot.SourceIteration,
			eligibility.Eligible, selectedThis, reason, eligibility.Reason, snapshot.Proposal.KL, eligibility.ThetaL2,
			snapshot.ExactHits, snapshot.Stats.NearTargetHits, snapshot.Stats.BestRank,
			snapshot.Stats.EliteMeanDistance, snapshot.InitialEliteDistance,
			eligibility.DistanceGain, eligibility.Priority)
	}
	return selected
}

func cemEvaluationCapacity(snapshotLimit int, evaluationWork, remainingWork, workPerSnapshot int64) int {
	if snapshotLimit <= 0 || workPerSnapshot <= 0 || evaluationWork <= 0 || remainingWork <= 0 {
		return 0
	}
	capacity := int(evaluationWork / workPerSnapshot)
	if affordable := int(remainingWork / workPerSnapshot); affordable < capacity {
		capacity = affordable
	}
	if capacity > snapshotLimit {
		capacity = snapshotLimit
	}
	return capacity
}

func evaluateCEMProposalSnapshot(snapshot CEMProposalSnapshot, original []GameProposalMeans,
	baseCampaign []*TeamCampaign, games []*GameType, table *Table, order []SortType,
	teamGroups []TeamType, samples int, workPerSample int64, rng *rand.Rand,
	groupID int) CEMProposalEvaluation {
	components := cemEvaluationMixture(original, snapshot)
	validateProposalMixture(components, len(games))
	pilot := &WeightedPilotResult{Proposal: SearchProposal{
		Name: fmt.Sprintf("cem_snapshot_team%d_rank%d_iteration%d", snapshot.CandidateTeam,
			snapshot.CandidatePosition, snapshot.SourceIteration),
		TargetRank: snapshot.CandidatePosition, Components: components,
	}}
	evaluateWeightedPilot(pilot, baseCampaign, games, original, table, order,
		teamGroups, snapshot.CandidateTeam, snapshot.CandidatePosition,
		samples, workPerSample, rng, groupID)
	for i, component := range components {
		log.Printf("rare-position-evaluation-component: group=%d team=%d position=%d snapshot_iteration=%d component=%s weight=%.3f samples=%d rank_hist=%v",
			groupID, snapshot.CandidateTeam, snapshot.CandidatePosition,
			snapshot.SourceIteration, component.Name, component.Weight,
			pilot.ComponentSamples[i], pilot.ComponentRankHists[i])
	}
	evaluation := summarizeCEMProposalEvaluation(snapshot, *pilot,
		int64(pilot.Samples)*workPerSample)
	log.Printf("rare-position-evaluation: group=%d team=%d position=%d snapshot_iteration=%d samples=%d hits=%d sumY=%.8g sumY2=%.8g p=%.8g se=%.3g relSE=%.3f ess=%.3f ess_per_million_work=%.3f second_moment=%.8g max_event_weight_share=%.3f work=%d",
		groupID, snapshot.CandidateTeam, snapshot.CandidatePosition, snapshot.SourceIteration,
		evaluation.Samples, evaluation.Hits, evaluation.SumY, evaluation.SumY2,
		evaluation.Probability, evaluation.StdErr, evaluation.RelSE, evaluation.ESS,
		evaluation.ESSPerWork*1e6, evaluation.SecondMoment,
		evaluation.MaxEventWeightShare, evaluation.Work)
	return evaluation
}

func updateCEMEvaluationDiagnostics(state *CEMEvaluationState, plainWorkPerSample int64) {
	state.Exact.summarize(state.Samples)
	state.Near1.summarize(state.Samples)
	state.Near2.summarize(state.Samples)
	plainEquivalent := 0.0
	if plainWorkPerSample > 0 {
		plainEquivalent = float64(state.Work) / float64(plainWorkPerSample)
	}
	// A zero scout count is floored at one scout observation.  These ratios
	// only schedule held-out work and never enter an estimate.
	floor := 1.0 / float64(ScoutIterations)
	state.Near1EfficiencyRatio = state.Near1.ESS / math.Max(plainEquivalent*math.Max(state.ScoutNear1Probability, floor), floor)
	state.Near2EfficiencyRatio = state.Near2.ESS / math.Max(plainEquivalent*math.Max(state.ScoutNear2Probability, floor), floor)
}

func cemEvaluationStateBetter(a, b *CEMEvaluationState) bool {
	if (a.Exact.Hits > 0) != (b.Exact.Hits > 0) {
		return a.Exact.Hits > 0
	}
	if a.Exact.Hits > 0 {
		if x, y := weightedESSPerWork(a.Exact, a.Work), weightedESSPerWork(b.Exact, b.Work); x != y {
			return x > y
		}
		if a.Exact.ESS != b.Exact.ESS {
			return a.Exact.ESS > b.Exact.ESS
		}
		if a.Exact.Hits != b.Exact.Hits {
			return a.Exact.Hits > b.Exact.Hits
		}
		if a.Exact.MaxWeightShare != b.Exact.MaxWeightShare {
			return a.Exact.MaxWeightShare < b.Exact.MaxWeightShare
		}
		if a.Near1EfficiencyRatio != b.Near1EfficiencyRatio {
			return a.Near1EfficiencyRatio > b.Near1EfficiencyRatio
		}
	} else {
		if a.Near1EfficiencyRatio != b.Near1EfficiencyRatio {
			return a.Near1EfficiencyRatio > b.Near1EfficiencyRatio
		}
		if x, y := weightedESSPerWork(a.Near1, a.Work), weightedESSPerWork(b.Near1, b.Work); x != y {
			return x > y
		}
		if a.Near2EfficiencyRatio != b.Near2EfficiencyRatio {
			return a.Near2EfficiencyRatio > b.Near2EfficiencyRatio
		}
		if x, y := weightedESSPerWork(a.Near2, a.Work), weightedESSPerWork(b.Near2, b.Work); x != y {
			return x > y
		}
	}
	if a.AdaptationPriority != b.AdaptationPriority {
		return a.AdaptationPriority > b.AdaptationPriority
	}
	if a.Snapshot.Stats.EliteMeanDistance != b.Snapshot.Stats.EliteMeanDistance {
		return a.Snapshot.Stats.EliteMeanDistance < b.Snapshot.Stats.EliteMeanDistance
	}
	if a.Snapshot.Proposal.KL != b.Snapshot.Proposal.KL {
		return a.Snapshot.Proposal.KL < b.Snapshot.Proposal.KL
	}
	if a.Snapshot.CandidateTeam != b.Snapshot.CandidateTeam {
		return a.Snapshot.CandidateTeam < b.Snapshot.CandidateTeam
	}
	if a.Snapshot.CandidatePosition != b.Snapshot.CandidatePosition {
		return a.Snapshot.CandidatePosition < b.Snapshot.CandidatePosition
	}
	return a.Snapshot.SourceIteration < b.Snapshot.SourceIteration
}

func rankCEMEvaluationStates(states []*CEMEvaluationState) {
	sort.SliceStable(states, func(i, j int) bool { return cemEvaluationStateBetter(states[i], states[j]) })
}

func cemEvaluationEarlyAccept(state *CEMEvaluationState) bool {
	return state.Exact.Hits >= 2 && state.Exact.ESS >= CEMEvaluationEarlyAcceptESS &&
		state.Exact.MaxWeightShare <= 0.90
}

func evaluateCEMProposalChunk(state *CEMEvaluationState, original []GameProposalMeans,
	baseCampaign []*TeamCampaign, games []*GameType, table *Table, order []SortType,
	teamGroups []TeamType, samples int, workPerSample, plainWorkPerSample int64,
	masterSeed int64, groupID int) {
	components := cemEvaluationMixture(original, state.Snapshot)
	validateProposalMixture(components, len(games))
	stream := fmt.Sprintf("evaluation/team=%d/position=%d/iteration=%d/chunk=%d",
		state.Snapshot.CandidateTeam, state.Snapshot.CandidatePosition,
		state.Snapshot.SourceIteration, state.Chunks)
	rng := rand.New(rand.NewSource(deriveRarePositionSeed(masterSeed, stream)))
	simCampaign := make([]*TeamCampaign, len(baseCampaign))
	teamSlice := make([]*TeamCampaign, len(teamGroups))
	logQ := make([]float64, len(components))
	componentWeights := make([]float64, len(components))
	for i := 0; i < samples; i++ {
		rank, weight, _ := simulateTargetTeamRankAndWeightMulti(baseCampaign, simCampaign,
			teamSlice, games, original, components, table, order, teamGroups,
			state.Snapshot.CandidateTeam, rng, logQ, componentWeights, nil)
		distance := absInt(rank - state.Snapshot.CandidatePosition)
		state.Exact.observe(distance == 0, weight)
		state.Near1.observe(distance <= 1, weight)
		state.Near2.observe(distance <= 2, weight)
	}
	state.Samples += samples
	state.Work += int64(samples) * workPerSample
	state.Chunks++
	updateCEMEvaluationDiagnostics(state, plainWorkPerSample)
	log.Printf("rare-position-evaluation-chunk: group=%d team=%d position=%d snapshot_iteration=%d stage=%d chunk=%d chunk_samples=%d cumulative_samples=%d cumulative_work=%d exact_hits=%d exact_ess=%.3f exact_ess_per_work=%.8g exact_max_weight_share=%.3f near1_hits=%d near1_ess=%.3f near1_efficiency_ratio=%.3f near2_hits=%d near2_ess=%.3f near2_efficiency_ratio=%.3f",
		groupID, state.Snapshot.CandidateTeam, state.Snapshot.CandidatePosition,
		state.Snapshot.SourceIteration, state.Stage, state.Chunks, samples, state.Samples,
		state.Work, state.Exact.Hits, state.Exact.ESS, weightedESSPerWork(state.Exact, state.Work),
		state.Exact.MaxWeightShare, state.Near1.Hits, state.Near1.ESS,
		state.Near1EfficiencyRatio, state.Near2.Hits, state.Near2.ESS, state.Near2EfficiencyRatio)
}

func logCEMEvaluationRace(groupID, stage int, ranked []*CEMEvaluationState) {
	for i, state := range ranked {
		status, reason := "survivor", state.EliminationReason
		if state.Eliminated {
			status = "eliminated"
		} else if i == 0 {
			status = "leader"
		}
		tier := "near_target"
		if state.Exact.Hits > 0 {
			tier = "exact"
		}
		log.Printf("rare-position-evaluation-race: group=%d stage=%d rank=%d team=%d position=%d snapshot_iteration=%d samples=%d score_tier=%s exact_hits=%d exact_ess_per_work=%.8g near1_efficiency_ratio=%.3f near2_efficiency_ratio=%.3f adaptation_priority=%.6g status=%s reason=%s",
			groupID, stage, i+1, state.Snapshot.CandidateTeam, state.Snapshot.CandidatePosition,
			state.Snapshot.SourceIteration, state.Samples, tier, state.Exact.Hits,
			weightedESSPerWork(state.Exact, state.Work), state.Near1EfficiencyRatio,
			state.Near2EfficiencyRatio, state.AdaptationPriority, status, reason)
	}
}

// raceCEMEvaluationSnapshots evaluates the already-frozen shortlist.  It
// returns all diagnostics, the eligible winner, work consumed, and whether it
// stopped on independent repeated exact evidence.
func raceCEMEvaluationSnapshots(snapshots []CEMProposalSnapshot, scoutCounts map[int][]int,
	scoutSamples int, original []GameProposalMeans, baseCampaign []*TeamCampaign,
	games []*GameType, table *Table, order []SortType, teamGroups []TeamType,
	workBudget, workPerSample, plainWorkPerSample int64, masterSeed int64,
	groupID int) ([]*CEMEvaluationState, *CEMProposalEvaluation, int64, bool) {
	maxSamples := CEMMaxEvaluationSamples
	if workPerSample <= 0 {
		return nil, nil, 0, false
	}
	if affordable := int(workBudget / workPerSample); affordable < maxSamples {
		maxSamples = affordable
	}
	states := make([]*CEMEvaluationState, 0, len(snapshots))
	for _, snapshot := range snapshots {
		state := &CEMEvaluationState{Snapshot: snapshot, Stage: 1,
			AdaptationPriority: cemSnapshotEligibility(snapshot, original).Priority}
		counts := scoutCounts[snapshot.CandidateTeam]
		if scoutSamples > 0 {
			for rank, count := range counts {
				d := absInt(rank - snapshot.CandidatePosition)
				if d <= 1 {
					state.ScoutNear1Probability += float64(count) / float64(scoutSamples)
				}
				if d <= 2 {
					state.ScoutNear2Probability += float64(count) / float64(scoutSamples)
				}
			}
		}
		states = append(states, state)
	}
	totalSamples := 0
	// Stage 1 is deliberately breadth-first: no observation can eliminate a
	// candidate before every frozen candidate gets its screening chunk.
	for _, state := range states {
		if totalSamples+CEMEvaluationChunkSamples > maxSamples {
			break
		}
		evaluateCEMProposalChunk(state, original, baseCampaign, games, table, order,
			teamGroups, CEMEvaluationChunkSamples, workPerSample, plainWorkPerSample,
			masterSeed, groupID)
		totalSamples += CEMEvaluationChunkSamples
	}
	rankCEMEvaluationStates(states)
	for i := 2; i < len(states); i++ {
		states[i].Eliminated = true
		states[i].EliminationReason = "stage1_bottom_rank"
	}
	logCEMEvaluationRace(groupID, 1, states)
	survivors := states
	if len(survivors) > 2 {
		survivors = survivors[:2]
	}
	// Stage 2 gives both survivors equal additional evidence.
	for _, state := range survivors {
		if totalSamples+CEMEvaluationChunkSamples > maxSamples {
			break
		}
		state.Stage = 2
		evaluateCEMProposalChunk(state, original, baseCampaign, games, table, order,
			teamGroups, CEMEvaluationChunkSamples, workPerSample, plainWorkPerSample,
			masterSeed, groupID)
		totalSamples += CEMEvaluationChunkSamples
	}
	rankCEMEvaluationStates(survivors)
	logCEMEvaluationRace(groupID, 2, survivors)
	early := false
	// Stage 3 re-ranks after every chunk, allowing the other survivor to lead.
	for len(survivors) > 0 && totalSamples+CEMEvaluationChunkSamples <= maxSamples {
		rankCEMEvaluationStates(survivors)
		leader := survivors[0]
		if cemEvaluationEarlyAccept(leader) {
			early = true
			break
		}
		if leader.Samples >= CEMEvaluationMaxSamplesPerSnapshot {
			if len(survivors) < 2 || survivors[1].Samples >= CEMEvaluationMaxSamplesPerSnapshot {
				break
			}
			leader = survivors[1]
		}
		leader.Stage = 3
		evaluateCEMProposalChunk(leader, original, baseCampaign, games, table, order,
			teamGroups, CEMEvaluationChunkSamples, workPerSample, plainWorkPerSample,
			masterSeed, groupID)
		totalSamples += CEMEvaluationChunkSamples
		rankCEMEvaluationStates(survivors)
		logCEMEvaluationRace(groupID, 3, survivors)
	}
	rankCEMEvaluationStates(survivors)
	if early {
		for _, state := range states {
			if state != survivors[0] && !state.Eliminated {
				state.EliminationReason = "early_accept_other_snapshot"
			}
		}
	} else {
		for _, state := range survivors {
			if state != survivors[0] {
				state.EliminationReason = "stage2_not_leader"
			}
		}
		if len(survivors) > 0 {
			survivors[0].EliminationReason = "evaluation_budget_exhausted"
		}
	}
	var winner *CEMProposalEvaluation
	if len(survivors) > 0 && survivors[0].Exact.Hits > 0 && survivors[0].Exact.ESS > 0 &&
		!math.IsNaN(survivors[0].Exact.SumY) && !math.IsInf(survivors[0].Exact.SumY, 0) {
		s := survivors[0]
		winner = &CEMProposalEvaluation{Snapshot: s.Snapshot, Samples: s.Samples,
			Hits: s.Exact.Hits, SumY: s.Exact.SumY, SumY2: s.Exact.SumY2,
			Probability: s.Exact.Probability, StdErr: s.Exact.StdErr, RelSE: s.Exact.RelSE,
			ESS: s.Exact.ESS, ESSPerWork: weightedESSPerWork(s.Exact, s.Work),
			MaxEventWeightShare: s.Exact.MaxWeightShare, Work: s.Work}
	}
	return states, winner, int64(totalSamples) * workPerSample, early
}

func summarizeCEMProposalEvaluation(snapshot CEMProposalSnapshot,
	pilot WeightedPilotResult, work int64) CEMProposalEvaluation {
	secondMoment := 0.0
	if pilot.Samples > 0 {
		secondMoment = pilot.SumY2 / float64(pilot.Samples)
	}
	evaluation := CEMProposalEvaluation{
		Snapshot: snapshot, Samples: pilot.Samples, Hits: pilot.Hits,
		SumY: pilot.SumY, SumY2: pilot.SumY2, Probability: pilot.Probability,
		StdErr: pilot.StdErr, RelSE: pilot.RelSE, ESS: pilot.ESS,
		ESSPerWork: pilot.ESSPerWork, SecondMoment: secondMoment,
		MaxEventWeightShare: pilot.MaxEventWeightShare,
		Work:                work,
	}
	if work > 0 {
		evaluation.ESSPerWork = evaluation.ESS / float64(work)
	}
	return evaluation
}

func selectCEMProductionEvaluation(evaluations []CEMProposalEvaluation) *CEMProposalEvaluation {
	var best *CEMProposalEvaluation
	for i := range evaluations {
		candidate := &evaluations[i]
		if candidate.Hits == 0 || candidate.ESS <= 0 || candidate.Work <= 0 {
			continue
		}
		if best == nil || cemSnapshotBetterForEvaluation(*candidate, *best) {
			best = candidate
		}
	}
	return best
}
