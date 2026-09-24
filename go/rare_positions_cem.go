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
	CEMBatchSamples                        = 300
	CEMMaxIterationsPerCandidate           = 12
	CEMEliteFraction                       = 0.15
	CEMSmoothing                           = 0.5
	CEMMaxKL                               = 3.0
	CEMExactEventThreshold                 = 5
	CEMValidationSamples                   = 500 // legacy-only; not used by active orchestration
	CEMConfirmationChunkSamples            = 300 // legacy-only; not used by active orchestration
	CEMMaxConfirmationChunks               = 3   // legacy-only; not used by active orchestration
	CEMMinAdaptationHitsForConfirmation    = 1   // legacy-only; not used by active orchestration
	CEMMinConfirmationHits                 = 1   // legacy-only; not used by active orchestration
	CEMMaxInitialCandidates                = 6
	CEMConfirmationMaxEventShare           = 0.95 // legacy-only
	CEMMaxExplorationFraction              = 0.40
	MaxCEMWorkFraction                     = 0.10
	MaxCEMEvaluationWorkFraction           = 0.02
	MaxCEMValidationWorkFraction           = 0.02 // legacy-only
	MaxCEMConfirmationWorkFraction         = 0.03 // legacy-only
	MaxCEMPlainEquivalentSamples           = 5000
	CEMMaxSnapshotsPerCandidate            = 3
	CEMMaxGlobalSnapshots                  = 12
	CEMMaxEvaluationSnapshots              = 3
	CEMEvaluationSamplesPerSnapshot        = 300
	CEMEvaluationChunkSamples              = 100
	CEMMaxEvaluationSamples                = 900
	CEMAdaptationExactMinEvaluationSamples = 400
	CEMEvaluationEarlyAcceptESS            = 2.0
	CEMMinEliteESSForUpdate                = 8.0
	CEMMeaningfulTeamLogShift              = 0.03
	CEMThetaStabilityThreshold             = 0.02
	CEMProposalThetaTolerance              = 1e-4
	CEMProposalMeanTolerance               = 1e-6
	CEMMinRelativeDistanceImprovement      = 0.20
	CEMMinMultiplier                       = 1e-12
	CEMMinBatchSamples                     = 100
	CEMMinRelativeProgress                 = 0.02
	CEMMaxStalledIterations                = 2
	CEMAttackDefenseFitMaxIterations       = 50
	CEMAttackDefenseFitTolerance           = 1e-8
	CEMAttackDefenseMomentFloor            = 1e-12
)

type CEMParameterization int

const (
	CEMTeamScoring CEMParameterization = iota
	CEMTeamAttackConcession
)

func cemParameterizationName(parameterization CEMParameterization) string {
	if parameterization == CEMTeamAttackConcession {
		return "team_attack_concession"
	}
	return "team_scoring"
}

func cemParameterizationFromEnvironment() CEMParameterization {
	if override := os.Getenv("RARE_POSITION_BENCHMARK_CEM_PARAMETERIZATION"); override != "" {
		if override == "team_attack_concession" {
			return CEMTeamAttackConcession
		}
		return CEMTeamScoring
	}
	if os.Getenv("RARE_POSITION_CEM_PARAMETERIZATION") == "team_attack_concession" {
		return CEMTeamAttackConcession
	}
	return CEMTeamScoring
}

type CEMProposal struct {
	Parameterization        CEMParameterization
	TeamLogMultipliers      map[int]float64
	AttackTheta             map[int]float64
	ConcessionTheta         map[int]float64
	PreviousAttackTheta     map[int]float64
	PreviousConcessionTheta map[int]float64
	PreviousLogTheta        map[int]float64
	EliteTeamGoals          map[int]float64
	EliteTeamGoalsConceded  map[int]float64
	Means                   []GameProposalMeans
	Iteration               int
	KL                      float64
	ChangedTeams            int
	MaxAbsTheta             float64
	ThetaDeltaL2            float64
	EliteESS                float64
	UpdateAllowed           bool
	FitIterations           int
	FitConverged            bool
	FitObjectiveStart       float64
	FitObjectiveEnd         float64
	FitUsedMomentFloor      bool
	FitWallTime             time.Duration
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
	Rank               int
	TeamGoals          []int
	TeamGoalsConceded  []int
	TeamScoredMoment   []float64
	TeamConcededMoment []float64
	LogWeight          float64 // log(P / Q_current), never a probability estimate
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
	CEMWork                           int64
	AttackConcessionFitWallTime       time.Duration
	AttackConcessionFitIterations     int
	AttackConcessionFitUpdates        int
	AttackConcessionFitConverged      int
	AttackConcessionFitObjectiveStart float64
	AttackConcessionFitObjectiveEnd   float64
	InitialAttackL2                   float64
	InitialConcessionL2               float64
	EliteDistanceAt300                float64
	EliteDistanceAt600                float64
	EliteDistanceAt900                float64
	MeanSamplesToFirstNear            float64
	MeanSamplesToFirstExact           float64
	BestNearTargetRate                float64
	BestExactRate                     float64
	CandidatesAt300                   int
	CandidatesAt600                   int
	CandidatesAt900                   int
	CandidatesWithFirstNear           int
	CandidatesWithFirstExact          int
	RepeatedExactHitBatches           int
	AdaptationExactHitBatches         int
	Eligible                          []*FrontierCandidate // legacy-only
	ValidationWork                    int64                // legacy-only
	ConfirmationWork                  int64                // legacy-only
	ConfirmationAttempts              int                  // legacy-only
	ConfirmationChunks                int                  // legacy-only
	ConfirmationSamples               int                  // legacy-only
	ConfirmationSuccesses             int                  // legacy-only
	ConfirmationFailures              int                  // legacy-only
	ConfirmationReadyCandidates       int                  // legacy-only
	ConfirmationSingleHitSuccesses    int                  // legacy-only
	ConfirmationMultiHitSuccesses     int                  // legacy-only
	ConfirmationFailedResumed         int                  // legacy-only
	ConfirmationFailedExhausted       int                  // legacy-only
	ConfirmationInconclusiveBudget    int                  // legacy-only
	ConfirmationHits                  int                  // legacy-only
	ValidationAttempts                int                  // legacy-only
	ValidationSuccesses               int                  // legacy-only
	CandidatesTotal                   int
	CandidatesAdmitted                int
	CandidatesNotAdmitted             int
	ExplorationSamples                int
	AdaptiveSamples                   int
	TargetsAttempted                  int
	Iterations                        int
	TargetsAnyExact                   int
	TargetsExactElite                 int
	TargetsValidated                  int // legacy-only
	Snapshots                         []CEMProposalSnapshot
	ChangedTeams                      int
	MaxChangedTeams                   int
	AbsThetaSum                       float64
	ThetaUpdates                      int
	ThetaParameterCount               int
	MaxAbsTheta                       float64
	ThetaDeltaL2Sum                   float64
	ValidatedESS                      float64 // legacy-only
	SchedulerBatches                  int
	OneBatchCandidates                int
	MultiBatchCandidates              int
	MaxBatchesPerCandidate            int
	AverageBatchesPerCandidate        float64
	BestCandidateTeam                 int
	BestCandidatePosition             int
	BestCandidateBatches              int
	BestCandidateDistanceImprovement  float64
	BestCandidateNearTargetRate       float64
	HighestNearTeam                   int
	HighestNearPosition               int
	HighestNearBatches                int
	HighestNearRate                   float64
	StopValidationReady               int // legacy-only
	StopStalled                       int
	StopLowEliteESS                   int
	StopRegression                    int
	StopPerCandidateCap               int
	StopGlobalBudget                  int
	Candidates                        []CEMCandidateState
}

type CEMCandidateState struct {
	Candidate                      *FrontierCandidate
	Proposal                       CEMProposal
	InitialProposal                CEMProposal
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
	FirstNearTargetSamples         int
	FirstExactSamples              int
	AdaptationExactHitBatches      int
	EliteDistanceAt300             float64
	EliteDistanceAt600             float64
	EliteDistanceAt900             float64
	HasEliteDistanceAt300          bool
	HasEliteDistanceAt600          bool
	HasEliteDistanceAt900          bool
	BestExactRate                  float64
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
	CandidateTeam            int
	CandidatePosition        int
	SourceIteration          int
	HadAdaptationExactHit    bool
	AdaptationExactHits      int
	AdaptationNearTargetHits int
	Proposal                 CEMProposal
	Stats                    CEMBatchStats
	InitialEliteDistance     float64
	ExactHits                int
	SearchScore              float64
	Reason                   string
}

type WeightedEventStats struct {
	Hits           int
	SumY           float64
	SumY2          float64
	Probability    float64
	StdErr         float64
	RelSE          float64
	ESS            float64
	MaxWeightShare float64
}

type CEMEvaluationState struct {
	Snapshot                 CEMProposalSnapshot
	HadAdaptationExactHit    bool
	AdaptationExactHits      int
	AdaptationNearTargetHits int
	Stage                    int
	Samples                  int
	Work                     int64
	Chunks                   int
	SamplesAt200             int
	ExactHitsAt200           int
	SamplesAt400             int
	ExactHitsAt400           int
	SamplesAt600             int
	ExactHitsAt600           int
	Exact                    WeightedEventStats
	Near1                    WeightedEventStats
	Near2                    WeightedEventStats
	Near1EfficiencyRatio     float64
	Near2EfficiencyRatio     float64
	AdaptationPriority       float64
	Eliminated               bool
	EliminationReason        string
}

type CEMEvaluationResult struct {
	Evaluations     []CEMProposalEvaluation
	Selected        *CEMProposalEvaluation
	TotalSamples    int
	TotalWork       int64
	SelectedSamples int
	SelectedWork    int64
	EarlyAccept     bool
	Reason          string
}

type CEMProposalEvaluation struct {
	Snapshot             CEMProposalSnapshot
	Samples              int
	SamplesAt200         int
	ExactHitsAt200       int
	SamplesAt400         int
	ExactHitsAt400       int
	SamplesAt600         int
	ExactHitsAt600       int
	Hits                 int
	SumY                 float64
	SumY2                float64
	Probability          float64
	StdErr               float64
	RelSE                float64
	ESS                  float64
	ESSPerWork           float64
	SecondMoment         float64
	MaxEventWeightShare  float64
	Work                 int64
	Exact                WeightedEventStats
	Near1                WeightedEventStats
	Near2                WeightedEventStats
	Near1EfficiencyRatio float64
	Near2EfficiencyRatio float64
	EarlyAccept          bool
}

func teamIDsFromGroups(groups []TeamType) []int {
	ids := make([]int, len(groups))
	for i, group := range groups {
		ids[i] = group.Team_id
	}
	sort.Ints(ids)
	return ids
}

type CEMInitializationMode int

const (
	CEMInitZero CEMInitializationMode = iota
	CEMInitStandingsDirected
	CEMWarmStartCompetitorMass = 1.0
	CEMWarmStartKL             = 0.15
)

