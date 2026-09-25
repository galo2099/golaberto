package main

import (
	"encoding/csv"
	"encoding/json"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"
)

// This opt-in harness consumes an existing serialized odds request for group
// 16982; normal test runs never execute the 20-seed experiment. Run it with:
//
//	RARE_POSITION_BENCHMARK=1 RARE_POSITION_BENCHMARK_GROUP_JSON=/path/group.json \
//	  go test ./go -run '^TestRarePositionMatchedComputeBenchmark$' -count=1
type rareBenchmarkTarget struct {
	Team      int     `json:"team"`
	Position  int     `json:"position"`
	Reference float64 `json:"reference_probability"`
}

var rareBenchmarkTargets = []rareBenchmarkTarget{
	{632, 7, 4.1e-5}, {173, 2, 2.4e-5}, {1252, 3, 4.2e-5}, {36, 17, 1.8e-5},
	{37, 10, 9.5e-5}, {37, 11, 3.8e-5}, {37, 12, 1.6e-5},
	{26, 8, 1.5e-4}, {26, 9, 6.4e-5}, {26, 10, 1.8e-5}, {26, 11, 1.0e-5},
	{44, 1, 8.1e-5}, {1528, 0, 3.0e-5}, {1558, 19, 5.3e-5},
}

type rareBenchmarkRow struct {
	Seed                             int64   `json:"seed"`
	Method                           string  `json:"method"`
	Team                             int     `json:"team"`
	Position                         int     `json:"position"`
	ReferenceProbability             float64 `json:"reference_probability"`
	Estimate                         float64 `json:"estimate"`
	StdErr                           float64 `json:"std_error"`
	RelativeSE                       float64 `json:"relative_se"`
	ESS                              float64 `json:"ess"`
	Hits                             int     `json:"hits"`
	ZeroEstimate                     bool    `json:"zero_estimate"`
	MeetsPrecisionGoal               bool    `json:"meets_precision_goal"`
	ProductionSamples                int     `json:"production_samples"`
	ProductionPlainMCEq              float64 `json:"production_plain_mc_equivalent"`
	SearchOverheadPlainMCEq          float64 `json:"search_overhead_plain_mc_equivalent"`
	ProductionDesign                 string  `json:"production_design"`
	CEMInitializationMode            string  `json:"cem_initialization_mode"`
	CEMInitializationSource          string  `json:"cem_initialization_source"`
	CEMShortlistFingerprint          string  `json:"cem_shortlist_fingerprint"`
	SelectedTeam                     int     `json:"selected_team"`
	SelectedPosition                 int     `json:"selected_position"`
	SelectedSnapshotIteration        int     `json:"selected_snapshot_iteration"`
	ScoutWork                        int64   `json:"scout_work"`
	AdaptationWork                   int64   `json:"adaptation_work"`
	EvaluationWork                   int64   `json:"evaluation_work"`
	ProductionWork                   int64   `json:"production_work"`
	TotalWork                        int64   `json:"total_work"`
	TotalWorkLimit                   int64   `json:"total_work_limit"`
	CandidatesAdmitted               int     `json:"candidates_admitted"`
	CEMBatches                       int     `json:"cem_batches"`
	AdaptationExactHitBatches        int     `json:"adaptation_exact_hit_batches"`
	AdaptationNearTargetSnapshots    int     `json:"adaptation_near_target_snapshots"`
	AdaptationExactShortlisted       int     `json:"adaptation_exact_snapshotted"`
	AdaptationExactLE200             int     `json:"adaptation_exact_le_200"`
	AdaptationExactGE400             int     `json:"adaptation_exact_ge_400"`
	AdaptationExactMeanSamples       float64 `json:"adaptation_exact_mean_samples"`
	AdaptationExactStarved           int     `json:"adaptation_exact_starved"`
	AdaptationExactRescued           int     `json:"adaptation_exact_rescued"`
	AdaptationExactStillZero400      int     `json:"adaptation_exact_still_zero_400"`
	AdaptationExactStillZero600      int     `json:"adaptation_exact_still_zero_600"`
	SurrogateOnlyOver400             int     `json:"surrogate_only_over_400"`
	RetainedSnapshots                int     `json:"retained_snapshots"`
	EligibleSnapshots                int     `json:"eligible_snapshots"`
	EvaluatedSnapshots               int     `json:"evaluated_snapshots"`
	EvaluationHits                   int     `json:"evaluation_hits"`
	EvaluationESS                    float64 `json:"evaluation_ess"`
	EvaluationESSPerWork             float64 `json:"evaluation_ess_per_work"`
	EvaluationWorkSavedPlainMCEq     float64 `json:"evaluation_work_saved_plain_mc_equivalent"`
	EvaluationTotalSamples           int     `json:"evaluation_total_samples"`
	EvaluationResultWork             int64   `json:"evaluation_result_work"`
	SelectedEvaluationSamples        int     `json:"selected_evaluation_samples"`
	SelectedEvaluationWork           int64   `json:"selected_evaluation_work"`
	SelectedProductionHits           int     `json:"selected_production_hits"`
	SelectedProductionESS            float64 `json:"selected_production_ess"`
	SelectedProductionRelSE          float64 `json:"selected_production_rel_se"`
	SelectedProductionMaxEventShare  float64 `json:"selected_production_max_event_weight_share"`
	SelectedProductionMeanWeight     float64 `json:"selected_production_mean_weight"`
	ExpectedPlainMCEventsProduction  float64 `json:"expected_plain_mc_events_at_production_work"`
	ExpectedPlainMCEvents100k        float64 `json:"expected_plain_mc_events_at_100k_work"`
	ISBreakEvenGainProduction        float64 `json:"is_ess_gain_vs_production_only_mc"`
	ISBreakEvenGain100k              float64 `json:"is_ess_gain_vs_full_100k_mc"`
	ExactHitCandidatesWithLater      int     `json:"exact_hit_candidates_with_later_adaptation"`
	FirstHitSnapshotsEvaluated       int     `json:"first_hit_snapshots_evaluated"`
	LaterSnapshotsEvaluated          int     `json:"later_snapshots_evaluated"`
	LaterSnapshotBetterESSPerWork    int     `json:"later_snapshot_better_ess_per_work"`
	FirstHitSnapshotBetterESSPerWork int     `json:"first_hit_snapshot_better_ess_per_work"`
	LaterOnlySnapshotEvaluated       int     `json:"later_only_snapshot_evaluated"`
	ISSelectedAfterExactHit          bool    `json:"is_selected_after_exact_hit"`
	ISSelectedWithoutExactHit        bool    `json:"is_selected_without_exact_hit"`
	ISSelectedNearTargetOnly         bool    `json:"is_selected_near_target_only"`
	ISSelectedAfterEvaluationHit     bool    `json:"is_selected_after_evaluation_hit"`
	WeakEvaluationEvidence           bool    `json:"weak_evaluation_evidence"`
}

type rareBenchmarkMethodSummary struct {
	MeanBias              float64 `json:"mean_bias"`
	RMSE                  float64 `json:"rmse"`
	MeanRelativeRMSE      float64 `json:"mean_relative_rmse"`
	MeanAbsoluteError     float64 `json:"mean_absolute_error"`
	ZeroEstimateRate      float64 `json:"zero_estimate_rate"`
	PrecisionGoalRate     float64 `json:"precision_goal_rate"`
	WholeTableLoss        float64 `json:"whole_table_loss"`
	MeanProductionSamples float64 `json:"mean_production_samples"`
	MeanSearchOverhead    float64 `json:"mean_search_overhead_plain_mc_equivalent"`
	RootNormalizedLoss    float64 `json:"root_normalized_squared_loss"`
	MeanReportedSE        float64 `json:"mean_reported_se"`
	EmpiricalSD           float64 `json:"empirical_sd"`
	MeanESS               float64 `json:"mean_ess"`
	MeanTotalWork         float64 `json:"mean_total_work"`
}

type rareBenchmarkCellSummary struct {
	Team                    int     `json:"team"`
	Position                int     `json:"position"`
	ReferenceProbability    float64 `json:"reference_probability"`
	PipelineMean            float64 `json:"pipeline_mean"`
	PlainMCMean             float64 `json:"plain_mc_mean"`
	PipelineBias            float64 `json:"pipeline_bias"`
	PlainMCBias             float64 `json:"plain_mc_bias"`
	PipelineRMSE            float64 `json:"pipeline_rmse"`
	PlainMCRMSE             float64 `json:"plain_mc_rmse"`
	RMSEratio               float64 `json:"rmse_ratio"`
	PipelineMAE             float64 `json:"pipeline_mae"`
	PlainMCMAE              float64 `json:"plain_mc_mae"`
	PipelineRelativeRMSE    float64 `json:"pipeline_relative_rmse"`
	PlainMCRelativeRMSE     float64 `json:"plain_mc_relative_rmse"`
	PipelineZeroRate        float64 `json:"pipeline_zero_rate"`
	PlainMCZeroRate         float64 `json:"plain_mc_zero_rate"`
	PipelineMeanReportedSE  float64 `json:"pipeline_mean_reported_se"`
	PlainMCMeanReportedSE   float64 `json:"plain_mc_mean_reported_se"`
	PipelineEmpiricalSD     float64 `json:"pipeline_empirical_sd"`
	PlainMCEmpiricalSD      float64 `json:"plain_mc_empirical_sd"`
	OldRacingMean           float64 `json:"old_racing_mean"`
	OldRacingBias           float64 `json:"old_racing_bias"`
	OldRacingRMSE           float64 `json:"old_racing_rmse"`
	OldRacingMAE            float64 `json:"old_racing_mae"`
	OldRacingZeroRate       float64 `json:"old_racing_zero_rate"`
	OldRacingMeanReportedSE float64 `json:"old_racing_mean_reported_se"`
	OldRacingEmpiricalSD    float64 `json:"old_racing_empirical_sd"`
}

type rareBenchmarkProbabilityBucket struct {
	Name                         string  `json:"name"`
	Cells                        int     `json:"cells"`
	PipelineRMSE                 float64 `json:"pipeline_rmse"`
	PlainMCRMSE                  float64 `json:"plain_mc_rmse"`
	RMSEratio                    float64 `json:"rmse_ratio"`
	PipelineNormalizedRMSE       float64 `json:"pipeline_normalized_rmse"`
	PlainMCNormalizedRMSE        float64 `json:"plain_mc_normalized_rmse"`
	OldRacingRMSE                float64 `json:"old_racing_rmse"`
	OldRacingNormalizedRMSE      float64 `json:"old_racing_normalized_rmse"`
	OldRacingToEvidenceAwareRMSE float64 `json:"old_racing_to_evidence_aware_rmse_ratio"`
}

