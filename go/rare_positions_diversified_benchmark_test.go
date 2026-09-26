package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"math"
	"math/rand"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"testing"
)

// Offline only. References are written to disk and are never passed to the
// estimator. Set RARE_POSITION_BENCHMARK_FIXTURES to comma-separated request
// JSON paths and RARE_POSITION_BENCHMARK_OUTPUT to a results directory.
type diversifiedReference struct {
	GroupID     int                  `json:"group"`
	InputSHA256 string               `json:"input_sha256"`
	Samples     int                  `json:"samples"`
	Seed        int64                `json:"seed"`
	Cells       []diversifiedRefCell `json:"cells"`
}

type diversifiedRefCell struct {
	Team     int     `json:"team"`
	Position int     `json:"position"`
	Count    int     `json:"count"`
	P        float64 `json:"p"`
}

type diversifiedBenchWork struct {
	Scout      int64 `json:"scout"`
	Discovery  int64 `json:"discovery"`
	Validation int64 `json:"validation"`
	Production int64 `json:"production"`
	Total      int64 `json:"total"`
	Limit      int64 `json:"limit"`
}

type diversifiedBenchQuality struct {
	CoverageScore float64 `json:"coverage_score"`
	Feasible      int     `json:"feasible_cells"`
	Nonzero       int     `json:"nonzero"`
	ESS4          int     `json:"ess_ge_4"`
	ESS10         int     `json:"ess_ge_10"`
	ESS25         int     `json:"ess_ge_25"`
	MedianRelSE   float64 `json:"median_rel_se"`
	P90RelSE      float64 `json:"p90_rel_se"`
}

type diversifiedBandQuality struct {
	Cells         int     `json:"cells"`
	Reliable      int     `json:"reference_count_ge_25"`
	CoverageScore float64 `json:"coverage_score"`
	ESS4          int     `json:"ess_ge_4"`
	ESS10         int     `json:"ess_ge_10"`
	ESS25         int     `json:"ess_ge_25"`
	MeanVariance  float64 `json:"mean_estimated_variance"`
	MeanRelVar    float64 `json:"mean_estimated_relative_variance_reliable"`
	RMSE          float64 `json:"rmse"`
	MeanAbsError  float64 `json:"mean_absolute_error"`
	MeanZ         float64 `json:"mean_z"`
	ZVariance     float64 `json:"z_variance"`
	Coverage95    float64 `json:"coverage_95"`
}

type diversifiedBenchResult struct {
	Commit           string                            `json:"commit"`
	Configuration    map[string]string                 `json:"configuration"`
	Group            int                               `json:"group"`
	Seed             int64                             `json:"seed"`
	Method           string                            `json:"method"`
	ReferenceSamples int                               `json:"reference_samples"`
	Work             diversifiedBenchWork              `json:"work"`
	Discovery        map[string]int                    `json:"discovery"`
	Shortlist        []DiversifiedProposal             `json:"shortlist,omitempty"`
	Validation       map[string]int                    `json:"validation"`
	ValidationCells  []DiversifiedValidationCell       `json:"validation_cells,omitempty"`
	Production       map[string]diversifiedBenchBatch  `json:"production"`
	Quality          diversifiedBenchQuality           `json:"quality"`
	TruthByBand      map[string]diversifiedBandQuality `json:"truth_by_band"`
	Cells            []diversifiedBenchCell            `json:"cells"`
}

type diversifiedBenchCell struct {
	Team           int     `json:"team"`
	Position       int     `json:"position"`
	Probability    float64 `json:"probability"`
	StdErr         float64 `json:"std_err"`
	ESS            float64 `json:"ess"`
	ReferenceP     float64 `json:"reference_probability"`
	ReferenceCount int     `json:"reference_count"`
}

type diversifiedBenchBatch struct {
	Samples int   `json:"samples"`
	Work    int64 `json:"work"`
}

