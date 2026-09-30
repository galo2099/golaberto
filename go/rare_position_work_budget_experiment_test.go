package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"math"
	"math/rand"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"
)

// Offline experiment: keep the production matrix and spend a bounded
// additional production work budget on its unresolved zeros. No production
// defaults or service responses are changed by this harness.
type budgetExperimentSample struct {
	Tilt       float64  `json:"tilt"`
	PointTilt  float64  `json:"point_tilt"`
	Samples    int      `json:"samples"`
	Hits       int      `json:"hits"`
	P          float64  `json:"p"`
	SE         float64  `json:"se"`
	ESS        float64  `json:"ess"`
	MaxShare   float64  `json:"max_share"`
	BatchGap   float64  `json:"batch_gap"`
	Relative   *float64 `json:"relative_se"`
	Valid      bool     `json:"valid"`
	StreamSeed int64    `json:"stream_seed,omitempty"`
}

func describeBudgetSample(result conditionedZeroResult, tilt, pointTilt float64) budgetExperimentSample {
	var relative *float64
	if result.probability > 0 {
		r := result.stdErr / result.probability
		relative = &r
	}
	return budgetExperimentSample{tilt, pointTilt, result.samples, result.hits,
		result.probability, result.stdErr, result.ess, result.maxWeightShare,
		result.batchGap, relative, conditionedPointTiltResultValid(result), 0}
}

type budgetExperimentCell struct {
	Team             int                      `json:"team"`
	Position         int                      `json:"position"`
	Before           ProductionEstimate       `json:"before"`
	After            ProductionEstimate       `json:"after"`
	Mass             float64                  `json:"mass"`
	Pilots           []budgetExperimentSample `json:"pilots"`
	Selected         budgetExperimentSample   `json:"selected"`
	Gentle           budgetExperimentSample   `json:"gentle"`
	SampledWitness   bool                     `json:"sampled_witness"`
	Accepted         bool                     `json:"accepted"`
	CheckConclusive  bool                     `json:"check_conclusive"`
	CheckContradicts bool                     `json:"check_contradicts"`
	Work             int64                    `json:"work"`
	event            *conditionedPointEvent
	best             [2]float64
}

type budgetExperimentRun struct {
	Group        int                                `json:"group"`
	Input        string                             `json:"input"`
	SHA256       string                             `json:"input_sha256"`
	Seed         int64                              `json:"seed"`
	Unplayed     int                                `json:"unplayed"`
	BaselineMS   float64                            `json:"baseline_ms"`
	ExtraMS      float64                            `json:"extra_ms"`
	BaselineWork int64                              `json:"baseline_work"`
	ExtraWork    int64                              `json:"extra_work"`
	WorkLimit    int64                              `json:"work_limit"`
	Baseline     map[int]map[int]ProductionEstimate `json:"baseline"`
	Extended     map[int]map[int]ProductionEstimate `json:"extended"`
	Cells        []budgetExperimentCell             `json:"cells"`
}

// parallelBudgetCells uses the same four-core limit as the production request.
func parallelBudgetCells(cells []budgetExperimentCell, run func(int)) {
	jobs := make(chan int, len(cells))
	var wait sync.WaitGroup
	for worker := 0; worker < min(4, len(cells)); worker++ {
		wait.Add(1)
		go func() {
			defer wait.Done()
			for index := range jobs {
				run(index)
			}
		}()
	}
	for i := range cells {
		jobs <- i
	}
	close(jobs)
	wait.Wait()
}