type rareBenchmarkReport struct {
	GroupID                                          int                                   `json:"group_id"`
	ReferenceProvenance                              string                                `json:"reference_provenance"`
	Seeds                                            []int64                               `json:"seeds"`
	PipelineScoutSamples                             int                                   `json:"pipeline_scout_samples"`
	BaselineSamples                                  int                                   `json:"baseline_samples"`
	Targets                                          []rareBenchmarkTarget                 `json:"targets"`
	Rows                                             []rareBenchmarkRow                    `json:"rows"`
	Summary                                          map[string]rareBenchmarkMethodSummary `json:"summary"`
	CellSummaries                                    []rareBenchmarkCellSummary            `json:"cell_summaries"`
	ProbabilityBuckets                               []rareBenchmarkProbabilityBucket      `json:"probability_buckets"`
	RMSEPipelineToBaseline                           float64                               `json:"pipeline_to_baseline_rmse_ratio"`
	RMSEDifferenceMean                               float64                               `json:"seed_level_rmse_difference_mean"`
	RMSEDifferenceSE                                 float64                               `json:"seed_level_rmse_difference_standard_error"`
	MAEDifferenceMean                                float64                               `json:"seed_level_mae_difference_mean"`
	MAEDifferenceSE                                  float64                               `json:"seed_level_mae_difference_standard_error"`
	WholeLossDifferenceMean                          float64                               `json:"seed_level_whole_table_loss_difference_mean"`
	WholeLossDifferenceSE                            float64                               `json:"seed_level_whole_table_loss_difference_standard_error"`
	PipelineSelectionRate                            float64                               `json:"pipeline_importance_sampling_selection_rate"`
	PlainSelectionRate                               float64                               `json:"pipeline_plain_mc_selection_rate"`
	OldRacingSelectionRate                           float64                               `json:"old_racing_importance_sampling_selection_rate"`
	RunsWithExactAdaptationHits                      int                                   `json:"runs_with_exact_adaptation_hits"`
	MeanRetainedSnapshots                            float64                               `json:"mean_retained_snapshots"`
	MeanEligibleSnapshots                            float64                               `json:"mean_eligible_snapshots"`
	MeanEvaluatedSnapshots                           float64                               `json:"mean_evaluated_snapshots"`
	ExactHitCandidatesWithLater                      int                                   `json:"exact_hit_candidates_with_later_adaptation"`
	ExactHitCandidatesTotal                          int                                   `json:"exact_hit_candidates_total"`
	FractionExactHitCandidatesWithLater              float64                               `json:"fraction_exact_hit_candidates_with_later_adaptation"`
	FirstHitSnapshotsEvaluated                       int                                   `json:"first_hit_snapshots_evaluated"`
	LaterSnapshotsEvaluated                          int                                   `json:"later_snapshots_evaluated"`
	LaterSnapshotBetterESSPerWork                    int                                   `json:"later_snapshot_better_ess_per_work"`
	FirstHitSnapshotBetterESSPerWork                 int                                   `json:"first_hit_snapshot_better_ess_per_work"`
	LaterOnlySnapshotEvaluated                       int                                   `json:"later_only_snapshot_evaluated"`
	NearTargetOnlyISSelections                       int                                   `json:"near_target_only_is_selections"`
	ISSelectionsAfterExactAdaptation                 int                                   `json:"is_selections_after_exact_adaptation"`
	ISSelectionsWithoutExactAdaptation               int                                   `json:"is_selections_without_exact_adaptation"`
	ISSelectionsAfterEvaluationHit                   int                                   `json:"is_selections_after_evaluation_hit"`
	WeakEvidenceISSelections                         int                                   `json:"weak_evidence_is_selections"`
	MeanEvaluationESSPerWork                         float64                               `json:"mean_evaluation_ess_per_work"`
	MeanEvaluationWorkSavedPlainMCEq                 float64                               `json:"mean_evaluation_work_saved_plain_mc_equivalent"`
	MeanProductionWeight                             float64                               `json:"mean_production_weight"`
	FlaggedMeanWeightRuns                            int                                   `json:"flagged_mean_weight_runs"`
	MeanISGainVs100kMC                               float64                               `json:"mean_is_ess_gain_vs_full_100k_mc"`
	OldRacingToEvidenceAwareRMSE                     float64                               `json:"old_racing_to_evidence_aware_rmse_ratio"`
	OldRacingRMSEDifferenceMean                      float64                               `json:"old_racing_seed_level_rmse_difference_mean"`
	OldRacingRMSEDifferenceSE                        float64                               `json:"old_racing_seed_level_rmse_difference_se"`
	OldRacingStarvedAdaptationExact                  int                                   `json:"old_racing_starved_adaptation_exact"`
	OldAdaptationExactSnapshotsShortlisted           int                                   `json:"old_adaptation_exact_snapshots_shortlisted"`
	EvidenceAwareAdaptationExactSnapshotsShortlisted int                                   `json:"evidence_aware_adaptation_exact_snapshots_shortlisted"`
	EvidenceAwareProtectedAdaptationExact            int                                   `json:"evidence_aware_protected_adaptation_exact"`
	RescuedAdaptationExact                           int                                   `json:"rescued_adaptation_exact"`
	AdaptationExactStillZeroAfter400                 int                                   `json:"adaptation_exact_still_zero_after_400"`
	AdaptationExactStillZeroAfter600                 int                                   `json:"adaptation_exact_still_zero_after_600"`
	SurrogateOnlyOver400Samples                      int                                   `json:"surrogate_only_over_400_samples"`
	OldMeanSamplesPerAdaptationExact                 float64                               `json:"old_mean_samples_per_adaptation_exact_snapshot"`
	NewMeanSamplesPerAdaptationExact                 float64                               `json:"new_mean_samples_per_adaptation_exact_snapshot"`
	WarmStartVerified                                bool                                  `json:"warm_start_verified"`
	ConsoleSummary                                   string                                `json:"console_summary"`
}

