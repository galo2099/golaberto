package main

import (
	"math"
	"os"
	"reflect"
	"testing"
)

func testExperiment() SequentialExperiment {
	games := []SequentialFixture{{10, 1, 2, 0, 1.2, .8, 2.1, .4}, {20, 3, 4, 1, .7, 1.3, .5, 1.8}, {30, 1, 3, 2, 1.5, 1.1, 2.0, .9}, {40, 2, 4, 3, .9, .9, .6, 1.4}}
	return SequentialExperiment{MasterSeed: 42, ProposalProbability: .95, Stride: 1, Context: OrderingContext{TargetTeam: 1, TargetPosition: 1, CurrentPoints: map[int]int{1: 3, 2: 3, 3: 3, 4: 3}, NormalMeanRank: map[int]float64{1: 2, 2: 0, 3: 1, 4: 3}, Remaining: games}}
}

func TestOrderingPreservesCompletedLatentSeason(t *testing.T) {
	e := testExperiment()
	original := BuildGameOrdering(OrderOriginal, e.Context)
	for sample := int64(0); sample < 100; sample++ {
		want := e.RunSample(sample, original, false)
		for _, name := range experimentOrderings {
			got := e.RunSample(sample, BuildGameOrdering(name, e.Context), false)
			if got.ProposalComponent != want.ProposalComponent || !reflect.DeepEqual(got.Scores, want.Scores) || !reflect.DeepEqual(got.Points, want.Points) || got.Rank != want.Rank || math.Float64bits(got.Weight) != math.Float64bits(want.Weight) {
				t.Fatalf("ordering %s changed sample %d: %#v != %#v", name, sample, got, want)
			}
		}
	}
}

func TestOrderingDoesNotDuplicateFixtures(t *testing.T) {
	e := testExperiment()
	for _, name := range experimentOrderings {
		b := BuildGameOrdering(name, e.Context)
		seen := map[int]bool{}
		for _, g := range b.Games {
			if seen[g.ID] {
				t.Fatalf("%s duplicated %d", name, g.ID)
			}
			seen[g.ID] = true
		}
		if len(seen) != len(e.Context.Remaining) {
			t.Fatalf("%s lost fixture", name)
		}
	}
}

func TestCalibrationRejectsNoTrueEvents(t *testing.T) {
	e := testExperiment()
	metrics, err := e.Calibrate(1000, 1)
	if err != nil {
		t.Fatal(err)
	}
	if len(metrics) != len(experimentOrderings)+1 {
		t.Fatalf("got %d metrics", len(metrics))
	}
}

func TestCalibrationSampleEnvironment(t *testing.T) {
	t.Setenv("RARE_POSITION_SEQUENTIAL_PRUNING_CALIBRATION_SAMPLES", "2000")
	if got := SequentialCalibrationSamples(os.Getenv); got != 2000 {
		t.Fatal(got)
	}
	t.Setenv("RARE_POSITION_SEQUENTIAL_PRUNING_CALIBRATION_SAMPLES", "500")
	if got := SequentialCalibrationSamples(os.Getenv); got != 1000 {
		t.Fatal(got)
	}
}

func TestProductionPlanUsesOnlyRuntimeAndPoolsExactStatistics(t *testing.T) {
	metrics := []OrderingMetrics{{Ordering: "full", SamplesPerSecond: 100}, {Ordering: OrderTargetFirst, Stride: 8, SamplesPerSecond: 200}, {Ordering: OrderRelevance, Stride: 8, SamplesPerSecond: 150}}
	p := FreezeProductionPlan(metrics, 50, 2, 3)
	if !p.PruningEnabled || p.Ordering != OrderTargetFirst || p.TargetSamples != 400 {
		t.Fatalf("bad frozen plan: %#v", p)
	}
	r := NewProductionReport(p, SufficientStatistics{N: 50, SumY: 2, SumY2: 1, Hits: 2}, SufficientStatistics{N: 400, SumY: 3, SumY2: 2, Hits: 3}, 5)
	if r.Combined.N != 450 || r.Combined.SumY != 5 || r.Combined.SumY2 != 3 || r.Combined.Hits != 5 || r.EndToEndSeconds != 8 {
		t.Fatalf("bad pooling: %#v", r)
	}
}

func TestSlowerPruningIsDisabled(t *testing.T) {
	p := FreezeProductionPlan([]OrderingMetrics{{Ordering: "full", SamplesPerSecond: 100}, {Ordering: OrderTargetFirst, SamplesPerSecond: 99}}, 10, 1, .5)
	if p.PruningEnabled || p.TargetSamples != 0 || p.DisableReason == "" {
		t.Fatalf("bad guard: %#v", p)
	}
}
