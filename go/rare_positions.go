package main

import (
	"encoding/binary"
	"fmt"
	"hash/fnv"
	"log"
	"math"
	"math/rand"
	"os"
	"sort"
	"strconv"
	"time"
)

const (
	MinInterestingProbability = 1e-5
	ScoutIterations           = 20000
)

func rarePositionSeed() (int64, string) {
	configured := os.Getenv("RARE_POSITION_RANDOM_SEED")
	if configured != "" {
		seed, err := strconv.ParseInt(configured, 10, 64)
		if err == nil {
			return seed, "configured"
		}
		log.Printf("rare-position-rng: invalid configured seed=%q; using time seed", configured)
	}
	return time.Now().UnixNano(), "time"
}

func cemFrontierFingerprint(candidates []*FrontierCandidate,
	searches map[int]*TeamRareSearch) string {
	h := fnv.New64a()
	teamIDs := make([]int, 0, len(searches))
	for teamID := range searches {
		teamIDs = append(teamIDs, teamID)
	}
	sort.Ints(teamIDs)
	for _, teamID := range teamIDs {
		search := searches[teamID]
		fmt.Fprintf(h, "team:%d:%.12g:%t/", teamID, search.NormalMeanRank, search.Has100PercentNormal)
		for _, position := range search.Positions {
			fmt.Fprintf(h, "%d:%d:%t:%d:%.12g/", position.Position, position.Status,
				position.Feasible, position.ObservedCount, position.NormalProb)
		}
	}
	ordered := append([]*FrontierCandidate(nil), candidates...)
	sort.Slice(ordered, func(i, j int) bool {
		if ordered[i].TeamID != ordered[j].TeamID {
			return ordered[i].TeamID < ordered[j].TeamID
		}
		if ordered[i].Position != ordered[j].Position {
			return ordered[i].Position < ordered[j].Position
		}
		return ordered[i].Direction < ordered[j].Direction
	})
	for _, candidate := range ordered {
		fmt.Fprintf(h, "candidate:%d:%d:%d:%d/", candidate.TeamID, candidate.Position,
			candidate.Direction, candidate.Priority)
	}
	return fmt.Sprintf("%016x", h.Sum64())
}

func deriveRarePositionSeed(seed int64, stream string) int64 {
	var encoded [8]byte
	binary.LittleEndian.PutUint64(encoded[:], uint64(seed))
	hash := fnv.New64a()
	_, _ = hash.Write(encoded[:])
	_, _ = hash.Write([]byte(stream))
	return int64(hash.Sum64())
}

func rarePositionScoutIterations() int {
	configured := os.Getenv("RARE_POSITION_BENCHMARK_ITERATIONS")
	if configured != "" {
		iterations, err := strconv.Atoi(configured)
		if err == nil && iterations > 0 {
			return iterations
		}
		log.Printf("rare-position-benchmark: invalid iteration count=%q; using %d", configured, ScoutIterations)
	}
	return ScoutIterations
}

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
	Probability         float64                        `json:"probability"`
	StdErr              float64                        `json:"std_err"`
	Samples             int                            `json:"samples"`
	Hits                int                            `json:"hits"`
	ESS                 float64                        `json:"ess"`
	MeanWeight          float64                        `json:"mean_weight"`
	WorkSpent           int64                          `json:"work_spent"`
	Available           bool                           `json:"available"`
	MeetsPrecisionGoal  bool                           `json:"meets_precision_goal"`
	RelativeSE          *float64                       `json:"relative_se"`
	MaxEventWeightShare float64                        `json:"max_event_weight_share"`
	ZeroHitUpper95      float64                        `json:"zero_hit_upper_95"`
	Design              string                         `json:"design"`
	SearchDiagnostics   *RarePositionSearchDiagnostics `json:"search_diagnostics,omitempty"`
}

