package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"math"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"testing"
	"time"
)

// Exhaust every outcome in small coupled fixture graphs. Points/wins ties
// must retain support for either rank side, including phases without wins.
func TestRankDomainsRetainEveryPossibleRankAssignment(t *testing.T) {
	for _, stride := range []int{1, 20} {
		games := []conditionedPointOutcomeGame{
			{home: 1, away: 2, prob: [3]float64{0.3, 0.2, 0.5}},
			{home: 2, away: 3, prob: [3]float64{0.3, 0.2, 0.5}},
			{home: 3, away: 1, prob: [3]float64{0.3, 0.2, 0.5}},
		}
		for i := range games {
			games[i].index = i
			games[i].homeGain = [3]int{0, stride, 3 * stride}
			games[i].awayGain = [3]int{3 * stride, stride, 0}
			if stride > 1 {
				games[i].homeGain[2]++
				games[i].awayGain[0]++
			}
		}
		for _, base := range [][]int{{0, 0, 0, 0}, {0, 2 * stride, 4 * stride, 6 * stride}, {0, stride + 2, 3*stride + 4, 2*stride + 1}} {
			for target := 0; target <= 12*stride; target++ {
				for rank := 0; rank <= 3; rank++ {
					domains := propagateConditionedRankDomains(games, base, []int32{1, 2, 3}, rank, target)
					for assignment := 0; assignment < 27; assignment++ {
						code := assignment
						outcomes := make([]int, len(games))
						final := append([]int(nil), base...)
						for i, game := range games {
							outcomes[i], code = code%3, code/3
							final[game.home] += game.homeGain[outcomes[i]]
							final[game.away] += game.awayGain[outcomes[i]]
						}
						above, below := 0, 0
						for _, score := range final[1:] {
							if score > target {
								above++
							}
							if score < target {
								below++
							}
						}
						if above > rank || below > 3-rank {
							continue
						}
						if !domains.feasible {
							t.Fatalf("stride=%d rank=%d target=%d valid assignment=%v rejected", stride, rank, target, outcomes)
						}
						prefix := append([]int(nil), base...)
						for step, game := range games {
							if !domains.allows(step, game, outcomes[step], prefix) {
								t.Fatalf("stride=%d rank=%d target=%d valid assignment=%v step=%d pruned", stride, rank, target, outcomes, step)
							}
							prefix[game.home] += game.homeGain[outcomes[step]]
							prefix[game.away] += game.awayGain[outcomes[step]]
						}
					}
				}
			}
		}
	}
}

func TestRankDomainsForceNecessaryRivalWin(t *testing.T) {
	// Two rivals are certainly below; the third must reach the target. A
	// draw cannot reach 51, so the only remaining outcome is its win.
	games := []conditionedPointOutcomeGame{{home: 1, away: 2,
		prob: [3]float64{0.2, 0.3, 0.5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}}}
	domains := propagateConditionedRankDomains(games, []int{51, 49, 30, 40}, []int32{1, 2, 3}, 1, 51)
	if !domains.feasible || domains.domains[0] != 1<<2 {
		t.Fatalf("necessary rival win not transferred: %+v", domains)
	}
}

func TestRankDomainConfirmationRequiresValidEvidence(t *testing.T) {
	valid := conditionedZeroResult{weighted: true, probability: 1e-18, stdErr: 1e-19,
		hits: 100, ess: 50, maxWeightShare: 0.1, batchGap: 0.2}
	invalid := valid
	invalid.ess = 2
	if _, accepted := conditionedDomainConfirmation(invalid, conditionedZeroResult{}); accepted {
		t.Fatal("an inconclusive check cannot accept an invalid selected estimate")
	}
	if result, accepted := conditionedDomainConfirmation(invalid, valid); !accepted || result.ess != valid.ess {
		t.Fatal("independent valid check should be usable")
	}
	contradiction := valid
	contradiction.probability, contradiction.stdErr, contradiction.batchGap = 1e-14, 1e-15, 1.5
	if _, accepted := conditionedDomainConfirmation(valid, contradiction); accepted {
		t.Fatal("conclusive but unstable contradictory check should reject the selected estimate")
	}
}