func TestRarePositionMatchedComputeBenchmark(t *testing.T) {
	if os.Getenv("RARE_POSITION_BENCHMARK") != "1" {
		t.Skip("set RARE_POSITION_BENCHMARK=1 to run the 20-seed matched-compute benchmark")
	}
	fixturePath := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if fixturePath == "" {
		t.Fatal("RARE_POSITION_BENCHMARK_GROUP_JSON must point to the saved group request JSON")
	}
	data, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatal(err)
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	if input.Id != 16982 {
		t.Fatalf("benchmark fixture group_id=%d, want 16982", input.Id)
	}

	const baselineSamples = 100000
	report := rareBenchmarkReport{GroupID: input.Id,
		ReferenceProvenance:  "User-provided high-run plain-MC values in the PR experiment request",
		PipelineScoutSamples: ScoutIterations, BaselineSamples: baselineSamples,
		Targets: append([]rareBenchmarkTarget(nil), rareBenchmarkTargets...),
		Summary: map[string]rareBenchmarkMethodSummary{}}
	for i := 0; i < 20; i++ {
		report.Seeds = append(report.Seeds, int64(1001+i))
	}
	oldSeed, hadSeed := os.LookupEnv("RARE_POSITION_RANDOM_SEED")
	oldSampling, hadSampling := os.LookupEnv("RARE_POSITION_IMPORTANCE_SAMPLING")
	oldIterations, hadIterations := os.LookupEnv("RARE_POSITION_BENCHMARK_ITERATIONS")
	oldRaceMode, hadRaceMode := os.LookupEnv("RARE_POSITION_CEM_RACING_MODE")
	oldInitMode, hadInitMode := os.LookupEnv("RARE_POSITION_BENCHMARK_CEM_INIT_MODE")
	t.Cleanup(func() {
		restoreBenchmarkEnv(t, "RARE_POSITION_RANDOM_SEED", oldSeed, hadSeed)
		restoreBenchmarkEnv(t, "RARE_POSITION_IMPORTANCE_SAMPLING", oldSampling, hadSampling)
		restoreBenchmarkEnv(t, "RARE_POSITION_BENCHMARK_ITERATIONS", oldIterations, hadIterations)
		restoreBenchmarkEnv(t, "RARE_POSITION_CEM_RACING_MODE", oldRaceMode, hadRaceMode)
		restoreBenchmarkEnv(t, "RARE_POSITION_BENCHMARK_CEM_INIT_MODE", oldInitMode, hadInitMode)
	})
	_ = os.Unsetenv("RARE_POSITION_BENCHMARK_ITERATIONS")

	var seedLevelRMSEDifferences []float64
	pipelineSelected, pipelinePlain := 0, 0
	runsWithExact := 0
	var retainedTotal, eligibleTotal, evaluatedTotal float64
	var comparison RarePositionSearchDiagnostics
	oldRacingSelected := 0
	oldRacingDiagnosticsBySeed := make(map[int64]*RarePositionSearchDiagnostics)
	newRacingDiagnosticsBySeed := make(map[int64]*RarePositionSearchDiagnostics)
	var oldNewSeedRMSEDifferences []float64
	unplayedGames := 0
	for _, game := range input.Games {
		if !game.Played {
			unplayedGames++
		}
	}
	plainWorkPerSample := estimateSeasonWork(unplayedGames, 1, len(input.Team_groups))
	totalWorkLimit := calculateMaxRareWork(unplayedGames, len(input.Team_groups))
	for _, seed := range report.Seeds {
		pipelineEstimates, diagnostics := runRareBenchmarkPipelineArm(t, input, seed, "")
		newRacingDiagnosticsBySeed[seed] = diagnostics
		oldPipelineEstimates, oldDiagnostics := runRareBenchmarkPipelineArm(t, input, seed, "legacy")
		oldRacingDiagnosticsBySeed[seed] = oldDiagnostics
		if diagnostics.CEMShortlistFingerprint != oldDiagnostics.CEMShortlistFingerprint {
			t.Fatalf("seed %d CEM shortlist differs between racing arms: evidence=%s old=%s",
				seed, diagnostics.CEMShortlistFingerprint, oldDiagnostics.CEMShortlistFingerprint)
		}
		isSelected := false
		for _, positions := range pipelineEstimates {
			for _, estimate := range positions {
				isSelected = isSelected || estimate.Design == "importance_sampling"
			}
		}
		if isSelected {
			pipelineSelected++
		} else {
			pipelinePlain++
		}
		oldSelected := false
		for _, positions := range oldPipelineEstimates {
			for _, estimate := range positions {
				oldSelected = oldSelected || estimate.Design == "importance_sampling"
			}
		}
		if oldSelected {
			oldRacingSelected++
		}
		if diagnostics != nil {
			if diagnostics.TotalWork < 0 || diagnostics.TotalWork > diagnostics.TotalWorkLimit {
				t.Fatalf("seed %d pipeline work %d exceeds limit %d", seed, diagnostics.TotalWork, diagnostics.TotalWorkLimit)
			}
			if diagnostics.AdaptationExactHitBatches > 0 {
				runsWithExact++
			}
			retainedTotal += float64(diagnostics.RetainedSnapshots)
			eligibleTotal += float64(diagnostics.EligibleSnapshots)
			evaluatedTotal += float64(diagnostics.EvaluatedSnapshots)
			comparison.ExactHitCandidatesWithLater += diagnostics.ExactHitCandidatesWithLater
			comparison.ExactHitCandidates += diagnostics.ExactHitCandidates
			comparison.FirstHitSnapshotEvaluated += diagnostics.FirstHitSnapshotEvaluated
			comparison.LaterSnapshotEvaluated += diagnostics.LaterSnapshotEvaluated
			comparison.LaterBetterESSPerWork += diagnostics.LaterBetterESSPerWork
			comparison.FirstHitBetterESSPerWork += diagnostics.FirstHitBetterESSPerWork
			comparison.LaterOnlySnapshotEvaluated += diagnostics.LaterOnlySnapshotEvaluated
		}

		baselineSeed := deriveRarePositionSeed(seed, "plain-mc-baseline")
		_ = os.Setenv("RARE_POSITION_RANDOM_SEED", strconv.FormatInt(baselineSeed, 10))
		_ = os.Unsetenv("RARE_POSITION_IMPORTANCE_SAMPLING")
		_ = os.Setenv("RARE_POSITION_BENCHMARK_ITERATIONS", strconv.Itoa(baselineSamples))
		baselineResult := cloneGroupForBenchmark(input).calculate_odds()
		baselineOdds, ok := baselineResult["team_odds"].(map[int]*TeamOdds)
		if !ok {
			t.Fatalf("seed %d returned no typed baseline odds", seed)
		}

		seedErrors := map[string]float64{}
		for _, target := range rareBenchmarkTargets {
			if got := pipelineEstimates[target.Team][target.Position]; got.Available {
				productionEquivalent := float64(got.WorkSpent) / float64(plainWorkPerSample)
				searchOverheadEquivalent := float64(totalWorkLimit-got.WorkSpent) / float64(plainWorkPerSample)
				if got.SearchDiagnostics != nil {
					searchOverheadEquivalent = got.SearchDiagnostics.SearchOverheadPlainMCEq
				}
				if got.WorkSpent < 0 || got.WorkSpent > totalWorkLimit {
					t.Fatalf("seed %d pipeline production work %d outside [0,%d]", seed, got.WorkSpent, totalWorkLimit)
				}
				row := benchmarkRow(seed, "adaptive_pipeline", target, got.Probability,
					got.StdErr, relativeSEValue(got.RelativeSE), got.ESS, got.Hits, got.MeetsPrecisionGoal,
					got.Samples, productionEquivalent, got.Design, searchOverheadEquivalent, got.SearchDiagnostics)
				row.ExpectedPlainMCEventsProduction = target.Reference * productionEquivalent
				row.ExpectedPlainMCEvents100k = target.Reference * float64(baselineSamples)
				row.ISBreakEvenGainProduction = safeRatio(got.ESS, row.ExpectedPlainMCEventsProduction)
				row.ISBreakEvenGain100k = safeRatio(got.ESS, row.ExpectedPlainMCEvents100k)
				if got.SearchDiagnostics != nil {
					row.SelectedProductionHits = got.SearchDiagnostics.SelectedProductionHits
					row.SelectedProductionESS = got.SearchDiagnostics.SelectedProductionESS
					row.SelectedProductionRelSE = got.SearchDiagnostics.SelectedProductionRelSE
					row.SelectedProductionMaxEventShare = got.SearchDiagnostics.SelectedProductionMaxEventWeightShare
					row.SelectedProductionMeanWeight = got.SearchDiagnostics.SelectedProductionMeanWeight
				}
				report.Rows = append(report.Rows, row)
				seedErrors["adaptive_pipeline"] += square(got.Probability - target.Reference)
			} else {
				row := benchmarkRow(seed, "adaptive_pipeline", target,
					0, 0, math.Inf(1), 0, 0, false, 0, 0, "", 0, diagnostics)
				report.Rows = append(report.Rows, row)
				seedErrors["adaptive_pipeline"] += square(target.Reference)
			}
			if got := oldPipelineEstimates[target.Team][target.Position]; got.Available {
				productionEquivalent := float64(got.WorkSpent) / float64(plainWorkPerSample)
				searchOverheadEquivalent := oldDiagnostics.SearchOverheadPlainMCEq
				row := benchmarkRow(seed, "old_racing", target, got.Probability,
					got.StdErr, relativeSEValue(got.RelativeSE), got.ESS, got.Hits, got.MeetsPrecisionGoal,
					got.Samples, productionEquivalent, got.Design, searchOverheadEquivalent, oldDiagnostics)
				row.ExpectedPlainMCEventsProduction = target.Reference * productionEquivalent
				row.ExpectedPlainMCEvents100k = target.Reference * float64(baselineSamples)
				row.ISBreakEvenGainProduction = safeRatio(got.ESS, row.ExpectedPlainMCEventsProduction)
				row.ISBreakEvenGain100k = safeRatio(got.ESS, row.ExpectedPlainMCEvents100k)
				row.SelectedProductionHits = oldDiagnostics.SelectedProductionHits
				row.SelectedProductionESS = oldDiagnostics.SelectedProductionESS
				row.SelectedProductionRelSE = oldDiagnostics.SelectedProductionRelSE
				row.SelectedProductionMaxEventShare = oldDiagnostics.SelectedProductionMaxEventWeightShare
				row.SelectedProductionMeanWeight = oldDiagnostics.SelectedProductionMeanWeight
				report.Rows = append(report.Rows, row)
				seedErrors["old_racing"] += square(got.Probability - target.Reference)
			} else {
				report.Rows = append(report.Rows, benchmarkRow(seed, "old_racing", target,
					0, 0, math.Inf(1), 0, 0, false, 0, 0, "", oldDiagnostics.SearchOverheadPlainMCEq, oldDiagnostics))
				seedErrors["old_racing"] += square(target.Reference)
			}
			odds := baselineOdds[target.Team]
			if odds == nil || target.Position >= len(odds.Pos) {
				t.Fatalf("baseline missing target team=%d position=%d", target.Team, target.Position)
			}
			p := odds.Pos[target.Position] / 100
			se := math.Sqrt(p * (1 - p) / baselineSamples)
			relSE := math.Inf(1)
			if p > 0 {
				relSE = se / p
			}
			baselineWork := int64(baselineSamples) * plainWorkPerSample
			if baselineWork > totalWorkLimit {
				t.Fatalf("seed %d baseline work %d exceeds matched work limit %d", seed, baselineWork, totalWorkLimit)
			}
			baselineRow := benchmarkRow(seed, "plain_mc_100k", target, p, se,
				relSE, float64(baselineSamples)*p, int(math.Round(p*baselineSamples)),
				estimateMeetsPrecisionGoal(float64(baselineSamples)*p, relSE), baselineSamples,
				float64(baselineSamples), "plain_mc", 0, nil)
			baselineRow.TotalWork, baselineRow.TotalWorkLimit = baselineWork, totalWorkLimit
			baselineRow.ProductionWork = baselineWork
			baselineRow.ExpectedPlainMCEventsProduction = target.Reference * float64(baselineSamples)
			report.Rows = append(report.Rows, baselineRow)
			seedErrors["plain_mc_100k"] += square(p - target.Reference)
		}
		seedLevelRMSEDifferences = append(seedLevelRMSEDifferences,
			math.Sqrt(seedErrors["adaptive_pipeline"]/float64(len(rareBenchmarkTargets)))-
				math.Sqrt(seedErrors["plain_mc_100k"]/float64(len(rareBenchmarkTargets))))
		oldNewSeedRMSEDifferences = append(oldNewSeedRMSEDifferences,
			math.Sqrt(seedErrors["old_racing"]/float64(len(rareBenchmarkTargets)))-
				math.Sqrt(seedErrors["adaptive_pipeline"]/float64(len(rareBenchmarkTargets))))
	}
	report.Summary["adaptive_pipeline"] = summarizeRareBenchmarkRows(report.Rows, "adaptive_pipeline", rareBenchmarkTargets)
	report.Summary["evidence_aware_racing"] = report.Summary["adaptive_pipeline"]
	report.Summary["old_racing"] = summarizeRareBenchmarkRows(report.Rows, "old_racing", rareBenchmarkTargets)
	report.Summary["plain_mc_100k"] = summarizeRareBenchmarkRows(report.Rows, "plain_mc_100k", rareBenchmarkTargets)
	report.CellSummaries = summarizeRareBenchmarkCells(report.Rows, rareBenchmarkTargets)
	report.ProbabilityBuckets = summarizeRareBenchmarkBuckets(report.CellSummaries)
	populateRareBenchmarkEconomics(&report)
	pipelineSummary, baselineSummary := report.Summary["adaptive_pipeline"], report.Summary["plain_mc_100k"]
	oldRacingSummary := report.Summary["old_racing"]
	if baselineSummary.RMSE > 0 {
		report.RMSEPipelineToBaseline = pipelineSummary.RMSE / baselineSummary.RMSE
	}
	if pipelineSummary.RMSE > 0 {
		report.OldRacingToEvidenceAwareRMSE = oldRacingSummary.RMSE / pipelineSummary.RMSE
	}
	report.OldRacingRMSEDifferenceMean, report.OldRacingRMSEDifferenceSE = meanAndSE(oldNewSeedRMSEDifferences)
	report.PipelineSelectionRate = float64(pipelineSelected) / float64(len(report.Seeds))
	report.PlainSelectionRate = float64(pipelinePlain) / float64(len(report.Seeds))
	report.OldRacingSelectionRate = float64(oldRacingSelected) / float64(len(report.Seeds))
	populateRaceComparison(&report, oldRacingDiagnosticsBySeed, newRacingDiagnosticsBySeed)
	report.RunsWithExactAdaptationHits = runsWithExact
	report.MeanRetainedSnapshots = retainedTotal / float64(len(report.Seeds))
	report.MeanEligibleSnapshots = eligibleTotal / float64(len(report.Seeds))
	report.MeanEvaluatedSnapshots = evaluatedTotal / float64(len(report.Seeds))
	report.ExactHitCandidatesWithLater = comparison.ExactHitCandidatesWithLater
	report.ExactHitCandidatesTotal = comparison.ExactHitCandidates
	if comparison.ExactHitCandidates > 0 {
		report.FractionExactHitCandidatesWithLater = float64(comparison.ExactHitCandidatesWithLater) / float64(comparison.ExactHitCandidates)
	}
	report.FirstHitSnapshotsEvaluated = comparison.FirstHitSnapshotEvaluated
	report.LaterSnapshotsEvaluated = comparison.LaterSnapshotEvaluated
	report.LaterSnapshotBetterESSPerWork = comparison.LaterBetterESSPerWork
	report.FirstHitSnapshotBetterESSPerWork = comparison.FirstHitBetterESSPerWork
	report.LaterOnlySnapshotEvaluated = comparison.LaterOnlySnapshotEvaluated
	report.RMSEDifferenceMean, report.RMSEDifferenceSE = meanAndSE(seedLevelRMSEDifferences)
	maeDifferences, lossDifferences := seedLevelMetricDifferences(report.Rows)
	report.MAEDifferenceMean, report.MAEDifferenceSE = meanAndSE(maeDifferences)
	report.WholeLossDifferenceMean, report.WholeLossDifferenceSE = meanAndSE(lossDifferences)
	report.ConsoleSummary = formatRareBenchmarkConsoleSummary(report)
	encoded, err := json.MarshalIndent(report, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	outputPath := os.Getenv("RARE_POSITION_BENCHMARK_OUTPUT")
	if outputPath == "" {
		outputPath = "/tmp/rare-position-benchmark.json"
	}
	if err := os.WriteFile(outputPath, encoded, 0600); err != nil {
		t.Fatal(err)
	}
	if err := writeRareBenchmarkCSV(strings.TrimSuffix(outputPath, filepath.Ext(outputPath))+".csv", report.Rows); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(strings.TrimSuffix(outputPath, filepath.Ext(outputPath))+"-summary.txt", []byte(report.ConsoleSummary+"\n"), 0600); err != nil {
		t.Fatal(err)
	}
	fmt.Printf("%s\nreport: %s\ncsv: %s\n", report.ConsoleSummary, outputPath,
		strings.TrimSuffix(outputPath, filepath.Ext(outputPath))+".csv")
}

func cloneGroupForBenchmark(group GroupType) *GroupType {
	data, _ := json.Marshal(group)
	var copy GroupType
	_ = json.Unmarshal(data, &copy)
	return &copy
}

func runRareBenchmarkPipelineArm(t *testing.T, input GroupType, seed int64, raceMode string) (map[int]map[int]ProductionEstimate, *RarePositionSearchDiagnostics) {
	t.Helper()
	if err := os.Setenv("RARE_POSITION_RANDOM_SEED", strconv.FormatInt(seed, 10)); err != nil {
		t.Fatal(err)
	}
	if err := os.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1"); err != nil {
		t.Fatal(err)
	}
	if err := os.Setenv("RARE_POSITION_BENCHMARK_ITERATIONS", strconv.Itoa(ScoutIterations)); err != nil {
		t.Fatal(err)
	}
	if err := os.Setenv("RARE_POSITION_BENCHMARK_CEM_INIT_MODE", "standings_directed"); err != nil {
		t.Fatal(err)
	}
	if raceMode == "" {
		_ = os.Unsetenv("RARE_POSITION_CEM_RACING_MODE")
	} else if err := os.Setenv("RARE_POSITION_CEM_RACING_MODE", raceMode); err != nil {
		t.Fatal(err)
	}
	result := cloneGroupForBenchmark(input).calculate_odds()
	estimates, ok := result["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	if !ok {
		t.Fatalf("seed %d race_mode=%q returned no typed rare-position estimates", seed, raceMode)
	}
	var diagnostics *RarePositionSearchDiagnostics
	for _, positions := range estimates {
		for _, estimate := range positions {
			if estimate.SearchDiagnostics != nil {
				diagnostics = estimate.SearchDiagnostics
				break
			}
		}
		if diagnostics != nil {
			break
		}
	}
	if diagnostics == nil {
		t.Fatalf("seed %d race_mode=%q returned no rare-position diagnostics", seed, raceMode)
	}
	if diagnostics.CEMInitializationMode != "standings_directed" || diagnostics.CEMInitializationSource != "benchmark_override" {
		t.Fatalf("seed %d race_mode=%q did not use requested standings-directed init: mode=%s source=%s",
			seed, raceMode, diagnostics.CEMInitializationMode, diagnostics.CEMInitializationSource)
	}
	return estimates, diagnostics
}

