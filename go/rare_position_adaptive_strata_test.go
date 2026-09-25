package main

import (
	"math"
	"testing"
)

func TestAdaptivePointStratifiedSearchScoutAndBounds(t *testing.T) {
	group, campaign, table, sortOrder, _ := createTestGroupForDiversified()
	workLimit := int64(500000)
	masterSeed := int64(12345)

	estimates, diag, ok := runAdaptivePointStratifiedSearch(group, campaign, table, sortOrder, workLimit, masterSeed)
	if !ok {
		t.Fatalf("expected adaptive point stratified search to succeed")
	}

	if diag.ScoutWork <= 0 {
		t.Errorf("expected positive scout work, got %d", diag.ScoutWork)
	}

	if diag.ProductionWork <= 0 {
		t.Errorf("expected positive production work, got %d", diag.ProductionWork)
	}

	totalSpent := diag.ScoutWork + diag.DiscoveryWork + diag.ValidationWork + diag.ProductionWork
	if totalSpent > workLimit {
		t.Errorf("total work spent %d exceeded limit %d", totalSpent, workLimit)
	}

	if len(estimates) != len(group.Team_groups) {
		t.Errorf("expected estimates for %d teams, got %d", len(group.Team_groups), len(estimates))
	}

	for teamID, posMap := range estimates {
		for pos, est := range posMap {
			if est.Probability < 0 || est.Probability > 1 {
				t.Errorf("invalid probability for team %d pos %d: %f", teamID, pos, est.Probability)
			}
			if math.IsNaN(est.StdErr) || est.StdErr < 0 {
				t.Errorf("invalid std_err for team %d pos %d: %f", teamID, pos, est.StdErr)
			}
		}
	}
}

func TestReconcileProbabilityMatrix(t *testing.T) {
	raw := map[int]map[int]float64{
		1: {0: 0.8, 1: 0.15, 2: 0.05},
		2: {0: 0.2, 1: 0.70, 2: 0.10},
		3: {0: 0.0, 1: 0.10, 2: 0.90},
	}
	teams := []int{1, 2, 3}

	reconciled := reconcileProbabilityMatrix(raw, teams)

	// Check row sums equal 1.0
	for _, tID := range teams {
		rowSum := 0.0
		for r := 0; r < 3; r++ {
			rowSum += reconciled[tID][r]
		}
		if math.Abs(rowSum-1.0) > 1e-6 {
			t.Errorf("team %d row sum = %f, want 1.0", tID, rowSum)
		}
	}

	// Check column sums equal 1.0
	for r := 0; r < 3; r++ {
		colSum := 0.0
		for _, tID := range teams {
			colSum += reconciled[tID][r]
		}
		if math.Abs(colSum-1.0) > 1e-6 {
			t.Errorf("rank %d column sum = %f, want 1.0", r, colSum)
		}
	}

	// Check zero entry remains zero
	if reconciled[3][0] != 0.0 {
		t.Errorf("reconciled[3][0] = %f, want 0.0", reconciled[3][0])
	}
}

func TestAdaptivePointStratifiedWorkAccountingAndFreeze(t *testing.T) {
	table := NewTable([]uint32{1, 2})
	group := &GroupType{
		Id:          16653,
		Team_groups: []TeamType{{Team_id: 1}, {Team_id: 2}},
		Games: []*GameType{
			{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1.5, AwayPower: 0.8, Played: false, home_table_index: 0, away_table_index: 1},
			{Id: 2, HomeId: 2, AwayId: 1, HomePower: 1.0, AwayPower: 1.2, Played: false, home_table_index: 1, away_table_index: 0},
		},
	}
	campaign := []*TeamCampaign{
		{id: 1, points: 17, points_win: 3, points_draw: 1, points_loss: 0},
		{id: 2, points: 20, points_win: 3, points_draw: 1, points_loss: 0},
	}
	sortOrder := []SortType{PT, GD}

	workLimit := int64(1000000)
	masterSeed := int64(9999)

	estimates, diag, ok := runAdaptivePointStratifiedSearch(group, campaign, table, sortOrder, workLimit, masterSeed)
	if !ok {
		t.Fatalf("expected adaptive search to run successfully")
	}

	spent := diag.ScoutWork + diag.DiscoveryWork + diag.ValidationWork + diag.ProductionWork
	if spent > workLimit {
		t.Fatalf("spent work %d > work limit %d", spent, workLimit)
	}

	for _, team := range group.Team_groups {
		for pos := 0; pos < len(group.Team_groups); pos++ {
			est := estimates[team.Team_id][pos]
			if !est.Available {
				t.Fatalf("estimate for team %d pos %d should be available", team.Team_id, pos)
			}
		}
	}
}

