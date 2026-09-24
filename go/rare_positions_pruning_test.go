package main

import (
	"bytes"
	"fmt"
	"log"
	"math"
	"math/rand"
	"sort"
	"strings"
	"testing"
)

func TestRarePositionEnvironmentIsPrintedWithEffectiveDefaults(t *testing.T) {
	t.Setenv("RARE_POSITION_RANDOM_SEED", "12345")
	t.Setenv("RARE_POSITION_BENCHMARK_ITERATIONS", "20000")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")
	t.Setenv("RARE_POSITION_CEM_RACING_MODE", "legacy")
	t.Setenv("RARE_POSITION_MIN_INTERESTING_PROBABILITY", "1e-8")
	t.Setenv("RARE_POSITION_SEQUENTIAL_PRUNING", "1")
	t.Setenv("RARE_POSITION_SEQUENTIAL_PRUNING_CALIBRATION_SAMPLES", "700")
	t.Setenv("RARE_POSITION_SEQUENTIAL_PRUNING_MAX_TARGET_SAMPLES", "90000")
	previousWriter := log.Writer()
	var output bytes.Buffer
	log.SetOutput(&output)
	defer log.SetOutput(previousWriter)

	logRarePositionRequestEnvironment()
	line := output.String()
	for _, expected := range []string{
		`RARE_POSITION_RANDOM_SEED="12345"`,
		`RARE_POSITION_BENCHMARK_ITERATIONS="20000"`,
		`RARE_POSITION_CEM_RACING_MODE="legacy"`,
		`effective_importance_sampling=true`,
		`effective_scout_iterations=20000`,
		`effective_cem_racing_mode=legacy`,
		`RARE_POSITION_SEQUENTIAL_PRUNING="1"`,
		`effective_sequential_pruning=true`,
		`effective_sequential_pruning_calibration_samples=700`,
		`effective_sequential_pruning_max_target_samples=90000`,
		`effective_min_interesting_probability=1e-08`,
	} {
		if !strings.Contains(line, expected) {
			t.Errorf("environment log does not contain %q: %s", expected, line)
		}
	}
}

func TestSequentialPruningProductionPlannerRespectsGlobalWorkBudget(t *testing.T) {
	plan := planSequentialPruningProduction(1100000, 100, 500, 1000, 2000,
		22, 44, 12, 2, 100000, 0.5)
	if !plan.Enabled || plan.CalibrationSamples != 500 || plan.TargetSamples <= 0 || plan.AllRankSamples <= 0 {
		t.Fatalf("unexpected production split: %+v", plan)
	}
	if plan.TotalWork != plan.CalibrationWork+plan.TargetWork+plan.AllRankWork || plan.TotalWork > 1100000 {
		t.Fatalf("planned production exceeds or misaccounts budget: %+v", plan)
	}
	if plan.CalibrationWork != 4*500*100 {
		t.Fatalf("calibration work=%d, want four fixed 500-sample calibrations", plan.CalibrationWork)
	}
	baselineSamplesAtSameRuntime := int(plan.ExpectedTargetSeconds * 1000)
	if plan.Speedup != 2 || plan.TargetSamples < int(1.5*float64(baselineSamplesAtSameRuntime)) {
		t.Fatalf("runtime calibration did not translate speedup into target samples: %+v", plan)
	}
	if capped := planSequentialPruningProduction(1100000, 100, 500, 1000, 2000,
		22, 44, 12, 2, 10, 0.5); !capped.Enabled || capped.TargetSamples > 10 {
		t.Fatalf("hard target sample cap was not applied: %+v", capped)
	}
	if noSpeedup := planSequentialPruningProduction(1100000, 100, 500, 1000, 950,
		22, 44, 12, 2, 100000, 0.5); noSpeedup.Enabled {
		t.Fatalf("pruning was enabled despite slower measured throughput: %+v", noSpeedup)
	}
	if insufficient := planSequentialPruningProduction(1000, 100, 500, 1000, 2000,
		22, 44, 12, 2, 100000, 0.5); insufficient.Enabled {
		t.Fatalf("pruning should not activate when fixed calibration cannot be budgeted: %+v", insufficient)
	}
}

