package main

import (
	"math"
	"testing"
)

func TestAdaptiveScoutUsesSingleSimulationPass(t *testing.T) {
	group, campaign, table, sortOrder, _ := createTestGroupForDiversified()
	scoutSamples := 1000
	unplayed := len(group.Games)
	plainCost := estimateSeasonWork(unplayed, 1, len(group.Team_groups))

	scout := runPlainMCScoutWithJointPoints(campaign, group.Games, table, sortOrder, group.Team_groups, scoutSamples, plainCost, nil)

	if scout.Samples != scoutSamples {
		t.Fatalf("scout.Samples = %d, want %d", scout.Samples, scoutSamples)
	}

	expectedWork := int64(scoutSamples) * plainCost
	if scout.Work != expectedWork {
		t.Fatalf("scout.Work = %d, want %d", scout.Work, expectedWork)
	}

	for _, team := range group.Team_groups {
		id := team.Team_id
		ts := scout.TeamScout[id]
		if ts == nil {
			t.Fatalf("missing TeamScout for team %d", id)
		}

		sumRank := 0
		for _, c := range ts.RankCounts {
			sumRank += c
		}
		if sumRank != scoutSamples {
			t.Fatalf("team %d sum of rank counts = %d, want %d", id, sumRank, scoutSamples)
		}

		sumPoints := 0
		for _, c := range ts.PointCounts {
			sumPoints += c
		}
		if sumPoints != scoutSamples {
			t.Fatalf("team %d sum of point counts = %d, want %d", id, sumPoints, scoutSamples)
		}
	}
}

func TestAdditionalPointsPMFSumsToOne(t *testing.T) {
	table := NewTable([]uint32{1, 2})
	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1.2, AwayPower: 1.0, Played: false, home_table_index: 0, away_table_index: 1},
		{Id: 2, HomeId: 1, AwayId: 2, HomePower: 1.5, AwayPower: 0.8, Played: false, home_table_index: 0, away_table_index: 1},
	}
	base := []*TeamCampaign{
		{id: 1, points: 10, points_win: 3, points_draw: 1, points_loss: 0},
		{id: 2, points: 12, points_win: 3, points_draw: 1, points_loss: 0},
	}

	universe, ok := pointOutcomeUniverse(1, base, table, games, 10)
	if !ok {
		t.Fatal("expected point outcome universe")
	}

	pmf := additionalPointsPMF(universe)
	sum := 0.0
	for _, p := range pmf {
		sum += p
	}

	if math.Abs(sum-1.0) > 1e-12 {
		t.Fatalf("PMF sum = %.15g, want 1.0", sum)
	}
}

func TestAdditionalPointsPMFMatchesTailDP(t *testing.T) {
	table := NewTable([]uint32{1, 2})
	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1.3, AwayPower: 0.9, Played: false, home_table_index: 0, away_table_index: 1},
		{Id: 2, HomeId: 2, AwayId: 1, HomePower: 1.1, AwayPower: 1.0, Played: false, home_table_index: 1, away_table_index: 0},
	}
	base := []*TeamCampaign{
		{id: 1, points: 5, points_win: 3, points_draw: 1, points_loss: 0},
		{id: 2, points: 8, points_win: 3, points_draw: 1, points_loss: 0},
	}

	universe, ok := pointOutcomeUniverse(1, base, table, games, 10)
	if !ok {
		t.Fatal("expected point outcome universe")
	}

	pmf := additionalPointsPMF(universe)

	for threshold := 0; threshold <= 6; threshold++ {
		tailDP, ok := makePointTailStratum(universe, threshold)
		if !ok {
			continue
		}
		pmfTailSum := 0.0
		for s, p := range pmf {
			if s >= threshold {
				pmfTailSum += p
			}
		}
		if math.Abs(pmfTailSum-tailDP.Mass) > 1e-12 {
			t.Fatalf("threshold %d: PMF tail sum = %.15g, DP mass = %.15g", threshold, pmfTailSum, tailDP.Mass)
		}
	}
}