func populateRaceComparison(report *rareBenchmarkReport, oldBySeed, newBySeed map[int64]*RarePositionSearchDiagnostics) {
	report.WarmStartVerified = true
	oldExactCount, newExactCount := 0, 0
	oldSamples, newSamples := 0.0, 0.0
	for _, seed := range report.Seeds {
		oldDiag := oldBySeed[seed]
		newDiag := newBySeed[seed]
		if oldDiag == nil || newDiag == nil {
			continue
		}
		if oldDiag.CEMShortlistFingerprint != newDiag.CEMShortlistFingerprint {
			panic(fmt.Sprintf("seed %d racing arms used different CEM shortlists", seed))
		}
		if oldDiag.CEMInitializationMode != "standings_directed" || newDiag.CEMInitializationMode != "standings_directed" {
			report.WarmStartVerified = false
		}
		oldExactCount += oldDiag.AdaptationExactSnapshotsShortlisted
		newExactCount += newDiag.AdaptationExactSnapshotsShortlisted
		report.OldRacingStarvedAdaptationExact += oldDiag.AdaptationExactStarved
		report.EvidenceAwareProtectedAdaptationExact += newDiag.AdaptationExactSnapshotsGE400
		report.RescuedAdaptationExact += newDiag.AdaptationExactRescued
		report.AdaptationExactStillZeroAfter400 += newDiag.AdaptationExactStillZeroAfter400
		report.AdaptationExactStillZeroAfter600 += newDiag.AdaptationExactStillZeroAfter600
		report.SurrogateOnlyOver400Samples += newDiag.SurrogateOnlySnapshotsOver400
		oldSamples += oldDiag.AdaptationExactMeanEvaluationSamples * float64(oldDiag.AdaptationExactSnapshotsShortlisted)
		newSamples += newDiag.AdaptationExactMeanEvaluationSamples * float64(newDiag.AdaptationExactSnapshotsShortlisted)
	}
	report.OldAdaptationExactSnapshotsShortlisted = oldExactCount
	report.EvidenceAwareAdaptationExactSnapshotsShortlisted = newExactCount
	if oldExactCount > 0 {
		report.OldMeanSamplesPerAdaptationExact = oldSamples / float64(oldExactCount)
	}
	if newExactCount > 0 {
		report.NewMeanSamplesPerAdaptationExact = newSamples / float64(newExactCount)
	}
}

func restoreBenchmarkEnv(t *testing.T, key, value string, existed bool) {
	t.Helper()
	if existed {
		if err := os.Setenv(key, value); err != nil {
			t.Error(err)
		}
	} else if err := os.Unsetenv(key); err != nil {
		t.Error(err)
	}
}

func benchmarkRow(seed int64, method string, target rareBenchmarkTarget, estimate, stdErr, relSE, ess float64,
	hits int, meets bool, samples int, productionEquivalent float64, design string, searchOverhead float64,
	diagnostics *RarePositionSearchDiagnostics) rareBenchmarkRow {
	zero := estimate == 0
	row := rareBenchmarkRow{Seed: seed, Method: method, Team: target.Team, Position: target.Position,
		ReferenceProbability: target.Reference, Estimate: estimate, StdErr: stdErr, RelativeSE: relSE,
		ESS: ess, Hits: hits, ZeroEstimate: zero, MeetsPrecisionGoal: meets,
		ProductionSamples: samples, ProductionPlainMCEq: productionEquivalent,
		SearchOverheadPlainMCEq: searchOverhead, ProductionDesign: design}
	if diagnostics != nil {
		row.CEMInitializationMode, row.CEMInitializationSource = diagnostics.CEMInitializationMode, diagnostics.CEMInitializationSource
		row.CEMShortlistFingerprint = diagnostics.CEMShortlistFingerprint
		row.SelectedTeam, row.SelectedPosition = diagnostics.SelectedTeam, diagnostics.SelectedPosition
		row.SelectedSnapshotIteration = diagnostics.SelectedSnapshotIteration
		row.AdaptationExactShortlisted = diagnostics.AdaptationExactSnapshotsShortlisted
		row.AdaptationExactLE200, row.AdaptationExactGE400 = diagnostics.AdaptationExactSnapshotsLE200, diagnostics.AdaptationExactSnapshotsGE400
		row.AdaptationExactMeanSamples = diagnostics.AdaptationExactMeanEvaluationSamples
		row.AdaptationExactStarved, row.AdaptationExactRescued = diagnostics.AdaptationExactStarved, diagnostics.AdaptationExactRescued
		row.AdaptationExactStillZero400, row.AdaptationExactStillZero600 = diagnostics.AdaptationExactStillZeroAfter400, diagnostics.AdaptationExactStillZeroAfter600
		row.SurrogateOnlyOver400 = diagnostics.SurrogateOnlySnapshotsOver400
		row.ScoutWork, row.AdaptationWork = diagnostics.ScoutWork, diagnostics.AdaptationWork
		row.EvaluationWork, row.ProductionWork = diagnostics.EvaluationWork, diagnostics.ProductionWork
		row.TotalWork, row.TotalWorkLimit = diagnostics.TotalWork, diagnostics.TotalWorkLimit
		row.CandidatesAdmitted, row.CEMBatches = diagnostics.CandidatesAdmitted, diagnostics.CEMBatches
		row.AdaptationExactHitBatches = diagnostics.AdaptationExactHitBatches
		row.AdaptationNearTargetSnapshots = diagnostics.NearTargetSnapshots
		row.RetainedSnapshots, row.EligibleSnapshots = diagnostics.RetainedSnapshots, diagnostics.EligibleSnapshots
		row.EvaluatedSnapshots, row.EvaluationHits = diagnostics.EvaluatedSnapshots, diagnostics.EvaluationHits
		row.EvaluationESS, row.EvaluationESSPerWork = diagnostics.EvaluationESS, diagnostics.EvaluationESSPerWork
		row.EvaluationWorkSavedPlainMCEq = diagnostics.EvaluationWorkSavedPlainMCEq
		row.EvaluationTotalSamples, row.EvaluationResultWork = diagnostics.EvaluationTotalSamples, diagnostics.EvaluationResultWork
		row.SelectedEvaluationSamples, row.SelectedEvaluationWork = diagnostics.SelectedEvaluationSamples, diagnostics.SelectedEvaluationWork
		row.SelectedProductionHits = diagnostics.SelectedProductionHits
		row.SelectedProductionESS = diagnostics.SelectedProductionESS
		row.SelectedProductionRelSE = diagnostics.SelectedProductionRelSE
		row.SelectedProductionMaxEventShare = diagnostics.SelectedProductionMaxEventWeightShare
		row.SelectedProductionMeanWeight = diagnostics.SelectedProductionMeanWeight
		row.ISSelectedAfterExactHit = diagnostics.ISSelectedAfterExactHit
		row.ISSelectedWithoutExactHit = diagnostics.ISSelectedWithoutExactHit
		row.ISSelectedNearTargetOnly = diagnostics.ISSelectedNearTargetOnly
		row.ISSelectedAfterEvaluationHit = diagnostics.ISSelectedAfterEvaluatedExactHit
		row.WeakEvaluationEvidence = diagnostics.WeakEvaluationEvidence
		row.ExactHitCandidatesWithLater = diagnostics.ExactHitCandidatesWithLater
		row.FirstHitSnapshotsEvaluated = diagnostics.FirstHitSnapshotEvaluated
		row.LaterSnapshotsEvaluated = diagnostics.LaterSnapshotEvaluated
		row.LaterSnapshotBetterESSPerWork = diagnostics.LaterBetterESSPerWork
		row.FirstHitSnapshotBetterESSPerWork = diagnostics.FirstHitBetterESSPerWork
		row.LaterOnlySnapshotEvaluated = diagnostics.LaterOnlySnapshotEvaluated
	}
	return row
}