type RarePositionSearchDiagnostics struct {
	ScoutWork                             int64           `json:"scout_work"`
	AdaptationWork                        int64           `json:"adaptation_work"`
	EvaluationWork                        int64           `json:"evaluation_work"`
	ProductionWork                        int64           `json:"production_work"`
	TotalWork                             int64           `json:"total_work"`
	TotalWorkLimit                        int64           `json:"total_work_limit"`
	EvaluationWorkSavedPlainMCEq          float64         `json:"evaluation_work_saved_plain_mc_equivalent"`
	EvaluationTotalSamples                int             `json:"evaluation_total_samples"`
	EvaluationResultWork                  int64           `json:"evaluation_result_work"`
	SelectedEvaluationSamples             int             `json:"selected_evaluation_samples"`
	SelectedEvaluationWork                int64           `json:"selected_evaluation_work"`
	CEMInitializationMode                 string          `json:"cem_initialization_mode"`
	CEMInitializationSource               string          `json:"cem_initialization_source"`
	CEMParameterization                   string          `json:"cem_parameterization"`
	CEMShortlistFingerprint               string          `json:"cem_shortlist_fingerprint"`
	CEMFrontierFingerprint                string          `json:"cem_frontier_fingerprint"`
	InitialAttackL2                       float64         `json:"initial_attack_l2"`
	InitialConcedeL2                      float64         `json:"initial_concede_l2"`
	SelectedAttackL2                      float64         `json:"selected_attack_l2"`
	SelectedConcedeL2                     float64         `json:"selected_concede_l2"`
	MaxAbsAttack                          float64         `json:"max_abs_attack"`
	MaxAbsConcede                         float64         `json:"max_abs_concede"`
	SelectedAttackParameters              map[int]float64 `json:"selected_attack_parameters,omitempty"`
	SelectedConcessionParameters          map[int]float64 `json:"selected_concession_parameters,omitempty"`
	FitIterations                         int             `json:"fit_iterations"`
	FitConverged                          bool            `json:"fit_converged"`
	FitObjectiveStart                     float64         `json:"fit_objective_start"`
	FitObjectiveEnd                       float64         `json:"fit_objective_end"`
	FitWallSeconds                        float64         `json:"fit_wall_seconds"`
	TotalWallSeconds                      float64         `json:"total_wall_seconds"`
	MeanExactESSPerWork                   float64         `json:"mean_exact_ess_per_work"`
	MeanNear1EfficiencyRatio              float64         `json:"mean_near1_efficiency_ratio"`
	MeanNear2EfficiencyRatio              float64         `json:"mean_near2_efficiency_ratio"`
	MeanWeightedExactToNear1Ratio         float64         `json:"mean_weighted_exact_to_near1_ratio"`
	Near1EfficientEvaluations             int             `json:"near1_efficient_evaluations"`
	Near1EfficientWithExactHit            int             `json:"near1_efficient_with_exact_hit"`
	Near1EfficientWithoutExactHit         int             `json:"near1_efficient_without_exact_hit"`
	EliteDistanceAt300                    float64         `json:"elite_distance_at_300"`
	EliteDistanceAt600                    float64         `json:"elite_distance_at_600"`
	EliteDistanceAt900                    float64         `json:"elite_distance_at_900"`
	MeanSamplesToFirstNear                float64         `json:"mean_samples_to_first_near"`
	MeanSamplesToFirstExact               float64         `json:"mean_samples_to_first_exact"`
	RepeatedExactHitBatches               int             `json:"repeated_exact_hit_batches"`
	BestNearTargetRate                    float64         `json:"best_near_target_rate"`
	BestExactRate                         float64         `json:"best_exact_rate"`
	AdaptationExactSnapshotsShortlisted   int             `json:"adaptation_exact_snapshots_shortlisted"`
	AdaptationExactSnapshotsLE200         int             `json:"adaptation_exact_snapshots_le_200_samples"`
	AdaptationExactSnapshotsGE400         int             `json:"adaptation_exact_snapshots_ge_400_samples"`
	AdaptationExactMeanEvaluationSamples  float64         `json:"adaptation_exact_mean_evaluation_samples"`
	AdaptationExactStarved                int             `json:"adaptation_exact_starved"`
	AdaptationExactProtected              int             `json:"adaptation_exact_protected"`
	AdaptationExactRescued                int             `json:"adaptation_exact_rescued"`
	AdaptationExactStillZeroAfter400      int             `json:"adaptation_exact_still_zero_after_400"`
	AdaptationExactStillZeroAfter600      int             `json:"adaptation_exact_still_zero_after_600"`
	SurrogateOnlySnapshotsOver400         int             `json:"surrogate_only_snapshots_over_400_samples"`
	CandidatesAdmitted                    int             `json:"candidates_admitted"`
	CEMBatches                            int             `json:"cem_batches"`
	AdaptationExactHitBatches             int             `json:"adaptation_exact_hit_batches"`
	TargetsWithExactHit                   int             `json:"targets_with_exact_hit"`
	TargetsWithNearTargetHit              int             `json:"targets_with_near_target_hit"`
	RetainedSnapshots                     int             `json:"retained_snapshots"`
	EligibleSnapshots                     int             `json:"eligible_snapshots"`
	EvaluatedSnapshots                    int             `json:"evaluated_snapshots"`
	EvaluationHits                        int             `json:"evaluation_hits"`
	EvaluationSnapshotsWithExactHit       int             `json:"evaluation_snapshots_with_exact_hit"`
	EvaluationESSPerWorkSum               float64         `json:"evaluation_ess_per_work_sum"`
	EvaluationESS                         float64         `json:"evaluation_ess"`
	EvaluationESSPerWork                  float64         `json:"evaluation_ess_per_work"`
	SelectedSnapshotIteration             int             `json:"selected_snapshot_iteration"`
	SearchOverheadPlainMCEq               float64         `json:"search_overhead_plain_mc_equivalent"`
	FreshProductionPlainMCEq              float64         `json:"fresh_production_plain_mc_equivalent"`
	ExactHitCandidates                    int             `json:"exact_hit_candidates"`
	ExactHitCandidatesWithLater           int             `json:"exact_hit_candidates_with_later_adaptation"`
	FirstHitSnapshotEvaluated             int             `json:"first_hit_snapshots_evaluated"`
	LaterSnapshotEvaluated                int             `json:"later_snapshots_evaluated"`
	LaterBetterESSPerWork                 int             `json:"later_snapshot_better_ess_per_work"`
	FirstHitBetterESSPerWork              int             `json:"first_hit_snapshot_better_ess_per_work"`
	LaterOnlySnapshotEvaluated            int             `json:"later_only_snapshot_evaluated"`
	NearTargetSnapshots                   int             `json:"near_target_snapshots"`
	SnapshotsRejectedEquivalentToP        int             `json:"snapshots_rejected_equivalent_to_P"`
	SnapshotsRejectedInsufficientProgress int             `json:"snapshots_rejected_insufficient_progress"`
	SnapshotsEligibleExactHit             int             `json:"snapshots_eligible_exact_hit"`
	SnapshotsEligibleNearTarget           int             `json:"snapshots_eligible_near_target"`
	SnapshotsEligibleBestRank             int             `json:"snapshots_eligible_best_rank_close"`
	SnapshotsEligibleDistance             int             `json:"snapshots_eligible_distance_improvement"`
	SelectedTeam                          int             `json:"selected_team"`
	SelectedPosition                      int             `json:"selected_position"`
	ISSelectedAfterExactHit               bool            `json:"is_selected_after_exact_hit"`
	ISSelectedWithoutExactHit             bool            `json:"is_selected_without_exact_hit"`
	ISSelectedNearTargetOnly              bool            `json:"is_selected_near_target_only"`
	ISSelectedAfterEvaluatedExactHit      bool            `json:"is_selected_after_evaluated_exact_hit"`
	WeakEvaluationEvidence                bool            `json:"weak_evaluation_evidence"`
	SelectedProductionHits                int             `json:"selected_production_hits"`
	SelectedProductionESS                 float64         `json:"selected_production_ess"`
	SelectedProductionRelSE               float64         `json:"selected_production_rel_se"`
	SelectedProductionMaxEventWeightShare float64         `json:"selected_production_max_event_weight_share"`
	SelectedProductionMeanWeight          float64         `json:"selected_production_mean_weight"`
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
	return initializeTeamRareSearchWithRNG(teamID, normalCounts, normalProbs, campaign,
		teamGroups, games, table, sortOrder, nil)
}