func diversifiedBenchmarkPaths(t *testing.T) ([]string, string) {
	t.Helper()
	raw := os.Getenv("RARE_POSITION_BENCHMARK_FIXTURES")
	out := os.Getenv("RARE_POSITION_BENCHMARK_OUTPUT")
	if raw == "" || out == "" {
		t.Skip("set RARE_POSITION_BENCHMARK_FIXTURES and RARE_POSITION_BENCHMARK_OUTPUT")
	}
	paths := strings.Split(raw, ",")
	for i := range paths {
		paths[i] = strings.TrimSpace(paths[i])
		if paths[i] == "" {
			t.Fatal("empty benchmark fixture path")
		}
	}
	if err := os.MkdirAll(out, 0755); err != nil {
		t.Fatal(err)
	}
	return paths, out
}

func diversifiedBenchmarkInput(t *testing.T, path string) (GroupType, string) {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var group GroupType
	if err := json.Unmarshal(data, &group); err != nil {
		t.Fatal(err)
	}
	if group.Phase == nil || group.Phase.Championship == nil || len(group.Team_groups) < 2 {
		t.Fatalf("fixture %s lacks phase, scoring rules, or teams", path)
	}
	return group, fmt.Sprintf("%x", sha256.Sum256(data))
}

func diversifiedBenchmarkSetup(group *GroupType) ([]*TeamCampaign, *Table, []SortType) {
	sortOrder := build_sorted_array(strings.Split(strings.Join(strings.Fields(group.Phase.Sort), ""), ","))
	usesHead := false
	for _, key := range sortOrder {
		usesHead = usesHead || key == HEAD
	}
	allIDs := map[int]bool{}
	for _, g := range group.Games {
		allIDs[g.HomeId], allIDs[g.AwayId] = true, true
	}
	for _, tg := range group.Team_groups {
		allIDs[tg.Team_id] = true
	}
	if len(allIDs) == 2 || len(allIDs) == 4 {
		allIDs[-1] = true
	}
	keys := make([]uint32, 0, len(allIDs))
	for id := range allIDs {
		keys = append(keys, uint32(id))
	}
	sort.Slice(keys, func(i, j int) bool { return keys[i] < keys[j] })
	table := NewTable(keys)
	campaign := make([]*TeamCampaign, len(keys))
	for _, tg := range group.Team_groups {
		campaign[table.Query(uint32(tg.Team_id))] = &TeamCampaign{
			id: tg.Team_id, bias: tg.Bias, points: tg.Add_sub, add_sub: tg.Add_sub,
			points_win:  group.Phase.Championship.Point_win,
			points_draw: group.Phase.Championship.Point_draw,
			points_loss: group.Phase.Championship.Point_loss, uses_head: usesHead,
		}
	}
	for _, g := range group.Games {
		g.home_table_index = table.Query(uint32(g.HomeId))
		g.away_table_index = table.Query(uint32(g.AwayId))
		if g.Played {
			if c := campaign[g.home_table_index]; c != nil {
				c.add_game(g)
			}
			if c := campaign[g.away_table_index]; c != nil {
				c.add_game(g)
			}
		}
	}
	return campaign, table, sortOrder
}

func diversifiedBenchmarkIntEnv(t *testing.T, name string, fallback int) int {
	t.Helper()
	if raw := os.Getenv(name); raw != "" {
		n, err := strconv.Atoi(raw)
		if err != nil || n <= 0 {
			t.Fatalf("%s must be a positive integer", name)
		}
		return n
	}
	return fallback
}

func diversifiedReferencePath(out string, group int) string {
	return filepath.Join(out, fmt.Sprintf("group-%d.reference.json", group))
}

func diversifiedReferenceDirectory(defaultDir string) string {
	if path := os.Getenv("RARE_POSITION_BENCHMARK_REFERENCE_DIR"); path != "" {
		return path
	}
	return defaultDir
}