func extendBudgetExperiment(input GroupType, baseline map[int]map[int]ProductionEstimate,
	seed int64, extraLimit int64) (map[int]map[int]ProductionEstimate, []budgetExperimentCell, int64) {
	group := cloneGroupForBenchmark(input)
	campaign, table, order := diversifiedBenchmarkSetup(group)
	bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
	samplers := newConditionedScoreSamplers(group.Games)
	extended := make(map[int]map[int]ProductionEstimate, len(baseline))
	var cells []budgetExperimentCell
	ids := teamIDsFromGroups(group.Team_groups)
	sort.Ints(ids)
	for _, id := range ids {
		extended[id] = make(map[int]ProductionEstimate, len(baseline[id]))
		for rank := range group.Team_groups {
			est := baseline[id][rank]
			extended[id][rank] = est
			if est.Probability > 0 || strings.HasPrefix(est.Reachability, "impossible") {
				continue
			}
			cell := budgetExperimentCell{Team: id, Position: rank + 1, Before: est, After: est}
			cell.event, _ = buildConditionedPointEvent(id, rank, nil, group, campaign, table, bounds)
			if cell.event != nil {
				cell.Mass = cell.event.mass
			}
			cells = append(cells, cell)
		}
	}
	eligible := 0
	for _, cell := range cells {
		if cell.event != nil && cell.Mass > 0 {
			eligible++
		}
	}
	if eligible == 0 {
		return extended, cells, 0
	}
	perDraw := estimateSeasonWork(len(group.Games)-countPlayedGames(group.Games), 1, len(group.Team_groups))
	// Probe both point-tilt directions at three existing rank-tilt strengths.
	// Direction and candidate admission do not depend on a particular rank.
	configs := [][2]float64{{3, 0.5}, {3, -0.5}, {8, 1}, {8, -1}, {12, 1}, {12, -1}}
	pilotSamples := min(5000, int(extraLimit/perDraw)/max(1, 2*eligible*len(configs)))
	parallelBudgetCells(cells, func(index int) {
		cell := &cells[index]
		if cell.event == nil || cell.Mass <= 0 {
			return
		}
		cell.best = configs[0]
		bestESS, bestHits := 0.0, 0
		for _, config := range configs {
			stream := deriveRarePositionSeed(seed, fmt.Sprintf("budget-pilot-%d-%d-%g-%g",
				cell.Team, cell.Position, config[0], config[1]))
			result, _ := sampleConditionedZeroRankLookaheadWithPointTiltMode(
				cell.event, cell.Team, cell.Position-1, group, campaign, table, order,
				bounds, samplers, pilotSamples, stream, config[0], config[1], true)
			cell.Work += result.work
			cell.Pilots = append(cell.Pilots, describeBudgetSample(result, config[0], config[1]))
			cell.SampledWitness = cell.SampledWitness || result.hits > 0
			if bestHits == 0 && result.hits > 0 || result.ess >= 10 && result.ess > bestESS {
				cell.best, bestESS, bestHits = config, result.ess, result.hits
			}
		}
	})
	var pilotWork int64
	for _, cell := range cells {
		pilotWork += cell.Work
	}
	drawsPerCell := int((extraLimit - pilotWork) / perDraw / int64(eligible))
	parallelBudgetCells(cells, func(index int) {
		cell := &cells[index]
		if cell.event == nil || cell.Mass <= 0 || drawsPerCell < 2 {
			return
		}
		selectedDraws := 2 * drawsPerCell / 3
		selectedSeed := deriveRarePositionSeed(seed, fmt.Sprintf("budget-confirm-%d-%d", cell.Team, cell.Position))
		selected, _ := sampleConditionedZeroRankLookaheadWithPointTilt(cell.event,
			cell.Team, cell.Position-1, group, campaign, table, order, bounds, samplers,
			selectedDraws, selectedSeed, cell.best[0], cell.best[1])
		gentleTilt := math.Copysign(0.5, cell.best[1])
		gentleSeed := deriveRarePositionSeed(seed, fmt.Sprintf("budget-gentle-%d-%d", cell.Team, cell.Position))
		gentle, _ := sampleConditionedZeroRankLookaheadWithPointTilt(cell.event,
			cell.Team, cell.Position-1, group, campaign, table, order, bounds, samplers,
			drawsPerCell-selectedDraws, gentleSeed, 3, gentleTilt)
		cell.Work += selected.work + gentle.work
		cell.Selected = describeBudgetSample(selected, cell.best[0], cell.best[1])
		cell.Gentle = describeBudgetSample(gentle, 3, gentleTilt)
		cell.SampledWitness = cell.SampledWitness || selected.hits > 0 || gentle.hits > 0
		cell.CheckConclusive = gentle.probability > 0 && gentle.hits >= 30 && gentle.ess >= 5 &&
			gentle.stdErr/gentle.probability <= 0.6
		if selected.probability > 0 && cell.CheckConclusive {
			ratio := selected.probability / gentle.probability
			cell.CheckContradicts = ratio < 1.0/30 || ratio > 30
		}
		result, valid := selected, conditionedPointTiltResultValid(selected)
		// Prefer a valid gentler independent confirmation. An inconclusive
		// check does not certify the stronger proposal's accuracy.
		if conditionedPointTiltResultValid(gentle) {
			result, valid = gentle, true
		} else if cell.CheckContradicts {
			valid = false
		}
		if valid {
			cell.Accepted = true
			cell.After = applyConditionedPointTiltEstimate(cell.Before, result, "offline_budget_point_tilt")
		} else if cell.SampledWitness {
			cell.After.Reachability = "reachable_by_sampling_without_estimate"
		}
	})
	var work int64
	for _, cell := range cells {
		work += cell.Work
		extended[cell.Team][cell.Position-1] = cell.After
	}
	return extended, cells, work
}

func TestRarePositionTenfoldWorkExperiment(t *testing.T) {
	runRarePositionWorkExperiment(t, 10)
}

func TestRarePositionHundredfoldWorkExperiment(t *testing.T) {
	runRarePositionWorkExperiment(t, 100)
}

