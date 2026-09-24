package main

import (
	"encoding/json"
	"fmt"
	"math"
	"math/rand"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"
)

type pruningCalibration struct {
	Method           string        `json:"method"`
	Stride           int           `json:"stride"`
	Samples          int           `json:"calibration_samples"`
	Elapsed          time.Duration `json:"-"`
	ElapsedSeconds   float64       `json:"elapsed_seconds"`
	SamplesPerSecond float64       `json:"samples_per_second"`
	GamesPerSample   float64       `json:"average_games_per_sample"`
	SolverDuration   time.Duration `json:"-"`
	SolverSeconds    float64       `json:"solver_seconds"`
	SolverFraction   float64       `json:"solver_fraction"`
	PrunedFraction   float64       `json:"pruned_fraction"`
}

type pruningBenchmarkEstimator struct {
	Samples          int                  `json:"samples"`
	Hits             int                  `json:"hits"`
	GamesSimulated   int64                `json:"games_simulated"`
	Estimate         float64              `json:"estimate"`
	StdErr           float64              `json:"standard_error"`
	RelativeSE       *float64             `json:"relative_standard_error"`
	ESS              float64              `json:"event_ess"`
	ElapsedSeconds   float64              `json:"elapsed_seconds"`
	SamplesPerSecond float64              `json:"samples_per_second"`
	SufficientStats  EventSufficientStats `json:"-"`
}

type sequentialPruningBenchmarkRow struct {
	Seed                    int64                     `json:"seed"`
	Baseline                pruningBenchmarkEstimator `json:"baseline_disabled"`
	AllRank                 pruningBenchmarkEstimator `json:"enabled_all_rank_stream"`
	Sequential              SequentialRareEstimate    `json:"enabled_sequential_stream"`
	PooledProbability       float64                   `json:"pooled_probability"`
	PooledStdErr            float64                   `json:"pooled_standard_error"`
	PooledRelativeSE        *float64                  `json:"pooled_relative_standard_error,omitempty"`
	PooledESS               float64                   `json:"pooled_event_ess"`
	PooledHits              int                       `json:"pooled_hits"`
	PooledSamples           int                       `json:"pooled_samples"`
	PooledESSPerSecond      float64                   `json:"pooled_ess_per_second"`
	EnabledWallSeconds      float64                   `json:"enabled_actual_wall_seconds"`
	EnabledGamesSimulated   int64                     `json:"enabled_games_simulated"`
	ExpectedBaselineSeconds float64                   `json:"expected_baseline_seconds"`
	ExpectedEnabledSeconds  float64                   `json:"expected_enabled_seconds"`
	ESSRateRatio            float64                   `json:"pooled_to_baseline_ess_per_second"`
	ReferenceProbability    float64                   `json:"reference_probability,omitempty"`
	BaselineSquaredError    float64                   `json:"baseline_squared_error,omitempty"`
	PooledSquaredError      float64                   `json:"pooled_squared_error,omitempty"`
}

