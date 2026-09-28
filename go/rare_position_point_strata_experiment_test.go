package main

import (
	"encoding/json"
	"fmt"
	"os"
	"sort"
	"strings"
	"testing"
	"time"
)

type pointStrataExperiment struct {
	group    *GroupType
	campaign []*TeamCampaign
	table    *Table
	order    []SortType
	bounds   pointRankBounds
	samplers []conditionedScoreSampler
	current  int
}

func TestPointStratifiedCoverageExperiment(t *testing.T) {
	experiment := loadPointStrataExperiment(t)
	for _, config := range []struct {
		rank int
		tilt float64
	}{{1, 2}, {2, 4}} {
		base, ok := buildConditionedPointEvent(68, config.rank, nil,
			experiment.group, experiment.campaign, experiment.table, experiment.bounds)
		if !ok {
			t.Fatal("could not build target point event")
		}
		pointSet := make(map[int]bool)
		for _, terminal := range base.terminal {
			pointSet[experiment.current+int(terminal.state[0])] = true
		}
		points := make([]int, 0, len(pointSet))
		for point := range pointSet {
			points = append(points, point)
		}
		sort.Sort(sort.Reverse(sort.IntSlice(points)))
		for _, point := range points {
			event := experiment.pointEvent(t, config.rank, point)
			start := time.Now()
			result, accepted := sampleConditionedZeroRankLookaheadWithTilt(event, 68,
				config.rank, experiment.group, experiment.campaign, experiment.table,
				experiment.order, experiment.bounds, experiment.samplers,
				20000, int64(20000+config.rank*100+point), config.tilt)
			fmt.Printf("point-coverage rank=%d points=%d mass=%.9g hits=%d p=%.9g se=%.3g ess=%.3g accepted=%v elapsed_ms=%.1f\n",
				config.rank+1, point, event.mass, result.hits, result.probability,
				result.stdErr, result.ess, accepted,
				float64(time.Since(start).Microseconds())/1000)
		}
	}
}

func loadPointStrataExperiment(t *testing.T) pointStrataExperiment {
	t.Helper()
	if os.Getenv("RARE_POSITION_POINT_STRATA_EXPERIMENT") != "1" {
		t.Skip("set RARE_POSITION_POINT_STRATA_EXPERIMENT=1 to run the sampling experiment")
	}
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to a saved group request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var group GroupType
	if err := json.Unmarshal(data, &group); err != nil {
		t.Fatal(err)
	}
	if group.Id != 16653 {
		t.Skip("the point-strata experiment uses group 16653")
	}
	order := build_sorted_array(strings.Split(strings.Join(strings.Fields(group.Phase.Sort), ""), ","))
	usesHead := false
	for _, criterion := range order {
		if criterion == HEAD {
			usesHead = true
		}
	}
	ids := make(map[int]struct{})
	for _, game := range group.Games {
		ids[game.HomeId] = struct{}{}
		ids[game.AwayId] = struct{}{}
	}
	for _, team := range group.Team_groups {
		ids[team.Team_id] = struct{}{}
	}
	keys := make([]uint32, 0, len(ids))
	for id := range ids {
		keys = append(keys, uint32(id))
	}
	table := NewTable(keys)
	for _, game := range group.Games {
		game.home_table_index = table.Query(uint32(game.HomeId))
		game.away_table_index = table.Query(uint32(game.AwayId))
	}
	campaign := make([]*TeamCampaign, len(keys))
	for _, team := range group.Team_groups {
		campaign[table.Query(uint32(team.Team_id))] = &TeamCampaign{
			id: team.Team_id, points: team.Add_sub, bias: team.Bias,
			add_sub: team.Add_sub, uses_head: usesHead,
			points_win:  group.Phase.Championship.Point_win,
			points_draw: group.Phase.Championship.Point_draw,
			points_loss: group.Phase.Championship.Point_loss,
		}
	}
	for _, game := range group.Games {
		if !game.Played {
			continue
		}
		if campaign[game.home_table_index] != nil {
			campaign[game.home_table_index].add_game(game)
		}
		if campaign[game.away_table_index] != nil {
			campaign[game.away_table_index].add_game(game)
		}
	}
	return pointStrataExperiment{
		group: &group, campaign: campaign, table: table, order: order,
		bounds:   buildPointRankBounds(campaign, group.Team_groups, group.Games, table),
		samplers: newConditionedScoreSamplers(group.Games),
		current:  campaign[table.Query(68)].points,
	}
}

