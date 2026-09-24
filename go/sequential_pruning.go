package main

// This file contains the deliberately self-contained kernel used by the
// target-aware ordering experiment.  It does not choose or train a proposal:
// callers freeze the means before constructing an Experiment.

import (
	"encoding/json"
	"fmt"
	"io"
	"math"
	"math/rand"
	"sort"
	"strconv"
	"strings"
	"time"
)

const DefaultSequentialPruningCalibrationSamples = 1000

type GameOrdering string

const (
	OrderOriginal           GameOrdering = "original_prune"
	OrderTargetFirst        GameOrdering = "target_first"
	OrderRelevance          GameOrdering = "relevance"
	OrderTargetThenBoundary GameOrdering = "target_boundary"
	OrderSolverImpact       GameOrdering = "solver_impact"
)

var experimentOrderings = []GameOrdering{
	OrderOriginal, OrderTargetFirst, OrderRelevance, OrderTargetThenBoundary, OrderSolverImpact,
}

// SequentialFixture keeps P and Q parameters on the fixture.  OriginalIndex
// is the stable input order and ID is the final ordering/RNG tie breaker.
type SequentialFixture struct {
	ID, HomeID, AwayID         int
	OriginalIndex              int
	OriginalHome, OriginalAway float64
	ProposalHome, ProposalAway float64
}

type OrderingContext struct {
	TargetTeam, TargetPosition int // positions are zero based
	CurrentPoints              map[int]int
	NormalMeanRank             map[int]float64
	Remaining                  []SequentialFixture
}

type BuiltOrdering struct {
	Name         GameOrdering
	Games        []SequentialFixture
	BuildSeconds float64
}

func BuildGameOrdering(name GameOrdering, c OrderingContext) BuiltOrdering {
	started := time.Now()
	games := append([]SequentialFixture(nil), c.Remaining...)
	remainingByTeam := make(map[int]int)
	for _, g := range games {
		remainingByTeam[g.HomeID]++
		remainingByTeam[g.AwayID]++
	}
	targetMin := c.CurrentPoints[c.TargetTeam]
	targetMax := targetMin + 3*remainingByTeam[c.TargetTeam]
	overlapsTarget := func(team int) bool {
		lo, hi := c.CurrentPoints[team], c.CurrentPoints[team]+3*remainingByTeam[team]
		return lo <= targetMax && targetMin <= hi
	}
	rankRelevance := func(team int) float64 {
		r, ok := c.NormalMeanRank[team]
		if !ok {
			return 0
		}
		return 1 / (1 + math.Abs(r-float64(c.TargetPosition)))
	}
	score := func(g SequentialFixture) float64 {
		s := rankRelevance(g.HomeID) + rankRelevance(g.AwayID)
		if overlapsTarget(g.HomeID) {
			s++
		}
		if overlapsTarget(g.AwayID) {
			s++
		}
		if (c.NormalMeanRank[g.HomeID] < float64(c.TargetPosition)) != (c.NormalMeanRank[g.AwayID] < float64(c.TargetPosition)) {
			s += 2
		}
		if g.HomeID == c.TargetTeam || g.AwayID == c.TargetTeam {
			s += 100
		}
		return s
	}
	inCorridor := func(team int) bool {
		r := c.NormalMeanRank[team]
		return r >= float64(c.TargetPosition-3) && r <= float64(c.TargetPosition+3)
	}
	impact := func(team int) float64 {
		lo, hi := c.CurrentPoints[team], c.CurrentPoints[team]+3*remainingByTeam[team]
		v := 0.0
		// Membership in the conservative above/below candidate sets is the
		// cheap proxy for how much resolving a fixture can tighten the bound.
		if hi >= targetMin {
			v++
		}
		if lo <= targetMax {
			v++
		}
		if overlapsTarget(team) {
			v += 2
		}
		return v
	}
	sort.SliceStable(games, func(i, j int) bool {
		a, b := games[i], games[j]
		priority := func(g SequentialFixture) float64 {
			target := g.HomeID == c.TargetTeam || g.AwayID == c.TargetTeam
			switch name {
			case OrderTargetFirst:
				if target {
					return 1
				}
				return 0
			case OrderTargetThenBoundary:
				if target {
					return 2
				}
				if inCorridor(g.HomeID) || inCorridor(g.AwayID) {
					return 1
				}
				return 0
			case OrderRelevance:
				return score(g)
			case OrderSolverImpact:
				v := impact(g.HomeID) + impact(g.AwayID)
				if target {
					v += 100
				}
				return v
			default:
				return 0
			}
		}
		pa, pb := priority(a), priority(b)
		if pa != pb {
			return pa > pb
		}
		if a.OriginalIndex != b.OriginalIndex && (name == OrderOriginal || name == OrderTargetFirst || name == OrderTargetThenBoundary) {
			return a.OriginalIndex < b.OriginalIndex
		}
		return a.ID < b.ID
	})
	return BuiltOrdering{Name: name, Games: games, BuildSeconds: time.Since(started).Seconds()}
}