func TestDiversifiedBuildReference(t *testing.T) {
	paths, out := diversifiedBenchmarkPaths(t)
	n := diversifiedBenchmarkIntEnv(t, "RARE_POSITION_REFERENCE_SAMPLES", 5000000)
	seed := int64(72991)
	for _, path := range paths {
		group, hash := diversifiedBenchmarkInput(t, path)
		campaign, table, sortOrder := diversifiedBenchmarkSetup(&group)
		counts := make(map[int][]int, len(group.Team_groups))
		for _, tg := range group.Team_groups {
			counts[tg.Team_id] = make([]int, len(group.Team_groups))
		}
		rng := rand.New(rand.NewSource(deriveRarePositionSeed(seed, fmt.Sprintf("reference-%d", group.Id))))
		for done := 0; done < n; {
			batch := n - done
			if batch > 100000 {
				batch = 100000
			}
			partial := simulatePlainRankCounts(campaign, group.Games, table, sortOrder, group.Team_groups, batch, rng)
			for id, positions := range partial {
				for pos, count := range positions {
					counts[id][pos] += count
				}
			}
			done += batch
		}
		ref := diversifiedReference{GroupID: group.Id, InputSHA256: hash, Samples: n, Seed: seed}
		for _, tg := range group.Team_groups {
			for pos, count := range counts[tg.Team_id] {
				ref.Cells = append(ref.Cells, diversifiedRefCell{tg.Team_id, pos, count, float64(count) / float64(n)})
			}
		}
		data, err := json.MarshalIndent(ref, "", "  ")
		if err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(diversifiedReferencePath(out, group.Id), append(data, '\n'), 0644); err != nil {
			t.Fatal(err)
		}
	}
}

func diversifiedProbabilityBand(p float64) string {
	switch {
	case p >= 1e-3:
		return "ge_1e-3"
	case p >= 1e-4:
		return "1e-4_to_1e-3"
	case p >= 1e-5:
		return "1e-5_to_1e-4"
	case p >= 1e-6:
		return "1e-6_to_1e-5"
	default:
		return "lt_1e-6"
	}
}

type diversifiedBenchEstimate struct{ p, se, ess float64 }

func diversifiedBenchMetrics(ref diversifiedReference, estimates map[[2]int]diversifiedBenchEstimate, feasibility map[[2]int]string) (diversifiedBenchQuality, map[string]diversifiedBandQuality) {
	quality := diversifiedBenchQuality{}
	band := map[string]diversifiedBandQuality{}
	var relSEs []float64
	type acc struct {
		cells, reliable, zCount, covered int
		ess4, ess10, ess25               int
		sumSq, sumAbs, sumZ, sumZ2       float64
		score, sumVar, sumRelVar         float64
	}
	accs := map[string]*acc{}
	for _, cell := range ref.Cells {
		key := [2]int{cell.Team, cell.Position}
		e := estimates[key]
		if feasibility[key] != "proven_impossible" {
			quality.Feasible++
			if e.p > 0 {
				quality.Nonzero++
				if e.se > 0 {
					relSEs = append(relSEs, e.se/e.p)
				}
			}
			if e.ess >= 4 {
				quality.ESS4++
			}
			if e.ess >= 10 {
				quality.ESS10++
			}
			if e.ess >= 25 {
				quality.ESS25++
			}
			quality.CoverageScore += math.Min(e.ess/10, 1)
		}
		name := diversifiedProbabilityBand(cell.P)
		a := accs[name]
		if a == nil {
			a = &acc{}
			accs[name] = a
		}
		a.cells++
		a.score += math.Min(e.ess/10, 1)
		if e.ess >= 4 {
			a.ess4++
		}
		if e.ess >= 10 {
			a.ess10++
		}
		if e.ess >= 25 {
			a.ess25++
		}
		a.sumVar += e.se * e.se
		if cell.Count < 25 {
			continue
		}
		a.reliable++
		if cell.P > 0 {
			a.sumRelVar += e.se * e.se / (cell.P * cell.P)
		}
		diff := e.p - cell.P
		a.sumSq += diff * diff
		a.sumAbs += math.Abs(diff)
		refVar := cell.P * (1 - cell.P) / float64(ref.Samples)
		se := math.Sqrt(e.se*e.se + refVar)
		if se > 0 {
			z := diff / se
			a.zCount++
			a.sumZ += z
			a.sumZ2 += z * z
			if math.Abs(z) <= 1.96 {
				a.covered++
			}
		}
	}
	sort.Float64s(relSEs)
	if len(relSEs) > 0 {
		quality.MedianRelSE = relSEs[len(relSEs)/2]
		quality.P90RelSE = relSEs[int(float64(len(relSEs)-1)*.9)]
	}
	for name, a := range accs {
		b := diversifiedBandQuality{Cells: a.cells, Reliable: a.reliable, CoverageScore: a.score, ESS4: a.ess4, ESS10: a.ess10, ESS25: a.ess25}
		if a.cells > 0 {
			b.MeanVariance = a.sumVar / float64(a.cells)
		}
		if a.reliable > 0 {
			b.MeanRelVar = a.sumRelVar / float64(a.reliable)
			b.RMSE = math.Sqrt(a.sumSq / float64(a.reliable))
			b.MeanAbsError = a.sumAbs / float64(a.reliable)
		}
		if a.zCount > 0 {
			b.MeanZ = a.sumZ / float64(a.zCount)
			b.ZVariance = a.sumZ2/float64(a.zCount) - b.MeanZ*b.MeanZ
			b.Coverage95 = float64(a.covered) / float64(a.zCount)
		}
		band[name] = b
	}
	return quality, band
}