func summarizeRareBenchmarkRows(rows []rareBenchmarkRow, method string, targets []rareBenchmarkTarget) rareBenchmarkMethodSummary {
	summary := rareBenchmarkMethodSummary{}
	perCellSq := make(map[[2]int]float64)
	perCellN := make(map[[2]int]int)
	for _, row := range rows {
		if row.Method != method {
			continue
		}
		errorValue := row.Estimate - row.ReferenceProbability
		summary.MeanBias += errorValue
		summary.MeanAbsoluteError += math.Abs(errorValue)
		summary.RMSE += errorValue * errorValue
		summary.WholeTableLoss += square(errorValue / math.Max(row.ReferenceProbability, MinInterestingProbability))
		if row.ZeroEstimate {
			summary.ZeroEstimateRate++
		}
		if row.MeetsPrecisionGoal {
			summary.PrecisionGoalRate++
		}
		summary.MeanProductionSamples += float64(row.ProductionSamples)
		summary.MeanSearchOverhead += row.SearchOverheadPlainMCEq
		summary.MeanReportedSE += row.StdErr
		summary.MeanESS += row.ESS
		summary.MeanTotalWork += float64(row.TotalWork)
		key := [2]int{row.Team, row.Position}
		perCellSq[key] += errorValue * errorValue
		perCellN[key]++
	}
	n := 0.0
	for _, row := range rows {
		if row.Method == method {
			n++
		}
	}
	if n == 0 {
		return summary
	}
	summary.MeanBias /= n
	summary.MeanAbsoluteError /= n
	summary.RMSE = math.Sqrt(summary.RMSE / n)
	summary.ZeroEstimateRate /= n
	summary.PrecisionGoalRate /= n
	summary.WholeTableLoss /= n
	summary.MeanProductionSamples /= n
	summary.MeanSearchOverhead /= n
	summary.MeanReportedSE /= n
	summary.MeanESS /= n
	summary.MeanTotalWork /= n
	summary.RootNormalizedLoss = math.Sqrt(summary.WholeTableLoss)
	for _, target := range targets {
		key := [2]int{target.Team, target.Position}
		if perCellN[key] > 0 && target.Reference > 0 {
			summary.MeanRelativeRMSE += math.Sqrt(perCellSq[key]/float64(perCellN[key])) / target.Reference
		}
	}
	summary.MeanRelativeRMSE /= float64(len(targets))
	if n > 1 {
		var variance float64
		for _, row := range rows {
			if row.Method == method {
				variance += square(row.Estimate - summary.MeanBias - row.ReferenceProbability)
			}
		}
		summary.EmpiricalSD = math.Sqrt(variance / (n - 1))
	}
	return summary
}

func meanAndSE(values []float64) (mean, standardError float64) {
	if len(values) == 0 {
		return 0, 0
	}
	for _, value := range values {
		mean += value
	}
	mean /= float64(len(values))
	if len(values) > 1 {
		variance := 0.0
		for _, value := range values {
			variance += square(value - mean)
		}
		standardError = math.Sqrt(variance / float64(len(values)-1) / float64(len(values)))
	}
	return mean, standardError
}

func seedLevelMetricDifferences(rows []rareBenchmarkRow) (maeDiff, lossDiff []float64) {
	type totals struct {
		mae, loss float64
		count     int
	}
	bySeed := make(map[int64]map[string]totals)
	for _, row := range rows {
		methods := bySeed[row.Seed]
		if methods == nil {
			methods = make(map[string]totals)
			bySeed[row.Seed] = methods
		}
		value := methods[row.Method]
		errorValue := row.Estimate - row.ReferenceProbability
		value.mae += math.Abs(errorValue)
		value.loss += square(errorValue / math.Max(row.ReferenceProbability, MinInterestingProbability))
		value.count++
		methods[row.Method] = value
	}
	for _, methods := range bySeed {
		pipeline, pipelineOK := methods["adaptive_pipeline"]
		baseline, baselineOK := methods["plain_mc_100k"]
		if !pipelineOK || !baselineOK || pipeline.count == 0 || baseline.count == 0 {
			continue
		}
		maeDiff = append(maeDiff, pipeline.mae/float64(pipeline.count)-baseline.mae/float64(baseline.count))
		lossDiff = append(lossDiff, pipeline.loss/float64(pipeline.count)-baseline.loss/float64(baseline.count))
	}
	return maeDiff, lossDiff
}

func square(value float64) float64 { return value * value }

func safeRatio(numerator, denominator float64) float64 {
	if denominator <= 0 || math.IsNaN(denominator) || math.IsInf(denominator, 0) {
		return 0
	}
	return numerator / denominator
}

func summarizeRareBenchmarkCells(rows []rareBenchmarkRow, targets []rareBenchmarkTarget) []rareBenchmarkCellSummary {
	results := make([]rareBenchmarkCellSummary, 0, len(targets))
	for _, target := range targets {
		cell := rareBenchmarkCellSummary{Team: target.Team, Position: target.Position,
			ReferenceProbability: target.Reference}
		for _, method := range []string{"adaptive_pipeline", "old_racing", "plain_mc_100k"} {
			var estimates, errors, reportedSE []float64
			zeros := 0
			for _, row := range rows {
				if row.Team != target.Team || row.Position != target.Position || row.Method != method {
					continue
				}
				estimates = append(estimates, row.Estimate)
				errors = append(errors, row.Estimate-target.Reference)
				reportedSE = append(reportedSE, row.StdErr)
				if row.ZeroEstimate {
					zeros++
				}
			}
			if len(estimates) == 0 {
				continue
			}
			bias, mae, mse, seMean := 0.0, 0.0, 0.0, 0.0
			for i, errValue := range errors {
				bias += errValue
				mae += math.Abs(errValue)
				mse += square(errValue)
				seMean += reportedSE[i]
			}
			bias /= float64(len(estimates))
			mae /= float64(len(estimates))
			rmse := math.Sqrt(mse / float64(len(estimates)))
			meanEstimate := 0.0
			for _, estimate := range estimates {
				meanEstimate += estimate
			}
			meanEstimate /= float64(len(estimates))
			empiricalSD := 0.0
			if len(estimates) > 1 {
				for _, estimate := range estimates {
					empiricalSD += square(estimate - meanEstimate)
				}
				empiricalSD = math.Sqrt(empiricalSD / float64(len(estimates)-1))
			}
			if method == "adaptive_pipeline" {
				cell.PipelineMean, cell.PipelineBias, cell.PipelineRMSE = meanEstimate, bias, rmse
				cell.PipelineMAE, cell.PipelineZeroRate = mae, float64(zeros)/float64(len(estimates))
				cell.PipelineMeanReportedSE = seMean / float64(len(estimates))
				cell.PipelineEmpiricalSD = empiricalSD
				cell.PipelineRelativeRMSE = safeRatio(rmse, target.Reference)
			} else if method == "old_racing" {
				cell.OldRacingMean, cell.OldRacingBias, cell.OldRacingRMSE = meanEstimate, bias, rmse
				cell.OldRacingMAE, cell.OldRacingZeroRate = mae, float64(zeros)/float64(len(estimates))
				cell.OldRacingMeanReportedSE = seMean / float64(len(estimates))
				cell.OldRacingEmpiricalSD = empiricalSD
			} else {
				cell.PlainMCMean, cell.PlainMCBias, cell.PlainMCRMSE = meanEstimate, bias, rmse
				cell.PlainMCMAE, cell.PlainMCZeroRate = mae, float64(zeros)/float64(len(estimates))
				cell.PlainMCMeanReportedSE = seMean / float64(len(estimates))
				cell.PlainMCEmpiricalSD = empiricalSD
				cell.PlainMCRelativeRMSE = safeRatio(rmse, target.Reference)
			}
		}
		cell.RMSEratio = safeRatio(cell.PipelineRMSE, cell.PlainMCRMSE)
		results = append(results, cell)
	}
	return results
}

func summarizeRareBenchmarkBuckets(cells []rareBenchmarkCellSummary) []rareBenchmarkProbabilityBucket {
	buckets := []rareBenchmarkProbabilityBucket{{Name: "below_1e-5"}, {Name: "1e-5_to_1e-4"}, {Name: "at_least_1e-4"}}
	for _, cell := range cells {
		index := 2
		if cell.ReferenceProbability < 1e-5 {
			index = 0
		} else if cell.ReferenceProbability < 1e-4 {
			index = 1
		}
		bucket := &buckets[index]
		bucket.Cells++
		bucket.PipelineRMSE += square(cell.PipelineRMSE)
		bucket.PlainMCRMSE += square(cell.PlainMCRMSE)
		bucket.PipelineNormalizedRMSE += square(cell.PipelineRelativeRMSE)
		bucket.PlainMCNormalizedRMSE += square(cell.PlainMCRelativeRMSE)
		bucket.OldRacingRMSE += square(cell.OldRacingRMSE)
		bucket.OldRacingNormalizedRMSE += square(safeRatio(cell.OldRacingRMSE, cell.ReferenceProbability))
	}
	for i := range buckets {
		bucket := &buckets[i]
		if bucket.Cells == 0 {
			continue
		}
		bucket.PipelineRMSE = math.Sqrt(bucket.PipelineRMSE / float64(bucket.Cells))
		bucket.PlainMCRMSE = math.Sqrt(bucket.PlainMCRMSE / float64(bucket.Cells))
		bucket.RMSEratio = safeRatio(bucket.PipelineRMSE, bucket.PlainMCRMSE)
		bucket.PipelineNormalizedRMSE = math.Sqrt(bucket.PipelineNormalizedRMSE / float64(bucket.Cells))
		bucket.PlainMCNormalizedRMSE = math.Sqrt(bucket.PlainMCNormalizedRMSE / float64(bucket.Cells))
		bucket.OldRacingRMSE = math.Sqrt(bucket.OldRacingRMSE / float64(bucket.Cells))
		bucket.OldRacingNormalizedRMSE = math.Sqrt(bucket.OldRacingNormalizedRMSE / float64(bucket.Cells))
		bucket.OldRacingToEvidenceAwareRMSE = safeRatio(bucket.OldRacingRMSE, bucket.PipelineRMSE)
	}
	return buckets
}

