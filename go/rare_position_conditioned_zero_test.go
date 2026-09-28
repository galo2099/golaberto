package main

import (
	"encoding/json"
	"math"
	"math/rand"
	"os"
	"sort"
	"testing"
)

func TestConditionedPointEventContainsEveryTargetFinish(t *testing.T) {
	group, campaign, table, order, _ := createTestGroupForDiversified()
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	samplers := newConditionedScoreSamplers(group.Games)
	for _, target := range group.Team_groups {
		for rank := range group.Team_groups {
			blockers := make([]int, 0, 2)
			for _, other := range group.Team_groups {
				if other.Team_id != target.Team_id {
					blockers = append(blockers, other.Team_id)
				}
			}
			event, ok := buildConditionedPointEvent(target.Team_id, rank, blockers,
				group, campaign, table, bounds)
			if !ok {
				t.Fatalf("target=%d rank=%d: point event could not be built", target.Team_id, rank)
			}
			if event.mass < 0 || event.mass > 1+1e-12 {
				t.Fatalf("target=%d rank=%d: invalid mass %g", target.Team_id, rank, event.mass)
			}
			terminal := make(map[conditionedPointState]bool, len(event.terminal))
			for _, final := range event.terminal {
				terminal[final.state] = true
			}
			enumeratedMass := 0.0
			var patternMass [27]float64
			for code := 0; code < 27; code++ {
				state := conditionedPointState{}
				probability := 1.0
				outcomes := make([]int, len(event.games))
				number := code
				for i, game := range event.games {
					outcome := number % 3
					number /= 3
					outcomes[i] = outcome
					probability *= game.prob[outcome]
					for slot := range event.teams {
						state[slot] += game.deltas[outcome][slot]
					}
				}
				if terminal[state] {
					enumeratedMass += probability
					patternMass[code] = probability
				}
				simCampaign := make([]*TeamCampaign, len(campaign))
				for i, team := range campaign {
					simCampaign[i] = team.clone()
				}
				for i, selected := range event.games {
					game := group.Games[selected.index]
					home, away := 0, 0
					if outcomes[i] == 0 {
						away = 1
					} else if outcomes[i] == 2 {
						home = 1
					}
					score := &GameType{Id: game.Id, HomeId: game.HomeId, AwayId: game.AwayId,
						HomeScore: home, AwayScore: away, Played: true}
					addSimulatedGame(simCampaign[game.home_table_index], simCampaign[game.away_table_index], score)
				}
				teams := append([]*TeamCampaign(nil), simCampaign...)
				sort.Sort(TeamCampaignSorted{t: teams, sort: order})
				if teams[rank].id == target.Team_id && !terminal[state] {
					t.Fatalf("target=%d rank=%d outcome=%d: rank finish outside necessary event",
						target.Team_id, rank, code)
				}
			}
			if math.Abs(enumeratedMass-event.mass) > 1e-12 {
				t.Fatalf("target=%d rank=%d: enumerated mass=%g DP mass=%g",
					target.Team_id, rank, enumeratedMass, event.mass)
			}
			if event.mass == 0 {
				continue
			}
			if target.Team_id == 1 && rank == 0 {
				rng := rand.New(rand.NewSource(29))
				outcomes := make([]uint8, len(group.Games))
				var counts [27]int
				const draws = 20000
				for sample := 0; sample < draws; sample++ {
					event.sampleOutcomes(rng, outcomes)
					state := conditionedPointState{}
					code, power := 0, 1
					for _, game := range event.games {
						code += int(outcomes[game.index]) * power
						power *= 3
						for slot := range event.teams {
							state[slot] += game.deltas[outcomes[game.index]][slot]
						}
					}
					if !terminal[state] {
						t.Fatalf("conditioned draw escaped target event: %+v", state)
					}
					counts[code]++
				}
				for code, mass := range patternMass {
					want := mass / event.mass
					got := float64(counts[code]) / draws
					if math.Abs(got-want) > 0.02 {
						t.Fatalf("outcome=%d: conditioned frequency=%g want=%g", code, got, want)
					}
				}
			}
			result := sampleConditionedZeroCell(event, target.Team_id, rank,
				group, campaign, table, order, samplers, 1000, 19)
			if result.hits < 0 || result.hits > result.samples || math.IsNaN(result.mass) {
				t.Fatalf("invalid sample result: %+v", result)
			}
		}
	}
}

