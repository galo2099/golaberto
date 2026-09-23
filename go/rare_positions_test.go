package main

import (
	"encoding/json"
	"math"
	"math/rand"
	"sort"
	"strings"
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

func TestRareEstimateUsability(t *testing.T) {
	// Statistically sound estimate with probability below MinInterestingProbability (1e-5)
	tinyEst := RarePositionEstimate{
		Probability: 2e-7,
		StdErr:      1e-8,
		Hits:        100,
		ESS:         50.0,
	}

	if !rareEstimateUsable(tinyEst) {
		t.Errorf("expected rareEstimateUsable to return true for statistically sound tiny probability, got false")
	}

	// Unsound estimate (0 hits)
	zeroEst := RarePositionEstimate{
		Probability: 0.0,
		StdErr:      0.0,
		Hits:        0,
		ESS:         0.0,
	}
	if rareEstimateUsable(zeroEst) {
		t.Errorf("expected rareEstimateUsable to return false for 0 hits, got true")
	}
}

func TestPlainProductionUsesOnlyFreshCountsAndRetainsLowQualityResults(t *testing.T) {
	// Scout observations are deliberately not an input to this helper; only the
	// predeclared fresh production batch determines this estimate.
	scoutCounts := []int{20, 0}
	_ = scoutCounts
	production := summarizePlainProductionCounts(map[int][]int{1: {3, 7}}, 10, 100)
	estimate := production[1][0]
	if !estimate.Available || estimate.Samples != 10 || estimate.Hits != 3 ||
		math.Abs(estimate.Probability-0.3) > 1e-12 || estimate.WorkSpent != 100 {
		t.Fatalf("plain production included scout counts or lost raw data: %+v", estimate)
	}
	zero := summarizePlainProductionCounts(map[int][]int{1: {0}}, 100, 1000)[1][0]
	if !zero.Available || zero.Probability != 0 || zero.Hits != 0 ||
		zero.MeetsPrecisionGoal || zero.ZeroHitUpper95 <= 0 {
		t.Fatalf("zero-hit production should remain an available raw estimate: %+v", zero)
	}
	if _, err := json.Marshal(zero); err != nil {
		t.Fatalf("zero-hit estimate should remain valid JSON with undefined relative SE: %v", err)
	}
	belowInterest := summarizePlainProductionCounts(map[int][]int{1: {1, 199999}}, 200000, 2000000)[1][0]
	if !belowInterest.Available || belowInterest.Probability != 5e-6 {
		t.Fatalf("below-interest production estimate was post-selected away: %+v", belowInterest)
	}
}

func TestProductionDesignFreezesProposalAndSampleCount(t *testing.T) {
	original := []GameProposalMeans{{Home: 1, Away: 1}}
	proposal := CEMProposal{Iteration: 4, TeamLogMultipliers: map[int]float64{1: 0.2},
		Means: []GameProposalMeans{{Home: 3, Away: 2}}}
	evaluation := &CEMProposalEvaluation{Snapshot: CEMProposalSnapshot{
		CandidateTeam: 1, CandidatePosition: 2, SourceIteration: 4, Proposal: proposal,
	}}
	design := freezeProductionDesign(evaluation, original, 1000, 10, 20)
	if design.Kind != "importance_sampling" || design.Samples != 50 || design.Work != 1000 ||
		design.Components[0].Weight != OriginalMixtureWeight || design.Components[1].Weight != 1-OriginalMixtureWeight {
		t.Fatalf("production design was not fully fixed from evaluation: %+v", design)
	}
	evaluation.Snapshot.Proposal.Means[0].Home = 99
	original[0].Home = 88
	if design.Components[0].Means[0].Home != 1 || design.Components[1].Means[0].Home != 3 || design.Samples != 50 {
		t.Fatalf("frozen production design changed after source mutation: %+v", design)
	}
	plain := freezeProductionDesign(nil, []GameProposalMeans{{Home: 1, Away: 2}}, 99, 10, 20)
	if plain.Kind != "plain_mc" || plain.Samples != 9 || plain.Components[0].Weight != 1 {
		t.Fatalf("plain P fallback was not frozen as a fresh fixed-N design: %+v", plain)
	}
}

func TestRareSearchKeepsScoutRowAndDoesNotPoolFreshPlainProduction(t *testing.T) {
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")
	groups := []TeamType{{Team_id: 1, Bias: 0}, {Team_id: 2, Bias: 1}}
	table := NewTable([]uint32{1, 2})
	campaign := make([]*TeamCampaign, 2)
	campaign[table.Query(1)] = &TeamCampaign{id: 1, points_win: 3, points_draw: 1, bias: 0}
	campaign[table.Query(2)] = &TeamCampaign{id: 2, points_win: 3, points_draw: 1, bias: 1}
	games := []*GameType{{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1, AwayPower: 1,
		home_table_index: table.Query(1), away_table_index: table.Query(2)}}
	group := &GroupType{Id: 16982, Team_groups: groups, Games: games}
	counts := map[int][]int{1: {4000, 16000}, 2: {16000, 4000}}
	teamOdds := []OddsType{{team: &TeamOdds{Pos: []float64{0.2, 0.8}}},
		{team: &TeamOdds{Pos: []float64{0.8, 0.2}}}}
	before := [][]float64{append([]float64(nil), teamOdds[0].team.Pos...), append([]float64(nil), teamOdds[1].team.Pos...)}
	results := searchAndMergeRarePositions(group, campaign, table, []SortType{PT, GD, GF, BIAS},
		counts, teamOdds, ScoutIterations)
	for teamIndex := range teamOdds {
		for pos := range teamOdds[teamIndex].team.Pos {
			if teamOdds[teamIndex].team.Pos[pos] != before[teamIndex][pos] {
				t.Fatalf("raw scout odds were mutated/renormalized: before=%v after=%v", before, teamOdds)
			}
		}
	}
	for _, teamEstimates := range results {
		for _, estimate := range teamEstimates {
			if !estimate.Available || estimate.Design != "plain_mc" || estimate.Samples != 80000 {
				t.Fatalf("plain fallback should be a fresh 80k-sample production estimate, got %+v", estimate)
			}
		}
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

func TestZeroHitImportanceProductionEstimateRemainsAvailableAtFixedN(t *testing.T) {
	groups := []TeamType{{Team_id: 1, Bias: 0}, {Team_id: 2, Bias: 1}}
	table := NewTable([]uint32{1, 2})
	campaign := []*TeamCampaign{
		{id: 1, points_win: 3, points_draw: 1, bias: 0},
		{id: 2, points_win: 3, points_draw: 1, bias: 1},
	}
	games := []*GameType{{Id: 1, HomeId: 1, AwayId: 2, HomePower: 0, AwayPower: 0,
		home_table_index: table.Query(1), away_table_index: table.Query(2)}}
	means := []GameProposalMeans{{Home: 0, Away: 0}}
	job := &RareSimulationJob{TeamID: 1, CandidatePositions: []int{2}, Iterations: 17,
		Components: []ProposalComponent{{Name: "P", Weight: 1, Means: means}}}
	results, _ := estimateRarePositionsForJob(campaign, games, means, table,
		[]SortType{PT, GD, GF, BIAS}, groups, job, rand.New(rand.NewSource(91)), 91)
	estimate := results[2]
	if !estimate.Available || estimate.Samples != job.Iterations || estimate.Hits != 0 ||
		estimate.Probability != 0 || estimate.MeetsPrecisionGoal || estimate.ZeroHitUpper95 <= 0 {
		t.Fatalf("fixed-N zero-hit production was discarded or replaced: %+v", estimate)
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

	compMeans := []GameProposalMeans{{Home: proposalMean(hMean, 0.4), Away: proposalMean(aMean, 2.5)}}
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

func TestWeightedPilotSelectionUsesESSPerWork(t *testing.T) {
	a := &WeightedPilotResult{Proposal: SearchProposal{StrengthLevel: 3}, Refined: true,
		Hits: 30, ESS: 1.2, WorkSpent: 1000, ESSPerWork: 0.0012}
	b := &WeightedPilotResult{Proposal: SearchProposal{StrengthLevel: 4}, Refined: true,
		Hits: 8, ESS: 4, WorkSpent: 1000, ESSPerWork: 0.004}
	if got := selectPilotMixture([]*WeightedPilotResult{a, b}); got != b {
		t.Fatal("higher weighted ESS/work must beat more raw hits")
	}
	oneHit := &WeightedPilotResult{Proposal: SearchProposal{StrengthLevel: 1}, Refined: true,
		Hits: 1, ESS: 1, WorkSpent: 1000, ESSPerWork: 0.001}
	if got := selectPilotMixture([]*WeightedPilotResult{oneHit}); got != nil {
		t.Fatal("one effective event cannot qualify for production")
	}
}

func TestWeightedPilotSelectionTieBreaks(t *testing.T) {
	a := &WeightedPilotResult{Proposal: SearchProposal{StrengthLevel: 4}, Refined: true,
		Hits: 5, ESS: 3, ESSPerWork: 0.003, MaxEventWeightShare: 0.6}
	b := &WeightedPilotResult{Proposal: SearchProposal{StrengthLevel: 3}, Refined: true,
		Hits: 4, ESS: 3, ESSPerWork: 0.003, MaxEventWeightShare: 0.4}
	if selectPilotMixture([]*WeightedPilotResult{a, b}) != b {
		t.Fatal("lower weight dominance must break equal ESS/work")
	}
	a.MaxEventWeightShare = b.MaxEventWeightShare
	if selectPilotMixture([]*WeightedPilotResult{a, b}) != b {
		t.Fatal("weaker level must break equal ESS/work and dominance")
	}
}

func TestProjectedProductionWork(t *testing.T) {
	pilot := &WeightedPilotResult{ESS: 3, WorkSpent: 1000}
	if got := projectedWorkForESS(pilot); got != 6250 {
		t.Fatalf("projected work = %d, want 6250", got)
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
				!strings.Contains(proposal.Components[i+1].Name, want+"_lvl") {
				t.Errorf("selected %d component %d = %s %.2f, want level %d weight %.2f",
					tc.selected, i+1, proposal.Components[i+1].Name, proposal.Components[i+1].Weight, level, tc.weights[i])
			}
		}
	}
}

func TestGlobalProductionPlanning(t *testing.T) {
	pilot := &WeightedPilotResult{ESS: 3, WorkSpent: 1000, ESSPerWork: 0.003}
	a := ProductionPlan{Candidate: &FrontierCandidate{TeamID: 1, Position: 0, Priority: 40},
		Pilot: pilot, ProjectedWork: 5000, ProjectedSamples: 5000}
	b := ProductionPlan{Candidate: &FrontierCandidate{TeamID: 2, Position: 1, Priority: 40},
		Pilot: pilot, ProjectedWork: 20000, ProjectedSamples: 20000}
	allocations := planProductionAllocations([]ProductionPlan{b, a}, 25000, 1)
	if len(allocations) != 2 || allocations[0].Plan.Candidate != a.Candidate {
		t.Fatalf("cheaper target must be first: %+v", allocations)
	}
	b.Critical100 = true
	allocations = planProductionAllocations([]ProductionPlan{a, b}, 25000, 1)
	if len(allocations) != 2 || allocations[0].Plan.Candidate != b.Candidate {
		t.Fatalf("100%% alternative must be first: %+v", allocations)
	}
}

func TestHundredPercentAlternativeBecomesCriticalPlan(t *testing.T) {
	pilot := &WeightedPilotResult{ESS: 3, WorkSpent: 1000, ESSPerWork: 0.003}
	candidate := &FrontierCandidate{TeamID: 7, Position: 1,
		SearchState: &PositionSearchState{Status: StatusPromising, BestPilot: pilot}}
	searches := map[int]*TeamRareSearch{7: {Has100PercentNormal: true}}
	plans := buildProductionPlans([]*FrontierCandidate{candidate}, 1, searches)
	if len(plans) != 1 || !plans[0].Critical100 {
		t.Fatalf("feasible alternative to apparent 100%% rank must be critical: %+v", plans)
	}
}

func TestProductionPlannerSkipsHopelessPartialJob(t *testing.T) {
	pilot := &WeightedPilotResult{ESS: 3, WorkSpent: 1000, ESSPerWork: 0.003}
	plan := ProductionPlan{Candidate: &FrontierCandidate{TeamID: 1}, Pilot: pilot,
		ProjectedWork: 5000, ProjectedSamples: 5000}
	if got := planProductionAllocations([]ProductionPlan{plan}, 1000, 1); len(got) != 0 {
		t.Fatalf("partial job predicting ESS 3 must be skipped: %+v", got)
	}
	got := planProductionAllocations([]ProductionPlan{plan}, 4000, 1)
	if len(got) != 1 || got[0].Samples != 4000 || got[0].PredictedESS < MinUsableESS {
		t.Fatalf("useful fixed partial job should be funded: %+v", got)
	}
}

func TestProductionPlanningHonorsWorkBudget(t *testing.T) {
	pilot := &WeightedPilotResult{ESS: 3, WorkSpent: 1000, ESSPerWork: 0.003}
	plans := []ProductionPlan{
		{Candidate: &FrontierCandidate{TeamID: 1}, Pilot: pilot, ProjectedWork: 9000, ProjectedSamples: 3000},
		{Candidate: &FrontierCandidate{TeamID: 2}, Pilot: pilot, ProjectedWork: 9000, ProjectedSamples: 3000},
	}
	const limit int64 = 12501
	allocations := planProductionAllocations(plans, limit, 3)
	var spent int64
	for _, allocation := range allocations {
		spent += allocation.Work
		if allocation.Work != int64(allocation.Samples)*3 {
			t.Fatal("allocation work and sample count disagree")
		}
	}
	if spent > limit || limit-spent < 0 {
		t.Fatalf("budget exceeded: spent=%d limit=%d", spent, limit)
	}
	if got := affordableSamples(2000, 7, 3); got != 2 {
		t.Fatalf("last action must truncate to 2 samples, got %d", got)
	}
}

func TestFrontierExpandsOnlyAfterResolution(t *testing.T) {
	search := &TeamRareSearch{TeamID: 1, NormalMeanRank: 0,
		Positions: []*PositionSearchState{
			{Position: 0, Status: StatusObserved, Feasible: true},
			{Position: 1, Status: StatusUnexplored, Feasible: true},
			{Position: 2, Status: StatusUnexplored, Feasible: true},
		}}
	first := discoverFrontier(search)
	if len(first) != 1 || first[0].Position != 1 {
		t.Fatalf("expected initial border at position 1, got %+v", first)
	}
	search.Positions[1].Status = StatusPromising
	if next := discoverFrontier(search); len(next) != 0 {
		t.Fatalf("unresolved pilot must not expand frontier: %+v", next)
	}
	search.Positions[1].Status = StatusResolved
	next := discoverFrontier(search)
	if len(next) != 1 || next[0].Position != 2 {
		t.Fatalf("resolved border should expose one adjacent position: %+v", next)
	}
}
func TestExactVeryRareProbabilityIsNotExplicitlyMerged(t *testing.T) {
	// Independent one-game Poisson reference for an extreme underdog win.
	hMean, aMean := 0.001, 6.0
	pWin := 0.0
	for h := 1; h <= 20; h++ {
		pAwayLess := 0.0
		for a := 0; a < h; a++ {
			pAwayLess += poisson_pmf(aMean, float64(a))
		}
		pWin += poisson_pmf(hMean, float64(h)) * pAwayLess
	}
	if pWin <= 0 || pWin >= MinInterestingProbability {
		t.Fatalf("synthetic probability %g should be positive and below interest threshold", pWin)
	}
	merged := mergeRarePositionEstimates(
		[]float64{0, 1}, []int{0, ScoutIterations},
		map[int]RarePositionEstimate{0: {Probability: pWin, Found: true}},
	)
	if merged[0] != 0 || merged[1] != 1 {
		t.Fatalf("below-interest estimate should not become explicit odds: %v", merged)
	}
}

func TestWeightedPilotPrefersEfficientSyntheticRareEventMixture(t *testing.T) {
	hMean, aMean := 0.05, 5.0
	exact := 0.0
	for h := 1; h <= 20; h++ {
		for a := 0; a < h; a++ {
			exact += poisson_pmf(hMean, float64(h)) * poisson_pmf(aMean, float64(a))
		}
	}
	if exact < 1e-4 || exact > 1e-3 {
		t.Fatalf("fixture should be moderately rare, got %g", exact)
	}
	groups := []TeamType{{Team_id: 1, Bias: 0}, {Team_id: 2, Bias: 1}}
	table := NewTable([]uint32{1, 2})
	campaign := []*TeamCampaign{
		{id: 1, points_win: 3, points_draw: 1, bias: 0},
		{id: 2, points_win: 3, points_draw: 1, bias: 1},
	}
	games := []*GameType{{Id: 1, HomeId: 1, AwayId: 2, HomePower: hMean,
		AwayPower: aMean, home_table_index: table.Query(1), away_table_index: table.Query(2)}}
	original := []GameProposalMeans{{Home: hMean, Away: aMean}}
	makeMixture := func(name string, means [3]GameProposalMeans) []ProposalComponent {
		return []ProposalComponent{
			{Name: "original", Weight: OriginalMixtureWeight, Means: original},
			{Name: name + "_1", Weight: 0.20, Means: []GameProposalMeans{means[0]}},
			{Name: name + "_2", Weight: 0.50, Means: []GameProposalMeans{means[1]}},
			{Name: name + "_3", Weight: 0.25, Means: []GameProposalMeans{means[2]}},
		}
	}
	moderate := &WeightedPilotResult{Proposal: SearchProposal{StrengthLevel: 2,
		Components: makeMixture("moderate", [3]GameProposalMeans{{0.2, 3}, {0.5, 2}, {1, 2}})}}
	aggressive := &WeightedPilotResult{Proposal: SearchProposal{StrengthLevel: 5,
		Components: makeMixture("aggressive", [3]GameProposalMeans{{4, 0.5}, {8, 0.1}, {12, 0.01}})}}
	order := []SortType{PT, GD, GF, BIAS}
	workPerSample := estimateSeasonWork(1, 4, 2)
	for i, pilot := range []*WeightedPilotResult{moderate, aggressive} {
		pilot.Refined = true
		evaluateWeightedPilot(pilot, campaign, games, original, table, order, groups,
			1, 0, 800, workPerSample, rand.New(rand.NewSource(int64(100+i))), 1)
	}
	if moderate.Hits < MinPilotHitsForProduction || moderate.ESS < MinPilotESSForProduction {
		t.Fatalf("moderate pilot lacked evidence: hits=%d ESS=%f", moderate.Hits, moderate.ESS)
	}
	if selected := selectPilotMixture([]*WeightedPilotResult{aggressive, moderate}); selected != moderate {
		t.Fatalf("weighted pilot selected aggressive mixture: moderate ESS=%f aggressive ESS=%f",
			moderate.ESS, aggressive.ESS)
	}
	run := func(pilot *WeightedPilotResult, seed int64) RarePositionEstimate {
		job := &RareSimulationJob{TeamID: 1, CandidatePositions: []int{0},
			Components: pilot.Proposal.Components, Iterations: 20000}
		results, _ := estimateRarePositionsForJob(campaign, games, original, table, order,
			groups, job, rand.New(rand.NewSource(seed)), 1)
		return results[0]
	}
	good := run(moderate, 500)
	bad := run(aggressive, 501)
	if good.Samples != 20000 || bad.Samples != 20000 || moderate.Samples != 800 {
		t.Fatal("pilot samples must not enter production estimates")
	}
	if math.Abs(good.Probability-exact) > 4*good.StdErr {
		t.Fatalf("selected mixture p=%g differs from exact p=%g by more than 4 SE=%g",
			good.Probability, exact, good.StdErr)
	}
	if good.ESS <= 2*bad.ESS {
		t.Fatalf("selected mixture should deliver much more event ESS: selected=%g aggressive=%g",
			good.ESS, bad.ESS)
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

func TestProposalMeanPreservesDirection(t *testing.T) {
	for _, tc := range []struct{ original, multiplier float64 }{
		{0.02, 0.93}, {0.02, 1.10}, {9, 1.10}, {9, 0.90}, {0, 2.20},
	} {
		got := proposalMean(tc.original, tc.multiplier)
		if math.IsNaN(got) || math.IsInf(got, 0) || got < 0 {
			t.Fatalf("invalid proposal mean %g from %g * %g", got, tc.original, tc.multiplier)
		}
		if tc.multiplier < 1 && got > tc.original || tc.multiplier > 1 && got < tc.original {
			t.Fatalf("proposal tilt reversed: %g * %g = %g", tc.original, tc.multiplier, got)
		}
		if tc.original == 0 && got != 0 {
			t.Fatal("zero original mean must stay zero")
		}
	}
}

func TestSparseProposalLimitsAndDeterminism(t *testing.T) {
	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1, AwayPower: 1}, // head-to-head
		{Id: 2, HomeId: 1, AwayId: 4, HomePower: 1, AwayPower: 1},
		{Id: 3, HomeId: 1, AwayId: 4, HomePower: 1, AwayPower: 1},
		{Id: 4, HomeId: 1, AwayId: 4, HomePower: 1, AwayPower: 1},
		{Id: 5, HomeId: 1, AwayId: 4, HomePower: 1, AwayPower: 1},
		{Id: 6, HomeId: 1, AwayId: 4, HomePower: 1, AwayPower: 1},
		{Id: 7, HomeId: 2, AwayId: 4, HomePower: 1, AwayPower: 1},
		{Id: 8, HomeId: 2, AwayId: 4, HomePower: 1, AwayPower: 1},
		{Id: 9, HomeId: 2, AwayId: 3, HomePower: 1, AwayPower: 1},
		{Id: 10, HomeId: 3, AwayId: 4, HomePower: 1, AwayPower: 1},
		{Id: 11, HomeId: 4, AwayId: 4, HomePower: 1, AwayPower: 1},
	}
	ranks := map[int]float64{1: 2.8, 2: 0.55, 3: 1.5, 4: 3}
	spec := SparseProposalSpec{TargetTeamID: 1, TargetRank: 0, Direction: RareBetter,
		StrengthLevel: 2, TargetGameLimit: 2, BoundaryCompetitorLimit: 1, CompetitorGameLimit: 1}
	cfg := buildSparseProposalConfig(games, spec, getStrengthDefinitions(RareBetter)[2], ranks)
	if len(cfg.RelevantTeams) != 1 || cfg.RelevantTeams[0] != 2 {
		t.Fatalf("expected closest boundary competitor 2, got %v", cfg.RelevantTeams)
	}
	if !strings.Contains(cfg.Name, "target2_comp1x1") {
		t.Fatalf("sparsity missing from name: %s", cfg.Name)
	}
	targetModified, blockerModified := 0, 0
	for i, g := range games {
		means := cfg.Means[i]
		if g.HomeId == 1 && means.Home != g.HomePower || g.AwayId == 1 && means.Away != g.AwayPower {
			targetModified++
			if g.HomeId == 1 && means.Home < g.HomePower || g.AwayId == 1 && means.Away < g.AwayPower {
				t.Fatal("RareBetter target direction reversed")
			}
		}
		if g.HomeId == 2 && means.Home != g.HomePower || g.AwayId == 2 && means.Away != g.AwayPower {
			blockerModified++
		}
		if g.HomeId != 1 && g.AwayId != 1 && g.HomeId != 2 && g.AwayId != 2 && means != (GameProposalMeans{g.HomePower, g.AwayPower}) {
			t.Fatal("unrelated game was changed")
		}
	}
	if targetModified != 2 || blockerModified != 1 || cfg.Means[0].Home == games[0].HomePower ||
		cfg.Means[0].Away == games[0].AwayPower || cfg.Means[8] != (GameProposalMeans{1, 1}) {
		t.Fatalf("sparse limits or head-to-head priority violated: target=%d blocker=%d means=%v", targetModified, blockerModified, cfg.Means)
	}
	second := buildSparseProposalConfig(games, spec, getStrengthDefinitions(RareBetter)[2], ranks)
	for i := range cfg.Means {
		if cfg.Means[i] != second.Means[i] {
			t.Fatalf("non-deterministic game selection at index %d", i)
		}
	}
	all := spec
	all.TargetGameLimit = -1
	all.BoundaryCompetitorLimit = 0
	all.CompetitorGameLimit = 0
	allCfg := buildSparseProposalConfig(games, all, getStrengthDefinitions(RareBetter)[2], ranks)
	allModified := 0
	for i, game := range games {
		if game.HomeId == 1 && allCfg.Means[i].Home != game.HomePower {
			allModified++
		}
	}
	if allModified != 6 || !strings.Contains(allCfg.Name, "targetall") {
		t.Fatalf("explicit all-target scope changed %d games: %s", allModified, allCfg.Name)
	}
}

func TestSparseProposalLeavesSelectedBlockersHeadToHeadUntouched(t *testing.T) {
	games := []*GameType{
		{Id: 1, HomeId: 2, AwayId: 3, HomePower: 1, AwayPower: 1},
		{Id: 2, HomeId: 1, AwayId: 2, HomePower: 1, AwayPower: 1},
		{Id: 3, HomeId: 1, AwayId: 3, HomePower: 1, AwayPower: 1},
		{Id: 4, HomeId: 2, AwayId: 4, HomePower: 1, AwayPower: 1},
	}
	spec := SparseProposalSpec{TargetTeamID: 1, TargetRank: 2, Direction: RareWorse,
		StrengthLevel: 2, TargetGameLimit: 1, BoundaryCompetitorLimit: 2, CompetitorGameLimit: 1}
	ranks := map[int]float64{1: 0, 2: 1.4, 3: 1.6, 4: 3}
	cfg := buildSparseProposalConfig(games, spec, getStrengthDefinitions(RareWorse)[2], ranks)
	if len(cfg.RelevantTeams) != 2 || cfg.Means[0] != (GameProposalMeans{1, 1}) {
		t.Fatalf("blocker-vs-blocker game changed: competitors=%v means=%v", cfg.RelevantTeams, cfg.Means[0])
	}
	for i, game := range games {
		if game.HomeId == 1 && cfg.Means[i].Home > game.HomePower ||
			game.AwayId == 1 && cfg.Means[i].Away > game.AwayPower {
			t.Fatal("RareWorse target direction reversed")
		}
		if game.HomeId == 2 && cfg.Means[i].Home < game.HomePower ||
			game.AwayId == 2 && cfg.Means[i].Away < game.AwayPower {
			t.Fatal("RareWorse blocker direction reversed")
		}
	}
}

func TestFallbackBudgetAndCountCombination(t *testing.T) {
	samples, work := planFallbackSamples(703, 7)
	if samples != 100 || work != 700 || 703-work < 0 {
		t.Fatalf("fallback budget mismatch: %d samples, %d work", samples, work)
	}
	table := NewTable([]uint32{1, 2, 3})
	initial := map[int][]int{1: {0, 0, ScoutIterations}}
	fallback := map[int][]int{1: {0, 1, 4999}}
	odds := make([]OddsType, 3)
	for i := range odds {
		odds[i].team = &TeamOdds{Pos: make([]float64, 3)}
	}
	newCells, broken := combineNormalPositionCounts(initial, fallback, ScoutIterations, 5000, odds, table)
	if newCells != 1 || broken != 1 || initial[1][1] != 1 || initial[1][2] != ScoutIterations+4999 {
		t.Fatalf("fallback counts not combined: %v new=%d broken=%d", initial[1], newCells, broken)
	}
	normal := odds[table.Query(1)].team.Pos
	if normal[2] >= 1 || math.Abs(normal[1]-1.0/float64(ScoutIterations+5000)) > 1e-12 {
		t.Fatalf("combined normal odds wrong: %v", normal)
	}
	merged := mergeRarePositionEstimates(normal, initial[1], map[int]RarePositionEstimate{
		1: {Probability: 0.1, Found: true},
	})
	if merged[1] != normal[1] || merged[2] != normal[2] {
		t.Fatalf("direct ordinary observation must override IS: %v vs %v", merged, normal)
	}
}

func TestPlainFallbackSamplesAllTeamsWithoutGameImportance(t *testing.T) {
	groups := []TeamType{{Team_id: 1}, {Team_id: 2}, {Team_id: 3}}
	table := NewTable([]uint32{1, 2, 3})
	base := make([]*TeamCampaign, 3)
	for _, team := range groups {
		base[table.Query(uint32(team.Team_id))] = &TeamCampaign{id: team.Team_id,
			points_win: 3, points_draw: 1}
	}
	game := &GameType{Id: 1, HomeId: 1, AwayId: 2, HomePower: 0.5, AwayPower: 1,
		home_table_index: table.Query(1), away_table_index: table.Query(2)}
	counts := simulatePlainRankCounts(base, []*GameType{game}, table,
		[]SortType{PT, GD, GF, BIAS}, groups, 100, rand.New(rand.NewSource(92)))
	for _, team := range groups {
		total := 0
		for _, count := range counts[team.Team_id] {
			total += count
		}
		if total != 100 {
			t.Fatalf("team %d has %d rank observations, want 100", team.Team_id, total)
		}
	}
}

func TestFixedPathMultiGameMixtureLikelihood(t *testing.T) {
	scores := [][2]int{{1, 0}, {0, 2}}
	original := []GameProposalMeans{{0.2, 1}, {0.5, 0.2}}
	components := []ProposalComponent{
		{Name: "P", Weight: 0.05, Means: original},
		{Name: "mild", Weight: 0.45, Means: []GameProposalMeans{{0.4, 0.8}, {0.6, 0.3}}},
		{Name: "strong", Weight: 0.50, Means: []GameProposalMeans{{1.5, 0.2}, {0.1, 1.2}}},
	}
	logs := make([]float64, len(components))
	weights := make([]float64, len(components))
	pPath, qMix := 1.0, 0.0
	for i, c := range components {
		qPath := 1.0
		for g, score := range scores {
			if i == 0 {
				pPath *= poisson_pmf(original[g].Home, float64(score[0])) * poisson_pmf(original[g].Away, float64(score[1]))
			}
			qPath *= poisson_pmf(c.Means[g].Home, float64(score[0])) * poisson_pmf(c.Means[g].Away, float64(score[1]))
			logs[i] += logPoissonQOverP(score[0], original[g].Home, c.Means[g].Home)
			logs[i] += logPoissonQOverP(score[1], original[g].Away, c.Means[g].Away)
		}
		qMix += c.Weight * qPath
		weights[i] = c.Weight
	}
	got := mixtureImportanceWeightMulti(logs, weights)
	if math.Abs(got-pPath/qMix) > 1e-12 {
		t.Fatalf("multi-game P/Qmix=%g, got %g", pPath/qMix, got)
	}
}

func TestMultiGameExactPoissonProbabilityAndSparseESS(t *testing.T) {
	groups := []TeamType{{Team_id: 1, Bias: 0}, {Team_id: 2, Bias: 1}, {Team_id: 3, Bias: 2}}
	table := NewTable([]uint32{1, 2, 3})
	base := make([]*TeamCampaign, 3)
	for _, team := range groups {
		base[table.Query(uint32(team.Team_id))] = &TeamCampaign{id: team.Team_id, bias: team.Bias, points_win: 3, points_draw: 1}
	}
	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2, HomePower: 0.2, AwayPower: 1,
			home_table_index: table.Query(1), away_table_index: table.Query(2)},
		{Id: 2, HomeId: 2, AwayId: 3, HomePower: 0.5, AwayPower: 0.2,
			home_table_index: table.Query(2), away_table_index: table.Query(3)},
	}
	original := []GameProposalMeans{{0.2, 1}, {0.5, 0.2}}
	order := []SortType{PT, GD, GF, BIAS}
	exact, includedMass := 0.0, 0.0
	for h1 := 0; h1 <= 10; h1++ {
		for a1 := 0; a1 <= 10; a1++ {
			for h2 := 0; h2 <= 10; h2++ {
				for a2 := 0; a2 <= 10; a2++ {
					mass := poisson_pmf(0.2, float64(h1)) * poisson_pmf(1, float64(a1)) *
						poisson_pmf(0.5, float64(h2)) * poisson_pmf(0.2, float64(a2))
					includedMass += mass
					teams := []*TeamCampaign{base[table.Query(1)].clone(), base[table.Query(2)].clone(), base[table.Query(3)].clone()}
					teams[0].add_game(&GameType{HomeId: 1, AwayId: 2, HomeScore: h1, AwayScore: a1})
					teams[1].add_game(&GameType{HomeId: 1, AwayId: 2, HomeScore: h1, AwayScore: a1})
					teams[1].add_game(&GameType{HomeId: 2, AwayId: 3, HomeScore: h2, AwayScore: a2})
					teams[2].add_game(&GameType{HomeId: 2, AwayId: 3, HomeScore: h2, AwayScore: a2})
					sort.Sort(TeamCampaignSorted{teams, order})
					if teams[0].id == 1 {
						exact += mass
					}
				}
			}
		}
	}
	if 1-includedMass > 1e-7 {
		t.Fatalf("exact enumeration omitted too much mass: %g", 1-includedMass)
	}
	ranks := map[int]float64{1: 2, 2: 0, 3: 1}
	sparse := buildSearchProposalForSpec(games, SparseProposalSpec{TargetTeamID: 1, TargetRank: 0,
		Direction: RareBetter, StrengthLevel: 3, TargetGameLimit: 1,
		BoundaryCompetitorLimit: 1, CompetitorGameLimit: 1}, ranks, original)
	broad := []ProposalComponent{
		{Name: "P", Weight: 0.05, Means: original},
		{Name: "broad_legacy_1", Weight: 0.20, Means: []GameProposalMeans{{2, 0.05}, {0.05, 2}}},
		{Name: "broad_legacy_2", Weight: 0.50, Means: []GameProposalMeans{{4, 0.02}, {0.02, 4}}},
		{Name: "broad_legacy_3", Weight: 0.25, Means: []GameProposalMeans{{8, 0.01}, {0.01, 8}}},
	}
	var ess [2]float64
	for i, components := range [][]ProposalComponent{sparse.Components, broad} {
		job := &RareSimulationJob{TeamID: 1, CandidatePositions: []int{0}, Components: components, Iterations: 50000}
		results, _ := estimateRarePositionsForJob(base, games, original, table, order, groups,
			job, rand.New(rand.NewSource(int64(55+i))), 1)
		est := results[0]
		if math.Abs(est.Probability-exact) > 4*est.StdErr {
			t.Fatalf("proposal %d: p=%g exact=%g SE=%g", i, est.Probability, exact, est.StdErr)
		}
		ess[i] = est.ESS
	}
	if ess[0] <= ess[1] {
		t.Fatalf("sparse overlap should beat broad_legacy overlap: sparse ESS=%g broad ESS=%g", ess[0], ess[1])
	}
}