var CEMDefaultInitializationMode = CEMInitStandingsDirected

func cemInitializationMode() (CEMInitializationMode, string) {
	if value := os.Getenv("RARE_POSITION_BENCHMARK_CEM_INIT_MODE"); value != "" {
		if value == "zero" {
			return CEMInitZero, "benchmark_override"
		}
		if value == "standings_directed" || value == "directed" {
			return CEMInitStandingsDirected, "benchmark_override"
		}
	}
	value := os.Getenv("RARE_POSITION_CEM_INIT_MODE")
	if value == "" {
		value = os.Getenv("RARE_POSITION_CEM_INIT") // compatibility with the prior experiment switch
	}
	if value != "" {
		if value == "zero" {
			return CEMInitZero, "config"
		}
		if value == "standings_directed" || value == "directed" {
			return CEMInitStandingsDirected, "config"
		}
	}
	return CEMDefaultInitializationMode, "default"
}

func cemInitializationModeName(mode CEMInitializationMode) string {
	if mode == CEMInitStandingsDirected {
		return "standings_directed"
	}
	return "zero"
}

func newZeroCEMProposal(original []GameProposalMeans, games []*GameType, teamIDs []int) CEMProposal {
	return newCEMProposal(original, games, teamIDs)
}

func newCEMProposalWithMode(mode CEMInitializationMode, candidate *FrontierCandidate,
	searches map[int]*TeamRareSearch, original []GameProposalMeans, games []*GameType,
	teamIDs []int, groupID int) (CEMProposal, map[int]float64) {
	return newCEMProposalWithModeAndParameterization(mode, CEMTeamScoring, candidate, searches,
		original, games, teamIDs, groupID)
}

func newCEMProposalWithModeAndParameterization(mode CEMInitializationMode,
	parameterization CEMParameterization, candidate *FrontierCandidate,
	searches map[int]*TeamRareSearch, original []GameProposalMeans, games []*GameType,
	teamIDs []int, groupID int) (CEMProposal, map[int]float64) {
	if mode == CEMInitStandingsDirected {
		return initializeDirectedCEMProposalWithParameterization(parameterization, candidate,
			searches, original, games, teamIDs, groupID)
	}
	return newCEMProposalForParameterization(parameterization, original, games, teamIDs), nil
}

// initializeDirectedCEMProposal preserves the existing standings corridor and
// KL trust-region warm-start construction used by the earlier PR iteration.
func initializeDirectedCEMProposal(candidate *FrontierCandidate, searches map[int]*TeamRareSearch,
	original []GameProposalMeans, games []*GameType, teamIDs []int, groupID int) (CEMProposal, map[int]float64) {
	return initializeDirectedCEMProposalWithParameterization(CEMTeamScoring, candidate,
		searches, original, games, teamIDs, groupID)
}

func initializeDirectedCEMProposalWithParameterization(parameterization CEMParameterization,
	candidate *FrontierCandidate, searches map[int]*TeamRareSearch,
	original []GameProposalMeans, games []*GameType, teamIDs []int, groupID int) (CEMProposal, map[int]float64) {
	targetTeamID, targetPosition, direction := candidate.TeamID, candidate.Position, candidate.Direction
	targetSearch := searches[targetTeamID]
	if targetSearch == nil {
		log.Printf("rare-position-cem-init-fallback: group=%d team=%d position=%d reason=no_target_search", groupID, targetTeamID, targetPosition)
		return newCEMProposalForParameterization(parameterization, original, games, teamIDs), nil
	}
	normalRank := targetSearch.NormalMeanRank
	corridor := make([]int, 0)
	role := "blocker"
	for teamID, search := range searches {
		if teamID == targetTeamID || search == nil {
			continue
		}
		rank := search.NormalMeanRank
		if direction == RareBetter && float64(targetPosition) <= rank && rank < normalRank ||
			direction == RareWorse && normalRank < rank && rank <= float64(targetPosition) {
			corridor = append(corridor, teamID)
		}
	}
	if direction == RareWorse {
		role = "overtaker"
	}
	if len(corridor) == 0 {
		for teamID, search := range searches {
			if teamID != targetTeamID && search != nil && math.Abs(search.NormalMeanRank-float64(targetPosition)) <= 1 {
				corridor = append(corridor, teamID)
			}
		}
	}
	if len(corridor) == 0 {
		log.Printf("rare-position-cem-init-fallback: group=%d team=%d position=%d reason=no_corridor_competitors", groupID, targetTeamID, targetPosition)
		return newCEMProposalForParameterization(parameterization, original, games, teamIDs), nil
	}
	sort.Ints(corridor)
	rawWeights, normalized := make(map[int]float64, len(corridor)), make(map[int]float64, len(corridor))
	sumRaw := 0.0
	for _, teamID := range corridor {
		weight := 1 / (1 + math.Abs(searches[teamID].NormalMeanRank-float64(targetPosition)))
		rawWeights[teamID], sumRaw = weight, sumRaw+weight
	}
	for _, teamID := range corridor {
		normalized[teamID] = rawWeights[teamID] / sumRaw
	}
	theta := make(map[int]float64, len(teamIDs))
	for _, teamID := range teamIDs {
		theta[teamID] = 0
	}
	directionSign := 1.0
	if direction == RareWorse {
		directionSign = -1
	}
	theta[targetTeamID] = directionSign
	for _, teamID := range corridor {
		if direction == RareBetter {
			theta[teamID] = -CEMWarmStartCompetitorMass * normalized[teamID]
		} else {
			theta[teamID] = CEMWarmStartCompetitorMass * normalized[teamID]
		}
	}
	unscaled := cemProposalFromStrengthTheta(parameterization, theta)
	unscaled.UpdateAllowed = true
	unscaled.Means = materializeCEMProposal(unscaled, original, games)
	unscaled.KL = cemTotalKL(unscaled.Means, original, games)
	alpha := 0.0
	proposal := newCEMProposalForParameterization(parameterization, original, games, teamIDs)
	if unscaled.KL > 0 {
		proposal = cemTrustRegion(unscaled, original, games, CEMWarmStartKL)
		if parameterization == CEMTeamScoring && proposal.TeamLogMultipliers[targetTeamID] != 0 {
			alpha = proposal.TeamLogMultipliers[targetTeamID] / directionSign
		} else if parameterization == CEMTeamAttackConcession {
			alpha = 2 * proposal.AttackTheta[targetTeamID] / directionSign
		}
	}
	directionName := "better"
	if direction == RareWorse {
		directionName = "worse"
	}
	log.Printf("rare-position-cem-init: group=%d team=%d position=%d mode=standings_directed direction=%s normal_mean_rank=%.2f target_position=%d competitor_count=%d competitor_mass=%.2f target_direction=%.1f unscaled_theta_l1=%.4f alpha=%.4f kl=%.4f",
		groupID, targetTeamID, targetPosition, directionName, normalRank, targetPosition, len(corridor), CEMWarmStartCompetitorMass,
		directionSign, sumAbsTheta(theta), alpha, proposal.KL)
	if parameterization == CEMTeamScoring {
		log.Printf("rare-position-cem-init-team: group=%d target_team=%d target_position=%d team_id=%d role=target normal_mean_rank=%.2f rank_distance_to_boundary=%.2f raw_relevance=1.0000 normalized_relevance=1.0000 direction=%.1f initial_theta=%.6f initial_multiplier=%.6f",
			groupID, targetTeamID, targetPosition, targetTeamID, normalRank, math.Abs(normalRank-float64(targetPosition)), directionSign,
			proposal.TeamLogMultipliers[targetTeamID], math.Exp(proposal.TeamLogMultipliers[targetTeamID]))
	} else {
		log.Printf("rare-position-cem-init-team-ad: group=%d target_team=%d target_position=%d team_id=%d role=target initial_attack=%.6f initial_concede=%.6f effective_strength=%.6f",
			groupID, targetTeamID, targetPosition, targetTeamID, proposal.AttackTheta[targetTeamID],
			proposal.ConcessionTheta[targetTeamID], proposal.AttackTheta[targetTeamID]-proposal.ConcessionTheta[targetTeamID])
	}
	for _, teamID := range corridor {
		rank := searches[teamID].NormalMeanRank
		competitorDirection := -directionSign
		if parameterization == CEMTeamScoring {
			log.Printf("rare-position-cem-init-team: group=%d target_team=%d target_position=%d team_id=%d role=%s normal_mean_rank=%.2f rank_distance_to_boundary=%.2f raw_relevance=%.4f normalized_relevance=%.4f direction=%.1f initial_theta=%.6f initial_multiplier=%.6f",
				groupID, targetTeamID, targetPosition, teamID, role, rank, math.Abs(rank-float64(targetPosition)),
				rawWeights[teamID], normalized[teamID], competitorDirection, proposal.TeamLogMultipliers[teamID], math.Exp(proposal.TeamLogMultipliers[teamID]))
		} else {
			log.Printf("rare-position-cem-init-team-ad: group=%d target_team=%d target_position=%d team_id=%d role=%s normal_mean_rank=%.2f raw_relevance=%.4f initial_attack=%.6f initial_concede=%.6f effective_strength=%.6f",
				groupID, targetTeamID, targetPosition, teamID, role, rank, rawWeights[teamID],
				proposal.AttackTheta[teamID], proposal.ConcessionTheta[teamID],
				proposal.AttackTheta[teamID]-proposal.ConcessionTheta[teamID])
		}
	}
	if parameterization == CEMTeamAttackConcession {
		attackL2, concedeL2 := 0.0, 0.0
		for _, value := range proposal.AttackTheta {
			attackL2 += value * value
		}
		for _, value := range proposal.ConcessionTheta {
			concedeL2 += value * value
		}
		log.Printf("rare-position-cem-init-family: group=%d team=%d position=%d parameterization=%s attack_l2=%.6g concede_l2=%.6g kl=%.6g target_kl=%.6g",
			groupID, targetTeamID, targetPosition, cemParameterizationName(parameterization),
			math.Sqrt(attackL2), math.Sqrt(concedeL2), proposal.KL, CEMWarmStartKL)
	}
	return proposal, theta
}

func newCEMProposal(original []GameProposalMeans, games []*GameType, teamIDs []int) CEMProposal {
	theta := make(map[int]float64, len(teamIDs))
	for _, teamID := range teamIDs {
		theta[teamID] = 0
	}
	proposal := CEMProposal{Parameterization: CEMTeamScoring, TeamLogMultipliers: theta, UpdateAllowed: true}
	proposal.Means = materializeCEMProposal(proposal, original, games)
	return proposal
}

func newCEMProposalForParameterization(parameterization CEMParameterization,
	original []GameProposalMeans, games []*GameType, teamIDs []int) CEMProposal {
	if parameterization == CEMTeamScoring {
		return newCEMProposal(original, games, teamIDs)
	}
	proposal := CEMProposal{Parameterization: CEMTeamAttackConcession,
		AttackTheta:     make(map[int]float64, len(teamIDs)),
		ConcessionTheta: make(map[int]float64, len(teamIDs)), UpdateAllowed: true}
	for _, teamID := range teamIDs {
		proposal.AttackTheta[teamID] = 0
		proposal.ConcessionTheta[teamID] = 0
	}
	proposal.Means = materializeCEMProposal(proposal, original, games)
	return proposal
}