type sequentialPruningBenchmarkReport struct {
	GroupID                 int                             `json:"group_id"`
	TeamID                  int                             `json:"team_id"`
	Position                int                             `json:"target_position"`
	Threshold               float64                         `json:"interesting_probability_threshold"`
	TargetRuntimeSeconds    float64                         `json:"target_runtime_seconds"`
	ChosenStride            int                             `json:"chosen_stride"`
	Calibration             []pruningCalibration            `json:"calibration"`
	BaselineSamples         int                             `json:"frozen_baseline_samples"`
	AllRankSamples          int                             `json:"frozen_enabled_all_rank_samples"`
	PrunedSamples           int                             `json:"frozen_enabled_target_samples"`
	HardTargetSampleCap     int                             `json:"hard_target_sample_cap"`
	Seeds                   []int64                         `json:"seeds"`
	Rows                    []sequentialPruningBenchmarkRow `json:"rows"`
	BaselineRMSE            float64                         `json:"baseline_rmse,omitempty"`
	PooledRMSE              float64                         `json:"pooled_rmse,omitempty"`
	RMSEratio               float64                         `json:"pooled_to_baseline_rmse,omitempty"`
	MeanBaselineWallSeconds float64                         `json:"mean_baseline_wall_seconds"`
	MeanEnabledWallSeconds  float64                         `json:"mean_enabled_wall_seconds"`
	MeanAllRankSamples      float64                         `json:"mean_all_rank_samples"`
	MeanTargetSamples       float64                         `json:"mean_target_samples"`
	MeanTotalTargetSamples  float64                         `json:"mean_total_target_samples"`
	MeanGamesSimulated      float64                         `json:"mean_games_simulated"`
	MeanPrunedFraction      float64                         `json:"mean_pruned_fraction"`
	MeanSolverFraction      float64                         `json:"mean_solver_fraction"`
	MeanHits                float64                         `json:"mean_hits"`
	MeanPooledESS           float64                         `json:"mean_pooled_ess"`
	MeanPooledESSPerSecond  float64                         `json:"mean_pooled_ess_per_second"`
	MeanPooledRelativeSE    *float64                        `json:"mean_pooled_relative_se,omitempty"`
}