func TestSequentialPruningDisabledUnlessExplicitlyEnabled(t *testing.T) {
	t.Setenv("RARE_POSITION_SEQUENTIAL_PRUNING", "")
	if sequentialPruningEnabled() {
		t.Fatal("pruning unexpectedly enabled when its environment variable is unset")
	}
	t.Setenv("RARE_POSITION_SEQUENTIAL_PRUNING", "1")
	if !sequentialPruningEnabled() {
		t.Fatal("pruning did not enable for explicit value 1")
	}
}

func TestSequentialPruningCalibrationAndSampleCapsHaveSafeDefaults(t *testing.T) {
	t.Setenv("RARE_POSITION_SEQUENTIAL_PRUNING_CALIBRATION_SAMPLES", "")
	t.Setenv("RARE_POSITION_SEQUENTIAL_PRUNING_MAX_TARGET_SAMPLES", "")
	if got := sequentialPruningCalibrationSampleCount(); got != 500 {
		t.Fatalf("default calibration samples=%d, want 500", got)
	}
	if got := sequentialPruningTargetSampleCap(); got != DefaultSequentialPruningTargetSampleCap {
		t.Fatalf("default target cap=%d, want %d", got, DefaultSequentialPruningTargetSampleCap)
	}
	t.Setenv("RARE_POSITION_SEQUENTIAL_PRUNING_CALIBRATION_SAMPLES", "9000")
	if got := sequentialPruningCalibrationSampleCount(); got != MaxSequentialPruningCalibrationSamples {
		t.Fatalf("calibration sample override escaped safety cap: got %d want %d", got, MaxSequentialPruningCalibrationSamples)
	}
}

func TestRuntimePlanningBuysSamplesFromMeasuredSpeedup(t *testing.T) {
	allRank, target := runtimeDerivedSampleCounts(10, 0.20, 1000, 2000, 100000)
	baselineAtTargetRuntime := int(float64(target) / 2)
	if allRank != 8000 || target != 4000 || target != 2*baselineAtTargetRuntime {
		t.Fatalf("runtime-derived counts do not convert 2x speedup into 2x target samples: all_rank=%d target=%d baseline_target_equivalent=%d",
			allRank, target, baselineAtTargetRuntime)
	}
	if _, capped := runtimeDerivedSampleCounts(10, 0.20, 1000, 2000, 123); capped != 123 {
		t.Fatalf("runtime-derived target count ignored hard cap: got %d want 123", capped)
	}
}

func TestEventSufficientStatsPoolingMatchesConcatenatedObservations(t *testing.T) {
	weights := []float64{0.125, 0, 0.5, 0.25, 0, 0.125, 0.25, 0}
	events := []bool{true, false, true, true, false, true, false, false}
	var left, right, combined EventSufficientStats
	for i, weight := range weights {
		combined.observe(weight, events[i])
		if i < len(weights)/2 {
			left.observe(weight, events[i])
		} else {
			right.observe(weight, events[i])
		}
	}
	pooled := left.pooled(right)
	if pooled != combined {
		t.Fatalf("pooled sufficient stats differ from concatenated observations: pooled=%+v combined=%+v", pooled, combined)
	}
	pooledEstimate := productionEstimateFromSufficientStats(pooled, 100, "pooled")
	combinedEstimate := productionEstimateFromSufficientStats(combined, 100, "combined")
	if pooledEstimate.Probability != combinedEstimate.Probability || pooledEstimate.StdErr != combinedEstimate.StdErr ||
		pooledEstimate.ESS != combinedEstimate.ESS || pooledEstimate.Hits != combinedEstimate.Hits ||
		pooledEstimate.MaxEventWeightShare != combinedEstimate.MaxEventWeightShare {
		t.Fatalf("pooled estimate differs from one combined batch: pooled=%+v combined=%+v", pooledEstimate, combinedEstimate)
	}
}

