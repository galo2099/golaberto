package main

import (
	"math"
	"math/rand"
	"testing"
)

func cemTestFixture() ([]*TeamCampaign, []*GameType, []GameProposalMeans, *Table, []TeamType, []SortType) {
	groups := []TeamType{{Team_id: 1, Bias: 0}, {Team_id: 2, Bias: 1},
		{Team_id: 3, Bias: 2}, {Team_id: 4, Bias: 3}}
	table := NewTable([]uint32{1, 2, 3, 4, 5, 6})
	base := make([]*TeamCampaign, 6)
	for _, team := range groups {
		base[table.Query(uint32(team.Team_id))] = &TeamCampaign{id: team.Team_id,
			bias: team.Bias, points_win: 3, points_draw: 1}
	}
	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2, HomePower: 0.05, AwayPower: 2.0},
		{Id: 2, HomeId: 1, AwayId: 3, HomePower: 0.05, AwayPower: 2.0},
		{Id: 3, HomeId: 1, AwayId: 4, HomePower: 0.05, AwayPower: 2.0},
		{Id: 4, HomeId: 5, AwayId: 6, HomePower: 1, AwayPower: 1}, // no group team: irrelevant
	}
	original := make([]GameProposalMeans, len(games))
	for i, game := range games {
		game.home_table_index = table.Query(uint32(game.HomeId))
		game.away_table_index = table.Query(uint32(game.AwayId))
		original[i] = GameProposalMeans{game.HomePower, game.AwayPower}
	}
	return base, games, original, table, groups, []SortType{PT, GD, GF, BIAS}
}

func TestCEMWeightedPoissonMomentAndSmoothing(t *testing.T) {
	if got := cemWeightedMean([]float64{1, 3, 5}, []float64{1, 2, 1}); got != 3 {
		t.Fatalf("weighted elite Poisson mean = %g, want 3", got)
	}
	if got := cemSmoothMean(1, 3); got != 2 {
		t.Fatalf("smoothed mean = %g, want 2", got)
	}
	games := []*GameType{{Id: 1, HomeId: 1, AwayId: 2}}
	original := []GameProposalMeans{{1, 0}}
	seasons := []CEMSeason{
		{Scores: []CEMScore{{Home: 1}}, LogWeight: 0},
		{Scores: []CEMScore{{Home: 3}}, LogWeight: math.Log(2)},
		{Scores: []CEMScore{{Home: 5}}, LogWeight: 0},
	}
	updated := cemUpdate(original, original, games, seasons, []int{0, 1, 2})
	if math.Abs(updated.Means[0].Home-2) > 1e-12 || updated.Means[0].Away != 0 {
		t.Fatalf("weighted CE moment plus smoothing wrong: %+v", updated.Means[0])
	}
}

func TestCEMKLAndTrustRegion(t *testing.T) {
	want := 2*math.Log(2) - 1
	if math.Abs(cemPoissonKL(2, 1)-want) > 1e-12 {
		t.Fatalf("Poisson KL wrong: %g", cemPoissonKL(2, 1))
	}
	games := []*GameType{{Id: 1}, {Id: 2}}
	original := []GameProposalMeans{{1, 0.5}, {2, 1}}
	proposed := []GameProposalMeans{{5, 0.1}, {8, 4}}
	manual := cemPoissonKL(5, 1) + cemPoissonKL(0.1, 0.5) +
		cemPoissonKL(8, 2) + cemPoissonKL(4, 1)
	if math.Abs(cemTotalKL(proposed, original, games)-manual) > 1e-12 {
		t.Fatal("season KL must equal sum of score-dimension KLs")
	}
	trusted, kl := cemTrustRegion(original, proposed, games, CEMMaxKL)
	if kl > CEMMaxKL+1e-9 || kl < CEMMaxKL-1e-6 {
		t.Fatalf("trust region KL=%g, want boundary %g", kl, CEMMaxKL)
	}
	for i := range original {
		for _, side := range []struct{ before, desired, got float64 }{
			{original[i].Home, proposed[i].Home, trusted[i].Home},
			{original[i].Away, proposed[i].Away, trusted[i].Away},
		} {
			if math.IsNaN(side.got) || math.IsInf(side.got, 0) || side.got < 0 ||
				side.desired > side.before && (side.got < side.before || side.got > side.desired) ||
				side.desired < side.before && (side.got > side.before || side.got < side.desired) {
				t.Fatalf("trust region changed direction: %+v", side)
			}
		}
	}
}

