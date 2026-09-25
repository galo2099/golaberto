package main

import (
	"math"
	"math/rand"
	"testing"
)

func createTestGroupForDiversified() (*GroupType, []*TeamCampaign, *Table, []SortType, map[int][]int) {
	tg1 := TeamType{Team_id: 1, Bias: 0}
	tg2 := TeamType{Team_id: 2, Bias: 1}
	tg3 := TeamType{Team_id: 3, Bias: 2}

	group := &GroupType{
		Id:          1,
		Team_groups: []TeamType{tg1, tg2, tg3},
		Games: []*GameType{
			{Id: 101, HomeId: 1, AwayId: 2, HomePower: 1.5, AwayPower: 1.0, Played: false, home_table_index: 0, away_table_index: 1},
			{Id: 102, HomeId: 2, AwayId: 3, HomePower: 1.2, AwayPower: 0.8, Played: false, home_table_index: 1, away_table_index: 2},
			{Id: 103, HomeId: 3, AwayId: 1, HomePower: 1.0, AwayPower: 1.4, Played: false, home_table_index: 2, away_table_index: 0},
		},
	}

	keys := []uint32{1, 2, 3}
	table := NewTable(keys)

	c1 := &TeamCampaign{id: 1, bias: 0, points: 10, points_win: 3, points_draw: 1, points_loss: 0}
	c2 := &TeamCampaign{id: 2, bias: 1, points: 9, points_win: 3, points_draw: 1, points_loss: 0}
	c3 := &TeamCampaign{id: 3, bias: 2, points: 1, points_win: 3, points_draw: 1, points_loss: 0}
	campaign := []*TeamCampaign{c1, c2, c3}

	for _, g := range group.Games {
		g.home_table_index = table.Query(uint32(g.HomeId))
		g.away_table_index = table.Query(uint32(g.AwayId))
	}

	sortOrder := []SortType{PT, GD, GF, BIAS}

	counts := map[int][]int{
		1: {500, 500, 0},
		2: {500, 500, 0},
		3: {0, 0, 1000},
	}

	return group, campaign, table, sortOrder, counts
}

func TestDiversifiedScoutAndTailDiscovery(t *testing.T) {
	group, campaign, table, sortOrder, _ := createTestGroupForDiversified()
	rng := rand.New(rand.NewSource(42))

	scout := runPlainMCScout(campaign, group.Games, table, sortOrder, group.Team_groups, 1000, 100, rng)

	if scout.Samples != 1000 {
		t.Errorf("Expected 1000 scout samples, got %d", scout.Samples)
	}

	tails := discoverDirectionalTails(scout, group.Team_groups)
	t.Logf("Discovered %d directional tails", len(tails))
	for _, tail := range tails {
		t.Logf("Tail: team=%d direction=%v unseen_count=%d first_unseen_pos=%d extremity=%.2f",
			tail.TeamID, tail.Direction, tail.UnseenFeasibleCount, tail.FirstUnseenPosition, tail.ExtremityScore)
	}
}

func TestDiversifiedProposalSearchAndFreeze(t *testing.T) {
	group, campaign, table, sortOrder, _ := createTestGroupForDiversified()
	rng := rand.New(rand.NewSource(12345))

	originalMeans := make([]GameProposalMeans, len(group.Games))
	for i, game := range group.Games {
		originalMeans[i] = GameProposalMeans{Home: game.HomePower, Away: game.AwayPower}
	}

	scout := runPlainMCScout(campaign, group.Games, table, sortOrder, group.Team_groups, 500, 100, rng)

	proposals, probeStats, searchWork := searchDiversifiedProposals(
		scout, campaign, group.Games, originalMeans, table, sortOrder, group.Team_groups, 50000, rng)

	if len(proposals) == 0 {
		t.Fatalf("Expected at least plain_mc proposal, got 0")
	}
	if proposals[0].Kind != ProposalPlainMC {
		t.Errorf("Expected first proposal to be plain_mc, got %s", proposals[0].Kind)
	}

	t.Logf("Proposals found: %d, searchWork: %d", len(proposals), searchWork)

	frozen := freezeDiversifiedDesign(proposals, probeStats, scout, group.Team_groups, 500000, scout.Work, searchWork)

	if len(frozen.Batches) == 0 {
		t.Fatalf("Expected non-empty frozen batches")
	}

	t.Logf("Frozen batches: %d, productionWork: %d", len(frozen.Batches), frozen.ProductionWork)
}