func TestPrunedSequentialSamplesAreCountedAsZeroObservations(t *testing.T) {
	base, games, original, groups, table, order := pruningFixture(t)
	components := []ProposalComponent{{Name: "P", Weight: 0.05, Means: original},
		{Name: "Q", Weight: 0.95, Means: original}}
	const samples = 1200
	estimate := runSequentialRareEstimate(base, games, original, components, table, order,
		groups, 1, 0, 1, samples, 99127, "pruned")
	if estimate.Pruned == 0 || estimate.SufficientStats.Samples != samples || estimate.Samples != samples {
		t.Fatalf("pruned observations were not included in the fixed denominator: %+v", estimate)
	}
	if estimate.SufficientStats.WeightSamples != samples-estimate.Pruned {
		t.Fatalf("pruned partial weights leaked into estimator diagnostics: stats=%+v pruned=%d", estimate.SufficientStats, estimate.Pruned)
	}
	if estimate.SufficientStats.Hits != estimate.RawHits || estimate.SufficientStats.SumY <= 0 || estimate.SufficientStats.SumY2 <= 0 {
		t.Fatalf("sequential event sufficient statistics are inconsistent: %+v estimate=%+v", estimate.SufficientStats, estimate)
	}
}

func TestIndependentAllRankAndSequentialStreamsPoolWithoutReplacingTargetEvidence(t *testing.T) {
	base, games, original, groups, table, order := pruningFixture(t)
	components := []ProposalComponent{{Name: "P", Weight: 0.05, Means: original},
		{Name: "Q", Weight: 0.95, Means: original}}
	const allRankN, sequentialN = 400, 300
	job := &RareSimulationJob{TeamID: groups[0].Team_id, CandidatePositions: []int{0},
		Components: components, Iterations: allRankN, CollectAllRanks: true}
	_, _ = estimateRarePositionsForJob(base, games, original, table, order, groups, job,
		rand.New(rand.NewSource(901)), 16982)
	allRank, ok := job.FullRankEstimates[groups[0].Team_id][0]
	if !ok {
		t.Fatal("all-rank production stream did not preserve target sufficient statistics")
	}
	sequential := runSequentialRareEstimate(base, games, original, components, table, order,
		groups, groups[0].Team_id, 0, 1, sequentialN, 902, "sequential_pruned_production")
	pooled := productionEstimateFromSufficientStats(allRank.SufficientStats.pooled(sequential.SufficientStats),
		allRank.WorkSpent, "pooled")
	if pooled.Samples != allRankN+sequentialN || pooled.Hits != allRank.Hits+sequential.RawHits {
		t.Fatalf("pooled estimator dropped one of its independent streams: all_rank=%d/%d sequential=%d/%d pooled=%d/%d",
			allRank.Samples, allRank.Hits, sequential.Samples, sequential.RawHits, pooled.Samples, pooled.Hits)
	}
	wantProbability := (allRank.SufficientStats.SumY + sequential.SufficientStats.SumY) / float64(allRankN+sequentialN)
	if pooled.Probability != wantProbability || pooled.ESS != pooled.SufficientStats.SumY*pooled.SufficientStats.SumY/pooled.SufficientStats.SumY2 {
		t.Fatalf("pooled estimator was not derived from raw sufficient statistics: pooled=%+v", pooled)
	}
}

func pruningFixture(t *testing.T) ([]*TeamCampaign, []*GameType, []GameProposalMeans, []TeamType, *Table, []SortType) {
	t.Helper()
	teams := []TeamType{{Team_id: 1, Bias: 2}, {Team_id: 2, Bias: 0}, {Team_id: 3, Bias: 1}, {Team_id: 4, Bias: 3}}
	keys := []uint32{1, 2, 3, 4}
	table := NewTable(keys)
	campaign := make([]*TeamCampaign, len(keys))
	for _, team := range teams {
		campaign[table.Query(uint32(team.Team_id))] = &TeamCampaign{id: team.Team_id, bias: team.Bias,
			points_win: 3, points_draw: 1, points_loss: 0}
	}
	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2, HomePower: 0.8, AwayPower: 1.1},
		{Id: 2, HomeId: 3, AwayId: 4, HomePower: 1.2, AwayPower: 0.7},
		{Id: 3, HomeId: 1, AwayId: 3, HomePower: 0.9, AwayPower: 1.0},
		{Id: 4, HomeId: 2, AwayId: 4, HomePower: 1.1, AwayPower: 0.9},
		{Id: 5, HomeId: 1, AwayId: 4, HomePower: 0.85, AwayPower: 1.0},
		{Id: 6, HomeId: 2, AwayId: 3, HomePower: 1.0, AwayPower: 1.05},
	}
	for _, game := range games {
		game.home_table_index = table.Query(uint32(game.HomeId))
		game.away_table_index = table.Query(uint32(game.AwayId))
	}
	means := make([]GameProposalMeans, len(games))
	for i, game := range games {
		means[i] = GameProposalMeans{Home: game.HomePower, Away: game.AwayPower}
	}
	return campaign, games, means, teams, table, []SortType{PT, GD, GF, BIAS}
}

