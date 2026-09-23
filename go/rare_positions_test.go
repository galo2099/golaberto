package main

import (
	"math"
	"math/rand"
	"testing"
)

func TestWorkAccounting(t *testing.T) {
	work0 := estimateSeasonWork(0, 1, 4)
	if work0 != 5 {
		t.Errorf("expected 5 work units when 0 unplayed games, got %d", work0)
	}

	work4Games := estimateSeasonWork(4, 4, 4)
	if work4Games != 20 {
		t.Errorf("expected 20 work units for 4 unplayed games, 4 comps, 4 teams; got %d", work4Games)
	}

	maxWork := calculateMaxRareWork(4, 4)
	if maxWork != 800000 {
		t.Errorf("expected 800000 max rare work, got %d", maxWork)
	}
}

func TestWorkBudgetScaling(t *testing.T) {
	testCases := []struct {
		unplayed int
		teams    int
		expected int64
	}{
		{0, 2, 100000 * 3},
		{1, 2, 100000 * 3},
		{5, 4, 100000 * 9},
		{10, 20, 100000 * 30},
	}

	for _, tc := range testCases {
		maxWork := calculateMaxRareWork(tc.unplayed, tc.teams)
		if maxWork != tc.expected {
			t.Errorf("unplayed=%d, teams=%d: expected maxWork=%d, got %d", tc.unplayed, tc.teams, tc.expected, maxWork)
		}
	}
}

func TestPositionSearchStatusString(t *testing.T) {
	statuses := []PositionSearchStatus{
		StatusUnexplored,
		StatusObserved,
		StatusFrontier,
		StatusPromising,
		StatusResolved,
		StatusExhausted,
		StatusProvenImpossible,
		StatusBelowInterest,
	}

	for _, st := range statuses {
		if st.String() == "" || st.String()[:7] == "Unknown" {
			t.Errorf("status %d failed string representation", st)
		}
	}
}

func TestBuildSearchProposalForLevel(t *testing.T) {
	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2, HomePower: 0.1, AwayPower: 2.0, Played: false},
	}
	originalMeans := []GameProposalMeans{{Home: 0.1, Away: 2.0}}
	normalRanks := map[int]float64{1: 1.0, 2: 0.0}

	prop := buildSearchProposalForLevel(games, 1, RareBetter, 0, 3, normalRanks, originalMeans)

	if prop.StrengthLevel != 3 {
		t.Errorf("expected strength level 3, got %d", prop.StrengthLevel)
	}
	if len(prop.Components) != 4 {
		t.Errorf("expected 4 components in proposal mixture, got %d", len(prop.Components))
	}
}

func TestNonContiguousFrontierDiscovery(t *testing.T) {
	normalCounts := []int{100, 0, 500, 9400, 0}
	normalProbs := []float64{0.01, 0.0, 0.05, 0.94, 0.0}

	teamGroups := []TeamType{
		{Team_id: 1, Bias: 0},
		{Team_id: 2, Bias: 1},
		{Team_id: 3, Bias: 2},
		{Team_id: 4, Bias: 3},
		{Team_id: 5, Bias: 4},
	}
	table := NewTable([]uint32{1, 2, 3, 4, 5})
	campaign := make([]*TeamCampaign, 5)
	for i := 1; i <= 5; i++ {
		campaign[table.Query(uint32(i))] = &TeamCampaign{id: i, points: 0, bias: i - 1, points_win: 3, points_draw: 1, points_loss: 0}
	}
	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1.0, AwayPower: 1.0, Played: false},
	}
	sortOrder := []SortType{PT, GD, GF, BIAS}

	teamSearch := initializeTeamRareSearch(1, normalCounts, normalProbs, campaign, teamGroups, games, table, sortOrder)

	if teamSearch.Positions[0].Status != StatusObserved {
		t.Errorf("expected pos 0 StatusObserved, got %s", teamSearch.Positions[0].Status)
	}
	if teamSearch.Positions[1].Status != StatusUnexplored {
		t.Errorf("expected pos 1 StatusUnexplored, got %s", teamSearch.Positions[1].Status)
	}
	if teamSearch.Positions[2].Status != StatusObserved {
		t.Errorf("expected pos 2 StatusObserved, got %s", teamSearch.Positions[2].Status)
	}

	frontier := discoverFrontier(teamSearch)
	if len(frontier) != 2 {
		t.Fatalf("expected 2 frontier candidates (pos 1 and pos 4), got %d", len(frontier))
	}

	foundPos1 := false
	foundPos4 := false
	for _, cand := range frontier {
		if cand.Position == 1 {
			foundPos1 = true
			if cand.SearchState.Status != StatusFrontier {
				t.Errorf("expected candidate pos 1 status StatusFrontier, got %s", cand.SearchState.Status)
			}
		}
		if cand.Position == 4 {
			foundPos4 = true
		}
	}

	if !foundPos1 || !foundPos4 {
		t.Errorf("expected frontier candidates for both gap pos 1 and adjacent pos 4")
	}
}