func splitMix64(x uint64) uint64 {
	x += 0x9e3779b97f4a7c15
	x = (x ^ (x >> 30)) * 0xbf58476d1ce4e5b9
	x = (x ^ (x >> 27)) * 0x94d049bb133111eb
	return x ^ (x >> 31)
}
func sampleSeed(master uint64, sample int64) uint64 { return splitMix64(master ^ uint64(sample)) }
func componentSeed(seed uint64) int64               { return int64(splitMix64(seed ^ 0x434f4d504f4e454e)) }
func fixtureSeed(seed uint64, id int) int64         { return int64(splitMix64(seed ^ splitMix64(uint64(id)))) }

func poissonWith(r *rand.Rand, mean float64) int {
	if mean <= 0 {
		return 0
	}
	t, n := 0.0, 0
	for {
		t += r.ExpFloat64()
		if t >= mean {
			return n
		}
		n++
	}
}

type SimulatedFixture struct{ ID, Home, Away int }
type SequentialSample struct {
	SampleIndex       int64
	ProposalComponent bool
	Scores            map[int]SimulatedFixture
	Points            map[int]int
	Rank              int
	Weight            float64
	GamesSimulated    int
	Pruned            bool
	SolverChecks      int
	SolverSeconds     float64
}

type SequentialExperiment struct {
	MasterSeed          uint64
	Context             OrderingContext
	ProposalProbability float64
	Stride              int
}

func poissonLogPMF(k int, mean float64) float64 {
	if mean == 0 {
		if k == 0 {
			return 0
		}
		return math.Inf(-1)
	}
	l, _ := math.Lgamma(float64(k) + 1)
	return float64(k)*math.Log(mean) - mean - l
}

func (e SequentialExperiment) RunSample(sample int64, order BuiltOrdering, pruning bool) SequentialSample {
	seed := sampleSeed(e.MasterSeed, sample)
	component := rand.New(rand.NewSource(componentSeed(seed))).Float64() < e.ProposalProbability
	points := make(map[int]int, len(e.Context.CurrentPoints))
	for k, v := range e.Context.CurrentPoints {
		points[k] = v
	}
	remaining := make(map[int]int)
	for _, g := range order.Games {
		remaining[g.HomeID]++
		remaining[g.AwayID]++
	}
	r := SequentialSample{SampleIndex: sample, ProposalComponent: component, Scores: make(map[int]SimulatedFixture), Points: points, Weight: 1}
	logP := make(map[int]float64, len(order.Games))
	logQ := make(map[int]float64, len(order.Games))
	stride := e.Stride
	if stride < 1 {
		stride = 1
	}
	for _, g := range order.Games {
		rng := rand.New(rand.NewSource(fixtureSeed(seed, g.ID)))
		hm, am := g.OriginalHome, g.OriginalAway
		if component {
			hm, am = g.ProposalHome, g.ProposalAway
		}
		h, a := poissonWith(rng, hm), poissonWith(rng, am)
		r.Scores[g.ID] = SimulatedFixture{g.ID, h, a}
		r.GamesSimulated++
		remaining[g.HomeID]--
		remaining[g.AwayID]--
		if h > a {
			points[g.HomeID] += 3
		} else if h < a {
			points[g.AwayID] += 3
		} else {
			points[g.HomeID]++
			points[g.AwayID]++
		}
		logP[g.ID] = poissonLogPMF(h, g.OriginalHome) + poissonLogPMF(a, g.OriginalAway)
		logQ[g.ID] = poissonLogPMF(h, g.ProposalHome) + poissonLogPMF(a, g.ProposalAway)
		if pruning && r.GamesSimulated%stride == 0 {
			started := time.Now()
			r.SolverChecks++
			best, worst := reachableRank(points, remaining, e.Context.TargetTeam)
			r.SolverSeconds += time.Since(started).Seconds()
			if e.Context.TargetPosition < best || e.Context.TargetPosition > worst {
				r.Pruned = true
				break
			}
		}
	}
	if !r.Pruned {
		r.Rank = finalPointsRank(points, e.Context.TargetTeam)
		// Sum in fixture-ID order so the last floating-point bit is independent
		// of the chosen simulation ordering.
		ids := make([]int, 0, len(logP))
		for id := range logP {
			ids = append(ids, id)
		}
		sort.Ints(ids)
		lp, lq := 0.0, 0.0
		for _, id := range ids {
			lp += logP[id]
			lq += logQ[id]
		}
		// The component draw is once per season, therefore the denominator is
		// the season-level mixture, not a product of per-game mixtures.
		mix := logAdd(math.Log1p(-e.ProposalProbability)+lp, math.Log(e.ProposalProbability)+lq)
		r.Weight = math.Exp(lp - mix)
	} else {
		r.Rank = -1
		r.Weight = 0
	}
	return r
}