func TestIncrementalPositionBoundsMatchConservativeBoundsOnRandomPrefixes(t *testing.T) {
	base, games, _, groups, table, _ := pruningFixture(t)
	for seed := int64(1); seed <= 30; seed++ {
		rng := rand.New(rand.NewSource(seed))
		campaign := make([]*TeamCampaign, len(base))
		for i, c := range base {
			campaign[i] = c.clone()
		}
		prefixGames := make([]*GameType, len(games))
		for i, game := range games {
			copy := *game
			prefixGames[i] = &copy
		}
		state := newIncrementalPositionBounds(prefixGames, groups)
		check := func(prefix int) {
			for target := range groups {
				wantBest, wantWorst, wantUnplayed := conservativePositionBounds(groups[target].Team_id, campaign,
					groups, prefixGames, table)
				gotBest, gotWorst, gotUnplayed := state.ranks(groups[target].Team_id, campaign, table)
				if gotBest != wantBest || gotWorst != wantWorst || gotUnplayed != wantUnplayed {
					t.Fatalf("seed=%d prefix=%d target=%d incremental=(%d,%d,%t) conservative=(%d,%d,%t)",
						seed, prefix, groups[target].Team_id, gotBest, gotWorst, gotUnplayed,
						wantBest, wantWorst, wantUnplayed)
				}
			}
		}
		check(0)
		for i, game := range prefixGames {
			home, away := poissonRand(rng, game.HomePower), poissonRand(rng, game.AwayPower)
			completed := &GameType{Id: game.Id, HomeId: game.HomeId, AwayId: game.AwayId,
				HomeScore: home, AwayScore: away, Played: true,
				home_table_index: game.home_table_index, away_table_index: game.away_table_index}
			campaign[game.home_table_index].add_game(completed)
			campaign[game.away_table_index].add_game(completed)
			game.Played = true
			state.gameCompleted(game)
			check(i + 1)
		}
	}
}

func testFullSample(t *testing.T, base []*TeamCampaign, games []*GameType, original []GameProposalMeans,
	components []ProposalComponent, table *Table, order []SortType, groups []TeamType,
	targetTeam int, seed int64) (rank int, weight float64, signature uint64) {
	t.Helper()
	rng := rand.New(rand.NewSource(seed))
	chosen := 0
	value, sum := rng.Float64(), 0.0
	for i, component := range components {
		sum += component.Weight
		if value <= sum || i == len(components)-1 {
			chosen = i
			break
		}
	}
	logs := make([]float64, len(components))
	campaign := make([]*TeamCampaign, len(base))
	for i, c := range base {
		campaign[i] = c.clone()
	}
	for i, game := range games {
		home := poissonRand(rng, components[chosen].Means[i].Home)
		away := poissonRand(rng, components[chosen].Means[i].Away)
		signature = (signature ^ uint64(uint32(game.Id))*0x9e3779b185ebca87 ^
			uint64(uint32(home))<<32 ^ uint64(uint32(away))) * 0x100000001b3
		for k, component := range components {
			logs[k] += logPoissonQOverP(home, original[i].Home, component.Means[i].Home)
			logs[k] += logPoissonQOverP(away, original[i].Away, component.Means[i].Away)
		}
		completed := &GameType{Id: game.Id, HomeId: game.HomeId, AwayId: game.AwayId,
			HomeScore: home, AwayScore: away, Played: true,
			home_table_index: game.home_table_index, away_table_index: game.away_table_index}
		campaign[game.home_table_index].add_game(completed)
		campaign[game.away_table_index].add_game(completed)
	}
	ordered := make([]*TeamCampaign, 0, len(groups))
	for _, team := range groups {
		ordered = append(ordered, campaign[table.Query(uint32(team.Team_id))])
	}
	sort.Sort(TeamCampaignSorted{t: ordered, sort: order, rng: rng})
	rank = -1
	for pos, team := range ordered {
		if team.id == targetTeam {
			rank = pos
			break
		}
	}
	weights := make([]float64, len(components))
	for i, component := range components {
		weights[i] = component.Weight
	}
	return rank, mixtureImportanceWeightMulti(logs, weights), signature
}

