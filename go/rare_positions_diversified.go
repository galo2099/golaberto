package main

import (
	"fmt"
	"log"
	"math"
	"math/rand"
	"os"
	"sort"
	"strconv"
)

const (
	DiversifiedDefaultScoutSamples       = 15000
	DiversifiedDefaultSearchWorkCap      = 5000 * 350 // MC-equivalent work
	DiversifiedDefaultTotalWork          = 35000000   // 35M nominal work
	DiversifiedDefensiveMixtureEpsilon   = 0.05
	DiversifiedProbeChunkWork            = 150 * 350
	DiversifiedAllocChunkWork            = 500 * 350
	DiversifiedDefaultMinPlainFraction   = 0.10
	DiversifiedMaxSingleTeamPerDirection = 2
	DiversifiedMaxCompetitors            = 3
	DiversifiedMaxKL                     = 3.0
	DiversifiedTargetESS                 = 10.0
)

func diversifiedIsEnabled() bool {
	return os.Getenv("RARE_POSITION_DIVERSIFIED_IS") == "1"
}

func diversifiedScoutSamples() int {
	if raw := os.Getenv("RARE_POSITION_DIVERSIFIED_SCOUT_SAMPLES"); raw != "" {
		if parsed, err := strconv.Atoi(raw); err == nil && parsed > 0 {
			return parsed
		}
	}
	return DiversifiedDefaultScoutSamples
}

func diversifiedMinPlainFraction() float64 {
	if raw := os.Getenv("RARE_POSITION_DIVERSIFIED_MIN_PLAIN_FRACTION"); raw != "" {
		if parsed, err := strconv.ParseFloat(raw, 64); err == nil && parsed >= 0 && parsed <= 1 {
			return parsed
		}
	}
	return DiversifiedDefaultMinPlainFraction
}

type DiversifiedProposalKind string

const (
	ProposalPlainMC            DiversifiedProposalKind = "plain_mc"
	ProposalSingleTeam         DiversifiedProposalKind = "single_team_strength"
	ProposalCompetitorAssisted DiversifiedProposalKind = "competitor_assisted"
)

type DiversifiedProposal struct {
	ID            string                  `json:"id"`
	Kind          DiversifiedProposalKind `json:"kind"`
	TargetTeam    int                     `json:"target_team"`
	Direction     RareDirection           `json:"direction"`
	Strength      float64                 `json:"strength"`
	Competitors   []int                   `json:"competitors,omitempty"`
	Means         []GameProposalMeans     `json:"-"`
	KL            float64                 `json:"kl"`
	WorkPerSample int64                   `json:"work_per_sample"`
}

type ProbeRankMetrics struct {
	TargetTeam        int     `json:"target_team"`
	Samples           int     `json:"samples"`
	MeanRank          float64 `json:"mean_rank"`
	MinRank           int     `json:"min_rank"`
	MaxRank           int     `json:"max_rank"`
	P10               float64 `json:"p10"`
	P25               float64 `json:"p25"`
	P50               float64 `json:"p50"`
	P75               float64 `json:"p75"`
	P90               float64 `json:"p90"`
	FrontierMass      float64 `json:"frontier_mass"`
	Near1Mass         float64 `json:"near1_mass"`
	Near2Mass         float64 `json:"near2_mass"`
	Near3Mass         float64 `json:"near3_mass"`
	UnresolvedSupport int     `json:"unresolved_support"`
}

type ProposalProbeStats struct {
	ProposalID                    string                   `json:"proposal_id"`
	Samples                       int                      `json:"samples"`
	Work                          int64                    `json:"work"`
	RankHistograms                map[int][]int            `json:"-"`
	CellHits                      map[[2]int]int           `json:"-"`
	CellSumR                      map[[2]int]float64       `json:"-"`
	CellSumR2OverDen              map[[2]int]float64       `json:"-"`
	CellSumZ1                     map[[2]int]float64       `json:"-"`
	CellSumZ1Sq                   map[[2]int]float64       `json:"-"`
	CellSumZ2                     map[[2]int]float64       `json:"-"`
	CellSumZ2Sq                   map[[2]int]float64       `json:"-"`
	CellESS                       map[[2]int]float64       `json:"-"`
	CellPredictedRawVarPerSample map[[2]int]float64       `json:"-"`
	CellPredictedVarPerSample     map[[2]int]float64       `json:"-"`
	Metrics                       map[int]ProbeRankMetrics `json:"metrics,omitempty"`
}

type FrozenProductionBatch struct {
	Proposal      DiversifiedProposal `json:"proposal"`
	Samples       int                 `json:"samples"`
	Work          int64               `json:"work"`
	WorkPerSample int64               `json:"work_per_sample"`
}

type FrozenDiversifiedDesign struct {
	Batches                   []FrozenProductionBatch       `json:"batches"`
	CellCombinationWeights    map[[2]int][]float64           `json:"-"` // (teamID, pos) -> beta per batch
	CellPredictedVarPerSample map[string]map[[2]int]float64 `json:"-"`
	TotalWork                 int64                         `json:"total_work"`
	ScoutWork                 int64                         `json:"scout_work"`
	SearchWork                int64                         `json:"search_work"`
	ProductionWork            int64                         `json:"production_work"`
	UnusedWork                int64                         `json:"unused_work"`
}

type DiversifiedEstimate struct {
	Probability         float64              `json:"probability"`
	StdErr              float64              `json:"std_err"`
	RelativeSE          *float64             `json:"relative_se"`
	ESS                 float64              `json:"ess"`
	ESSPerWork          float64              `json:"ess_per_work"`
	RawHits             int                  `json:"raw_hits"`
	Samples             int                  `json:"samples"`
	WorkSpent           int64                `json:"work_spent"`
	Available           bool                 `json:"available"`
	MeetsPrecisionGoal  bool                 `json:"meets_precision_goal"`
	ZeroHitUpper95      float64              `json:"zero_hit_upper_95"`
	Design              string               `json:"design"`
	BatchProbabilities  []float64            `json:"batch_probabilities,omitempty"`
	BatchVariances      []float64            `json:"batch_variances,omitempty"`
	CombinationBetas    []float64            `json:"combination_betas,omitempty"`
}

type ScoutData struct {
	Samples              int
	Work                 int64
	TeamCounts           map[int][]int
	TeamProbs            map[int][]float64
	TeamMeanRanks        map[int]float64
	MinObservedRank      map[int]int
	MaxObservedRank      map[int]int
	Feasibility          map[[2]int]string // (teamID, pos) -> "observed", "feasible_unseen", "proven_impossible"
}

func runPlainMCScout(
	baseCampaign []*TeamCampaign,
	games []*GameType,
	table *Table,
	sortOrder []SortType,
	teamGroups []TeamType,
	scoutSamples int,
	plainWorkPerSample int64,
	rng *rand.Rand,
) ScoutData {
	counts := simulatePlainRankCounts(baseCampaign, games, table, sortOrder, teamGroups, scoutSamples, rng)
	numPositions := len(teamGroups)

	data := ScoutData{
		Samples:         scoutSamples,
		Work:            int64(scoutSamples) * plainWorkPerSample,
		TeamCounts:      counts,
		TeamProbs:       make(map[int][]float64, len(teamGroups)),
		TeamMeanRanks:   make(map[int]float64, len(teamGroups)),
		MinObservedRank: make(map[int]int, len(teamGroups)),
		MaxObservedRank: make(map[int]int, len(teamGroups)),
		Feasibility:     make(map[[2]int]string),
	}

	for _, team := range teamGroups {
		id := team.Team_id
		teamCounts := counts[id]
		probs := make([]float64, numPositions)
		meanRank := 0.0
		minObserved := numPositions
		maxObserved := -1

		for pos, c := range teamCounts {
			p := float64(c) / float64(scoutSamples)
			probs[pos] = p
			meanRank += float64(pos) * p
			if c > 0 {
				if pos < minObserved {
					minObserved = pos
				}
				if pos > maxObserved {
					maxObserved = pos
				}
				data.Feasibility[[2]int{id, pos}] = "observed"
			} else {
				feasible := possiblePositionByPointsBoundsWithRNG(id, pos, baseCampaign, teamGroups, games, table, sortOrder, rng)
				if feasible {
					data.Feasibility[[2]int{id, pos}] = "feasible_unseen"
				} else {
					data.Feasibility[[2]int{id, pos}] = "proven_impossible"
				}
			}
		}

		data.TeamProbs[id] = probs
		data.TeamMeanRanks[id] = meanRank
		data.MinObservedRank[id] = minObserved
		data.MaxObservedRank[id] = maxObserved
	}

	return data
}

type DirectionalTail struct {
	TeamID                  int           `json:"team_id"`
	Direction               RareDirection `json:"direction"`
	ObservedMinRank         int           `json:"observed_min_rank"`
	ObservedMaxRank         int           `json:"observed_max_rank"`
	FrontierRank            int           `json:"frontier_rank"`
	FirstUnseenPosition     int           `json:"first_unseen_position"`
	UnseenFeasiblePositions []int         `json:"unseen_feasible_positions"`
	UnseenFeasibleCount     int           `json:"unseen_feasible_count"`
	ScoutMeanRank           float64       `json:"scout_mean_rank"`
	ScoutP10                float64       `json:"scout_p10"`
	ScoutP25                float64       `json:"scout_p25"`
	ScoutP50                float64       `json:"scout_p50"`
	ScoutP75                float64       `json:"scout_p75"`
	ScoutP90                float64       `json:"scout_p90"`
	ExtremityScore          float64       `json:"extremity_score"`
	Priority                float64       `json:"priority"`
}

