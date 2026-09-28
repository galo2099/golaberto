package main

import (
	"encoding/json"
	"math/rand"
	"os"
	"sort"
	"strings"
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

func TestGroup16653Team68TopThreeWitnesses(t *testing.T) {
	var fixture struct {
		GroupID       int    `json:"group_id"`
		PhaseID       int    `json:"phase_id"`
		TeamID        int    `json:"team_id"`
		Sort          string `json:"sort"`
		PointWin      int    `json:"point_win"`
		PointDraw     int    `json:"point_draw"`
		PointLoss     int    `json:"point_loss"`
		TeamIDs       []int  `json:"team_ids"`
		ChangesSecond []int  `json:"changes_for_second"`
		ChangesThird  []int  `json:"changes_for_third"`
		Games         []struct {
			ID             int     `json:"id"`
			HomeID         int     `json:"home_id"`
			AwayID         int     `json:"away_id"`
			PlayedScore    []int   `json:"played_score"`
			WitnessOutcome string  `json:"witness_outcome"`
			HomePower      float64 `json:"home_power"`
			AwayPower      float64 `json:"away_power"`
		} `json:"games"`
	}
	data, err := os.ReadFile("../experiments/rare_positions/2026-09-27-group-16653-team-68-top3-witness.json")
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatal(err)
	}
	if fixture.GroupID != 16653 || fixture.PhaseID != 4489 || fixture.TeamID != 68 ||
		len(fixture.TeamIDs) != 20 || len(fixture.Games) != 380 {
		t.Fatalf("unexpected Avaí witness metadata: group=%d phase=%d team=%d teams=%d games=%d",
			fixture.GroupID, fixture.PhaseID, fixture.TeamID, len(fixture.TeamIDs), len(fixture.Games))
	}
	keys := make([]uint32, len(fixture.TeamIDs))
	for i, id := range fixture.TeamIDs {
		keys[i] = uint32(id)
	}
	table := NewTable(keys)
	order := build_sorted_array(strings.Split(fixture.Sort, ","))
	usesHead := false
	for _, criterion := range order {
		if criterion == HEAD {
			usesHead = true
		}
	}
	base := make([]*TeamCampaign, len(keys))
	for _, id := range fixture.TeamIDs {
		base[table.Query(uint32(id))] = &TeamCampaign{
			id: id, points_win: fixture.PointWin, points_draw: fixture.PointDraw,
			points_loss: fixture.PointLoss, uses_head: usesHead,
		}
	}
	remaining := 0
	for _, game := range fixture.Games {
		if len(game.PlayedScore) == 2 {
			played := &GameType{Id: game.ID, HomeId: game.HomeID, AwayId: game.AwayID,
				HomeScore: game.PlayedScore[0], AwayScore: game.PlayedScore[1], Played: true}
			base[table.Query(uint32(game.HomeID))].add_game(played)
			base[table.Query(uint32(game.AwayID))].add_game(played)
			continue
		}
		if len(game.PlayedScore) != 0 || game.HomePower <= 0 || game.AwayPower <= 0 {
			t.Fatalf("game %d has an invalid unplayed fixture", game.ID)
		}
		remaining++
	}
	if remaining != 85 {
		t.Fatalf("remaining fixtures=%d, want 85", remaining)
	}
	for _, test := range []struct {
		rank    int
		changes []int
		top     []int
	}{
		{1, nil, []int{68, 2064, 588}},
		{2, fixture.ChangesSecond, []int{70, 68, 2064}},
		{3, fixture.ChangesThird, []int{70, 22, 68}},
	} {
		t.Run(string(rune('0'+test.rank)), func(t *testing.T) {
			changed := make(map[int]bool, len(test.changes))
			for _, id := range test.changes {
				changed[id] = true
			}
			campaign := make([]*TeamCampaign, len(base))
			for index, team := range base {
				campaign[index] = team.clone()
			}
			for _, game := range fixture.Games {
				if len(game.PlayedScore) != 0 {
					continue
				}
				outcome := game.WitnessOutcome
				if changed[game.ID] {
					if outcome != "draw" || game.HomeID == fixture.TeamID || game.AwayID == fixture.TeamID {
						t.Fatalf("game %d is not an independent draw to change", game.ID)
					}
					outcome = "home_win"
					delete(changed, game.ID)
				}
				score := &GameType{Id: game.ID, HomeId: game.HomeID, AwayId: game.AwayID, Played: true,
					home_table_index: table.Query(uint32(game.HomeID)),
					away_table_index: table.Query(uint32(game.AwayID))}
				switch outcome {
				case "home_win":
					score.HomeScore = 1
				case "away_win":
					score.AwayScore = 1
				case "draw":
				default:
					t.Fatalf("game %d has invalid witness outcome %q", game.ID, outcome)
				}
				addSimulatedGame(campaign[score.home_table_index], campaign[score.away_table_index], score)
			}
			if len(changed) != 0 {
				t.Fatalf("unknown game modifications: %v", changed)
			}
			teams := make([]*TeamCampaign, len(fixture.TeamIDs))
			for i, id := range fixture.TeamIDs {
				teams[i] = campaign[table.Query(uint32(id))]
			}
			sort.Sort(TeamCampaignSorted{t: teams, sort: order, rng: rand.New(rand.NewSource(1))})
			for i, want := range test.top {
				if teams[i].id != want {
					t.Fatalf("rank=%d: team=%d, want %d", i+1, teams[i].id, want)
				}
			}
			if teams[test.rank-1].id != fixture.TeamID || teams[test.rank-1].points != 57 {
				t.Fatalf("Avaí rank=%d has team=%d points=%d", test.rank,
					teams[test.rank-1].id, teams[test.rank-1].points)
			}
		})
	}
}