func TestDiversifiedProductionExecution(t *testing.T) {
	group, campaign, table, sortOrder, _ := createTestGroupForDiversified()
	rng := rand.New(rand.NewSource(999))

	originalMeans := make([]GameProposalMeans, len(group.Games))
	for i, game := range group.Games {
		originalMeans[i] = GameProposalMeans{Home: game.HomePower, Away: game.AwayPower}
	}

	scout := runPlainMCScout(campaign, group.Games, table, sortOrder, group.Team_groups, 500, 100, rng)
	proposals, probeStats, searchWork := searchDiversifiedProposals(
		scout, campaign, group.Games, originalMeans, table, sortOrder, group.Team_groups, 50000, rng)

	frozen := freezeDiversifiedDesign(proposals, probeStats, scout, group.Team_groups, 200000, scout.Work, searchWork)

	estimates := runDiversifiedProduction(frozen, campaign, group.Games, originalMeans, table, sortOrder, group.Team_groups, 54321)

	if len(estimates) != len(group.Team_groups) {
		t.Errorf("Expected estimates for %d teams, got %d", len(group.Team_groups), len(estimates))
	}

	for teamID, posMap := range estimates {
		for pos, est := range posMap {
			if est.Probability < 0 || est.Probability > 1 {
				t.Errorf("Invalid probability for team %d pos %d: %f", teamID, pos, est.Probability)
			}
			if est.ESS < 0 {
				t.Errorf("Invalid ESS for team %d pos %d: %f", teamID, pos, est.ESS)
			}
		}
	}
}

func TestEvaluateProposalQualityRankMovementWithoutExactHits(t *testing.T) {
	tail := DirectionalTail{
		TeamID:              37,
		Direction:           RareWorse,
		FrontierRank:        10,
		ScoutMeanRank:       1.5,
		ScoutP75:            2.0,
		ScoutP90:            3.0,
		UnseenFeasibleCount: 4,
	}

	// Zero exact hits at rank 10 (FrontierMass = 0), but mean rank moved to 6.5, P75 moved to 8.0, P90 to 9.0
	metrics := ProbeRankMetrics{
		TargetTeam:        37,
		Samples:           200,
		MeanRank:          6.5,
		P10:               2.0,
		P25:               4.0,
		P50:               6.0,
		P75:               8.0,
		P90:               9.0,
		FrontierMass:      0.0,
		Near1Mass:         0.04,
		Near2Mass:         0.08,
		Near3Mass:         0.15,
		UnresolvedSupport: 3,
	}

	q := evaluateProposalQuality(tail, metrics)

	if !q.ShouldRetain {
		t.Fatalf("Expected proposal to be retained due to rank movement and near-frontier mass, got reason=%s", q.Reason)
	}
	if q.TailScore < 3.0 {
		t.Errorf("Expected tail score >= 3.0, got %f", q.TailScore)
	}
}

func TestCompetitorShiftScaling(t *testing.T) {
	group, _, _, _, _ := createTestGroupForDiversified()
	originalMeans := make([]GameProposalMeans, len(group.Games))
	for i, game := range group.Games {
		originalMeans[i] = GameProposalMeans{Home: game.HomePower, Away: game.AwayPower}
	}
	normalMeanRanks := map[int]float64{1: 0.5, 2: 1.5, 3: 2.5}
	teamIDs := []int{1, 2, 3}

	prop05, ok05 := buildCompetitorAssistedProposal(1, RareWorse, 0.50, []int{2}, 0.50, normalMeanRanks, originalMeans, group.Games, teamIDs, 3)
	prop10, ok10 := buildCompetitorAssistedProposal(1, RareWorse, 1.00, []int{2}, 0.50, normalMeanRanks, originalMeans, group.Games, teamIDs, 3)

	if !ok05 || !ok10 {
		t.Fatalf("Failed to build competitor proposals: ok05=%v ok10=%v", ok05, ok10)
	}

	// For game 0 (Home=1, Away=2): Away power ratio under prop10 should be square of ratio under prop05 (since log theta doubles)
	logRatio05 := math.Log(prop05.Means[0].Away / originalMeans[0].Away)
	logRatio10 := math.Log(prop10.Means[0].Away / originalMeans[0].Away)

	if logRatio05 == 0 {
		t.Fatalf("Expected non-zero competitor log shift for s=0.50")
	}
	ratio := logRatio10 / logRatio05
	if ratio < 1.95 || ratio > 2.05 {
		t.Errorf("Expected competitor log-theta ratio 2.0 when strength doubles, got %f (logRatio05=%f, logRatio10=%f)", ratio, logRatio05, logRatio10)
	}
}