func TestConditionedScoreSamplerPreservesOutcome(t *testing.T) {
	group, _, _, _, _ := createTestGroupForDiversified()
	samplers := newConditionedScoreSamplers(group.Games)
	rng := rand.New(rand.NewSource(73))
	for i, game := range group.Games {
		for outcome, mass := range targetOutcomeProbabilities(game) {
			if mass == 0 {
				continue
			}
			for sample := 0; sample < 100; sample++ {
				home, away := samplers[i].sample(rng, uint8(outcome))
				if got := targetOutcomeDigit(true, home, away); got != outcome {
					t.Fatalf("game=%d desired=%d score=%d-%d got=%d", game.Id, outcome, home, away, got)
				}
			}
		}
	}
}

func TestConditionedExtraGainUsesUpperBoundReduction(t *testing.T) {
	if gain := conditionedExtraGain(1e-8, 1e-6); gain <= 0 {
		t.Fatalf("expected extra search to improve the upper bound, gain=%g", gain)
	}
	if gain := conditionedExtraGain(1e-13, 1e-6); gain != 0 {
		t.Fatalf("already resolved upper bound should not receive extra search, gain=%g", gain)
	}
	if gain := conditionedExtraGain(1e-8, 1); gain != 0 {
		t.Fatalf("broader event should not receive extra search, gain=%g", gain)
	}
}

func TestConditionedExtraCanSelectMiddleRank(t *testing.T) {
	group, campaign, table, order, _ := createTestGroupForDiversified()
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	event, ok := buildConditionedPointEvent(2, 1, nil, group, campaign, table, bounds)
	if !ok || event.mass <= 0 {
		t.Fatal("could not build middle-rank point event")
	}
	cells := []conditionedZeroCell{{2, 1}}
	results := []conditionedZeroSearchResult{{result: conditionedZeroResult{
		mass: event.mass, samples: 1000, work: 1000,
	}}}
	estimates := map[int]map[int]ProductionEstimate{
		2: {1: {ZeroHitUpper95: 1}},
	}
	allocateConditionedZeroExtra(group, campaign, table, order, 808,
		[]int{1, 2, 3}, nil, nil, bounds, newConditionedScoreSamplers(group.Games),
		cells, estimates, results)
	if got := results[0].result.samples; got != conditionedZeroExtraRuns {
		t.Fatalf("middle rank received %d extra draws, want %d", got, conditionedZeroExtraRuns)
	}
}

func TestConditionedRankPointScreenMatchesFullSampling(t *testing.T) {
	group, campaign, table, order, _ := createTestGroupForDiversified()
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	samplers := newConditionedScoreSamplers(group.Games)
	const draws = 20000
	for rank := range group.Team_groups {
		var event *conditionedPointEvent
		target := 0
		for _, team := range group.Team_groups {
			candidate, ok := buildConditionedPointEvent(team.Team_id, rank, nil,
				group, campaign, table, bounds)
			if ok && candidate.mass > 0 {
				event, target = candidate, team.Team_id
				break
			}
		}
		if event == nil {
			t.Fatalf("rank=%d: could not build point event", rank)
		}
		full := sampleConditionedZeroCell(event, target, rank, group, campaign,
			table, order, samplers, draws, 46)
		fast := sampleConditionedZeroCellFast(event, target, rank, group, campaign,
			table, order, samplers, draws, 76)
		if fast.samples != draws || math.Abs(float64(full.hits-fast.hits)/draws) > 0.02 {
			t.Fatalf("rank=%d: hit rates differ: full=%+v fast=%+v", rank, full, fast)
		}
	}
}

func TestConditionedRankLookaheadMatchesFullSampling(t *testing.T) {
	group, campaign, table, order, _ := createTestGroupForDiversified()
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	samplers := newConditionedScoreSamplers(group.Games)
	const draws = 50000
	for rank := range group.Team_groups {
		var event *conditionedPointEvent
		target := 0
		for _, team := range group.Team_groups {
			candidate, ok := buildConditionedPointEvent(team.Team_id, rank, nil,
				group, campaign, table, bounds)
			if ok && candidate.mass > 0 {
				event, target = candidate, team.Team_id
				break
			}
		}
		if event == nil {
			t.Fatalf("rank=%d: could not build point event", rank)
		}
		full := sampleConditionedZeroCell(event, target, rank, group, campaign,
			table, order, samplers, draws, 29)
		weighted, _ := sampleConditionedZeroRankLookahead(event, target, rank,
			group, campaign, table, order, bounds, samplers, draws, 57)
		want := event.mass * float64(full.hits) / draws
		if weighted.samples != draws || weighted.hits <= 0 || !weighted.weighted ||
			math.Abs(weighted.probability-want) > 0.015 {
			t.Fatalf("rank=%d: lookahead probability=%g, full probability=%g: %+v",
				rank, weighted.probability, want, weighted)
		}
	}
}