func logAdd(a, b float64) float64 {
	if a < b {
		a, b = b, a
	}
	return a + math.Log1p(math.Exp(b-a))
}

func reachableRank(points, remaining map[int]int, target int) (int, int) {
	tMin, tMax := points[target], points[target]+3*remaining[target]
	definitely, potentially := 0, 0
	for team, p := range points {
		if team == target {
			continue
		}
		lo, hi := p, p+3*remaining[team]
		if lo > tMax {
			definitely++
		}
		if hi >= tMin {
			potentially++
		}
	}
	return definitely, potentially
}
func finalPointsRank(points map[int]int, target int) int {
	rank := 0
	for team, p := range points {
		if p > points[target] || (p == points[target] && team < target) {
			rank++
		}
	}
	return rank
}

type OrderingMetrics struct {
	Target                                                                                       string       `json:"target"`
	Ordering                                                                                     GameOrdering `json:"ordering"`
	Stride                                                                                       int          `json:"stride"`
	Samples                                                                                      int          `json:"samples"`
	SamplesPerSecond, AverageGames, MedianGames                                                  float64
	PrunePercent                                                                                 float64            `json:"prune_percent"`
	PruneQuantiles                                                                               map[string]float64 `json:"prune_game_index"`
	PruningByQuartile                                                                            [4]int             `json:"pruning_by_season_quartile"`
	SolverChecksPerSample, SolverSecondsPerSample, SolverFraction, OrderingBuildSeconds, Speedup float64
	TargetTeamGames                                                                              int `json:"target_team_games"`
}

// ProductionPlan is frozen after calibration.  Calibration time is reported
// but never deducted from either statistical stream's requested horizon.
type ProductionPlan struct {
	Ordering            GameOrdering `json:"ordering"`
	Stride              int          `json:"stride"`
	BaselineSecondsEach float64      `json:"baseline_seconds_per_sample"`
	PrunedSecondsEach   float64      `json:"pruned_seconds_per_sample"`
	AllRankSamples      int          `json:"n_all_rank"`
	TargetSamples       int          `json:"n_target"`
	PruningEnabled      bool         `json:"pruning_enabled"`
	DisableReason       string       `json:"disable_reason,omitempty"`
	CalibrationWallTime float64      `json:"calibration_wall_seconds"`
}

// FreezeProductionPlan performs runtime-only selection. Event hits, weights,
// probability and ESS are intentionally not accepted as inputs.
func FreezeProductionPlan(metrics []OrderingMetrics, allRankSamples int, targetRuntimeSeconds, calibrationWallSeconds float64) ProductionPlan {
	p := ProductionPlan{AllRankSamples: allRankSamples, CalibrationWallTime: calibrationWallSeconds}
	if len(metrics) == 0 || metrics[0].SamplesPerSecond <= 0 {
		p.DisableReason = "missing full-season runtime calibration"
		return p
	}
	p.BaselineSecondsEach = 1 / metrics[0].SamplesPerSecond
	best, enabled := BestRuntimeOrdering(metrics)
	if !enabled {
		p.DisableReason = "no sequential ordering beat the target-only full-season baseline"
		return p
	}
	p.PruningEnabled, p.Ordering, p.Stride = true, best.Ordering, best.Stride
	p.PrunedSecondsEach = 1 / best.SamplesPerSecond
	p.TargetSamples = int(math.Floor(targetRuntimeSeconds / p.PrunedSecondsEach))
	return p
}

type SufficientStatistics struct {
	N           int `json:"n"`
	SumY, SumY2 float64
	Hits        int `json:"hits"`
}