func TestCompetitorCorridorSelection(t *testing.T) {
	tg1 := TeamType{Team_id: 1}
	tg2 := TeamType{Team_id: 2}
	tg3 := TeamType{Team_id: 3}
	tg4 := TeamType{Team_id: 4}
	teamGroups := []TeamType{tg1, tg2, tg3, tg4}

	// Scout data:
	// Team 1 (target) has mean rank 1.0
	// Team 2 has mass at ranks 8..12 (around frontier rank 10)
	// Team 3 has mass at ranks 0..3 (far from frontier rank 10)
	// Team 4 has mass at ranks 4..6
	scout := ScoutData{
		Samples: 1000,
		TeamMeanRanks: map[int]float64{
			1: 1.0,
			2: 9.5,
			3: 1.5,
			4: 5.0,
		},
		TeamCounts: map[int][]int{
			1: {500, 500, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0},
			2: {0, 0, 0, 0, 0, 0, 0, 0, 300, 400, 300, 0, 0, 0},
			3: {400, 400, 200, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0},
			4: {0, 0, 0, 0, 300, 400, 300, 0, 0, 0, 0, 0, 0, 0},
		},
	}

	comps := selectCompetitorsForTail(1, RareWorse, 10, scout, teamGroups, 2)

	if len(comps) < 1 {
		t.Fatalf("Expected at least 1 competitor selected, got %d", len(comps))
	}
	if comps[0] != 2 {
		t.Errorf("Expected team 2 (corridor mass around rank 10) to be selected first, got team %d", comps[0])
	}
}

func TestKLLimitRejection(t *testing.T) {
	group, _, _, _, _ := createTestGroupForDiversified()
	originalMeans := make([]GameProposalMeans, len(group.Games))
	for i, game := range group.Games {
		originalMeans[i] = GameProposalMeans{Home: game.HomePower, Away: game.AwayPower}
	}
	teamIDs := []int{1, 2, 3}

	// Extreme strength s = 15.0 (RareBetter) should exceed KL max limit of 3.0
	_, ok := buildDirectTeamProposal(1, RareBetter, 15.0, originalMeans, group.Games, teamIDs, 3)

	if ok {
		t.Errorf("Expected extreme proposal (s=10.0) to be rejected due to KL limit, but build succeeded")
	}
}

func TestBetaSampleCountScaling(t *testing.T) {
	// Equal per-sample variance (0.01) for Batch A (n=10000) and Batch B (n=1000)
	varA, varB := 0.01, 0.01
	nA, nB := 10000, 1000

	scoreA := float64(nA) / varA
	scoreB := float64(nB) / varB
	tot := scoreA + scoreB

	betaA := scoreA / tot
	betaB := scoreB / tot

	if math.Abs(betaA-0.90909) > 0.001 || math.Abs(betaB-0.09091) > 0.001 {
		t.Errorf("Expected betaA ~ 0.909 and betaB ~ 0.091 based on sample count scaling, got betaA=%f, betaB=%f", betaA, betaB)
	}
}

func TestBetaLowerVariancePreference(t *testing.T) {
	// Batch A has 10x fewer samples (nA=1000 vs nB=10000) but 100x lower per-sample variance (varA=1e-6 vs varB=1e-4)
	varA, varB := 1e-6, 1e-4
	nA, nB := 1000, 10000

	scoreA := float64(nA) / varA // 1e9
	scoreB := float64(nB) / varB // 1e8
	tot := scoreA + scoreB

	betaA := scoreA / tot
	betaB := scoreB / tot

	if betaA <= betaB {
		t.Errorf("Expected batch A (lower variance) to receive larger beta despite fewer samples, got betaA=%f, betaB=%f", betaA, betaB)
	}
	if math.Abs(betaA-0.90909) > 0.001 {
		t.Errorf("Expected betaA ~ 0.909, got %f", betaA)
	}
}

