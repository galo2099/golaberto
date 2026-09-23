package main

import (
	"math"
	"math/rand"
	"testing"
)

func cemTeamFixture() ([]*GameType, []GameProposalMeans, []int) {
	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2},
		{Id: 2, HomeId: 1, AwayId: 3},
		{Id: 3, HomeId: 3, AwayId: 4},
		{Id: 4, HomeId: 5, AwayId: 6},
	}
	original := []GameProposalMeans{
		{Home: 4, Away: 6},
		{Home: 6, Away: 3},
		{Home: 5, Away: 7},
		{Home: 2, Away: 1},
	}
	return games, original, []int{1, 2, 3, 4, 5, 6}
}

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
		{Id: 4, HomeId: 5, AwayId: 6, HomePower: 1, AwayPower: 1},
	}
	original := make([]GameProposalMeans, len(games))
	for i, game := range games {
		game.home_table_index = table.Query(uint32(game.HomeId))
		game.away_table_index = table.Query(uint32(game.AwayId))
		original[i] = GameProposalMeans{game.HomePower, game.AwayPower}
	}
	return base, games, original, table, groups, []SortType{PT, GD, GF, BIAS}
}

func TestCEMTeamMomentMatchingAndLogSmoothing(t *testing.T) {
	games, original, teamIDs := cemTeamFixture()
	current := newCEMProposal(original, games, teamIDs)
	seasons := make([]CEMSeason, 8)
	for i := range seasons {
		seasons[i] = CEMSeason{TeamGoals: []int{8, 12, 15, 7, 3, 2}}
	}
	elite := []int{0, 1, 2, 3, 4, 5, 6, 7}
	updated := cemUpdateTeam(current, original, games, teamIDs, seasons, elite)
	if !updated.UpdateAllowed {
		t.Fatal("uniform elite set should allow a CEM update")
	}
	// Team 1 has Lambda=10 and mean elite goals 8; team 2 has Lambda=6
	// and mean elite goals 12. Smoothing is applied to log multipliers.
	want1 := 0.5 * math.Log(0.8)
	want2 := 0.5 * math.Log(2.0)
	if math.Abs(updated.TeamLogMultipliers[1]-want1) > 1e-12 {
		t.Fatalf("team 1 theta=%g, want %g", updated.TeamLogMultipliers[1], want1)
	}
	if math.Abs(updated.TeamLogMultipliers[2]-want2) > 1e-12 {
		t.Fatalf("team 2 theta=%g, want %g", updated.TeamLogMultipliers[2], want2)
	}
	if math.Abs(cemSmoothLogTheta(0, math.Log(0.64))-math.Log(0.8)) > 1e-12 {
		t.Fatal("log-space smoothing did not average the log multiplier")
	}
}

func TestCEMTeamMaterializationPreservesUnrelatedAndZeroMeans(t *testing.T) {
	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2},
		{Id: 2, HomeId: 3, AwayId: 4},
		{Id: 3, HomeId: 1, AwayId: 4, Played: true},
	}
	original := []GameProposalMeans{{2, 3}, {0, 4}, {8, 9}}
	proposal := CEMProposal{TeamLogMultipliers: map[int]float64{1: math.Log(0.5), 2: math.Log(2)}}
	got := materializeCEMProposal(proposal, original, games)
	if got[0].Home != 1 || got[0].Away != 6 {
		t.Fatalf("team tilts not applied to game 1: %+v", got[0])
	}
	if got[1].Home != 0 || got[1].Away != 4 {
		t.Fatalf("zero or unrelated team means changed: %+v", got[1])
	}
	if got[2] != original[2] {
		t.Fatalf("played game must remain untouched: got=%+v original=%+v", got[2], original[2])
	}
}

func TestCEMTeamMomentMatchingDoesNotMoveIrrelevantTeams(t *testing.T) {
	games, original, teamIDs := cemTeamFixture()
	teamIDs = append(teamIDs, 99) // team 99 has no remaining games
	current := newCEMProposal(original, games, teamIDs)
	seasons := make([]CEMSeason, 8)
	for i := range seasons {
		goals := []int{20, 2, 8, 7, 3, 2, 0}
		seasons[i] = CEMSeason{TeamGoals: goals}
	}
	updated := cemUpdateTeam(current, original, games, teamIDs, seasons, []int{0, 1, 2, 3, 4, 5, 6, 7})
	if updated.TeamLogMultipliers[1] <= 0 || updated.TeamLogMultipliers[2] >= 0 {
		t.Fatalf("team goal moments learned wrong directions: theta=%v", updated.TeamLogMultipliers)
	}
	if updated.TeamLogMultipliers[99] != 0 {
		t.Fatalf("team with no remaining games should retain zero theta: %v", updated.TeamLogMultipliers)
	}
	// Both remaining games involving team 1 must receive the same multiplier.
	if math.Abs(updated.Means[0].Home/original[0].Home-updated.Means[1].Home/original[1].Home) > 1e-12 {
		t.Fatal("team-level tilt is not coherent across games")
	}
}