func TestRankDomainRescueDefaultAndOverride(t *testing.T) {
	t.Setenv("RARE_POSITION_PROPAGATED_DOMAINS", "")
	if !conditionedDomainRescueEnabled() {
		t.Fatal("domain rescue should be enabled by default")
	}
	t.Setenv("RARE_POSITION_PROPAGATED_DOMAINS", "0")
	if conditionedDomainRescueEnabled() {
		t.Fatal("explicit opt-out should disable domain rescue")
	}
}

func TestRankDomainEligibilityUsesWinsOnlyInAllowedPrefix(t *testing.T) {
	group, campaign, table, _, _ := createTestGroupForDiversified()
	group.Games = nil
	for _, team := range campaign {
		team.points, team.wins = 10, 1
	}
	campaign[table.Query(2)].wins = 5
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	event := &conditionedPointEvent{mass: 1, terminal: []conditionedPointTerminal{{}}}
	t.Setenv("RARE_POSITION_WIN_AWARE_RANK", "1")
	if !conditionedDomainUniformSide(event, conditionedZeroCell{1, 1}, group, campaign, table, []SortType{PT, W, GD}, bounds) {
		t.Fatal("wins should occupy the necessary above-rank slot in a points/wins phase")
	}
	if conditionedDomainUniformSide(event, conditionedZeroCell{1, 1}, group, campaign, table, []SortType{PT, GD, W}, bounds) {
		t.Fatal("wins must not occupy that slot when goal difference comes first")
	}
}

func TestRankDomainsSavedFortaleza(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set the saved current group 16653 request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if fmt.Sprintf("%x", sha256.Sum256(data)) != "71d4fea8502c3086b8787c764a94b5f8a5caa6549300f2a7687ead8897e11aec" {
		t.Skip("different input")
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	group := cloneGroupForBenchmark(input)
	campaign, table, order := diversifiedBenchmarkSetup(group)
	problem := newJointPointCapProblem(group, campaign)
	points := append([]int(nil), problem.base...)
	target, rival := table.Query(22), table.Query(95)
	var games []conditionedPointOutcomeGame
	for _, game := range problem.games {
		if game.home == target {
			points[game.home] += game.homeGain[0]
			points[game.away] += game.awayGain[0]
			continue
		}
		if game.away == target {
			points[game.home] += game.homeGain[2]
			points[game.away] += game.awayGain[2]
			continue
		}
		games = append(games, conditionedPointOutcomeGame{index: game.index, home: game.home, away: game.away, prob: game.prob, homeGain: game.homeGain, awayGain: game.awayGain})
	}
	var rivals []int32
	for index := range points {
		if int32(index) != target {
			rivals = append(rivals, int32(index))
		}
	}
	domains := propagateConditionedRankDomains(games, points, rivals, 17, points[target])
	if !domains.feasible {
		t.Fatal("Fortaleza's loss-only target assignment should remain feasible")
	}
	forced := 0
	for index, game := range games {
		if game.home == rival {
			if domains.domains[index] != 1<<2 {
				t.Fatal("Londrina home win not forced")
			}
			forced++
		}
		if game.away == rival {
			if domains.domains[index] != 1<<0 {
				t.Fatal("Londrina away win not forced")
			}
			forced++
		}
	}
	if forced != 8 {
		t.Fatalf("forced %d Londrina wins, want 8", forced)
	}
	if len(domains.forced) != 8 || len(domains.variable) != len(games)-8 {
		t.Fatalf("forced wins were not compacted: fixed=%d variable=%d remaining=%d", len(domains.forced), len(domains.variable), len(games))
	}
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	event, ok := buildConditionedPointEvent(22, 17, nil, group, campaign, table, bounds)
	if !ok || !conditionedDomainUniformSide(event, conditionedZeroCell{22, 17}, group, campaign, table, order, bounds) {
		t.Fatal("Fortaleza 18th should qualify for the bounded rescue")
	}
	event, ok = buildConditionedPointEvent(125, 12, nil, group, campaign, table, bounds)
	if !ok || conditionedDomainUniformSide(event, conditionedZeroCell{125, 12}, group, campaign, table, order, bounds) {
		t.Fatal("mixed target-total modes should remain in the existing general search")
	}
	t.Setenv("RARE_POSITION_RANDOM_SEED", "808")
	t.Setenv("RARE_POSITION_BENCHMARK_ITERATIONS", "20000")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "0")
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_PROPAGATED_DOMAINS", "0")
	baseline := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	t.Setenv("RARE_POSITION_PROPAGATED_DOMAINS", "")
	variant := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	est := variant[22][17]
	if baseline[22][17].Probability != 0 || est.Probability <= 0 || est.Design != "matched_point_pool_conditioned_rank_domains" || est.ESS < 8 || est.ConditionalHits < 30 {
		t.Fatalf("Fortaleza 18th baseline=%+v variant=%+v", baseline[22][17], est)
	}
	for team, row := range baseline {
		for rank, est := range row {
			if est.Probability > 0 && variant[team][rank].Probability == 0 {
				t.Fatalf("lost team=%d rank=%d", team, rank+1)
			}
		}
	}
}

