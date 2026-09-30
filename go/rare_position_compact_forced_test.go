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
