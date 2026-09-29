package main

import (
	"encoding/json"
	"io"
	"log"
	"math/rand"
	"os"
	"testing"
)

func TestJointPointCapPropagation(t *testing.T) {
	// The target has at most five points; the leader already has six.
	// Teams 3 and 4 each have four and must draw their mutual game,
	// forcing both to lose to team 5. Team 5 then finishes on nine.
	problem := &jointPointCapProblem{
		base: []int{5, 6, 4, 4, 3},
		games: []jointPointCapGame{
			{home: 2, away: 3, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
			{home: 4, away: 2, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
			{home: 4, away: 3, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
		},
	}
	if impossible, nodes := problem.proveJointPointCapImpossible(0, 1, 5, 500); !impossible || nodes != 1 {
		t.Fatalf("second place: impossible=%v nodes=%d", impossible, nodes)
	}
	if impossible, _ := problem.proveJointPointCapImpossible(0, 2, 5, 500); impossible {
		t.Fatal("third place was incorrectly ruled out")
	}
}

func BenchmarkJointPointCapRealGroup16653(b *testing.B) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		b.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to a saved group request")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		b.Fatal(err)
	}
	var input GroupType
	if err := json.Unmarshal(data, &input); err != nil {
		b.Fatal(err)
	}
	if input.Id != 16653 {
		b.Skip("this benchmark uses group 16653")
	}
	b.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	b.Setenv("RARE_POSITION_RANDOM_SEED", "808")
	b.Setenv("RARE_POSITION_JOINT_POINT_CAP", "0")
	previous := log.Writer()
	log.SetOutput(io.Discard)
	defer log.SetOutput(previous)
	estimates := input.calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	keys := make([]uint32, len(input.Team_groups))
	for index, team := range input.Team_groups {
		keys[index] = uint32(team.Team_id)
	}
	table := NewTable(keys)
	campaign := make([]*TeamCampaign, len(keys))
	for _, team := range input.Team_groups {
		campaign[table.Query(uint32(team.Team_id))] = &TeamCampaign{
			id: team.Team_id, points: team.Add_sub,
			points_win:  input.Phase.Championship.Point_win,
			points_draw: input.Phase.Championship.Point_draw,
			points_loss: input.Phase.Championship.Point_loss,
		}
	}
	for _, game := range input.Games {
		game.home_table_index = table.Query(uint32(game.HomeId))
		game.away_table_index = table.Query(uint32(game.AwayId))
		if game.Played {
			campaign[game.home_table_index].add_game(game)
			campaign[game.away_table_index].add_game(game)
		}
	}
	problem := newJointPointCapProblem(&input, campaign)
	type target struct {
		index     int32
		rank, cap int
	}
	var targets []target
	for _, team := range input.Team_groups {
		index := table.Query(uint32(team.Team_id))
		cap := campaign[index].points
		for _, game := range problem.games {
			if game.home == index {
				cap += max(game.homeGain[0], max(game.homeGain[1], game.homeGain[2]))
			} else if game.away == index {
				cap += max(game.awayGain[0], max(game.awayGain[1], game.awayGain[2]))
			}
		}
		for rank := range input.Team_groups {
			if estimates[team.Team_id][rank].Reachability == "undecided" {
				targets = append(targets, target{index, rank, cap})
			}
		}
	}
	if len(targets) != 23 {
		b.Fatalf("undecided cells=%d, want 23", len(targets))
	}
	b.ResetTimer()
	for iteration := 0; iteration < b.N; iteration++ {
		for _, cell := range targets {
			_, _ = problem.proveJointPointCapImpossible(cell.index, cell.rank, cell.cap, jointPointCapNodesPerCell)
		}
	}
}

func TestJointPointCapNeverRejectsPossibleSmallSchedules(t *testing.T) {
	rng := rand.New(rand.NewSource(93))
	for trial := 0; trial < 40; trial++ {
		problem := &jointPointCapProblem{base: make([]int, 4)}
		for index := range problem.base {
			problem.base[index] = rng.Intn(9)
		}
		for gameIndex := 0; gameIndex < 5; gameIndex++ {
			home := rng.Intn(4)
			away := (home + 1 + rng.Intn(3)) % 4
			problem.games = append(problem.games, jointPointCapGame{
				home: int32(home), away: int32(away),
				homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0},
			})
		}
		dual := problem.negated()
		for target := 0; target < 4; target++ {
			cap := problem.base[target]
			floor := dual.base[target]
			for _, game := range problem.games {
				if game.home == int32(target) || game.away == int32(target) {
					cap += 3
				}
			}
			for _, game := range dual.games {
				if game.home == int32(target) {
					floor += max(game.homeGain[0], max(game.homeGain[1], game.homeGain[2]))
				} else if game.away == int32(target) {
					floor += max(game.awayGain[0], max(game.awayGain[1], game.awayGain[2]))
				}
			}
			var possible [4]bool
			for code := 0; code < 243; code++ {
				points := append([]int(nil), problem.base...)
				number := code
				for _, game := range problem.games {
					outcome := number % 3
					number /= 3
					points[game.home] += game.homeGain[outcome]
					points[game.away] += game.awayGain[outcome]
				}
				above, tied := 0, 0
				for index, pointTotal := range points {
					if index == target {
						continue
					}
					if pointTotal > points[target] {
						above++
					} else if pointTotal == points[target] {
						tied++
					}
				}
				for rank := above; rank <= above+tied; rank++ {
					possible[rank] = true
				}
			}
			for rank := 0; rank < 4; rank++ {
				if impossible, _ := problem.proveJointPointCapImpossible(int32(target), rank, cap, 500); impossible && possible[rank] {
					t.Fatalf("trial=%d target=%d rank=%d: rejected a possible schedule", trial, target, rank)
				}
				if impossible, _ := dual.proveJointPointCapImpossible(int32(target), 3-rank, floor, 500); impossible && possible[rank] {
					t.Fatalf("trial=%d target=%d rank=%d: floor rejected a possible schedule", trial, target, rank)
				}
			}
		}
	}
}