func TestAdaptivePointStratifiedAutomaticStratumSelection(t *testing.T) {
	// Group where team 1 is behind team 2 and requires points tail to reach 1st place
	table := NewTable([]uint32{1, 2})
	group := &GroupType{
		Id:          1001,
		Team_groups: []TeamType{{Team_id: 1}, {Team_id: 2}},
		Games: []*GameType{
			{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1.8, AwayPower: 0.5, Played: false, home_table_index: 0, away_table_index: 1},
			{Id: 2, HomeId: 1, AwayId: 2, HomePower: 1.8, AwayPower: 0.5, Played: false, home_table_index: 0, away_table_index: 1},
		},
	}
	campaign := []*TeamCampaign{
		{id: 1, points: 10, points_win: 3, points_draw: 1, points_loss: 0},
		{id: 2, points: 15, points_win: 3, points_draw: 1, points_loss: 0},
	}
	sortOrder := []SortType{PT, GD}

	workLimit := int64(500000)
	masterSeed := int64(42)

	_, diag, ok := runAdaptivePointStratifiedSearch(group, campaign, table, sortOrder, workLimit, masterSeed)
	if !ok {
		t.Fatalf("adaptive search failed")
	}

	// Verify upper bounds were computed for cells
	if len(diag.UpperBounds) == 0 {
		t.Errorf("expected non-empty upper bounds map")
	}

	// Verify reconciled matrix is populated
	if len(diag.ReconciledMatrix) != 2 {
		t.Errorf("expected 2 teams in reconciled matrix, got %d", len(diag.ReconciledMatrix))
	}
}

func TestAdaptivePointStratifiedPartitionIdentity(t *testing.T) {
	table := NewTable([]uint32{1, 2})
	game := &GameType{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1.2, AwayPower: 1.0,
		home_table_index: table.Query(1), away_table_index: table.Query(2)}
	base := []*TeamCampaign{
		{id: 1, points: -1, points_win: 3, points_draw: 1, points_loss: 0},
		{id: 2, points: 0, points_win: 3, points_draw: 1, points_loss: 0},
	}
	teams := []TeamType{{Team_id: 1}, {Team_id: 2}}

	universe, ok := pointStratumUniverse(1, base, table, []*GameType{game}, 10)
	if !ok {
		t.Fatal("expected a one-game outcome universe")
	}
	stratum, ok := makePointTailStratum(universe, 3)
	if !ok {
		t.Fatal("expected win stratum")
	}

	plainN, conditionalN := 100000, 10000
	plainCounts, outsideCounts := simulatePointHybridSeasons(base, []*GameType{game}, table, []SortType{PT}, teams, stratum, false, plainN, 123)
	condCounts, _ := simulatePointHybridSeasons(base, []*GameType{game}, table, []SortType{PT}, teams, stratum, true, conditionalN, 456)

	pOutside := float64(outsideCounts[0]) / float64(plainN)
	pInside := float64(condCounts[[2]int{1, 0}]) / float64(conditionalN)

	got := pOutside + stratum.Mass*pInside
	want := float64(plainCounts[[2]int{1, 0}]) / float64(plainN)

	se := math.Sqrt(want * (1 - want) / float64(plainN))
	if math.Abs(got-want) > 4*se {
		t.Fatalf("hybrid estimate %f differs from plain MC %f (SE %f)", got, want, se)
	}
}

func TestSimulateAdaptivePlainSeasonsOutsideTracking(t *testing.T) {
	table := NewTable([]uint32{1, 2})
	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1.2, AwayPower: 1.0, Played: false, home_table_index: table.Query(1), away_table_index: table.Query(2)},
		{Id: 2, HomeId: 1, AwayId: 2, HomePower: 1.2, AwayPower: 1.0, Played: false, home_table_index: table.Query(1), away_table_index: table.Query(2)},
	}
	base := []*TeamCampaign{
		{id: 1, points: 10, points_win: 3, points_draw: 1, points_loss: 0},
		{id: 2, points: 12, points_win: 3, points_draw: 1, points_loss: 0},
	}
	teams := []TeamType{{Team_id: 1}, {Team_id: 2}}

	universe, ok := pointStratumUniverse(1, base, table, games, 10)
	if !ok {
		t.Fatal("expected outcome universe")
	}
	// Stratum A: team 1 gains >= 6 points (2 wins)
	stratum, ok := makePointTailStratum(universe, 6)
	if !ok {
		t.Fatal("expected stratum A")
	}

	strata := map[int]*PointStratum{1: stratum}
	samples := 10000
	plainCounts, outsideCounts := simulateAdaptivePlainSeasons(base, games, table, []SortType{PT}, teams, strata, samples, 9876)

	// Team 1 can finish 2nd with < 6 points (e.g. 0, 1, 2, 3, or 4 points).
	// Therefore outsideCounts[[2]int{1, 1}] must be > 0.
	outside2nd := outsideCounts[[2]int{1, 1}]
	plain2nd := plainCounts[[2]int{1, 1}]

	if outside2nd <= 0 {
		t.Errorf("expected positive outsideCounts for team 1 at 2nd place, got %d", outside2nd)
	}

	if plain2nd < outside2nd {
		t.Errorf("plainCounts (%d) must be >= outsideCounts (%d)", plain2nd, outside2nd)
	}
}