func TestProposalTargetRanks(t *testing.T) {
	candidates := []int{7, 8, 9, 10, 11, 12, 13, 14, 15}
	mild, medium, strong := proposalTargetRanks(candidates, 16.0, RareBetter)
	if mild != 15 || medium != 11 || strong != 7 {
		t.Errorf("expected mild=15, medium=11, strong=7 for RareBetter, got mild=%d, medium=%d, strong=%d", mild, medium, strong)
	}

	candidatesWorse := []int{17, 18, 19, 20}
	mildW, mediumW, strongW := proposalTargetRanks(candidatesWorse, 16.0, RareWorse)
	if mildW != 17 || mediumW != 19 || strongW != 20 {
		t.Errorf("expected mild=17, medium=19, strong=20 for RareWorse, got mild=%d, medium=%d, strong=%d", mildW, mediumW, strongW)
	}

	m1, med1, s1 := proposalTargetRanks([]int{5}, 10.0, RareBetter)
	if m1 != 5 || med1 != 5 || s1 != 5 {
		t.Errorf("expected all 5 for 1 candidate, got %d, %d, %d", m1, med1, s1)
	}
}

func TestMultiComponentMixtureWeights(t *testing.T) {
	logQOverP := []float64{0.0, 0.0, 0.0, 0.0}
	weights := []float64{0.05, 0.20, 0.45, 0.30}
	w := mixtureImportanceWeightMulti(logQOverP, weights)
	if math.Abs(w-1.0) > 1e-9 {
		t.Errorf("expected weight 1.0 when all components equal P, got %f", w)
	}

	logQOverPExtreme := []float64{0.0, 100.0, 500.0, 1000.0}
	wBound := mixtureImportanceWeightMulti(logQOverPExtreme, weights)
	if wBound > 20.0+1e-9 {
		t.Errorf("expected weight <= 20.0, got %f", wBound)
	}
}

func TestDirectImportanceSamplingEstimator(t *testing.T) {
	hMean := 0.05
	aMean := 5.0

	exactPWin := 0.0
	for h := 1; h <= 20; h++ {
		pH := poisson_pmf(hMean, float64(h))
		pALessH := 0.0
		for a := 0; a < h; a++ {
			pALessH += poisson_pmf(aMean, float64(a))
		}
		exactPWin += pH * pALessH
	}

	teamGroups := []TeamType{{Team_id: 1, Bias: 0}, {Team_id: 2, Bias: 1}}
	table := NewTable([]uint32{1, 2})
	campaign := make([]*TeamCampaign, 2)
	campaign[table.Query(1)] = &TeamCampaign{id: 1, points: 0, bias: 0, points_win: 3, points_draw: 1, points_loss: 0}
	campaign[table.Query(2)] = &TeamCampaign{id: 2, points: 0, bias: 1, points_win: 3, points_draw: 1, points_loss: 0}

	games := []*GameType{
		{
			Id:               1,
			HomeId:           1,
			AwayId:           2,
			HomePower:        hMean,
			AwayPower:        aMean,
			Played:           false,
			home_table_index: table.Query(1),
			away_table_index: table.Query(2),
		},
	}
	sortOrder := []SortType{PT, GD, GF, BIAS}

	originalMeans := []GameProposalMeans{{Home: hMean, Away: aMean}}
	job := &RareSimulationJob{
		TeamID:             1,
		Direction:          RareBetter,
		CandidatePositions: []int{0},
		Components: []ProposalComponent{
			{Name: "original", Weight: 0.05, Means: originalMeans},
			{Name: "test_prop", Weight: 0.95, Means: []GameProposalMeans{{Home: 1.0, Away: 0.5}}},
		},
		Iterations: 20000,
	}

	rng := rand.New(rand.NewSource(12345))

	results, _ := estimateRarePositionsForJob(
		campaign, games, originalMeans, table, sortOrder, teamGroups,
		job, rng, 100,
	)

	est, ok := results[0]
	if !ok || !est.Found {
		t.Fatalf("expected estimate for position 0 to be found")
	}

	if math.Abs(est.Probability-exactPWin) > 3.0*est.StdErr {
		t.Errorf("Estimate %f was not within 3 stdErr (%f) of exact P %f", est.Probability, est.StdErr, exactPWin)
	}
}

