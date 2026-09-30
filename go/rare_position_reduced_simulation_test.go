package main

import (
	"encoding/json"
	"fmt"
	"math"
	"os"
	"strings"
	"testing"
)

func TestReducedSimulationMarginalizesMassAndRetainsTies(t *testing.T) {
	games := []conditionedPointOutcomeGame{
		{index: 1, home: 1, away: 2, prob: [3]float64{.2, .3, .5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
		{index: 2, home: 2, away: 3, prob: [3]float64{0, .3, .7}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
		{index: 3, home: 3, away: 4, prob: [3]float64{.2, .3, .5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
	}
	// Team 1 can tie the target and its game must remain. Teams 2, 3,
	// and 4 are strictly below even with all wins; their mutual games vanish.
	base := []int{20, 19, 0, 1, 2}
	cache := newConditionedRankDomainCache(games, nil, []int32{1, 2, 3, 4}, 0, 5)
	cache.reduce = true
	d := cache.get(base, 20, make([]uint8, 4))
	if d == nil || !d.feasible || d.omitted[0] || !d.omitted[1] || !d.omitted[2] || len(d.variable) != 1 {
		t.Fatalf("incorrect reduction: %+v", d)
	}
	if d.forcedMass != 1 || d.forced[0].outcome != 1 {
		t.Fatalf("marginalized mass must be one, representative must have support: %+v", d)
	}
	for code := 0; code < 27; code++ {
		final := append([]int(nil), base...)
		assignment := code
		for _, game := range games {
			o := assignment % 3
			assignment /= 3
			final[game.home] += game.homeGain[o]
			final[game.away] += game.awayGain[o]
		}
		for team := 2; team < len(final); team++ {
			if final[team] >= base[0] {
				t.Fatalf("discarded relevant result: assignment=%d team=%d", code, team)
			}
		}
	}
}

func TestReducedSimulationRequiresStrictOrderedBounds(t *testing.T) {
	for _, stride := range []int{1, 40} {
		g := conditionedPointOutcomeGame{index: 0, home: 1, away: 2, prob: [3]float64{.2, .3, .5},
			homeGain: [3]int{0, stride, 3 * stride}, awayGain: [3]int{3 * stride, stride, 0}}
		if stride > 1 {
			g.homeGain[2]++
			g.awayGain[0]++
		}
		for _, base := range [][]int{{20 * stride, 17 * stride, 0}, {20 * stride, 21 * stride, 0}, {20 * stride, 0, 0}} {
			rank := 0
			if base[1] > base[0] {
				rank = 1
			}
			cache := newConditionedRankDomainCache([]conditionedPointOutcomeGame{g}, nil, []int32{1, 2}, rank, 3)
			cache.reduce = true
			d := cache.get(base, base[0], []uint8{0})
			want := base[1] > base[0] || base[1]+g.homeGain[2] < base[0]
			got := d != nil && len(d.omitted) > 0 && d.omitted[0]
			if got != want {
				t.Fatalf("stride=%d base=%v reduced=%t want=%t", stride, base, got, want)
			}
		}
	}
}

func TestReducedSimulationModeDefaults(t *testing.T) {
	for _, mode := range []string{"", "0", "1", "all"} {
		t.Setenv("RARE_POSITION_REDUCED_SIMULATION", mode)
		if reducedSimulationEnabled() != (mode == "" || mode == "1" || mode == "all") {
			t.Fatalf("incorrect reduced-simulation mode %q", mode)
		}
	}
}

func TestReducedSimulationDoesNotAddRestrictionsToDirectedProposal(t *testing.T) {
	games := []conditionedPointOutcomeGame{
		{index: 0, home: 1, away: 2, prob: [3]float64{.2, .3, .5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
		{index: 1, home: 2, away: 3, prob: [3]float64{.2, .3, .5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
	}
	// Two rivals cannot reach the target; a domain solver would require
	// team 1's win. Pure marginalization must leave that proposal unchanged.
	base := []int{20, 18, 0, 0}
	cache := newConditionedRankDomainCache(games, nil, []int32{1, 2, 3}, 1, 4)
	cache.reduce, cache.propagate = true, false
	d := cache.get(base, 20, []uint8{0, 0})
	if d == nil || d.restricted || d.domains[0] != 7 || !d.omitted[1] {
		t.Fatalf("marginalization changed the proposal's rank restrictions: %+v", d)
	}
}

func reducedSimulationToy() (*GroupType, []*TeamCampaign, *Table) {
	keys := []uint32{1, 2, 3, 4, 5}
	table := NewTable(keys)
	group := &GroupType{Id: 1}
	campaign := make([]*TeamCampaign, len(keys))
	for i, points := range []int{20, 19, 0, 1, 2} {
		id := i + 1
		group.Team_groups = append(group.Team_groups, TeamType{Team_id: id, Bias: id})
		campaign[table.Query(uint32(id))] = &TeamCampaign{id: id, points: points, wins: 1, bias: id, points_win: 3, points_draw: 1}
	}
	for i, pair := range [][2]int{{1, 2}, {2, 3}, {3, 4}, {4, 5}, {5, 1}} {
		group.Games = append(group.Games, &GameType{Id: i + 1, HomeId: pair[0], AwayId: pair[1], HomePower: 1.4, AwayPower: 1.1,
			home_table_index: table.Query(uint32(pair[0])), away_table_index: table.Query(uint32(pair[1]))})
	}
	return group, campaign, table
}

func TestReducedSimulationWeightsMatchPlainSampling(t *testing.T) {
	group, campaign, table := reducedSimulationToy()
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	samplers := newConditionedScoreSamplers(group.Games)
	for _, order := range [][]SortType{{PT, W, GD, GF, BIAS}, {PT, GD, W, GF, BIAS}} {
		for rank := 0; rank < 2; rank++ {
			event, ok := buildConditionedPointEvent(1, rank, nil, group, campaign, table, bounds)
			if !ok {
				t.Fatal("missing toy event")
			}
			const draws = 60000
			plain := sampleConditionedZeroCell(event, 1, rank, group, campaign, table, order, samplers, draws, 6411)
			want := event.mass * float64(plain.hits) / draws
			plainSE := event.mass * math.Sqrt(float64(plain.hits)/draws*(1-float64(plain.hits)/draws)/draws)
			for _, arm := range []string{"0", "1", "all"} {
				t.Setenv("RARE_POSITION_REDUCED_SIMULATION", arm)
				for _, propagate := range []bool{false, true} {
					weighted, _ := sampleConditionedZeroRankLookaheadPolicy(event, 1, rank, group, campaign, table, order, bounds, samplers, draws, 6511, 3, .5, false, propagate)
					if weighted.hits == 0 || math.Abs(weighted.probability-want) > 6*math.Hypot(plainSE, weighted.stdErr)+1e-5 {
						t.Fatalf("order=%v rank=%d arm=%s propagate=%t weighted=%g plain=%g", order, rank+1, arm, propagate, weighted.probability, want)
					}
				}
			}
		}
	}
}

func BenchmarkReducedSimulation(b *testing.B) {
	benchmarkReducedSimulation(b, true)
}

func BenchmarkReducedDirectedSimulation(b *testing.B) {
	benchmarkReducedSimulation(b, false)
}

func benchmarkReducedSimulation(b *testing.B, propagate bool) {
	paths := os.Getenv("RARE_POSITION_REDUCED_EXPERIMENT_REQUESTS")
	if paths == "" {
		b.Skip("set saved requests")
	}
	for _, path := range strings.Split(paths, ",") {
		data, err := os.ReadFile(path)
		if err != nil {
			b.Fatal(err)
		}
		var input GroupType
		if err := json.Unmarshal(data, &input); err != nil {
			b.Fatal(err)
		}
		group := cloneGroupForBenchmark(input)
		campaign, table, order := diversifiedBenchmarkSetup(group)
		bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
		samplers := newConditionedScoreSamplers(group.Games)
		cells := []conditionedZeroCell{{16, 16}, {17, 13}, {5, 19}, {15, 19}, {74, 19}}
		if input.Id == 16653 {
			cells = []conditionedZeroCell{{68, 1}, {22, 17}, {125, 9}}
		}
		if input.Id != 16498 && input.Id != 16653 {
			continue
		}
		for _, cell := range cells {
			event, ok := buildConditionedPointEvent(cell.id, cell.rank, nil, group, campaign, table, bounds)
			if !ok || event.mass <= 0 {
				continue
			}
			pointTilt := -.5
			if cell.rank < len(group.Team_groups)/2 {
				pointTilt = .5
			}
			arms := []string{"0", "1"}
			if !propagate {
				arms[1] = "all"
			}
			for _, arm := range arms {
				b.Run(fmt.Sprintf("group-%d-fixtures-%d/team-%d-rank-%d/reduced-%s", input.Id, countPlayedGames(group.Games), cell.id, cell.rank+1, arm), func(b *testing.B) {
					b.Setenv("RARE_POSITION_REDUCED_SIMULATION", arm)
					b.ReportAllocs()
					b.ResetTimer()
					for i := 0; i < b.N; i++ {
						result, _ := sampleConditionedZeroRankLookaheadPolicy(event, cell.id, cell.rank, group, campaign, table, order, bounds, samplers, 30000, 7011, 6, pointTilt, false, propagate)
						b.ReportMetric(float64(result.omittedDraws)/float64(result.samples), "omitted/draw")
						b.ReportMetric(float64(result.hits), "hits/op")
						if result.probability > 0 {
							b.ReportMetric(math.Log10(result.probability), "log10-p")
						}
					}
				})
			}
		}
	}
}