func populateRareBenchmarkEconomics(report *rareBenchmarkReport) {
	perSeed := make(map[int64]rareBenchmarkRow)
	for _, row := range report.Rows {
		if row.Method == "adaptive_pipeline" {
			if _, exists := perSeed[row.Seed]; !exists {
				perSeed[row.Seed] = row
			}
		}
	}
	if len(perSeed) == 0 {
		return
	}
	var essPerWork, saved, meanWeight, gain float64
	var weightCount int
	for _, row := range perSeed {
		if row.ISSelectedNearTargetOnly {
			report.NearTargetOnlyISSelections++
		}
		if row.ISSelectedAfterExactHit {
			report.ISSelectionsAfterExactAdaptation++
		}
		if row.ISSelectedWithoutExactHit {
			report.ISSelectionsWithoutExactAdaptation++
		}
		if row.ISSelectedAfterEvaluationHit {
			report.ISSelectionsAfterEvaluationHit++
		}
		if row.WeakEvaluationEvidence {
			report.WeakEvidenceISSelections++
		}
		essPerWork += row.EvaluationESSPerWork
		saved += row.EvaluationWorkSavedPlainMCEq
		meanWeight += row.SelectedProductionMeanWeight
		if row.ProductionDesign == "importance_sampling" {
			weightCount++
			gain += row.ISBreakEvenGain100k
			if row.SelectedProductionMeanWeight > 20+1e-9 {
				report.FlaggedMeanWeightRuns++
			}
		}
	}
	n := float64(len(perSeed))
	report.MeanEvaluationESSPerWork, report.MeanEvaluationWorkSavedPlainMCEq = essPerWork/n, saved/n
	if weightCount > 0 {
		report.MeanProductionWeight = meanWeight / float64(weightCount)
		report.MeanISGainVs100kMC = gain / float64(weightCount)
	}
}

func formatRareBenchmarkConsoleSummary(report rareBenchmarkReport) string {
	pipeline, baseline, old := report.Summary["adaptive_pipeline"], report.Summary["plain_mc_100k"], report.Summary["old_racing"]
	return fmt.Sprintf("rare-position racing benchmark: group=%d seeds=%d old_rmse=%.6g evidence_aware_rmse=%.6g 100k_plain_rmse=%.6g old/new_rmse_ratio=%.3f old_starved_adaptation_exact=%d new_protected=%d rescued=%d zero_after_400=%d zero_after_600=%d surrogate_only_over_400=%d old_IS_selection=%.1f%% new_IS_selection=%.1f%% warm_start_verified=%t",
		report.GroupID, len(report.Seeds), old.RMSE, pipeline.RMSE, baseline.RMSE,
		report.OldRacingToEvidenceAwareRMSE, report.OldRacingStarvedAdaptationExact,
		report.EvidenceAwareProtectedAdaptationExact, report.RescuedAdaptationExact,
		report.AdaptationExactStillZeroAfter400, report.AdaptationExactStillZeroAfter600,
		report.SurrogateOnlyOver400Samples, report.OldRacingSelectionRate*100,
		report.PipelineSelectionRate*100, report.WarmStartVerified)
}

func writeRareBenchmarkCSV(path string, rows []rareBenchmarkRow) error {
	file, err := os.OpenFile(path, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, 0600)
	if err != nil {
		return err
	}
	defer file.Close()
	w := csv.NewWriter(file)
	defer w.Flush()
	if err := w.Write([]string{"seed", "method", "team", "position", "reference_probability", "estimate", "std_error", "relative_se", "ess", "hits", "samples", "production_plain_mc_equivalent", "search_overhead_plain_mc_equivalent", "design", "cem_initialization_mode", "cem_initialization_source", "cem_shortlist_fingerprint", "selected_team", "selected_position", "selected_snapshot_iteration", "scout_work", "adaptation_work", "evaluation_work", "evaluation_total_samples", "evaluation_result_work", "selected_evaluation_samples", "selected_evaluation_work", "production_work", "total_work", "total_work_limit", "adaptation_exact_shortlisted", "adaptation_exact_le_200", "adaptation_exact_ge_400", "adaptation_exact_starved", "adaptation_exact_rescued", "adaptation_exact_still_zero_400", "adaptation_exact_still_zero_600", "surrogate_only_over_400", "evaluation_ess_per_work", "selected_production_mean_weight", "is_ess_gain_vs_100k_mc"}); err != nil {
		return err
	}
	for _, r := range rows {
		values := []string{strconv.FormatInt(r.Seed, 10), r.Method, strconv.Itoa(r.Team), strconv.Itoa(r.Position),
			formatBenchmarkFloat(r.ReferenceProbability), formatBenchmarkFloat(r.Estimate), formatBenchmarkFloat(r.StdErr),
			formatBenchmarkFloat(r.RelativeSE), formatBenchmarkFloat(r.ESS), strconv.Itoa(r.Hits), strconv.Itoa(r.ProductionSamples),
			formatBenchmarkFloat(r.ProductionPlainMCEq), formatBenchmarkFloat(r.SearchOverheadPlainMCEq), r.ProductionDesign,
			r.CEMInitializationMode, r.CEMInitializationSource, r.CEMShortlistFingerprint,
			strconv.Itoa(r.SelectedTeam), strconv.Itoa(r.SelectedPosition), strconv.Itoa(r.SelectedSnapshotIteration),
			strconv.FormatInt(r.ScoutWork, 10), strconv.FormatInt(r.AdaptationWork, 10), strconv.FormatInt(r.EvaluationWork, 10),
			strconv.Itoa(r.EvaluationTotalSamples), strconv.FormatInt(r.EvaluationResultWork, 10),
			strconv.Itoa(r.SelectedEvaluationSamples), strconv.FormatInt(r.SelectedEvaluationWork, 10),
			strconv.FormatInt(r.ProductionWork, 10), strconv.FormatInt(r.TotalWork, 10), strconv.FormatInt(r.TotalWorkLimit, 10),
			strconv.Itoa(r.AdaptationExactShortlisted), strconv.Itoa(r.AdaptationExactLE200), strconv.Itoa(r.AdaptationExactGE400),
			strconv.Itoa(r.AdaptationExactStarved), strconv.Itoa(r.AdaptationExactRescued),
			strconv.Itoa(r.AdaptationExactStillZero400), strconv.Itoa(r.AdaptationExactStillZero600), strconv.Itoa(r.SurrogateOnlyOver400),
			formatBenchmarkFloat(r.EvaluationESSPerWork), formatBenchmarkFloat(r.SelectedProductionMeanWeight),
			formatBenchmarkFloat(r.ISBreakEvenGain100k)}
		if err := w.Write(values); err != nil {
			return err
		}
	}
	w.Flush()
	return w.Error()
}

func formatBenchmarkFloat(value float64) string {
	if math.IsInf(value, 1) {
		return "Inf"
	}
	return strconv.FormatFloat(value, 'g', -1, 64)
}

func TestRareBenchmarkCellAndBucketSummaries(t *testing.T) {
	target := rareBenchmarkTarget{Team: 9, Position: 2, Reference: 1e-4}
	rows := []rareBenchmarkRow{
		{Seed: 1, Method: "adaptive_pipeline", Team: 9, Position: 2, ReferenceProbability: 1e-4, Estimate: 0, ZeroEstimate: true, StdErr: 2e-5},
		{Seed: 2, Method: "adaptive_pipeline", Team: 9, Position: 2, ReferenceProbability: 1e-4, Estimate: 2e-4, StdErr: 3e-5},
		{Seed: 1, Method: "plain_mc_100k", Team: 9, Position: 2, ReferenceProbability: 1e-4, Estimate: 1e-4, StdErr: 1e-5},
		{Seed: 2, Method: "plain_mc_100k", Team: 9, Position: 2, ReferenceProbability: 1e-4, Estimate: 1e-4, StdErr: 1e-5},
	}
	cells := summarizeRareBenchmarkCells(rows, []rareBenchmarkTarget{target})
	if len(cells) != 1 || cells[0].PipelineRMSE <= 0 || cells[0].PlainMCRMSE != 0 || cells[0].PipelineZeroRate != 0.5 {
		t.Fatalf("unexpected per-cell summary: %+v", cells)
	}
	buckets := summarizeRareBenchmarkBuckets(cells)
	if len(buckets) != 3 || buckets[2].Cells != 1 || buckets[2].RMSEratio != 0 {
		t.Fatalf("unexpected probability-bucket summary: %+v", buckets)
	}
}

func TestRareBenchmarkCSVRowsMatchHeader(t *testing.T) {
	path := filepath.Join(t.TempDir(), "benchmark.csv")
	row := rareBenchmarkRow{Seed: 1, Method: "old_racing", Team: 2, Position: 3, ReferenceProbability: 1e-4,
		CEMInitializationMode: "standings_directed", CEMInitializationSource: "benchmark_override",
		CEMShortlistFingerprint: "abc", EvaluationTotalSamples: 900, EvaluationResultWork: 612000}
	if err := writeRareBenchmarkCSV(path, []rareBenchmarkRow{row}); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	records, err := csv.NewReader(strings.NewReader(string(data))).ReadAll()
	if err != nil {
		t.Fatal(err)
	}
	if len(records) != 2 || len(records[0]) != len(records[1]) {
		t.Fatalf("benchmark CSV header/data widths differ: records=%v", records)
	}
}

type cemFamilyBenchmarkRow struct {
	Seed                             int64           `json:"seed"`
	Method                           string          `json:"method"`
	Team                             int             `json:"team"`
	Position                         int             `json:"position"`
	Reference                        float64         `json:"reference_probability"`
	Estimate                         float64         `json:"estimate"`
	StdErr                           float64         `json:"std_error"`
	RelativeSE                       float64         `json:"relative_se"`
	ESS                              float64         `json:"ess"`
	Hits                             int             `json:"hits"`
	Samples                          int             `json:"samples"`
	Design                           string          `json:"design"`
	TotalWork                        int64           `json:"total_work"`
	WorkLimit                        int64           `json:"work_limit"`
	ProductionWork                   int64           `json:"production_work"`
	Parameterization                 string          `json:"parameterization"`
	FrontierFingerprint              string          `json:"frontier_fingerprint"`
	InitialAttackL2                  float64         `json:"initial_attack_l2"`
	InitialConcedeL2                 float64         `json:"initial_concede_l2"`
	SelectedAttackL2                 float64         `json:"selected_attack_l2"`
	SelectedConcedeL2                float64         `json:"selected_concede_l2"`
	MaxAbsAttack                     float64         `json:"max_abs_attack"`
	MaxAbsConcede                    float64         `json:"max_abs_concede"`
	FitObjectiveStart                float64         `json:"fit_objective_start"`
	FitObjectiveEnd                  float64         `json:"fit_objective_end"`
	FitWallSeconds                   float64         `json:"fit_wall_time_seconds"`
	TotalWallSeconds                 float64         `json:"total_wall_time_seconds"`
	FitIterations                    int             `json:"fit_iterations"`
	FitConverged                     bool            `json:"fit_converged"`
	AttackParameters                 map[int]float64 `json:"selected_attack_parameters,omitempty"`
	ConcessionParameters             map[int]float64 `json:"selected_concession_parameters,omitempty"`
	AdaptationExactHits              int             `json:"adaptation_exact_hits"`
	AdaptationExactCandidates        int             `json:"adaptation_exact_candidate_count"`
	RetainedAdaptationExactSnapshots int             `json:"retained_adaptation_exact_snapshots"`
	HeldoutExactHits                 int             `json:"heldout_exact_hits"`
	Near1Efficiency                  float64         `json:"near1_efficiency_ratio"`
	Near2Efficiency                  float64         `json:"near2_efficiency_ratio"`
	WeightedExactToNear1             float64         `json:"weighted_exact_to_near1_ratio"`
	ExactESSPerWork                  float64         `json:"exact_ess_per_work"`
	Near1HighWithExact               int             `json:"near1_gt2_with_exact"`
	Near1HighWithoutExact            int             `json:"near1_gt2_without_exact"`
	EvaluatedSnapshotCount           int             `json:"evaluated_snapshot_count"`
	ProductionESS                    float64         `json:"production_event_ess"`
	ProductionRelSE                  float64         `json:"production_rel_se"`
	MaxEventWeightShare              float64         `json:"max_event_weight_share"`
	MeanWeight                       float64         `json:"mean_importance_weight"`
	SearchWorkPlainMCEq              float64         `json:"search_work_plain_mc_equivalent"`
	EliteDistanceAt300               float64         `json:"elite_distance_after_300"`
	EliteDistanceAt600               float64         `json:"elite_distance_after_600"`
	EliteDistanceAt900               float64         `json:"elite_distance_after_900"`
	SamplesToFirstNear               float64         `json:"samples_to_first_near"`
	SamplesToFirstExact              float64         `json:"samples_to_first_exact"`
	RepeatedExactHitBatches          int             `json:"repeated_exact_hit_batches"`
	BestNearTargetRate               float64         `json:"best_near_target_rate"`
	BestExactRate                    float64         `json:"best_exact_rate"`
}

