package main

import (
	"math"
	"math/rand"
	"testing"
)

func TestPoissonRand(t *testing.T) {
	rng := rand.New(rand.NewSource(42))
	mean := 3.5
	samples := 100000
	sum := 0
	for i := 0; i < samples; i++ {
		sum += poissonRand(rng, mean)
	}
	sampleMean := float64(sum) / float64(samples)
	if math.Abs(sampleMean-mean) > 0.05 {
		t.Errorf("expected mean near %f, got %f", mean, sampleMean)
	}
}

func TestSameProposalAndOriginalMeans(t *testing.T) {
	mu := 2.5
	lambda := 2.5
	for score := 0; score <= 10; score++ {
		logQOverP := logPoissonQOverP(score, lambda, mu)
		if math.Abs(logQOverP) > 1e-12 {
			t.Errorf("for score %d, expected log(Q/P) == 0, got %f", score, logQOverP)
		}
		w := mixtureImportanceWeight(logQOverP, 0.05)
		if math.Abs(w-1.0) > 1e-12 {
			t.Errorf("for score %d, expected mixture weight 1.0, got %f", score, w)
		}
	}
}

func TestPoissonRatio(t *testing.T) {
	mu := 4.0
	lambda := 1.5
	for score := 0; score <= 5; score++ {
		logQOverP := logPoissonQOverP(score, lambda, mu)
		ratio := math.Exp(logQOverP)

		pMu := poisson_pmf(mu, float64(score))
		pLambda := poisson_pmf(lambda, float64(score))
		expectedRatio := pMu / pLambda

		if math.Abs(ratio-expectedRatio) > 1e-9 {
			t.Errorf("for score %d: got ratio %f, expected %f", score, ratio, expectedRatio)
		}
	}
}

func TestMixtureWeightBound(t *testing.T) {
	beta := 0.05
	maxAllowedWeight := 1.0 / beta // 20.0

	// Test extreme logQOverP values (-1000 to +1000)
	for logQOverP := -1000.0; logQOverP <= 1000.0; logQOverP += 10.0 {
		w := mixtureImportanceWeight(logQOverP, beta)
		if w > maxAllowedWeight+1e-10 {
			t.Errorf("for logQOverP=%f, weight %f exceeded maximum allowed %f", logQOverP, w, maxAllowedWeight)
		}
		if w < 0 {
			t.Errorf("for logQOverP=%f, got negative weight %f", logQOverP, w)
		}
	}
}

func TestLogAddExp(t *testing.T) {
	a := 2.0
	b := 3.0
	got := logAddExp(a, b)
	expected := math.Log(math.Exp(a) + math.Exp(b))
	if math.Abs(got-expected) > 1e-12 {
		t.Errorf("expected %f, got %f", expected, got)
	}

	// Underflow / infinity handling
	if logAddExp(math.Inf(-1), 5.0) != 5.0 {
		t.Errorf("expected 5.0 for logAddExp(-Inf, 5.0)")
	}
	if logAddExp(5.0, math.Inf(-1)) != 5.0 {
		t.Errorf("expected 5.0 for logAddExp(5.0, -Inf)")
	}
}