// Opt-in equal-runtime comparison. Calibration samples use separate derived
// streams and never enter either estimator. Both estimation counts are frozen
// before any evaluation sample is generated.
func TestSequentialRarePositionEqualRuntimeBenchmark(t *testing.T) {
	if os.Getenv("RARE_POSITION_SEQUENTIAL_PRUNING_BENCHMARK") != "1" {
		t.Skip("set RARE_POSITION_SEQUENTIAL_PRUNING_BENCHMARK=1 to run the sequential-pruning comparison")
	}
	fixturePath := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if fixturePath == "" {
		t.Fatal("RARE_POSITION_BENCHMARK_GROUP_JSON must point to the group request JSON")
	}
	data, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatal(err)
	}
	var group GroupType
	if err := json.Unmarshal(data, &group); err != nil {
		t.Fatal(err)
	}
	if group.Id != 16982 {
		t.Fatalf("matched-runtime benchmark fixture group_id=%d, want 16982", group.Id)
	}
	teams := make(map[int]bool)
	for _, team := range group.Team_groups {
		teams[team.Team_id] = true
	}
	for _, game := range group.Games {
		teams[game.HomeId], teams[game.AwayId] = true, true
	}
	teamID := benchmarkIntEnv(t, "RARE_POSITION_PRUNING_TEAM", 1553)
	position := benchmarkIntEnv(t, "RARE_POSITION_PRUNING_POSITION", 17)
	if !teams[teamID] || position < 0 || position >= len(group.Team_groups) {
		t.Fatalf("invalid target team=%d position=%d", teamID, position)
	}
	base, games, original, table, order := pruningBenchmarkContext(t, &group)
	validateIncrementalBoundsOnFixture(t, base, games, group.Team_groups, table, teamID)
	components := buildProposalComponents(games, teamID, RareBetter, []int{position}, map[int]float64{})
	if len(components) == 0 {
		t.Fatal("proposal construction returned no components")
	}
	validateProposalMixture(components, len(games))
	if math.Abs(components[0].Weight-OriginalMixtureWeight) > 1e-12 {
		t.Fatalf("benchmark proposal original-P weight=%g, want %g", components[0].Weight, OriginalMixtureWeight)
	}
	thresholdOld, thresholdSet := os.LookupEnv("RARE_POSITION_MIN_INTERESTING_PROBABILITY")
	threshold := ExperimentalRarePositionMinProbability
	if thresholdSet {
		threshold = rarePositionMinInterestingProbability()
	} else if err := os.Setenv("RARE_POSITION_MIN_INTERESTING_PROBABILITY", strconv.FormatFloat(threshold, 'g', -1, 64)); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if thresholdSet {
			_ = os.Setenv("RARE_POSITION_MIN_INTERESTING_PROBABILITY", thresholdOld)
		} else {
			_ = os.Unsetenv("RARE_POSITION_MIN_INTERESTING_PROBABILITY")
		}
	})
	targetRuntime := benchmarkDurationEnv("RARE_POSITION_PRUNING_TARGET_SECONDS", 20*time.Second)
	calibrationSamples := benchmarkIntEnv(t, "RARE_POSITION_SEQUENTIAL_PRUNING_CALIBRATION_SAMPLES", 500)
	if calibrationSamples < 10 {
		t.Fatal("calibration sample count must be at least 10")
	}
	calibrationSeed := deriveRarePositionSeed(872341, "pruning-calibration")
	calibrations := make([]pruningCalibration, 0, 4)
	baselineCal := calibratePruningMethod(base, games, original, components, table, order,
		group.Team_groups, teamID, position, 0, calibrationSamples, calibrationSeed, "baseline")
	calibrations = append(calibrations, baselineCal)
	bestStride, bestRate := 1, -1.0
	for _, stride := range []int{1, 4, 8} {
		cal := calibratePruningMethod(base, games, original, components, table, order,
			group.Team_groups, teamID, position, stride, calibrationSamples, calibrationSeed, "pruned")
		calibrations = append(calibrations, cal)
		if cal.SamplesPerSecond > bestRate {
			bestStride, bestRate = stride, cal.SamplesPerSecond
		}
	}
	hardTargetSampleCap := benchmarkIntEnv(t, "RARE_POSITION_SEQUENTIAL_PRUNING_MAX_TARGET_SAMPLES", DefaultSequentialPruningTargetSampleCap)
	allRankSamples, prunedSamples := runtimeDerivedSampleCounts(targetRuntime.Seconds(),
		SequentialPruningProductionBudgetFraction, baselineCal.SamplesPerSecond, bestRate, hardTargetSampleCap)
	baselineSamples := maxInt(1, int(math.Floor(targetRuntime.Seconds()*baselineCal.SamplesPerSecond)))
	if allRankSamples <= 0 || prunedSamples <= 0 {
		t.Fatal("runtime calibration did not produce positive fixed production sample counts")
	}
	seeds := benchmarkSeedList(t)
	report := sequentialPruningBenchmarkReport{GroupID: group.Id, TeamID: teamID, Position: position,
		Threshold: threshold, TargetRuntimeSeconds: targetRuntime.Seconds(), ChosenStride: bestStride,
		Calibration: calibrations, BaselineSamples: baselineSamples, AllRankSamples: allRankSamples,
		PrunedSamples: prunedSamples, HardTargetSampleCap: hardTargetSampleCap, Seeds: seeds}
	reference, hasReference := benchmarkFloatEnv("RARE_POSITION_PRUNING_REFERENCE_PROBABILITY")
	var baselineSquared, pooledSquared float64
	var sums [12]float64
	var relativeSECount int
	for _, seed := range seeds {
		baselineSeed := deriveRarePositionSeed(seed, "paired-benchmark-disabled")
		allRankSeed := deriveRarePositionSeed(seed, "paired-benchmark-enabled-all-rank")
		sequentialSeed := deriveRarePositionSeed(seed, "paired-benchmark-enabled-sequential")
		baseline := runAllRankBenchmarkEstimator(t, base, games, original, components, table, order,
			group.Team_groups, teamID, position, baselineSamples, baselineSeed)
		allRank := runAllRankBenchmarkEstimator(t, base, games, original, components, table, order,
			group.Team_groups, teamID, position, allRankSamples, allRankSeed)
		sequential := runSequentialRareEstimate(base, games, original, components, table, order,
			group.Team_groups, teamID, position, "original", bestStride, prunedSamples, sequentialSeed, "pruned", nil)
		pooledStats := allRank.SufficientStats.pooled(sequential.SufficientStats)
		pooled := productionEstimateFromSufficientStats(pooledStats, 0, "benchmark_pooled")
		enabledWall := allRank.ElapsedSeconds + sequential.ElapsedSeconds
		if math.Abs(baseline.ElapsedSeconds-enabledWall) > targetRuntime.Seconds()*0.5 {
			t.Logf("seed=%d actual runtime mismatch is over 50%% of target (baseline=%.3fs enabled=%.3fs)", seed, baseline.ElapsedSeconds, enabledWall)
		}
		row := sequentialPruningBenchmarkRow{Seed: seed, Baseline: baseline,
			AllRank: allRank, Sequential: sequential, PooledProbability: pooled.Probability,
			PooledStdErr: pooled.StdErr, PooledRelativeSE: pooled.RelativeSE,
			PooledESS: pooled.ESS, PooledHits: pooled.Hits, PooledSamples: pooled.Samples,
			PooledESSPerSecond: safeRatio(pooled.ESS, enabledWall), EnabledWallSeconds: enabledWall,
			EnabledGamesSimulated:   allRank.GamesSimulated + sequential.GamesSimulated,
			ExpectedBaselineSeconds: float64(baselineSamples) / baselineCal.SamplesPerSecond,
			ExpectedEnabledSeconds: float64(allRankSamples)/baselineCal.SamplesPerSecond +
				float64(prunedSamples)/bestRate,
			ESSRateRatio: safeRatio(safeRatio(pooled.ESS, enabledWall), safeRatio(baseline.ESS, baseline.ElapsedSeconds))}
		if hasReference {
			row.ReferenceProbability = reference
			row.BaselineSquaredError = square(baseline.Estimate - reference)
			row.PooledSquaredError = square(pooled.Probability - reference)
			baselineSquared += row.BaselineSquaredError
			pooledSquared += row.PooledSquaredError
		}
		report.Rows = append(report.Rows, row)
		sums[0] += baseline.ElapsedSeconds
		sums[1] += enabledWall
		sums[2] += float64(allRankSamples)
		sums[3] += float64(prunedSamples)
		sums[4] += float64(pooled.Samples)
		sums[5] += float64(allRank.GamesSimulated + sequential.GamesSimulated)
		sums[6] += sequential.PrunedFraction
		sums[7] += sequential.SolverFraction
		sums[8] += float64(pooled.Hits)
		sums[9] += pooled.ESS
		if pooled.RelativeSE != nil {
			sums[10] += *pooled.RelativeSE
			relativeSECount++
		}
		sums[11] += safeRatio(pooled.ESS, enabledWall)
	}
	seedCount := float64(len(seeds))
	report.MeanBaselineWallSeconds, report.MeanEnabledWallSeconds = sums[0]/seedCount, sums[1]/seedCount
	report.MeanAllRankSamples, report.MeanTargetSamples = sums[2]/seedCount, sums[3]/seedCount
	report.MeanTotalTargetSamples, report.MeanGamesSimulated = sums[4]/seedCount, sums[5]/seedCount
	report.MeanPrunedFraction, report.MeanSolverFraction = sums[6]/seedCount, sums[7]/seedCount
	report.MeanHits, report.MeanPooledESS = sums[8]/seedCount, sums[9]/seedCount
	if relativeSECount > 0 {
		meanRelativeSE := sums[10] / float64(relativeSECount)
		report.MeanPooledRelativeSE = &meanRelativeSE
	}
	report.MeanPooledESSPerSecond = sums[11] / seedCount
	if hasReference {
		report.BaselineRMSE = math.Sqrt(baselineSquared / float64(len(seeds)))
		report.PooledRMSE = math.Sqrt(pooledSquared / float64(len(seeds)))
		report.RMSEratio = safeRatio(report.PooledRMSE, report.BaselineRMSE)
	}
	output := os.Getenv("RARE_POSITION_PRUNING_BENCHMARK_OUTPUT")
	if output == "" {
		output = filepath.Join(os.TempDir(), "rare-position-sequential-pruning.json")
	}
	encoded, err := json.MarshalIndent(report, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(output, encoded, 0o600); err != nil {
		t.Fatal(err)
	}
	t.Logf("sequential-pruning matched-runtime benchmark group=%d team=%d position=%d stride=%d seeds=%d baseline_N=%d all_rank_N=%d target_N=%d total_target_N=%d baseline_wall=%.3f enabled_wall=%.3f pruned_fraction=%.4f solver_fraction=%.4f pooled_hits=%.2f pooled_ESS=%.3f pooled_ESS_per_second=%.4f pooled_relSE=%.4f baseline_RMSE=%.8g pooled_RMSE=%.8g report=%s",
		group.Id, teamID, position, bestStride, len(seeds), baselineSamples, allRankSamples,
		prunedSamples, allRankSamples+prunedSamples, report.MeanBaselineWallSeconds,
		report.MeanEnabledWallSeconds, report.MeanPrunedFraction, report.MeanSolverFraction,
		report.MeanHits, report.MeanPooledESS, report.MeanPooledESSPerSecond,
		relativeSEValue(report.MeanPooledRelativeSE), report.BaselineRMSE, report.PooledRMSE, output)
}