func computeRankQuantiles(hist []int, samples int) (p10, p25, p50, p75, p90 float64) {
	if samples <= 0 {
		return 0, 0, 0, 0, 0
	}
	getQuantile := func(q float64) float64 {
		targetCount := float64(samples) * q
		cum := 0
		maxPos := 0
		for pos, c := range hist {
			if c > 0 {
				maxPos = pos
			}
			cum += c
			if float64(cum) >= targetCount {
				return float64(pos)
			}
		}
		return float64(maxPos)
	}
	return getQuantile(0.10), getQuantile(0.25), getQuantile(0.50), getQuantile(0.75), getQuantile(0.90)
}

func computeProbeRankMetrics(
	targetTeam int,
	frontierRank int,
	direction RareDirection,
	unresolvedCells map[[2]int]bool,
	hist []int,
) ProbeRankMetrics {
	tot := 0
	for _, c := range hist {
		tot += c
	}
	if tot == 0 {
		return ProbeRankMetrics{TargetTeam: targetTeam}
	}

	sumRank := 0.0
	minR, maxR := -1, -1
	for pos, c := range hist {
		if c > 0 {
			if minR == -1 {
				minR = pos
			}
			maxR = pos
			sumRank += float64(pos * c)
		}
	}

	p10, p25, p50, p75, p90 := computeRankQuantiles(hist, tot)

	r := frontierRank
	frontierCount := 0
	near1Count, near2Count, near3Count := 0, 0, 0

	for pos, c := range hist {
		if pos == r {
			frontierCount += c
		}
		dist := pos - r
		if dist < 0 {
			dist = -dist
		}
		if dist <= 1 {
			near1Count += c
		}
		if dist <= 2 {
			near2Count += c
		}
		if dist <= 3 {
			near3Count += c
		}
	}

	unresolvedSupport := 0
	for pos, c := range hist {
		cell := [2]int{targetTeam, pos}
		if unresolvedCells[cell] && float64(c)/float64(tot) >= 0.005 {
			unresolvedSupport++
		}
	}

	return ProbeRankMetrics{
		TargetTeam:        targetTeam,
		Samples:           tot,
		MeanRank:          sumRank / float64(tot),
		MinRank:           minR,
		MaxRank:           maxR,
		P10:               p10,
		P25:               p25,
		P50:               p50,
		P75:               p75,
		P90:               p90,
		FrontierMass:      float64(frontierCount) / float64(tot),
		Near1Mass:         float64(near1Count) / float64(tot),
		Near2Mass:         float64(near2Count) / float64(tot),
		Near3Mass:         float64(near3Count) / float64(tot),
		UnresolvedSupport: unresolvedSupport,
	}
}

func discoverDirectionalTails(scout ScoutData, teamGroups []TeamType) []DirectionalTail {
	var tails []DirectionalTail
	numPositions := len(teamGroups)

	for _, team := range teamGroups {
		id := team.Team_id
		minObserved := scout.MinObservedRank[id]
		maxObserved := scout.MaxObservedRank[id]
		hist := scout.TeamCounts[id]
		p10, p25, p50, p75, p90 := computeRankQuantiles(hist, scout.Samples)

		// Better tail (positions < minObserved)
		var betterUnseen []int
		firstBetter := -1
		for pos := minObserved - 1; pos >= 0; pos-- {
			if scout.Feasibility[[2]int{id, pos}] == "feasible_unseen" {
				betterUnseen = append(betterUnseen, pos)
				if firstBetter == -1 {
					firstBetter = pos
				}
			}
		}
		if len(betterUnseen) > 0 {
			frontier := minObserved - 1
			extremity := scout.TeamMeanRanks[id] - float64(firstBetter)
			priority := extremity * float64(len(betterUnseen))
			tails = append(tails, DirectionalTail{
				TeamID:                  id,
				Direction:               RareBetter,
				ObservedMinRank:         minObserved,
				ObservedMaxRank:         maxObserved,
				FrontierRank:            frontier,
				FirstUnseenPosition:     firstBetter,
				UnseenFeasiblePositions: betterUnseen,
				UnseenFeasibleCount:     len(betterUnseen),
				ScoutMeanRank:           scout.TeamMeanRanks[id],
				ScoutP10:                p10, ScoutP25: p25, ScoutP50: p50, ScoutP75: p75, ScoutP90: p90,
				ExtremityScore:          extremity,
				Priority:                priority,
			})
		}

		// Worse tail (positions > maxObserved)
		var worseUnseen []int
		firstWorse := -1
		for pos := maxObserved + 1; pos < numPositions; pos++ {
			if scout.Feasibility[[2]int{id, pos}] == "feasible_unseen" {
				worseUnseen = append(worseUnseen, pos)
				if firstWorse == -1 {
					firstWorse = pos
				}
			}
		}
		if len(worseUnseen) > 0 {
			frontier := maxObserved + 1
			extremity := float64(firstWorse) - scout.TeamMeanRanks[id]
			priority := extremity * float64(len(worseUnseen))
			tails = append(tails, DirectionalTail{
				TeamID:                  id,
				Direction:               RareWorse,
				ObservedMinRank:         minObserved,
				ObservedMaxRank:         maxObserved,
				FrontierRank:            frontier,
				FirstUnseenPosition:     firstWorse,
				UnseenFeasiblePositions: worseUnseen,
				UnseenFeasibleCount:     len(worseUnseen),
				ScoutMeanRank:           scout.TeamMeanRanks[id],
				ScoutP10:                p10, ScoutP25: p25, ScoutP50: p50, ScoutP75: p75, ScoutP90: p90,
				ExtremityScore:          extremity,
				Priority:                priority,
			})
		}
	}

	// Sort tails by priority descending
	sort.Slice(tails, func(i, j int) bool {
		if tails[i].Priority != tails[j].Priority {
			return tails[i].Priority > tails[j].Priority
		}
		return tails[i].ExtremityScore > tails[j].ExtremityScore
	})

	return tails
}

func probeProposal(
	proposal DiversifiedProposal,
	baseCampaign []*TeamCampaign,
	games []*GameType,
	originalMeans []GameProposalMeans,
	table *Table,
	sortOrder []SortType,
	teamGroups []TeamType,
	probeWork int64,
	rng *rand.Rand,
	targetTeam int,
	frontierRank int,
	unresolvedCells map[[2]int]bool,
	scout ScoutData,
) ProposalProbeStats {
	samples := int(probeWork / proposal.WorkPerSample)
	if samples < 150 {
		samples = 200 // Default 200 proposal Q samples for robust quantiles
	}
	actualWork := int64(samples) * proposal.WorkPerSample

	stats := ProposalProbeStats{
		ProposalID:                    proposal.ID,
		Samples:                       samples,
		Work:                          actualWork,
		RankHistograms:                make(map[int][]int, len(teamGroups)),
		CellHits:                      make(map[[2]int]int),
		CellSumR:                      make(map[[2]int]float64),
		CellSumR2OverDen:              make(map[[2]int]float64),
		CellSumZ1:                     make(map[[2]int]float64),
		CellSumZ1Sq:                   make(map[[2]int]float64),
		CellSumZ2:                     make(map[[2]int]float64),
		CellSumZ2Sq:                   make(map[[2]int]float64),
		CellESS:                       make(map[[2]int]float64),
		CellPredictedRawVarPerSample: make(map[[2]int]float64),
		CellPredictedVarPerSample:     make(map[[2]int]float64),
		Metrics:                       make(map[int]ProbeRankMetrics),
	}

	for _, team := range teamGroups {
		stats.RankHistograms[team.Team_id] = make([]int, len(teamGroups))
	}

	// Pure Q simulation for proposal discovery
	activeMeans := proposal.Means
	if proposal.Kind == ProposalPlainMC {
		activeMeans = originalMeans
	}

	simCampaign := make([]*TeamCampaign, len(baseCampaign))
	teamSlice := make([]*TeamCampaign, len(teamGroups))

	for s := 0; s < samples; s++ {
		for k, v := range baseCampaign {
			if v != nil {
				simCampaign[k] = v.clone()
			} else {
				simCampaign[k] = nil
			}
		}

		logQOverP := 0.0

		for i, g := range games {
			if g.Played {
				continue
			}
			hScore := poissonRand(rng, activeMeans[i].Home)
			aScore := poissonRand(rng, activeMeans[i].Away)

			if proposal.Kind != ProposalPlainMC {
				logQOverP += logPoissonQOverP(hScore, originalMeans[i].Home, activeMeans[i].Home)
				logQOverP += logPoissonQOverP(aScore, originalMeans[i].Away, activeMeans[i].Away)
			}

			home, away := g.home_table_index, g.away_table_index
			if simCampaign[home] != nil {
				simCampaign[home].add_game(&GameType{g.Id, g.HomeId, g.AwayId, hScore, aScore, 0, 0, true, home, away})
			}
			if simCampaign[away] != nil {
				simCampaign[away].add_game(&GameType{g.Id, g.HomeId, g.AwayId, hScore, aScore, 0, 0, true, home, away})
			}
		}

		// Weight under pure Q draw: r = P/Q = exp(-logQOverP)
		r := 1.0
		if proposal.Kind != ProposalPlainMC {
			r = math.Exp(-logQOverP)
			if math.IsNaN(r) || math.IsInf(r, 0) {
				r = 0.0
			}
		}
		den := DiversifiedDefensiveMixtureEpsilon*r + (1.0 - DiversifiedDefensiveMixtureEpsilon)
		y1 := r
		y2 := (r * r) / den

		idx := 0
		for _, tg := range teamGroups {
			if c := simCampaign[table.Query(uint32(tg.Team_id))]; c != nil {
				teamSlice[idx] = c
				idx++
			}
		}
		sort.Sort(TeamCampaignSorted{t: teamSlice[:idx], sort: sortOrder, rng: rng})

		for rankPos, teamComp := range teamSlice[:idx] {
			teamID := teamComp.id
			stats.RankHistograms[teamID][rankPos]++
			cell := [2]int{teamID, rankPos}
			stats.CellHits[cell]++
			stats.CellSumR[cell] += y1
			stats.CellSumR2OverDen[cell] += y2
			stats.CellSumZ1[cell] += y1
			stats.CellSumZ1Sq[cell] += y1 * y1
			stats.CellSumZ2[cell] += y2
			stats.CellSumZ2Sq[cell] += y2 * y2
		}
	}

	// Compute exact production mixture M variance and predicted per-sample variance for every cell
	numTeams := len(teamGroups)
	eps := DiversifiedDefensiveMixtureEpsilon
	n := float64(samples)

	for _, team := range teamGroups {
		id := team.Team_id
		for pos := 0; pos < numTeams; pos++ {
			cell := [2]int{id, pos}
			scoutHits := 0
			if len(scout.TeamCounts[id]) > pos {
				scoutHits = scout.TeamCounts[id][pos]
			}
			pReg := (float64(scoutHits) + 0.5) / (float64(scout.Samples) + 1.0)
			refVar := pReg * (1.0 - pReg)

			if proposal.Kind == ProposalPlainMC {
				stats.CellPredictedRawVarPerSample[cell] = refVar
				stats.CellPredictedVarPerSample[cell] = refVar
				if scoutHits > 0 {
					stats.CellESS[cell] = float64(scoutHits)
				}
			} else {
				qHits := stats.CellHits[cell]
				if qHits == 0 {
					stats.CellPredictedRawVarPerSample[cell] = math.Inf(1)
					stats.CellPredictedVarPerSample[cell] = math.Inf(1)
				} else {
					sumZ1 := stats.CellSumZ1[cell]
					sumZ2 := stats.CellSumZ2[cell]
					sumZ2Sq := stats.CellSumZ2Sq[cell]

					muHat := sumZ1 / n
					meanZ2 := sumZ2 / n

					sampleVarZ2 := 0.0
					if samples > 1 {
						sampleVarZ2 = (sumZ2Sq - n*meanZ2*meanZ2) / (n - 1.0)
						if sampleVarZ2 < 0 {
							sampleVarZ2 = 0.0
						}
					}

					seMeanZ2 := math.Sqrt(sampleVarZ2 / n)
					m2Upper := meanZ2 + 1.96*seMeanZ2

					rawVar := math.Max(1e-18, meanZ2-muHat*muHat)
					stats.CellPredictedRawVarPerSample[cell] = rawVar

					consVar := 1e-18
					if pReg < 1e-2 {
						consVar = math.Max(1e-18, m2Upper)
					} else {
						consVar = math.Max(1e-18, m2Upper-muHat*muHat)
					}
					stats.CellPredictedVarPerSample[cell] = consVar

					if rawVar > 0 {
						stats.CellESS[cell] = (muHat * muHat) / (rawVar + muHat*muHat*eps)
					}
				}
			}
		}
	}

	// Compute probe rank metrics for target team
	if targetTeam > 0 {
		stats.Metrics[targetTeam] = computeProbeRankMetrics(
			targetTeam, frontierRank, proposal.Direction, unresolvedCells, stats.RankHistograms[targetTeam])
	}

	return stats
}