func TestZeroSearchHitsBetaZero(t *testing.T) {
	batches := []FrozenProductionBatch{
		{Proposal: DiversifiedProposal{ID: "targeted", Kind: ProposalSingleTeam}, Samples: 2000},
		{Proposal: DiversifiedProposal{ID: "plain_mc", Kind: ProposalPlainMC}, Samples: 48000},
	}

	cell := [2]int{1, 10} // rare cell
	probeStats := map[string]ProposalProbeStats{
		"targeted": {CellPredictedVarPerSample: map[[2]int]float64{cell: math.Inf(1)}}, // 0 hits => Inf variance
		"plain_mc": {CellPredictedVarPerSample: map[[2]int]float64{cell: 1e-4}},
	}

	betas := make([]float64, len(batches))
	sumScore := 0.0
	for j, batch := range batches {
		varPerSample := probeStats[batch.Proposal.ID].CellPredictedVarPerSample[cell]
		score := 0.0
		if !math.IsNaN(varPerSample) && !math.IsInf(varPerSample, 0) && varPerSample > 0 && batch.Samples > 0 {
			score = float64(batch.Samples) / varPerSample
		}
		betas[j] = score
		sumScore += score
	}
	for j := range betas {
		betas[j] /= sumScore
	}

	if betas[0] != 0.0 || betas[1] != 1.0 {
		t.Errorf("Expected targeted proposal with zero hits to receive beta=0, got betas[0]=%f, betas[1]=%f", betas[0], betas[1])
	}
}

func TestPureQToProductionMixtureVarianceTransform(t *testing.T) {
	// Discrete distribution over 3 outcomes {1, 2, 3}
	// Probabilities under P: [0.10, 0.20, 0.70]
	// Probabilities under Q: [0.50, 0.30, 0.20]
	pProbs := []float64{0.10, 0.20, 0.70}
	qProbs := []float64{0.50, 0.30, 0.20}
	eps := 0.05

	// Exact moments for event A = {outcome 0} (p0)
	// Under mixture M = eps P + (1-eps) Q
	mProbs := make([]float64, 3)
	for i := 0; i < 3; i++ {
		mProbs[i] = eps*pProbs[i] + (1.0-eps)*qProbs[i]
	}

	// Exact E_M[Y^2] for outcome 0 where Y = w_M I_0 = (P(x)/M(x)) I_0
	wM0 := pProbs[0] / mProbs[0]
	exactEM_Y2 := wM0 * wM0 * mProbs[0] // = P(0)^2 / M(0)

	// Simulated pure-Q estimation of E_M[Y^2]
	rng := rand.New(rand.NewSource(42))
	samples := 100000
	sumY2 := 0.0

	for s := 0; s < samples; s++ {
		u := rng.Float64()
		outcome := 0
		if u > qProbs[0] {
			outcome = 1
			if u > qProbs[0]+qProbs[1] {
				outcome = 2
			}
		}

		if outcome == 0 {
			r := pProbs[0] / qProbs[0]
			den := eps*r + (1.0-eps)
			y2 := (r * r) / den
			sumY2 += y2
		}
	}

	estimatedEM_Y2 := sumY2 / float64(samples)

	diff := math.Abs(estimatedEM_Y2 - exactEM_Y2)
	if diff > 0.005 {
		t.Errorf("Pure-Q to mixture M variance transform disagree with exact moment: estimated=%f, exact=%f, diff=%f", estimatedEM_Y2, exactEM_Y2, diff)
	}
}