type cemFamilySummary struct {
	RMSE                    float64 `json:"rmse"`
	RelativeRMSE            float64 `json:"relative_rmse"`
	MAE                     float64 `json:"mae"`
	ZeroRate                float64 `json:"zero_estimate_rate"`
	MeanESS                 float64 `json:"mean_ess"`
	MeanProductionESS       float64 `json:"mean_production_ess"`
	MeanMaxEventWeightShare float64 `json:"mean_max_event_weight_share"`
	MeanWeight              float64 `json:"mean_importance_weight"`
	MeanSearchWork          float64 `json:"mean_search_work_plain_mc_equivalent"`
	MeanFitSeconds          float64 `json:"mean_fit_wall_seconds"`
	MeanTotalSeconds        float64 `json:"mean_total_wall_seconds"`
	AdaptationExactHitRate  float64 `json:"adaptation_exact_hit_rate"`
	HeldoutExactHitRate     float64 `json:"heldout_exact_hit_rate"`
	MeanNear1Efficiency     float64 `json:"mean_near1_efficiency_ratio"`
	MeanNear2Efficiency     float64 `json:"mean_near2_efficiency_ratio"`
	MeanExactESSPerWork     float64 `json:"mean_exact_ess_per_work"`
	MeanSamplesToFirstNear  float64 `json:"mean_samples_to_first_near"`
	MeanSamplesToFirstExact float64 `json:"mean_samples_to_first_exact"`
	RepeatedExactHitBatches float64 `json:"mean_repeated_exact_hit_batches"`
}

type cemFamilyReport struct {
	GroupID               int                           `json:"group_id"`
	Seeds                 []int64                       `json:"seeds"`
	Rows                  []cemFamilyBenchmarkRow       `json:"rows"`
	Summaries             map[string]cemFamilySummary   `json:"summaries"`
	BucketRMSE            map[string]map[string]float64 `json:"bucket_rmse"`
	CellRMSE              map[string]map[string]float64 `json:"cell_rmse"`
	AttackToScoringRMSE   float64                       `json:"attack_to_scoring_rmse_ratio"`
	ScoringToPlainRMSE    float64                       `json:"scoring_to_plain_rmse_ratio"`
	AttackToPlainRMSE     float64                       `json:"attack_to_plain_rmse_ratio"`
	ScoutFrontiersMatched bool                          `json:"scout_frontiers_matched"`
	EqualTotalWork        bool                          `json:"equal_total_work_budget"`
	SelectionRates        map[string]float64
	Near1Efficiency       map[string]float64
	Near1ExactConversion  map[string]map[string]float64 `json:"near1_exact_conversion"`
	Targets               []rareBenchmarkTarget
}

