package main

import (
	"fmt"
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
	confirmationBudget := int64(float64(limit) * MaxCEMConfirmationWorkFraction)
	explorationBudget := int64(float64(cemBudget) * CEMMaxExplorationFraction)
	round := runCEMRound([]*FrontierCandidate{candidate}, searches, group, base, table, order,
		original, &cemBudget, &confirmationBudget, &validationBudget, &remaining,
		&explorationBudget, adaptCost, adaptCost, validationCost, limit,
		rand.New(rand.NewSource(78)))
	if round.TargetsAttempted != 1 || round.CEMWork <= 0 ||
		round.CEMWork > int64(float64(limit)*MaxCEMWorkFraction) || remaining < 0 ||
		round.ValidationWork > int64(float64(limit)*MaxCEMValidationWorkFraction) ||
		round.ConfirmationWork > int64(float64(limit)*MaxCEMConfirmationWorkFraction) {
		t.Fatalf("CEM budget accounting failed: round=%+v remaining=%d", round, remaining)
	}
	if round.ValidationWork > 0 && (len(candidate.SearchState.Pilots) != 1 ||
		candidate.SearchState.Pilots[0].Samples != CEMValidationSamples) {
		t.Fatalf("validation estimator did not use its fixed fresh sample: %+v", candidate.SearchState.Pilots)
	}
	if round.ValidationAttempts > 0 && round.ConfirmationSuccesses == 0 {
		t.Fatal("mixture validation ran without successful independent confirmation")
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

func cemSchedulerFixture(count int) ([]*CEMCandidateState, map[int]*TeamRareSearch) {
	states := make([]*CEMCandidateState, count)
	searches := make(map[int]*TeamRareSearch, count)
	for i := 0; i < count; i++ {
		teamID := i + 1
		positions := []*PositionSearchState{
			{Position: 0, Status: StatusObserved},
			{Position: 1, Status: StatusFrontier},
			{Position: 2, Status: StatusObserved},
		}
		candidate := &FrontierCandidate{TeamID: teamID, Position: 1,
			Direction: RareBetter, SearchState: positions[1]}
		states[i] = &CEMCandidateState{Candidate: candidate, Active: true}
		searches[teamID] = &TeamRareSearch{TeamID: teamID, Positions: positions, NormalMeanRank: 1}
	}
	return states, searches
}

func TestCEMSchedulerGivesEveryCandidateFairnessBatchFirst(t *testing.T) {
	states, searches := cemSchedulerFixture(4)
	remaining := 5
	var order []int
	runCEMAdaptiveSchedule(states, searches, func() bool { return remaining > 0 }, func() bool { return remaining > 0 },
		func(state *CEMCandidateState, _ int, reason string) bool {
			if reason == "initial_fairness" {
				order = append(order, state.Candidate.TeamID)
			} else {
				order = append(order, state.Candidate.TeamID)
			}
			state.Iterations++
			remaining--
			if reason == "initial_fairness" {
				state.ProgressScore = float64(state.Candidate.TeamID)
			} else {
				state.Active = false
			}
			return true
		})
	if len(order) != 5 {
		t.Fatalf("scheduler made %d decisions, want 5: %v", len(order), order)
	}
	for i, teamID := range order[:4] {
		if teamID != i+1 {
			t.Fatalf("fairness phase order=%v", order)
		}
	}
	if order[4] != 4 {
		t.Fatalf("adaptive batch went to team %d, want highest progress team 4", order[4])
	}
	for i := 0; i < 3; i++ {
		if states[i].Iterations != 1 {
			t.Fatalf("team %d received %d batches before fairness completed", i+1, states[i].Iterations)
		}
	}
	if states[3].Iterations != 2 {
		t.Fatalf("highest progress candidate received %d batches after fairness", states[3].Iterations)
	}
}

func TestCEMSchedulerFollowsProgressAndRecentRegression(t *testing.T) {
	states, searches := cemSchedulerFixture(3)
	states[0].HasStats, states[0].Iterations = true, 3
	states[0].FirstStats = CEMBatchStats{EliteMeanDistance: 8}
	states[0].BestStats = CEMBatchStats{EliteMeanDistance: 6, NearTargetRate: 0.03}
	states[0].BestEliteDistance, states[0].BestNearTargetRate = 6, 0.03
	states[0].PreviousStats = CEMBatchStats{EliteMeanDistance: 6}
	states[0].LastStats = CEMBatchStats{EliteMeanDistance: 6.5, NearTargetRate: 0}
	states[0].StalledIterations, states[0].RegressionIterations = 1, 1
	states[1].HasStats, states[1].Iterations = true, 3
	states[1].FirstStats = CEMBatchStats{EliteMeanDistance: 5}
	states[1].BestStats = CEMBatchStats{EliteMeanDistance: 3.5}
	states[1].BestEliteDistance = 3.5
	states[1].PreviousStats = CEMBatchStats{EliteMeanDistance: 4}
	states[1].LastStats = CEMBatchStats{EliteMeanDistance: 3.5}
	states[2].HasStats, states[2].Iterations = true, 3
	states[2].FirstStats = CEMBatchStats{EliteMeanDistance: 8}
	states[2].BestStats = CEMBatchStats{EliteMeanDistance: 7.5}
	states[2].BestEliteDistance = 7.5
	states[2].LastStats = CEMBatchStats{EliteMeanDistance: 7.5}
	for _, state := range states {
		state.Active = true
		state.ProgressScore = updateCEMProgressScore(state)
	}
	if got := selectCEMCandidate(states, searches); got != states[1] {
		t.Fatalf("recently regressing candidate won scheduling: scores=%g,%g,%g",
			states[0].ProgressScore, states[1].ProgressScore, states[2].ProgressScore)
	}
}

func TestCEMExactHitGetsSchedulerPriority(t *testing.T) {
	states, searches := cemSchedulerFixture(2)
	states[0].Active, states[1].Active = true, true
	states[0].HasStats, states[0].Iterations = true, 1
	states[0].EverHadExactHit, states[0].ExactHitPriorityActive = true, true
	states[0].MaxExactHits = 1
	states[0].ProgressScore = updateCEMProgressScore(states[0])
	states[1].HasStats, states[1].Iterations = true, 3
	states[1].FirstStats = CEMBatchStats{EliteMeanDistance: 10}
	states[1].BestStats = CEMBatchStats{EliteMeanDistance: 1}
	states[1].BestEliteDistance = 1
	states[1].ProgressScore = updateCEMProgressScore(states[1])
	if got := selectCEMCandidate(states, searches); got != states[0] {
		t.Fatalf("candidate without an exact hit outranked an observed exact hit: scores=%g,%g",
			states[0].ProgressScore, states[1].ProgressScore)
	}
}

func TestCEMHistoricalHitDoesNotKeepSchedulerBonusAfterFailure(t *testing.T) {
	states, searches := cemSchedulerFixture(2)
	for _, state := range states {
		state.Active = true
		state.HasStats = true
		state.FirstStats = CEMBatchStats{EliteMeanDistance: 10}
		state.BestEliteDistance = 6
	}
	states[0].EverHadExactHit = true
	states[0].ExactHitPriorityActive = false
	states[0].MaxExactHits = 1
	states[1].LastStats.EliteESS = 30
	states[1].LastStats.NearTargetRate = 0.25
	states[0].ProgressScore = updateCEMProgressScore(states[0])
	states[1].ProgressScore = updateCEMProgressScore(states[1])
	if states[0].ProgressScore >= 1000 || selectCEMCandidate(states, searches) != states[1] {
		t.Fatalf("historical exact hit retained priority: scores=%g,%g", states[0].ProgressScore, states[1].ProgressScore)
	}
	states[0].ExactHitPriorityActive = true
	states[0].ProgressScore = updateCEMProgressScore(states[0])
	if selectCEMCandidate(states, searches) != states[0] {
		t.Fatal("new exact-hit evidence did not restore scheduler priority")
	}
}

func TestCEMAdaptiveSchedulerCanExceedFourBatchesAndStopWeakCandidate(t *testing.T) {
	states, searches := cemSchedulerFixture(2)
	remainingSamples := 3000
	spent := 0
	runCEMAdaptiveSchedule(states, searches,
		func() bool { return remainingSamples >= CEMMinBatchSamples },
		func() bool { return remainingSamples >= CEMMinBatchSamples },
		func(state *CEMCandidateState, _ int, _ string) bool {
			samples := min(CEMBatchSamples, remainingSamples)
			remainingSamples -= samples
			spent += samples
			state.Iterations++
			if state.Candidate.TeamID == 1 {
				state.ProgressScore = float64(state.Iterations)
			} else {
				state.StalledIterations++
				if state.StalledIterations >= CEMMaxStalledIterations {
					state.Active = false
					state.StopReason = "stalled"
				}
			}
			if state.Iterations >= 8 && state.Candidate.TeamID == 1 {
				state.Active = false
				state.StopReason = "per_candidate_cap"
			}
			return true
		})
	if states[0].Iterations <= 4 || states[0].Iterations <= states[1].Iterations {
		t.Fatalf("strong candidate did not receive adaptive work: strong=%d weak=%d", states[0].Iterations, states[1].Iterations)
	}
	if spent > 5000 || spent != 3000 {
		t.Fatalf("adaptive scheduler spent %d samples against 5000 cap and 3000 test budget", spent)
	}
}

func TestCEMAdaptiveSchedulerNeverExceeds5000EquivalentSamples(t *testing.T) {
	states, searches := cemSchedulerFixture(5)
	remaining, spent := MaxCEMPlainEquivalentSamples, 0
	runCEMAdaptiveSchedule(states, searches,
		func() bool { return remaining >= CEMMinBatchSamples },
		func() bool { return remaining >= CEMMinBatchSamples },
		func(state *CEMCandidateState, _ int, _ string) bool {
			samples := min(CEMBatchSamples, remaining)
			remaining -= samples
			spent += samples
			state.Iterations++
			state.ProgressScore = float64(state.Candidate.TeamID)
			if state.Iterations >= 12 {
				state.Active = false
				state.StopReason = "per_candidate_cap"
			}
			return true
		})
	if spent > MaxCEMPlainEquivalentSamples || spent+remaining != MaxCEMPlainEquivalentSamples {
		t.Fatalf("CEM scheduler spent %d and left %d from 5000", spent, remaining)
	}
	for i, state := range states {
		if state.Iterations == 0 {
			t.Fatalf("candidate %d did not receive its fairness batch", i+1)
		}
	}
}

func TestCEMAdmissionCapsInitialCandidatesAndExplorationBudget(t *testing.T) {
	states, searches := cemSchedulerFixture(12)
	admitted, excluded := admitCEMCandidates(states, searches, CEMMaxInitialCandidates, 12*CEMBatchSamples)
	if len(admitted) != 6 || len(excluded) != 6 {
		t.Fatalf("candidate cap admitted=%d excluded=%d, want 6 and 6", len(admitted), len(excluded))
	}
	for i, state := range admitted {
		if state.Candidate.TeamID != i+1 {
			t.Fatalf("admission order team=%d at %d; want deterministic team %d", state.Candidate.TeamID, i, i+1)
		}
	}
	admitted, excluded = admitCEMCandidates(states, searches, CEMMaxInitialCandidates, 4*CEMBatchSamples)
	if len(admitted) != 4 || len(excluded) != 8 {
		t.Fatalf("exploration budget admitted=%d excluded=%d, want 4 and 8", len(admitted), len(excluded))
	}
}

func TestCEMAdmissionRunsEveryInitialBatchBeforeAdaptiveScheduling(t *testing.T) {
	states, searches := cemSchedulerFixture(12)
	admitted, excluded := admitCEMCandidates(states, searches, CEMMaxInitialCandidates, 6*CEMBatchSamples)
	remainingInitial := len(admitted)
	initialOrder := []int{}
	adaptiveStarted := false
	runCEMAdaptiveSchedule(admitted, searches,
		func() bool { return remainingInitial > 0 },
		func() bool { return false },
		func(state *CEMCandidateState, _ int, reason string) bool {
			if reason != "initial_fairness" {
				adaptiveStarted = true
				return false
			}
			if adaptiveStarted {
				t.Fatal("adaptive scheduling started before all initial candidates were piloted")
			}
			initialOrder = append(initialOrder, state.Candidate.TeamID)
			state.Iterations++
			remainingInitial--
			return true
		})
	if len(initialOrder) != 6 || remainingInitial != 0 {
		t.Fatalf("initial fairness reached %d candidates with %d remaining", len(initialOrder), remainingInitial)
	}
	for _, state := range excluded {
		if state.Iterations != 0 {
			t.Fatalf("excluded candidate %d received an initial batch", state.Candidate.TeamID)
		}
	}
}

func TestCEMAdmissionPrefersTeamDiversityOnFirstPass(t *testing.T) {
	states := make([]*CEMCandidateState, 3)
	searches := map[int]*TeamRareSearch{}
	for i := range states {
		team := 1
		if i == 2 {
			team = 2
		}
		pos := i + 1
		positionStates := []*PositionSearchState{{Position: 0, Status: StatusUnexplored},
			{Position: 1, Status: StatusUnexplored}, {Position: 2, Status: StatusUnexplored},
			{Position: 3, Status: StatusUnexplored}}
		positionStates[pos].Status = StatusFrontier
		candidate := &FrontierCandidate{TeamID: team, Position: pos, SearchState: positionStates[pos]}
		states[i] = &CEMCandidateState{Candidate: candidate, Active: true}
		searches[team] = &TeamRareSearch{TeamID: team, Positions: positionStates, NormalMeanRank: 0}
	}
	admitted, _ := admitCEMCandidates(states, searches, 2, 2*CEMBatchSamples)
	if len(admitted) != 2 || admitted[0].Candidate.TeamID != 1 || admitted[1].Candidate.TeamID != 2 {
		got := []int{}
		for _, state := range admitted {
			got = append(got, state.Candidate.TeamID)
		}
		t.Fatalf("diverse admission teams=%v, want [1 2]", got)
	}
}

func TestCEMConfirmationRequiresIndependentExactEventsAndHealthyWeights(t *testing.T) {
	acceptable := summarizeCEMConfirmation([]CEMSeason{
		{Rank: 3, LogWeight: 0}, {Rank: 3, LogWeight: 0}, {Rank: 4, LogWeight: 0},
	}, 3)
	if ok, _ := cemConfirmationAccepted(acceptable); !ok || acceptable.Hits != 2 || acceptable.EventESS != 2 {
		t.Fatalf("balanced repeated events should confirm: stats=%+v", acceptable)
	}
	pathological := summarizeCEMConfirmation([]CEMSeason{
		{Rank: 3, LogWeight: 10}, {Rank: 3, LogWeight: 0}, {Rank: 3, LogWeight: 0},
	}, 3)
	if ok, reason := cemConfirmationAccepted(pathological); ok || reason != "pathological_event_weight_share" || pathological.MaxEventWeightShare <= 0.95 {
		t.Fatalf("concentrated event weight should reject: stats=%+v reason=%q", pathological, reason)
	}
	if ok, reason := cemConfirmationAccepted(CEMConfirmationStats{Hits: 2, EventESS: 1, MaxEventWeightShare: 0.5}); ok || reason != "pathological_event_ess" {
		t.Fatalf("multi-hit confirmation with pathological ESS should reject: reason=%q", reason)
	}
	oneHit := summarizeCEMConfirmation([]CEMSeason{{Rank: 3, LogWeight: 0}}, 3)
	if ok, reason := cemConfirmationAccepted(oneHit); !ok || reason != "single_independent_exact_event" {
		t.Fatalf("one hit should confirm despite event ESS/share of one: stats=%+v reason=%q", oneHit, reason)
	}
}

func TestCEMConfirmationSummaryKeepsExactPOverQWeights(t *testing.T) {
	stats := summarizeCEMConfirmation([]CEMSeason{{Rank: 2, LogWeight: math.Log(0.25)},
		{Rank: 1, LogWeight: 0}, {Rank: 2, LogWeight: math.Log(0.75)}}, 2)
	want := (0.25 + 0.75) / 3
	if math.Abs(stats.Probability-want) > 1e-12 {
		t.Fatalf("confirmation p-hat=%g, want direct P/Q estimate %g", stats.Probability, want)
	}
}

func runCEMConfirmationFixture(t *testing.T, hitChunks [][]CEMSeason, budgetChunks int) (*CEMCandidateState, CEMConfirmationOutcome, CEMRoundResult, int64, int64) {
	t.Helper()
	const workPerSample int64 = 350
	state := &CEMCandidateState{
		Candidate:              &FrontierCandidate{TeamID: 7, Position: 4, SearchState: &PositionSearchState{}},
		Proposal:               CEMProposal{Iteration: 8, TeamLogMultipliers: map[int]float64{7: 0.3}, Means: []GameProposalMeans{{2, 1}}},
		ConfirmationProposal:   CEMProposal{Iteration: 7, TeamLogMultipliers: map[int]float64{7: 0.2}, Means: []GameProposalMeans{{1.5, 1}}},
		EverHadExactHit:        true,
		ExactHitPriorityActive: true,
	}
	confirmationBudget := int64(budgetChunks*CEMConfirmationChunkSamples) * workPerSample
	remainingWork := confirmationBudget
	result := CEMRoundResult{}
	chunkIndex := 0
	firstProposal := CEMProposal{}
	outcome := runCEMConfirmation(state, 99, &confirmationBudget, &remainingWork, workPerSample,
		func(proposal CEMProposal, samples int) []CEMSeason {
			if samples != CEMConfirmationChunkSamples {
				t.Fatalf("chunk samples=%d", samples)
			}
			if chunkIndex == 0 {
				firstProposal = cloneCEMProposal(proposal)
			} else if proposal.Iteration != firstProposal.Iteration ||
				!equalCEMTheta(proposal.TeamLogMultipliers, firstProposal.TeamLogMultipliers) ||
				proposal.Means[0] != firstProposal.Means[0] {
				t.Fatalf("frozen Q changed between chunks: first=%+v next=%+v", firstProposal, proposal)
			}
			seasons := make([]CEMSeason, CEMConfirmationChunkSamples)
			for i := range seasons {
				seasons[i].Rank = 9
			}
			if chunkIndex < len(hitChunks) {
				copy(seasons, hitChunks[chunkIndex])
			}
			chunkIndex++
			return seasons
		}, &result)
	if state.Proposal.Iteration != 8 || state.Proposal.TeamLogMultipliers[7] != 0.3 {
		t.Fatalf("confirmation mutated adaptation proposal: %+v", state.Proposal)
	}
	return state, outcome, result, confirmationBudget, remainingWork
}

func TestCEMSequentialConfirmationRunsThreeZeroHitChunks(t *testing.T) {
	state, outcome, result, remainingConfirmation, remainingGlobal := runCEMConfirmationFixture(t, nil, 3)
	if !outcome.Failed || outcome.Confirmed || state.Confirmation.Samples != 900 || state.Confirmation.Hits != 0 {
		t.Fatalf("0/900 should be statistical failure: outcome=%+v stats=%+v", outcome, state.Confirmation)
	}
	if state.StopReason != "confirmation_failed" || state.ExactHitPriorityActive || !state.EverHadExactHit {
		t.Fatalf("failed confirmation state lost history or retained active priority: %+v", state)
	}
	if result.ConfirmationAttempts != 1 || result.ConfirmationChunks != 3 || result.ConfirmationSamples != 900 ||
		result.ConfirmationWork != 315000 || remainingConfirmation != 0 || remainingGlobal != 0 {
		t.Fatalf("three-chunk accounting mismatch: result=%+v confirmRemaining=%d globalRemaining=%d", result, remainingConfirmation, remainingGlobal)
	}
}

func TestCEMSequentialConfirmationStopsOnFirstHitAtEachChunk(t *testing.T) {
	for hitAt := 1; hitAt <= 3; hitAt++ {
		t.Run(fmt.Sprintf("chunk_%d", hitAt), func(t *testing.T) {
			chunks := make([][]CEMSeason, hitAt)
			for i := range chunks {
				chunks[i] = make([]CEMSeason, CEMConfirmationChunkSamples)
				for j := range chunks[i] {
					chunks[i][j].Rank = 9
				}
			}
			chunks[hitAt-1][0] = CEMSeason{Rank: 4, LogWeight: math.Log(0.5)}
			state, outcome, result, _, _ := runCEMConfirmationFixture(t, chunks, 3)
			wantSamples := hitAt * CEMConfirmationChunkSamples
			if !outcome.Confirmed || state.Confirmation.Samples != wantSamples || state.Confirmation.Hits != 1 {
				t.Fatalf("hit at chunk %d not accepted promptly: outcome=%+v stats=%+v", hitAt, outcome, state.Confirmation)
			}
			if result.ConfirmationChunks != hitAt || result.ConfirmationWork != int64(wantSamples*350) {
				t.Fatalf("confirmation did not stop after successful chunk: result=%+v", result)
			}
			if state.Confirmation.EventESS != 1 || state.Confirmation.MaxEventWeightShare != 1 {
				t.Fatalf("single-hit diagnostics should be degenerate: %+v", state.Confirmation)
			}
		})
	}
}

func TestCEMSequentialConfirmationAccumulatesWeightedStatistics(t *testing.T) {
	chunks := make([][]CEMSeason, 2)
	for i := range chunks {
		chunks[i] = make([]CEMSeason, CEMConfirmationChunkSamples)
		for j := range chunks[i] {
			chunks[i][j].Rank = 9
		}
	}
	chunks[1][0] = CEMSeason{Rank: 4, LogWeight: math.Log(0.5)}
	chunks[1][1] = CEMSeason{Rank: 4, LogWeight: math.Log(0.75)}
	state, outcome, result, _, _ := runCEMConfirmationFixture(t, chunks, 3)
	wantP := 1.25 / 600
	if !outcome.Confirmed || state.Confirmation.Samples != 600 || state.Confirmation.Hits != 2 ||
		math.Abs(state.Confirmation.Probability-wantP) > 1e-12 || state.Confirmation.EventESS <= 1 ||
		math.Abs(state.Confirmation.MaxEventWeightShare-0.6) > 1e-12 {
		t.Fatalf("cumulative confirmation diagnostics are wrong: stats=%+v outcome=%+v", state.Confirmation, outcome)
	}
	if result.ConfirmationWork != 210000 {
		t.Fatalf("two chunks cost %d, want 210000", result.ConfirmationWork)
	}
}

func TestCEMSequentialConfirmationBudgetExhaustionIsInconclusive(t *testing.T) {
	state, outcome, result, remainingConfirmation, remainingGlobal := runCEMConfirmationFixture(t, nil, 1)
	if !outcome.Inconclusive || outcome.Failed || outcome.Confirmed || state.StopReason != "confirmation_budget_exhausted" {
		t.Fatalf("one zero chunk without budget for another must be inconclusive: outcome=%+v state=%+v", outcome, state)
	}
	if result.ConfirmationFailures != 0 || result.ConfirmationInconclusiveBudget != 1 ||
		result.ConfirmationWork != 105000 || remainingConfirmation != 0 || remainingGlobal != 0 {
		t.Fatalf("budget exhaustion was not accounted distinctly: result=%+v", result)
	}
}

func TestCEMBestProposalSnapshotSurvivesRegressionAndDeepCopies(t *testing.T) {
	proposal := CEMProposal{Iteration: 3, TeamLogMultipliers: map[int]float64{1: 0.2},
		Means: []GameProposalMeans{{Home: 2, Away: 1}}}
	saved := cloneCEMProposal(proposal)
	bestStats := CEMBatchStats{NearTargetRate: 0.08, EliteMeanDistance: 3}
	regressed := CEMBatchStats{NearTargetRate: 0.01, EliteMeanDistance: 4}
	if cemSnapshotBetter(regressed, CEMProposal{KL: 0.1}, bestStats, saved, true) {
		t.Fatal("regressed proposal replaced iteration 3 best snapshot")
	}
	proposal.TeamLogMultipliers[1] = -9
	proposal.Means[0].Home = 99
	if saved.TeamLogMultipliers[1] != 0.2 || saved.Means[0].Home != 2 || saved.Iteration != 3 {
		t.Fatalf("saved proposal was not deeply copied: %+v", saved)
	}
}

func TestCEMCandidateStopsAfterStallAndUsesHighSafetyCap(t *testing.T) {
	state := &CEMCandidateState{Active: true, Iterations: 3, StalledIterations: CEMMaxStalledIterations}
	if got := cemCandidateStopReason(state); got != "stalled" {
		t.Fatalf("stall stop reason=%q, want stalled", got)
	}
	state.StalledIterations = 0
	state.Iterations = CEMMaxIterationsPerCandidate
	if got := cemCandidateStopReason(state); got != "per_candidate_cap" {
		t.Fatalf("safety stop reason=%q, want per_candidate_cap", got)
	}
	if CEMMaxIterationsPerCandidate <= 4 {
		t.Fatalf("per-candidate safety cap %d must permit more than four batches", CEMMaxIterationsPerCandidate)
	}
}

func TestCEMStallRequiresTwoConsecutiveNonImprovingBatches(t *testing.T) {
	state := &CEMCandidateState{Iterations: 2, HasBest: true,
		PreviousStats:     CEMBatchStats{EliteMeanDistance: 5, NearTargetRate: 0},
		BestEliteDistance: 5, BestNearTargetRate: 0}
	noProgress := CEMBatchStats{EliteMeanDistance: 5, NearTargetRate: 0}
	updateCEMStallCounters(state, noProgress)
	if state.StalledIterations != 1 || cemCandidateStopReason(state) != "" {
		t.Fatalf("one stagnant batch should remain active: stalled=%d reason=%q", state.StalledIterations, cemCandidateStopReason(state))
	}
	state.Iterations++
	state.PreviousStats = noProgress
	updateCEMStallCounters(state, noProgress)
	if state.StalledIterations != 2 || cemCandidateStopReason(state) != "stalled" {
		t.Fatalf("two stagnant batches should stop: stalled=%d reason=%q", state.StalledIterations, cemCandidateStopReason(state))
	}
}

func TestCEMValidationUsesConfirmedFrozenSnapshot(t *testing.T) {
	state := &CEMCandidateState{Candidate: &FrontierCandidate{TeamID: 1, Position: 4},
		MaxExactHits: 1, ConfirmationAccepted: true,
		ConfirmationBatchStats: CEMBatchStats{ExactHits: 1, NearTargetRate: 0.07},
		ConfirmationProposal: CEMProposal{Iteration: 4, TeamLogMultipliers: map[int]float64{1: 0.4},
			Means: []GameProposalMeans{{Home: 1.5, Away: 1}}}}
	if !state.ConfirmationAccepted {
		t.Fatal("independently confirmed proposal should be eligible for validation")
	}
	if state.ConfirmationProposal.Iteration != 4 || state.ConfirmationProposal.Means[0].Home != 1.5 {
		t.Fatal("validation snapshot lost its frozen proposal state")
	}
	state.Proposal = CEMProposal{Means: []GameProposalMeans{{Home: 9, Away: 1}}}
	mixture := cemValidationMixture([]GameProposalMeans{{Home: 1, Away: 1}}, state)
	if mixture[1].Means[0].Home != 1.5 {
		t.Fatalf("validation mixture used current/regressed proposal instead of confirmed snapshot: %+v", mixture)
	}
}

func TestCEMFirstAdaptationHitFreezesSampledProposalAndPausesScheduler(t *testing.T) {
	state := &CEMCandidateState{Active: true}
	sampled := CEMProposal{Iteration: 3, TeamLogMultipliers: map[int]float64{1: 0.2}, Means: []GameProposalMeans{{Home: 2, Away: 1}}}
	updated := CEMProposal{Iteration: 4, TeamLogMultipliers: map[int]float64{1: 0.4}, Means: []GameProposalMeans{{Home: 3, Away: 1}}}
	stats := CEMBatchStats{ExactHits: 1, ExactRate: 1.0 / 300, EliteESS: 23.88}
	if !snapshotCEMConfirmationCandidate(state, sampled, stats, 4) {
		t.Fatal("one exact adaptation hit should trigger confirmation immediately")
	}
	state.Proposal = updated
	if state.ConfirmationProposal.Means[0].Home != 2 || state.ConfirmationSourceBatch != 4 {
		t.Fatalf("confirmation snapshot drifted to updated Q: %+v", state.ConfirmationProposal)
	}
	if state.Active || state.StopReason != "confirmation_ready" {
		t.Fatalf("exact hit did not immediately pause adaptation: active=%t reason=%q", state.Active, state.StopReason)
	}
	if selected := selectCEMCandidate([]*CEMCandidateState{state}, nil); selected != nil {
		t.Fatal("confirmation-ready candidate was selected for more adaptation")
	}
	state.HasConfirmationCandidate = false
	state.HasFailedConfirmationProposal = true
	state.LastFailedConfirmationProposal = cloneCEMProposal(sampled)
	if snapshotCEMConfirmationCandidate(state, sampled, stats, 5) {
		t.Fatal("unchanged failed proposal must not be confirmed again")
	}
	if !snapshotCEMConfirmationCandidate(state, updated, stats, 6) {
		t.Fatal("new sampled proposal with a new exact hit should be confirmable")
	}
	lowESSState := &CEMCandidateState{Active: true}
	if !snapshotCEMConfirmationCandidate(lowESSState, sampled,
		CEMBatchStats{ExactHits: 1, EliteESS: CEMMinEliteESSForUpdate - 1}, 7) {
		t.Fatal("low elite ESS must not erase a sampled proposal that produced an exact event")
	}
}

func TestCEMRoundConfirmsFirstExactBatchBeforeMoreAdaptation(t *testing.T) {
	base, games, _, table, groups, order := cemTestFixture()
	original := make([]GameProposalMeans, len(games)) // Zero-rate outcomes make the ranking reproducible.
	for i := range original {
		original[i] = GameProposalMeans{}
	}
	teamIDs := teamIDsFromGroups(groups)
	proposal := newCEMProposal(original, games, teamIDs)
	rank := simulateCEMBatchForTeams(base, games, original, proposal.Means, table, order,
		groups, teamIDs, 1, 1, rand.New(rand.NewSource(991)))[0].Rank
	positionStates := []*PositionSearchState{{Position: rank, Status: StatusFrontier}}
	candidate := &FrontierCandidate{TeamID: 1, Position: rank, Direction: RareBetter, SearchState: positionStates[0]}
	searches := map[int]*TeamRareSearch{1: {TeamID: 1, Positions: positionStates, NormalMeanRank: float64(rank)}}
	group := &GroupType{Id: 19, Games: games, Team_groups: groups}
	adaptCost := estimateSeasonWork(len(games), 1, len(groups))
	validationCost := estimateSeasonWork(len(games), 2, len(groups))
	cemBudget := int64(MaxCEMPlainEquivalentSamples) * adaptCost
	explorationBudget := int64(float64(cemBudget) * CEMMaxExplorationFraction)
	confirmationBudget := 1000 * adaptCost
	validationBudget := 1000 * validationCost
	totalLimit := int64(1_000_000) * adaptCost
	remaining := totalLimit
	result := runCEMRound([]*FrontierCandidate{candidate}, searches, group, base, table,
		order, original, &cemBudget, &confirmationBudget, &validationBudget, &remaining,
		&explorationBudget, adaptCost, adaptCost, validationCost, totalLimit,
		rand.New(rand.NewSource(991)))
	if result.AdaptationExactHitBatches < 1 || result.ConfirmationAttempts != 1 {
		t.Fatalf("first exact adaptation event was not promptly confirmed: %+v", result)
	}
	if result.SchedulerBatches != 1 || result.Candidates[0].ConfirmationSourceBatch != 1 {
		t.Fatalf("candidate adapted again before confirmation: batches=%d source=%d", result.SchedulerBatches, result.Candidates[0].ConfirmationSourceBatch)
	}
	if result.ValidationAttempts != 1 || result.ConfirmationSuccesses != 1 {
		t.Fatalf("independent confirmation did not gate mixture validation: attempts=%d successes=%d validation=%d",
			result.ConfirmationAttempts, result.ConfirmationSuccesses, result.ValidationAttempts)
	}
}

func TestCEMSchedulerTieBreakIsDeterministic(t *testing.T) {
	states, searches := cemSchedulerFixture(3)
	for _, state := range states {
		state.Active = true
		state.ProgressScore = 1
	}
	for i := 0; i < 5; i++ {
		if got := selectCEMCandidate(states, searches); got != states[0] {
			t.Fatalf("tie-break selected team %d, want team 1", got.Candidate.TeamID)
		}
	}
}

func TestCEMThetaSummaryReportsL1AndTruePerTeamAverage(t *testing.T) {
	averageL1, averageAbs := cemThetaSummary(1.2, 3, 12)
	maxAbsTeamTheta := 0.2
	if math.Abs(averageL1-0.4) > 1e-12 || math.Abs(averageAbs-0.1) > 1e-12 {
		t.Fatalf("theta summary averages L1=%g per-team=%g", averageL1, averageAbs)
	}
	if averageAbs > maxAbsTeamTheta {
		t.Fatalf("average abs team theta %g exceeds max %g", averageAbs, maxAbsTeamTheta)
	}
}