func selectCompetitorsForTail(
	targetTeamID int,
	direction RareDirection,
	frontierRank int,
	scout ScoutData,
	teamGroups []TeamType,
	limit int,
) []int {
	type candidate struct {
		id           int
		corridorMass float64
		meanDist     float64
	}
	var candidates []candidate
	numTeams := len(teamGroups)

	rMin := frontierRank - 2
	if rMin < 0 {
		rMin = 0
	}
	rMax := frontierRank + 2
	if rMax >= numTeams {
		rMax = numTeams - 1
	}

	for _, team := range teamGroups {
		id := team.Team_id
		if id == targetTeamID {
			continue
		}
		counts := scout.TeamCounts[id]
		corridorCount := 0
		for p := rMin; p <= rMax; p++ {
			if p >= 0 && p < len(counts) {
				corridorCount += counts[p]
			}
		}
		cMass := float64(corridorCount) / float64(scout.Samples)
		meanDist := math.Abs(scout.TeamMeanRanks[id] - float64(frontierRank))

		candidates = append(candidates, candidate{
			id:           id,
			corridorMass: cMass,
			meanDist:     meanDist,
		})
	}

	sort.Slice(candidates, func(i, j int) bool {
		if candidates[i].corridorMass != candidates[j].corridorMass {
			return candidates[i].corridorMass > candidates[j].corridorMass
		}
		return candidates[i].meanDist < candidates[j].meanDist
	})

	if limit > len(candidates) {
		limit = len(candidates)
	}
	res := make([]int, limit)
	for i := 0; i < limit; i++ {
		res[i] = candidates[i].id
	}
	return res
}

func buildDirectTeamProposal(
	targetTeamID int,
	direction RareDirection,
	strength float64,
	original []GameProposalMeans,
	games []*GameType,
	teamIDs []int,
	numTeams int,
) (DiversifiedProposal, bool) {
	s := strength
	if direction == RareWorse {
		s = -strength
	}
	attackTheta := make(map[int]float64, len(teamIDs))
	concedeTheta := make(map[int]float64, len(teamIDs))
	for _, id := range teamIDs {
		attackTheta[id] = 0.0
		concedeTheta[id] = 0.0
	}
	attackTheta[targetTeamID] = s / 2.0
	concedeTheta[targetTeamID] = -s / 2.0

	cemProp := CEMProposal{
		Parameterization: CEMTeamAttackConcession,
		AttackTheta:      attackTheta,
		ConcessionTheta:  concedeTheta,
	}
	cemProp = normalizeCEMGauge(cemProp)
	means := materializeCEMProposal(cemProp, original, games)
	kl := cemTotalKL(means, original, games)

	if math.IsNaN(kl) || math.IsInf(kl, 0) || kl > DiversifiedMaxKL {
		return DiversifiedProposal{}, false
	}

	unplayedGames := 0
	for _, g := range games {
		if !g.Played {
			unplayedGames++
		}
	}
	workPerSample := estimateSeasonWork(unplayedGames, 2, numTeams)

	id := fmt.Sprintf("team%d_%s_s%.2f", targetTeamID, directionName(direction), strength)
	return DiversifiedProposal{
		ID:            id,
		Kind:          ProposalSingleTeam,
		TargetTeam:    targetTeamID,
		Direction:     direction,
		Strength:      strength,
		Means:         means,
		KL:            kl,
		WorkPerSample: workPerSample,
	}, true
}

func buildCompetitorAssistedProposal(
	targetTeamID int,
	direction RareDirection,
	strength float64,
	competitors []int,
	competitorMass float64,
	normalMeanRanks map[int]float64,
	original []GameProposalMeans,
	games []*GameType,
	teamIDs []int,
	numTeams int,
) (DiversifiedProposal, bool) {
	if len(competitors) == 0 {
		return DiversifiedProposal{}, false
	}

	targetSign := 1.0
	if direction == RareWorse {
		targetSign = -1.0
	}
	s := strength * targetSign

	attackTheta := make(map[int]float64, len(teamIDs))
	concedeTheta := make(map[int]float64, len(teamIDs))
	for _, id := range teamIDs {
		attackTheta[id] = 0.0
		concedeTheta[id] = 0.0
	}
	attackTheta[targetTeamID] = s / 2.0
	concedeTheta[targetTeamID] = -s / 2.0

	targetPos := normalMeanRanks[targetTeamID]
	rawWeights := make(map[int]float64, len(competitors))
	sumRaw := 0.0
	for _, compID := range competitors {
		w := 1.0 / (1.0 + math.Abs(normalMeanRanks[compID]-targetPos))
		rawWeights[compID] = w
		sumRaw += w
	}

	for _, compID := range competitors {
		normW := rawWeights[compID]
		if sumRaw > 0 {
			normW /= sumRaw
		}
		compSign := -targetSign
		compShift := strength * competitorMass * normW * compSign
		attackTheta[compID] = compShift / 2.0
		concedeTheta[compID] = -compShift / 2.0
	}

	cemProp := CEMProposal{
		Parameterization: CEMTeamAttackConcession,
		AttackTheta:      attackTheta,
		ConcessionTheta:  concedeTheta,
	}
	cemProp = normalizeCEMGauge(cemProp)
	means := materializeCEMProposal(cemProp, original, games)
	kl := cemTotalKL(means, original, games)

	if math.IsNaN(kl) || math.IsInf(kl, 0) || kl > DiversifiedMaxKL {
		return DiversifiedProposal{}, false
	}

	unplayedGames := 0
	for _, g := range games {
		if !g.Played {
			unplayedGames++
		}
	}
	workPerSample := estimateSeasonWork(unplayedGames, 2, numTeams)

	id := fmt.Sprintf("team%d_%s_s%.2f_comp%d", targetTeamID, directionName(direction), strength, len(competitors))
	return DiversifiedProposal{
		ID:            id,
		Kind:          ProposalCompetitorAssisted,
		TargetTeam:    targetTeamID,
		Direction:     direction,
		Strength:      strength,
		Competitors:   competitors,
		Means:         means,
		KL:            kl,
		WorkPerSample: workPerSample,
	}, true
}

func directionName(d RareDirection) string {
	if d == RareBetter {
		return "better"
	}
	return "worse"
}

type TailCoverageStatus int

const (
	TailUncovered TailCoverageStatus = iota
	TailPartiallyCovered
	TailFrontierCovered
	TailExhausted
)

type ProposalQuality struct {
	TailScore           float64
	DirectionalProgress float64
	QuantileProgress    float64
	FrontierDistance    float64
	FrontierMass        float64
	Near1Mass           float64
	Near2Mass           float64
	Near3Mass           float64
	UnresolvedSupport   int
	ShouldRetain        bool
	IsFrontierCovered   bool
	ShouldEscalate      bool
	IsOvershot          bool
	Reason              string
}