func validateIncrementalBoundsOnFixture(t *testing.T, base []*TeamCampaign, games []*GameType,
	teams []TeamType, table *Table, target int) {
	t.Helper()
	for seed := int64(1); seed <= 5; seed++ {
		campaign := make([]*TeamCampaign, len(base))
		for i, team := range base {
			campaign[i] = team.clone()
		}
		prefixGames := make([]*GameType, len(games))
		for i, game := range games {
			copy := *game
			prefixGames[i] = &copy
		}
		state := newIncrementalPositionBounds(prefixGames, teams)
		rng := rand.New(rand.NewSource(deriveRarePositionSeed(seed, "fixture-prefix-validation")))
		compare := func(prefix int) {
			wantBest, wantWorst, wantRemaining := conservativePositionBounds(target, campaign, teams, prefixGames, table)
			gotBest, gotWorst, gotRemaining := state.ranks(target, campaign, table)
			if wantBest != gotBest || wantWorst != gotWorst || wantRemaining != gotRemaining {
				t.Fatalf("real fixture prefix mismatch seed=%d prefix=%d incremental=(%d,%d,%t) conservative=(%d,%d,%t)",
					seed, prefix, gotBest, gotWorst, gotRemaining, wantBest, wantWorst, wantRemaining)
			}
		}
		compare(0)
		prefix := 0
		for _, game := range prefixGames {
			if game.Played {
				continue
			}
			home, away := poissonRand(rng, game.HomePower), poissonRand(rng, game.AwayPower)
			completed := &GameType{Id: game.Id, HomeId: game.HomeId, AwayId: game.AwayId,
				HomeScore: home, AwayScore: away, Played: true,
				home_table_index: game.home_table_index, away_table_index: game.away_table_index}
			if campaign[game.home_table_index] != nil {
				campaign[game.home_table_index].add_game(completed)
			}
			if campaign[game.away_table_index] != nil {
				campaign[game.away_table_index].add_game(completed)
			}
			game.Played = true
			state.gameCompleted(game)
			prefix++
			compare(prefix)
		}
	}
}