func runRarePositionWorkExperiment(t *testing.T, multiplier int64) {
	t.Helper()
	paths, output := os.Getenv("RARE_POSITION_BUDGET_EXPERIMENT_REQUESTS"), os.Getenv("RARE_POSITION_BUDGET_EXPERIMENT_OUTPUT")
	if paths == "" || output == "" {
		t.Skip("set request paths and output directory for the offline work-budget experiment")
	}
	if !filepath.IsAbs(output) {
		t.Fatal("use an absolute output path in the ignored experiments/rare_positions/local directory")
	}
	previousProcs := runtime.GOMAXPROCS(4)
	defer runtime.GOMAXPROCS(previousProcs)
	if err := os.MkdirAll(output, 0700); err != nil {
		t.Fatal(err)
	}
	for _, flag := range []string{"RARE_POSITION_MATCHED_POINT_POOL", "RARE_POSITION_CONDITIONED_ZERO",
		"RARE_POSITION_CONDITIONED_ZERO_LOOKAHEAD", "RARE_POSITION_CONDITIONED_POINT_TILT",
		"RARE_POSITION_POINT_TILT_UNDECIDED", "RARE_POSITION_POINT_TILT_CROSSCHECK", "RARE_POSITION_DIRECTIONAL_PEER_RESCUE"} {
		t.Setenv(flag, "1")
	}
	t.Setenv("RARE_POSITION_IMPORTANCE_SAMPLING", "0")
	t.Setenv("RARE_POSITION_BENCHMARK_ITERATIONS", "20000")
	t.Setenv("RARE_POSITION_MATCHED_POINT_POOL_WORKERS", "4")
	seeds := os.Getenv("RARE_POSITION_BUDGET_EXPERIMENT_SEEDS")
	if seeds == "" {
		seeds = "801,804,808,817,911"
	}
	previousLog := log.Writer()
	log.SetOutput(io.Discard)
	defer log.SetOutput(previousLog)
	for _, path := range strings.Split(paths, ",") {
		data, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		var input GroupType
		if err := json.Unmarshal(data, &input); err != nil {
			t.Fatal(err)
		}
		hash := fmt.Sprintf("%x", sha256.Sum256(data))
		unplayed := len(input.Games) - countPlayedGames(input.Games)
		for _, rawSeed := range strings.Split(seeds, ",") {
			seed, err := strconv.ParseInt(rawSeed, 10, 64)
			if err != nil {
				t.Fatal(err)
			}
			t.Setenv("RARE_POSITION_RANDOM_SEED", rawSeed)
			started := time.Now()
			baseline := cloneGroupForBenchmark(input).calculate_odds()["rare_position_estimates"].(map[int]map[int]ProductionEstimate)
			baselineMS := float64(time.Since(started).Microseconds()) / 1000
			var productionWork int64
			for _, row := range baseline {
				for _, est := range row {
					productionWork = max(productionWork, est.WorkSpent)
				}
			}
			baselineWork := productionWork + 20000*estimateSeasonWork(unplayed, 1, len(input.Team_groups))
			started = time.Now()
			extended, cells, extraWork := extendBudgetExperiment(input, baseline, seed, (multiplier-1)*baselineWork)
			extraMS := float64(time.Since(started).Microseconds()) / 1000
			if extraWork > (multiplier-1)*baselineWork {
				t.Fatalf("work cap exceeded: extra=%d cap=%d", extraWork, (multiplier-1)*baselineWork)
			}
			for team, row := range baseline {
				for rank, est := range row {
					if est.Probability > 0 && extended[team][rank].Probability != est.Probability {
						t.Fatalf("baseline estimate changed: team=%d rank=%d", team, rank+1)
					}
				}
			}
			run := budgetExperimentRun{input.Id, path, hash, seed, unplayed, baselineMS, extraMS,
				baselineWork, extraWork, multiplier * baselineWork, baseline, extended, cells}
			encoded, err := json.MarshalIndent(run, "", "  ")
			if err != nil {
				t.Fatal(err)
			}
			name := fmt.Sprintf("group-%d-%s-seed-%d.json", input.Id, hash[:8], seed)
			if err := os.WriteFile(filepath.Join(output, name), encoded, 0600); err != nil {
				t.Fatal(err)
			}
			gained, witnesses := 0, 0
			for _, cell := range cells {
				if cell.Accepted {
					gained++
				}
				if cell.SampledWitness {
					witnesses++
				}
			}
			t.Logf("group=%d hash=%s seed=%d zeros_searched=%d gained=%d sampled_witnesses=%d work=%.2fx baseline=%.0fms extra=%.0fms",
				input.Id, hash[:8], seed, len(cells), gained, witnesses,
				float64(baselineWork+extraWork)/float64(baselineWork), baselineMS, extraMS)
		}
	}
}