func TestCEMInterpolationKeepsPoissonMeansFiniteAndNonNegative(t *testing.T) {
	games := []*GameType{{Id: 1}}
	original := []GameProposalMeans{{1, 2}}
	proposed := []GameProposalMeans{{math.NaN(), math.Inf(1)}}
	got := cemInterpolatedMeans(original, proposed, games, 1)
	if got[0] != original[0] {
		t.Fatalf("invalid proposal means must fall back to original: got=%+v original=%+v", got[0], original[0])
	}

	proposed = []GameProposalMeans{{0, 0}}
	got = cemInterpolatedMeans(original, proposed, games, 1)
	if got[0] != original[0] {
		t.Fatalf("zero proposal means must not create invalid trust-region parameters: got=%+v", got[0])
	}
	if cemPoissonKL(-1, 1) != math.Inf(1) || cemPoissonKL(math.NaN(), 1) != math.Inf(1) {
		t.Fatal("invalid Poisson KL inputs must be rejected")
	}
}

func TestCEMEliteESSUsesImportanceWeights(t *testing.T) {
	seasons := []CEMSeason{{LogWeight: 0}, {LogWeight: 0}, {LogWeight: 0}, {LogWeight: 0}}
	_, ess := cemEliteWeights(seasons, []int{0, 1, 2, 3})
	if math.Abs(ess-4) > 1e-12 {
		t.Fatalf("uniform elite weights ESS=%g, want 4", ess)
	}
	seasons[0].LogWeight = math.Log(100)
	_, ess = cemEliteWeights(seasons, []int{0, 1, 2, 3})
	if ess >= 1.2 {
		t.Fatalf("one dominant elite weight should have ESS near 1, got %g", ess)
	}
}

func TestCEMSparsifiesGameUpdatesBeforeTrustRegion(t *testing.T) {
	const gameCount = 100
	games := make([]*GameType, gameCount)
	original := make([]GameProposalMeans, gameCount)
	seasons := make([]CEMSeason, 4)
	for i := range games {
		games[i] = &GameType{Id: i + 1, HomeId: i + 1, AwayId: i + 1001}
		original[i] = GameProposalMeans{Home: 1, Away: 1}
	}
	for i := range seasons {
		seasons[i].Scores = make([]CEMScore, gameCount)
		seasons[i].LogWeight = 0
		for game := range seasons[i].Scores {
			seasons[i].Scores[game] = CEMScore{Home: 1, Away: 1}
		}
		for game := 0; game < 3; game++ {
			seasons[i].Scores[game] = CEMScore{Home: 5, Away: 1}
		}
	}
	updated := cemUpdate(original, original, games, seasons, []int{0, 1, 2, 3})
	if len(updated.SelectedGames) != 3 || updated.ChangedGames != 3 {
		t.Fatalf("sparse CEM selected %d changed %d games, want 3: %+v", len(updated.SelectedGames), updated.ChangedGames, updated)
	}
	for i, mean := range updated.Means {
		changed := mean != original[i]
		if i < 3 && !changed {
			t.Fatalf("signal game %d was not updated: %+v", i, updated)
		}
		if i >= 3 && changed {
			t.Fatalf("noise game %d changed: got=%+v original=%+v", i, mean, original[i])
		}
	}
}

func TestCEMTopKSelectionAndHysteresisAreDeterministic(t *testing.T) {
	signals := make([]CEMGameSignal, 20)
	for i := range signals {
		signals[i] = CEMGameSignal{GameIndex: i, GameScore: float64(i + 1)}
	}
	selected := selectCEMGames(signals, []int{0})
	if len(selected) != CEMMaxChangedGames {
		t.Fatalf("selected %d games, want %d", len(selected), CEMMaxChangedGames)
	}
	for _, signal := range selected {
		if !signal.Selected {
			t.Fatal("selected game was not marked selected")
		}
	}
	if selected[0].GameIndex != 19 {
		t.Fatalf("top game selection is not deterministic: first=%d", selected[0].GameIndex)
	}
}

func TestCEMValidationEvidenceGate(t *testing.T) {
	if ok, _ := cemValidationEvidence(CEMBatchStats{}, 0); ok {
		t.Fatal("zero-progress CEM must not validate")
	}
	if ok, reason := cemValidationEvidence(CEMBatchStats{}, CEMMinExactHitsForValidation); !ok || reason == "" {
		t.Fatal("two exact adaptation hits should pass validation gate")
	}
	if ok, _ := cemValidationEvidence(CEMBatchStats{ExactHits: 1, NearTargetRate: CEMNearTargetRateForValidation}, 1); !ok {
		t.Fatal("one exact hit with neighborhood evidence should pass validation gate")
	}
	if ok, _ := cemValidationEvidence(CEMBatchStats{NearTargetRate: CEMStrongNearTargetRate}, 0); !ok {
		t.Fatal("strong near-target concentration should pass validation gate")
	}
}