func (s SufficientStatistics) Add(o SufficientStatistics) SufficientStatistics {
	return SufficientStatistics{N: s.N + o.N, SumY: s.SumY + o.SumY, SumY2: s.SumY2 + o.SumY2, Hits: s.Hits + o.Hits}
}
func (s SufficientStatistics) Probability() float64 {
	if s.N == 0 {
		return 0
	}
	return s.SumY / float64(s.N)
}
func (s SufficientStatistics) ESS() float64 {
	if s.SumY2 == 0 {
		return 0
	}
	return s.SumY * s.SumY / s.SumY2
}

type ProductionReport struct {
	Plan                      ProductionPlan `json:"plan"`
	AllRank, Target, Combined SufficientStatistics
	EstimatorSeconds          float64 `json:"estimator_seconds"`
	EndToEndSeconds           float64 `json:"end_to_end_seconds"`
}

func NewProductionReport(plan ProductionPlan, allRank, target SufficientStatistics, estimatorSeconds float64) ProductionReport {
	return ProductionReport{Plan: plan, AllRank: allRank, Target: target, Combined: allRank.Add(target), EstimatorSeconds: estimatorSeconds, EndToEndSeconds: estimatorSeconds + plan.CalibrationWallTime}
}

func percentile(sorted []int, p float64) float64 {
	if len(sorted) == 0 {
		return 0
	}
	x := p * float64(len(sorted)-1)
	lo := int(x)
	hi := int(math.Ceil(x))
	if lo == hi {
		return float64(sorted[lo])
	}
	return float64(sorted[lo]) + (x-float64(lo))*float64(sorted[hi]-sorted[lo])
}

// Calibrate runs identical sample indices for every ordering and validates all
// pruning decisions against the latent full season before considering timing.
func (e SequentialExperiment) Calibrate(samples, repetitions int) ([]OrderingMetrics, error) {
	if samples <= 0 {
		samples = DefaultSequentialPruningCalibrationSamples
	}
	if repetitions <= 0 {
		repetitions = 3
	}
	full := BuildGameOrdering(OrderOriginal, e.Context)
	truth := make([]SequentialSample, samples)
	for i := range truth {
		truth[i] = e.RunSample(int64(i), full, false)
	}
	baselineDurations := make([]float64, repetitions)
	for rep := 0; rep < repetitions; rep++ {
		s := time.Now()
		for i := 0; i < samples; i++ {
			_ = e.RunSample(int64(i), full, false)
		}
		baselineDurations[rep] = time.Since(s).Seconds()
	}
	baseRate := float64(samples) / medianFloat(baselineDurations)
	metrics := make([]OrderingMetrics, 0, len(experimentOrderings)+1)
	metrics = append(metrics, OrderingMetrics{Target: targetLabel(e.Context), Ordering: "full", SamplesPerSecond: baseRate, AverageGames: float64(len(full.Games)), MedianGames: float64(len(full.Games)), Speedup: 1, OrderingBuildSeconds: full.BuildSeconds})
	metrics[0].Samples = samples
	// Alternate the strategy traversal between repetitions to reduce cache bias.
	for _, name := range experimentOrderings {
		built := BuildGameOrdering(name, e.Context)
		durations := make([]float64, repetitions)
		var results []SequentialSample
		for rep := 0; rep < repetitions; rep++ {
			s := time.Now()
			current := make([]SequentialSample, samples)
			for k := 0; k < samples; k++ {
				i := k
				if rep%2 == 1 {
					i = samples - 1 - k
				}
				current[i] = e.RunSample(int64(i), built, true)
			}
			durations[rep] = time.Since(s).Seconds()
			results = current
		}
		for i, r := range results {
			if r.Pruned && truth[i].Rank == e.Context.TargetPosition {
				return nil, fmt.Errorf("unsafe prune: ordering=%s sample=%d", name, i)
			}
			if !r.Pruned && (r.Rank != truth[i].Rank || math.Float64bits(r.Weight) != math.Float64bits(truth[i].Weight)) {
				return nil, fmt.Errorf("paired invariant failed: ordering=%s sample=%d", name, i)
			}
		}
		metrics = append(metrics, summarizeOrdering(e, built, results, medianFloat(durations), baseRate))
	}
	return metrics, nil
}