func TestCEMEliteESSUsesExactImportanceWeightsAndGuardsUpdate(t *testing.T) {
	seasons := make([]CEMSeason, 8)
	for i := range seasons {
		seasons[i] = CEMSeason{TeamGoals: []int{100, 0}, LogWeight: 0}
	}
	_, ess := cemEliteWeights(seasons, []int{0, 1, 2, 3, 4, 5, 6, 7})
	if math.Abs(ess-8) > 1e-12 {
		t.Fatalf("uniform elite ESS=%g, want 8", ess)
	}
	seasons[0].LogWeight = math.Log(1000)
	_, ess = cemEliteWeights(seasons, []int{0, 1, 2, 3, 4, 5, 6, 7})
	if ess >= CEMMinEliteESSForUpdate {
		t.Fatalf("dominant importance weight should trigger low ESS, got %g", ess)
	}
	games := []*GameType{{Id: 1, HomeId: 1, AwayId: 2}}
	original := []GameProposalMeans{{1, 1}}
	current := newCEMProposal(original, games, []int{1, 2})
	updated := cemUpdateTeam(current, original, games, []int{1, 2}, seasons, []int{0, 1, 2, 3, 4, 5, 6, 7})
	if updated.UpdateAllowed {
		t.Fatal("low elite ESS must stop the update")
	}
	if updated.Means[0] != original[0] {
		t.Fatalf("low ESS update changed proposal means: %+v", updated.Means[0])
	}
}

func TestCEMTeamTrustRegionScalesThetaVector(t *testing.T) {
	wantKL := 2*math.Log(2) - 1
	if math.Abs(cemPoissonKL(2, 1)-wantKL) > 1e-12 {
		t.Fatalf("Poisson KL=%g, want %g", cemPoissonKL(2, 1), wantKL)
	}
	games, original, teamIDs := cemTeamFixture()
	proposal := newCEMProposal(original, games, teamIDs)
	proposal.TeamLogMultipliers[1] = 2
	proposal.TeamLogMultipliers[2] = -1
	proposal.Means = materializeCEMProposal(proposal, original, games)
	proposal.KL = cemTotalKL(proposal.Means, original, games)
	trusted := cemTrustRegionTeam(proposal, original, games, CEMMaxKL)
	if trusted.KL > CEMMaxKL+1e-9 {
		t.Fatalf("team trust region KL=%g exceeds limit %g", trusted.KL, CEMMaxKL)
	}
	if trusted.TeamLogMultipliers[1] <= 0 || trusted.TeamLogMultipliers[2] >= 0 {
		t.Fatalf("trust region did not preserve update directions: %v", trusted.TeamLogMultipliers)
	}
	if len(trusted.TeamLogMultipliers) != len(teamIDs) {
		t.Fatalf("CEM dimension=%d, want one parameter per team (%d)", len(trusted.TeamLogMultipliers), len(teamIDs))
	}
}

func TestCEMLikelihoodRemainsExactGameLevelPoissonRatio(t *testing.T) {
	const x = 3
	original, proposal := 1.2, 2.4
	got := logPoissonQOverP(x, original, proposal)
	want := float64(x)*math.Log(proposal/original) - (proposal - original)
	if math.Abs(got-want) > 1e-12 {
		t.Fatalf("log Q/P=%g, want exact game-level value %g", got, want)
	}
}

func TestCEMEliteSelectionKeepsExactThresholdAndRankOrdering(t *testing.T) {
	seasons := []CEMSeason{{Rank: 9}, {Rank: 11}, {Rank: 8}, {Rank: 12}, {Rank: 0}}
	elite, exact := cemEliteIndices(seasons, 10, RareBetter)
	if exact || len(elite) != 1 || elite[0] != 0 {
		t.Fatalf("nearest rank elite selection=%v exact=%t", elite, exact)
	}
	seasons = make([]CEMSeason, 20)
	for i := range seasons {
		seasons[i].Rank = 12
	}
	for i := 0; i < CEMExactEventThreshold; i++ {
		seasons[i].Rank = 10
	}
	elite, exact = cemEliteIndices(seasons, 10, RareBetter)
	if !exact || len(elite) != CEMExactEventThreshold {
		t.Fatalf("exact elites=%v exact=%t", elite, exact)
	}
}