func evaluateProposalQuality(
	tail DirectionalTail,
	metrics ProbeRankMetrics,
) ProposalQuality {
	r := float64(tail.FrontierRank)

	directionalProgress := 0.0
	quantileProgress := 0.0
	frontierDistance := 0.0
	isOvershot := false

	if tail.Direction == RareBetter {
		directionalProgress = tail.ScoutMeanRank - metrics.MeanRank
		quantileProgress = tail.ScoutP25 - metrics.P25
		frontierDistance = metrics.MeanRank - r
		if metrics.P90 <= r {
			isOvershot = true
		}
	} else {
		directionalProgress = metrics.MeanRank - tail.ScoutMeanRank
		quantileProgress = metrics.P75 - tail.ScoutP75
		frontierDistance = r - metrics.MeanRank
		if metrics.P10 >= r {
			isOvershot = true
		}
	}

	isFrontierCovered := (metrics.FrontierMass >= 0.01 || metrics.Near1Mass >= 0.03 || metrics.Near2Mass >= 0.05)

	tailScore := 10.0*metrics.FrontierMass +
		5.0*metrics.Near1Mass +
		3.0*metrics.Near2Mass +
		1.0*metrics.Near3Mass +
		2.0*directionalProgress +
		1.5*quantileProgress +
		4.0*float64(metrics.UnresolvedSupport)

	shouldRetain := false
	reason := "insufficient_movement"

	if isFrontierCovered {
		shouldRetain = true
		reason = "frontier_mass_coverage"
	} else if metrics.Near2Mass >= 0.03 {
		shouldRetain = true
		reason = "near_frontier_mass_support"
	} else if metrics.UnresolvedSupport >= 2 {
		shouldRetain = true
		reason = "broad_unresolved_support"
	} else if tailScore >= 3.0 && directionalProgress >= 0.5 {
		shouldRetain = true
		reason = "useful_tail_shift"
	} else if metrics.Near1Mass > 0 && quantileProgress >= 1.0 {
		shouldRetain = true
		reason = "quantile_progress_near_frontier"
	}

	shouldEscalate := false
	if !shouldRetain && (tailScore < 1.5 || directionalProgress < 0.3) && !isOvershot {
		shouldEscalate = true
		reason = "frontier_too_far"
	}

	return ProposalQuality{
		TailScore:           tailScore,
		DirectionalProgress: directionalProgress,
		QuantileProgress:    quantileProgress,
		FrontierDistance:    frontierDistance,
		FrontierMass:        metrics.FrontierMass,
		Near1Mass:           metrics.Near1Mass,
		Near2Mass:           metrics.Near2Mass,
		Near3Mass:           metrics.Near3Mass,
		UnresolvedSupport:   metrics.UnresolvedSupport,
		ShouldRetain:        shouldRetain,
		IsFrontierCovered:   isFrontierCovered,
		ShouldEscalate:      shouldEscalate,
		IsOvershot:          isOvershot,
		Reason:              reason,
	}
}

func proposalHistogramTVD(h1, h2 []int, s1, s2 int) float64 {
	if s1 <= 0 || s2 <= 0 {
		return 1.0
	}
	sum := 0.0
	for i := 0; i < len(h1) && i < len(h2); i++ {
		p1 := float64(h1[i]) / float64(s1)
		p2 := float64(h2[i]) / float64(s2)
		diff := p1 - p2
		if diff < 0 {
			diff = -diff
		}
		sum += diff
	}
	return 0.5 * sum
}

func searchDiversifiedProposals(
	scout ScoutData,
	baseCampaign []*TeamCampaign,
	games []*GameType,
	original []GameProposalMeans,
	table *Table,
	sortOrder []SortType,
	teamGroups []TeamType,
	searchWorkCap int64,
	rng *rand.Rand,
) ([]DiversifiedProposal, map[string]ProposalProbeStats, int64) {
	var retained []DiversifiedProposal
	probeStatsMap := make(map[string]ProposalProbeStats)

	unplayedGames := 0
	for _, g := range games {
		if !g.Played {
			unplayedGames++
		}
	}
	numTeams := len(teamGroups)
	plainWorkPerSample := estimateSeasonWork(unplayedGames, 1, numTeams)
	plainProp := DiversifiedProposal{
		ID:            "plain_mc",
		Kind:          ProposalPlainMC,
		Means:         original,
		KL:            0.0,
		WorkPerSample: plainWorkPerSample,
	}
	retained = append(retained, plainProp)

	plainStats := ProposalProbeStats{
		ProposalID:                "plain_mc",
		Samples:                   scout.Samples,
		Work:                      scout.Work,
		RankHistograms:            scout.TeamCounts,
		CellHits:                  make(map[[2]int]int),
		CellSumR:                  make(map[[2]int]float64),
		CellSumR2OverDen:          make(map[[2]int]float64),
		CellESS:                   make(map[[2]int]float64),
		CellPredictedVarPerSample: make(map[[2]int]float64),
		Metrics:                   make(map[int]ProbeRankMetrics),
	}
	for teamID, counts := range scout.TeamCounts {
		for pos, c := range counts {
			cell := [2]int{teamID, pos}
			plainStats.CellHits[cell] = c
			plainStats.CellSumR[cell] = float64(c)
			plainStats.CellSumR2OverDen[cell] = float64(c)
			if c > 0 {
				plainStats.CellESS[cell] = float64(c)
			}
			pReg := (float64(c) + 0.5) / (float64(scout.Samples) + 1.0)
			plainStats.CellPredictedVarPerSample[cell] = pReg * (1.0 - pReg)
		}
	}
	probeStatsMap["plain_mc"] = plainStats

	// Identify all unresolved cells (scout count == 0 OR predicted ESS < 10)
	unresolvedCells := make(map[[2]int]bool)
	for _, team := range teamGroups {
		id := team.Team_id
		for pos := 0; pos < numTeams; pos++ {
			cell := [2]int{id, pos}
			if scout.Feasibility[cell] != "proven_impossible" {
				if scout.TeamCounts[id][pos] < 10 {
					unresolvedCells[cell] = true
				}
			}
		}
	}

	tails := discoverDirectionalTails(scout, teamGroups)
	teamIDs := teamIDsFromGroups(teamGroups)

	workSpent := int64(0)
	strengthLadder := []float64{0.50, 0.75, 1.00, 1.50, 2.00, 0.25}

	tailsTotal := len(tails)
	tailsFrontierCoveredSingleTeam := 0
	tailsFrontierCoveredCompetitors := 0
	tailsPartiallyCovered := 0
	tailsExhausted := 0

	for _, tail := range tails {
		if workSpent >= searchWorkCap {
			break
		}

		retainedForTail := make([]DiversifiedProposal, 0)
		tailFrontierCovered := false

		for _, s := range strengthLadder {
			if workSpent >= searchWorkCap || len(retainedForTail) >= DiversifiedMaxSingleTeamPerDirection {
				break
			}

			prop, ok := buildDirectTeamProposal(tail.TeamID, tail.Direction, s, original, games, teamIDs, numTeams)
			if !ok {
				log.Printf("rare-position-candidate: team=%d dir=%s frontier=%d type=single_team strength=%.2f decision=reject reason=KL_limit",
					tail.TeamID, directionName(tail.Direction), tail.FrontierRank, s)
				continue
			}

			probeWork := int64(200 * prop.WorkPerSample)
			if workSpent+probeWork > searchWorkCap {
				probeWork = searchWorkCap - workSpent
			}
			if probeWork < 10*prop.WorkPerSample {
				break
			}

			stats := probeProposal(prop, baseCampaign, games, original, table, sortOrder, teamGroups, probeWork, rng, tail.TeamID, tail.FrontierRank, unresolvedCells, scout)
			workSpent += stats.Work

			m := stats.Metrics[tail.TeamID]
			q := evaluateProposalQuality(tail, m)

			log.Printf("rare-position-candidate: team=%d dir=%s frontier=%d type=single_team strength=%.2f KL=%.3f samples=%d work=%d mean_rank=%.2f p10=%.1f p25=%.1f p50=%.1f p75=%.1f p90=%.1f frontier_mass=%.3f near1_mass=%.3f near2_mass=%.3f unresolved_support=%d tail_score=%.2f frontier_covered=%t decision=%s reason=%s",
				tail.TeamID, directionName(tail.Direction), tail.FrontierRank, s, prop.KL, stats.Samples, stats.Work,
				m.MeanRank, m.P10, m.P25, m.P50, m.P75, m.P90, m.FrontierMass, m.Near1Mass, m.Near2Mass, m.UnresolvedSupport, q.TailScore, q.IsFrontierCovered,
				map[bool]string{true: "retain", false: "escalate"}[q.ShouldRetain], q.Reason)

			if q.IsFrontierCovered {
				tailFrontierCovered = true
			}

			if q.ShouldRetain {
				// Deduplicate against already retained proposals for this tail
				isDuplicate := false
				for _, prev := range retainedForTail {
					prevStats := probeStatsMap[prev.ID]
					tvd := proposalHistogramTVD(stats.RankHistograms[tail.TeamID], prevStats.RankHistograms[tail.TeamID], stats.Samples, prevStats.Samples)
					if tvd < 0.15 {
						isDuplicate = true
						log.Printf("rare-position-candidate: team=%d dir=%s prop=%s decision=skip_duplicate_tvd tvd=%.3f",
							tail.TeamID, directionName(tail.Direction), prop.ID, tvd)
						break
					}
				}

				if !isDuplicate {
					retained = append(retained, prop)
					probeStatsMap[prop.ID] = stats
					retainedForTail = append(retainedForTail, prop)
				}
			}

			if q.IsOvershot || len(retainedForTail) >= DiversifiedMaxSingleTeamPerDirection {
				break
			}
		}

		// Competitor Escalation if frontier was NOT covered by target-only search
		competitorCovered := false
		if !tailFrontierCovered && workSpent < searchWorkCap {
			competitors := selectCompetitorsForTail(tail.TeamID, tail.Direction, tail.FrontierRank, scout, teamGroups, DiversifiedMaxCompetitors)
			if len(competitors) > 0 {
				for _, s := range []float64{0.75, 1.25} {
					if workSpent >= searchWorkCap || len(retainedForTail) >= DiversifiedMaxSingleTeamPerDirection+1 {
						break
					}

					prop, ok := buildCompetitorAssistedProposal(tail.TeamID, tail.Direction, s, competitors, 0.50, scout.TeamMeanRanks, original, games, teamIDs, numTeams)
					if !ok {
						log.Printf("rare-position-candidate: team=%d dir=%s frontier=%d type=competitor strength=%.2f decision=reject reason=KL_limit",
							tail.TeamID, directionName(tail.Direction), tail.FrontierRank, s)
						continue
					}

					probeWork := int64(200 * prop.WorkPerSample)
					if workSpent+probeWork > searchWorkCap {
						probeWork = searchWorkCap - workSpent
					}
					if probeWork < 10*prop.WorkPerSample {
						break
					}

					stats := probeProposal(prop, baseCampaign, games, original, table, sortOrder, teamGroups, probeWork, rng, tail.TeamID, tail.FrontierRank, unresolvedCells, scout)
					workSpent += stats.Work

					m := stats.Metrics[tail.TeamID]
					q := evaluateProposalQuality(tail, m)

					log.Printf("rare-position-candidate: team=%d dir=%s frontier=%d type=competitor strength=%.2f competitors=%v KL=%.3f samples=%d work=%d mean_rank=%.2f p10=%.1f p25=%.1f p50=%.1f p75=%.1f p90=%.1f frontier_mass=%.3f near1_mass=%.3f near2_mass=%.3f unresolved_support=%d tail_score=%.2f frontier_covered=%t decision=%s reason=%s",
						tail.TeamID, directionName(tail.Direction), tail.FrontierRank, s, competitors, prop.KL, stats.Samples, stats.Work,
						m.MeanRank, m.P10, m.P25, m.P50, m.P75, m.P90, m.FrontierMass, m.Near1Mass, m.Near2Mass, m.UnresolvedSupport, q.TailScore, q.IsFrontierCovered,
						map[bool]string{true: "retain_competitor", false: "escalate"}[q.ShouldRetain], q.Reason)

					if q.IsFrontierCovered {
						tailFrontierCovered = true
						competitorCovered = true
					}

					if q.ShouldRetain {
						retained = append(retained, prop)
						probeStatsMap[prop.ID] = stats
						retainedForTail = append(retainedForTail, prop)
					}
				}
			}
		}

		if tailFrontierCovered {
			if competitorCovered {
				tailsFrontierCoveredCompetitors++
			} else {
				tailsFrontierCoveredSingleTeam++
			}
		} else if len(retainedForTail) > 0 {
			tailsPartiallyCovered++
		} else {
			tailsExhausted++
			log.Printf("rare-position-diversified-search: team=%d direction=%s decision=search_exhausted",
				tail.TeamID, directionName(tail.Direction))
		}
	}

	log.Printf("rare-position-diversified-search-summary: tails_total=%d tails_frontier_covered_single_team=%d tails_frontier_covered_competitors=%d tails_partially_covered=%d tails_search_exhausted=%d proposals_retained=%d search_work_spent=%d search_work_cap=%d",
		tailsTotal, tailsFrontierCoveredSingleTeam, tailsFrontierCoveredCompetitors, tailsPartiallyCovered, tailsExhausted, len(retained), workSpent, searchWorkCap)

	return retained, probeStatsMap, workSpent
}