// Optional diagnostics outside the tenfold coverage budget. These use different
// proposal strengths and fresh streams to check selected new tail estimates.
func TestRarePositionBudgetProposalCheck(t *testing.T) {
	paths := os.Getenv("RARE_POSITION_BUDGET_EXPERIMENT_REQUESTS")
	output := os.Getenv("RARE_POSITION_BUDGET_EXPERIMENT_OUTPUT")
	specs := os.Getenv("RARE_POSITION_BUDGET_CHECK_CELLS") // group:team:position,...
	if paths == "" || output == "" || specs == "" {
		t.Skip("set request paths, output directory and diagnostic cells")
	}
	if !filepath.IsAbs(output) {
		t.Fatal("use an absolute output path")
	}
	if err := os.MkdirAll(output, 0700); err != nil {
		t.Fatal(err)
	}
	previousProcs := runtime.GOMAXPROCS(4)
	defer runtime.GOMAXPROCS(previousProcs)
	samples := diversifiedBenchmarkIntEnv(t, "RARE_POSITION_BUDGET_CHECK_SAMPLES", 300000)
	checkSeed := int64(1211)
	if raw := os.Getenv("RARE_POSITION_BUDGET_CHECK_SEED"); raw != "" {
		var err error
		checkSeed, err = strconv.ParseInt(raw, 10, 64)
		if err != nil {
			t.Fatal(err)
		}
	}
	for _, path := range strings.Split(paths, ",") {
		data, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		var input GroupType
		if err := json.Unmarshal(data, &input); err != nil {
			t.Fatal(err)
		}
		group := cloneGroupForBenchmark(input)
		campaign, table, order := diversifiedBenchmarkSetup(group)
		bounds := buildPointRankBounds(campaign, group.Team_groups, group.Games, table)
		samplers := newConditionedScoreSamplers(group.Games)
		standing := make([]*TeamCampaign, 0, len(group.Team_groups))
		for _, team := range group.Team_groups {
			standing = append(standing, campaign[table.Query(uint32(team.Team_id))])
		}
		sort.Sort(TeamCampaignSorted{t: standing, sort: order, rng: rand.New(rand.NewSource(1))})
		current := make(map[int]int, len(standing))
		for rank, team := range standing {
			current[team.id] = rank
		}
		var cells []budgetExperimentCell
		for _, spec := range strings.Split(specs, ",") {
			var id, team, position int
			if _, err := fmt.Sscanf(spec, "%d:%d:%d", &id, &team, &position); err != nil {
				t.Fatal(err)
			}
			if id != input.Id {
				continue
			}
			if position < 1 || position > len(input.Team_groups) {
				t.Fatal("diagnostic position out of range")
			}
			cell := budgetExperimentCell{Team: team, Position: position}
			cell.event, _ = buildConditionedPointEvent(team, position-1, nil, group, campaign, table, bounds)
			if cell.event == nil {
				t.Fatal("diagnostic point event unavailable")
			}
			cell.Mass = cell.event.mass
			cells = append(cells, cell)
		}
		parallelBudgetCells(cells, func(index int) {
			cell := &cells[index]
			direction := 1.0
			if cell.Position-1 > current[cell.Team] {
				direction = -1
			}
			for _, config := range [][2]float64{{4, 0.5}, {6, 0.75}, {10, 1}} {
				stream := deriveRarePositionSeed(checkSeed, fmt.Sprintf("budget-check-%d-%d-%d-%g", input.Id,
					cell.Team, cell.Position, config[0]))
				result, _ := sampleConditionedZeroRankLookaheadWithPointTilt(cell.event,
					cell.Team, cell.Position-1, group, campaign, table, order, bounds, samplers,
					samples, stream, config[0], direction*config[1])
				cell.Work += result.work
				cell.Pilots = append(cell.Pilots, describeBudgetSample(result, config[0], direction*config[1]))
				cell.Pilots[len(cell.Pilots)-1].StreamSeed = stream
			}
		})
		hash := fmt.Sprintf("%x", sha256.Sum256(data))
		encoded, err := json.MarshalIndent(cells, "", "  ")
		if err != nil {
			t.Fatal(err)
		}
		name := fmt.Sprintf("proposal-check-%d-%s.json", input.Id, hash[:8])
		if err := os.WriteFile(filepath.Join(output, name), encoded, 0600); err != nil {
			t.Fatal(err)
		}
		for _, cell := range cells {
			for _, sample := range cell.Pilots {
				t.Logf("group=%d team=%d position=%d tilt=%g p=%g hits=%d ess=%.1f valid=%t", input.Id,
					cell.Team, cell.Position, sample.Tilt, sample.P, sample.Hits, sample.ESS, sample.Valid)
			}
		}
	}
}