func diversifiedBenchmarkCells(ref diversifiedReference, estimates map[[2]int]diversifiedBenchEstimate) []diversifiedBenchCell {
	cells := make([]diversifiedBenchCell, 0, len(ref.Cells))
	for _, cell := range ref.Cells {
		e := estimates[[2]int{cell.Team, cell.Position}]
		cells = append(cells, diversifiedBenchCell{cell.Team, cell.Position, e.p, e.se, e.ess, cell.P, cell.Count})
	}
	return cells
}

func TestDiversifiedBenchmarkCoverageScore(t *testing.T) {
	ref := diversifiedReference{Samples: 5000000, Cells: []diversifiedRefCell{
		{Team: 1, Position: 0, Count: 500, P: 1e-4},
		{Team: 1, Position: 1, Count: 0, P: 0},
		{Team: 2, Position: 0, Count: 500000, P: .1},
	}}
	estimates := map[[2]int]diversifiedBenchEstimate{
		{1, 0}: {1e-4, 1e-4 / math.Sqrt(5), 5},
		{1, 1}: {0, 0, 0},
		{2, 0}: {.1, .001, 10000},
	}
	quality, bands := diversifiedBenchMetrics(ref, estimates, map[[2]int]string{{1, 1}: "proven_impossible"})
	if quality.Feasible != 2 || quality.CoverageScore != 1.5 || quality.ESS10 != 1 || bands["1e-4_to_1e-3"].Reliable != 1 {
		t.Fatalf("unexpected whole-table benchmark metrics: quality=%+v bands=%+v", quality, bands)
	}
}