func regularizedScoutProbability(scout ScoutData, cell [2]int) float64 {
	scoutHits := 0
	if len(scout.TeamCounts[cell[0]]) > cell[1] {
		scoutHits = scout.TeamCounts[cell[0]][cell[1]]
	}
	pReg := (float64(scoutHits) + 0.5) / (float64(scout.Samples) + 1.0)
	if pReg < 1e-12 {
		pReg = 1e-12
	}
	return pReg
}

func diversifiedChunkUtility(
	proposal DiversifiedProposal,
	chunkSamples int,
	chunkWork int64,
	feasibleCells [][2]int,
	predictedRelESS map[[2]int]float64,
	pReg map[[2]int]float64,
	precPerSample map[string]map[[2]int]float64,
	targetESS float64,
) float64 {
	if chunkWork <= 0 || chunkSamples <= 0 {
		return 0.0
	}

	totRelESSGain := 0.0
	propPrecMap := precPerSample[proposal.ID]

	for _, cell := range feasibleCells {
		p := pReg[cell]
		currentESS := predictedRelESS[cell]
		deficitESS := math.Max(0, targetESS-currentESS)
		if deficitESS <= 0 {
			continue
		}

		prec := propPrecMap[cell]
		if prec <= 0 || math.IsNaN(prec) || math.IsInf(prec, 0) {
			continue
		}

		deltaPrec := float64(chunkSamples) * prec
		deltaRelESS := (p * p) * deltaPrec
		gain := math.Min(deficitESS, deltaRelESS)
		totRelESSGain += gain
	}

	return totRelESSGain / float64(chunkWork)
}