func TestConservativePositionBounds(t *testing.T) {
	// 4 teams: Team 1 (30 pts), Team 2 (20 pts), Team 3 (10 pts), Team 4 (0 pts)
	// 1 game left for Team 3 vs Team 4.
	// Team 1: 30 pts (0 unplayed) -> min 30, max 30
	// Team 2: 20 pts (0 unplayed) -> min 20, max 20
	// Team 3: 10 pts (1 unplayed) -> win=3, draw=1, loss=0 -> min 10, max 13
	// Team 4: 0 pts (1 unplayed)  -> min 0, max 3

	teamGroups := []TeamType{
		{Team_id: 1}, {Team_id: 2}, {Team_id: 3}, {Team_id: 4},
	}
	table := NewTable([]uint32{1, 2, 3, 4})
	campaign := make([]*TeamCampaign, 4)
	campaign[table.Query(1)] = &TeamCampaign{id: 1, points: 30, points_win: 3, points_draw: 1, points_loss: 0}
	campaign[table.Query(2)] = &TeamCampaign{id: 2, points: 20, points_win: 3, points_draw: 1, points_loss: 0}
	campaign[table.Query(3)] = &TeamCampaign{id: 3, points: 10, points_win: 3, points_draw: 1, points_loss: 0}
	campaign[table.Query(4)] = &TeamCampaign{id: 4, points: 0, points_win: 3, points_draw: 1, points_loss: 0}

	games := []*GameType{
		{Id: 1, HomeId: 3, AwayId: 4, Played: false},
	}

	sortOrder := []SortType{PT, GD, GF}

	// For Team 3 (max 13): Team 1 (min 30 > 13) and Team 2 (min 20 > 13) are strictly better (2 teams).
	// Team 4 (max 3 < min 10) is strictly worse (1 team).
	// Best rank for Team 3: index 2 (3rd place). Worst rank for Team 3: 4 - 1 - 1 = index 2 (3rd place).
	if !possiblePositionByPointsBounds(3, 2, campaign, teamGroups, games, table, sortOrder) {
		t.Errorf("Team 3 should be able to reach index 2 (3rd place)")
	}
	if possiblePositionByPointsBounds(3, 0, campaign, teamGroups, games, table, sortOrder) {
		t.Errorf("Team 3 should NOT be able to reach index 0 (1st place)")
	}
	if possiblePositionByPointsBounds(3, 3, campaign, teamGroups, games, table, sortOrder) {
		t.Errorf("Team 3 should NOT be able to reach index 3 (4th place)")
	}

	// All games played case
	gamesFinished := []*GameType{
		{Id: 1, HomeId: 3, AwayId: 4, HomeScore: 1, AwayScore: 0, Played: true},
	}
	campaignFinished := make([]*TeamCampaign, 4)
	campaignFinished[table.Query(1)] = &TeamCampaign{id: 1, points: 30}
	campaignFinished[table.Query(2)] = &TeamCampaign{id: 2, points: 20}
	campaignFinished[table.Query(3)] = &TeamCampaign{id: 3, points: 13}
	campaignFinished[table.Query(4)] = &TeamCampaign{id: 4, points: 0}

	// Team 3 is 3rd place (index 2)
	if !possiblePositionByPointsBounds(3, 2, campaignFinished, teamGroups, gamesFinished, table, sortOrder) {
		t.Errorf("Finished season: Team 3 should be index 2")
	}
	if possiblePositionByPointsBounds(3, 1, campaignFinished, teamGroups, gamesFinished, table, sortOrder) {
		t.Errorf("Finished season: Team 3 should NOT be index 1")
	}
}

func TestMergeRarePositionEstimates(t *testing.T) {
	normalProbs := []float64{1.0, 0.0}
	normalCounts := []int{10000, 0}

	rareEstimates := map[int]RarePositionEstimate{
		1: {Probability: 0.00004, Found: true},
	}

	final := mergeRarePositionEstimates(normalProbs, normalCounts, rareEstimates)

	if math.Abs(final[1]-0.00004) > 1e-9 {
		t.Errorf("expected position 1 rare prob 0.00004, got %f", final[1])
	}
	if math.Abs(final[0]-0.99996) > 1e-9 {
		t.Errorf("expected position 0 adjusted prob 0.99996, got %f", final[0])
	}

	sum := final[0] + final[1]
	if math.Abs(sum-1.0) > 1e-9 {
		t.Errorf("expected sum == 1.0, got %f", sum)
	}
}