func TestDiversifiedMatchedWorkBenchmark(t *testing.T) {
	paths, out := diversifiedBenchmarkPaths(t)
	seeds := diversifiedBenchmarkIntEnv(t, "RARE_POSITION_BENCHMARK_SEEDS", 5)
	commit := "unknown"
	if data, err := exec.Command("git", "rev-parse", "HEAD").Output(); err == nil {
		commit = strings.TrimSpace(string(data))
	}
	if label := os.Getenv("RARE_POSITION_BENCHMARK_COMMIT"); label != "" {
		commit = label
	}
	config := map[string]string{}
	config["variant"] = os.Getenv("RARE_POSITION_BENCHMARK_VARIANT")
	for _, key := range []string{"RARE_POSITION_DIVERSIFIED_SCOUT_SAMPLES", "RARE_POSITION_DIVERSIFIED_DISCOVERY_EQ", "RARE_POSITION_DIVERSIFIED_VALIDATION_EQ", "RARE_POSITION_DIVERSIFIED_MAX_VALIDATED_PROPOSALS", "RARE_POSITION_DIVERSIFIED_VALIDATION_MIN_SAMPLES", "RARE_POSITION_DIVERSIFIED_MIN_PLAIN_FRACTION", "RARE_POSITION_POINT_HYBRID_GROUP", "RARE_POSITION_POINT_HYBRID_TEAM", "RARE_POSITION_POINT_HYBRID_MAX_RANK", "RARE_POSITION_POINT_HYBRID_TARGET_MASS", "RARE_POSITION_POINT_HYBRID_Q_PERCENT", "RARE_POSITION_ADAPTIVE_SCOUT_SAMPLES", "RARE_POSITION_ADAPTIVE_TAIL_ONLY", "RARE_POSITION_ADAPTIVE_POINT_GROUP_MODE", "RARE_POSITION_ADAPTIVE_POINT_GROUP_TVD", "RARE_POSITION_ADAPTIVE_VALIDATION_INITIAL_SAMPLES", "RARE_POSITION_ADAPTIVE_MAX_VALIDATED_PROPOSALS", "RARE_POSITION_ADAPTIVE_MIN_PLAIN_PRODUCTION_FRACTION", "RARE_POSITION_BENCHMARK_WORK_LIMIT", "RARE_POSITION_LEGACY_PROPOSAL_SEARCH"} {
		config[key] = os.Getenv(key)
	}
	resultPath := filepath.Join(out, "runs.jsonl")
	file, err := os.OpenFile(resultPath, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0644)
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	encoder := json.NewEncoder(file)
	for _, path := range paths {
		group, hash := diversifiedBenchmarkInput(t, path)
		data, err := os.ReadFile(diversifiedReferencePath(diversifiedReferenceDirectory(out), group.Id))
		if err != nil {
			t.Fatal(err)
		}
		var ref diversifiedReference
		if err := json.Unmarshal(data, &ref); err != nil {
			t.Fatal(err)
		}
		if ref.InputSHA256 != hash || ref.GroupID != group.Id || ref.Samples < 5000000 {
			t.Fatalf("missing or stale high-precision reference for group %d", group.Id)
		}
		campaign, table, sortOrder := diversifiedBenchmarkSetup(&group)
		unplayed := 0
		for _, g := range group.Games {
			if !g.Played {
				unplayed++
			}
		}
		plainCost := estimateSeasonWork(unplayed, 1, len(group.Team_groups))
		limit := int64(DiversifiedDefaultTotalWork)
		if raw := os.Getenv("RARE_POSITION_BENCHMARK_WORK_LIMIT"); raw != "" {
			parsed, err := strconv.ParseInt(raw, 10, 64)
			if err != nil || parsed <= 0 || parsed > int64(DiversifiedDefaultTotalWork) {
				t.Fatalf("RARE_POSITION_BENCHMARK_WORK_LIMIT must be in 1..%d", DiversifiedDefaultTotalWork)
			}
			limit = parsed
		}
		if plainCost <= 0 {
			t.Fatal("invalid work per sample")
		}
		for i := 0; i < seeds; i++ {
			seed := int64(1001 + i)
			var diagnostics DiversifiedRunDiagnostics
			var div map[int]map[int]ProductionEstimate
			if os.Getenv("RARE_POSITION_POINT_HYBRID_GROUP") != "" {
				div = runDiversifiedSearchAndProductionDetailed(&group, campaign, table, sortOrder, nil, limit, seed, &diagnostics)
				if len(diagnostics.Shortlist) != 1 || diagnostics.Shortlist[0].Kind != "point_outcome_stratum" {
					t.Fatalf("point hybrid production path did not activate for group %d", group.Id)
				}
			} else if raw := os.Getenv("RARE_POSITION_POINT_HYBRID_TEAM"); raw != "" {
				targetTeam, err := strconv.Atoi(raw)
				if err != nil {
					t.Fatal(err)
				}
				maxRank := diversifiedBenchmarkIntEnv(t, "RARE_POSITION_POINT_HYBRID_MAX_RANK", len(group.Team_groups)-5)
				var hybrid PointHybridDiagnostics
				var ok bool
				div, hybrid, ok = runPointHybrid(&group, campaign, table, sortOrder, limit, seed, targetTeam, maxRank)
				if !ok {
					t.Fatalf("point hybrid unavailable for group %d", group.Id)
				}
				recordPointHybridDiagnostics(&diagnostics, hybrid, targetTeam, maxRank, plainCost,
					estimateSeasonWork(unplayed, 2, len(group.Team_groups)), limit)
				t.Log(pointHybridDescription(hybrid))
			} else {
				div = runDiversifiedSearchAndProductionDetailed(&group, campaign, table, sortOrder, nil, limit, seed, &diagnostics)
			}
			d := diagnostics.Design
			if d.ScoutWork+d.DiscoveryWork+d.ValidationWork+d.ProductionWork > limit {
				t.Fatal("diversified work exceeded 35M")
			}
			divCells := map[[2]int]diversifiedBenchEstimate{}
			for team, positions := range div {
				for pos, e := range positions {
					if math.IsNaN(e.Probability) || e.Probability < 0 || e.Probability > 1 || math.IsNaN(e.StdErr) || e.StdErr < 0 {
						t.Fatalf("invalid estimate group=%d seed=%d cell=%d/%d", group.Id, seed, team, pos)
					}
					divCells[[2]int{team, pos}] = diversifiedBenchEstimate{e.Probability, e.StdErr, e.ESS}
				}
			}
			q, bands := diversifiedBenchMetrics(ref, divCells, diagnostics.Feasibility)
			production := map[string]diversifiedBenchBatch{}
			for _, batch := range d.Batches {
				production[batch.Proposal.ID] = diversifiedBenchBatch{batch.Samples, batch.Work}
			}
			run := diversifiedBenchResult{Commit: commit, Configuration: config, Group: group.Id, Seed: seed, Method: "diversified", ReferenceSamples: ref.Samples,
				Work:      diversifiedBenchWork{d.ScoutWork, d.DiscoveryWork, d.ValidationWork, d.ProductionWork, d.ScoutWork + d.DiscoveryWork + d.ValidationWork + d.ProductionWork, limit},
				Discovery: map[string]int{"candidates": diagnostics.Candidates, "shortlisted": diagnostics.Shortlisted}, Shortlist: diagnostics.Shortlist, Validation: diagnostics.ValidationSamples, ValidationCells: diagnostics.ValidationCells, Production: production, Quality: q, TruthByBand: bands, Cells: diversifiedBenchmarkCells(ref, divCells)}
			if err := encoder.Encode(run); err != nil {
				t.Fatal(err)
			}
			plainSamples := int(limit / plainCost)
			plainCounts := simulatePlainRankCounts(campaign, group.Games, table, sortOrder, group.Team_groups, plainSamples, rand.New(rand.NewSource(deriveRarePositionSeed(seed, "plain-mc-baseline"))))
			plainCells := map[[2]int]diversifiedBenchEstimate{}
			for team, positions := range plainCounts {
				for pos, count := range positions {
					p := float64(count) / float64(plainSamples)
					se := math.Sqrt(p * (1 - p) / float64(plainSamples))
					ess := 0.0
					if se > 0 {
						ess = p * p / (se * se)
					} else if p == 1 {
						ess = float64(plainSamples)
					}
					plainCells[[2]int{team, pos}] = diversifiedBenchEstimate{p, se, ess}
				}
			}
			q, bands = diversifiedBenchMetrics(ref, plainCells, diagnostics.Feasibility)
			plainRun := diversifiedBenchResult{Commit: commit, Configuration: config, Group: group.Id, Seed: seed, Method: "plain_mc", ReferenceSamples: ref.Samples,
				Work:       diversifiedBenchWork{Production: int64(plainSamples) * plainCost, Total: int64(plainSamples) * plainCost, Limit: limit},
				Production: map[string]diversifiedBenchBatch{"plain_mc": {plainSamples, int64(plainSamples) * plainCost}}, Quality: q, TruthByBand: bands, Cells: diversifiedBenchmarkCells(ref, plainCells)}
			if err := encoder.Encode(plainRun); err != nil {
				t.Fatal(err)
			}
		}
	}
}