func freezeDiversifiedDesign(
	proposals []DiversifiedProposal,
	probeStats map[string]ProposalProbeStats,
	scout ScoutData,
	teamGroups []TeamType,
	totalWorkBudget int64,
	scoutWork int64,
	searchWork int64,
) FrozenDiversifiedDesign {
	numPositions := len(teamGroups)

	productionWorkBudget := totalWorkBudget - scoutWork - searchWork
	if productionWorkBudget < 0 {
		productionWorkBudget = 0
	}

	plainProp := proposals[0]
	// 1. Minimum Plain-MC share
	minPlainWork := int64(float64(productionWorkBudget) * diversifiedMinPlainFraction())
	allocatedWork := make(map[string]int64, len(proposals))
	allocatedWork["plain_mc"] = minPlainWork

	remainingProdWork := productionWorkBudget - minPlainWork

	// 2. Compute predicted per-sample precision prec = 1/varPerSample for each proposal per cell (conservative and raw)
	precPerSample := make(map[string]map[[2]int]float64, len(proposals))
	precPerSampleRaw := make(map[string]map[[2]int]float64, len(proposals))

	for _, prop := range proposals {
		stats := probeStats[prop.ID]
		pMap := make(map[[2]int]float64)
		pMapRaw := make(map[[2]int]float64)

		for cell, varPerSample := range stats.CellPredictedVarPerSample {
			if !math.IsNaN(varPerSample) && !math.IsInf(varPerSample, 0) && varPerSample > 0 {
				pMap[cell] = 1.0 / varPerSample
			}
		}
		for cell, rawVar := range stats.CellPredictedRawVarPerSample {
			if !math.IsNaN(rawVar) && !math.IsInf(rawVar, 0) && rawVar > 0 {
				pMapRaw[cell] = 1.0 / rawVar
			}
		}

		precPerSample[prop.ID] = pMap
		precPerSampleRaw[prop.ID] = pMapRaw
	}

	// 3. Greedy Marginal-Utility Allocation based on predicted precision gain
	feasibleCells := make([][2]int, 0)
	for _, team := range teamGroups {
		for pos := 0; pos < numPositions; pos++ {
			cell := [2]int{team.Team_id, pos}
			if scout.Feasibility[cell] != "proven_impossible" {
				feasibleCells = append(feasibleCells, cell)
			}
		}
	}

	pRegMap := make(map[[2]int]float64, len(feasibleCells))
	for _, cell := range feasibleCells {
		pRegMap[cell] = regularizedScoutProbability(scout, cell)
	}

	// Initial deficit summary logging after mandatory Plain P allocation
	initPrec := make(map[[2]int]float64, len(feasibleCells))
	initRelESSMap := make(map[[2]int]float64, len(feasibleCells))
	initRelESSList := make([]float64, 0, len(feasibleCells))
	zeroDeficitCells, posDeficitCells := 0, 0

	for _, cell := range feasibleCells {
		pReg := pRegMap[cell]
		workP := allocatedWork["plain_mc"]
		samplesP := workP / plainProp.WorkPerSample
		precP := float64(samplesP) * precPerSample["plain_mc"][cell]
		initPrec[cell] = precP

		relESS := (pReg * pReg) * precP
		initRelESSMap[cell] = relESS
		initRelESSList = append(initRelESSList, relESS)

		if relESS >= DiversifiedTargetESS {
			zeroDeficitCells++
		} else {
			posDeficitCells++
		}
	}

	medianRelESS, p10RelESS, p90RelESS := 0.0, 0.0, 0.0
	if len(initRelESSList) > 0 {
		sort.Float64s(initRelESSList)
		medianRelESS = initRelESSList[len(initRelESSList)/2]
		p10RelESS = initRelESSList[int(float64(len(initRelESSList)-1)*0.10)]
		p90RelESS = initRelESSList[int(float64(len(initRelESSList)-1)*0.90)]
	}

	log.Printf("rare-position-diversified-initial-deficits: feasible_cells=%d cells_zero_deficit=%d cells_pos_deficit=%d median_rel_ess_predicted=%.4f p10_rel_ess=%.4f p90_rel_ess=%.4f min_plain_work=%d",
		len(feasibleCells), zeroDeficitCells, posDeficitCells, medianRelESS, p10RelESS, p90RelESS, minPlainWork)

	// First-round winner comparison logging: raw vs conservative variance model
	chunkSamplesInit := int(DiversifiedAllocChunkWork / plainProp.WorkPerSample)
	actualChunkWorkInit := int64(chunkSamplesInit) * plainProp.WorkPerSample

	rawWinnerID, consWinnerID := "", ""
	rawWinnerUtil, consWinnerUtil := -1.0, -1.0

	for _, prop := range proposals {
		uRaw := diversifiedChunkUtility(prop, chunkSamplesInit, actualChunkWorkInit, feasibleCells, initRelESSMap, pRegMap, precPerSampleRaw, DiversifiedTargetESS)
		uCons := diversifiedChunkUtility(prop, chunkSamplesInit, actualChunkWorkInit, feasibleCells, initRelESSMap, pRegMap, precPerSample, DiversifiedTargetESS)

		if uRaw > rawWinnerUtil {
			rawWinnerUtil = uRaw
			rawWinnerID = prop.ID
		}
		if uCons > consWinnerUtil {
			consWinnerUtil = uCons
			consWinnerID = prop.ID
		}
	}

	log.Printf("rare-position-diversified-alloc-first-round-comparison: raw_winner=%s raw_util=%.6e conservative_winner=%s conservative_util=%.6e",
		rawWinnerID, rawWinnerUtil, consWinnerID, consWinnerUtil)

	// Sample adequacy per retained proposal
	for _, prop := range proposals {
		stats := probeStats[prop.ID]
		qGe1, qGe3, qGe5, qGe10 := 0, 0, 0, 0
		for _, cell := range feasibleCells {
			hits := stats.CellHits[cell]
			if hits >= 1 {
				qGe1++
			}
			if hits >= 3 {
				qGe3++
			}
			if hits >= 5 {
				qGe5++
			}
			if hits >= 10 {
				qGe10++
			}
		}
		log.Printf("rare-position-diversified-proposal-adequacy: prop_id=%s kind=%s q_hits_ge_1=%d q_hits_ge_3=%d q_hits_ge_5=%d q_hits_ge_10=%d",
			prop.ID, prop.Kind, qGe1, qGe3, qGe5, qGe10)
	}

	roundCount := 0
	allocChunk := int64(DiversifiedAllocChunkWork)
	for remainingProdWork >= allocChunk {
		roundCount++

		// Compute current predicted relative ESS per cell
		predictedRelESS := make(map[[2]int]float64, len(feasibleCells))
		for _, cell := range feasibleCells {
			p := pRegMap[cell]
			totPrec := 0.0
			for _, prop := range proposals {
				work := allocatedWork[prop.ID]
				samples := work / prop.WorkPerSample
				totPrec += float64(samples) * precPerSample[prop.ID][cell]
			}
			predictedRelESS[cell] = (p * p) * totPrec
		}

		type propUtil struct {
			id      string
			utility float64
		}
		var propUtils []propUtil

		for _, prop := range proposals {
			chunkSamples := int(allocChunk / prop.WorkPerSample)
			actualChunkWork := int64(chunkSamples) * prop.WorkPerSample
			if actualChunkWork <= 0 || actualChunkWork > remainingProdWork {
				continue
			}

			utility := diversifiedChunkUtility(
				prop, chunkSamples, actualChunkWork, feasibleCells,
				predictedRelESS, pRegMap, precPerSample, DiversifiedTargetESS)

			propUtils = append(propUtils, propUtil{id: prop.ID, utility: utility})
		}

		sort.Slice(propUtils, func(i, j int) bool {
			return propUtils[i].utility > propUtils[j].utility
		})

		bestPropID := ""
		bestUtility := -1.0
		secondBestPropID := ""
		secondBestUtility := -1.0

		if len(propUtils) > 0 {
			bestPropID = propUtils[0].id
			bestUtility = propUtils[0].utility
		}
		if len(propUtils) > 1 {
			secondBestPropID = propUtils[1].id
			secondBestUtility = propUtils[1].utility
		}

		if roundCount <= 10 {
			log.Printf("rare-position-diversified-alloc-round: round=%d best_prop=%s best_rel_ess_gain_per_work=%.6e second_prop=%s second_rel_ess_gain_per_work=%.6e remaining_work=%d",
				roundCount, bestPropID, bestUtility, secondBestPropID, secondBestUtility, remainingProdWork)
		}

		if bestPropID == "" || bestUtility <= 0 {
			// Deficits saturated or no proposal offers utility; allocate rest to plain MC
			allocatedWork["plain_mc"] += remainingProdWork
			remainingProdWork = 0
			break
		}

		// Allocate chunk to best proposal
		prop := findProposalByID(proposals, bestPropID)
		chunkSamples := int(allocChunk / prop.WorkPerSample)
		actualChunkWork := int64(chunkSamples) * prop.WorkPerSample
		allocatedWork[bestPropID] += actualChunkWork
		remainingProdWork -= actualChunkWork
	}

	// Any small leftover unallocated production work goes to plain MC
	if remainingProdWork > 0 {
		allocatedWork["plain_mc"] += remainingProdWork
		remainingProdWork = 0
	}

	targetedWork := int64(0)
	for _, prop := range proposals {
		work := allocatedWork[prop.ID]
		samples := work / prop.WorkPerSample

		if prop.Kind != ProposalPlainMC {
			targetedWork += work
		}

		posPrecCells := 0
		posDeficitGainCells := 0
		totPredictedUtil := 0.0

		for _, cell := range feasibleCells {
			prec := precPerSample[prop.ID][cell]
			p := pRegMap[cell]
			if prec > 0 {
				posPrecCells++
			}
			initESS := (p * p) * initPrec[cell]
			deficitESS := math.Max(0, DiversifiedTargetESS-initESS)
			if prec > 0 && deficitESS > 0 {
				posDeficitGainCells++
				totPredictedUtil += math.Min(deficitESS, (p*p)*float64(samples)*prec)
			}
		}

		log.Printf("rare-position-diversified-proposal-alloc: prop_id=%s kind=%s team=%d dir=%s strength=%.2f allocated_work=%d planned_samples=%d pos_prec_cells=%d pos_deficit_gain_cells=%d tot_predicted_rel_ess_gain=%.6e",
			prop.ID, prop.Kind, prop.TargetTeam, directionName(prop.Direction), prop.Strength, work, samples, posPrecCells, posDeficitGainCells, totPredictedUtil)

		if prop.Kind != ProposalPlainMC {
			type cellGainInfo struct {
				team, pos           int
				pReg                float64
				initRelESS          float64
				deltaRelESSPerChunk float64
			}
			var topCells []cellGainInfo
			for _, cell := range feasibleCells {
				prec := precPerSample[prop.ID][cell]
				p := pRegMap[cell]
				if prec > 0 {
					initE := (p * p) * initPrec[cell]
					deltaE := (p * p) * prec * float64(allocChunk/prop.WorkPerSample)
					topCells = append(topCells, cellGainInfo{
						team: cell[0], pos: cell[1], pReg: p, initRelESS: initE, deltaRelESSPerChunk: deltaE,
					})
				}
			}
			sort.Slice(topCells, func(i, j int) bool {
				return topCells[i].deltaRelESSPerChunk > topCells[j].deltaRelESSPerChunk
			})
			if len(topCells) > 5 {
				topCells = topCells[:5]
			}
			for _, tc := range topCells {
				log.Printf("  top_cell: prop_id=%s team=%d rank=%d pReg=%.3e init_rel_ess=%.2f delta_rel_ess_per_chunk=%.3f",
					prop.ID, tc.team, tc.pos, tc.pReg, tc.initRelESS, tc.deltaRelESSPerChunk)
			}
		}
	}

	if len(proposals) > 1 && targetedWork == 0 {
		log.Printf("rare-position-diversified-warning: %d targeted proposal(s) were retained during search, but freeze allocated 100%% work to plain_mc! pos_deficits=%d",
			len(proposals)-1, posDeficitCells)

		bestTargetedProp := proposals[1]
		statsP := probeStats["plain_mc"]
		statsQ := probeStats[bestTargetedProp.ID]

		log.Printf("rare-position-diversified-allocation-pathology: comparing plain_mc vs %s:", bestTargetedProp.ID)
		topCount := 0
		for _, cell := range feasibleCells {
			p := pRegMap[cell]
			def := math.Max(0, DiversifiedTargetESS-initRelESSMap[cell])
			if def > 0 && topCount < 20 {
				topCount++
				qHits := statsQ.CellHits[cell]
				plainVar := statsP.CellPredictedVarPerSample[cell]
				rawVar := statsQ.CellPredictedRawVarPerSample[cell]
				consVar := statsQ.CellPredictedVarPerSample[cell]

				n := float64(statsQ.Samples)
				meanZ2 := statsQ.CellSumZ2[cell] / n
				sampleVarZ2 := 0.0
				if statsQ.Samples > 1 {
					sampleVarZ2 = (statsQ.CellSumZ2Sq[cell] - n*meanZ2*meanZ2) / (n - 1.0)
					if sampleVarZ2 < 0 {
						sampleVarZ2 = 0
					}
				}
				seM2 := math.Sqrt(sampleVarZ2 / n)
				m2Upper := meanZ2 + 1.96*seM2
				seFactor := 0.0
				if meanZ2 > 0 {
					seFactor = (1.96 * seM2) / meanZ2
				}

				gainP := (p * p) * (1.0 / plainVar) / float64(plainProp.WorkPerSample)
				gainRawQ := (p * p) * (1.0 / rawVar) / float64(bestTargetedProp.WorkPerSample)
				gainConsQ := (p * p) * (1.0 / consVar) / float64(bestTargetedProp.WorkPerSample)

				log.Printf("  cell=(team:%d,pos:%d) qHits=%d pReg=%.3e defRelESS=%.3f rawM2=%.3e seM2=%.3e m2Upper=%.3e seFactor=%.2f rawVar=%.3e consVar=%.3e plainVar=%.3e gainP/work=%.3e gainRawQ/work=%.3e gainConsQ/work=%.3e",
					cell[0], cell[1], qHits, p, def, meanZ2, seM2, m2Upper, seFactor, rawVar, consVar, plainVar, gainP, gainRawQ, gainConsQ)
			}
		}
	}

	// Build FrozenProductionBatch
	batches := make([]FrozenProductionBatch, 0)
	actualProductionWork := int64(0)
	for _, prop := range proposals {
		work := allocatedWork[prop.ID]
		if work <= 0 {
			continue
		}
		samples := int(work / prop.WorkPerSample)
		if samples <= 0 {
			continue
		}
		actualWork := int64(samples) * prop.WorkPerSample
		actualProductionWork += actualWork
		batches = append(batches, FrozenProductionBatch{
			Proposal:      prop,
			Samples:       samples,
			Work:          actualWork,
			WorkPerSample: prop.WorkPerSample,
		})
	}

	// 4. Compute statistically optimal inverse-variance combination weights (beta_j,A) using planned batch sample counts
	cellBetas := make(map[[2]int][]float64, len(feasibleCells))
	for _, cell := range feasibleCells {
		betas := make([]float64, len(batches))
		sumScore := 0.0
		for j, batch := range batches {
			stats := probeStats[batch.Proposal.ID]
			varPerSample := stats.CellPredictedVarPerSample[cell]
			score := 0.0
			if !math.IsNaN(varPerSample) && !math.IsInf(varPerSample, 0) && varPerSample > 0 && batch.Samples > 0 {
				score = float64(batch.Samples) / varPerSample
			}
			betas[j] = score
			sumScore += score
		}

		if sumScore > 0 {
			for j := range betas {
				betas[j] /= sumScore
			}
		} else {
			// Fallback: 100% plain MC
			for j, batch := range batches {
				if batch.Proposal.Kind == ProposalPlainMC {
					betas[j] = 1.0
				} else {
					betas[j] = 0.0
				}
			}
		}

		cellBetas[cell] = betas
	}

	unusedWork := totalWorkBudget - scoutWork - searchWork - actualProductionWork

	predVarMap := make(map[string]map[[2]int]float64, len(proposals))
	for _, prop := range proposals {
		stats := probeStats[prop.ID]
		predVarMap[prop.ID] = stats.CellPredictedVarPerSample
	}

	return FrozenDiversifiedDesign{
		Batches:                   batches,
		CellCombinationWeights:    cellBetas,
		CellPredictedVarPerSample: predVarMap,
		TotalWork:                 totalWorkBudget,
		ScoutWork:                 scoutWork,
		SearchWork:                searchWork,
		ProductionWork:            actualProductionWork,
		UnusedWork:                unusedWork,
	}
}