func TestRankDomainImportanceWeightsMatchPlainSampling(t *testing.T) {
	t.Setenv("RARE_POSITION_PROPAGATED_DOMAINS", "1")
	group, campaign, table, _, _ := createTestGroupForDiversified()
	order := []SortType{PT, W, GD, GF, BIAS}
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	samplers := newConditionedScoreSamplers(group.Games)
	for _, team := range group.Team_groups {
		for rank := range group.Team_groups {
			event, ok := buildConditionedPointEvent(team.Team_id, rank, nil, group, campaign, table, bounds)
			if !ok || event.mass <= 0 {
				continue
			}
			const draws = 30000
			plain := sampleConditionedZeroCell(event, team.Team_id, rank, group, campaign, table, order, samplers, draws, 4301+int64(rank))
			weighted, _ := sampleConditionedZeroRankWithDomains(event, team.Team_id, rank, group, campaign, table, order, bounds, samplers, draws, 5101+int64(rank), 3, 0.5)
			want := event.mass * float64(plain.hits) / draws
			plainSE := event.mass * math.Sqrt(float64(plain.hits)/draws*(1-float64(plain.hits)/draws)/draws)
			if math.Abs(weighted.probability-want) > 6*math.Hypot(plainSE, weighted.stdErr)+1e-5 {
				t.Fatalf("team=%d rank=%d weighted=%g plain=%g", team.Team_id, rank+1, weighted.probability, want)
			}
		}
	}
}

// Paired full production requests at the existing budget; raw exports stay in
// the ignored local experiment directory. Opt in with absolute input/output.
func TestPropagatedDomainsFullRequestExperiment(t *testing.T) {
	paths, output := os.Getenv("RARE_POSITION_DOMAIN_EXPERIMENT_REQUESTS"), os.Getenv("RARE_POSITION_DOMAIN_EXPERIMENT_OUTPUT")
	runRankSamplerFullRequestExperiment(t, paths, output, os.Getenv("RARE_POSITION_DOMAIN_EXPERIMENT_SEEDS"), "RARE_POSITION_PROPAGATED_DOMAINS")
}

func TestCompactForcedFullRequestExperiment(t *testing.T) {
	paths, output := os.Getenv("RARE_POSITION_COMPACT_EXPERIMENT_REQUESTS"), os.Getenv("RARE_POSITION_COMPACT_EXPERIMENT_OUTPUT")
	runRankSamplerFullRequestExperiment(t, paths, output, os.Getenv("RARE_POSITION_COMPACT_EXPERIMENT_SEEDS"), "RARE_POSITION_COMPACT_FORCED_FIXTURES")
}

func TestCompactZeroGuideFullRequestExperiment(t *testing.T) {
	runRankSamplerFullRequestExperiment(t, os.Getenv("RARE_POSITION_COMPACT_EXPERIMENT_REQUESTS"),
		os.Getenv("RARE_POSITION_COMPACT_EXPERIMENT_OUTPUT"), os.Getenv("RARE_POSITION_COMPACT_EXPERIMENT_SEEDS"),
		"RARE_POSITION_COMPACT_ZERO_GUIDE")
}