func runAllRankBenchmarkEstimator(t *testing.T, base []*TeamCampaign, games []*GameType,
	original []GameProposalMeans, components []ProposalComponent, table *Table,
	order []SortType, groups []TeamType, team, position, samples int, seed int64) pruningBenchmarkEstimator {
	t.Helper()
	job := &RareSimulationJob{TeamID: team, Direction: RareBetter,
		CandidatePositions: []int{position}, Components: cloneProposalComponents(components),
		Iterations: samples, CollectAllRanks: true}
	started := time.Now()
	_, _ = estimateRarePositionsForJob(base, games, original, table, order, groups, job,
		rand.New(rand.NewSource(seed)), 16982)
	elapsed := time.Since(started).Seconds()
	estimate, ok := job.FullRankEstimates[team][position]
	if !ok {
		t.Fatalf("all-rank benchmark estimator returned no estimate for team=%d position=%d", team, position)
	}
	unplayed := 0
	for _, game := range games {
		if !game.Played {
			unplayed++
		}
	}
	return pruningBenchmarkEstimator{
		Samples: estimate.Samples, Hits: estimate.Hits,
		GamesSimulated: int64(estimate.Samples * unplayed), Estimate: estimate.Probability,
		StdErr: estimate.StdErr, RelativeSE: relativeSEPointer(estimate.RelativeSE), ESS: estimate.ESS,
		ElapsedSeconds: elapsed, SamplesPerSecond: safeRatio(float64(samples), elapsed),
		SufficientStats: estimate.SufficientStats,
	}
}

