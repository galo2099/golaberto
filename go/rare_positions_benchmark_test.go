package main

import (
	"encoding/json"
	"fmt"
	"math"
	"os"
	"strconv"
	"testing"
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
	SelectedSnapshotIteration        int     `json:"selected_snapshot_iteration"`
	CandidatesAdmitted               int     `json:"candidates_admitted"`
	CEMBatches                       int     `json:"cem_batches"`
	AdaptationExactHitBatches        int     `json:"adaptation_exact_hit_batches"`
	RetainedSnapshots                int     `json:"retained_snapshots"`
	EligibleSnapshots                int     `json:"eligible_snapshots"`
	EvaluatedSnapshots               int     `json:"evaluated_snapshots"`
	EvaluationHits                   int     `json:"evaluation_hits"`
	EvaluationESS                    float64 `json:"evaluation_ess"`
	EvaluationESSPerWork             float64 `json:"evaluation_ess_per_work"`
	ExactHitCandidatesWithLater      int     `json:"exact_hit_candidates_with_later_adaptation"`
	FirstHitSnapshotsEvaluated       int     `json:"first_hit_snapshots_evaluated"`
	LaterSnapshotsEvaluated          int     `json:"later_snapshots_evaluated"`
	LaterSnapshotBetterESSPerWork    int     `json:"later_snapshot_better_ess_per_work"`
	FirstHitSnapshotBetterESSPerWork int     `json:"first_hit_snapshot_better_ess_per_work"`
	LaterOnlySnapshotEvaluated       int     `json:"later_only_snapshot_evaluated"`
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
}