func TestPointSetStratumSamplingAndMass(t *testing.T) {
	table := NewTable([]uint32{1, 2})
	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1.2, AwayPower: 1.0, Played: false, home_table_index: 0, away_table_index: 1},
		{Id: 2, HomeId: 1, AwayId: 2, HomePower: 1.4, AwayPower: 0.9, Played: false, home_table_index: 0, away_table_index: 1},
	}
	base := []*TeamCampaign{
		{id: 1, points: 10, points_win: 3, points_draw: 1, points_loss: 0},
		{id: 2, points: 12, points_win: 3, points_draw: 1, points_loss: 0},
	}

	universe, ok := pointOutcomeUniverse(1, base, table, games, 10)
	if !ok {
		t.Fatal("expected point outcome universe")
	}

	pmf := additionalPointsPMF(universe)
	allowed := []int{2, 4, 6}

	stratum, ok := makePointSetStratum(universe, allowed)
	if !ok {
		t.Fatal("expected point set stratum")
	}

	expectedMass := 0.0
	for _, pts := range allowed {
		expectedMass += pmf[pts]
	}

	if math.Abs(stratum.Mass-expectedMass) > 1e-12 {
		t.Fatalf("point set stratum mass = %.15g, want %.15g", stratum.Mass, expectedMass)
	}
}

func TestComputePriorityEstimate(t *testing.T) {
	pmf := map[int]float64{0: 0.5, 3: 0.3, 6: 0.2}
	scout := &TeamPointRankScout{
		Samples:    100,
		RankCounts: []int{20, 80},
		PointRankCounts: map[int][]int{
			0: {0, 50},
			3: {5, 25},
			6: {15, 5},
		},
		PointCounts: map[int]int{0: 50, 3: 30, 6: 20},
	}

	priority := computePriorityEstimate(1, 0, pmf, scout, 2)
	if priority <= 0 || priority > 1 {
		t.Fatalf("priority estimate = %f, expected between 0 and 1", priority)
	}
}

func TestExactPositionPointsUpperBoundContainsTrueProbability(t *testing.T) {
	table := NewTable([]uint32{1, 2})
	game := &GameType{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1.2, AwayPower: 1.0,
		home_table_index: table.Query(1), away_table_index: table.Query(2)}
	base := []*TeamCampaign{
		{id: 1, points: 10, points_win: 3, points_draw: 1, points_loss: 0},
		{id: 2, points: 11, points_win: 3, points_draw: 1, points_loss: 0},
	}
	teamGroups := []TeamType{{Team_id: 1}, {Team_id: 2}}

	universe, ok := pointOutcomeUniverse(1, base, table, []*GameType{game}, 10)
	if !ok {
		t.Fatal("expected universe")
	}
	pmf := additionalPointsPMF(universe)

	hardUB, _, provenImp := computeHardCellUpperBound(1, 0, pmf, base, teamGroups, []*GameType{game}, table)
	trueP := targetOutcomeProbabilities(game)[2]

	if provenImp {
		t.Fatalf("rank 0 should not be proven impossible for team 1")
	}

	if trueP > hardUB+1e-12 {
		t.Fatalf("true P(1st) = %.15g > hard upper bound %.15g", trueP, hardUB)
	}
}

func TestBetterAndWorseTailUpperBounds(t *testing.T) {
	table := NewTable([]uint32{1, 2, 3})
	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1.2, AwayPower: 1.0, Played: false, home_table_index: 0, away_table_index: 1},
		{Id: 2, HomeId: 1, AwayId: 3, HomePower: 1.2, AwayPower: 1.0, Played: false, home_table_index: 0, away_table_index: 2},
		{Id: 3, HomeId: 2, AwayId: 3, HomePower: 1.0, AwayPower: 1.5, Played: false, home_table_index: 1, away_table_index: 2},
	}
	base := []*TeamCampaign{
		{id: 1, points: 10, points_win: 3, points_draw: 1, points_loss: 0},
		{id: 2, points: 15, points_win: 3, points_draw: 1, points_loss: 0},
		{id: 3, points: 12, points_win: 3, points_draw: 1, points_loss: 0},
	}
	teamGroups := []TeamType{{Team_id: 1}, {Team_id: 2}, {Team_id: 3}}

	universe, ok := pointOutcomeUniverse(1, base, table, games, 10)
	if !ok {
		t.Fatal("expected universe")
	}
	pmf := additionalPointsPMF(universe)

	ub1st, _, provenImp1st := computeHardCellUpperBound(1, 0, pmf, base, teamGroups, games, table)
	if provenImp1st || ub1st <= 0 {
		t.Fatalf("1st place should be possible with 2 wins for team 1")
	}

	ub3rd, _, provenImp3rd := computeHardCellUpperBound(1, 2, pmf, base, teamGroups, games, table)
	if provenImp3rd || ub3rd <= 0 {
		t.Fatalf("3rd place should be possible for team 1")
	}
}