func runRankSamplerFullRequestExperiment(t *testing.T, paths, output, seeds, flag string) {
	t.Helper()
	if paths == "" || output == "" {
		t.Skip("set saved requests and an absolute output directory")
	}
	if !filepath.IsAbs(output) {
		t.Fatal("use an absolute output directory")
	}
	if err := os.MkdirAll(output, 0700); err != nil {
		t.Fatal(err)
	}
	previousProcs := runtime.GOMAXPROCS(4)
	defer runtime.GOMAXPROCS(previousProcs)
	previousLog := log.Writer()
	log.SetOutput(io.Discard)
	defer log.SetOutput(previousLog)
	for _, flag := range []string{"RARE_POSITION_MATCHED_POINT_POOL", "RARE_POSITION_CONDITIONED_ZERO", "RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD", "RARE_POSITION_CONDITIONED_POINT_TILT", "RARE_POSITION_POINT_TILT_UNDECIDED", "RARE_POSITION_POINT_TILT_CROSSCHECK", "RARE_POSITION_DIRECTIONAL_PEER_RESCUE"} {
		t.Setenv(flag, "1")
	}
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "0")
	t.Setenv("RARE_POSITION_BENCHMARK_ITERATIONS", "20000")
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL_WORKERS", "4")
	if flag == "RARE_POSITION_COMPACT_FORCED_FIXTURES" {
		t.Setenv("RARE_POSITION_PROPAGATED_DOMAINS", "1")
	}
	if seeds == "" {
		seeds = "801,804,808,817,911"
	}
	for _, path := range strings.Split(paths, ",") {
		data, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		var input GroupType
		if err := json.Unmarshal(data, &input); err != nil {
			t.Fatal(err)
		}
		hash := fmt.Sprintf("%x", sha256.Sum256(data))
		for _, seed := range strings.Split(seeds, ",") {
			t.Setenv("RARE_POSITION_RANDOM_SEED", seed)
			report := map[string]interface{}{"group": input.Id, "input_sha256": hash, "seed": seed, "flag": flag}
			arms := []string{"0", "1"}
			seedValue, err := strconv.ParseInt(seed, 10, 64)
			if err != nil {
				t.Fatal(err)
			}
			if seedValue%2 == 0 {
				arms = []string{"1", "0"}
			}
			for _, arm := range arms {
				t.Setenv(flag, arm)
				started := time.Now()
				matrix := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
				report["matrix_"+arm] = matrix
				report["ms_"+arm] = float64(time.Since(started).Microseconds()) / 1000
			}
			gained, lost := 0, 0
			a, b := report["matrix_0"].(map[int]map[int]ProductionEstimate), report["matrix_1"].(map[int]map[int]ProductionEstimate)
			for team, row := range a {
				for rank, est := range row {
					other := b[team][rank]
					if (flag == "RARE_POSITION_COMPACT_FORCED_FIXTURES" || flag == "RARE_POSITION_COMPACT_ZERO_GUIDE") &&
						(est.ConditionalHits != other.ConditionalHits || est.ConditionalSamples != other.ConditionalSamples ||
							est.Samples != other.Samples || est.Hits != other.Hits || est.WorkSpent != other.WorkSpent ||
							est.Design != other.Design || est.Reachability != other.Reachability ||
							est.Available != other.Available || est.MeetsPrecisionGoal != other.MeetsPrecisionGoal ||
							math.Abs(est.Probability-other.Probability) > 1e-9*math.Max(est.Probability, other.Probability)) {
						t.Fatalf("compaction changed team=%d rank=%d: before=%+v after=%+v", team, rank+1, est, other)
					}
					if est.Probability == 0 && b[team][rank].Probability > 0 {
						gained++
					}
					if est.Probability > 0 && b[team][rank].Probability == 0 {
						lost++
					}
				}
			}
			encoded, err := json.MarshalIndent(report, "", "  ")
			if err != nil {
				t.Fatal(err)
			}
			name := fmt.Sprintf("group-%d-%s-seed-%s.json", input.Id, hash[:8], seed)
			if err := os.WriteFile(filepath.Join(output, name), encoded, 0600); err != nil {
				t.Fatal(err)
			}
			t.Logf("group=%d hash=%s seed=%s gained=%d lost=%d before=%.0fms after=%.0fms", input.Id, hash[:8], seed, gained, lost, report["ms_0"], report["ms_1"])
		}
	}
}