func TestCEMValidationEvidenceGate(t *testing.T) {
	if ok, _ := cemValidationEvidence(CEMBatchStats{}, 0); ok {
		t.Fatal("no target evidence must not validate")
	}
	if ok, _ := cemValidationEvidence(CEMBatchStats{}, CEMMinExactHitsForValidation); !ok {
		t.Fatal("two exact hits should pass the validation gate")
	}
	if ok, _ := cemValidationEvidence(CEMBatchStats{ExactHits: 1, NearTargetRate: CEMNearTargetRateForValidation}, 1); !ok {
		t.Fatal("one exact hit with neighborhood evidence should pass")
	}
	if ok, _ := cemValidationEvidence(CEMBatchStats{NearTargetRate: CEMStrongNearTargetRate}, 0); !ok {
		t.Fatal("strong neighborhood evidence should pass")
	}
}

func TestCEMValidationPriorityPrefersUsefulRate(t *testing.T) {
	useful := &WeightedPilotResult{Samples: 1000, Hits: 10, ESSPerWork: 0.00001}
	excessive := &WeightedPilotResult{Samples: 1000, Hits: 80, ESSPerWork: 0.00001}
	if !(cemValidationPriority(useful) > cemValidationPriority(excessive)) {
		t.Fatalf("useful-rate candidate priority %g should exceed excessive-rate priority %g",
			cemValidationPriority(useful), cemValidationPriority(excessive))
	}
}

func TestCEMRoundHonorsWorkBudgetsAndUsesFreshValidation(t *testing.T) {
	base, games, original, table, groups, order := cemTestFixture()
	group := &GroupType{Id: 7, Games: games, Team_groups: groups}
	states := make([]*PositionSearchState, len(groups))
	for i := range states {
		states[i] = &PositionSearchState{Position: i, Status: StatusObserved}
	}
	states[0].Status = StatusFrontier
	searches := map[int]*TeamRareSearch{1: {TeamID: 1, Positions: states, NormalMeanRank: 2.5}}
	candidate := &FrontierCandidate{TeamID: 1, Position: 0, Direction: RareBetter, SearchState: states[0]}
	adaptCost := estimateSeasonWork(len(games), 1, len(groups))
	validationCost := estimateSeasonWork(len(games), 2, len(groups))
	limit := calculateMaxRareWork(len(games), len(groups))
	remaining := limit
	cemBudget := int64(float64(limit) * MaxCEMWorkFraction)
	validationBudget := int64(float64(limit) * MaxCEMValidationWorkFraction)
	round := runCEMRound([]*FrontierCandidate{candidate}, searches, group, base, table, order,
		original, &cemBudget, &validationBudget, &remaining, adaptCost, validationCost,
		rand.New(rand.NewSource(78)))
	if round.TargetsAttempted != 1 || round.CEMWork <= 0 ||
		round.CEMWork > int64(float64(limit)*MaxCEMWorkFraction) || remaining < 0 ||
		round.ValidationWork > int64(float64(limit)*MaxCEMValidationWorkFraction) {
		t.Fatalf("CEM budget accounting failed: round=%+v remaining=%d", round, remaining)
	}
	if round.ValidationWork > 0 && (len(candidate.SearchState.Pilots) != 1 ||
		candidate.SearchState.Pilots[0].Samples != CEMValidationSamples) {
		t.Fatalf("validation estimator did not use its fixed fresh sample: %+v", candidate.SearchState.Pilots)
	}
}

func TestCEMBatchStoresTeamTotalsWithCompactSeasonState(t *testing.T) {
	base, games, original, table, groups, order := cemTestFixture()
	teamIDs := teamIDsFromGroups(groups)
	proposal := newCEMProposal(original, games, teamIDs)
	batch := simulateCEMBatchForTeams(base, games, original, proposal.Means, table, order,
		groups, teamIDs, 1, 20, rand.New(rand.NewSource(123)))
	if len(batch) != 20 {
		t.Fatalf("batch has %d seasons, want 20", len(batch))
	}
	for _, season := range batch {
		if len(season.TeamGoals) != len(teamIDs) {
			t.Fatalf("stored %d team totals, want %d", len(season.TeamGoals), len(teamIDs))
		}
	}
}