func initializeTeamRareSearchWithRNG(
	teamID int, normalCounts []int, normalProbs []float64,
	campaign []*TeamCampaign, teamGroups []TeamType, games []*GameType,
	table *Table, sortOrder []SortType, rng *rand.Rand,
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
			feasible := possiblePositionByPointsBoundsWithRNG(
				teamID, pos, campaign, teamGroups, games, table, sortOrder,
				rng,
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
			nil,
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
	return possiblePositionByPointsBoundsWithRNG(targetTeamID, targetPosition,
		campaign, teamGroups, games, table, sortOrder, nil)
}

func possiblePositionByPointsBoundsWithRNG(
	targetTeamID int, targetPosition int, campaign []*TeamCampaign,
	teamGroups []TeamType, games []*GameType, table *Table,
	sortOrder []SortType, rng *rand.Rand,
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
		sortedTeams := TeamCampaignSorted{t: teamSlice, sort: sortOrder, rng: rng}
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
		sort.Sort(TeamCampaignSorted{t: teamSlice, sort: sortOrder, rng: rng})
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
	estimates := runRarePositionSearchEvaluationProduction(group, campaign, table, sortOrder,
		normalPositionCounts, teamOdds, normalSamples)
	for _, tg := range group.Team_groups {
		teamID := tg.Team_id
		if teamEstimates := estimates[teamID]; len(teamEstimates) > 0 {
			rareMap := make(map[int]RarePositionEstimate, len(teamEstimates))
			for pos, est := range teamEstimates {
				if est.Available {
					relSE := 0.0
					if est.RelativeSE != nil {
						relSE = *est.RelativeSE
					}
					rareMap[pos] = RarePositionEstimate{
						Probability: est.Probability, StdErr: est.StdErr, Samples: est.Samples,
						Hits: est.Hits, ESS: est.ESS, MeanWeight: est.MeanWeight,
						Available: est.Available, MeetsPrecisionGoal: est.MeetsPrecisionGoal,
						RelativeSE: relSE, MaxEventWeightShare: est.MaxEventWeightShare,
						ZeroHitUpper95: est.ZeroHitUpper95, WorkSpent: est.WorkSpent,
						Found: true,
					}
				}
			}
			if len(rareMap) > 0 {
				index := table.Query(uint32(teamID))
				odds := teamOdds[index].team
				final := mergeRarePositionEstimates(odds.Pos, normalPositionCounts[teamID], rareMap)
				copy(odds.Pos, final)
			}
		}
	}
	return estimates
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
	seed, seedSource := rarePositionSeed()
	parameterization := cemParameterizationFromEnvironment()
	family := "scoring"
	if parameterization == CEMTeamAttackConcession {
		family = "attack-concession"
	}
	frontierSeed := deriveRarePositionSeed(seed, "rare-frontier")
	searchSeed := deriveRarePositionSeed(seed, family+"-search")
	evaluationSeed := deriveRarePositionSeed(seed, family+"-evaluation")
	productionSeed := deriveRarePositionSeed(seed, family+"-production")
	frontierRNG := rand.New(rand.NewSource(frontierSeed))
	searchRNG := rand.New(rand.NewSource(searchSeed))
	productionRNG := rand.New(rand.NewSource(productionSeed))
	log.Printf("rare-position-rng: group=%d seed=%d parameterization=%s frontier_seed=%d search_seed=%d evaluation_seed=%d production_seed=%d source=%s phase=search,evaluation,production",
		group.Id, seed, cemParameterizationName(parameterization), frontierSeed, searchSeed, evaluationSeed, productionSeed, seedSource)
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
		search := initializeTeamRareSearchWithRNG(teamID, normalPositionCounts[teamID],
			teamOdds[index].team.Pos, campaign, group.Team_groups, group.Games, table, sortOrder, frontierRNG)
		teamSearches[teamID] = search
		for _, position := range search.Positions {
			if position.ObservedCount > 0 {
				scoutResolvedCells++
			}
		}
	}

	candidates := collectFrontierCandidates(teamSearches)
	frontierFingerprint := cemFrontierFingerprint(candidates, teamSearches)
	var cemRound CEMRoundResult
	initializationMode, initializationSource := cemInitializationMode()
	log.Printf("rare-position-cem-init-mode: group=%d mode=%s source=%s parameterization=%s",
		group.Id, cemInitializationModeName(initializationMode), initializationSource, cemParameterizationName(parameterization))
	if len(candidates) > 0 && remainingWork > 0 && cemWorkRemaining > 0 {
		cemRound = runCEMAdaptationRoundWithModeAndParameterization(initializationMode, parameterization, candidates, teamSearches, group, campaign,
			table, sortOrder, originalMeans, &cemWorkRemaining, &remainingWork,
			&explorationWorkRemaining, adaptWorkPerSample, searchRNG)
	}
	if remainingWork < 0 || cemWorkRemaining < 0 || explorationWorkRemaining < 0 {
		panic("rare-position adaptation work budget exceeded")
	}
	adaptationWork := cemRound.CEMWork

	evaluationCapacity := cemEvaluationCapacity(CEMMaxEvaluationSnapshots,
		evaluationWorkRemaining, remainingWork, int64(CEMEvaluationChunkSamples)*evaluationWorkPerSample)
	evaluationCandidates := prepareCEMEvaluationSnapshots(group.Id, cemRound.Snapshots,
		originalMeans, evaluationCapacity)

	legacyRacing := os.Getenv("RARE_POSITION_CEM_RACING_MODE") == "legacy"
	evaluationResult := runCEMProposalRacingWithResult(
		group.Id,
		evaluationCandidates,
		originalMeans,
		campaign,
		group.Games,
		table,
		sortOrder,
		group.Team_groups,
		normalPositionCounts,
		normalSamples,
		&evaluationWorkRemaining,
		&remainingWork,
		evaluationWorkPerSample,
		evaluationSeed,
		legacyRacing,
	)
	if evaluationResult.TotalWork != evaluationWorkLimit-evaluationWorkRemaining {
		panic(fmt.Sprintf("CEM evaluation work accounting mismatch: race_result=%d budget_delta=%d",
			evaluationResult.TotalWork, evaluationWorkLimit-evaluationWorkRemaining))
	}
	evaluations, selectedEvaluation := evaluationResult.Evaluations, evaluationResult.Selected
	evaluationWork := evaluationResult.TotalWork
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
				Components: cloneProposalComponents(design.Components), Iterations: design.Samples,
				CollectAllRanks: true, FullRankWork: design.Work}
			raw, _ := estimateRarePositionsForJob(campaign, group.Games, originalMeans,
				table, sortOrder, group.Team_groups, job, productionRNG, group.Id)
			_ = raw // The complete rank table below is accumulated from the same fresh production draws.
			for teamID, rankEstimates := range job.FullRankEstimates {
				productionEstimates[teamID] = make(map[int]ProductionEstimate, len(rankEstimates))
				for position, estimate := range rankEstimates {
					productionEstimates[teamID][position] = ProductionEstimate{
						Probability: estimate.Probability, StdErr: estimate.StdErr,
						Samples: estimate.Samples, Hits: estimate.Hits, ESS: estimate.ESS,
						MeanWeight: estimate.MeanWeight, WorkSpent: estimate.WorkSpent,
						Available: estimate.Available, MeetsPrecisionGoal: estimate.MeetsPrecisionGoal,
						RelativeSE:          relativeSEPointer(estimate.RelativeSE),
						MaxEventWeightShare: estimate.MaxEventWeightShare,
						ZeroHitUpper95:      estimate.ZeroHitUpper95, Design: design.Kind,
					}
				}
			}
		} else {
			counts := simulatePlainRankCounts(campaign, group.Games, table, sortOrder,
				group.Team_groups, design.Samples, productionRNG)
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
	diagnostics := rarePositionSearchDiagnostics(cemRound, evaluations, selectedEvaluation,
		originalMeans, design, scoutWork, adaptationWork, evaluationWork, productionWork,
		totalWorkLimit, evaluationWorkLimit, plainWorkPerSample, productionEstimates)
	diagnostics.EvaluationTotalSamples = evaluationResult.TotalSamples
	diagnostics.EvaluationResultWork = evaluationResult.TotalWork
	diagnostics.SelectedEvaluationSamples = evaluationResult.SelectedSamples
	diagnostics.SelectedEvaluationWork = evaluationResult.SelectedWork
	diagnostics.CEMInitializationMode = cemInitializationModeName(initializationMode)
	diagnostics.CEMInitializationSource = initializationSource
	diagnostics.CEMParameterization = cemParameterizationName(parameterization)
	diagnostics.CEMShortlistFingerprint = cemEvaluationShortlistFingerprint(evaluations)
	diagnostics.CEMFrontierFingerprint = frontierFingerprint
	diagnostics.InitialAttackL2 = cemRound.InitialAttackL2
	diagnostics.InitialConcedeL2 = cemRound.InitialConcessionL2
	diagnostics.FitIterations = cemRound.AttackConcessionFitIterations
	diagnostics.FitConverged = cemRound.AttackConcessionFitUpdates == 0 ||
		cemRound.AttackConcessionFitConverged == cemRound.AttackConcessionFitUpdates
	diagnostics.FitObjectiveStart = cemRound.AttackConcessionFitObjectiveStart
	diagnostics.FitObjectiveEnd = cemRound.AttackConcessionFitObjectiveEnd
	diagnostics.FitWallSeconds = cemRound.AttackConcessionFitWallTime.Seconds()
	if selectedEvaluation != nil && selectedEvaluation.Snapshot.Proposal.Parameterization == CEMTeamAttackConcession {
		diagnostics.SelectedAttackParameters = copyTheta(selectedEvaluation.Snapshot.Proposal.AttackTheta)
		diagnostics.SelectedConcessionParameters = copyTheta(selectedEvaluation.Snapshot.Proposal.ConcessionTheta)
		for _, value := range selectedEvaluation.Snapshot.Proposal.AttackTheta {
			diagnostics.SelectedAttackL2 += value * value
			diagnostics.MaxAbsAttack = math.Max(diagnostics.MaxAbsAttack, math.Abs(value))
		}
		for _, value := range selectedEvaluation.Snapshot.Proposal.ConcessionTheta {
			diagnostics.SelectedConcedeL2 += value * value
			diagnostics.MaxAbsConcede = math.Max(diagnostics.MaxAbsConcede, math.Abs(value))
		}
		diagnostics.SelectedAttackL2 = math.Sqrt(diagnostics.SelectedAttackL2)
		diagnostics.SelectedConcedeL2 = math.Sqrt(diagnostics.SelectedConcedeL2)
	}
	for teamID, positions := range productionEstimates {
		for position, estimate := range positions {
			estimate.SearchDiagnostics = diagnostics
			productionEstimates[teamID][position] = estimate
		}
	}

	workSpent := scoutWork + adaptationWork + evaluationWork + productionWork
	if workSpent > totalWorkLimit || remainingWork < 0 || workSpent+remainingWork != totalWorkLimit {
		panic("rare-position total work budget exceeded or phase accounting is inconsistent")
	}
	searchOverhead := scoutWork + adaptationWork + evaluationWork
	log.Printf("rare-position-summary: parameterization=%s group=%d scout_samples=%d scout_work=%d adaptation_work=%d adaptation_plain_mc_equiv=%.1f evaluation_work=%d evaluation_plain_mc_equiv=%.1f snapshots_retained=%d snapshots_evaluated=%d production_design=%s production_samples=%d production_work=%d fresh_final_estimator_samples=%d search_overhead_plain_mc_equiv=%.1f fresh_production_plain_mc_equiv=%.1f total_work_limit=%d work_spent=%d unused_work=%d scout_resolved_cells=%d cem_batches=%d candidates_admitted=%d exact_hit_targets=%d plain_P_selected=%t",
		cemParameterizationName(parameterization), group.Id, normalSamples, scoutWork, adaptationWork, float64(adaptationWork)/float64(plainWorkPerSample),
		evaluationWork, float64(evaluationWork)/float64(plainWorkPerSample), len(cemRound.Snapshots),
		len(evaluations), design.Kind, design.Samples, productionWork, design.Samples,
		float64(searchOverhead)/float64(plainWorkPerSample),
		float64(productionWork)/float64(plainWorkPerSample), totalWorkLimit,
		workSpent, remainingWork, scoutResolvedCells, cemRound.Iterations,
		cemRound.CandidatesAdmitted, cemRound.TargetsAnyExact, design.Kind == "plain_mc")
	return productionEstimates
}