func TestCompetitorDependentExactPoissonValidation(t *testing.T) {
	hMean := 0.05
	aMean := 5.0

	exactPHomeWin := 0.0
	for h := 1; h <= 20; h++ {
		pH := poisson_pmf(hMean, float64(h))
		pALessH := 0.0
		for a := 0; a < h; a++ {
			pALessH += poisson_pmf(aMean, float64(a))
		}
		exactPHomeWin += pH * pALessH
	}

	teamGroups := []TeamType{
		{Team_id: 1, Add_sub: 0, Bias: 15},
		{Team_id: 2, Add_sub: 0, Bias: 20},
		{Team_id: 3, Add_sub: 0, Bias: 10},
	}
	table := NewTable([]uint32{1, 2, 3})
	campaign := make([]*TeamCampaign, 3)
	campaign[table.Query(1)] = &TeamCampaign{id: 1, points: 0, bias: 15, points_win: 3, points_draw: 1, points_loss: 0}
	campaign[table.Query(2)] = &TeamCampaign{id: 2, points: 0, bias: 20, points_win: 3, points_draw: 1, points_loss: 0}
	campaign[table.Query(3)] = &TeamCampaign{id: 3, points: 0, bias: 10, points_win: 3, points_draw: 1, points_loss: 0}

	games := []*GameType{
		{
			Id:               1,
			HomeId:           2,
			AwayId:           3,
			HomePower:        hMean,
			AwayPower:        aMean,
			Played:           false,
			home_table_index: table.Query(2),
			away_table_index: table.Query(3),
		},
	}
	sortOrder := []SortType{PT, GD, GF, BIAS}

	originalMeans := []GameProposalMeans{{Home: hMean, Away: aMean}}

	compMeans := []GameProposalMeans{{Home: clampMean(hMean * 0.4), Away: clampMean(aMean * 2.5)}}
	components := []ProposalComponent{
		{Name: "original", Weight: 0.05, Means: originalMeans},
		{Name: "test_comp", Weight: 0.95, Means: compMeans},
	}

	job := &RareSimulationJob{
		TeamID:             1,
		Direction:          RareBetter,
		CandidatePositions: []int{1},
		Components:         components,
		Iterations:         20000,
	}

	rng := rand.New(rand.NewSource(42))

	results, _ := estimateRarePositionsForJob(
		campaign, games, originalMeans, table, sortOrder, teamGroups,
		job, rng, 888,
	)

	est, ok := results[1]
	if !ok || !est.Found {
		t.Fatalf("expected estimate for Team 1 position 1 (2nd place) to be found")
	}

	if est.Hits < 10 {
		t.Errorf("expected proposal to generate candidate hits, got %d hits", est.Hits)
	}

	if est.ESS < 10.0 {
		t.Errorf("expected ESS >= 10, got %f", est.ESS)
	}
}