func cemProposalFromStrengthTheta(parameterization CEMParameterization, theta map[int]float64) CEMProposal {
	if parameterization == CEMTeamScoring {
		return CEMProposal{TeamLogMultipliers: copyTheta(theta)}
	}
	proposal := CEMProposal{Parameterization: CEMTeamAttackConcession,
		AttackTheta:     make(map[int]float64, len(theta)),
		ConcessionTheta: make(map[int]float64, len(theta))}
	for teamID, value := range theta {
		proposal.AttackTheta[teamID] = value / 2
		proposal.ConcessionTheta[teamID] = -value / 2
	}
	return normalizeCEMGauge(proposal)
}

func materializeCEMProposal(proposal CEMProposal, original []GameProposalMeans, games []*GameType) []GameProposalMeans {
	means := append([]GameProposalMeans(nil), original...)
	for i, game := range games {
		if game.Played {
			continue
		}
		homeTheta := proposal.TeamLogMultipliers[game.HomeId]
		awayTheta := proposal.TeamLogMultipliers[game.AwayId]
		if proposal.Parameterization == CEMTeamAttackConcession {
			homeTheta = proposal.AttackTheta[game.HomeId] + proposal.ConcessionTheta[game.AwayId]
			awayTheta = proposal.AttackTheta[game.AwayId] + proposal.ConcessionTheta[game.HomeId]
		}
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
	return cemTrustRegion(proposal, original, games, maxKL)
}

func normalizeCEMGauge(proposal CEMProposal) CEMProposal {
	if proposal.Parameterization != CEMTeamAttackConcession || len(proposal.ConcessionTheta) == 0 {
		return proposal
	}
	proposal.AttackTheta = copyTheta(proposal.AttackTheta)
	proposal.ConcessionTheta = copyTheta(proposal.ConcessionTheta)
	mean := 0.0
	teamIDs := make([]int, 0, len(proposal.ConcessionTheta))
	for teamID := range proposal.ConcessionTheta {
		teamIDs = append(teamIDs, teamID)
	}
	sort.Ints(teamIDs)
	for _, teamID := range teamIDs {
		mean += proposal.ConcessionTheta[teamID]
	}
	mean /= float64(len(proposal.ConcessionTheta))
	for _, teamID := range teamIDs {
		value := proposal.ConcessionTheta[teamID]
		proposal.ConcessionTheta[teamID] = value - mean
		proposal.AttackTheta[teamID] += mean
	}
	return proposal
}

func scaleCEMProposal(proposal CEMProposal, scale float64) CEMProposal {
	proposal = cloneCEMProposal(proposal)
	if proposal.Parameterization == CEMTeamAttackConcession {
		for teamID, value := range proposal.AttackTheta {
			proposal.AttackTheta[teamID] = value * scale
		}
		for teamID, value := range proposal.ConcessionTheta {
			proposal.ConcessionTheta[teamID] = value * scale
		}
		return normalizeCEMGauge(proposal)
	}
	for teamID, value := range proposal.TeamLogMultipliers {
		proposal.TeamLogMultipliers[teamID] = value * scale
	}
	return proposal
}

func cemTrustRegion(proposal CEMProposal, original []GameProposalMeans, games []*GameType, maxKL float64) CEMProposal {
	if proposal.KL <= maxKL {
		return proposal
	}
	lo, hi := 0.0, 1.0
	for i := 0; i < 55; i++ {
		mid := (lo + hi) / 2
		candidate := scaleCEMProposal(proposal, mid)
		candidate.Means = materializeCEMProposal(candidate, original, games)
		candidate.KL = cemTotalKL(candidate.Means, original, games)
		if candidate.KL <= maxKL {
			lo = mid
		} else {
			hi = mid
		}
	}
	scaled := scaleCEMProposal(proposal, lo)
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
	if current.Parameterization == CEMTeamAttackConcession {
		return cemUpdateAttackConcession(current, original, games, teamIDs, seasons, elite)
	}
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
	updated.EliteTeamGoalsConceded = make(map[int]float64, len(teamIDs))
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

func cemUpdateAttackConcession(current CEMProposal, original []GameProposalMeans,
	games []*GameType, teamIDs []int, seasons []CEMSeason, elite []int) CEMProposal {
	weights, eliteESS := cemEliteWeights(seasons, elite)
	updated := cloneCEMProposal(current)
	updated.EliteESS = eliteESS
	if len(elite) == 0 || eliteESS < CEMMinEliteESSForUpdate {
		updated.UpdateAllowed = false
		return updated
	}
	started := time.Now()
	updated.PreviousAttackTheta = copyTheta(current.AttackTheta)
	updated.PreviousConcessionTheta = copyTheta(current.ConcessionTheta)
	scored, conceded := make(map[int]float64, len(teamIDs)), make(map[int]float64, len(teamIDs))
	updated.EliteTeamGoals = make(map[int]float64, len(teamIDs))
	updated.EliteTeamGoalsConceded = make(map[int]float64, len(teamIDs))
	for i, teamID := range teamIDs {
		for j, seasonIndex := range elite {
			season := seasons[seasonIndex]
			if i < len(season.TeamScoredMoment) {
				scored[teamID] += weights[j] * season.TeamScoredMoment[i]
			} else if i < len(season.TeamGoals) {
				scored[teamID] += weights[j] * float64(season.TeamGoals[i])
			}
			if i < len(season.TeamConcededMoment) {
				conceded[teamID] += weights[j] * season.TeamConcededMoment[i]
			} else if i < len(season.TeamGoalsConceded) {
				conceded[teamID] += weights[j] * float64(season.TeamGoalsConceded[i])
			}
		}
		updated.EliteTeamGoals[teamID] = scored[teamID]
		updated.EliteTeamGoalsConceded[teamID] = conceded[teamID]
	}
	weightSum := 0.0
	for _, weight := range weights {
		weightSum += weight
	}
	if weightSum <= 0 {
		updated.UpdateAllowed = false
		return updated
	}
	for _, teamID := range teamIDs {
		scored[teamID] /= weightSum
		conceded[teamID] /= weightSum
		updated.EliteTeamGoals[teamID] = scored[teamID]
		updated.EliteTeamGoalsConceded[teamID] = conceded[teamID]
	}
	fit := cloneCEMProposal(current)
	fit = normalizeCEMGauge(fit)
	fitObjective := cemAttackConcessionObjective(fit, original, games, teamIDs, scored, conceded)
	updated.FitObjectiveStart = fitObjective
	floorUsed := false
	for iteration := 1; iteration <= CEMAttackDefenseFitMaxIterations; iteration++ {
		maxDelta := 0.0
		for _, teamID := range teamIDs {
			denom := cemAttackDenominator(teamID, fit.ConcessionTheta, original, games)
			moment := scored[teamID]
			if moment < CEMAttackDefenseMomentFloor {
				moment = CEMAttackDefenseMomentFloor
				floorUsed = true
			}
			if denom <= 0 && scored[teamID] <= CEMAttackDefenseMomentFloor {
				continue
			}
			if denom <= 0 || math.IsNaN(denom) || math.IsInf(denom, 0) {
				updated.UpdateAllowed = false
				updated.FitWallTime = time.Since(started)
				return updated
			}
			value := math.Log(moment / denom)
			maxDelta = math.Max(maxDelta, math.Abs(value-fit.AttackTheta[teamID]))
			fit.AttackTheta[teamID] = value
		}
		for _, teamID := range teamIDs {
			denom := cemConcessionDenominator(teamID, fit.AttackTheta, original, games)
			moment := conceded[teamID]
			if moment < CEMAttackDefenseMomentFloor {
				moment = CEMAttackDefenseMomentFloor
				floorUsed = true
			}
			if denom <= 0 && conceded[teamID] <= CEMAttackDefenseMomentFloor {
				continue
			}
			if denom <= 0 || math.IsNaN(denom) || math.IsInf(denom, 0) {
				updated.UpdateAllowed = false
				updated.FitWallTime = time.Since(started)
				return updated
			}
			value := math.Log(moment / denom)
			maxDelta = math.Max(maxDelta, math.Abs(value-fit.ConcessionTheta[teamID]))
			fit.ConcessionTheta[teamID] = value
		}
		fit = normalizeCEMGauge(fit)
		objective := cemAttackConcessionObjective(fit, original, games, teamIDs, scored, conceded)
		if objective+1e-9 < fitObjective {
			log.Printf("rare-position-cem-ad-fit-warning: objective_decreased previous=%.12g current=%.12g", fitObjective, objective)
		}
		fitObjective = objective
		updated.FitIterations = iteration
		if maxDelta <= CEMAttackDefenseFitTolerance {
			updated.FitConverged = true
			break
		}
	}
	updated.FitObjectiveEnd = fitObjective
	updated.FitUsedMomentFloor = floorUsed
	updated.FitWallTime = time.Since(started)
	if floorUsed {
		log.Printf("rare-position-cem-ad-fit: moment_floor_used=true floor=%.3g", CEMAttackDefenseMomentFloor)
	}
	if updated.FitConverged {
		log.Printf("rare-position-cem-ad-fit: converged=true iterations=%d objective_start=%.12g objective_end=%.12g wall_time=%s",
			updated.FitIterations, updated.FitObjectiveStart, updated.FitObjectiveEnd, updated.FitWallTime)
	} else {
		log.Printf("rare-position-cem-ad-fit: converged=false iteration_limit=%d objective_start=%.12g objective_end=%.12g wall_time=%s",
			updated.FitIterations, updated.FitObjectiveStart, updated.FitObjectiveEnd, updated.FitWallTime)
	}
	old := cloneCEMProposal(current)
	updated.AttackTheta = make(map[int]float64, len(teamIDs))
	updated.ConcessionTheta = make(map[int]float64, len(teamIDs))
	for _, teamID := range teamIDs {
		updated.AttackTheta[teamID] = cemSmoothLogTheta(current.AttackTheta[teamID], fit.AttackTheta[teamID])
		updated.ConcessionTheta[teamID] = cemSmoothLogTheta(current.ConcessionTheta[teamID], fit.ConcessionTheta[teamID])
	}
	updated = normalizeCEMGauge(updated)
	updated.Means = materializeCEMProposal(updated, original, games)
	updated.KL = cemTotalKL(updated.Means, original, games)
	updated = cemTrustRegion(updated, original, games, CEMMaxKL)
	updated.UpdateAllowed = true
	updated.ChangedTeams, updated.MaxAbsTheta, updated.ThetaDeltaL2 = 0, 0, 0
	for _, teamID := range teamIDs {
		for _, value := range []float64{updated.AttackTheta[teamID], updated.ConcessionTheta[teamID]} {
			if math.Abs(value) >= CEMMeaningfulTeamLogShift {
				updated.ChangedTeams++
			}
			updated.MaxAbsTheta = math.Max(updated.MaxAbsTheta, math.Abs(value))
		}
		updated.ThetaDeltaL2 += math.Pow(updated.AttackTheta[teamID]-old.AttackTheta[teamID], 2)
		updated.ThetaDeltaL2 += math.Pow(updated.ConcessionTheta[teamID]-old.ConcessionTheta[teamID], 2)
	}
	updated.ThetaDeltaL2 = math.Sqrt(updated.ThetaDeltaL2)
	return updated
}

func cemAttackDenominator(teamID int, concede map[int]float64,
	original []GameProposalMeans, games []*GameType) float64 {
	total := 0.0
	for i, game := range games {
		if game.Played {
			continue
		}
		if game.HomeId == teamID {
			total += original[i].Home * math.Exp(concede[game.AwayId])
		} else if game.AwayId == teamID {
			total += original[i].Away * math.Exp(concede[game.HomeId])
		}
	}
	return total
}

func cemConcessionDenominator(teamID int, attack map[int]float64,
	original []GameProposalMeans, games []*GameType) float64 {
	total := 0.0
	for i, game := range games {
		if game.Played {
			continue
		}
		if game.HomeId == teamID {
			total += original[i].Away * math.Exp(attack[game.AwayId])
		} else if game.AwayId == teamID {
			total += original[i].Home * math.Exp(attack[game.HomeId])
		}
	}
	return total
}

func cemAttackConcessionObjective(proposal CEMProposal, original []GameProposalMeans,
	games []*GameType, teamIDs []int, scored, conceded map[int]float64) float64 {
	objective := 0.0
	for _, teamID := range teamIDs {
		objective += scored[teamID]*proposal.AttackTheta[teamID] +
			conceded[teamID]*proposal.ConcessionTheta[teamID]
	}
	for i, game := range games {
		if game.Played {
			continue
		}
		objective -= original[i].Home * math.Exp(proposal.AttackTheta[game.HomeId]+proposal.ConcessionTheta[game.AwayId])
		objective -= original[i].Away * math.Exp(proposal.AttackTheta[game.AwayId]+proposal.ConcessionTheta[game.HomeId])
	}
	return objective
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
		season := CEMSeason{TeamGoals: make([]int, len(teamIDs)), TeamGoalsConceded: make([]int, len(teamIDs)),
			TeamScoredMoment: make([]float64, len(teamIDs)), TeamConcededMoment: make([]float64, len(teamIDs))}
		for i, game := range games {
			if game.Played {
				continue
			}
			home := poissonRand(rng, proposal[i].Home)
			away := poissonRand(rng, proposal[i].Away)
			if index, ok := teamIndex[game.HomeId]; ok {
				season.TeamGoals[index] += home
				season.TeamGoalsConceded[index] += away
				season.TeamScoredMoment[index] += float64(home)
				season.TeamConcededMoment[index] += float64(away)
			}
			if index, ok := teamIndex[game.AwayId]; ok {
				season.TeamGoals[index] += away
				season.TeamGoalsConceded[index] += home
				season.TeamScoredMoment[index] += float64(away)
				season.TeamConcededMoment[index] += float64(home)
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

func logCEMAttackConcessionChanges(groupID, targetTeam, position, iteration int,
	teamIDs []int, original []GameProposalMeans, games []*GameType, proposal CEMProposal) {
	type teamMovement struct {
		id       int
		strength float64
	}
	movements := make([]teamMovement, 0, len(teamIDs))
	for _, teamID := range teamIDs {
		oldAttack := proposal.PreviousAttackTheta[teamID]
		oldConcede := proposal.PreviousConcessionTheta[teamID]
		attack, concede := proposal.AttackTheta[teamID], proposal.ConcessionTheta[teamID]
		if math.Abs(attack-oldAttack) >= CEMMeaningfulTeamLogShift || math.Abs(concede-oldConcede) >= CEMMeaningfulTeamLogShift {
			scored, conceded := proposal.EliteTeamGoals[teamID], proposal.EliteTeamGoalsConceded[teamID]
			log.Printf("rare-position-cem-team-ad: group=%d target_team=%d target_position=%d iteration=%d team_id=%d old_attack=%.6g new_attack=%.6g attack_multiplier=%.6g old_concede=%.6g new_concede=%.6g concede_multiplier=%.6g weighted_elite_goals_scored=%.6g weighted_elite_goals_conceded=%.6g expected_goals_scored_under_new_q=%.6g expected_goals_conceded_under_new_q=%.6g",
				groupID, targetTeam, position, iteration, teamID, oldAttack, attack, math.Exp(attack),
				oldConcede, concede, math.Exp(concede), scored, conceded,
				cemAttackDenominator(teamID, proposal.ConcessionTheta, original, games),
				cemConcessionDenominator(teamID, proposal.AttackTheta, original, games))
		}
		movements = append(movements, teamMovement{id: teamID, strength: attack - concede})
	}
	sort.Slice(movements, func(i, j int) bool {
		if math.Abs(movements[i].strength) != math.Abs(movements[j].strength) {
			return math.Abs(movements[i].strength) > math.Abs(movements[j].strength)
		}
		return movements[i].id < movements[j].id
	})
	for _, stronger := range []bool{true, false} {
		label := "weakened"
		if stronger {
			label = "strengthened"
		}
		rank := 0
		for _, movement := range movements {
			if stronger && movement.strength <= 0 || !stronger && movement.strength >= 0 {
				continue
			}
			rank++
			if rank > 5 {
				break
			}
			log.Printf("rare-position-cem-strength-movement: group=%d target_team=%d target_position=%d iteration=%d category=%s rank=%d team_id=%d effective_strength=%.6g",
				groupID, targetTeam, position, iteration, label, rank, movement.id, movement.strength)
		}
	}
	type meanChange struct {
		gameID, homeID, awayID             int
		oldHome, newHome, oldAway, newAway float64
		change                             float64
	}
	changes := make([]meanChange, 0, len(games))
	for i, game := range games {
		if game.Played || original[i].Home <= 0 || original[i].Away <= 0 {
			continue
		}
		oldHome, oldAway := original[i].Home, original[i].Away
		newHome, newAway := proposal.Means[i].Home, proposal.Means[i].Away
		changes = append(changes, meanChange{gameID: game.Id, homeID: game.HomeId, awayID: game.AwayId,
			oldHome: oldHome, newHome: newHome, oldAway: oldAway, newAway: newAway})
	}
	for _, metric := range []struct {
		label string
		value func(meanChange) float64
	}{
		{"home_positive", func(c meanChange) float64 { return c.newHome - c.oldHome }},
		{"home_negative", func(c meanChange) float64 { return c.oldHome - c.newHome }},
		{"away_positive", func(c meanChange) float64 { return c.newAway - c.oldAway }},
		{"away_negative", func(c meanChange) float64 { return c.oldAway - c.newAway }},
	} {
		ordered := append([]meanChange(nil), changes...)
		sort.Slice(ordered, func(i, j int) bool { return metric.value(ordered[i]) > metric.value(ordered[j]) })
		for i := 0; i < len(ordered) && i < 5 && metric.value(ordered[i]) > 0; i++ {
			change := ordered[i]
			log.Printf("rare-position-cem-game-mean-change: group=%d target_team=%d target_position=%d iteration=%d kind=%s rank=%d game_id=%d home_team=%d away_team=%d home_old=%.6g home_new=%.6g away_old=%.6g away_new=%.6g delta=%.6g",
				groupID, targetTeam, position, iteration, metric.label, i+1, change.gameID,
				change.homeID, change.awayID, change.oldHome, change.newHome, change.oldAway, change.newAway,
				metric.value(change))
		}
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
	teamIDs := make(map[int]bool, len(a)+len(b))
	for teamID := range a {
		teamIDs[teamID] = true
	}
	for teamID := range b {
		teamIDs[teamID] = true
	}
	ordered := make([]int, 0, len(teamIDs))
	for teamID := range teamIDs {
		ordered = append(ordered, teamID)
	}
	sort.Ints(ordered)
	for _, teamID := range ordered {
		delta := a[teamID] - b[teamID]
		sum += delta * delta
	}
	return math.Sqrt(sum)
}

func retainCEMProposalSnapshot(groupID int, state *CEMCandidateState, proposal CEMProposal,
	stats CEMBatchStats, iteration int, reason string) bool {
	proposal = normalizeCEMGauge(proposal)
	snapshot := CEMProposalSnapshot{
		CandidateTeam: state.Candidate.TeamID, CandidatePosition: state.Candidate.Position,
		SourceIteration: iteration, Proposal: cloneCEMProposal(proposal), Stats: stats,
		InitialEliteDistance: state.FirstStats.EliteMeanDistance,
		ExactHits:            stats.ExactHits, HadAdaptationExactHit: stats.ExactHits > 0,
		AdaptationExactHits: stats.ExactHits, AdaptationNearTargetHits: stats.NearTargetHits, Reason: reason,
		SearchScore: stats.NearTargetRate*100 - stats.EliteMeanDistance + float64(stats.ExactHits)*10,
	}
	nearestDistance := 0.0
	if len(state.Snapshots) > 0 {
		nearestDistance = math.Inf(1)
		for _, previous := range state.Snapshots {
			nearestDistance = math.Min(nearestDistance,
				cemProposalParameterDistance(snapshot.Proposal, previous.Proposal))
		}
	}
	for i := range state.Snapshots {
		if cemProposalParameterDistance(snapshot.Proposal,
			state.Snapshots[i].Proposal) < CEMThetaStabilityThreshold {
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

func snapshotHadAdaptationExactHit(snapshot CEMProposalSnapshot) bool {
	return snapshot.Stats.ExactHits > 0
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
	log.Printf("rare-position-cem-snapshot: group=%d team=%d position=%d source_iteration=%d reason=%s adaptation_exact=%t adaptation_exact_hits=%d adaptation_near_target_hits=%d exact_hits=%d near_target_rate=%.5f elite_mean_distance=%.4f elite_ess=%.3f kl=%.4f theta_l2_from_previous_snapshot=%.5f snapshot_count_for_candidate=%d",
		groupID, snapshot.CandidateTeam, snapshot.CandidatePosition, snapshot.SourceIteration,
		snapshot.Reason, snapshot.HadAdaptationExactHit, snapshot.AdaptationExactHits,
		snapshot.AdaptationNearTargetHits, snapshot.ExactHits, snapshot.Stats.NearTargetRate,
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
	copy.AttackTheta = copyTheta(proposal.AttackTheta)
	copy.ConcessionTheta = copyTheta(proposal.ConcessionTheta)
	copy.PreviousAttackTheta = copyTheta(proposal.PreviousAttackTheta)
	copy.PreviousConcessionTheta = copyTheta(proposal.PreviousConcessionTheta)
	copy.PreviousLogTheta = copyTheta(proposal.PreviousLogTheta)
	copy.EliteTeamGoals = make(map[int]float64, len(proposal.EliteTeamGoals))
	for teamID, goals := range proposal.EliteTeamGoals {
		copy.EliteTeamGoals[teamID] = goals
	}
	copy.EliteTeamGoalsConceded = make(map[int]float64, len(proposal.EliteTeamGoalsConceded))
	for teamID, goals := range proposal.EliteTeamGoalsConceded {
		copy.EliteTeamGoalsConceded[teamID] = goals
	}
	copy.Means = append([]GameProposalMeans(nil), proposal.Means...)
	return copy
}

func cemProposalParameterDistance(a, b CEMProposal) float64 {
	if a.Parameterization == CEMTeamAttackConcession || b.Parameterization == CEMTeamAttackConcession {
		sum := 0.0
		teamIDs := make(map[int]bool, len(a.AttackTheta)+len(b.AttackTheta))
		for teamID := range a.AttackTheta {
			teamIDs[teamID] = true
		}
		for teamID := range b.AttackTheta {
			teamIDs[teamID] = true
		}
		ordered := make([]int, 0, len(teamIDs))
		for teamID := range teamIDs {
			ordered = append(ordered, teamID)
		}
		sort.Ints(ordered)
		for _, teamID := range ordered {
			delta := a.AttackTheta[teamID] - b.AttackTheta[teamID]
			sum += delta * delta
			delta = a.ConcessionTheta[teamID] - b.ConcessionTheta[teamID]
			sum += delta * delta
		}
		return math.Sqrt(sum)
	}
	return thetaDistanceL2(a.TeamLogMultipliers, b.TeamLogMultipliers)
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
	if state.Iterations == 1 {
		state.FirstStats = stats
		state.BestEliteDistance = stats.EliteMeanDistance
		state.BestNearTargetRate = stats.NearTargetRate
	} else {
		state.PreviousStats = state.LastStats
	}
	state.LastStats = stats
	state.HasStats = true
	if stats.NearTargetHits > 0 && state.FirstNearTargetSamples == 0 {
		state.FirstNearTargetSamples = state.Samples
	}
	if stats.ExactHits > 0 {
		if state.FirstExactSamples == 0 {
			state.FirstExactSamples = state.Samples
		}
		state.AdaptationExactHitBatches++
	}
	state.BestExactRate = math.Max(state.BestExactRate, stats.ExactRate)
	if state.Samples >= 300 && !state.HasEliteDistanceAt300 {
		state.EliteDistanceAt300, state.HasEliteDistanceAt300 = stats.EliteMeanDistance, true
	}
	if state.Samples >= 600 && !state.HasEliteDistanceAt600 {
		state.EliteDistanceAt600, state.HasEliteDistanceAt600 = stats.EliteMeanDistance, true
	}
	if state.Samples >= 900 && !state.HasEliteDistanceAt900 {
		state.EliteDistanceAt900, state.HasEliteDistanceAt900 = stats.EliteMeanDistance, true
	}
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
	if updated.Parameterization == CEMTeamAttackConcession {
		result.AttackConcessionFitUpdates++
		result.AttackConcessionFitWallTime += updated.FitWallTime
		result.AttackConcessionFitIterations += updated.FitIterations
		result.AttackConcessionFitObjectiveStart += updated.FitObjectiveStart
		result.AttackConcessionFitObjectiveEnd += updated.FitObjectiveEnd
		if updated.FitConverged {
			result.AttackConcessionFitConverged++
		}
	}
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
	if updated.Parameterization == CEMTeamAttackConcession {
		logCEMAttackConcessionChanges(group.Id, state.Candidate.TeamID, state.Candidate.Position,
			state.Iterations, teamIDs, original, group.Games, updated)
	} else {
		logCEMTeamChanges(group.Id, state.Candidate.TeamID, state.Candidate.Position,
			state.Iterations, teamIDs, original, group.Games, updated)
	}
	log.Printf("rare-position-cem: parameterization=%s group=%d team=%d position=%d iteration=%d samples=%d exact_hits=%d exact_hit_rate=%.4f near_target_rate=%.4f elite_count=%d elite_ess=%.2f exact_elites=%t mean_rank=%.3f best_rank=%d elite_mean_distance=%.3f kl=%.3f changed_teams=%d max_abs_team_theta=%.4f theta_delta_l2=%.4f progress_score=%.4f",
		cemParameterizationName(updated.Parameterization), group.Id, state.Candidate.TeamID, state.Candidate.Position, state.Iterations, samples,
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
	mode, source := cemInitializationMode()
	log.Printf("rare-position-cem-init-mode: group=%d mode=%s source=%s parameterization=%s", group.Id, cemInitializationModeName(mode), source, cemParameterizationName(cemParameterizationFromEnvironment()))
	return runCEMAdaptationRoundWithMode(mode, candidates, searches, group, campaign, table, order,
		original, cemRemaining, remainingWork, explorationRemaining, adaptWorkPerSample, rng)
}

func runCEMAdaptationRoundWithMode(mode CEMInitializationMode, candidates []*FrontierCandidate, searches map[int]*TeamRareSearch,
	group *GroupType, campaign []*TeamCampaign, table *Table, order []SortType,
	original []GameProposalMeans, cemRemaining, remainingWork, explorationRemaining *int64,
	adaptWorkPerSample int64, rng *rand.Rand) CEMRoundResult {
	return runCEMAdaptationRoundWithModeAndParameterization(mode, CEMTeamScoring, candidates,
		searches, group, campaign, table, order, original, cemRemaining, remainingWork,
		explorationRemaining, adaptWorkPerSample, rng)
}

func runCEMAdaptationRoundWithModeAndParameterization(mode CEMInitializationMode,
	parameterization CEMParameterization, candidates []*FrontierCandidate,
	searches map[int]*TeamRareSearch, group *GroupType, campaign []*TeamCampaign,
	table *Table, order []SortType, original []GameProposalMeans,
	theCEMRemaining, remainingWork, explorationRemaining *int64,
	adaptWorkPerSample int64, rng *rand.Rand) CEMRoundResult {
	result := CEMRoundResult{CandidatesTotal: len(candidates)}
	teamIDs := teamIDsFromGroups(group.Team_groups)
	states := make([]*CEMCandidateState, 0, len(candidates))
	for _, candidate := range candidates {
		proposal, _ := newCEMProposalWithModeAndParameterization(mode, parameterization,
			candidate, searches, original, group.Games, teamIDs, group.Id)
		states = append(states, &CEMCandidateState{
			Candidate: candidate,
			Proposal:  proposal, InitialProposal: cloneCEMProposal(proposal),
			Active: true,
		})
	}
	cemRemaining := theCEMRemaining
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
		if state.HasEliteDistanceAt300 {
			result.EliteDistanceAt300 += state.EliteDistanceAt300
			result.CandidatesAt300++
		}
		if state.HasEliteDistanceAt600 {
			result.EliteDistanceAt600 += state.EliteDistanceAt600
			result.CandidatesAt600++
		}
		if state.HasEliteDistanceAt900 {
			result.EliteDistanceAt900 += state.EliteDistanceAt900
			result.CandidatesAt900++
		}
		if state.FirstNearTargetSamples > 0 {
			result.MeanSamplesToFirstNear += float64(state.FirstNearTargetSamples)
			result.CandidatesWithFirstNear++
		}
		if state.FirstExactSamples > 0 {
			result.MeanSamplesToFirstExact += float64(state.FirstExactSamples)
			result.CandidatesWithFirstExact++
		}
		result.RepeatedExactHitBatches += max(0, state.AdaptationExactHitBatches-1)
		result.BestNearTargetRate = math.Max(result.BestNearTargetRate, state.BestNearTargetRate)
		result.BestExactRate = math.Max(result.BestExactRate, state.BestExactRate)
		if state.InitialProposal.Parameterization == CEMTeamAttackConcession {
			for _, value := range state.InitialProposal.AttackTheta {
				result.InitialAttackL2 += value * value
			}
			for _, value := range state.InitialProposal.ConcessionTheta {
				result.InitialConcessionL2 += value * value
			}
		}
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
	result.InitialAttackL2 = math.Sqrt(result.InitialAttackL2)
	result.InitialConcessionL2 = math.Sqrt(result.InitialConcessionL2)
	if result.CandidatesAt300 > 0 {
		result.EliteDistanceAt300 /= float64(result.CandidatesAt300)
	}
	if result.CandidatesAt600 > 0 {
		result.EliteDistanceAt600 /= float64(result.CandidatesAt600)
	}
	if result.CandidatesAt900 > 0 {
		result.EliteDistanceAt900 /= float64(result.CandidatesAt900)
	}
	if result.CandidatesWithFirstNear > 0 {
		result.MeanSamplesToFirstNear /= float64(result.CandidatesWithFirstNear)
	}
	if result.CandidatesWithFirstExact > 0 {
		result.MeanSamplesToFirstExact /= float64(result.CandidatesWithFirstExact)
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
	thetaL2 := cemProposalParameterDistance(snapshot.Proposal, CEMProposal{Parameterization: snapshot.Proposal.Parameterization})
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
	seenThetas := make([]CEMProposal, 0, limit)
	appendUnique := func(snapshot CEMProposalSnapshot) {
		for _, proposal := range seenThetas {
			if cemProposalParameterDistance(proposal, snapshot.Proposal) < CEMThetaStabilityThreshold {
				return
			}
		}
		selected = append(selected, snapshot)
		seenTargets[[2]int{snapshot.CandidateTeam, snapshot.CandidatePosition}] = true
		seenThetas = append(seenThetas, cloneCEMProposal(snapshot.Proposal))
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
				if cemProposalParameterDistance(snapshot.Proposal, chosen.Proposal) < CEMThetaStabilityThreshold {
					reason = "duplicate_snapshot"
					break
				}
			}
		}
		log.Printf("rare-position-pilot-selection: group=%d team=%d position=%d source_iteration=%d eligible=%t selected=%t reason=%s eligible_reason=%s kl=%.6g theta_l2=%.6g adaptation_exact=%t adaptation_exact_hits=%d adaptation_near_target_hits=%d exact_hits=%d near_target_hits=%d best_rank=%d snapshot_distance=%.6g initial_distance=%.6g relative_distance_improvement=%.6g priority=%.6g",
			groupID, snapshot.CandidateTeam, snapshot.CandidatePosition, snapshot.SourceIteration,
			eligibility.Eligible, selectedThis, reason, eligibility.Reason, snapshot.Proposal.KL, eligibility.ThetaL2,
			snapshotHadAdaptationExactHit(snapshot), snapshot.Stats.ExactHits, snapshot.Stats.NearTargetHits,
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
	state := &CEMEvaluationState{
		Snapshot:           snapshot,
		Stage:              1,
		AdaptationPriority: cemSnapshotPriority(snapshot),
	}
	evaluateCEMChunk(state, original, baseCampaign, games, table, order, teamGroups,
		samples, workPerSample, rng, groupID, 0, 0)
	evals := convertToEvaluations([]*CEMEvaluationState{state})
	return evals[0]
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

func cemSnapshotPriority(snap CEMProposalSnapshot) float64 {
	initialDistance := math.Max(1, snap.InitialEliteDistance)
	distanceGain := (snap.InitialEliteDistance - snap.Stats.EliteMeanDistance) / initialDistance
	return float64(snap.ExactHits)*1000 + snap.Stats.NearTargetRate*100 + distanceGain*10 + math.Min(snap.Stats.EliteESS, 100)/100 - snap.Proposal.KL*0.01
}

func scoutPNeighborhood(normalPositionCounts map[int][]int, teamID, targetPosition, normalSamples int) (float64, float64, float64) {
	if normalSamples <= 0 {
		return 0, 0, 0
	}
	counts, ok := normalPositionCounts[teamID]
	if !ok {
		return 0, 0, 0
	}
	c0, c1, c2 := 0, 0, 0
	for pos, count := range counts {
		dist := absInt(pos - targetPosition)
		if dist == 0 {
			c0 += count
		}
		if dist <= 1 {
			c1 += count
		}
		if dist <= 2 {
			c2 += count
		}
	}
	N := float64(normalSamples)
	return float64(c0) / N, float64(c1) / N, float64(c2) / N
}

func evaluateCEMChunk(
	state *CEMEvaluationState,
	original []GameProposalMeans,
	baseCampaign []*TeamCampaign,
	games []*GameType,
	table *Table,
	order []SortType,
	teamGroups []TeamType,
	chunkSamples int,
	workPerSample int64,
	rng *rand.Rand,
	groupID int,
	scoutP1, scoutP2 float64,
) {
	components := cemEvaluationMixture(original, state.Snapshot)
	validateProposalMixture(components, len(games))

	teamID := state.Snapshot.CandidateTeam
	targetPos := state.Snapshot.CandidatePosition
	numComps := len(components)

	logQBuf := make([]float64, numComps)
	compWeightsBuf := make([]float64, numComps)
	for i := 0; i < numComps; i++ {
		compWeightsBuf[i] = components[i].Weight
	}

	simCampaign := make([]*TeamCampaign, len(baseCampaign))
	teamSlice := make([]*TeamCampaign, len(teamGroups))

	chunkWork := int64(chunkSamples) * workPerSample

	for s := 0; s < chunkSamples; s++ {
		for k, v := range baseCampaign {
			if v != nil {
				simCampaign[k] = v.clone()
			} else {
				simCampaign[k] = nil
			}
		}

		for i := 0; i < numComps; i++ {
			logQBuf[i] = 0.0
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
				origH := original[i].Home
				origA := original[i].Away

				hScore := poissonRand(rng, activeMeans[i].Home)
				aScore := poissonRand(rng, activeMeans[i].Away)

				for k := 0; k < numComps; k++ {
					cMeans := components[k].Means[i]
					logQBuf[k] += logPoissonQOverP(hScore, origH, cMeans.Home)
					logQBuf[k] += logPoissonQOverP(aScore, origA, cMeans.Away)
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

		w := mixtureImportanceWeightMulti(logQBuf, compWeightsBuf)

		idx := 0
		for _, tg := range teamGroups {
			c := simCampaign[table.Query(uint32(tg.Team_id))]
			if c != nil {
				teamSlice[idx] = c
				idx++
			}
		}

		sortedTeams := TeamCampaignSorted{t: teamSlice[:idx], sort: order, rng: rng}
		sort.Sort(sortedTeams)

		rank := -1
		for pos, t := range sortedTeams.t {
			if t.id == teamID {
				rank = pos
				break
			}
		}

		dist := absInt(rank - targetPos)

		if dist == 0 {
			state.Exact.Hits++
			state.Exact.SumY += w
			state.Exact.SumY2 += w * w
			if w > state.Exact.MaxWeightShare {
				state.Exact.MaxWeightShare = w
			}
		}
		if dist <= 1 {
			state.Near1.Hits++
			state.Near1.SumY += w
			state.Near1.SumY2 += w * w
			if w > state.Near1.MaxWeightShare {
				state.Near1.MaxWeightShare = w
			}
		}
		if dist <= 2 {
			state.Near2.Hits++
			state.Near2.SumY += w
			state.Near2.SumY2 += w * w
			if w > state.Near2.MaxWeightShare {
				state.Near2.MaxWeightShare = w
			}
		}
	}

	state.Samples += chunkSamples
	state.Work += chunkWork
	state.Chunks++

	updateEventStats := func(stats *WeightedEventStats, totalSamples int) {
		if totalSamples <= 0 || stats.Hits == 0 {
			stats.Probability = 0
			stats.StdErr = 0
			stats.RelSE = math.Inf(1)
			stats.ESS = 0
			return
		}
		N := float64(totalSamples)
		pHat := stats.SumY / N
		sampleVar := (stats.SumY2 - N*pHat*pHat) / (N - 1.0)
		if sampleVar < 0 {
			sampleVar = 0
		}
		stdErr := math.Sqrt(sampleVar / N)
		relSE := math.Inf(1)
		if pHat > 0 {
			relSE = stdErr / pHat
		}
		ess := 0.0
		if stats.SumY2 > 0 {
			ess = (stats.SumY * stats.SumY) / stats.SumY2
		}
		stats.Probability = pHat
		stats.StdErr = stdErr
		stats.RelSE = relSE
		stats.ESS = ess
	}

	updateEventStats(&state.Exact, state.Samples)
	updateEventStats(&state.Near1, state.Samples)
	updateEventStats(&state.Near2, state.Samples)

	plainEquivWork := float64(state.Samples)
	p1Floor := math.Max(scoutP1, 1e-6)
	p2Floor := math.Max(scoutP2, 1e-6)

	if plainEquivWork > 0 && p1Floor > 0 {
		state.Near1EfficiencyRatio = state.Near1.ESS / (plainEquivWork * p1Floor)
	}
	if plainEquivWork > 0 && p2Floor > 0 {
		state.Near2EfficiencyRatio = state.Near2.ESS / (plainEquivWork * p2Floor)
	}

	exactESSPerWork := 0.0
	if state.Work > 0 {
		exactESSPerWork = state.Exact.ESS / float64(state.Work)
	}

	log.Printf("rare-position-evaluation-chunk: group=%d team=%d position=%d snapshot_iteration=%d stage=%d chunk=%d chunk_samples=%d cumulative_samples=%d cumulative_work=%d exact_hits=%d exact_ess=%.3f exact_ess_per_work=%.8g exact_max_weight_share=%.3f near1_hits=%d near1_ess=%.3f near1_efficiency_ratio=%.3f near2_hits=%d near2_ess=%.3f near2_efficiency_ratio=%.3f",
		groupID, teamID, targetPos, state.Snapshot.SourceIteration, state.Stage, state.Chunks,
		chunkSamples, state.Samples, state.Work,
		state.Exact.Hits, state.Exact.ESS, exactESSPerWork, state.Exact.MaxWeightShare,
		state.Near1.Hits, state.Near1.ESS, state.Near1EfficiencyRatio,
		state.Near2.Hits, state.Near2.ESS, state.Near2EfficiencyRatio)
}

func cemStateBetterForRace(a, b *CEMEvaluationState) bool {
	aHasExact := a.Exact.Hits > 0
	bHasExact := b.Exact.Hits > 0

	if aHasExact != bHasExact {
		return aHasExact
	}

	if aHasExact && bHasExact {
		aESSPerWork := cemESSPerWork(a.Exact, a.Work)
		bESSPerWork := cemESSPerWork(b.Exact, b.Work)
		if aESSPerWork != bESSPerWork {
			return aESSPerWork > bESSPerWork
		}
		if a.Exact.ESS != b.Exact.ESS {
			return a.Exact.ESS > b.Exact.ESS
		}
		if a.Exact.Hits != b.Exact.Hits {
			return a.Exact.Hits > b.Exact.Hits
		}
		aShare := 0.0
		if a.Exact.SumY > 0 {
			aShare = a.Exact.MaxWeightShare / a.Exact.SumY
		}
		bShare := 0.0
		if b.Exact.SumY > 0 {
			bShare = b.Exact.MaxWeightShare / b.Exact.SumY
		}
		if aShare != bShare {
			return aShare < bShare
		}
		if a.Near1EfficiencyRatio != b.Near1EfficiencyRatio {
			return a.Near1EfficiencyRatio > b.Near1EfficiencyRatio
		}
		if a.HadAdaptationExactHit != b.HadAdaptationExactHit {
			return a.HadAdaptationExactHit
		}
		if a.AdaptationPriority != b.AdaptationPriority {
			return a.AdaptationPriority > b.AdaptationPriority
		}
		if a.Snapshot.Proposal.KL != b.Snapshot.Proposal.KL {
			return a.Snapshot.Proposal.KL < b.Snapshot.Proposal.KL
		}
		return a.Snapshot.SourceIteration < b.Snapshot.SourceIteration
	}
	if a.HadAdaptationExactHit != b.HadAdaptationExactHit {
		return a.HadAdaptationExactHit
	}
	aProtected, bProtected := cemStateNeedsProtectedSamples(a), cemStateNeedsProtectedSamples(b)
	if aProtected != bProtected {
		return aProtected
	}

	if a.Near1EfficiencyRatio != b.Near1EfficiencyRatio {
		return a.Near1EfficiencyRatio > b.Near1EfficiencyRatio
	}
	aNear1ESSPerWork := cemESSPerWork(a.Near1, a.Work)
	bNear1ESSPerWork := cemESSPerWork(b.Near1, b.Work)
	if aNear1ESSPerWork != bNear1ESSPerWork {
		return aNear1ESSPerWork > bNear1ESSPerWork
	}
	if a.Near2EfficiencyRatio != b.Near2EfficiencyRatio {
		return a.Near2EfficiencyRatio > b.Near2EfficiencyRatio
	}
	aNear2ESSPerWork := cemESSPerWork(a.Near2, a.Work)
	bNear2ESSPerWork := cemESSPerWork(b.Near2, b.Work)
	if aNear2ESSPerWork != bNear2ESSPerWork {
		return aNear2ESSPerWork > bNear2ESSPerWork
	}
	if a.AdaptationExactHits != b.AdaptationExactHits {
		return a.AdaptationExactHits > b.AdaptationExactHits
	}
	if a.AdaptationNearTargetHits != b.AdaptationNearTargetHits {
		return a.AdaptationNearTargetHits > b.AdaptationNearTargetHits
	}
	if a.AdaptationPriority != b.AdaptationPriority {
		return a.AdaptationPriority > b.AdaptationPriority
	}
	if a.Snapshot.Proposal.KL != b.Snapshot.Proposal.KL {
		return a.Snapshot.Proposal.KL < b.Snapshot.Proposal.KL
	}
	return cemStateIdentityLess(a, b)
}

func cemESSPerWork(stats WeightedEventStats, work int64) float64 {
	if work <= 0 {
		return 0
	}
	return stats.ESS / float64(work)
}

func cemStateIdentityLess(a, b *CEMEvaluationState) bool {
	if a.Snapshot.CandidateTeam != b.Snapshot.CandidateTeam {
		return a.Snapshot.CandidateTeam < b.Snapshot.CandidateTeam
	}
	if a.Snapshot.CandidatePosition != b.Snapshot.CandidatePosition {
		return a.Snapshot.CandidatePosition < b.Snapshot.CandidatePosition
	}
	return a.Snapshot.SourceIteration < b.Snapshot.SourceIteration
}

func cemStateNeedsProtectedSamples(state *CEMEvaluationState) bool {
	return state != nil && state.HadAdaptationExactHit && state.Exact.Hits == 0 &&
		state.Samples < CEMAdaptationExactMinEvaluationSamples
}

func cemAnyHeldOutExact(states []*CEMEvaluationState) bool {
	for _, state := range states {
		if state.Exact.Hits > 0 {
			return true
		}
	}
	return false
}

func cemProtectedCandidates(states []*CEMEvaluationState) []*CEMEvaluationState {
	protected := make([]*CEMEvaluationState, 0, len(states))
	for _, state := range states {
		if !state.Eliminated && cemStateNeedsProtectedSamples(state) {
			protected = append(protected, state)
		}
	}
	sort.Slice(protected, func(i, j int) bool { return cemStateIdentityLess(protected[i], protected[j]) })
	return protected
}

func nextCEMProtectedCandidate(states []*CEMEvaluationState, cursor int) (*CEMEvaluationState, int) {
	protected := cemProtectedCandidates(states)
	if len(protected) == 0 {
		return nil, 0
	}
	minSamples := protected[0].Samples
	for _, state := range protected[1:] {
		if state.Samples < minSamples {
			minSamples = state.Samples
		}
	}
	fair := make([]*CEMEvaluationState, 0, len(protected))
	for _, state := range protected {
		if state.Samples == minSamples {
			fair = append(fair, state)
		}
	}
	index := cursor % len(fair)
	return fair[index], (index + 1) % len(fair)
}

func cemStateBetterForRaceLegacy(a, b *CEMEvaluationState) bool {
	if (a.Exact.Hits > 0) != (b.Exact.Hits > 0) {
		return a.Exact.Hits > 0
	}
	if a.Exact.Hits > 0 {
		aRate, bRate := cemESSPerWork(a.Exact, a.Work), cemESSPerWork(b.Exact, b.Work)
		if aRate != bRate {
			return aRate > bRate
		}
		if a.Exact.ESS != b.Exact.ESS {
			return a.Exact.ESS > b.Exact.ESS
		}
		if a.Exact.Hits != b.Exact.Hits {
			return a.Exact.Hits > b.Exact.Hits
		}
		aShare, bShare := 0.0, 0.0
		if a.Exact.SumY > 0 {
			aShare = a.Exact.MaxWeightShare / a.Exact.SumY
		}
		if b.Exact.SumY > 0 {
			bShare = b.Exact.MaxWeightShare / b.Exact.SumY
		}
		if aShare != bShare {
			return aShare < bShare
		}
		if a.Near1EfficiencyRatio != b.Near1EfficiencyRatio {
			return a.Near1EfficiencyRatio > b.Near1EfficiencyRatio
		}
		if a.AdaptationPriority != b.AdaptationPriority {
			return a.AdaptationPriority > b.AdaptationPriority
		}
	} else {
		if a.Near1EfficiencyRatio != b.Near1EfficiencyRatio {
			return a.Near1EfficiencyRatio > b.Near1EfficiencyRatio
		}
		aNear1PerWork, bNear1PerWork := cemESSPerWork(a.Near1, a.Work), cemESSPerWork(b.Near1, b.Work)
		if aNear1PerWork != bNear1PerWork {
			return aNear1PerWork > bNear1PerWork
		}
		if a.Near2EfficiencyRatio != b.Near2EfficiencyRatio {
			return a.Near2EfficiencyRatio > b.Near2EfficiencyRatio
		}
		aNear2PerWork, bNear2PerWork := cemESSPerWork(a.Near2, a.Work), cemESSPerWork(b.Near2, b.Work)
		if aNear2PerWork != bNear2PerWork {
			return aNear2PerWork > bNear2PerWork
		}
		if a.AdaptationPriority != b.AdaptationPriority {
			return a.AdaptationPriority > b.AdaptationPriority
		}
		if a.Snapshot.InitialEliteDistance != b.Snapshot.InitialEliteDistance {
			return a.Snapshot.InitialEliteDistance < b.Snapshot.InitialEliteDistance
		}
	}
	if a.Snapshot.Proposal.KL != b.Snapshot.Proposal.KL {
		return a.Snapshot.Proposal.KL < b.Snapshot.Proposal.KL
	}
	return a.Snapshot.SourceIteration < b.Snapshot.SourceIteration
}

func cemStateEarlyAccept(st *CEMEvaluationState) bool {
	if st == nil {
		return false
	}
	maxShare := 0.0
	if st.Exact.SumY > 0 {
		maxShare = st.Exact.MaxWeightShare / st.Exact.SumY
	}
	return st.Exact.Hits >= 2 && st.Exact.ESS >= CEMEvaluationEarlyAcceptESS && maxShare <= 0.90
}

func convertToEvaluations(states []*CEMEvaluationState) []CEMProposalEvaluation {
	evals := make([]CEMProposalEvaluation, len(states))
	for i, st := range states {
		essPerWork := 0.0
		if st.Work > 0 {
			essPerWork = st.Exact.ESS / float64(st.Work)
		}
		secondMoment := 0.0
		if st.Samples > 0 {
			secondMoment = st.Exact.SumY2 / float64(st.Samples)
		}
		maxShare := 0.0
		if st.Exact.SumY > 0 {
			maxShare = st.Exact.MaxWeightShare / st.Exact.SumY
		}
		evals[i] = CEMProposalEvaluation{
			Snapshot:     st.Snapshot,
			Samples:      st.Samples,
			SamplesAt200: st.SamplesAt200, ExactHitsAt200: st.ExactHitsAt200,
			SamplesAt400: st.SamplesAt400, ExactHitsAt400: st.ExactHitsAt400,
			SamplesAt600: st.SamplesAt600, ExactHitsAt600: st.ExactHitsAt600,
			Hits:                 st.Exact.Hits,
			SumY:                 st.Exact.SumY,
			SumY2:                st.Exact.SumY2,
			Probability:          st.Exact.Probability,
			StdErr:               st.Exact.StdErr,
			RelSE:                st.Exact.RelSE,
			ESS:                  st.Exact.ESS,
			ESSPerWork:           essPerWork,
			SecondMoment:         secondMoment,
			MaxEventWeightShare:  maxShare,
			Work:                 st.Work,
			Exact:                st.Exact,
			Near1:                st.Near1,
			Near2:                st.Near2,
			Near1EfficiencyRatio: st.Near1EfficiencyRatio,
			Near2EfficiencyRatio: st.Near2EfficiencyRatio,
		}
	}
	return evals
}

func convertToSingleEvaluation(groupID int, st *CEMEvaluationState, earlyAccept bool, reason string) *CEMProposalEvaluation {
	if st == nil {
		return nil
	}
	evals := convertToEvaluations([]*CEMEvaluationState{st})
	res := evals[0]
	res.EarlyAccept = earlyAccept
	log.Printf("rare-position-evaluation-selected: group=%d selected=importance_sampling team=%d position=%d snapshot_iteration=%d selected_evaluation_samples=%d selected_evaluation_work=%d exact_hits=%d exact_ess=%.3f exact_ess_per_work=%.8g near1_ess=%.3f near1_efficiency_ratio=%.3f near2_ess=%.3f near2_efficiency_ratio=%.3f early_accept=%t reason=%s",
		groupID, st.Snapshot.CandidateTeam, st.Snapshot.CandidatePosition, st.Snapshot.SourceIteration,
		st.Samples, st.Work, st.Exact.Hits, st.Exact.ESS, res.ESSPerWork,
		st.Near1.ESS, st.Near1EfficiencyRatio, st.Near2.ESS, st.Near2EfficiencyRatio, earlyAccept, reason)
	return &res
}

func finalizeCEMEvaluationResult(groupID int, states []*CEMEvaluationState,
	selected *CEMProposalEvaluation, earlyAccept bool, reason string, workPerSample int64) CEMEvaluationResult {
	result := CEMEvaluationResult{Evaluations: convertToEvaluations(states), Selected: selected,
		EarlyAccept: earlyAccept, Reason: reason}
	for _, state := range states {
		result.TotalSamples += state.Samples
		result.TotalWork += state.Work
	}
	selectedTeam, selectedPosition, selectedIteration := 0, -1, 0
	exactHits, near1ESS, near2ESS, exactESS, exactESSPerWork := 0, 0.0, 0.0, 0.0, 0.0
	if selected != nil {
		result.SelectedSamples, result.SelectedWork = selected.Samples, selected.Work
		selectedTeam, selectedPosition, selectedIteration = selected.Snapshot.CandidateTeam,
			selected.Snapshot.CandidatePosition, selected.Snapshot.SourceIteration
		exactHits, near1ESS, near2ESS = selected.Exact.Hits, selected.Near1.ESS, selected.Near2.ESS
		exactESS, exactESSPerWork = selected.Exact.ESS, selected.ESSPerWork
	}
	if result.TotalWork != int64(result.TotalSamples)*workPerSample {
		panic(fmt.Sprintf("CEM evaluation work mismatch: samples=%d work=%d work_per_sample=%d",
			result.TotalSamples, result.TotalWork, workPerSample))
	}
	selection := "plain_mc"
	if selected != nil {
		selection = "importance_sampling"
	}
	log.Printf("rare-position-evaluation-result: group=%d selected=%s total_evaluation_samples=%d total_evaluation_work=%d selected_team=%d selected_position=%d selected_snapshot_iteration=%d selected_evaluation_samples=%d selected_evaluation_work=%d exact_hits=%d exact_ess=%.3f exact_ess_per_work=%.8g near1_ess=%.3f near2_ess=%.3f early_accept=%t reason=%s",
		groupID, selection, result.TotalSamples, result.TotalWork, selectedTeam, selectedPosition,
		selectedIteration, result.SelectedSamples, result.SelectedWork, exactHits, exactESS,
		exactESSPerWork, near1ESS, near2ESS, earlyAccept, reason)
	return result
}

func runCEMProposalRacing(groupID int, snapshots []CEMProposalSnapshot,
	original []GameProposalMeans, baseCampaign []*TeamCampaign, games []*GameType,
	table *Table, order []SortType, teamGroups []TeamType,
	normalPositionCounts map[int][]int, normalSamples int, evaluationWorkRemaining *int64,
	remainingWork *int64, workPerSample int64, evaluationSeed int64) ([]CEMProposalEvaluation, *CEMProposalEvaluation) {
	result := runCEMProposalRacingWithResult(groupID, snapshots, original, baseCampaign, games,
		table, order, teamGroups, normalPositionCounts, normalSamples, evaluationWorkRemaining,
		remainingWork, workPerSample, evaluationSeed, false)
	return result.Evaluations, result.Selected
}

func runCEMProposalRacingWithResult(
	groupID int,
	snapshots []CEMProposalSnapshot,
	original []GameProposalMeans,
	baseCampaign []*TeamCampaign,
	games []*GameType,
	table *Table,
	order []SortType,
	teamGroups []TeamType,
	normalPositionCounts map[int][]int,
	normalSamples int,
	evaluationWorkRemaining *int64,
	remainingWork *int64,
	workPerSample int64,
	evaluationSeed int64,
	legacyRacing bool,
) CEMEvaluationResult {
	if len(snapshots) == 0 {
		return finalizeCEMEvaluationResult(groupID, nil, nil, false, "no_shortlisted_snapshots", workPerSample)
	}

	states := make([]*CEMEvaluationState, len(snapshots))
	for i, snap := range snapshots {
		states[i] = &CEMEvaluationState{
			Snapshot: snap, HadAdaptationExactHit: snapshotHadAdaptationExactHit(snap),
			AdaptationExactHits: snap.Stats.ExactHits, AdaptationNearTargetHits: snap.Stats.NearTargetHits,
			Stage: 1, AdaptationPriority: cemSnapshotPriority(snap),
		}
	}
	rankBeforeProtected := cemStateBetterForRace
	if legacyRacing {
		rankBeforeProtected = cemStateBetterForRaceLegacy
	}

	chunkWork := int64(CEMEvaluationChunkSamples) * workPerSample

	logRaceRanking := func(stage int, reason string) {
		ranked := append([]*CEMEvaluationState(nil), states...)
		sort.Slice(ranked, func(i, j int) bool {
			return rankBeforeProtected(ranked[i], ranked[j])
		})
		for r, st := range ranked {
			tier := "near_target"
			if st.Exact.Hits > 0 {
				tier = "exact"
			}
			status := "survivor"
			if r == 0 {
				status = "leader"
			}
			if st.Eliminated {
				status = "eliminated"
			}
			exactESSPerWork := 0.0
			if st.Work > 0 {
				exactESSPerWork = st.Exact.ESS / float64(st.Work)
			}
			protected := !legacyRacing && cemStateNeedsProtectedSamples(st) && !cemAnyHeldOutExact(states)
			minimumRemaining := max(0, CEMAdaptationExactMinEvaluationSamples-st.Samples)
			log.Printf("rare-position-evaluation-race: group=%d stage=%d rank=%d team=%d position=%d snapshot_iteration=%d samples=%d score_tier=%s exact_hits=%d exact_ess_per_work=%.8g near1_efficiency_ratio=%.3f near2_efficiency_ratio=%.3f adaptation_exact=%t protected=%t minimum_evaluation_remaining=%d adaptation_exact_hits=%d adaptation_near_target_hits=%d adaptation_priority=%.3f status=%s reason=%s",
				groupID, stage, r+1, st.Snapshot.CandidateTeam, st.Snapshot.CandidatePosition,
				st.Snapshot.SourceIteration, st.Samples, tier, st.Exact.Hits, exactESSPerWork,
				st.Near1EfficiencyRatio, st.Near2EfficiencyRatio, st.HadAdaptationExactHit,
				protected, minimumRemaining, st.AdaptationExactHits, st.AdaptationNearTargetHits,
				st.AdaptationPriority, status, reason)
		}
	}
	evaluateChunk := func(state *CEMEvaluationState, stage int, isProtected bool) bool {
		if *remainingWork < chunkWork || *evaluationWorkRemaining < chunkWork ||
			cemTotalStateSamples(states)+CEMEvaluationChunkSamples > CEMMaxEvaluationSamples {
			return false
		}
		state.Stage = stage
		_, p1, p2 := scoutPNeighborhood(normalPositionCounts, state.Snapshot.CandidateTeam,
			state.Snapshot.CandidatePosition, normalSamples)
		chunkSeed := deriveRarePositionSeed(evaluationSeed, fmt.Sprintf("eval-chunk-team%d-pos%d-iter%d-chunk%d",
			state.Snapshot.CandidateTeam, state.Snapshot.CandidatePosition, state.Snapshot.SourceIteration, state.Chunks+1))
		if isProtected {
			log.Printf("rare-position-evaluation-protected: group=%d team=%d position=%d snapshot_iteration=%d current_samples=%d minimum_samples=%d adaptation_exact_hits=%d held_out_exact_hits=%d chunk_samples=%d reason=adaptation_exact_minimum",
				groupID, state.Snapshot.CandidateTeam, state.Snapshot.CandidatePosition,
				state.Snapshot.SourceIteration, state.Samples, CEMAdaptationExactMinEvaluationSamples,
				state.AdaptationExactHits, state.Exact.Hits, CEMEvaluationChunkSamples)
		}
		evaluateCEMChunk(state, original, baseCampaign, games, table, order, teamGroups,
			CEMEvaluationChunkSamples, workPerSample,
			rand.New(rand.NewSource(chunkSeed)), groupID, p1, p2)
		switch state.Samples {
		case 200:
			state.SamplesAt200, state.ExactHitsAt200 = state.Samples, state.Exact.Hits
		case 400:
			state.SamplesAt400, state.ExactHitsAt400 = state.Samples, state.Exact.Hits
		case 600:
			state.SamplesAt600, state.ExactHitsAt600 = state.Samples, state.Exact.Hits
		}
		*evaluationWorkRemaining -= chunkWork
		*remainingWork -= chunkWork
		return true
	}

	// STAGE 1: Screening - 100 samples per shortlisted proposal
	for _, st := range states {
		if !evaluateChunk(st, 1, false) {
			st.Eliminated = true
			st.EliminationReason = "evaluation_budget_exhausted"
		}
	}

	logRaceRanking(1, "stage1_screening_complete")

	rankedStage1 := append([]*CEMEvaluationState(nil), states...)
	sort.Slice(rankedStage1, func(i, j int) bool { return rankBeforeProtected(rankedStage1[i], rankedStage1[j]) })

	if len(rankedStage1) > 0 && cemStateEarlyAccept(rankedStage1[0]) {
		leader := rankedStage1[0]
		selected := convertToSingleEvaluation(groupID, leader, true, "early_accept_stage1")
		return finalizeCEMEvaluationResult(groupID, states, selected, true, "early_accept_stage1", workPerSample)
	}

	// STAGE 2: Narrowing - Keep top 2, evaluate +100 samples
	var stage2Survivors []*CEMEvaluationState
	var stage2ToSample []*CEMEvaluationState
	for i, st := range rankedStage1 {
		if st.Eliminated {
			continue
		}
		if i < 2 {
			st.Stage = 2
			stage2Survivors = append(stage2Survivors, st)
			stage2ToSample = append(stage2ToSample, st)
			continue
		}
		if !legacyRacing && !cemAnyHeldOutExact(states) && st.HadAdaptationExactHit && st.Exact.Hits == 0 {
			st.Stage = 3
			st.EliminationReason = "preserved_for_adaptation_exact_minimum"
			stage2Survivors = append(stage2Survivors, st)
			continue
		}
		st.Eliminated = true
		st.EliminationReason = "stage1_bottom_rank"
	}

	for _, st := range stage2ToSample {
		if !evaluateChunk(st, 2, false) {
			st.Eliminated = true
			st.EliminationReason = "evaluation_budget_exhausted"
		}
	}

	logRaceRanking(2, "stage2_narrowing_complete")

	sort.Slice(stage2Survivors, func(i, j int) bool {
		return rankBeforeProtected(stage2Survivors[i], stage2Survivors[j])
	})

	if len(stage2Survivors) > 0 && cemStateEarlyAccept(stage2Survivors[0]) {
		leader := stage2Survivors[0]
		selected := convertToSingleEvaluation(groupID, leader, true, "early_accept_stage2")
		return finalizeCEMEvaluationResult(groupID, states, selected, true, "early_accept_stage2", workPerSample)
	}

	// STAGE 3A: Protect proposals with adaptation exact-event evidence from
	// being starved by surrogate-only held-out rankings.
	if !legacyRacing && !cemAnyHeldOutExact(states) {
		cursor := 0
		for !cemAnyHeldOutExact(states) &&
			cemTotalStateSamples(states)+CEMEvaluationChunkSamples <= CEMMaxEvaluationSamples {
			state, nextCursor := nextCEMProtectedCandidate(stage2Survivors, cursor)
			if state == nil {
				break
			}
			cursor = nextCursor
			if !evaluateChunk(state, 3, true) {
				break
			}
			logRaceRanking(3, "stage3a_adaptation_exact_minimum")
		}
	}

	// STAGE 3B: Open racing among survivors after protected minimum allocation.
	var activeSurvivors []*CEMEvaluationState
	for _, st := range stage2Survivors {
		if !st.Eliminated {
			st.Stage = 4
			activeSurvivors = append(activeSurvivors, st)
		}
	}

	for cemTotalStateSamples(states) < CEMMaxEvaluationSamples && *remainingWork >= chunkWork && *evaluationWorkRemaining >= chunkWork && len(activeSurvivors) > 0 {
		sort.Slice(activeSurvivors, func(i, j int) bool {
			return rankBeforeProtected(activeSurvivors[i], activeSurvivors[j])
		})

		currentLeader := activeSurvivors[0]
		if cemStateEarlyAccept(currentLeader) {
			selected := convertToSingleEvaluation(groupID, currentLeader, true, "early_accept_stage3b")
			return finalizeCEMEvaluationResult(groupID, states, selected, true, "early_accept_stage3b", workPerSample)
		}
		if !evaluateChunk(currentLeader, 4, false) {
			break
		}

		logRaceRanking(4, "stage3b_open_racing_chunk")

		if cemStateEarlyAccept(currentLeader) {
			selected := convertToSingleEvaluation(groupID, currentLeader, true, "early_accept_stage3b")
			return finalizeCEMEvaluationResult(groupID, states, selected, true, "early_accept_stage3b", workPerSample)
		}
	}

	finalRanked := append([]*CEMEvaluationState(nil), states...)
	sort.Slice(finalRanked, func(i, j int) bool {
		return rankBeforeProtected(finalRanked[i], finalRanked[j])
	})

	if len(finalRanked) > 0 {
		best := finalRanked[0]
		if best.Exact.Hits >= 1 && best.Exact.ESS > 0 {
			selected := convertToSingleEvaluation(groupID, best, false, "final_evaluation_exact_event_winner")
			return finalizeCEMEvaluationResult(groupID, states, selected, false, "final_evaluation_exact_event_winner", workPerSample)
		}
	}
	return finalizeCEMEvaluationResult(groupID, states, nil, false, "all_proposals_zero_exact_hits_in_evaluation", workPerSample)
}

func cemTotalStateSamples(states []*CEMEvaluationState) int {
	total := 0
	for _, state := range states {
		total += state.Samples
	}
	return total
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
