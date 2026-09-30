package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"math"
	"math/rand"
	"os"
	"sort"
	"strings"
	"testing"
)

func TestCompactForcedPlanPreservesPrefixesAndMass(t *testing.T) {
	games := []conditionedPointOutcomeGame{
		{index: 10, home: 1, away: 2, prob: [3]float64{.2, .3, .5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
		{index: 11, home: 2, away: 3, prob: [3]float64{.2, .3, .5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
		{index: 12, home: 3, away: 1, prob: [3]float64{.2, .3, .5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
		{index: 13, home: 1, away: 2, prob: [3]float64{.2, .3, .5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
		{index: 14, home: 2, away: 3, prob: [3]float64{.2, .3, .5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
	}
	// Fixed results before, between and after unresolved fixtures.
	domains := &conditionedRankDomains{domains: []uint8{1 << 2, 7, 1 << 0, 7, 1 << 1}}
	base := []int{51, 30, 40, 49}
	domains.compact(games, base)
	if len(domains.variable) != 2 || len(domains.forced) != 3 || domains.forcedAfter != 1 || math.Abs(domains.forcedMass-.03) > 1e-15 {
		t.Fatalf("incorrect compact plan: %+v", domains)
	}
	for code := 0; code < 9; code++ {
		assignment := []int{2, code % 3, 0, code / 3, 1}
		original, compact := append([]int(nil), base...), append([]int(nil), domains.compactBase...)
		variable := 0
		for step, game := range games {
			if step == 1 || step == 3 {
				entry := domains.variable[variable]
				if entry.step != step || entry.skippedBefore != 1 ||
					compact[game.home]-entry.homeForced != original[game.home] ||
					compact[game.away]-entry.awayForced != original[game.away] {
					t.Fatalf("assignment=%v step=%d proposal prefix changed", assignment, step)
				}
				compact[game.home] += game.homeGain[assignment[step]]
				compact[game.away] += game.awayGain[assignment[step]]
				variable++
			}
			original[game.home] += game.homeGain[assignment[step]]
			original[game.away] += game.awayGain[assignment[step]]
		}
		for team := range original {
			if original[team] != compact[team] {
				t.Fatalf("assignment=%v final score changed", assignment)
			}
		}
	}
	for index, forced := range domains.forced {
		if forced.index != []int{10, 12, 14}[index] || forced.outcome != []uint8{2, 0, 1}[index] {
			t.Fatalf("fixed fixture identity changed: %+v", domains.forced)
		}
	}
}

func TestCompactForcedSamplingPreservesSeededResults(t *testing.T) {
	group, campaign, table, _, _ := createTestGroupForDiversified()
	campaign[0].wins, campaign[1].wins, campaign[2].wins = 4, 3, 1
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	samplers := newConditionedScoreSamplers(group.Games)
	for _, order := range [][]SortType{{PT, W, GD, GF, BIAS}, {PT, GD, W, GF, BIAS}} {
		for _, team := range group.Team_groups {
			for rank := range group.Team_groups {
				event, ok := buildConditionedPointEvent(team.Team_id, rank, nil, group, campaign, table, bounds)
				if !ok || event.mass <= 0 {
					continue
				}
				t.Setenv("RARE_POSITION_COMPACT_FORCED_FIXTURES", "0")
				a, _ := sampleConditionedZeroRankWithDomains(event, team.Team_id, rank, group, campaign, table, order, bounds, samplers, 15000, 7011, 6, .5)
				t.Setenv("RARE_POSITION_COMPACT_FORCED_FIXTURES", "1")
				b, _ := sampleConditionedZeroRankWithDomains(event, team.Team_id, rank, group, campaign, table, order, bounds, samplers, 15000, 7011, 6, .5)
				if a.hits != b.hits || a.samples != b.samples ||
					math.Abs(a.probability-b.probability) > 1e-12*math.Max(a.probability, b.probability) ||
					math.Abs(a.stdErr-b.stdErr) > 1e-12*math.Max(a.stdErr, b.stdErr) {
					t.Fatalf("team=%d rank=%d seeded results changed: before=%+v after=%+v", team.Team_id, rank+1, a, b)
				}
			}
		}
	}
}

func TestCompactZeroGuideRetainsZeroScoreRejections(t *testing.T) {
	game := conditionedPointOutcomeGame{home: 1, away: 2, prob: [3]float64{.2, .3, .5},
		homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}}
	for _, test := range []struct {
		name                      string
		base, target              int
		above, below, probability float64
		want                      bool
	}{
		{"ties remain positive", 9, 10, 1, 0, .3, true},
		{"certainly above remains positive", 10, 10, 1, 0, .3, true},
		{"certainly below remains positive", 8, 10, 0, 1, .3, true},
		{"zero below score", 8, 10, 1, 0, .3, false},
		{"zero above score", 10, 10, 0, 1, .3, false},
		{"forced score underflows", 10, 10, 1e-100, 0, math.SmallestNonzeroFloat64, false},
	} {
		t.Run(test.name, func(t *testing.T) {
			g := game
			g.prob[1] = test.probability
			d := &conditionedRankDomains{domains: []uint8{1 << 1},
				lower: []int{0, test.base + 1, test.base + 1}, upper: []int{0, test.base + 1, test.base + 1},
				minimumSuffix: [][]int{{0, 1, 1}}, maximumSuffix: [][]int{{0, 1, 1}}}
			suffix := [][][]float64{nil, {{1}, {1}, {1}}}
			if got := d.canCompactZeroGuide([]conditionedPointOutcomeGame{g}, []int{0, test.base, test.base},
				test.target, suffix, test.above, test.below); got != test.want {
				t.Fatalf("canCompact=%t want=%t", got, test.want)
			}
		})
	}
}

func TestCompactZeroGuideSavedBoundary(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set saved group 16498 request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		t.Fatal(err)
	}
	if input.Id != 16498 {
		t.Skip("different group")
	}
	group := cloneGroupForBenchmark(input)
	campaign, table, order := diversifiedBenchmarkSetup(group)
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	samplers := newConditionedScoreSamplers(group.Games)
	for _, cell := range []conditionedZeroCell{{15, 19}, {74, 19}} {
		event, ok := buildConditionedPointEvent(cell.id, cell.rank, nil, group, campaign, table, bounds)
		if !ok {
			t.Fatal("missing event")
		}
		seed := deriveRarePositionSeed(1790746108560577000, fmt.Sprintf("directional-peer-rescue-%d-%d", cell.id, cell.rank))
		t.Setenv("RARE_POSITION_COMPACT_ZERO_GUIDE", "0")
		a, _ := sampleConditionedZeroRankWithDomains(event, cell.id, cell.rank, group, campaign, table, order, bounds, samplers, 45000, seed, 3, -.5)
		t.Setenv("RARE_POSITION_COMPACT_ZERO_GUIDE", "1")
		b, _ := sampleConditionedZeroRankWithDomains(event, cell.id, cell.rank, group, campaign, table, order, bounds, samplers, 45000, seed, 3, -.5)
		if a.hits != b.hits || a.samples != b.samples || a.work != b.work ||
			math.Abs(a.probability-b.probability) > 1e-12*math.Max(a.probability, b.probability) ||
			math.Abs(a.stdErr-b.stdErr) > 1e-12*math.Max(a.stdErr, b.stdErr) {
			t.Fatalf("cell=%+v before=%+v after=%+v", cell, a, b)
		}
	}
}

func TestCompactZeroGuideSafetyAcrossCoupledPrefixes(t *testing.T) {
	games := []conditionedPointOutcomeGame{
		{home: 1, away: 2, prob: [3]float64{.2, .3, .5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
		{home: 2, away: 3, prob: [3]float64{.2, .3, .5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
		{home: 3, away: 1, prob: [3]float64{.2, .3, .5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
	}
	suffix := conditionedRankSuffix(games, 4, 20)
	checked := 0
	for _, base := range [][]int{{0, 1, 4, 6}, {0, 6, 6, 6}, {0, 0, 0, 0}} {
		for target := 0; target <= 12; target++ {
			for _, rank := range []int{0, 3} {
				d := propagateConditionedRankDomains(games, base, []int32{1, 2, 3}, rank, target)
				if !d.feasible || len(d.forced) == 0 {
					continue
				}
				above, below := 1.0, 0.0
				if rank == 0 {
					above, below = 0, 1
				}
				if !d.canCompactZeroGuide(games, base, target, suffix, above, below) {
					continue
				}
				checked++
				for assignment := 0; assignment < 27; assignment++ {
					code := assignment
					points := append([]int(nil), base...)
					for step, g := range games {
						o := code % 3
						code /= 3
						if !d.allows(step, g, o, points) {
							break
						}
						if d.domains[step] == 1<<o {
							score := g.prob[o] * conditionedRankSideFactor(suffix[step+1][g.home], target-points[g.home]-g.homeGain[o], below, above) * conditionedRankSideFactor(suffix[step+1][g.away], target-points[g.away]-g.awayGain[o], below, above)
							if !(score > 0) {
								t.Fatalf("accepted unsafe compact plan: base=%v rank=%d target=%d assignment=%d step=%d", base, rank, target, assignment, step)
							}
						}
						points[g.home] += g.homeGain[o]
						points[g.away] += g.awayGain[o]
					}
				}
			}
		}
	}
	if checked == 0 {
		t.Fatal("no positive compact plan exercised")
	}
}

func BenchmarkCompactZeroGuide(b *testing.B) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		b.Skip("set saved group 16498 request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		b.Fatal(err)
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		b.Fatal(err)
	}
	if input.Id != 16498 {
		b.Skip("different group")
	}
	group := cloneGroupForBenchmark(input)
	campaign, table, order := diversifiedBenchmarkSetup(group)
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	samplers := newConditionedScoreSamplers(group.Games)
	for _, cell := range []conditionedZeroCell{{15, 19}, {74, 19}} {
		event, ok := buildConditionedPointEvent(cell.id, cell.rank, nil, group, campaign, table, bounds)
		if !ok {
			b.Fatal("missing event")
		}
		for _, arm := range []string{"0", "1"} {
			b.Run(fmt.Sprintf("team-%d-rank-%d/compact-%s", cell.id, cell.rank+1, arm), func(b *testing.B) {
				b.Setenv("RARE_POSITION_COMPACT_ZERO_GUIDE", arm)
				seed := deriveRarePositionSeed(1790746108560577000, fmt.Sprintf("directional-peer-rescue-%d-%d", cell.id, cell.rank))
				b.ReportAllocs()
				b.ResetTimer()
				for i := 0; i < b.N; i++ {
					sampleConditionedZeroRankWithDomains(event, cell.id, cell.rank, group, campaign, table, order, bounds, samplers, 45000, seed, 3, -.5)
				}
			})
		}
	}
}

func BenchmarkCompactForcedFixtures(b *testing.B) {
	paths := os.Getenv("RARE_POSITION_COMPACT_EXPERIMENT_REQUESTS")
	if paths == "" {
		b.Skip("set saved current/earlier group request paths")
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
		if input.Id != 16653 {
			continue
		}
		group := cloneGroupForBenchmark(input)
		campaign, table, order := diversifiedBenchmarkSetup(group)
		standing := make([]*TeamCampaign, 0, len(group.Team_groups))
		for _, team := range group.Team_groups {
			standing = append(standing, campaign[table.Query(uint32(team.Team_id))])
		}
		sort.Sort(TeamCampaignSorted{t: standing, sort: order, rng: rand.New(rand.NewSource(1))})
		current := make(map[int]int, len(standing))
		for rank, team := range standing {
			current[team.id] = rank
		}
		bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
		samplers := newConditionedScoreSamplers(group.Games)
		hash := fmt.Sprintf("%x", sha256.Sum256(data))
		for _, cell := range []conditionedZeroCell{{22, 17}, {77, 18}, {12, 17}, {279, 17}, {9, 13}} {
			event, ok := buildConditionedPointEvent(cell.id, cell.rank, nil, group, campaign, table, bounds)
			if !ok || event.mass <= 0 {
				continue
			}
			pointTilt := .5
			if cell.rank > current[cell.id] {
				pointTilt = -.5
			}
			for _, arm := range []string{"0", "1"} {
				name := fmt.Sprintf("%s/team-%d-rank-%d/compact-%s", hash[:8], cell.id, cell.rank+1, arm)
				b.Run(name, func(b *testing.B) {
					b.Setenv("RARE_POSITION_COMPACT_FORCED_FIXTURES", arm)
					b.ReportAllocs()
					b.ResetTimer()
					for draw := 0; draw < b.N; draw++ {
						sampleConditionedZeroRankWithDomains(event, cell.id, cell.rank, group, campaign, table, order, bounds, samplers, 15000, 7011, 6, pointTilt)
					}
				})
			}
		}
	}
}