func TestTargetWithNoGamesRemainingCompetitorProposal(t *testing.T) {
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")

	teamGroups := []TeamType{
		{Team_id: 1, Add_sub: 0, Bias: 1},
		{Team_id: 2, Add_sub: 0, Bias: 0},
		{Team_id: 3, Add_sub: 0, Bias: 0},
	}
	table := NewTable([]uint32{1, 2, 3})
	campaign := make([]*TeamCampaign, 3)
	campaign[table.Query(1)] = &TeamCampaign{id: 1, points: 0, bias: 1, points_win: 3, points_draw: 1, points_loss: 0}
	campaign[table.Query(2)] = &TeamCampaign{id: 2, points: 0, bias: 0, points_win: 3, points_draw: 1, points_loss: 0}
	campaign[table.Query(3)] = &TeamCampaign{id: 3, points: 0, bias: 0, points_win: 3, points_draw: 1, points_loss: 0}

	games := []*GameType{
		{Id: 1, HomeId: 2, AwayId: 3, HomePower: 0.01, AwayPower: 8.0, Played: false},
	}

	group := &GroupType{
		Id: 888,
		Phase: &PhaseType{
			Championship: &ChampionshipType{Point_win: 3, Point_draw: 1, Point_loss: 0},
			Sort:         "pt,gd,gf,bias",
		},
		Team_groups: teamGroups,
		Games:       games,
	}

	res := group.calculate_odds()
	teamOddsMap := res["team_odds"].(map[int]*TeamOdds)

	team1Odds := teamOddsMap[1]
	sum := team1Odds.Pos[0] + team1Odds.Pos[1] + team1Odds.Pos[2]
	if math.Abs(sum-100.0) > 1e-6 {
		t.Errorf("Expected team position odds to sum to 100%%, got %f", sum)
	}
}

func TestMultiPositionSharing(t *testing.T) {
	teamGroups := []TeamType{
		{Team_id: 1, Bias: 0}, {Team_id: 2, Bias: 1}, {Team_id: 3, Bias: 2}, {Team_id: 4, Bias: 3},
	}
	table := NewTable([]uint32{1, 2, 3, 4})
	campaign := make([]*TeamCampaign, 4)
	campaign[table.Query(1)] = &TeamCampaign{id: 1, points: 0, bias: 0, points_win: 3, points_draw: 1, points_loss: 0}
	campaign[table.Query(2)] = &TeamCampaign{id: 10, bias: 1, points_win: 3, points_draw: 1, points_loss: 0}
	campaign[table.Query(3)] = &TeamCampaign{id: 10, bias: 2, points_win: 3, points_draw: 1, points_loss: 0}
	campaign[table.Query(4)] = &TeamCampaign{id: 10, bias: 3, points_win: 3, points_draw: 1, points_loss: 0}

	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2, HomePower: 0.1, AwayPower: 2.0, Played: false, home_table_index: table.Query(1), away_table_index: table.Query(2)},
		{Id: 2, HomeId: 1, AwayId: 3, HomePower: 0.1, AwayPower: 2.0, Played: false, home_table_index: table.Query(1), away_table_index: table.Query(3)},
		{Id: 3, HomeId: 1, AwayId: 4, HomePower: 0.1, AwayPower: 2.0, Played: false, home_table_index: table.Query(1), away_table_index: table.Query(4)},
	}
	sortOrder := []SortType{PT, GD, GF, BIAS}

	originalMeans := make([]GameProposalMeans, len(games))
	for i, g := range games {
		originalMeans[i] = GameProposalMeans{Home: g.HomePower, Away: g.AwayPower}
	}

	job := &RareSimulationJob{
		TeamID:             1,
		Direction:          RareBetter,
		CandidatePositions: []int{0, 1},
		Components:         buildProposalComponents(games, 1, RareBetter, []int{0, 1}, map[int]float64{1: 3.0, 2: 0.0, 3: 1.0, 4: 2.0}),
		Iterations:         2000,
	}

	rng := rand.New(rand.NewSource(999))
	results, _ := estimateRarePositionsForJob(
		campaign, games, originalMeans, table, sortOrder, teamGroups,
		job, rng, 500,
	)

	if len(results) != 2 {
		t.Fatalf("expected job to estimate 2 candidate positions, got %d", len(results))
	}
	if _, ok := results[0]; !ok {
		t.Errorf("expected candidate position 0 in results")
	}
	if _, ok := results[1]; !ok {
		t.Errorf("expected candidate position 1 in results")
	}
}

func TestPilotSelectionScoring(t *testing.T) {
	s1 := scorePilotProposal(0.20, 0.01, 2)
	s2 := scorePilotProposal(0.20, 0.30, 4)

	if s1 <= s2 {
		t.Errorf("expected scorePilotProposal for moderate candidate rate with low overshoot (%f) to exceed overshooting score (%f)", s1, s2)
	}
}