type rareBenchmarkReport struct {
	GroupID                             int                                   `json:"group_id"`
	ReferenceProvenance                 string                                `json:"reference_provenance"`
	Seeds                               []int64                               `json:"seeds"`
	PipelineScoutSamples                int                                   `json:"pipeline_scout_samples"`
	BaselineSamples                     int                                   `json:"baseline_samples"`
	Targets                             []rareBenchmarkTarget                 `json:"targets"`
	Rows                                []rareBenchmarkRow                    `json:"rows"`
	Summary                             map[string]rareBenchmarkMethodSummary `json:"summary"`
	RMSEPipelineToBaseline              float64                               `json:"pipeline_to_baseline_rmse_ratio"`
	RMSEDifferenceMean                  float64                               `json:"seed_level_rmse_difference_mean"`
	RMSEDifferenceSE                    float64                               `json:"seed_level_rmse_difference_standard_error"`
	MAEDifferenceMean                   float64                               `json:"seed_level_mae_difference_mean"`
	MAEDifferenceSE                     float64                               `json:"seed_level_mae_difference_standard_error"`
	WholeLossDifferenceMean             float64                               `json:"seed_level_whole_table_loss_difference_mean"`
	WholeLossDifferenceSE               float64                               `json:"seed_level_whole_table_loss_difference_standard_error"`
	PipelineSelectionRate               float64                               `json:"pipeline_importance_sampling_selection_rate"`
	PlainSelectionRate                  float64                               `json:"pipeline_plain_mc_selection_rate"`
	RunsWithExactAdaptationHits         int                                   `json:"runs_with_exact_adaptation_hits"`
	MeanRetainedSnapshots               float64                               `json:"mean_retained_snapshots"`
	MeanEligibleSnapshots               float64                               `json:"mean_eligible_snapshots"`
	MeanEvaluatedSnapshots              float64                               `json:"mean_evaluated_snapshots"`
	ExactHitCandidatesWithLater         int                                   `json:"exact_hit_candidates_with_later_adaptation"`
	ExactHitCandidatesTotal             int                                   `json:"exact_hit_candidates_total"`
	FractionExactHitCandidatesWithLater float64                               `json:"fraction_exact_hit_candidates_with_later_adaptation"`
	FirstHitSnapshotsEvaluated          int                                   `json:"first_hit_snapshots_evaluated"`
	LaterSnapshotsEvaluated             int                                   `json:"later_snapshots_evaluated"`
	LaterSnapshotBetterESSPerWork       int                                   `json:"later_snapshot_better_ess_per_work"`
	FirstHitSnapshotBetterESSPerWork    int                                   `json:"first_hit_snapshot_better_ess_per_work"`
	LaterOnlySnapshotEvaluated          int                                   `json:"later_only_snapshot_evaluated"`
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
	t.Cleanup(func() {
		restoreBenchmarkEnv(t, "RARE_POSITION_RANDOM_SEED", oldSeed, hadSeed)
		restoreBenchmarkEnv(t, "RARE_POSITION_IMPORTANCE_SAMPLING", oldSampling, hadSampling)
		restoreBenchmarkEnv(t, "RARE_POSITION_BENCHMARK_ITERATIONS", oldIterations, hadIterations)
	})
	_ = os.Unsetenv("RARE_POSITION_BENCHMARK_ITERATIONS")

	var seedLevelRMSEDifferences []float64
	pipelineSelected, pipelinePlain := 0, 0
	runsWithExact := 0
	var retainedTotal, eligibleTotal, evaluatedTotal float64
	var comparison RarePositionSearchDiagnostics
	unplayedGames := 0
	for _, game := range input.Games {
		if !game.Played {
			unplayedGames++
		}
	}
	plainWorkPerSample := estimateSeasonWork(unplayedGames, 1, len(input.Team_groups))
	totalWorkLimit := calculateMaxRareWork(unplayedGames, len(input.Team_groups))
	for _, seed := range report.Seeds {
		_ = os.Setenv("RARE_POSITION_RANDOM_SEED", strconv.FormatInt(seed, 10))
		_ = os.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")
		pipelineResult := cloneGroupForBenchmark(input).calculate_odds()
		pipelineEstimates, ok := pipelineResult["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
		if !ok {
			t.Fatalf("seed %d returned no typed rare-position estimates", seed)
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
		var diagnostics *RarePositionSearchDiagnostics
		for _, estimates := range pipelineEstimates {
			for _, estimate := range estimates {
				if estimate.SearchDiagnostics != nil {
					diagnostics = estimate.SearchDiagnostics
					break
				}
			}
			if diagnostics != nil {
				break
			}
		}
		if diagnostics != nil {
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
				report.Rows = append(report.Rows, benchmarkRow(seed, "adaptive_pipeline", target, got.Probability,
					got.StdErr, relativeSEValue(got.RelativeSE), got.ESS, got.Hits, got.MeetsPrecisionGoal,
					got.Samples, productionEquivalent, got.Design, searchOverheadEquivalent, got.SearchDiagnostics))
				seedErrors["adaptive_pipeline"] += square(got.Probability - target.Reference)
			} else {
				report.Rows = append(report.Rows, benchmarkRow(seed, "adaptive_pipeline", target,
					0, 0, math.Inf(1), 0, 0, false, 0, 0, "", 0, nil))
				seedErrors["adaptive_pipeline"] += square(target.Reference)
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
			report.Rows = append(report.Rows, benchmarkRow(seed, "plain_mc_100k", target, p, se,
				relSE, float64(baselineSamples)*p, int(math.Round(p*baselineSamples)),
				estimateMeetsPrecisionGoal(float64(baselineSamples)*p, relSE), baselineSamples,
				float64(baselineSamples), "plain_mc", 0, nil))
			seedErrors["plain_mc_100k"] += square(p - target.Reference)
		}
		seedLevelRMSEDifferences = append(seedLevelRMSEDifferences,
			math.Sqrt(seedErrors["adaptive_pipeline"]/float64(len(rareBenchmarkTargets)))-
				math.Sqrt(seedErrors["plain_mc_100k"]/float64(len(rareBenchmarkTargets))))
	}
	report.Summary["adaptive_pipeline"] = summarizeRareBenchmarkRows(report.Rows, "adaptive_pipeline", rareBenchmarkTargets)
	report.Summary["plain_mc_100k"] = summarizeRareBenchmarkRows(report.Rows, "plain_mc_100k", rareBenchmarkTargets)
	pipelineSummary, baselineSummary := report.Summary["adaptive_pipeline"], report.Summary["plain_mc_100k"]
	if baselineSummary.RMSE > 0 {
		report.RMSEPipelineToBaseline = pipelineSummary.RMSE / baselineSummary.RMSE
	}
	report.PipelineSelectionRate = float64(pipelineSelected) / float64(len(report.Seeds))
	report.PlainSelectionRate = float64(pipelinePlain) / float64(len(report.Seeds))
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
	fmt.Printf("rare-position benchmark report: %s\n", outputPath)
}

func cloneGroupForBenchmark(group GroupType) *GroupType {
	data, _ := json.Marshal(group)
	var copy GroupType
	_ = json.Unmarshal(data, &copy)
	return &copy
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
		row.SelectedSnapshotIteration = diagnostics.SelectedSnapshotIteration
		row.CandidatesAdmitted, row.CEMBatches = diagnostics.CandidatesAdmitted, diagnostics.CEMBatches
		row.AdaptationExactHitBatches = diagnostics.AdaptationExactHitBatches
		row.RetainedSnapshots, row.EligibleSnapshots = diagnostics.RetainedSnapshots, diagnostics.EligibleSnapshots
		row.EvaluatedSnapshots, row.EvaluationHits = diagnostics.EvaluatedSnapshots, diagnostics.EvaluationHits
		row.EvaluationESS, row.EvaluationESSPerWork = diagnostics.EvaluationESS, diagnostics.EvaluationESSPerWork
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
	for _, target := range targets {
		key := [2]int{target.Team, target.Position}
		if perCellN[key] > 0 && target.Reference > 0 {
			summary.MeanRelativeRMSE += math.Sqrt(perCellSq[key]/float64(perCellN[key])) / target.Reference
		}
	}
	summary.MeanRelativeRMSE /= float64(len(targets))
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
