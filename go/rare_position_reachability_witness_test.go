package main

import (
	"encoding/json"
	"os"
	"sort"
	"testing"
)

func TestGroup16653Team95FirstPlaceWitness(t *testing.T) {
	var fixture struct {
		GroupID   int `json:"group_id"`
		TeamID    int `json:"team_id"`
		PointWin  int `json:"point_win"`
		PointDraw int `json:"point_draw"`
		PointLoss int `json:"point_loss"`
		Games     []struct {
			ID             int    `json:"id"`
			HomeID         int    `json:"home_id"`
			AwayID         int    `json:"away_id"`
			PlayedScore    []int  `json:"played_score"`
			WitnessOutcome string `json:"witness_outcome"`
		} `json:"games"`
	}
	data, err := os.ReadFile("../experiments/rare_positions/2026-09-26-group-16653-team-95-first-place-witness.json")
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatal(err)
	}
	if fixture.GroupID != 16653 || fixture.TeamID != 95 || len(fixture.Games) != 380 {
		t.Fatalf("unexpected witness metadata: group=%d team=%d games=%d", fixture.GroupID, fixture.TeamID, len(fixture.Games))
	}

	teamSet := make(map[int]bool)
	gameSet := make(map[int]bool)
	for _, game := range fixture.Games {
		if gameSet[game.ID] {
			t.Fatalf("duplicate game %d", game.ID)
		}
		gameSet[game.ID] = true
		teamSet[game.HomeID] = true
		teamSet[game.AwayID] = true
	}
	teamIDs := make([]int, 0, len(teamSet))
	for id := range teamSet {
		teamIDs = append(teamIDs, id)
	}
	sort.Ints(teamIDs)
	if len(teamIDs) != 20 {
		t.Fatalf("teams=%d, want 20", len(teamIDs))
	}
	keys := make([]uint32, len(teamIDs))
	groups := make([]TeamType, len(teamIDs))
	for i, id := range teamIDs {
		keys[i] = uint32(id)
		groups[i] = TeamType{Team_id: id}
	}
	table := NewTable(keys)
	campaign := make([]*TeamCampaign, len(teamIDs))
	for _, id := range teamIDs {
		campaign[table.Query(uint32(id))] = &TeamCampaign{
			id: id, points_win: fixture.PointWin, points_draw: fixture.PointDraw, points_loss: fixture.PointLoss,
		}
	}

	remaining := make([]*GameType, 0, 90)
	completed := make([]*GameType, 0, 90)
	targetWins := 0
	for _, game := range fixture.Games {
		g := &GameType{Id: game.ID, HomeId: game.HomeID, AwayId: game.AwayID}
		if len(game.PlayedScore) == 2 {
			if game.WitnessOutcome != "" {
				t.Fatalf("played game %d also has a witness outcome", game.ID)
			}
			g.HomeScore, g.AwayScore = game.PlayedScore[0], game.PlayedScore[1]
			g.Played = true
			campaign[table.Query(uint32(g.HomeId))].add_game(g)
			campaign[table.Query(uint32(g.AwayId))].add_game(g)
			continue
		}
		if len(game.PlayedScore) != 0 {
			t.Fatalf("game %d has an invalid played score", game.ID)
		}
		remaining = append(remaining, g)
		switch game.WitnessOutcome {
		case "home_win":
			g.HomeScore = 1
			if g.HomeId == fixture.TeamID {
				targetWins++
			}
		case "draw":
		case "away_win":
			g.AwayScore = 1
			if g.AwayId == fixture.TeamID {
				targetWins++
			}
		default:
			t.Fatalf("game %d has invalid outcome %q", game.ID, game.WitnessOutcome)
		}
		completed = append(completed, g)
	}
	if len(remaining) != 90 || targetWins != 9 {
		t.Fatalf("remaining=%d target wins=%d", len(remaining), targetWins)
	}
	if campaign[table.Query(uint32(fixture.TeamID))].points != 28 {
		t.Fatal("team 95's starting points changed")
	}
	order := []SortType{PT, W, GD, GF, HEAD}
	if !possiblePositionByPointsBounds(fixture.TeamID, 0, campaign, groups, remaining, table, order) {
		t.Fatal("conservative Go bounds incorrectly rule out first place")
	}
	for _, game := range completed {
		campaign[table.Query(uint32(game.HomeId))].add_game(game)
		campaign[table.Query(uint32(game.AwayId))].add_game(game)
	}
	sorted := TeamCampaignSorted{t: campaign, sort: order}
	sort.Sort(sorted)
	if sorted.t[0].id != fixture.TeamID || sorted.t[0].points != 55 || sorted.t[1].points != 54 {
		t.Fatalf("witness did not put team 95 first: first=%d/%d second=%d/%d",
			sorted.t[0].id, sorted.t[0].points, sorted.t[1].id, sorted.t[1].points)
	}
}