func TestSequentialPruningIsZeroContributionAndPreservesPairedSamples(t *testing.T) {
	base, games, original, groups, table, order := pruningFixture(t)
	shifted := append([]GameProposalMeans(nil), original...)
	for i := range shifted {
		shifted[i].Home *= 1.6
		shifted[i].Away *= 0.7
	}
	components := []ProposalComponent{
		{Name: "P", Weight: 0.05, Means: original},
		{Name: "Q", Weight: 0.95, Means: shifted},
	}
	pruned := 0
	for index := 0; index < 500; index++ {
		seed := targetSampleSeed(9001, index)
		wantRank, wantWeight, wantSignature := testFullSample(t, base, games, original, components, table, order, groups, 1, seed)
		rank, weight, stats := simulateTargetSequential(base, games, original, components,
			table, order, groups, 1, 0, 1, true, rand.New(rand.NewSource(seed)))
		if stats.Pruned {
			pruned++
			if rank != -1 || weight != 0 || wantRank == 0 {
				t.Fatalf("pruned sample was not a proven zero: stats=%+v rank=%d weight=%g baseline_rank=%d", stats, rank, weight, wantRank)
			}
			continue
		}
		if rank != wantRank || weight != wantWeight || stats.ScorelineSignature != wantSignature {
			t.Fatalf("unpruned sample differs: got rank=%d weight=%.17g signature=%x want rank=%d weight=%.17g signature=%x",
				rank, weight, stats.ScorelineSignature, wantRank, wantWeight, wantSignature)
		}
	}
	if pruned == 0 {
		t.Fatal("fixture produced no mathematically prunable samples")
	}
}

func TestRareProbabilityThresholdClassificationDoesNotFloorEstimate(t *testing.T) {
	threshold := 1e-8
	t.Setenv("RARE_POSITION_MIN_INTERESTING_PROBABILITY", "1e-8")
	tests := []struct {
		p, se float64
		want  rarePositionProbabilityClass
	}{
		{1.1e-8, 1e-9, ProbabilityAtOrAboveThreshold},
		{0.8e-8, 0.2e-8, ProbabilityUncertainAtThreshold},
		{0.5e-8, 0.1e-8, ProbabilityBelowThreshold},
	}
	for _, test := range tests {
		if got := classifyRareProbability(test.p, test.se, threshold); got != test.want {
			t.Errorf("classifyRareProbability(%g,%g)=%s want %s", test.p, test.se, got, test.want)
		}
	}
	if got := classifyRareProbabilityWithUpper(0, 1e-4, threshold); got != ProbabilityUncertainAtThreshold {
		t.Fatalf("zero-hit estimate with a wide upper bound was labeled %s", got)
	}
	if got := classifyRareProbabilityWithUpper(0.4e-8, 0.9e-8, threshold); got != ProbabilityBelowThreshold {
		t.Fatalf("estimate whose upper confidence bound is below threshold was labeled %s", got)
	}
	if !estimateMayMeetInterestingThreshold(RarePositionEstimate{Probability: 0.8e-8, StdErr: 0.2e-8}, threshold) {
		t.Fatal("uncertain estimate was discarded despite its confidence interval overlapping the threshold")
	}
	if estimateMayMeetInterestingThreshold(RarePositionEstimate{Probability: 0.5e-8, StdErr: 0.1e-8}, threshold) {
		t.Fatal("estimate with upper confidence bound below threshold was retained")
	}
	merged := mergeRarePositionEstimates([]float64{1, 0}, []int{10, 0}, map[int]RarePositionEstimate{
		1: {Probability: 0.8e-8, StdErr: 0.2e-8, Available: true},
	})
	if merged[1] != 0.8e-8 {
		t.Fatalf("uncertain raw probability was discarded or floored: merged=%g", merged[1])
	}
	below := mergeRarePositionEstimates([]float64{1, 0}, []int{10, 0}, map[int]RarePositionEstimate{
		1: {Probability: 0.5e-8, StdErr: 0.1e-8, Available: true},
	})
	if below[1] != 0 {
		t.Fatalf("estimate whose upper bound is below threshold was merged: %g", below[1])
	}
}