func TestDegeneratePoissonLikelihood(t *testing.T) {
	// Original mean = 0, proposal mean = 1.0
	// score == 0 -> Q/P = exp(-1.0) -> log(Q/P) = -1.0
	logQOverPZero := logPoissonQOverP(0, 0.0, 1.0)
	if math.Abs(logQOverPZero - (-1.0)) > 1e-9 {
		t.Errorf("expected logQOverP for score=0 to be -1.0, got %f", logQOverPZero)
	}

	// score > 0 -> Q/P = +Inf -> log(Q/P) = +Inf -> weight = 0.0
	logQOverPPos := logPoissonQOverP(1, 0.0, 1.0)
	if !math.IsInf(logQOverPPos, 1) {
		t.Errorf("expected logQOverP for score>0 with originalMean=0 to be +Inf, got %f", logQOverPPos)
	}
	w := mixtureImportanceWeight(logQOverPPos, 0.05)
	if w != 0.0 {
		t.Errorf("expected mixture weight for +Inf logQOverP to be 0.0, got %f", w)
	}
}

func TestDirectImportanceSamplingEstimator(t *testing.T) {
	// Deterministic validation of estimateRarePositionsForJob against exact analytical Poisson win probability.
	// Home ~ Poisson(0.05), Away ~ Poisson(5.0)
	// P(HomeScore > AwayScore)
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

func TestProposalTargetRanks(t *testing.T) {
	// Test 3+ candidates for RareBetter
	candidates := []int{7, 8, 9, 10, 11, 12, 13, 14, 15}
	mild, medium, strong := proposalTargetRanks(candidates, 16.0, RareBetter)
	if mild != 15 || medium != 11 || strong != 7 {
		t.Errorf("expected mild=15, medium=11, strong=7 for RareBetter, got mild=%d, medium=%d, strong=%d", mild, medium, strong)
	}

	// Test 3+ candidates for RareWorse
	candidatesWorse := []int{17, 18, 19, 20}
	mildW, mediumW, strongW := proposalTargetRanks(candidatesWorse, 16.0, RareWorse)
	if mildW != 17 || mediumW != 19 || strongW != 20 {
		t.Errorf("expected mild=17, medium=19, strong=20 for RareWorse, got mild=%d, medium=%d, strong=%d", mildW, mediumW, strongW)
	}

	// Test 1 candidate
	m1, med1, s1 := proposalTargetRanks([]int{5}, 10.0, RareBetter)
	if m1 != 5 || med1 != 5 || s1 != 5 {
		t.Errorf("expected all 5 for 1 candidate, got %d, %d, %d", m1, med1, s1)
	}
}

func TestMultiComponentMixtureWeights(t *testing.T) {
	// All components equal to P -> logQOverP = [0, 0, 0, 0]
	logQOverP := []float64{0.0, 0.0, 0.0, 0.0}
	weights := []float64{0.05, 0.20, 0.45, 0.30}
	w := mixtureImportanceWeightMulti(logQOverP, weights)
	if math.Abs(w-1.0) > 1e-9 {
		t.Errorf("expected weight 1.0 when all components equal P, got %f", w)
	}

	// Mixture including alphaOriginal = 0.05 guarantees weight <= 1 / 0.05 = 20
	logQOverPExtreme := []float64{0.0, 100.0, 500.0, 1000.0}
	wBound := mixtureImportanceWeightMulti(logQOverPExtreme, weights)
	if wBound > 20.0+1e-9 {
		t.Errorf("expected weight <= 20.0, got %f", wBound)
	}
}

func TestCompetitorDependentExactPoissonValidation(t *testing.T) {
	// Deterministic validation: Target team 1 has 0 remaining games and 0 points (bias 15).
	// Team 2 (0 pts, bias 20) vs Team 3 (0 pts, bias 10) play 1 remaining game with HomePower = 0.05, AwayPower = 5.0.
	// If Game 2 vs 3 ends in Away win (Team 3 wins): Team 3 has 3 pts (1st).
	// Team 1 has 0 pts, GD 0 (2nd!). Team 2 has 0 pts, GD -1 (3rd!).
	// Therefore, Team 1 reaches 2nd place (index 1) ONLY IF Game 2 vs 3 is an Away win for Team 3!
	// Exact analytical P(Away win) = Sum_{a=1..20} Poisson(a, 5.0) * Sum_{h=0..a-1} Poisson(h, 0.05)
	hMean := 0.05
	aMean := 5.0

	exactPAwayWin := 0.0
	for a := 1; a <= 20; a++ {
		pA := poisson_pmf(aMean, float64(a))
		pHLessA := 0.0
		for h := 0; h < a; h++ {
			pHLessA += poisson_pmf(hMean, float64(h))
		}
		exactPAwayWin += pA * pHLessA
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

	// Proposal targeting Team 1 (which has no games) by boosting Away (Team 3):
	compMeans := []GameProposalMeans{{Home: clampMean(hMean * 0.4), Away: clampMean(aMean * 2.5)}}
	components := []ProposalComponent{
		{Name: "original", Weight: 0.05, Means: originalMeans},
		{Name: "test_comp", Weight: 0.95, Means: compMeans},
	}

	job := &RareSimulationJob{
		TeamID:             1,
		Direction:          RareBetter,
		CandidatePositions: []int{1},
		RelevantTeams:      []int{3},
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

	// Verify hits occurred
	if est.Hits < 10 {
		t.Errorf("expected proposal to generate candidate hits, got %d hits", est.Hits)
	}

	// Verify ESS is materially useful (>= 10)
	if est.ESS < 10.0 {
		t.Errorf("expected ESS >= 10, got %f", est.ESS)
	}

	// Verify agreement within 3 standard errors of exact Away win probability
	if math.Abs(est.Probability-exactPAwayWin) > 3.0*est.StdErr {
		t.Errorf("Estimate %f deviated from exact P(Away win) %f by more than 3 stdErr (%f)", est.Probability, exactPAwayWin, est.StdErr)
	}
}

func TestTargetWithNoGamesRemainingCompetitorProposal(t *testing.T) {
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")

	// 3 teams: Team 1 (0 games left, 0 pts), Team 2 (1 game left vs Team 3, 0 pts), Team 3 (1 game left vs Team 2, 0 pts)
	// Team 1 has finished all games. But if Team 2 vs Team 3 ends in draw, Team 1 can reach 2nd place or move in ranks.
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
	// 4-team group where team 1 is candidate for position 0 (1st) and position 1 (2nd) in RareBetter direction.
	// Verify that a single RareSimulationJob estimates both positions simultaneously.
	teamGroups := []TeamType{
		{Team_id: 1, Bias: 0}, {Team_id: 2, Bias: 1}, {Team_id: 3, Bias: 2}, {Team_id: 4, Bias: 3},
	}
	table := NewTable([]uint32{1, 2, 3, 4})
	campaign := make([]*TeamCampaign, 4)
	campaign[table.Query(1)] = &TeamCampaign{id: 1, points: 0, bias: 0, points_win: 3, points_draw: 1, points_loss: 0}
	campaign[table.Query(2)] = &TeamCampaign{id: 2, points: 10, bias: 1, points_win: 3, points_draw: 1, points_loss: 0}
	campaign[table.Query(3)] = &TeamCampaign{id: 3, points: 10, bias: 2, points_win: 3, points_draw: 1, points_loss: 0}
	campaign[table.Query(4)] = &TeamCampaign{id: 4, points: 10, bias: 3, points_win: 3, points_draw: 1, points_loss: 0}

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

func TestGlobalBudgetEnforcement(t *testing.T) {
	var jobs []*RareSimulationJob
	for i := 1; i <= 20; i++ {
		jobs = append(jobs, &RareSimulationJob{
			TeamID:             i,
			Direction:          RareBetter,
			CandidatePositions: []int{0},
			Priority:           i * 10,
		})
	}

	allocateRareSimulationBudget(jobs, MaxRareIterations)

	sumIterations := 0
	for _, job := range jobs {
		sumIterations += job.Iterations
	}

	if sumIterations > MaxRareIterations {
		t.Errorf("total allocated iterations %d exceeded global budget %d", sumIterations, MaxRareIterations)
	}
}

func TestBudgetAllocationScaling(t *testing.T) {
	testCounts := []int{5, 10, 25, 50}

	for _, count := range testCounts {
		var jobs []*RareSimulationJob
		for i := 1; i <= count; i++ {
			jobs = append(jobs, &RareSimulationJob{
				TeamID:             i,
				Direction:          RareBetter,
				CandidatePositions: []int{0},
				Priority:           i * 5,
			})
		}

		allocateRareSimulationBudget(jobs, MaxRareIterations)

		sumIter := 0
		minIter := jobs[0].Iterations
		maxIter := jobs[0].Iterations

		for _, job := range jobs {
			sumIter += job.Iterations
			if job.Iterations < minIter {
				minIter = job.Iterations
			}
			if job.Iterations > maxIter {
				maxIter = job.Iterations
			}
		}

		if sumIter > MaxRareIterations {
			t.Errorf("for %d jobs, total iterations %d exceeded budget %d", count, sumIter, MaxRareIterations)
		}

		if count > 1 && maxIter > minIter*10 {
			t.Errorf("for %d jobs, allocation imbalance too large: min=%d, max=%d", count, minIter, maxIter)
		}
	}
}

func TestSyntheticUnderdogRareEvent(t *testing.T) {
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")

	// Two-team group with 1 remaining game.
	// HomePower = 0.02, AwayPower = 5.0.
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

	// End-to-end invariant assertions:
	// Rare position 1st place odds should be non-negative (e.g. > 0 if rare correction triggered, or 0 if unobserved)
	if p1stPercent < 0 {
		t.Errorf("Expected non-negative 1st place odds, got %f", p1stPercent)
	}

	// Verify total probabilities sum to 100%
	sum := team1Odds.Pos[0] + team1Odds.Pos[1]
	if math.Abs(sum-100.0) > 1e-6 {
		t.Errorf("Expected team position odds to sum to 100%%, got %f", sum)
	}
}

func TestFalse100PercentCorrection(t *testing.T) {
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "1")

	// Group where normal MC gives 100% for Team 2 (1st place) and 0% for Team 1 (1st place),
	// but Team 1 finishing 1st has a small nonzero probability.
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

	// Team 1 finishing 1st (index 0) should be > 0%
	if team1Odds.Pos[0] <= 0.0 {
		t.Errorf("Team 1 1st place odds should be > 0%%, got %f", team1Odds.Pos[0])
	}
	// Team 2 finishing 1st (index 0) should be < 100%
	if team2Odds.Pos[0] >= 100.0 {
		t.Errorf("Team 2 1st place odds should be < 100%%, got %f", team2Odds.Pos[0])
	}

	// Total sum for both teams should equal 100%
	sum1 := team1Odds.Pos[0] + team1Odds.Pos[1]
	sum2 := team2Odds.Pos[0] + team2Odds.Pos[1]
	if math.Abs(sum1-100.0) > 1e-6 {
		t.Errorf("Expected team 1 position odds to sum to 100%%, got %f", sum1)
	}
	if math.Abs(sum2-100.0) > 1e-6 {
		t.Errorf("Expected team 2 position odds to sum to 100%%, got %f", sum2)
	}
}

func TestImpossiblePositionStaysZero(t *testing.T) {
	// 2 teams, 0 remaining games. Team 1 has 3 pts, Team 2 has 0 pts.
	group := &GroupType{
		Id: 998,
		Phase: &PhaseType{
			Championship: &ChampionshipType{Point_win: 3, Point_draw: 1, Point_loss: 0},
			Sort:         "pt,gd,gf",
		},
		Team_groups: []TeamType{
			{Team_id: 1, Add_sub: 3},
			{Team_id: 2, Add_sub: 0},
		},
		Games: []*GameType{
			{
				Id:        1,
				HomeId:    1,
				AwayId:    2,
				HomeScore: 1,
				AwayScore: 0,
				Played:    true,
			},
		},
	}

	res := group.calculate_odds()
	teamOddsMap := res["team_odds"].(map[int]*TeamOdds)

	if teamOddsMap[1].Pos[1] != 0.0 {
		t.Errorf("Team 1 position 2 should be exactly 0, got %f", teamOddsMap[1].Pos[1])
	}
	if teamOddsMap[2].Pos[0] != 0.0 {
		t.Errorf("Team 2 position 1 should be exactly 0, got %f", teamOddsMap[2].Pos[0])
	}
}