func rarePositionSearchDiagnostics(round CEMRoundResult, evaluations []CEMProposalEvaluation,
	selected *CEMProposalEvaluation, originalMeans []GameProposalMeans, design ProductionDesign,
	scoutWork, adaptationWork, evaluationWork, productionWork, totalWorkLimit,
	evaluationWorkLimit, plainWorkPerSample int64,
	productionEstimates map[int]map[int]ProductionEstimate) *RarePositionSearchDiagnostics {
	diagnostics := &RarePositionSearchDiagnostics{
		ScoutWork: scoutWork, AdaptationWork: adaptationWork, EvaluationWork: evaluationWork,
		ProductionWork: productionWork, TotalWork: scoutWork + adaptationWork + evaluationWork + productionWork,
		TotalWorkLimit:     totalWorkLimit,
		CandidatesAdmitted: round.CandidatesAdmitted, CEMBatches: round.SchedulerBatches,
		AdaptationExactHitBatches: round.AdaptationExactHitBatches,
		TargetsWithExactHit:       round.TargetsAnyExact, RetainedSnapshots: len(round.Snapshots),
		EvaluatedSnapshots: len(evaluations), SelectedSnapshotIteration: 0,
		EliteDistanceAt300: round.EliteDistanceAt300, EliteDistanceAt600: round.EliteDistanceAt600,
		EliteDistanceAt900: round.EliteDistanceAt900, MeanSamplesToFirstNear: round.MeanSamplesToFirstNear,
		MeanSamplesToFirstExact: round.MeanSamplesToFirstExact, RepeatedExactHitBatches: round.RepeatedExactHitBatches,
		BestNearTargetRate: round.BestNearTargetRate, BestExactRate: round.BestExactRate,
	}
	nearTargets := make(map[[2]int]bool)
	for _, snapshot := range round.Snapshots {
		eligibility := cemSnapshotEligibility(snapshot, originalMeans)
		if snapshot.Stats.NearTargetHits > 0 {
			diagnostics.NearTargetSnapshots++
			nearTargets[[2]int{snapshot.CandidateTeam, snapshot.CandidatePosition}] = true
		}
		if eligibility.Eligible {
			diagnostics.EligibleSnapshots++
		}
		switch eligibility.Reason {
		case "equivalent_to_P":
			diagnostics.SnapshotsRejectedEquivalentToP++
		case "insufficient_progress":
			diagnostics.SnapshotsRejectedInsufficientProgress++
		case "exact_hit":
			diagnostics.SnapshotsEligibleExactHit++
		case "near_target_hit":
			diagnostics.SnapshotsEligibleNearTarget++
		case "best_rank_close":
			diagnostics.SnapshotsEligibleBestRank++
		case "distance_improvement":
			diagnostics.SnapshotsEligibleDistance++
		}
	}
	diagnostics.TargetsWithNearTargetHit = len(nearTargets)
	for _, evaluation := range evaluations {
		if evaluation.Near1.Probability > 0 {
			diagnostics.MeanWeightedExactToNear1Ratio += evaluation.Exact.Probability / evaluation.Near1.Probability
		}
		diagnostics.MeanExactESSPerWork += evaluation.ESSPerWork
		diagnostics.MeanNear1EfficiencyRatio += evaluation.Near1EfficiencyRatio
		diagnostics.MeanNear2EfficiencyRatio += evaluation.Near2EfficiencyRatio
		if evaluation.Near1EfficiencyRatio > 2 {
			diagnostics.Near1EfficientEvaluations++
			if evaluation.Hits > 0 {
				diagnostics.Near1EfficientWithExactHit++
			} else {
				diagnostics.Near1EfficientWithoutExactHit++
			}
		}
		diagnostics.EvaluationHits += evaluation.Hits
		diagnostics.EvaluationESSPerWorkSum += evaluation.ESSPerWork
		if evaluation.Hits > 0 {
			diagnostics.EvaluationSnapshotsWithExactHit++
		}
		if snapshotHadAdaptationExactHit(evaluation.Snapshot) {
			diagnostics.AdaptationExactSnapshotsShortlisted++
			diagnostics.AdaptationExactMeanEvaluationSamples += float64(evaluation.Samples)
			if evaluation.Samples <= 200 && evaluation.Hits == 0 {
				diagnostics.AdaptationExactSnapshotsLE200++
				diagnostics.AdaptationExactStarved++
			}
			if evaluation.Samples >= CEMAdaptationExactMinEvaluationSamples {
				diagnostics.AdaptationExactSnapshotsGE400++
				diagnostics.AdaptationExactProtected++
			}
			if evaluation.ExactHitsAt200 == 0 && evaluation.Samples > 200 && evaluation.Hits > 0 {
				diagnostics.AdaptationExactRescued++
			}
			if evaluation.SamplesAt400 >= 400 && evaluation.ExactHitsAt400 == 0 {
				diagnostics.AdaptationExactStillZeroAfter400++
			}
			if evaluation.SamplesAt600 >= 600 && evaluation.ExactHitsAt600 == 0 {
				diagnostics.AdaptationExactStillZeroAfter600++
			}
		} else if evaluation.Samples > 400 {
			diagnostics.SurrogateOnlySnapshotsOver400++
		}
	}
	if len(evaluations) > 0 {
		diagnostics.MeanExactESSPerWork /= float64(len(evaluations))
		diagnostics.MeanNear1EfficiencyRatio /= float64(len(evaluations))
		diagnostics.MeanNear2EfficiencyRatio /= float64(len(evaluations))
		diagnostics.MeanWeightedExactToNear1Ratio /= float64(len(evaluations))
	}
	if diagnostics.AdaptationExactSnapshotsShortlisted > 0 {
		diagnostics.AdaptationExactMeanEvaluationSamples /= float64(diagnostics.AdaptationExactSnapshotsShortlisted)
	}
	if selected != nil {
		diagnostics.EvaluationESS = selected.ESS
		diagnostics.EvaluationESSPerWork = selected.ESSPerWork
		diagnostics.SelectedSnapshotIteration = selected.Snapshot.SourceIteration
	}
	diagnostics.SelectedTeam, diagnostics.SelectedPosition = design.TargetTeam, design.TargetPosition
	if selected != nil {
		selectedExact := selected.Snapshot.ExactHits > 0 || selected.Snapshot.Stats.ExactHits > 0
		selectedNear := selected.Snapshot.Stats.NearTargetHits > 0
		diagnostics.ISSelectedAfterExactHit = selectedExact
		diagnostics.ISSelectedWithoutExactHit = !selectedExact
		diagnostics.ISSelectedNearTargetOnly = !selectedExact && selectedNear
		diagnostics.ISSelectedAfterEvaluatedExactHit = selected.Hits > 0
		diagnostics.WeakEvaluationEvidence = selected.Hits == 1 && selected.ESS <= 1+1e-12
	}
	if plainWorkPerSample > 0 {
		diagnostics.SearchOverheadPlainMCEq = float64(scoutWork+adaptationWork+evaluationWork) / float64(plainWorkPerSample)
		diagnostics.FreshProductionPlainMCEq = float64(productionWork) / float64(plainWorkPerSample)
		evaluationWorkSaved := evaluationWorkLimit - evaluationWork
		if evaluationWorkSaved > 0 {
			diagnostics.EvaluationWorkSavedPlainMCEq = float64(evaluationWorkSaved) / float64(plainWorkPerSample)
		}
	}
	if design.Kind == "importance_sampling" {
		if estimate, ok := productionEstimates[design.TargetTeam][design.TargetPosition]; ok {
			diagnostics.SelectedProductionHits = estimate.Hits
			diagnostics.SelectedProductionESS = estimate.ESS
			diagnostics.SelectedProductionRelSE = relativeSEValue(estimate.RelativeSE)
			diagnostics.SelectedProductionMaxEventWeightShare = estimate.MaxEventWeightShare
			diagnostics.SelectedProductionMeanWeight = estimate.MeanWeight
		}
	}
	evaluationBySnapshot := make(map[[3]int]CEMProposalEvaluation, len(evaluations))
	for _, evaluation := range evaluations {
		snapshot := evaluation.Snapshot
		evaluationBySnapshot[[3]int{snapshot.CandidateTeam, snapshot.CandidatePosition, snapshot.SourceIteration}] = evaluation
	}
	type exactCandidate struct {
		team, position, firstIteration int
		later                          bool
	}
	exact := make(map[[2]int]exactCandidate)
	for _, snapshot := range round.Snapshots {
		if snapshot.ExactHits <= 0 && snapshot.Stats.ExactHits <= 0 {
			continue
		}
		key := [2]int{snapshot.CandidateTeam, snapshot.CandidatePosition}
		candidate, exists := exact[key]
		if !exists || snapshot.SourceIteration < candidate.firstIteration {
			candidate = exactCandidate{team: key[0], position: key[1], firstIteration: snapshot.SourceIteration}
		}
		exact[key] = candidate
	}
	for key, candidate := range exact {
		for _, snapshot := range round.Snapshots {
			if snapshot.CandidateTeam == key[0] && snapshot.CandidatePosition == key[1] &&
				snapshot.SourceIteration > candidate.firstIteration {
				candidate.later = true
				break
			}
		}
		if candidate.later {
			diagnostics.ExactHitCandidatesWithLater++
		}
		diagnostics.ExactHitCandidates++
		first, firstEvaluated := evaluationBySnapshot[[3]int{candidate.team, candidate.position, candidate.firstIteration}]
		if firstEvaluated {
			diagnostics.FirstHitSnapshotEvaluated++
		}
		for _, snapshot := range round.Snapshots {
			if snapshot.CandidateTeam != candidate.team || snapshot.CandidatePosition != candidate.position ||
				snapshot.SourceIteration <= candidate.firstIteration {
				continue
			}
			later, laterEvaluated := evaluationBySnapshot[[3]int{candidate.team, candidate.position, snapshot.SourceIteration}]
			if !laterEvaluated {
				continue
			}
			diagnostics.LaterSnapshotEvaluated++
			if !firstEvaluated {
				diagnostics.LaterOnlySnapshotEvaluated++
				continue
			}
			if later.ESSPerWork > first.ESSPerWork {
				diagnostics.LaterBetterESSPerWork++
			} else if first.ESSPerWork > later.ESSPerWork {
				diagnostics.FirstHitBetterESSPerWork++
			}
		}
	}
	return diagnostics
}

