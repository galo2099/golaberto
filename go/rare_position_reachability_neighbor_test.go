package main

import (
	"encoding/json"
	"os"
	"testing"
)

func TestReachabilityNeighborsVerifyCompleteStandings(t *testing.T) {
	group, campaign, table, order, _ := createTestGroupForDiversified()
	cells := make([]conditionedZeroCell, 0, 9)
	estimates := make(map[int]map[int]ProductionEstimate)
	for _, team := range group.Team_groups {
		estimates[team.Team_id] = make(map[int]ProductionEstimate)
		for rank := range group.Team_groups {
			cells = append(cells, conditionedZeroCell{team.Team_id, rank})
			estimates[team.Team_id][rank] = ProductionEstimate{Reachability: "undecided"}
		}
	}
	seed := []uint8{1, 1, 1}
	results := []conditionedZeroSearchResult{{witnessOutcomes: seed}}
	proofs, attempts := searchReachabilityNeighbors(group, campaign, table,
		order, cells, results, estimates, 100)
	if len(proofs) == 0 || attempts == 0 || attempts > 100 {
		t.Fatalf("proofs=%d attempts=%d", len(proofs), attempts)
	}
	for _, proof := range proofs {
		if got := canonicalReachabilityRank(group, campaign, table, order,
			proof.outcomes, proof.cell.id); got != proof.cell.rank {
			t.Errorf("team=%d rank=%d: complete standings gave %d",
				proof.cell.id, proof.cell.rank, got)
		}
		for index, game := range group.Games {
			if targetOutcomeProbabilities(game)[proof.outcomes[index]] <= 0 {
				t.Errorf("team=%d rank=%d: impossible outcome for game %d",
					proof.cell.id, proof.cell.rank, game.Id)
			}
		}
	}
}

func TestReachabilityNeighborWalkUsesVerifiedIntermediateAssignment(t *testing.T) {
	group := &GroupType{Id: 2, Team_groups: []TeamType{
		{Team_id: 1, Bias: 0}, {Team_id: 2, Bias: 1},
		{Team_id: 3, Bias: 2}, {Team_id: 4, Bias: 3},
		{Team_id: 5, Bias: 4},
	}}
	for index := 0; index < 4; index++ {
		group.Games = append(group.Games, &GameType{
			Id: index + 1, HomeId: 1, AwayId: 5,
			HomePower: 1.5, AwayPower: 1.2,
			home_table_index: 0, away_table_index: 4,
		})
	}
	table := NewTable([]uint32{1, 2, 3, 4, 5})
	campaign := []*TeamCampaign{
		{id: 1, points: 0, points_win: 3, points_draw: 1},
		{id: 2, points: 5, points_win: 3, points_draw: 1},
		{id: 3, points: 8, points_win: 3, points_draw: 1},
		{id: 4, points: 11, points_win: 3, points_draw: 1},
		{id: 5, points: 0, points_win: 3, points_draw: 1},
	}
	order := []SortType{PT, GD, GF, BIAS}
	seed := []uint8{0, 0, 0, 0}
	intermediate := []uint8{2, 2, 0, 0}
	intermediateRank := canonicalReachabilityRank(group, campaign, table, order, intermediate, 1)
	if intermediateRank <= 0 || intermediateRank >= 4 {
		t.Fatalf("unexpected intermediate rank %d", intermediateRank)
	}
	cells := []conditionedZeroCell{{id: 1, rank: intermediateRank}, {id: 1, rank: 0}}
	makeEstimates := func() map[int]map[int]ProductionEstimate {
		return map[int]map[int]ProductionEstimate{1: {
			intermediateRank: {Reachability: "undecided"},
			0:                {Reachability: "undecided"},
		}}
	}
	oneRound := makeEstimates()
	proofs, attempts := extendReachabilityNeighbors(group, campaign, table,
		order, cells, [][]uint8{seed}, oneRound, 1, 1000)
	if proofs != 1 || attempts > 1000 || oneRound[1][intermediateRank].Reachability != "reachable_by_construction" ||
		oneRound[1][0].Reachability != "undecided" {
		t.Fatalf("one round: proofs=%d attempts=%d estimates=%+v", proofs, attempts, oneRound[1])
	}
	twoRounds := makeEstimates()
	proofs, attempts = extendReachabilityNeighbors(group, campaign, table,
		order, cells, [][]uint8{seed}, twoRounds, 2, 1000)
	if proofs != 2 || attempts > 2000 || twoRounds[1][0].Reachability != "reachable_by_construction" {
		t.Fatalf("two rounds: proofs=%d attempts=%d estimates=%+v", proofs, attempts, twoRounds[1])
	}
}

func TestReachabilityNeighborsRealGroup16653(t *testing.T) {
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
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "0")
	t.Setenv("RARE_POSITION_RANDOM_SEED", "808")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_POINT_TILT", "1")
	t.Setenv("RARE_POSITION_CONDITIONED_ZERO_DEEP", "0")
	t.Setenv("RARE_POSITION_NEIGHBORHOOD_SEARCH", "1")
	estimates := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
	for _, rank := range []int{1, 2} {
		est := estimates[68][rank]
		if est.Probability <= 0 || est.Reachability != "witness" ||
			est.Design != "matched_point_pool_conditioned_point_tilt" || est.ESS < 8 {
			t.Errorf("Avaí rank=%d: invalid point-tilt estimate: %+v", rank+1, est)
		}
		t.Logf("Avaí rank=%d probability=%g ess=%g reachability=%s", rank+1, est.Probability, est.ESS, est.Reachability)
	}
}