func TestTargetEstimatorsUseSameFixedSampleIndicesAndPruningOnlySavesWork(t *testing.T) {
	base, games, original, groups, table, order := pruningFixture(t)
	components := []ProposalComponent{{Name: "P", Weight: 0.05, Means: original},
		{Name: "Q", Weight: 0.95, Means: original}}
	const samples = 1200
	for seedIndex := int64(0); seedIndex < 20; seedIndex++ {
		seed := deriveRarePositionSeed(7741, fmt.Sprintf("paired-pruning-test-%d", seedIndex))
		baseline := runSequentialRareEstimate(base, games, original, components, table, order,
			groups, 1, 0, 0, samples, seed, "baseline")
		pruned := runSequentialRareEstimate(base, games, original, components, table, order,
			groups, 1, 0, 1, samples, seed, "pruned")
		if baseline.Samples != samples || pruned.Samples != samples {
			t.Fatalf("seed=%d sample counts changed after start: baseline=%d pruned=%d", seedIndex, baseline.Samples, pruned.Samples)
		}
		if baseline.Estimate != pruned.Estimate || baseline.StdErr != pruned.StdErr || baseline.ESS != pruned.ESS ||
			baseline.RawHits != pruned.RawHits || baseline.SufficientStats.SumY != pruned.SufficientStats.SumY ||
			baseline.SufficientStats.SumY2 != pruned.SufficientStats.SumY2 {
			t.Fatalf("seed=%d paired estimates diverged despite zero-contribution pruning: baseline=%+v pruned=%+v", seedIndex, baseline, pruned)
		}
		if pruned.Pruned == 0 || pruned.GamesSimulated >= baseline.GamesSimulated {
			t.Fatalf("seed=%d pruning did not save game work: baseline=%+v pruned=%+v", seedIndex, baseline, pruned)
		}
	}
}

func TestSequentialPruningDoesNotEarlyAcceptGuaranteedEvent(t *testing.T) {
	groups := []TeamType{{Team_id: 1}, {Team_id: 2}, {Team_id: 3}}
	table := NewTable([]uint32{1, 2, 3})
	base := make([]*TeamCampaign, 3)
	base[table.Query(1)] = &TeamCampaign{id: 1, points: 100, points_win: 3, points_draw: 1, points_loss: 0}
	base[table.Query(2)] = &TeamCampaign{id: 2, points_win: 3, points_draw: 1, points_loss: 0}
	base[table.Query(3)] = &TeamCampaign{id: 3, points_win: 3, points_draw: 1, points_loss: 0}
	games := []*GameType{{Id: 1, HomeId: 2, AwayId: 3, HomePower: 1, AwayPower: 1},
		{Id: 2, HomeId: 1, AwayId: 2, HomePower: 1, AwayPower: 1},
		{Id: 3, HomeId: 1, AwayId: 3, HomePower: 1, AwayPower: 1}}
	for _, game := range games {
		game.home_table_index, game.away_table_index = table.Query(uint32(game.HomeId)), table.Query(uint32(game.AwayId))
	}
	means := make([]GameProposalMeans, len(games))
	for i, game := range games {
		means[i] = GameProposalMeans{Home: game.HomePower, Away: game.AwayPower}
	}
	_, _, stats := simulateTargetSequential(base, games, means,
		[]ProposalComponent{{Name: "P", Weight: 1, Means: means}}, table, []SortType{PT, BIAS}, groups,
		1, 0, 1, false, rand.New(rand.NewSource(3)))
	if stats.Pruned || stats.GamesSimulated != len(games) || stats.CompletedRank != 0 {
		t.Fatalf("guaranteed exact event was early-accepted instead of fully simulated: %+v", stats)
	}
}