func cemEvaluationShortlistFingerprint(evaluations []CEMProposalEvaluation) string {
	h := fnv.New64a()
	ordered := append([]CEMProposalEvaluation(nil), evaluations...)
	sort.Slice(ordered, func(i, j int) bool {
		a, b := ordered[i].Snapshot, ordered[j].Snapshot
		if a.CandidateTeam != b.CandidateTeam {
			return a.CandidateTeam < b.CandidateTeam
		}
		if a.CandidatePosition != b.CandidatePosition {
			return a.CandidatePosition < b.CandidatePosition
		}
		return a.SourceIteration < b.SourceIteration
	})
	for _, evaluation := range ordered {
		snapshot := evaluation.Snapshot
		fmt.Fprintf(h, "%d/%d/%d/%d/%d/", snapshot.CandidateTeam, snapshot.CandidatePosition,
			snapshot.SourceIteration, snapshot.Stats.ExactHits, snapshot.Stats.NearTargetHits)
		fmt.Fprintf(h, "parameterization:%d/", snapshot.Proposal.Parameterization)
		teamIDs := make([]int, 0, len(snapshot.Proposal.TeamLogMultipliers))
		for teamID := range snapshot.Proposal.TeamLogMultipliers {
			teamIDs = append(teamIDs, teamID)
		}
		sort.Ints(teamIDs)
		for _, teamID := range teamIDs {
			fmt.Fprintf(h, "%d:%.12g/", teamID, snapshot.Proposal.TeamLogMultipliers[teamID])
		}
		for _, parameters := range []map[int]float64{snapshot.Proposal.AttackTheta, snapshot.Proposal.ConcessionTheta} {
			teamIDs = teamIDs[:0]
			for teamID := range parameters {
				teamIDs = append(teamIDs, teamID)
			}
			sort.Ints(teamIDs)
			for _, teamID := range teamIDs {
				fmt.Fprintf(h, "%d:%.12g/", teamID, parameters[teamID])
			}
		}
		for _, means := range snapshot.Proposal.Means {
			fmt.Fprintf(h, "%.12g,%.12g/", means.Home, means.Away)
		}
	}
	return fmt.Sprintf("%016x", h.Sum64())
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
			(est.Found || est.Available) && est.Probability >= MinInterestingProbability {
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
		if normalCounts[pos] == 0 && isRare && (est.Found || est.Available) && est.Probability >= MinInterestingProbability {
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
	CollectAllRanks      bool
	FullRankEstimates    map[int]map[int]RarePositionEstimate
	FullRankWork         int64
}

type weightedRankAccumulator struct {
	hits       map[int][]int
	sumY       map[int][]float64
	sumY2      map[int][]float64
	maxEventY  map[int][]float64
	sumWeight  map[int]float64
	sampleSize int
}

func newWeightedRankAccumulator(teamGroups []TeamType, numPositions int) *weightedRankAccumulator {
	acc := &weightedRankAccumulator{
		hits: make(map[int][]int, len(teamGroups)), sumY: make(map[int][]float64, len(teamGroups)),
		sumY2: make(map[int][]float64, len(teamGroups)), maxEventY: make(map[int][]float64, len(teamGroups)),
		sumWeight: make(map[int]float64, len(teamGroups)),
	}
	for _, team := range teamGroups {
		acc.hits[team.Team_id] = make([]int, numPositions)
		acc.sumY[team.Team_id] = make([]float64, numPositions)
		acc.sumY2[team.Team_id] = make([]float64, numPositions)
		acc.maxEventY[team.Team_id] = make([]float64, numPositions)
	}
	return acc
}

func (acc *weightedRankAccumulator) observe(sortedTeams []*TeamCampaign, weight float64) {
	if acc == nil {
		return
	}
	acc.sampleSize++
	for rank, team := range sortedTeams {
		acc.hits[team.id][rank]++
		acc.sumY[team.id][rank] += weight
		acc.sumY2[team.id][rank] += weight * weight
		if weight > acc.maxEventY[team.id][rank] {
			acc.maxEventY[team.id][rank] = weight
		}
		acc.sumWeight[team.id] += weight
	}
}

func (acc *weightedRankAccumulator) estimates(work int64) map[int]map[int]RarePositionEstimate {
	if acc == nil || acc.sampleSize == 0 {
		return map[int]map[int]RarePositionEstimate{}
	}
	estimates := make(map[int]map[int]RarePositionEstimate, len(acc.hits))
	for teamID, rankHits := range acc.hits {
		estimates[teamID] = make(map[int]RarePositionEstimate, len(rankHits))
		for position, hits := range rankHits {
			probability, stdErr, ess := eventEstimateStats(acc.sumY[teamID][position], acc.sumY2[teamID][position], acc.sampleSize)
			relSE := math.Inf(1)
			if probability > 0 {
				relSE = stdErr / probability
			}
			maxShare := 0.0
			if acc.sumY[teamID][position] > 0 {
				maxShare = acc.maxEventY[teamID][position] / acc.sumY[teamID][position]
			}
			upper := 0.0
			if hits == 0 {
				upper = weightedZeroHitUpper95(acc.sampleSize, 1/OriginalMixtureWeight)
			}
			estimates[teamID][position] = RarePositionEstimate{
				Probability: probability, StdErr: stdErr, Samples: acc.sampleSize, Hits: hits,
				ESS: ess, MeanWeight: acc.sumWeight[teamID] / float64(acc.sampleSize),
				Available: true, MeetsPrecisionGoal: estimateMeetsPrecisionGoal(ess, relSE),
				RelativeSE: relSE, MaxEventWeightShare: maxShare, ZeroHitUpper95: upper, WorkSpent: work,
			}
		}
	}
	return estimates
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
	allRankAccumulator *weightedRankAccumulator,
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

	sortedTeams := TeamCampaignSorted{t: teamSlice[:idx], sort: sortOrder, rng: rng}
	sort.Sort(sortedTeams)
	allRankAccumulator.observe(sortedTeams.t, weight)

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
	var fullRankAccumulator *weightedRankAccumulator
	if job.CollectAllRanks {
		fullRankAccumulator = newWeightedRankAccumulator(teamGroups, numPositions)
	}
	for i := 0; i < totalN; i++ {
		rank, w, chosenComponent := simulateTargetTeamRankAndWeightMulti(
			baseCampaign, simCampaign, teamSlice, games,
			originalMeans, job.Components, table, sortOrder, teamGroups,
			job.TeamID, rng, logQBuf, compWeightsBuf, fullRankAccumulator,
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

	if fullRankAccumulator != nil {
		job.FullRankEstimates = fullRankAccumulator.estimates(job.FullRankWork)
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