func findProposalByID(proposals []DiversifiedProposal, id string) DiversifiedProposal {
	for _, p := range proposals {
		if p.ID == id {
			return p
		}
	}
	return proposals[0]
}

func runDiversifiedProduction(
	design FrozenDiversifiedDesign,
	baseCampaign []*TeamCampaign,
	games []*GameType,
	originalMeans []GameProposalMeans,
	table *Table,
	sortOrder []SortType,
	teamGroups []TeamType,
	masterSeed int64,
) map[int]map[int]DiversifiedEstimate {
	numTeams := len(teamGroups)
	numPositions := len(teamGroups)
	numBatches := len(design.Batches)

	// Collect per-batch, per-cell statistics:
	// batchHits[j][cell], batchSumY[j][cell], batchSumY2[j][cell]
	batchHits := make([]map[[2]int]int, numBatches)
	batchSumY := make([]map[[2]int]float64, numBatches)
	batchSumY2 := make([]map[[2]int]float64, numBatches)

	for j, batch := range design.Batches {
		batchHits[j] = make(map[[2]int]int)
		batchSumY[j] = make(map[[2]int]float64)
		batchSumY2[j] = make(map[[2]int]float64)

		batchSeed := deriveRarePositionSeed(masterSeed, fmt.Sprintf("diversified-prod-batch-%d-%s", j, batch.Proposal.ID))
		rng := rand.New(rand.NewSource(batchSeed))

		components := []ProposalComponent{
			{Name: "P", Weight: DiversifiedDefensiveMixtureEpsilon, Means: originalMeans},
			{Name: "Q", Weight: 1.0 - DiversifiedDefensiveMixtureEpsilon, Means: batch.Proposal.Means},
		}
		compWeights := []float64{DiversifiedDefensiveMixtureEpsilon, 1.0 - DiversifiedDefensiveMixtureEpsilon}
		if batch.Proposal.Kind == ProposalPlainMC {
			components = []ProposalComponent{{Name: "P", Weight: 1.0, Means: originalMeans}}
			compWeights = []float64{1.0}
		}

		simCampaign := make([]*TeamCampaign, len(baseCampaign))
		teamSlice := make([]*TeamCampaign, len(teamGroups))
		logQBuf := make([]float64, len(components))

		for s := 0; s < batch.Samples; s++ {
			for k, v := range baseCampaign {
				if v != nil {
					simCampaign[k] = v.clone()
				} else {
					simCampaign[k] = nil
				}
			}

			for i := range logQBuf {
				logQBuf[i] = 0.0
			}

			chosen := 0
			if len(components) > 1 {
				chosen = 1
				if rng.Float64() <= DiversifiedDefensiveMixtureEpsilon {
					chosen = 0
				}
			}
			activeMeans := components[chosen].Means

			for i, g := range games {
				if g.Played {
					continue
				}
				hScore := poissonRand(rng, activeMeans[i].Home)
				aScore := poissonRand(rng, activeMeans[i].Away)

				for k, component := range components {
					logQBuf[k] += logPoissonQOverP(hScore, originalMeans[i].Home, component.Means[i].Home)
					logQBuf[k] += logPoissonQOverP(aScore, originalMeans[i].Away, component.Means[i].Away)
				}

				home, away := g.home_table_index, g.away_table_index
				if simCampaign[home] != nil {
					simCampaign[home].add_game(&GameType{g.Id, g.HomeId, g.AwayId, hScore, aScore, 0, 0, true, home, away})
				}
				if simCampaign[away] != nil {
					simCampaign[away].add_game(&GameType{g.Id, g.HomeId, g.AwayId, hScore, aScore, 0, 0, true, home, away})
				}
			}

			w := mixtureImportanceWeightMulti(logQBuf, compWeights)

			idx := 0
			for _, tg := range teamGroups {
				if c := simCampaign[table.Query(uint32(tg.Team_id))]; c != nil {
					teamSlice[idx] = c
					idx++
				}
			}
			sort.Sort(TeamCampaignSorted{t: teamSlice[:idx], sort: sortOrder, rng: rng})

			for rankPos, teamComp := range teamSlice[:idx] {
				cell := [2]int{teamComp.id, rankPos}
				batchHits[j][cell]++
				batchSumY[j][cell] += w
				batchSumY2[j][cell] += w * w
			}
		}
	}

	// Compute predicted vs realized variance calibration ratios separately for Plain P and targeted M batches
	var pVarRatios []float64
	var targetedVarRatios []float64

	for j, batch := range design.Batches {
		for cell, hits := range batchHits[j] {
			if hits > 1 {
				n := batch.Samples
				pBatch := batchSumY[j][cell] / float64(n)
				s2Batch := (batchSumY2[j][cell] - float64(n)*pBatch*pBatch) / float64(n-1)
				if s2Batch > 0 {
					predVar := design.CellPredictedVarPerSample[batch.Proposal.ID][cell]
					if !math.IsNaN(predVar) && !math.IsInf(predVar, 0) && predVar > 0 {
						ratio := s2Batch / predVar
						if batch.Proposal.Kind == ProposalPlainMC {
							pVarRatios = append(pVarRatios, ratio)
						} else {
							targetedVarRatios = append(targetedVarRatios, ratio)
						}
					}
				}
			}
		}
	}

	medianPVarRatio, p10PVarRatio, p90PVarRatio := 1.0, 1.0, 1.0
	if len(pVarRatios) > 0 {
		sort.Float64s(pVarRatios)
		medianPVarRatio = pVarRatios[len(pVarRatios)/2]
		p10PVarRatio = pVarRatios[int(float64(len(pVarRatios)-1)*0.10)]
		p90PVarRatio = pVarRatios[int(float64(len(pVarRatios)-1)*0.90)]
	}

	medianTargetedVarRatio, p10TargetedVarRatio, p90TargetedVarRatio := 1.0, 1.0, 1.0
	if len(targetedVarRatios) > 0 {
		sort.Float64s(targetedVarRatios)
		medianTargetedVarRatio = targetedVarRatios[len(targetedVarRatios)/2]
		p10TargetedVarRatio = targetedVarRatios[int(float64(len(targetedVarRatios)-1)*0.10)]
		p90TargetedVarRatio = targetedVarRatios[int(float64(len(targetedVarRatios)-1)*0.90)]
	}

	log.Printf("rare-position-diversified-variance-calibration-plain: evaluated_batch_cells=%d median_var_ratio=%.4f p10_var_ratio=%.4f p90_var_ratio=%.4f",
		len(pVarRatios), medianPVarRatio, p10PVarRatio, p90PVarRatio)
	log.Printf("rare-position-diversified-variance-calibration-targeted: evaluated_batch_cells=%d median_var_ratio=%.4f p10_var_ratio=%.4f p90_var_ratio=%.4f",
		len(targetedVarRatios), medianTargetedVarRatio, p10TargetedVarRatio, p90TargetedVarRatio)

	// Combine batch estimates per cell
	estimates := make(map[int]map[int]DiversifiedEstimate, numTeams)
	for _, team := range teamGroups {
		id := team.Team_id
		estimates[id] = make(map[int]DiversifiedEstimate, numPositions)

		for pos := 0; pos < numPositions; pos++ {
			cell := [2]int{id, pos}
			betas, ok := design.CellCombinationWeights[cell]
			if !ok || len(betas) != numBatches {
				betas = make([]float64, numBatches)
				for j, batch := range design.Batches {
					if batch.Proposal.Kind == ProposalPlainMC {
						betas[j] = 1.0
					}
				}
			}

			pHatCombined := 0.0
			varCombined := 0.0
			totHits := 0
			totSamples := 0
			totWork := int64(0)
			batchProbs := make([]float64, numBatches)
			batchVars := make([]float64, numBatches)

			for j, batch := range design.Batches {
				n := batch.Samples
				hits := batchHits[j][cell]
				sumY := batchSumY[j][cell]
				sumY2 := batchSumY2[j][cell]

				totHits += hits
				totSamples += n
				totWork += batch.Work

				pBatch := 0.0
				s2Batch := 0.0
				if n > 0 {
					pBatch = sumY / float64(n)
					if n > 1 {
						s2Batch = (sumY2 - float64(n)*pBatch*pBatch) / float64(n-1)
						if s2Batch < 0 {
							s2Batch = 0
						}
					}
				}

				batchProbs[j] = pBatch
				batchVars[j] = s2Batch

				beta := betas[j]
				pHatCombined += beta * pBatch
				if n > 0 {
					varCombined += beta * beta * (s2Batch / float64(n))
				}
			}

			stdErr := math.Sqrt(varCombined)
			relSE := math.Inf(1)
			if pHatCombined > 0 {
				relSE = stdErr / pHatCombined
			}

			relativeESS := 0.0
			if varCombined > 0 {
				relativeESS = (pHatCombined * pHatCombined) / varCombined
			}

			essPerWork := 0.0
			if totWork > 0 {
				essPerWork = relativeESS / float64(totWork)
			}

			upper95 := pHatCombined + 1.96*stdErr
			if totHits == 0 {
				upper95 = zeroHitUpper95(totSamples)
			}

			estimates[id][pos] = DiversifiedEstimate{
				Probability:         pHatCombined,
				StdErr:              stdErr,
				RelativeSE:          relativeSEPointer(relSE),
				ESS:                 relativeESS,
				ESSPerWork:          essPerWork,
				RawHits:             totHits,
				Samples:             totSamples,
				WorkSpent:           totWork,
				Available:           totSamples > 0,
				MeetsPrecisionGoal:  estimateMeetsPrecisionGoal(relativeESS, relSE),
				ZeroHitUpper95:      upper95,
				Design:              "diversified_importance_sampling",
				BatchProbabilities:  batchProbs,
				BatchVariances:      batchVars,
				CombinationBetas:    betas,
			}
		}
	}

	return estimates
}