func TestIntermediateProposalUncoveredFrontier(t *testing.T) {
	tail := DirectionalTail{
		TeamID:        37,
		Direction:     RareWorse,
		FrontierRank:  12,
		ScoutMeanRank: 1.0,
		ScoutP75:      1.5,
		ScoutP90:      2.0,
	}

	// Intermediate proposal metrics: mean rank moves to 3.0, P90 moves to 4.0
	// Frontier is at 12, so FrontierMass = 0, Near1Mass = 0, Near2Mass = 0.005
	metrics := ProbeRankMetrics{
		TargetTeam:        37,
		Samples:           200,
		MeanRank:          3.0,
		P10:               1.0,
		P25:               2.0,
		P50:               3.0,
		P75:               3.5,
		P90:               4.0,
		FrontierMass:      0.0,
		Near1Mass:         0.0,
		Near2Mass:         0.005,
		Near3Mass:         0.010,
		UnresolvedSupport: 2,
	}

	q := evaluateProposalQuality(tail, metrics)

	if !q.ShouldRetain {
		t.Errorf("Expected intermediate proposal to be retained as useful shift, got reason=%s", q.Reason)
	}
	if q.IsFrontierCovered {
		t.Errorf("Expected tail frontier (12) to NOT be covered when p90=4.0")
	}
}

func TestTeam37TailMovementRegression(t *testing.T) {
	// Create a 4-team group where team 1 (team 37 surrogate) starts with high points (mean rank ~0)
	// and has a worse tail (positions 1, 2, 3 unseen in scout).
	tg1 := TeamType{Team_id: 1, Bias: 0}
	tg2 := TeamType{Team_id: 2, Bias: 1}
	tg3 := TeamType{Team_id: 3, Bias: 2}
	tg4 := TeamType{Team_id: 4, Bias: 3}

	group := &GroupType{
		Id:          16982,
		Team_groups: []TeamType{tg1, tg2, tg3, tg4},
		Games: []*GameType{
			{Id: 101, HomeId: 1, AwayId: 2, HomePower: 1.5, AwayPower: 1.0, Played: false},
			{Id: 102, HomeId: 1, AwayId: 3, HomePower: 1.4, AwayPower: 1.1, Played: false},
			{Id: 103, HomeId: 1, AwayId: 4, HomePower: 1.6, AwayPower: 0.9, Played: false},
			{Id: 104, HomeId: 2, AwayId: 3, HomePower: 1.2, AwayPower: 1.2, Played: false},
			{Id: 105, HomeId: 2, AwayId: 4, HomePower: 1.3, AwayPower: 1.0, Played: false},
			{Id: 106, HomeId: 3, AwayId: 4, HomePower: 1.1, AwayPower: 1.1, Played: false},
		},
	}

	keys := []uint32{1, 2, 3, 4}
	table := NewTable(keys)

	for _, g := range group.Games {
		g.home_table_index = table.Query(uint32(g.HomeId))
		g.away_table_index = table.Query(uint32(g.AwayId))
	}

	c1 := &TeamCampaign{id: 1, bias: 0, points: 18, points_win: 3, points_draw: 1, points_loss: 0}
	c2 := &TeamCampaign{id: 2, bias: 1, points: 11, points_win: 3, points_draw: 1, points_loss: 0}
	c3 := &TeamCampaign{id: 3, bias: 2, points: 11, points_win: 3, points_draw: 1, points_loss: 0}
	c4 := &TeamCampaign{id: 4, bias: 3, points: 11, points_win: 3, points_draw: 1, points_loss: 0}
	campaign := []*TeamCampaign{c1, c2, c3, c4}

	sortOrder := []SortType{PT, GD, GF, BIAS}
	rng := rand.New(rand.NewSource(16982))

	originalMeans := make([]GameProposalMeans, len(group.Games))
	for i, g := range group.Games {
		originalMeans[i] = GameProposalMeans{Home: g.HomePower, Away: g.AwayPower}
	}

	scout := runPlainMCScout(campaign, group.Games, table, sortOrder, group.Team_groups, 1000, 100, rng)

	proposals, _, _ := searchDiversifiedProposals(
		scout, campaign, group.Games, originalMeans, table, sortOrder, group.Team_groups, 100000, rng)

	if len(proposals) <= 1 {
		t.Fatalf("Expected team 1 worse-tail proposals to be retained based on rank movement, but got only %d proposal(s)", len(proposals))
	}

	t.Logf("Retained %d proposal(s) for team 1 worse-tail search!", len(proposals))
	for _, p := range proposals {
		t.Logf("Retained proposal: ID=%s Kind=%s Strength=%.2f KL=%.3f", p.ID, p.Kind, p.Strength, p.KL)
	}
}
