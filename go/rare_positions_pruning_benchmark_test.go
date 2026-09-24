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
	GamesPerSample   float64       `json:"games_per_sample"`
	SolverDuration   time.Duration `json:"-"`
	SolverSeconds    float64       `json:"solver_seconds"`
	SolverFraction   float64       `json:"solver_fraction"`
}

type sequentialPruningBenchmarkRow struct {
	Baseline             SequentialRareEstimate `json:"baseline"`
	Pruned               SequentialRareEstimate `json:"pruned"`
	ESSRateRatio         float64                `json:"pruned_to_baseline_ess_per_second"`
	ReferenceProbability float64                `json:"reference_probability,omitempty"`
}

type sequentialPruningBenchmarkReport struct {
	GroupID              int                             `json:"group_id"`
	TeamID               int                             `json:"team_id"`
	Position             int                             `json:"target_position"`
	Threshold            float64                         `json:"interesting_probability_threshold"`
	TargetRuntimeSeconds float64                         `json:"target_runtime_seconds"`
	ChosenStride         int                             `json:"chosen_stride"`
	Calibration          []pruningCalibration            `json:"calibration"`
	BaselineSamples      int                             `json:"frozen_baseline_samples"`
	PrunedSamples        int                             `json:"frozen_pruned_samples"`
	Seeds                []int64                         `json:"seeds"`
	Rows                 []sequentialPruningBenchmarkRow `json:"rows"`
	BaselineRMSE         float64                         `json:"baseline_rmse,omitempty"`
	PrunedRMSE           float64                         `json:"pruned_rmse,omitempty"`
	RMSEratio            float64                         `json:"pruned_to_baseline_rmse,omitempty"`
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
	teams := make(map[int]bool)
	for _, team := range group.Team_groups {
		teams[team.Team_id] = true
	}
	for _, game := range group.Games {
		teams[game.HomeId], teams[game.AwayId] = true, true
	}
	teamID, position := benchmarkIntEnv(t, "RARE_POSITION_PRUNING_TEAM", 0), benchmarkIntEnv(t, "RARE_POSITION_PRUNING_POSITION", 0)
	if teamID == 0 {
		if len(group.Team_groups) == 0 {
			t.Fatal("group has no teams")
		}
		teamID = group.Team_groups[0].Team_id
	}
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
	calibrationSamples := benchmarkIntEnv(t, "RARE_POSITION_PRUNING_CALIBRATION_SAMPLES", 400)
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
	baselineSamples := maxInt(1, int(math.Round(targetRuntime.Seconds()*baselineCal.SamplesPerSecond)))
	prunedSamples := maxInt(1, int(math.Round(targetRuntime.Seconds()*bestRate)))
	seeds := benchmarkSeedList(t)
	report := sequentialPruningBenchmarkReport{GroupID: group.Id, TeamID: teamID, Position: position,
		Threshold: threshold, TargetRuntimeSeconds: targetRuntime.Seconds(), ChosenStride: bestStride,
		Calibration: calibrations, BaselineSamples: baselineSamples, PrunedSamples: prunedSamples, Seeds: seeds}
	reference, hasReference := benchmarkFloatEnv("RARE_POSITION_PRUNING_REFERENCE_PROBABILITY")
	var baselineSquared, prunedSquared float64
	for _, seed := range seeds {
		baseline := runSequentialRareEstimate(base, games, original, components, table, order,
			group.Team_groups, teamID, position, 0, baselineSamples, seed, "baseline")
		pruned := runSequentialRareEstimate(base, games, original, components, table, order,
			group.Team_groups, teamID, position, bestStride, prunedSamples, seed, "pruned")
		if math.Abs(baseline.ElapsedSeconds-pruned.ElapsedSeconds) > targetRuntime.Seconds()*0.5 {
			t.Logf("seed=%d actual runtime mismatch is over 50%% of target (baseline=%.3fs pruned=%.3fs)", seed, baseline.ElapsedSeconds, pruned.ElapsedSeconds)
		}
		row := sequentialPruningBenchmarkRow{Baseline: baseline, Pruned: pruned,
			ESSRateRatio: safeRatio(pruned.ESSPerSecond, baseline.ESSPerSecond)}
		if hasReference {
			row.ReferenceProbability = reference
			baselineSquared += square(baseline.Estimate - reference)
			prunedSquared += square(pruned.Estimate - reference)
		}
		report.Rows = append(report.Rows, row)
	}
	if hasReference {
		report.BaselineRMSE = math.Sqrt(baselineSquared / float64(len(seeds)))
		report.PrunedRMSE = math.Sqrt(prunedSquared / float64(len(seeds)))
		report.RMSEratio = safeRatio(report.PrunedRMSE, report.BaselineRMSE)
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
	t.Logf("sequential-pruning benchmark group=%d team=%d position=%d stride=%d baseline_N=%d pruned_N=%d target_runtime=%s report=%s",
		group.Id, teamID, position, bestStride, baselineSamples, prunedSamples, targetRuntime, output)
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
	for i := 0; i < samples; i++ {
		rng := rand.New(rand.NewSource(targetSampleSeed(deriveRarePositionSeed(seed, fmt.Sprintf("%s-%d", method, stride)), i)))
		if method == "baseline" {
			sim := make([]*TeamCampaign, len(base))
			teams := make([]*TeamCampaign, len(groups))
			logs, weights := make([]float64, len(components)), make([]float64, len(components))
			simulateTargetTeamRankAndWeightMulti(base, sim, teams, games, original, components,
				table, order, groups, team, rng, logs, weights, nil)
		} else {
			_, _, stats := simulateTargetSequential(base, games, original, components, table,
				order, groups, team, position, stride, false, rng)
			cal.SolverDuration += stats.SolverDuration
			cal.GamesPerSample += float64(stats.GamesSimulated)
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
		return []int64{1001}
	}
	var seeds []int64
	for _, field := range strings.Split(value, ",") {
		seed, err := strconv.ParseInt(strings.TrimSpace(field), 10, 64)
		if err != nil {
			t.Fatalf("invalid benchmark seed %q: %v", field, err)
		}
		seeds = append(seeds, seed)
	}
	if len(seeds) == 0 {
		t.Fatal("at least one benchmark seed is required")
	}
	return seeds
}