func TestAdaptiveScoutRecordsPointRankJointCounts(t *testing.T) {
	group, campaign, table, sortOrder, _ := createTestGroupForDiversified()
	scout := runPlainMCScoutWithJointPoints(campaign, group.Games, table, sortOrder, group.Team_groups, 500, 100, nil)

	if len(scout.TeamScout) != len(group.Team_groups) {
		t.Fatalf("expected joint scout for %d teams, got %d", len(group.Team_groups), len(scout.TeamScout))
	}

	for _, team := range group.Team_groups {
		ts := scout.TeamScout[team.Team_id]
		if ts == nil {
			t.Fatalf("missing joint scout for team %d", team.Team_id)
		}
		if len(ts.PointCounts) == 0 {
			t.Fatalf("team %d should have non-empty PointCounts", team.Team_id)
		}
		if len(ts.PointRankCounts) == 0 {
			t.Fatalf("team %d should have non-empty PointRankCounts", team.Team_id)
		}
	}
}

func TestAdaptiveValidationNeverExceedsGlobalCap(t *testing.T) {
	group, campaign, table, sortOrder, _ := createTestGroupForDiversified()
	workLimit := int64(1000000)
	masterSeed := int64(12345)

	_, diag, ok := runAdaptivePointStratifiedSearch(group, campaign, table, sortOrder, workLimit, masterSeed)
	if !ok {
		t.Fatalf("adaptive search failed")
	}

	unplayed := len(group.Games)
	plainCost := estimateSeasonWork(unplayed, 1, len(group.Team_groups))
	valWorkCap := int64(3000) * plainCost

	if diag.ValidationWork > valWorkCap {
		t.Fatalf("validation work %d > global cap %d", diag.ValidationWork, valWorkCap)
	}
}

func TestAdaptiveProductionKeepsMinimumPlainFraction(t *testing.T) {
	group, campaign, table, sortOrder, _ := createTestGroupForDiversified()
	workLimit := int64(1000000)
	masterSeed := int64(12345)

	_, diag, ok := runAdaptivePointStratifiedSearch(group, campaign, table, sortOrder, workLimit, masterSeed)
	if !ok {
		t.Fatalf("adaptive search failed")
	}

	unplayed := len(group.Games)
	plainCost := estimateSeasonWork(unplayed, 1, len(group.Team_groups))
	plainProdWork := int64(diag.PlainSamples) * plainCost

	plainFraction := float64(plainProdWork) / float64(diag.ProductionWork)
	if plainFraction < 0.799 {
		t.Fatalf("plain production fraction %f < 80%% floor", plainFraction)
	}
}

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

	for _, tID := range teams {
		rowSum := 0.0
		for r := 0; r < 3; r++ {
			rowSum += reconciled[tID][r]
		}
		if math.Abs(rowSum-1.0) > 1e-6 {
			t.Errorf("team %d row sum = %f, want 1.0", tID, rowSum)
		}
	}

	for r := 0; r < 3; r++ {
		colSum := 0.0
		for _, tID := range teams {
			colSum += reconciled[tID][r]
		}
		if math.Abs(colSum-1.0) > 1e-6 {
			t.Errorf("rank %d column sum = %f, want 1.0", r, colSum)
		}
	}

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

	if len(diag.UpperBounds) == 0 {
		t.Errorf("expected non-empty upper bounds map")
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
	stratum, ok := makePointTailStratum(universe, 6)
	if !ok {
		t.Fatal("expected stratum A")
	}

	strata := map[int]*PointStratum{1: stratum}
	samples := 10000
	plainCounts, outsideCounts := simulateAdaptivePlainSeasons(base, games, table, []SortType{PT}, teams, strata, samples, 9876)

	outside2nd := outsideCounts[[2]int{1, 1}]
	plain2nd := plainCounts[[2]int{1, 1}]

	if outside2nd <= 0 {
		t.Errorf("expected positive outsideCounts for team 1 at 2nd place, got %d", outside2nd)
	}

	if plain2nd < outside2nd {
		t.Errorf("plainCounts (%d) must be >= outsideCounts (%d)", plain2nd, outside2nd)
	}
}