func TestConditionedGuidedSearchCanSelectMiddleRank(t *testing.T) {
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_DEEP", "0")
	group, campaign, table, order, _ := createTestGroupForDiversified()
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	event, ok := buildConditionedPointEvent(1, 1, nil, group, campaign, table, bounds)
	if !ok || event.mass <= 0 {
		t.Fatal("could not build a middle-rank point event")
	}
	results := []conditionedZeroSearchResult{{
		result: conditionedZeroResult{mass: event.mass, samples: 1000}, event: event,
	}}
	runConditionedGuidedSearch(group, campaign, table, order, 808,
		[]int{1, 2, 3}, nil, nil, bounds, newConditionedScoreSamplers(group.Games),
		[]conditionedZeroCell{{1, 1}}, results)
	if result := results[0].result; !result.weighted || result.hits <= 0 ||
		result.samples != conditionedZeroLookaheadSamples {
		t.Fatalf("middle rank did not receive a guided estimate: %+v", result)
	}
}

func TestConditionedZeroRealGroup(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to a saved group request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "0")
	t.Setenv("RARE_POSITION_RANDOM_SEED", "808")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD", "0")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO", "0")
	baseline := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO", "1")
	result := cloneGroupForBenchmark(input).calculate_odds()
	estimates := result["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	baselineZeros, zeros, witnesses, attempted, impossible, targetedSamples := 0, 0, 0, 0, 0, 0
	for _, team := range input.Team_groups {
		for rank := range input.Team_groups {
			est := estimates[team.Team_id][rank]
			prior := baseline[team.Team_id][rank]
			if prior.Probability == 0 {
				baselineZeros++
			} else if est.Probability == 0 {
				t.Fatalf("team=%d rank=%d: existing positive estimate became zero", team.Team_id, rank+1)
			}
			if est.Probability == 0 {
				zeros++
			}
			if est.Reachability == "witness" {
				if prior.Probability != 0 || est.Probability <= 0 ||
					est.Probability > est.ConditionalMass*(1+1e-6) {
					t.Fatalf("team=%d rank=%d: invalid witnessed estimate: %+v", team.Team_id, rank+1, est)
				}
				witnesses++
			}
			if est.ConditionalSamples > 0 {
				attempted++
				targetedSamples += est.ConditionalSamples
			}
			if est.Reachability == "impossible_by_points" {
				impossible++
			}
		}
	}
	if baselineZeros-zeros != witnesses {
		t.Fatalf("baseline zeros=%d final zeros=%d witnesses=%d", baselineZeros, zeros, witnesses)
	}
	if input.Id == 16653 {
		ceara := estimates[69][0]
		if (ceara.Reachability != "witness" && ceara.Reachability != "undecided") ||
			(ceara.ZeroHitUpper95 <= 0 && ceara.Reachability == "undecided") ||
			math.Abs(ceara.ConditionalMass-5.14342797285146e-7) > 1e-11 {
			t.Fatalf("Ceará first-place search did not match the joint-points event: %+v", ceara)
		}
	}
	for rank := range input.Team_groups {
		column := 0.0
		for _, team := range input.Team_groups {
			column += estimates[team.Team_id][rank].Probability
		}
		if math.Abs(column-1) > 1e-6 {
			t.Fatalf("rank=%d: column sum=%g", rank, column)
		}
	}
	t.Logf("group=%d baseline_zeros=%d final_zeros=%d witnesses=%d attempted=%d impossible=%d targeted_samples=%d work=%d",
		input.Id, baselineZeros, zeros, witnesses, attempted, impossible, targetedSamples,
		estimates[input.Team_groups[0].Team_id][0].WorkSpent)
	if input.Id == 16653 {
		t.Logf("Ceará first: %+v", estimates[69][0])
	}
}

func TestConditionedZeroDeepRealGroup(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to a saved group request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	if input.Id != 16653 {
		t.Skip("the deep first-place regression uses group 16653")
	}
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "0")
	t.Setenv("RARE_POSITION_RANDOM_SEED", "808")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_DEEP", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD", "0")
	estimates := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	for _, id := range []int{457, 68} {
		est := estimates[id][0]
		if est.Probability <= 0 || est.ConditionalHits <= 0 || est.Reachability != "witness" {
			t.Fatalf("team=%d first-place deep search did not find a witness: %+v", id, est)
		}
		if est.ConditionalSamples != conditionedZeroDeepRuns {
			t.Fatalf("team=%d first-place deep search used %d samples", id, est.ConditionalSamples)
		}
		t.Logf("team=%d first-place probability=%.12g mass=%.12g hits=%d samples=%d",
			id, est.Probability, est.ConditionalMass, est.ConditionalHits, est.ConditionalSamples)
	}
}