func TestWeakestUsefulPilot(t *testing.T) {
	for _, tc := range []struct {
		name  string
		rates []float64
		want  int
	}{
		{"first useful", []float64{0, 0, 0.01, 0.05, 0.10}, 2},
		{"more hits do not win", []float64{0, 0, 0.015, 0.04, 0.12}, 2},
		{"jump over band", []float64{0, 0.002, 0.06, 0.10, 0.15}, 2},
		{"difficult fallback", []float64{0, 0, 0, 0.001, 0.003}, 4},
		{"no hits", []float64{0, 0, 0, 0, 0}, -1},
	} {
		t.Run(tc.name, func(t *testing.T) {
			results := make([]PilotProposalResult, len(tc.rates))
			for i, rate := range tc.rates {
				results[i] = PilotProposalResult{
					Config:        ProposalConfig{StrengthLevel: i + 1},
					Samples:       1000,
					CandidateHits: int(rate * 1000),
					CandidateRate: rate,
				}
			}
			if got := selectWeakestUsefulPilot(results); got != tc.want {
				t.Fatalf("selected index %d, want %d", got, tc.want)
			}
		})
	}
}

func TestEdgeProductionMixturesUseDistinctLevels(t *testing.T) {
	games := []*GameType{{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1, AwayPower: 1}}
	original := []GameProposalMeans{{Home: 1, Away: 1}}
	ranks := map[int]float64{1: 1, 2: 0}
	for _, tc := range []struct {
		selected int
		levels   []int
		weights  []float64
	}{
		{1, []int{1, 2, 3}, []float64{0.50, 0.25, 0.20}},
		{3, []int{2, 3, 4}, []float64{0.20, 0.50, 0.25}},
		{5, []int{3, 4, 5}, []float64{0.20, 0.25, 0.50}},
	} {
		proposal := buildSearchProposalForLevel(games, 1, RareBetter, 0, tc.selected, ranks, original)
		if len(proposal.Components) != 4 || proposal.Components[0].Weight != 0.05 {
			t.Fatalf("level %d: expected original plus three proposal components", tc.selected)
		}
		for i, level := range tc.levels {
			want := getStrengthDefinitions(RareBetter)[level].Name
			if proposal.Components[i+1].Weight != tc.weights[i] ||
				len(proposal.Components[i+1].Name) < len(want) ||
				proposal.Components[i+1].Name[:len(want)] != want {
				t.Errorf("selected %d component %d = %s %.2f, want level %d weight %.2f",
					tc.selected, i+1, proposal.Components[i+1].Name, proposal.Components[i+1].Weight, level, tc.weights[i])
			}
		}
	}
}

func TestProductionPlanningWaitsForPilotsAndPrioritizesWeakProposal(t *testing.T) {
	weak := &FrontierCandidate{TeamID: 1, Position: 0, Priority: 40,
		SearchState: &PositionSearchState{Status: StatusPromising, BestProposal: &SearchProposal{StrengthLevel: 3}, BestPilotRate: 0.01}}
	strong := &FrontierCandidate{TeamID: 2, Position: 1, Priority: 40,
		SearchState: &PositionSearchState{Status: StatusFrontier, BestProposal: &SearchProposal{StrengthLevel: 5}, BestPilotRate: 0.08}}
	candidates := []*FrontierCandidate{strong, weak}
	if allocations := planProductionAllocations(candidates, 30000, 1); len(allocations) != 0 {
		t.Fatal("production allocated before every frontier candidate was piloted")
	}
	strong.SearchState.Status = StatusPromising
	if productionPriority(weak) <= productionPriority(strong) {
		t.Fatal("weak useful proposal must outrank strong high-hit proposal")
	}
	allocations := planProductionAllocations(candidates, 30000, 1)
	if len(allocations) != 2 || allocations[0].Candidate != weak || allocations[0].Samples <= allocations[1].Samples {
		t.Fatalf("unexpected allocations: %+v", allocations)
	}
}

