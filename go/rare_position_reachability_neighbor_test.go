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
