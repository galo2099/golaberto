package main

import (
	"encoding/json"
	"os"
	"testing"
)

func TestJointPointWitnessVerifiesFullStandings(t *testing.T) {
	group, campaign, table, order, _ := createTestGroupForDiversified()
	problem := newJointPointCapProblem(group, campaign)
	search := jointPointWitnessSearch{
		problem: problem, group: group, campaign: campaign, table: table, order: order,
		cell: conditionedZeroCell{id: 2, rank: 0}, cap: 15, maxNodes: 300,
	}
	if !search.find(table.Query(2)) {
		t.Fatal("could not construct an attainable first rank")
	}
	if search.nodes > 300 || search.verified == 0 {
		t.Fatalf("nodes=%d verified=%d", search.nodes, search.verified)
	}
	if got := canonicalReachabilityRank(group, campaign, table, order, search.proof, 2); got != 0 {
		t.Fatalf("constructed rank=%d, want 0", got)
	}
	for index, game := range group.Games {
		if targetOutcomeProbabilities(game)[search.proof[index]] <= 0 {
			t.Fatalf("game %d has an impossible constructed outcome", game.Id)
		}
	}
}

func TestJointPointWitnessMinimumPointsVerifiesFullStandings(t *testing.T) {
	group, campaign, table, order, _ := createTestGroupForDiversified()
	problem := newJointPointCapProblem(group, campaign)
	target := int32(table.Query(2))
	minimum := problem.base[target]
	for _, game := range problem.games {
		if game.home == target {
			minimum += min(game.homeGain[0], min(game.homeGain[1], game.homeGain[2]))
		} else if game.away == target {
			minimum += min(game.awayGain[0], min(game.awayGain[1], game.awayGain[2]))
		}
	}
	for rank := 0; rank < len(group.Team_groups); rank++ {
		search := jointPointWitnessSearch{
			problem: problem, group: group, campaign: campaign, table: table, order: order,
			cell: conditionedZeroCell{id: 2, rank: rank}, cap: minimum, maxNodes: 300,
			targetMinimum: true,
		}
		if !search.find(target) {
			continue
		}
		if got := canonicalReachabilityRank(group, campaign, table, order, search.proof, 2); got != rank {
			t.Fatalf("constructed minimum-point rank=%d, want %d", got, rank)
		}
		for index, game := range group.Games {
			if targetOutcomeProbabilities(game)[search.proof[index]] <= 0 {
				t.Fatalf("game %d has an impossible minimum-point outcome", game.Id)
			}
		}
		t.Logf("minimum-point witness: team=2 rank=%d points=%d nodes=%d", rank+1, minimum, search.nodes)
		return
	}
	t.Fatalf("could not construct a minimum-point finish for team 2")
}

func TestJointPointWitnessReferenceGroups(t *testing.T) {
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
	var cells []conditionedZeroCell
	switch input.Id {
	case 16653:
		cells = []conditionedZeroCell{{95, 2}, {125, 9}}
	case 16498:
		cells = []conditionedZeroCell{{20, 1}, {110, 2}, {318, 2}}
	default:
		t.Skip("this regression uses group 16653 or 16498")
	}
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL", "1")
	t.Setenv("RARE_POSITION_RANDOM_SEED", "808")
	t.Setenv("RARE_POSITION_JOINT_POINT_WITNESS", "1")
	estimates := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	for _, cell := range cells {
		est := estimates[cell.id][cell.rank]
		if est.Reachability != "reachable_by_construction" || est.Probability != 0 || est.ZeroHitUpper95 <= 0 {
			t.Errorf("team=%d rank=%d: %+v", cell.id, cell.rank+1, est)
		}
	}
	if input.Id == 16653 {
		// An independent 85-fixture CSP assignment from the point-cap domains.
		// Digits follow the unplayed fixtures in request order: home loss,
		// draw, home win. The production search may find a different witness.
		const assignment = "0122010121210002112000222010000022020222202202110200100121210202022002010202002002112"
		campaign, table, order := diversifiedBenchmarkSetup(&input)
		outcomes := make([]uint8, len(input.Games))
		remaining := 0
		for index, game := range input.Games {
			if game.Played {
				continue
			}
			if remaining >= len(assignment) {
				t.Fatal("assignment is shorter than the remaining fixture list")
			}
			outcomes[index] = assignment[remaining] - '0'
			if targetOutcomeProbabilities(game)[outcomes[index]] <= 0 {
				t.Fatalf("fixture %d has a zero-probability outcome", game.Id)
			}
			remaining++
		}
		if remaining != len(assignment) {
			t.Fatalf("assignment length=%d, remaining fixtures=%d", len(assignment), remaining)
		}
		for _, teamID := range []int{70, 22, 95, 588, 2064, 73, 279} {
			t.Logf("witness team=%d rank=%d", teamID,
				canonicalReachabilityRank(&input, campaign, table, order, outcomes, teamID)+1)
		}
		if got := canonicalReachabilityRank(&input, campaign, table, order, outcomes, 95); got != 2 {
			t.Fatalf("complete fixture assignment puts Londrina in rank %d, want 3", got+1)
		}
	}
}
