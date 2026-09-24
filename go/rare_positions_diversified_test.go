package main

import (
	"math/rand"
	"testing"
)

func createTestGroupForDiversified() (*GroupType, []*TeamCampaign, *Table, []SortType, map[int][]int) {
	tg1 := TeamType{Team_id: 1, Bias: 0}
	tg2 := TeamType{Team_id: 2, Bias: 1}
	tg3 := TeamType{Team_id: 3, Bias: 2}

	group := &GroupType{
		Id:          1,
		Team_groups: []TeamType{tg1, tg2, tg3},
		Games: []*GameType{
			{Id: 101, HomeId: 1, AwayId: 2, HomePower: 1.5, AwayPower: 1.0, Played: false, home_table_index: 0, away_table_index: 1},
			{Id: 102, HomeId: 2, AwayId: 3, HomePower: 1.2, AwayPower: 0.8, Played: false, home_table_index: 1, away_table_index: 2},
			{Id: 103, HomeId: 3, AwayId: 1, HomePower: 1.0, AwayPower: 1.4, Played: false, home_table_index: 2, away_table_index: 0},
		},
	}

	keys := []uint32{1, 2, 3}
	table := NewTable(keys)

	c1 := &TeamCampaign{id: 1, bias: 0, points: 10, points_win: 3, points_draw: 1, points_loss: 0}
	c2 := &TeamCampaign{id: 2, bias: 1, points: 9, points_win: 3, points_draw: 1, points_loss: 0}
	c3 := &TeamCampaign{id: 3, bias: 2, points: 1, points_win: 3, points_draw: 1, points_loss: 0}
	campaign := []*TeamCampaign{c1, c2, c3}

	for _, g := range group.Games {
		g.home_table_index = table.Query(uint32(g.HomeId))
		g.away_table_index = table.Query(uint32(g.AwayId))
	}

	sortOrder := []SortType{PT, GD, GF, BIAS}

	counts := map[int][]int{
		1: {500, 500, 0},
		2: {500, 500, 0},
		3: {0, 0, 1000},
	}

	return group, campaign, table, sortOrder, counts
}

func TestDiversifiedScoutAndTailDiscovery(t *testing.T) {
	group, campaign, table, sortOrder, _ := createTestGroupForDiversified()
	rng := rand.New(rand.NewSource(42))

	scout := runPlainMCScout(campaign, group.Games, table, sortOrder, group.Team_groups, 1000, 100, rng)

	if scout.Samples != 1000 {
		t.Errorf("Expected 1000 scout samples, got %d", scout.Samples)
	}

	tails := discoverDirectionalTails(scout, group.Team_groups)
	t.Logf("Discovered %d directional tails", len(tails))
	for _, tail := range tails {
		t.Logf("Tail: team=%d direction=%v unseen_count=%d first_unseen_pos=%d extremity=%.2f",
			tail.TeamID, tail.Direction, tail.UnseenFeasibleCount, tail.FirstUnseenPosition, tail.ExtremityScore)
	}
}

func TestDiversifiedProposalSearchAndFreeze(t *testing.T) {
	group, campaign, table, sortOrder, _ := createTestGroupForDiversified()
	rng := rand.New(rand.NewSource(12345))

	originalMeans := make([]GameProposalMeans, len(group.Games))
	for i, game := range group.Games {
		originalMeans[i] = GameProposalMeans{Home: game.HomePower, Away: game.AwayPower}
	}

	scout := runPlainMCScout(campaign, group.Games, table, sortOrder, group.Team_groups, 500, 100, rng)

	proposals, probeStats, searchWork := searchDiversifiedProposals(
		scout, campaign, group.Games, originalMeans, table, sortOrder, group.Team_groups, 50000, rng)

	if len(proposals) == 0 {
		t.Fatalf("Expected at least plain_mc proposal, got 0")
	}
	if proposals[0].Kind != ProposalPlainMC {
		t.Errorf("Expected first proposal to be plain_mc, got %s", proposals[0].Kind)
	}

	t.Logf("Proposals found: %d, searchWork: %d", len(proposals), searchWork)

	frozen := freezeDiversifiedDesign(proposals, probeStats, scout, group.Team_groups, 500000, scout.Work, searchWork)

	if len(frozen.Batches) == 0 {
		t.Fatalf("Expected non-empty frozen batches")
	}

	t.Logf("Frozen batches: %d, productionWork: %d", len(frozen.Batches), frozen.ProductionWork)
}

func TestDiversifiedProductionExecution(t *testing.T) {
	group, campaign, table, sortOrder, _ := createTestGroupForDiversified()
	rng := rand.New(rand.NewSource(999))

	originalMeans := make([]GameProposalMeans, len(group.Games))
	for i, game := range group.Games {
		originalMeans[i] = GameProposalMeans{Home: game.HomePower, Away: game.AwayPower}
	}

	scout := runPlainMCScout(campaign, group.Games, table, sortOrder, group.Team_groups, 500, 100, rng)
	proposals, probeStats, searchWork := searchDiversifiedProposals(
		scout, campaign, group.Games, originalMeans, table, sortOrder, group.Team_groups, 50000, rng)

	frozen := freezeDiversifiedDesign(proposals, probeStats, scout, group.Team_groups, 200000, scout.Work, searchWork)

	estimates := runDiversifiedProduction(frozen, campaign, group.Games, originalMeans, table, sortOrder, group.Team_groups, 54321)

	if len(estimates) != len(group.Team_groups) {
		t.Errorf("Expected estimates for %d teams, got %d", len(group.Team_groups), len(estimates))
	}

	for teamID, posMap := range estimates {
		for pos, est := range posMap {
			if est.Probability < 0 || est.Probability > 1 {
				t.Errorf("Invalid probability for team %d pos %d: %f", teamID, pos, est.Probability)
			}
			if est.ESS < 0 {
				t.Errorf("Invalid ESS for team %d pos %d: %f", teamID, pos, est.ESS)
			}
		}
	}
}