func summarizeOrdering(e SequentialExperiment, b BuiltOrdering, rs []SequentialSample, duration, baseRate float64) OrderingMetrics {
	games, prunes, checks, solver := 0, []int{}, 0, 0.0
	targetGames := 0
	for _, g := range b.Games {
		if g.HomeID == e.Context.TargetTeam || g.AwayID == e.Context.TargetTeam {
			targetGames++
		}
	}
	for _, r := range rs {
		games += r.GamesSimulated
		checks += r.SolverChecks
		solver += r.SolverSeconds
		if r.Pruned {
			prunes = append(prunes, r.GamesSimulated)
		}
	}
	sort.Ints(prunes)
	m := OrderingMetrics{Target: targetLabel(e.Context), Ordering: b.Name, Stride: e.Stride, SamplesPerSecond: float64(len(rs)) / duration, AverageGames: float64(games) / float64(len(rs)), SolverChecksPerSample: float64(checks) / float64(len(rs)), SolverSecondsPerSample: solver / float64(len(rs)), SolverFraction: solver / duration, OrderingBuildSeconds: b.BuildSeconds, TargetTeamGames: targetGames}
	m.Samples = len(rs)
	m.Speedup = m.SamplesPerSecond / baseRate
	m.PrunePercent = 100 * float64(len(prunes)) / float64(len(rs))
	all := append([]int(nil), prunes...)
	if len(all) == 0 {
		for _, r := range rs {
			all = append(all, r.GamesSimulated)
		}
		sort.Ints(all)
	}
	m.MedianGames = percentile(all, .5)
	m.PruneQuantiles = map[string]float64{"p10": percentile(prunes, .1), "p25": percentile(prunes, .25), "p50": percentile(prunes, .5), "p75": percentile(prunes, .75), "p90": percentile(prunes, .9)}
	for _, v := range prunes {
		q := v * 4 / (len(b.Games) + 1)
		if q > 3 {
			q = 3
		}
		m.PruningByQuartile[q]++
	}
	return m
}
func medianFloat(v []float64) float64 {
	c := append([]float64(nil), v...)
	sort.Float64s(c)
	return percentileFloat(c, .5)
}
func percentileFloat(v []float64, p float64) float64 {
	if len(v) == 0 {
		return 0
	}
	x := p * float64(len(v)-1)
	lo := int(x)
	hi := int(math.Ceil(x))
	return v[lo] + (x-float64(lo))*(v[hi]-v[lo])
}
func targetLabel(c OrderingContext) string {
	return strconv.Itoa(c.TargetTeam) + "/" + strconv.Itoa(c.TargetPosition)
}

func BestRuntimeOrdering(ms []OrderingMetrics) (OrderingMetrics, bool) {
	if len(ms) == 0 {
		return OrderingMetrics{}, false
	}
	baseline := ms[0]
	best := OrderingMetrics{}
	for _, m := range ms[1:] {
		if m.SamplesPerSecond > best.SamplesPerSecond {
			best = m
		}
	}
	return best, best.SamplesPerSecond > baseline.SamplesPerSecond
}

func WriteExperimentReport(w io.Writer, ms []OrderingMetrics) error {
	fmt.Fprintln(w, "target  ordering              stride  prune%  avg_games  p50_prune  solver%  samples/s  speedup")
	for _, m := range ms {
		stride := "-"
		if m.Ordering != "full" {
			stride = strconv.Itoa(m.Stride)
		}
		p50 := "-"
		if m.PrunePercent > 0 {
			p50 = fmt.Sprintf("%.0f", m.PruneQuantiles["p50"])
		}
		fmt.Fprintf(w, "%-7s %-21s %6s %7.1f %10.1f %10s %8.1f %10.0f %8.2f\n", m.Target, m.Ordering, stride, m.PrunePercent, m.AverageGames, p50, 100*m.SolverFraction, m.SamplesPerSecond, m.Speedup)
	}
	payload := struct {
		CalibrationSamples int               `json:"calibration_samples"`
		Results            []OrderingMetrics `json:"results"`
	}{0, ms}
	if len(ms) > 0 {
		payload.CalibrationSamples = ms[0].Samples
	}
	data, err := json.Marshal(payload)
	if err != nil {
		return err
	}
	fmt.Fprintf(w, "EXPERIMENT_JSON=%s\n", data)
	return nil
}

func SequentialCalibrationSamples(getenv func(string) string) int {
	v := strings.TrimSpace(getenv("RARE_POSITION_SEQUENTIAL_PRUNING_CALIBRATION_SAMPLES"))
	if v == "" {
		return DefaultSequentialPruningCalibrationSamples
	}
	n, err := strconv.Atoi(v)
	if err != nil || n < 1000 {
		return DefaultSequentialPruningCalibrationSamples
	}
	return n
}