// Opt-in 20-seed controlled comparison of the existing scoring-only family
// and attack+concession. It consumes the saved group-16982 request fixture.
func TestRarePositionCEMFamilyMatchedComputeBenchmark(t *testing.T) {
	if os.Getenv("RARE_POSITION_CEM_FAMILY_BENCHMARK") != "1" {
		t.Skip("set RARE_POSITION_CEM_FAMILY_BENCHMARK=1 to run the 20-seed CEM family benchmark")
	}
	fixture := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if fixture == "" {
		t.Fatal("RARE_POSITION_BENCHMARK_GROUP_JSON must point to the saved group request JSON")
	}
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	if input.Id != 16982 {
		t.Fatalf("fixture group_id=%d, want 16982", input.Id)
	}
	const plainSamples = 100000
	report := cemFamilyReport{GroupID: input.Id, Summaries: make(map[string]cemFamilySummary),
		BucketRMSE: make(map[string]map[string]float64), ScoutFrontiersMatched: true,
		CellRMSE: make(map[string]map[string]float64), Near1ExactConversion: make(map[string]map[string]float64),
		EqualTotalWork: true, SelectionRates: make(map[string]float64),
		Near1Efficiency: make(map[string]float64), Targets: append([]rareBenchmarkTarget(nil), rareBenchmarkTargets...)}
	for i := 0; i < 20; i++ {
		report.Seeds = append(report.Seeds, int64(1001+i))
	}
	envNames := []string{"RARE_POSITION_RANDOM_SEED", "RARE_POSITION_IMPORTANCE_SAMPLING", "RARE_POSITION_BENCHMARK_ITERATIONS", "RARE_POSITION_BENCHMARK_CEM_INIT_MODE", "RARE_POSITION_CEM_PARAMETERIZATION", "RARE_POSITION_BENCHMARK_CEM_PARAMETERIZATION", "RARE_POSITION_CEM_RACING_MODE"}
	for _, key := range envNames {
		value, ok := os.LookupEnv(key)
		key := key
		t.Cleanup(func() { restoreBenchmarkEnv(t, key, value, ok) })
	}
	for _, seed := range report.Seeds {
		scoring, scoreDiag := runRareCEMFamilyBenchmarkArm(t, input, seed, CEMTeamScoring)
		attack, attackDiag := runRareCEMFamilyBenchmarkArm(t, input, seed, CEMTeamAttackConcession)
		if scoreDiag.CEMFrontierFingerprint != attackDiag.CEMFrontierFingerprint {
			report.ScoutFrontiersMatched = false
			t.Fatalf("seed %d scout/frontier mismatch", seed)
		}
		if scoreDiag.TotalWork > scoreDiag.TotalWorkLimit || attackDiag.TotalWork > attackDiag.TotalWorkLimit || scoreDiag.TotalWorkLimit != attackDiag.TotalWorkLimit {
			report.EqualTotalWork = false
			t.Fatalf("seed %d work-budget mismatch scoring=%d/%d attack=%d/%d", seed, scoreDiag.TotalWork, scoreDiag.TotalWorkLimit, attackDiag.TotalWork, attackDiag.TotalWorkLimit)
		}
		plainSeed := deriveRarePositionSeed(seed, "plain-mc")
		_ = os.Setenv("RARE_POSITION_RANDOM_SEED", strconv.FormatInt(plainSeed, 10))
		_ = os.Unsetenv("RARE_POSITION_IMPORTANCE_SAMPLING")
		_ = os.Setenv("RARE_POSITION_BENCHMARK_ITERATIONS", strconv.Itoa(plainSamples))
		_ = os.Setenv("RARE_POSITION_BENCHMARK_CEM_PARAMETERIZATION", "")
		baseline := cloneGroupForBenchmark(input).calculate_odds()
		baselineOdds, ok := baseline["team_odds"].(map[int]*TeamOdds)
		if !ok {
			t.Fatalf("seed %d returned no plain-MC odds", seed)
		}
		unplayed := 0
		for _, g := range input.Games {
			if !g.Played {
				unplayed++
			}
		}
		workPerSample := estimateSeasonWork(unplayed, 1, len(input.Team_groups))
		plainWork := int64(plainSamples) * workPerSample
		if plainWork != scoreDiag.TotalWorkLimit {
			report.EqualTotalWork = false
			t.Fatalf("seed %d plain work=%d pipeline limit=%d", seed, plainWork, scoreDiag.TotalWorkLimit)
		}
		for _, target := range rareBenchmarkTargets {
			report.Rows = append(report.Rows, cemFamilyBenchmarkRowFromEstimate(seed, "scoring_only", target, scoring[target.Team][target.Position], scoreDiag),
				cemFamilyBenchmarkRowFromEstimate(seed, "attack_concession", target, attack[target.Team][target.Position], attackDiag))
			odds := baselineOdds[target.Team]
			if odds == nil || target.Position >= len(odds.Pos) {
				t.Fatalf("plain baseline missing team=%d position=%d", target.Team, target.Position)
			}
			p := odds.Pos[target.Position] / 100
			report.Rows = append(report.Rows, cemFamilyBenchmarkRow{Seed: seed, Method: "plain_mc_100k", Team: target.Team, Position: target.Position, Reference: target.Reference,
				Estimate: p, StdErr: math.Sqrt(p * (1 - p) / plainSamples), RelativeSE: safeRatio(math.Sqrt(p*(1-p)/plainSamples), p), ESS: float64(plainSamples) * p,
				Hits: int(math.Round(float64(plainSamples) * p)), Samples: plainSamples, Design: "plain_mc", TotalWork: plainWork, WorkLimit: plainWork, ProductionWork: plainWork, MeanWeight: 1})
		}
	}
	methods := []string{"plain_mc_100k", "scoring_only", "attack_concession"}
	for _, method := range methods {
		report.Summaries[method] = summarizeCEMFamily(report.Rows, method)
	}
	for _, target := range rareBenchmarkTargets {
		cellKey := fmt.Sprintf("%d/%d", target.Team, target.Position)
		report.CellRMSE[cellKey] = make(map[string]float64)
		for _, method := range methods {
			report.CellRMSE[cellKey][method] = cemFamilyCellRMSE(report.Rows, method, target)
		}
	}
	for _, bucket := range []struct {
		name      string
		low, high float64
	}{{"1e-5_to_3e-5", 1e-5, 3e-5}, {"3e-5_to_1e-4", 3e-5, 1e-4}, {"1e-4_to_2e-4", 1e-4, 2e-4 + 1e-15}} {
		report.BucketRMSE[bucket.name] = make(map[string]float64)
		for _, method := range methods {
			report.BucketRMSE[bucket.name][method] = cemFamilyBucketRMSE(report.Rows, method, bucket.low, bucket.high)
		}
	}
	for _, method := range []string{"scoring_only", "attack_concession"} {
		report.SelectionRates[method] = cemFamilyISSelectionRate(report.Rows, method)
		report.Near1Efficiency[method] = cemFamilyNear1Mean(report.Rows, method)
		report.Near1ExactConversion[method] = cemFamilyNear1Conversion(report.Rows, method)
	}
	report.AttackToScoringRMSE = safeRatio(report.Summaries["attack_concession"].RMSE, report.Summaries["scoring_only"].RMSE)
	report.ScoringToPlainRMSE = safeRatio(report.Summaries["scoring_only"].RMSE, report.Summaries["plain_mc_100k"].RMSE)
	report.AttackToPlainRMSE = safeRatio(report.Summaries["attack_concession"].RMSE, report.Summaries["plain_mc_100k"].RMSE)
	encoded, err := json.MarshalIndent(report, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	output := os.Getenv("RARE_POSITION_CEM_FAMILY_BENCHMARK_OUTPUT")
	if output == "" {
		output = "/tmp/rare-position-cem-family-benchmark.json"
	}
	if err := os.WriteFile(output, encoded, 0600); err != nil {
		t.Fatal(err)
	}
	fmt.Printf("CEM family benchmark: scoring RMSE=%.6g; attack+concession RMSE=%.6g; plain MC RMSE=%.6g; attack/scoring=%.3f; report=%s\n",
		report.Summaries["scoring_only"].RMSE, report.Summaries["attack_concession"].RMSE, report.Summaries["plain_mc_100k"].RMSE, report.AttackToScoringRMSE, output)
}

func runRareCEMFamilyBenchmarkArm(t *testing.T, input GroupType, seed int64,
	parameterization CEMParameterization) (map[int]map[int]ProductionEstimate, *RarePositionSearchDiagnostics) {
	t.Helper()
	_ = os.Setenv("RARE_POSITION_CEM_PARAMETERIZATION", "")
	_ = os.Setenv("RARE_POSITION_BENCHMARK_CEM_PARAMETERIZATION", cemParameterizationName(parameterization))
	started := time.Now()
	estimates, diagnostics := runRareBenchmarkPipelineArm(t, input, seed, "")
	diagnostics.TotalWallSeconds = time.Since(started).Seconds()
	if diagnostics.CEMParameterization != cemParameterizationName(parameterization) {
		t.Fatalf("seed %d parameterization=%s, want %s", seed, diagnostics.CEMParameterization, cemParameterizationName(parameterization))
	}
	return estimates, diagnostics
}

func cemFamilyBenchmarkRowFromEstimate(seed int64, method string, target rareBenchmarkTarget,
	estimate ProductionEstimate, d *RarePositionSearchDiagnostics) cemFamilyBenchmarkRow {
	r := cemFamilyBenchmarkRow{Seed: seed, Method: method, Team: target.Team, Position: target.Position, Reference: target.Reference,
		Parameterization: d.CEMParameterization, FrontierFingerprint: d.CEMFrontierFingerprint, TotalWork: d.TotalWork, WorkLimit: d.TotalWorkLimit,
		ProductionWork: d.ProductionWork, InitialAttackL2: d.InitialAttackL2, InitialConcedeL2: d.InitialConcedeL2,
		SelectedAttackL2: d.SelectedAttackL2, SelectedConcedeL2: d.SelectedConcedeL2, MaxAbsAttack: d.MaxAbsAttack, MaxAbsConcede: d.MaxAbsConcede,
		AttackParameters: d.SelectedAttackParameters, ConcessionParameters: d.SelectedConcessionParameters,
		FitIterations: d.FitIterations, FitConverged: d.FitConverged, FitObjectiveStart: d.FitObjectiveStart, FitObjectiveEnd: d.FitObjectiveEnd,
		FitWallSeconds: d.FitWallSeconds, TotalWallSeconds: d.TotalWallSeconds, AdaptationExactHits: d.AdaptationExactHitBatches,
		AdaptationExactCandidates: d.TargetsWithExactHit, RetainedAdaptationExactSnapshots: d.AdaptationExactSnapshotsShortlisted,
		HeldoutExactHits: d.EvaluationSnapshotsWithExactHit, Near1Efficiency: d.MeanNear1EfficiencyRatio, Near2Efficiency: d.MeanNear2EfficiencyRatio,
		WeightedExactToNear1: d.MeanWeightedExactToNear1Ratio,
		ExactESSPerWork:      d.MeanExactESSPerWork, Near1HighWithExact: d.Near1EfficientWithExactHit, Near1HighWithoutExact: d.Near1EfficientWithoutExactHit,
		EvaluatedSnapshotCount: d.EvaluatedSnapshots,
		ProductionESS:          d.SelectedProductionESS, ProductionRelSE: d.SelectedProductionRelSE,
		MaxEventWeightShare: d.SelectedProductionMaxEventWeightShare, MeanWeight: d.SelectedProductionMeanWeight, SearchWorkPlainMCEq: d.SearchOverheadPlainMCEq,
		EliteDistanceAt300: d.EliteDistanceAt300, EliteDistanceAt600: d.EliteDistanceAt600, EliteDistanceAt900: d.EliteDistanceAt900,
		SamplesToFirstNear: d.MeanSamplesToFirstNear, SamplesToFirstExact: d.MeanSamplesToFirstExact,
		RepeatedExactHitBatches: d.RepeatedExactHitBatches, BestNearTargetRate: d.BestNearTargetRate, BestExactRate: d.BestExactRate}
	if estimate.Available {
		r.Estimate, r.StdErr, r.RelativeSE, r.ESS, r.Hits, r.Samples, r.Design = estimate.Probability, estimate.StdErr, relativeSEValue(estimate.RelativeSE), estimate.ESS, estimate.Hits, estimate.Samples, estimate.Design
		r.ProductionESS, r.ProductionRelSE = estimate.ESS, relativeSEValue(estimate.RelativeSE)
		r.MaxEventWeightShare, r.MeanWeight = estimate.MaxEventWeightShare, estimate.MeanWeight
	}
	return r
}

func summarizeCEMFamily(rows []cemFamilyBenchmarkRow, method string) cemFamilySummary {
	var s cemFamilySummary
	n := 0.0
	seenSeed := make(map[int64]bool)
	for _, r := range rows {
		if r.Method != method {
			continue
		}
		err := r.Estimate - r.Reference
		s.RMSE += err * err
		s.RelativeRMSE += square(safeRatio(err, r.Reference))
		s.MAE += math.Abs(err)
		if r.Estimate == 0 {
			s.ZeroRate++
		}
		s.MeanESS += r.ESS
		s.MeanProductionESS += r.ProductionESS
		s.MeanMaxEventWeightShare += r.MaxEventWeightShare
		s.MeanWeight += r.MeanWeight
		s.MeanSearchWork += r.SearchWorkPlainMCEq
		s.MeanFitSeconds += r.FitWallSeconds
		s.MeanTotalSeconds += r.TotalWallSeconds
		if !seenSeed[r.Seed] {
			seenSeed[r.Seed] = true
			if r.AdaptationExactCandidates > 0 {
				s.AdaptationExactHitRate++
			}
			if r.HeldoutExactHits > 0 {
				s.HeldoutExactHitRate++
			}
			s.MeanNear1Efficiency += r.Near1Efficiency
			s.MeanNear2Efficiency += r.Near2Efficiency
			s.MeanExactESSPerWork += r.ExactESSPerWork
			s.MeanSamplesToFirstNear += r.SamplesToFirstNear
			s.MeanSamplesToFirstExact += r.SamplesToFirstExact
			s.RepeatedExactHitBatches += float64(r.RepeatedExactHitBatches)
		}
		n++
	}
	if n == 0 {
		return s
	}
	s.RMSE = math.Sqrt(s.RMSE / n)
	s.RelativeRMSE = math.Sqrt(s.RelativeRMSE / n)
	s.MAE /= n
	s.ZeroRate /= n
	s.MeanESS /= n
	s.MeanProductionESS /= n
	s.MeanMaxEventWeightShare /= n
	s.MeanWeight /= n
	s.MeanSearchWork /= n
	s.MeanFitSeconds /= n
	s.MeanTotalSeconds /= n
	seedCount := float64(len(seenSeed))
	if seedCount > 0 {
		s.AdaptationExactHitRate /= seedCount
		s.HeldoutExactHitRate /= seedCount
		s.MeanNear1Efficiency /= seedCount
		s.MeanNear2Efficiency /= seedCount
		s.MeanExactESSPerWork /= seedCount
		s.MeanSamplesToFirstNear /= seedCount
		s.MeanSamplesToFirstExact /= seedCount
		s.RepeatedExactHitBatches /= seedCount
	}
	return s
}

func cemFamilyCellRMSE(rows []cemFamilyBenchmarkRow, method string, target rareBenchmarkTarget) float64 {
	sum, count := 0.0, 0
	for _, row := range rows {
		if row.Method == method && row.Team == target.Team && row.Position == target.Position {
			sum += square(row.Estimate - target.Reference)
			count++
		}
	}
	if count == 0 {
		return 0
	}
	return math.Sqrt(sum / float64(count))
}

func cemFamilyNear1Conversion(rows []cemFamilyBenchmarkRow, method string) map[string]float64 {
	perSeed := make(map[int64]cemFamilyBenchmarkRow)
	for _, row := range rows {
		if row.Method == method {
			perSeed[row.Seed] = row
		}
	}
	high, exact, noExact, evaluated := 0, 0, 0, 0
	for _, row := range perSeed {
		high += row.Near1HighWithExact + row.Near1HighWithoutExact
		exact += row.Near1HighWithExact
		noExact += row.Near1HighWithoutExact
		evaluated += row.EvaluatedSnapshotCount
	}
	return map[string]float64{"fraction_near1_efficiency_gt2": safeRatio(float64(high), float64(evaluated)),
		"exact_given_near1_gt2":      safeRatio(float64(exact), float64(high)),
		"zero_exact_given_near1_gt2": safeRatio(float64(noExact), float64(high))}
}

func cemFamilyBucketRMSE(rows []cemFamilyBenchmarkRow, method string, low, high float64) float64 {
	sum := 0.0
	n := 0
	for _, r := range rows {
		if r.Method == method && r.Reference >= low && r.Reference < high {
			sum += square(r.Estimate - r.Reference)
			n++
		}
	}
	if n == 0 {
		return 0
	}
	return math.Sqrt(sum / float64(n))
}
func cemFamilyISSelectionRate(rows []cemFamilyBenchmarkRow, method string) float64 {
	seeds, selected := map[int64]bool{}, map[int64]bool{}
	for _, r := range rows {
		if r.Method == method {
			seeds[r.Seed] = true
			if r.Design == "importance_sampling" {
				selected[r.Seed] = true
			}
		}
	}
	if len(seeds) == 0 {
		return 0
	}
	return float64(len(selected)) / float64(len(seeds))
}
func cemFamilyNear1Mean(rows []cemFamilyBenchmarkRow, method string) float64 {
	bySeed := map[int64]float64{}
	for _, r := range rows {
		if r.Method == method {
			bySeed[r.Seed] = r.Near1Efficiency
		}
	}
	if len(bySeed) == 0 {
		return 0
	}
	sum := 0.0
	for _, v := range bySeed {
		sum += v
	}
	return sum / float64(len(bySeed))
}