func TestPointRankProfileDistance(t *testing.T) {
	a := []float64{0.5, 0.3, 0.2}
	b := []float64{0.5, 0.3, 0.2}
	if dist := pointRankProfileDistance(a, b); dist != 0.0 {
		t.Errorf("expected 0 for identical vectors, got %f", dist)
	}

	c := []float64{1.0, 0.0, 0.0}
	d := []float64{0.0, 1.0, 0.0}
	if dist := pointRankProfileDistance(c, d); math.Abs(dist-1.0) > 1e-6 {
		t.Errorf("expected 1.0 for disjoint vectors, got %f", dist)
	}
}

func TestPointRankProfileNormalization(t *testing.T) {
	pmf := map[int]float64{0: 0.2, 3: 0.5, 6: 0.3}
	scout := &TeamPointRankScout{
		Samples: 100,
		PointCounts: map[int]int{0: 20, 3: 50, 6: 30},
		PointRankCounts: map[int][]int{
			0: {0, 5, 15},
			3: {10, 30, 10},
			6: {25, 5, 0},
		},
	}

	for pts := range pmf {
		prof := computePointRankProfile(pts, pmf, scout, 3, 1.5)
		probSum := 0.0
		for _, p := range prof.RankProb {
			probSum += p
		}
		if math.Abs(probSum-1.0) > 1e-6 {
			t.Errorf("profile for points %d rank sum = %f, want 1.0", pts, probSum)
		}
	}
}

func TestPointProfileGrouping(t *testing.T) {
	profiles := []PointRankProfile{
		{AddedPoints: 10, PointMass: 0.1, RankProb: []float64{0.9, 0.1, 0.0}},
		{AddedPoints: 11, PointMass: 0.1, RankProb: []float64{0.85, 0.15, 0.0}},
		{AddedPoints: 12, PointMass: 0.1, RankProb: []float64{0.0, 0.1, 0.9}},
	}

	groups := groupPointRankProfiles(profiles, 3, "profile", 0.20)
	if len(groups) < 2 {
		t.Fatalf("expected at least 2 base groups due to TVD split between 11 and 12, got %d", len(groups))
	}

	base0 := groups[0]
	if len(base0.AllowedPoints) != 2 || base0.AllowedPoints[0] != 10 || base0.AllowedPoints[1] != 11 {
		t.Errorf("expected base group 0 to have allowed points [10, 11], got %v", base0.AllowedPoints)
	}
}

func TestPointRankProfileDoesNotLeakNearbyRanksExcessively(t *testing.T) {
	pmf := map[int]float64{0: 0.5, 3: 0.5}
	scout := &TeamPointRankScout{
		Samples: 100,
		PointCounts: map[int]int{0: 50, 3: 50},
		PointRankCounts: map[int][]int{
			0: {0, 0, 50},
			3: {50, 0, 0},
		},
	}

	prof0 := computePointRankProfile(0, pmf, scout, 3, 1.5)
	// Without rank-distance blurring, rank 0 probability at point total 0 is ~0.119 (from point-distance weighting e^-2),
	// far below the ~0.51 that rank-distance kernel leakage produced.
	if prof0.RankProb[0] > 0.15 {
		t.Errorf("expected rank 0 prob at points 0 to be <= 0.15 without rank-distance leakage, got %f", prof0.RankProb[0])
	}
}

func TestPerCellValidationGating(t *testing.T) {
	scout := &TeamPointRankScout{
		PointRankCounts: map[int][]int{
			0: {10, 20},
			3: {5, 25},
			6: {30, 0},
		},
	}
	outsideHits := directScoutOutsideHits(1, 0, []int{6}, scout)
	if outsideHits != 15 {
		t.Errorf("expected 15 outside hits, got %d", outsideHits)
	}
}