func TestProductionPlanningHonorsWorkBudget(t *testing.T) {
	candidates := []*FrontierCandidate{
		{TeamID: 1, SearchState: &PositionSearchState{Status: StatusPromising, BestProposal: &SearchProposal{StrengthLevel: 3}, BestPilotRate: 0.01}},
		{TeamID: 2, SearchState: &PositionSearchState{Status: StatusPromising, BestProposal: &SearchProposal{StrengthLevel: 4}, BestPilotRate: 0.02}},
	}
	const workLimit int64 = 12501
	const workPerSample int64 = 3
	allocations := planProductionAllocations(candidates, workLimit, workPerSample)
	remaining := workLimit
	for _, allocation := range allocations {
		actual := affordableSamples(allocation.Samples, remaining, workPerSample)
		remaining -= int64(actual) * workPerSample
	}
	if remaining < 0 || workLimit-remaining > workLimit {
		t.Fatalf("budget exceeded: remaining=%d limit=%d", remaining, workLimit)
	}
	if got := affordableSamples(2000, 7, workPerSample); got != 2 {
		t.Fatalf("final action should truncate to 2 affordable samples, got %d", got)
	}
}

func TestValidateProposalMixture(t *testing.T) {
	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1.0, AwayPower: 1.0},
	}
	means := []GameProposalMeans{{Home: 1.0, Away: 1.0}}

	validComps := []ProposalComponent{
		{Name: "original", Weight: 0.05, Means: means},
		{Name: "mild", Weight: 0.95, Means: means},
	}
	validateProposalMixture(validComps, len(games))
}

func TestSyntheticUnderdogRareEvent(t *testing.T) {
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")

	hMean := 0.02
	aMean := 5.0

	group := &GroupType{
		Id: 999,
		Phase: &PhaseType{
			Championship: &ChampionshipType{Point_win: 3, Point_draw: 1, Point_loss: 0},
			Sort:         "pt,gd,gf,bias",
		},
		Team_groups: []TeamType{
			{Team_id: 1, Add_sub: 0, Bias: 0},
			{Team_id: 2, Add_sub: 0, Bias: 1},
		},
		Games: []*GameType{
			{
				Id:        1,
				HomeId:    1,
				AwayId:    2,
				HomePower: hMean,
				AwayPower: aMean,
				Played:    false,
			},
		},
	}

	res := group.calculate_odds()
	teamOddsMap := res["team_odds"].(map[int]*TeamOdds)

	team1Odds := teamOddsMap[1]
	p1stPercent := team1Odds.Pos[0]

	if p1stPercent < 0 {
		t.Errorf("Expected non-negative 1st place odds, got %f", p1stPercent)
	}

	sum := team1Odds.Pos[0] + team1Odds.Pos[1]
	if math.Abs(sum-100.0) > 1e-6 {
		t.Errorf("Expected team position odds to sum to 100%%, got %f", sum)
	}
}

func TestFalse100PercentCorrection(t *testing.T) {
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")

	hMean := 0.02
	aMean := 5.0

	group := &GroupType{
		Id: 997,
		Phase: &PhaseType{
			Championship: &ChampionshipType{Point_win: 3, Point_draw: 1, Point_loss: 0},
			Sort:         "pt,gd,gf,bias",
		},
		Team_groups: []TeamType{
			{Team_id: 1, Add_sub: 0, Bias: 0},
			{Team_id: 2, Add_sub: 0, Bias: 1},
		},
		Games: []*GameType{
			{
				Id:        1,
				HomeId:    1,
				AwayId:    2,
				HomePower: hMean,
				AwayPower: aMean,
				Played:    false,
			},
		},
	}

	res := group.calculate_odds()
	teamOddsMap := res["team_odds"].(map[int]*TeamOdds)

	team1Odds := teamOddsMap[1]
	team2Odds := teamOddsMap[2]

	sum1 := team1Odds.Pos[0] + team1Odds.Pos[1]
	sum2 := team2Odds.Pos[0] + team2Odds.Pos[1]
	if math.Abs(sum1-100.0) > 1e-6 {
		t.Errorf("Expected team 1 position odds to sum to 100%%, got %f", sum1)
	}
	if math.Abs(sum2-100.0) > 1e-6 {
		t.Errorf("Expected team 2 position odds to sum to 100%%, got %f", sum2)
	}
}