func TestCEMRelaxedAndExactEliteSelection(t *testing.T) {
	seasons := make([]CEMSeason, 20)
	for i := range seasons {
		seasons[i].Rank = 3
	}
	seasons[0].Rank, seasons[1].Rank, seasons[2].Rank = 1, 2, 2
	elite, exact := cemEliteIndices(seasons, 0, RareBetter)
	if exact || len(elite) != 3 || elite[0] != 0 {
		t.Fatalf("zero-hit batch must use nearest-rank elites: %v exact=%t", elite, exact)
	}
	for i := 0; i < CEMExactEventThreshold; i++ {
		seasons[i].Rank = 0
	}
	elite, exact = cemEliteIndices(seasons, 0, RareBetter)
	if !exact || len(elite) != CEMExactEventThreshold {
		t.Fatalf("exact hits must replace relaxed elites: %v exact=%t", elite, exact)
	}
	for _, index := range elite {
		if seasons[index].Rank != 0 {
			t.Fatal("non-event included after exact-event threshold")
		}
	}
}

func TestCEMLearnsRelevantMultiGameMeansWithoutExactFirstBatch(t *testing.T) {
	base, games, original, table, groups, order := cemTestFixture()
	rng := rand.New(rand.NewSource(314))
	proposal := append([]GameProposalMeans(nil), original...)
	first := simulateCEMBatch(base, games, original, proposal, table, order, groups, 1,
		CEMBatchSamples, rng)
	elite, exact := cemEliteIndices(first, 0, RareBetter)
	if exact {
		t.Fatal("fixture must exercise relaxed first-step CEM learning")
	}
	firstUpdate := cemUpdate(proposal, original, games, first, elite)
	if firstUpdate.Means[0].Home <= original[0].Home &&
		firstUpdate.Means[1].Home <= original[1].Home &&
		firstUpdate.Means[2].Home <= original[2].Home {
		t.Fatalf("relaxed elite update failed to raise any relevant target mean: %+v", firstUpdate.Means)
	}
	proposal = firstUpdate.Means
	for iteration := 1; iteration < CEMMaxIterations; iteration++ {
		batch := simulateCEMBatch(base, games, original, proposal, table, order, groups, 1,
			CEMBatchSamples, rng)
		elite, _ := cemEliteIndices(batch, 0, RareBetter)
		proposal = cemUpdate(proposal, original, games, batch, elite).Means
	}
	relevantChange := 0.0
	for i := 0; i < 3; i++ {
		relevantChange += cemGameChange(proposal[i], original[i])
	}
	irrelevantChange := cemGameChange(proposal[3], original[3])
	if relevantChange/3 <= irrelevantChange {
		t.Fatalf("irrelevant game moved more than event games: relevant=%g irrelevant=%g proposal=%v",
			relevantChange/3, irrelevantChange, proposal)
	}
	plain := simulateCEMBatch(base, games, original, original, table, order, groups, 1,
		5000, rand.New(rand.NewSource(99)))
	learned := simulateCEMBatch(base, games, original, proposal, table, order, groups, 1,
		5000, rand.New(rand.NewSource(99)))
	plainHits, learnedHits := 0, 0
	for i := range plain {
		if plain[i].Rank == 0 {
			plainHits++
		}
		if learned[i].Rank == 0 {
			learnedHits++
		}
	}
	if learnedHits <= plainHits {
		t.Fatalf("CEM did not increase exact-event frequency: plain=%d learned=%d", plainHits, learnedHits)
	}
	aggressive := append([]GameProposalMeans(nil), original...)
	for i := 0; i < 3; i++ {
		aggressive[i] = GameProposalMeans{Home: 5, Away: 0.1}
	}
	ess := make([]float64, 2)
	for i, learnedMeans := range [][]GameProposalMeans{proposal, aggressive} {
		pilot := &WeightedPilotResult{Proposal: SearchProposal{Components: buildCEMMixture(original, learnedMeans)}}
		evaluateWeightedPilot(pilot, base, games, original, table, order, groups,
			1, 0, 20000, estimateSeasonWork(len(games), 2, len(groups)),
			rand.New(rand.NewSource(int64(100+i))), 1)
		ess[i] = pilot.ESS
	}
	if ess[0] <= ess[1] {
		t.Fatalf("learned proposal should retain more weighted overlap: CEM ESS=%g aggressive ESS=%g", ess[0], ess[1])
	}
}