func calibratePruningMethod(base []*TeamCampaign, games []*GameType, original []GameProposalMeans,
	components []ProposalComponent, table *Table, order []SortType, groups []TeamType,
	team, position, stride, samples int, seed int64, method string) pruningCalibration {
	cal := pruningCalibration{Method: method, Stride: stride, Samples: samples}
	unplayed := 0
	for _, game := range games {
		if !game.Played {
			unplayed++
		}
	}
	started := time.Now()
	pruned := 0
	var sim []*TeamCampaign
	var teams []*TeamCampaign
	var logs, weights []float64
	var allRanks *weightedRankAccumulator
	if method == "baseline" {
		sim = make([]*TeamCampaign, len(base))
		teams = make([]*TeamCampaign, len(groups))
		logs, weights = make([]float64, len(components)), make([]float64, len(components))
		allRanks = newWeightedRankAccumulator(groups, len(groups))
	}
	for i := 0; i < samples; i++ {
		rng := rand.New(rand.NewSource(targetSampleSeed(deriveRarePositionSeed(seed, fmt.Sprintf("%s-%d", method, stride)), i)))
		if method == "baseline" {
			simulateTargetTeamRankAndWeightMulti(base, sim, teams, games, original, components,
				table, order, groups, team, rng, logs, weights, allRanks)
		} else {
			_, _, stats := simulateTargetSequential(base, games, nil, original, components, table,
				order, groups, team, position, stride, false, rng)
			cal.SolverDuration += stats.SolverDuration
			cal.GamesPerSample += float64(stats.GamesSimulated)
			if stats.Pruned {
				pruned++
			}
		}
	}
	cal.Elapsed = time.Since(started)
	cal.ElapsedSeconds = cal.Elapsed.Seconds()
	cal.SolverSeconds = cal.SolverDuration.Seconds()
	if cal.Elapsed > 0 {
		cal.SamplesPerSecond = float64(samples) / cal.Elapsed.Seconds()
		cal.SolverFraction = cal.SolverDuration.Seconds() / cal.Elapsed.Seconds()
	}
	if method == "baseline" {
		cal.GamesPerSample = float64(samples * unplayed)
	} else if samples > 0 {
		cal.GamesPerSample /= float64(samples)
		cal.PrunedFraction = float64(pruned) / float64(samples)
	}
	return cal
}