func runDiversifiedSearchAndProduction(
	group *GroupType,
	campaign []*TeamCampaign,
	table *Table,
	sortOrder []SortType,
	teamOdds []OddsType,
	totalWorkLimit int64,
	masterSeed int64,
) map[int]map[int]ProductionEstimate {
	scoutSeed := deriveRarePositionSeed(masterSeed, "diversified-scout")
	searchSeed := deriveRarePositionSeed(masterSeed, "diversified-search")
	prodSeed := deriveRarePositionSeed(masterSeed, "diversified-production")

	scoutRNG := rand.New(rand.NewSource(scoutSeed))
	searchRNG := rand.New(rand.NewSource(searchSeed))

	unplayedGames := 0
	for _, game := range group.Games {
		if !game.Played {
			unplayedGames++
		}
	}
	numTeams := len(group.Team_groups)
	plainWorkPerSample := estimateSeasonWork(unplayedGames, 1, numTeams)

	originalMeans := make([]GameProposalMeans, len(group.Games))
	for i, game := range group.Games {
		originalMeans[i] = GameProposalMeans{Home: game.HomePower, Away: game.AwayPower}
	}

	scoutSamples := minInt(diversifiedScoutSamples(), int(totalWorkLimit/plainWorkPerSample))
	log.Printf("rare-position-diversified-scout: group=%d scout_samples=%d scout_work=%d",
		group.Id, scoutSamples, int64(scoutSamples)*plainWorkPerSample)

	scout := runPlainMCScout(campaign, group.Games, table, sortOrder, group.Team_groups, scoutSamples, plainWorkPerSample, scoutRNG)

	searchCapWork := int64(DiversifiedDefaultSearchWorkCap)
	proposals, probeStats, searchWork := searchDiversifiedProposals(
		scout, campaign, group.Games, originalMeans, table, sortOrder, group.Team_groups, searchCapWork, searchRNG)

	frozenDesign := freezeDiversifiedDesign(proposals, probeStats, scout, group.Team_groups, totalWorkLimit, scout.Work, searchWork)

	log.Printf("rare-position-diversified-freeze: group=%d proposals=%d scout_work=%d search_work=%d production_work=%d unused_work=%d",
		group.Id, len(frozenDesign.Batches), frozenDesign.ScoutWork, frozenDesign.SearchWork, frozenDesign.ProductionWork, frozenDesign.UnusedWork)

	for _, batch := range frozenDesign.Batches {
		log.Printf("rare-position-diversified-batch: group=%d prop_id=%s kind=%s samples=%d work=%d",
			group.Id, batch.Proposal.ID, batch.Proposal.Kind, batch.Samples, batch.Work)
	}

	// Beta distribution summary metrics
	var plainBetas []float64
	gt010, gt050, gt090 := 0, 0, 0
	for _, betas := range frozenDesign.CellCombinationWeights {
		plainBeta := 0.0
		maxTargetedBeta := 0.0
		for j, b := range betas {
			if frozenDesign.Batches[j].Proposal.Kind == ProposalPlainMC {
				plainBeta = b
			} else if b > maxTargetedBeta {
				maxTargetedBeta = b
			}
		}
		plainBetas = append(plainBetas, plainBeta)
		if maxTargetedBeta > 0.10 {
			gt010++
		}
		if maxTargetedBeta > 0.50 {
			gt050++
		}
		if maxTargetedBeta > 0.90 {
			gt090++
		}
	}

	medianPlainBeta, p10PlainBeta, p90PlainBeta := 1.0, 1.0, 1.0
	if len(plainBetas) > 0 {
		sort.Float64s(plainBetas)
		medianPlainBeta = plainBetas[len(plainBetas)/2]
		p10PlainBeta = plainBetas[int(float64(len(plainBetas)-1)*0.10)]
		p90PlainBeta = plainBetas[int(float64(len(plainBetas)-1)*0.90)]
	}

	log.Printf("rare-position-diversified-beta-summary: group=%d feasible_cells=%d median_plain_beta=%.4f p10_plain_beta=%.4f p90_plain_beta=%.4f cells_targeted_beta_gt_0.10=%d cells_targeted_beta_gt_0.50=%d cells_targeted_beta_gt_0.90=%d",
		group.Id, len(frozenDesign.CellCombinationWeights), medianPlainBeta, p10PlainBeta, p90PlainBeta, gt010, gt050, gt090)

	divEstimates := runDiversifiedProduction(frozenDesign, campaign, group.Games, originalMeans, table, sortOrder, group.Team_groups, prodSeed)

	estimates := make(map[int]map[int]ProductionEstimate, numTeams)
	for teamID, posMap := range divEstimates {
		estimates[teamID] = make(map[int]ProductionEstimate, len(posMap))
		for pos, est := range posMap {
			estimates[teamID][pos] = ProductionEstimate{
				Probability:         est.Probability,
				StdErr:              est.StdErr,
				Samples:             est.Samples,
				Hits:                est.RawHits,
				ESS:                 est.ESS,
				MeanWeight:          1.0,
				WorkSpent:           est.WorkSpent,
				Available:           est.Available,
				MeetsPrecisionGoal:  est.MeetsPrecisionGoal,
				RelativeSE:          est.RelativeSE,
				MaxEventWeightShare: 0.0,
				ZeroHitUpper95:      est.ZeroHitUpper95,
				Design:              est.Design,
			}
		}
	}

	feasibleCells, nonzeroCells := 0, 0
	relESS4, relESS10, relESS25 := 0, 0, 0
	relSE50, relSE32, relSE20 := 0, 0, 0
	var relSEs []float64

	for teamID, posMap := range divEstimates {
		for pos, est := range posMap {
			cell := [2]int{teamID, pos}
			if scout.Feasibility[cell] != "proven_impossible" {
				feasibleCells++
			}
			if est.Probability > 0 {
				nonzeroCells++
			}
			if est.ESS >= 4 {
				relESS4++
			}
			if est.ESS >= 10 {
				relESS10++
			}
			if est.ESS >= 25 {
				relESS25++
			}
			if est.RelativeSE != nil {
				rSE := *est.RelativeSE
				if rSE <= 0.50 {
					relSE50++
				}
				if rSE <= 0.32 {
					relSE32++
				}
				if rSE <= 0.20 {
					relSE20++
				}
				if est.Probability > 0 {
					relSEs = append(relSEs, rSE)
				}
			}
		}
	}

	medianRelSE := math.Inf(1)
	p90RelSE := math.Inf(1)
	if len(relSEs) > 0 {
		sort.Float64s(relSEs)
		medianRelSE = relSEs[len(relSEs)/2]
		p90RelSE = relSEs[int(float64(len(relSEs)-1)*0.90)]
	}

	log.Printf("rare-position-diversified-quality: group=%d feasible_cells=%d nonzero_cells=%d rel_ess_ge_4=%d rel_ess_ge_10=%d rel_ess_ge_25=%d rel_se_le_50pct=%d rel_se_le_32pct=%d rel_se_le_20pct=%d median_rel_se=%.4f p90_rel_se=%.4f",
		group.Id, feasibleCells, nonzeroCells, relESS4, relESS10, relESS25, relSE50, relSE32, relSE20, medianRelSE, p90RelSE)

	return estimates
}
