package main

import (
	"math"
	"math/rand"
	"testing"
)

func TestPointStratumExactMassAndConditionalRank(t *testing.T) {
	table := NewTable([]uint32{1, 2})
	game := &GameType{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1.2, AwayPower: 1.0,
		home_table_index: table.Query(1), away_table_index: table.Query(2)}
	base := []*TeamCampaign{
		{id: 1, points: -1, points_win: 3, points_draw: 1, points_loss: 0},
		{id: 2, points: 0, points_win: 3, points_draw: 1, points_loss: 0},
	}
	teams := []TeamType{{Team_id: 1}, {Team_id: 2}}
	minimum, ok := minimumPointsForRank(1, 0, base)
	if !ok || minimum != 1 {
		t.Fatalf("minimum points for first place = %d, want 1", minimum)
	}
	universe, ok := pointStratumUniverse(1, base, table, []*GameType{game}, 10)
	if !ok {
		t.Fatal("expected a one-game outcome universe")
	}
	stratum, ok := makePointStratum(universe, 0, -math.MaxInt)
	if !ok || len(stratum.Patterns) != 1 || stratum.Patterns[0].Code != 2 || stratum.Patterns[0].AddedPoints != 3 {
		t.Fatalf("top rank should require a win: %+v", stratum)
	}
	want := targetOutcomeProbabilities(game)[2]
	if math.Abs(stratum.Mass-want) > 1e-12 {
		t.Fatalf("stratum mass %.15g, want win probability %.15g", stratum.Mass, want)
	}
	plain, outside := simulatePointHybridSeasons(base, []*GameType{game}, table, []SortType{PT, GD},
		teams, stratum, false, 2000, 17)
	if outside[0] != 0 || plain[[2]int{1, 0}] <= 0 {
		t.Fatalf("a top finish must be inside the win stratum: outside=%d plain=%d", outside[0], plain[[2]int{1, 0}])
	}
	conditional, _ := simulatePointHybridSeasons(base, []*GameType{game}, table, []SortType{PT, GD},
		teams, stratum, true, 2000, 23)
	if conditional[[2]int{1, 0}] != 2000 {
		t.Fatalf("conditional wins should always finish first: %d/2000", conditional[[2]int{1, 0}])
	}
	// P(rank 1) = P(A)*P(rank 1|A) + P(rank 1,A^c) = P(win).
	combined := stratum.Mass*float64(conditional[[2]int{1, 0}])/2000 + float64(outside[0])/2000
	if math.Abs(combined-want) > 1e-12 {
		t.Fatalf("partition identity failed: got %.15g, want %.15g", combined, want)
	}
}

func TestPointTailDynamicProgramMatchesPatternEnumeration(t *testing.T) {
	base, games, _, _, table, _ := pruningFixture(t)
	universe, ok := pointStratumUniverse(1, base, table, games, 10)
	if !ok {
		t.Fatal("expected an outcome universe")
	}
	tail, ok := makePointTailStratum(universe, 4)
	if !ok {
		t.Fatal("expected a nonempty points tail")
	}
	want := 0.0
	for _, pattern := range universe.Patterns {
		if pattern.AddedPoints >= 4 {
			want += pattern.Probability
		}
	}
	if math.Abs(tail.Mass-want) > 1e-12 {
		t.Fatalf("DP mass %.15g, enumerated mass %.15g", tail.Mass, want)
	}
	rng := rand.New(rand.NewSource(87))
	for i := 0; i < 10000; i++ {
		if code := tail.samplePattern(rng); !tail.contains(code) {
			t.Fatalf("conditional pattern %d did not meet the threshold", code)
		}
	}
}

func TestPointHybridPartitionWithInsideAndOutsideMass(t *testing.T) {
	table := NewTable([]uint32{1, 2})
	base := []*TeamCampaign{
		{id: 1, points: -1, points_win: 3, points_draw: 1, points_loss: 0},
		{id: 2, points: 0, points_win: 3, points_draw: 1, points_loss: 0},
	}
	games := []*GameType{
		{Id: 1, HomeId: 1, AwayId: 2, HomePower: 1.3, AwayPower: .9, home_table_index: table.Query(1), away_table_index: table.Query(2)},
		{Id: 2, HomeId: 2, AwayId: 1, HomePower: 1.1, AwayPower: 1.0, home_table_index: table.Query(2), away_table_index: table.Query(1)},
	}
	universe, ok := pointOutcomeUniverse(1, base, table, games, 10)
	if !ok {
		t.Fatal("expected a two-game outcome universe")
	}
	stratum, ok := makePointTailStratum(universe, 6)
	if !ok {
		t.Fatal("expected a two-win stratum")
	}
	teams := []TeamType{{Team_id: 1}, {Team_id: 2}}
	plainN, conditionalN := 200000, 20000
	_, outside := simulatePointHybridSeasons(base, games, table, []SortType{PT, GD}, teams, stratum, false, plainN, 101)
	conditional, _ := simulatePointHybridSeasons(base, games, table, []SortType{PT, GD}, teams, stratum, true, conditionalN, 202)
	if conditional[[2]int{1, 0}] != conditionalN {
		t.Fatalf("two wins should always finish first: %d/%d", conditional[[2]int{1, 0}], conditionalN)
	}
	first, second := universe.OutcomeProbabilities[0], universe.OutcomeProbabilities[1]
	want := first[2]*second[2] + first[2]*second[1] + first[1]*second[2]
	got := float64(outside[0])/float64(plainN) + stratum.Mass*float64(conditional[[2]int{1, 0}])/float64(conditionalN)
	se := math.Sqrt(want * (1 - want) / float64(plainN))
	if math.Abs(got-want) > 5*se {
		t.Fatalf("hybrid partition %.6f differs from exact W/D/L probability %.6f (SE %.6f)", got, want, se)
	}
}