func TestJointPointCapRealGroup16653(t *testing.T) {
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
		t.Skip("this regression uses group 16653")
	}
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_RANDOM_SEED", "808")
	t.Setenv("RARE_POSITION_JOINT_POINT_CAP", "0")
	without := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	t.Setenv("RARE_POSITION_JOINT_POINT_CAP", "1")
	estimates := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	if est := estimates[95][1]; est.Reachability != "impossible_by_joint_points" ||
		est.Probability != 0 || est.ZeroHitUpper95 != 0 {
		t.Fatalf("Londrina second place: %+v", est)
	}
	if est := estimates[125][8]; est.Reachability != "impossible_by_joint_points" ||
		est.Probability != 0 || est.ZeroHitUpper95 != 0 {
		t.Fatalf("team 125 ninth place: %+v", est)
	}
	for _, team := range input.Team_groups {
		for rank := range input.Team_groups {
			before := without[team.Team_id][rank]
			after := estimates[team.Team_id][rank]
			if (before.Probability > 0) != (after.Probability > 0) {
				t.Fatalf("team=%d rank=%d: nonzero estimate changed", team.Team_id, rank+1)
			}
		}
	}
	firstTeam := input.Team_groups[0].Team_id
	if estimates[firstTeam][0].WorkSpent >= without[firstTeam][0].WorkSpent {
		t.Fatal("early cap proof did not reduce search work")
	}
}

func TestJointPointFloorRealGroup16498(t *testing.T) {
	path := os.Getenv("RARE_POSITION_BENCHMARK_GROUP_JSON")
	if path == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_GROUP_JSON to the group 16498 request")
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
		t.Skip("this regression uses group 16498")
	}
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_RANDOM_SEED", "808")
	t.Setenv("RARE_POSITION_JOINT_POINT_FLOOR", "0")
	without := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	t.Setenv("RARE_POSITION_JOINT_POINT_FLOOR", "1")
	estimates := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	for _, cell := range []conditionedZeroCell{{17, 15}, {16, 17}} {
		est := estimates[cell.id][cell.rank]
		if est.Probability != 0 || est.Reachability != "impossible_by_joint_points" {
			t.Fatalf("team=%d rank=%d: %+v", cell.id, cell.rank+1, est)
		}
	}
	for _, team := range input.Team_groups {
		for rank := range input.Team_groups {
			before := without[team.Team_id][rank]
			after := estimates[team.Team_id][rank]
			if (before.Probability > 0) != (after.Probability > 0) {
				t.Fatalf("team=%d rank=%d: nonzero estimate changed", team.Team_id, rank+1)
			}
		}
	}
	firstTeam := input.Team_groups[0].Team_id
	if estimates[firstTeam][0].WorkSpent >= without[firstTeam][0].WorkSpent {
		t.Fatal("early floor proof did not reduce search work")
	}
}