func TestCEMRoundUsesFreshValidationAndHonorsWorkBudget(t *testing.T) {
	base, games, original, table, groups, order := cemTestFixture()
	group := &GroupType{Id: 7, Games: games, Team_groups: groups}
	states := make([]*PositionSearchState, len(groups))
	for i := range states {
		states[i] = &PositionSearchState{Position: i, Status: StatusObserved}
	}
	states[0].Status = StatusFrontier
	searches := map[int]*TeamRareSearch{1: {TeamID: 1, Positions: states, NormalMeanRank: 2.5}}
	candidate := &FrontierCandidate{TeamID: 1, Position: 0, Direction: RareBetter,
		SearchState: states[0], Priority: 40}
	adaptCost := estimateSeasonWork(len(games), 1, len(groups))
	validationCost := estimateSeasonWork(len(games), 2, len(groups))
	limit := calculateMaxRareWork(len(games), len(groups))
	remaining := limit
	cemBudget := int64(float64(limit) * MaxCEMWorkFraction)
	validationBudget := int64(float64(limit) * MaxCEMValidationWorkFraction)
	round := runCEMRound([]*FrontierCandidate{candidate}, searches, group, base, table, order,
		original, &cemBudget, &validationBudget, &remaining,
		adaptCost, validationCost, rand.New(rand.NewSource(78)))
	if round.TargetsAttempted != 1 || round.CEMWork <= 0 || round.CEMWork > int64(float64(limit)*MaxCEMWorkFraction) ||
		remaining < 0 || round.ValidationWork > int64(float64(limit)*MaxCEMValidationWorkFraction) {
		t.Fatalf("CEM work budget failed: %+v remaining=%d", round, remaining)
	}
	if round.ValidationWork > 0 {
		if len(candidate.SearchState.Pilots) != 1 ||
			candidate.SearchState.Pilots[0].Samples != CEMValidationSamples {
			t.Fatalf("adaptation samples entered validation estimate: %+v", candidate.SearchState.Pilots)
		}
	}
	productionWork := int64(0)
	if len(round.Eligible) > 0 {
		plans := buildProductionPlans(round.Eligible, validationCost, searches)
		allocations := planProductionAllocations(plans, remaining, validationCost)
		if len(allocations) != 1 {
			t.Fatalf("validated CEM proposal did not reach production planning: %+v", allocations)
		}
		allocation := allocations[0]
		job := &RareSimulationJob{TeamID: 1, CandidatePositions: []int{0},
			Components: candidate.SearchState.BestProposal.Components, Iterations: allocation.Samples}
		estimates, _ := estimateRarePositionsForJob(base, games, original, table, order,
			groups, job, rand.New(rand.NewSource(79)), group.Id)
		if estimates[0].Samples != allocation.Samples || estimates[0].Samples ==
			candidate.SearchState.Pilots[0].Samples+round.Iterations*CEMBatchSamples {
			t.Fatal("adaptation or validation samples entered fresh fixed-N production estimate")
		}
		productionWork = allocation.Work
		remaining -= productionWork
	}
	fallbackSamples, fallbackWork := planFallbackSamples(remaining, adaptCost)
	if fallbackSamples <= 0 || round.CEMWork+round.ValidationWork+productionWork+fallbackWork > limit ||
		remaining-fallbackWork < 0 || remaining-fallbackWork >= adaptCost {
		t.Fatalf("CEM/validation/fallback ledger invalid: %+v fallback=%d remaining=%d",
			round, fallbackWork, remaining-fallbackWork)
	}
}

func TestCEMValidationPriorityPrefersUsefulRate(t *testing.T) {
	a := &WeightedPilotResult{Samples: 1000, Hits: 10, ESSPerWork: 0.00001}
	b := &WeightedPilotResult{Samples: 1000, Hits: 80, ESSPerWork: 0.00001}
	if !(cemValidationPriority(a) > cemValidationPriority(b)) {
		t.Fatalf("useful pilot rate should outrank excessive raw hit rate: a=%g b=%g",
			cemValidationPriority(a), cemValidationPriority(b))
	}
}