func (experiment pointStrataExperiment) pointEvent(t *testing.T, rank, finalPoints int) *conditionedPointEvent {
	t.Helper()
	event, ok := buildConditionedPointEvent(68, rank, nil, experiment.group,
		experiment.campaign, experiment.table, experiment.bounds)
	if !ok {
		t.Fatal("could not build target point event")
	}
	filtered := *event
	filtered.terminal = nil
	filtered.mass = 0
	last := event.forward[len(event.forward)-1]
	for _, terminal := range event.terminal {
		if experiment.current+int(terminal.state[0]) != finalPoints {
			continue
		}
		filtered.mass += last[terminal.state]
		terminal.cumulative = filtered.mass
		filtered.terminal = append(filtered.terminal, terminal)
	}
	return &filtered
}

// Run with RARE_POSITION_POINT_STRATA_EXPERIMENT=1 and a saved group request
// to compare directed proposals within the exact 57-point stratum. Output is
// an experiment, not a production estimate: other point totals are not covered.
func TestPointStratifiedRankProposalExperiment(t *testing.T) {
	experiment := loadPointStrataExperiment(t)
	for _, rank := range []int{1, 2} {
		event := experiment.pointEvent(t, rank, 57)
		if event.mass <= 0 {
			t.Fatal("Avaí 57-point stratum is empty")
		}
		for _, tilt := range []float64{0.5, 1, 2, 4, 8} {
			for _, seed := range []int64{808, 809, 810} {
				start := time.Now()
				result, accepted := sampleConditionedZeroRankLookaheadWithTilt(event, 68, rank,
					experiment.group, experiment.campaign, experiment.table, experiment.order,
					experiment.bounds, experiment.samplers, 100000, seed, tilt)
				fmt.Printf("point-strata rank=%d points=57 tilt=%g seed=%d mass=%.9g hits=%d p=%.9g se=%.3g ess=%.3g share=%.3g accepted=%v elapsed_ms=%.1f\n",
					rank+1, tilt, seed, event.mass, result.hits, result.probability,
					result.stdErr, result.ess, result.maxWeightShare, accepted,
					float64(time.Since(start).Microseconds())/1000)
			}
		}
	}
}

// Measures a small per-cell budget that could replace, rather than add to,
// some of the request's current zero-cell search work.
func TestPointStratifiedBudgetExperiment(t *testing.T) {
	experiment := loadPointStrataExperiment(t)
	for _, config := range []struct {
		rank, samples int
		tilt          float64
	}{{1, 50000, 2}, {2, 20000, 4}} {
		event := experiment.pointEvent(t, config.rank, 57)
		for _, seed := range []int64{808, 809, 810, 811, 812} {
			start := time.Now()
			result, accepted := sampleConditionedZeroRankLookaheadWithTilt(event, 68,
				config.rank, experiment.group, experiment.campaign, experiment.table,
				experiment.order, experiment.bounds, experiment.samplers,
				config.samples, seed, config.tilt)
			fmt.Printf("point-budget rank=%d points=57 samples=%d tilt=%g seed=%d hits=%d p=%.9g se=%.3g ess=%.3g share=%.3g accepted=%v elapsed_ms=%.1f\n",
				config.rank+1, config.samples, config.tilt, seed,
				result.hits, result.probability, result.stdErr, result.ess,
				result.maxWeightShare, accepted,
				float64(time.Since(start).Microseconds())/1000)
		}
	}
}