func TestConditionedZeroLookaheadRealGroup(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to a saved group request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	if input.Id != 16653 {
		t.Skip("the lookahead first-place regression uses group 16653")
	}
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "0")
	t.Setenv("RARE_POSITION_RANDOM_SEED", "808")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_DEEP", "0")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD", "1")
	estimates := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	for _, id := range []int{457, 68} {
		est := estimates[id][0]
		if est.Probability <= 0 || est.ConditionalHits <= 0 || est.Reachability != "witness" ||
			est.Design != "matched_point_pool_conditioned_lookahead" ||
			est.ConditionalSamples != conditionedZeroLookaheadSamples ||
			est.MaxEventWeightShare > 0.03 || est.ESS < 200 {
			t.Fatalf("team=%d lookahead did not produce a stable first-place estimate: %+v", id, est)
		}
		t.Logf("team=%d probability=%.12g mass=%.12g hits=%d ESS=%.1f max_share=%.4g",
			id, est.Probability, est.ConditionalMass, est.ConditionalHits,
			est.ESS, est.MaxEventWeightShare)
	}
	if ceara := estimates[69][0]; ceara.Reachability != "witness" ||
		ceara.Probability <= 0 || ceara.Design != "matched_point_pool_conditioned_lookahead" {
		t.Fatalf("general guided search missed Ceará first place: %+v", ceara)
	}
	second := estimates[457][1]
	if second.Probability <= 0 || second.Reachability != "witness" ||
		second.ConditionalSamples != conditionedZeroExtraRuns || second.ConditionalHits <= 0 ||
		second.ConditionalMass <= 0 || second.Probability > second.ConditionalMass {
		t.Fatalf("Botafogo-SP second-place search did not find a valid estimate: %+v", second)
	}
	t.Logf("Botafogo-SP second-place probability=%.12g mass=%.12g hits=%d samples=%d",
		second.Probability, second.ConditionalMass, second.ConditionalHits, second.ConditionalSamples)
	for _, cell := range [][2]int{{95, 2}, {22, 16}} {
		est := estimates[cell[0]][cell[1]]
		if est.ConditionalSamples != conditionedZeroExtraRuns || est.ConditionalMass <= 0 {
			t.Fatalf("team=%d rank=%d did not receive difficulty-allocated search: %+v",
				cell[0], cell[1]+1, est)
		}
	}
	extraCells := 0
	for _, team := range input.Team_groups {
		for rank := range input.Team_groups {
			if estimates[team.Team_id][rank].ConditionalSamples == conditionedZeroExtraRuns {
				extraCells++
			}
		}
	}
	if extraCells > conditionedZeroExtraBudget/conditionedZeroExtraRuns {
		t.Fatalf("extra search used %d batches, budget allows %d",
			extraCells, conditionedZeroExtraBudget/conditionedZeroExtraRuns)
	}
	for rank := range input.Team_groups {
		column := 0.0
		for _, team := range input.Team_groups {
			column += estimates[team.Team_id][rank].Probability
		}
		if math.Abs(column-1) > 1e-6 {
			t.Fatalf("rank=%d: column sum=%g", rank, column)
		}
	}
}

func TestConditionedZeroLookaheadSeedRobustness(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to a saved group request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	if input.Id != 16653 {
		t.Skip("the seed robustness regression uses group 16653")
	}
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")
	t.Setenv("RARE_POSITION_DIVERSIFIED_IS", "1")
	t.Setenv("RARE_POSITION_CEM_PARAMETERIZATION", "team_attack_concession")
	t.Setenv("RARE_POSITION_SEQUENTIAL_PRUNING", "1")
	t.Setenv("RARE_POSITION_MIN_INTERESTING_PROBABILITY", "1e-6")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_DEEP", "0")
	for _, seed := range []string{"804", "805", "811"} {
		t.Run(seed, func(t *testing.T) {
			t.Setenv("RARE_POSITION_RANDOM_SEED", seed)
			estimates := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
			for _, id := range []int{457, 68, 69} {
				if estimate := estimates[id][0]; estimate.Probability <= 0 {
					t.Fatalf("seed=%s team=%d first place was missed: %+v", seed, id, estimate)
				}
			}
		})
	}
}