func pruningBenchmarkContext(t *testing.T, group *GroupType) ([]*TeamCampaign, []*GameType,
	[]GameProposalMeans, *Table, []SortType) {
	t.Helper()
	keySet := make(map[uint32]bool)
	for _, team := range group.Team_groups {
		keySet[uint32(team.Team_id)] = true
	}
	for _, game := range group.Games {
		keySet[uint32(game.HomeId)] = true
		keySet[uint32(game.AwayId)] = true
	}
	keys := make([]uint32, 0, len(keySet)+1)
	for id := range keySet {
		keys = append(keys, id)
	}
	if len(keys) == 2 || len(keys) == 4 {
		keys = append(keys, ^uint32(0))
	}
	table := NewTable(keys)
	for _, game := range group.Games {
		game.home_table_index = table.Query(uint32(game.HomeId))
		game.away_table_index = table.Query(uint32(game.AwayId))
	}
	sortOrder := []SortType{PT, GD, GF, BIAS}
	usesHead := false
	if group.Phase != nil && group.Phase.Sort != "" {
		sortOrder = build_sorted_array(strings.Split(strings.Join(strings.Fields(group.Phase.Sort), ""), ","))
		for _, criterion := range sortOrder {
			usesHead = usesHead || criterion == HEAD
		}
	}
	win, draw, loss := 3, 1, 0
	if group.Phase != nil && group.Phase.Championship != nil {
		win, draw, loss = group.Phase.Championship.Point_win, group.Phase.Championship.Point_draw, group.Phase.Championship.Point_loss
	}
	campaign := make([]*TeamCampaign, len(keys))
	for _, team := range group.Team_groups {
		campaign[table.Query(uint32(team.Team_id))] = &TeamCampaign{id: team.Team_id, points: team.Add_sub,
			bias: team.Bias, add_sub: team.Add_sub, points_win: win, points_draw: draw, points_loss: loss, uses_head: usesHead}
	}
	for _, game := range group.Games {
		if game.Played {
			if campaign[game.home_table_index] != nil {
				campaign[game.home_table_index].add_game(game)
			}
			if campaign[game.away_table_index] != nil {
				campaign[game.away_table_index].add_game(game)
			}
		}
	}
	means := make([]GameProposalMeans, len(group.Games))
	for i, game := range group.Games {
		means[i] = GameProposalMeans{Home: game.HomePower, Away: game.AwayPower}
	}
	return campaign, group.Games, means, table, sortOrder
}

func benchmarkIntEnv(t *testing.T, name string, fallback int) int {
	t.Helper()
	value := os.Getenv(name)
	if value == "" {
		return fallback
	}
	parsed, err := strconv.Atoi(value)
	if err != nil {
		t.Fatalf("%s=%q: %v", name, value, err)
	}
	return parsed
}

func maxInt(a, b int) int {
	if a > b {
		return a
	}
	return b
}

func benchmarkDurationEnv(name string, fallback time.Duration) time.Duration {
	value := os.Getenv(name)
	if value == "" {
		return fallback
	}
	seconds, err := strconv.ParseFloat(value, 64)
	if err != nil || seconds <= 0 {
		return fallback
	}
	return time.Duration(seconds * float64(time.Second))
}

func benchmarkFloatEnv(name string) (float64, bool) {
	value := os.Getenv(name)
	if value == "" {
		return 0, false
	}
	parsed, err := strconv.ParseFloat(value, 64)
	return parsed, err == nil && parsed >= 0 && !math.IsNaN(parsed) && !math.IsInf(parsed, 0)
}

func benchmarkSeedList(t *testing.T) []int64 {
	t.Helper()
	value := os.Getenv("RARE_POSITION_PRUNING_SEEDS")
	if value == "" {
		seeds := make([]int64, 20)
		for i := range seeds {
			seeds[i] = int64(1001 + i)
		}
		return seeds
	}
	var seeds []int64
	for _, field := range strings.Split(value, ",") {
		seed, err := strconv.ParseInt(strings.TrimSpace(field), 10, 64)
		if err != nil {
			t.Fatalf("invalid benchmark seed %q: %v", field, err)
		}
		seeds = append(seeds, seed)
	}
	if len(seeds) < 20 {
		t.Fatalf("matched-runtime benchmark requires at least 20 deterministic seeds; got %d", len(seeds))
	}
	return seeds
}