func TestSyntheticExactRarePositionProbabilityNearOneE8(t *testing.T) {
	// Team 1 must win both matches to finish first: teams 2 and 3 each start
	// three points ahead, and lower bias loses any points tie. Each home win is
	// approximately 1e-4 under independent Poisson(1e-4) scores.
	teams := []TeamType{{Team_id: 1, Bias: 0}, {Team_id: 2, Bias: 1}, {Team_id: 3, Bias: 2}}
	table := NewTable([]uint32{1, 2, 3})
	base := make([]*TeamCampaign, 3)
	for _, team := range teams {
		points := 3
		if team.Team_id == 1 {
			points = 0
		}
		base[table.Query(uint32(team.Team_id))] = &TeamCampaign{id: team.Team_id, bias: team.Bias,
			points: points, points_win: 3, points_draw: 1, points_loss: 0}
	}
	games := []*GameType{{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1e-4, AwayPower: 1e-4},
		{Id: 2, HomeId: 1, AwayId: 3, HomePower: 1e-4, AwayPower: 1e-4}}
	for _, game := range games {
		game.home_table_index, game.away_table_index = table.Query(uint32(game.HomeId)), table.Query(uint32(game.AwayId))
	}
	finishRank := func(scores [][2]int) int {
		campaign := make([]*TeamCampaign, len(base))
		for i, team := range base {
			campaign[i] = team.clone()
		}
		for i, game := range games {
			completed := &GameType{Id: game.Id, HomeId: game.HomeId, AwayId: game.AwayId,
				HomeScore: scores[i][0], AwayScore: scores[i][1], Played: true,
				home_table_index: game.home_table_index, away_table_index: game.away_table_index}
			campaign[game.home_table_index].add_game(completed)
			campaign[game.away_table_index].add_game(completed)
		}
		ordered := []*TeamCampaign{campaign[table.Query(1)], campaign[table.Query(2)], campaign[table.Query(3)]}
		sort.Sort(TeamCampaignSorted{t: ordered, sort: []SortType{PT, BIAS}})
		for position, team := range ordered {
			if team.id == 1 {
				return position
			}
		}
		return -1
	}
	if finishRank([][2]int{{1, 0}, {1, 0}}) != 0 || finishRank([][2]int{{1, 0}, {0, 1}}) == 0 {
		t.Fatal("synthetic exact-rank fixture does not require wins in both target matches")
	}
	mu := 1e-4
	homeWin := 0.0
	for home := 1; home <= 12; home++ {
		awayLess := 0.0
		for away := 0; away < home; away++ {
			awayLess += poisson_pmf(mu, float64(away))
		}
		homeWin += poisson_pmf(mu, float64(home)) * awayLess
	}
	probability := homeWin * homeWin
	enumeratedRankProbability := 0.0
	for h1 := 0; h1 <= 12; h1++ {
		for a1 := 0; a1 <= 12; a1++ {
			for h2 := 0; h2 <= 12; h2++ {
				for a2 := 0; a2 <= 12; a2++ {
					if finishRank([][2]int{{h1, a1}, {h2, a2}}) == 0 {
						enumeratedRankProbability += poisson_pmf(mu, float64(h1)) * poisson_pmf(mu, float64(a1)) *
							poisson_pmf(mu, float64(h2)) * poisson_pmf(mu, float64(a2))
					}
				}
			}
		}
	}
	if math.Abs(enumeratedRankProbability-probability) > 1e-20 {
		t.Fatalf("direct rank enumeration=%g disagrees with independent product=%g", enumeratedRankProbability, probability)
	}
	if probability < 0.9e-8 || probability > 1.1e-8 {
		t.Fatalf("independently enumerated exact rank probability=%g, want approximately 1e-8", probability)
	}
	if math.Abs(probability-1e-8) > 2e-10 {
		t.Fatalf("Poisson truncation error was not negligible: p=%g", probability)
	}
	// For k>=13, each succeeding Poisson PMF term has ratio <= mu/14, so
	// p(X>=13) <= pmf(13)/(1-mu/14), far below 1e-50 for mu=1e-4.
	tailBound := poisson_pmf(mu, 13) / (1 - mu/14)
	if 2*tailBound > 1e-50 {
		t.Fatalf("omitted two-game score tail bound=%g is not negligible", 2*tailBound)
	}
}
