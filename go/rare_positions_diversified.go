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
	DiversifiedDefaultScoutSamples          = 15000
	DiversifiedDefaultDiscoveryWorkCap      = 5000 * 350 // MC-equivalent work
	DiversifiedDefaultValidationWorkCap     = 5000 * 350 // MC-equivalent work
	DiversifiedDefaultMaxValidatedProposals = 6
	DiversifiedValidationMinSamples         = 200
	DiversifiedDiscoverySamples             = 200
	DiversifiedDefaultTotalWork             = 35000000 // 35M nominal work
	DiversifiedDefensiveMixtureEpsilon      = 0.05
	DiversifiedProbeChunkWork               = 150 * 350
	DiversifiedAllocChunkWork               = 500 * 350
	DiversifiedDefaultMinPlainFraction      = 0.10
	DiversifiedMaxSingleTeamPerDirection    = 2
	DiversifiedMaxCompetitors               = 3
	DiversifiedMaxKL                        = 3.0
	DiversifiedTargetESS                    = 10.0
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

func diversifiedWorkCap(envName string, defaultEquivalent int64, plainWorkPerSample int64) int64 {
	equivalent := defaultEquivalent
	if raw := os.Getenv(envName); raw != "" {
		if parsed, err := strconv.ParseInt(raw, 10, 64); err == nil && parsed >= 0 {
			equivalent = parsed
		}
	}
	if plainWorkPerSample <= 0 {
		return 0
	}
	return equivalent * plainWorkPerSample
}

func diversifiedMaxValidatedProposals() int {
	if raw := os.Getenv("RARE_POSITION_DIVERSIFIED_MAX_VALIDATED_PROPOSALS"); raw != "" {
		if n, err := strconv.Atoi(raw); err == nil && n >= 0 {
			return n
		}
	}
	return DiversifiedDefaultMaxValidatedProposals
}

func diversifiedValidationMinSamples() int {
	if raw := os.Getenv("RARE_POSITION_DIVERSIFIED_VALIDATION_MIN_SAMPLES"); raw != "" {
		if n, err := strconv.Atoi(raw); err == nil && n > 0 {
			return n
		}
	}
	return DiversifiedValidationMinSamples
}

func logDiversifiedEnvironment() {
	keys := []string{"RARE_POSITION_IMPORTANCE_SAMPLING", "RARE_POSITION_DIVERSIFIED_IS", "RARE_POSITION_DIVERSIFIED_SCOUT_SAMPLES", "RARE_POSITION_DIVERSIFIED_MIN_PLAIN_FRACTION", "RARE_POSITION_DIVERSIFIED_DISCOVERY_EQ", "RARE_POSITION_DIVERSIFIED_VALIDATION_EQ", "RARE_POSITION_DIVERSIFIED_MAX_VALIDATED_PROPOSALS", "RARE_POSITION_DIVERSIFIED_VALIDATION_MIN_SAMPLES", "RARE_POSITION_POINT_HYBRID_GROUP", "RARE_POSITION_POINT_HYBRID_TEAM", "RARE_POSITION_POINT_HYBRID_MAX_RANK", "RARE_POSITION_POINT_HYBRID_TARGET_MASS", "RARE_POSITION_POINT_HYBRID_Q_PERCENT", "RARE_POSITION_MIN_INTERESTING_PROBABILITY"}
	for _, key := range keys {
		value, ok := os.LookupEnv(key)
		if !ok {
			value = "<unset>"
		}
		log.Printf("rare-position-environment: %s=%q", key, value)
	}
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

type DiversifiedDiscoveryStats struct {
	ProposalID     string                   `json:"proposal_id"`
	Samples        int                      `json:"samples"`
	Work           int64                    `json:"work"`
	RankHistograms map[int][]int            `json:"-"`
	CellHits       map[[2]int]int           `json:"-"`
	Metrics        map[int]ProbeRankMetrics `json:"metrics,omitempty"`
}

type DiversifiedCandidate struct {
	Proposal        DiversifiedProposal
	Tail            DirectionalTail
	IntendedCells   map[[2]int]bool
	DiscoveryScore  float64
	FrontierCovered bool
	Discovery       DiversifiedDiscoveryStats
}

type DiversifiedValidationStats struct {
	ProposalID            string
	Samples               int
	Work                  int64
	CellHits              map[[2]int]int
	CellSumY              map[[2]int]float64
	CellSumY2             map[[2]int]float64
	CellMaxY              map[[2]int]float64
	CellEventESS          map[[2]int]float64
	CellVariancePerSample map[[2]int]float64
	ComponentSampleCount  map[int]int
	ComponentRankHist     map[int]map[int][]int
}

type FrozenProductionBatch struct {
	Proposal      DiversifiedProposal `json:"proposal"`
	Samples       int                 `json:"samples"`
	Work          int64               `json:"work"`
	WorkPerSample int64               `json:"work_per_sample"`
}

type FrozenDiversifiedDesign struct {
	Batches                    []FrozenProductionBatch       `json:"batches"`
	CellCombinationWeights     map[[2]int][]float64          `json:"-"` // (teamID, pos) -> beta per batch
	CellValidationVarPerSample map[string]map[[2]int]float64 `json:"-"`
	CellValidationESS          map[string]map[[2]int]float64 `json:"-"`
	IntendedCells              map[string]map[[2]int]bool    `json:"-"`
	TotalWork                  int64                         `json:"total_work"`
	ScoutWork                  int64                         `json:"scout_work"`
	DiscoveryWork              int64                         `json:"discovery_work"`
	ValidationWork             int64                         `json:"validation_work"`
	ProductionWork             int64                         `json:"production_work"`
	UnusedWork                 int64                         `json:"unused_work"`
}

// DiversifiedRunDiagnostics is populated before fresh production starts. It is
// used by offline benchmarks; none of its fields feed back into the estimator.
type DiversifiedRunDiagnostics struct {
	Design            FrozenDiversifiedDesign
	Feasibility       map[[2]int]string
	Candidates        int
	Shortlisted       int
	Validated         int
	Shortlist         []DiversifiedProposal
	ValidationSamples map[string]int
	ValidationCells   []DiversifiedValidationCell
}

type DiversifiedValidationCell struct {
	Proposal          string  `json:"proposal"`
	Team              int     `json:"team"`
	Position          int     `json:"position"`
	Intended          bool    `json:"intended"`
	ESS               float64 `json:"ess"`
	Eligible          bool    `json:"eligible"`
	VariancePerSample float64 `json:"variance_per_sample"`
}

type DiversifiedEstimate struct {
	Probability        float64   `json:"probability"`
	StdErr             float64   `json:"std_err"`
	RelativeSE         *float64  `json:"relative_se"`
	ESS                float64   `json:"ess"`
	ESSPerWork         float64   `json:"ess_per_work"`
	RawHits            int       `json:"raw_hits"`
	Samples            int       `json:"samples"`
	WorkSpent          int64     `json:"work_spent"`
	Available          bool      `json:"available"`
	MeetsPrecisionGoal bool      `json:"meets_precision_goal"`
	ZeroHitUpper95     float64   `json:"zero_hit_upper_95"`
	Design             string    `json:"design"`
	BatchProbabilities []float64 `json:"batch_probabilities,omitempty"`
	BatchVariances     []float64 `json:"batch_variances,omitempty"`
	CombinationBetas   []float64 `json:"combination_betas,omitempty"`
}

type ScoutData struct {
	Samples         int
	Work            int64
	TeamCounts      map[int][]int
	TeamProbs       map[int][]float64
	TeamMeanRanks   map[int]float64
	MinObservedRank map[int]int
	MaxObservedRank map[int]int
	Feasibility     map[[2]int]string // (teamID, pos) -> "observed", "feasible_unseen", "proven_impossible"
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
				ExtremityScore: extremity,
				Priority:       priority,
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
				ExtremityScore: extremity,
				Priority:       priority,
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
) DiversifiedDiscoveryStats {
	samples := int(probeWork / proposal.WorkPerSample)
	stats := DiversifiedDiscoveryStats{
		ProposalID: proposal.ID, Samples: samples, Work: int64(samples) * proposal.WorkPerSample,
		RankHistograms: make(map[int][]int, len(teamGroups)),
		CellHits:       make(map[[2]int]int), Metrics: make(map[int]ProbeRankMetrics),
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

		for i, g := range games {
			if g.Played {
				continue
			}
			hScore := poissonRand(rng, activeMeans[i].Home)
			aScore := poissonRand(rng, activeMeans[i].Away)

			home, away := g.home_table_index, g.away_table_index
			if simCampaign[home] != nil {
				simCampaign[home].add_game(&GameType{g.Id, g.HomeId, g.AwayId, hScore, aScore, 0, 0, true, home, away})
			}
			if simCampaign[away] != nil {
				simCampaign[away].add_game(&GameType{g.Id, g.HomeId, g.AwayId, hScore, aScore, 0, 0, true, home, away})
			}
		}

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
		}
	}

	// Compute probe rank metrics for target team
	if targetTeam > 0 {
		stats.Metrics[targetTeam] = computeProbeRankMetrics(
			targetTeam, frontierRank, proposal.Direction, unresolvedCells, stats.RankHistograms[targetTeam])
	}

	return stats
}

func makeDiversifiedCandidate(proposal DiversifiedProposal, tail DirectionalTail, discovery DiversifiedDiscoveryStats, quality ProposalQuality, scout ScoutData) DiversifiedCandidate {
	intended := make(map[[2]int]bool)
	for pos := 0; pos < len(discovery.RankHistograms[tail.TeamID]); pos++ {
		cell := [2]int{tail.TeamID, pos}
		count := 0
		if len(scout.TeamCounts[tail.TeamID]) > pos {
			count = scout.TeamCounts[tail.TeamID][pos]
		}
		if count >= int(DiversifiedTargetESS) || scout.Feasibility[cell] == "proven_impossible" {
			continue
		}
		if tail.Direction == RareBetter && pos >= tail.ObservedMinRank || tail.Direction == RareWorse && pos <= tail.ObservedMaxRank {
			continue
		}
		if scout.Feasibility[cell] == "feasible_unseen" {
			intended[cell] = true
		}
	}
	return DiversifiedCandidate{Proposal: proposal, Tail: tail, IntendedCells: intended, DiscoveryScore: quality.TailScore, FrontierCovered: quality.IsFrontierCovered, Discovery: discovery}
}

func shortlistDiversifiedCandidates(candidates []DiversifiedCandidate, limit int) []DiversifiedCandidate {
	if len(candidates) == 0 {
		return nil
	}
	plain := candidates[0]
	others := append([]DiversifiedCandidate(nil), candidates[1:]...)
	sort.SliceStable(others, func(i, j int) bool {
		if discoveryCandidatePriority(others[i]) != discoveryCandidatePriority(others[j]) {
			return discoveryCandidatePriority(others[i]) > discoveryCandidatePriority(others[j])
		}
		return others[i].Proposal.ID < others[j].Proposal.ID
	})
	if limit < 0 {
		limit = 0
	}
	selected := []DiversifiedCandidate{plain}
	used := make(map[string]bool)
	// First take the strongest discovery candidate for each team/tail direction.
	for _, c := range others {
		key := fmt.Sprintf("%d/%d", c.Tail.TeamID, c.Tail.Direction)
		if !used[key] && len(selected)-1 < limit {
			selected = append(selected, c)
			used[key] = true
		}
	}
	for _, c := range others {
		if len(selected)-1 >= limit {
			break
		}
		found := false
		for _, kept := range selected {
			if kept.Proposal.ID == c.Proposal.ID {
				found = true
				break
			}
		}
		if !found {
			selected = append(selected, c)
		}
	}
	return selected
}

func discoveryCandidatePriority(c DiversifiedCandidate) float64 {
	priority := c.DiscoveryScore - 0.05*c.Proposal.Strength - 0.01*c.Proposal.KL
	if c.FrontierCovered {
		priority += 1
	}
	if c.Proposal.Kind == ProposalCompetitorAssisted {
		priority -= 0.1
	}
	return priority
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

func discoverDiversifiedProposals(
	scout ScoutData,
	baseCampaign []*TeamCampaign,
	games []*GameType,
	original []GameProposalMeans,
	table *Table,
	sortOrder []SortType,
	teamGroups []TeamType,
	discoveryWorkCap int64,
	rng *rand.Rand,
) ([]DiversifiedCandidate, int64) {
	var retained []DiversifiedCandidate

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
	plainStats := DiversifiedDiscoveryStats{ProposalID: "plain_mc", Samples: scout.Samples, Work: scout.Work, RankHistograms: scout.TeamCounts, CellHits: make(map[[2]int]int), Metrics: make(map[int]ProbeRankMetrics)}
	for teamID, counts := range scout.TeamCounts {
		for pos, c := range counts {
			cell := [2]int{teamID, pos}
			plainStats.CellHits[cell] = c
		}
	}
	retained = append(retained, DiversifiedCandidate{Proposal: plainProp, Discovery: plainStats})

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
		if workSpent >= discoveryWorkCap {
			break
		}

		retainedForTail := make([]DiversifiedCandidate, 0)
		tailFrontierCovered := false

		for _, s := range strengthLadder {
			if workSpent >= discoveryWorkCap || len(retainedForTail) >= DiversifiedMaxSingleTeamPerDirection {
				break
			}

			prop, ok := buildDirectTeamProposal(tail.TeamID, tail.Direction, s, original, games, teamIDs, numTeams)
			if !ok {
				log.Printf("rare-position-candidate: team=%d dir=%s frontier=%d type=single_team strength=%.2f decision=reject reason=KL_limit",
					tail.TeamID, directionName(tail.Direction), tail.FrontierRank, s)
				continue
			}

			probeWork := int64(DiversifiedDiscoverySamples) * prop.WorkPerSample
			if workSpent+probeWork > discoveryWorkCap {
				probeWork = discoveryWorkCap - workSpent
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
					prevStats := prev.Discovery
					tvd := proposalHistogramTVD(stats.RankHistograms[tail.TeamID], prevStats.RankHistograms[tail.TeamID], stats.Samples, prevStats.Samples)
					if tvd < 0.15 {
						isDuplicate = true
						log.Printf("rare-position-candidate: team=%d dir=%s prop=%s decision=skip_duplicate_tvd tvd=%.3f",
							tail.TeamID, directionName(tail.Direction), prop.ID, tvd)
						break
					}
				}

				if !isDuplicate {
					candidate := makeDiversifiedCandidate(prop, tail, stats, q, scout)
					retained = append(retained, candidate)
					retainedForTail = append(retainedForTail, candidate)
				}
			}

			if q.IsOvershot || len(retainedForTail) >= DiversifiedMaxSingleTeamPerDirection {
				break
			}
		}

		// Competitor Escalation if frontier was NOT covered by target-only search
		competitorCovered := false
		if !tailFrontierCovered && workSpent < discoveryWorkCap {
			competitors := selectCompetitorsForTail(tail.TeamID, tail.Direction, tail.FrontierRank, scout, teamGroups, DiversifiedMaxCompetitors)
			if len(competitors) > 0 {
				for _, s := range []float64{0.75, 1.25} {
					if workSpent >= discoveryWorkCap || len(retainedForTail) >= DiversifiedMaxSingleTeamPerDirection+1 {
						break
					}

					prop, ok := buildCompetitorAssistedProposal(tail.TeamID, tail.Direction, s, competitors, 0.50, scout.TeamMeanRanks, original, games, teamIDs, numTeams)
					if !ok {
						log.Printf("rare-position-candidate: team=%d dir=%s frontier=%d type=competitor strength=%.2f decision=reject reason=KL_limit",
							tail.TeamID, directionName(tail.Direction), tail.FrontierRank, s)
						continue
					}

					probeWork := int64(DiversifiedDiscoverySamples) * prop.WorkPerSample
					if workSpent+probeWork > discoveryWorkCap {
						probeWork = discoveryWorkCap - workSpent
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
						q.ShouldRetain = true
						q.TailScore = m.FrontierMass + m.Near1Mass + m.Near2Mass
						candidate := makeDiversifiedCandidate(prop, tail, stats, q, scout)
						retained = append(retained, candidate)
						retainedForTail = append(retainedForTail, candidate)
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

	log.Printf("rare-position-diversified-discovery-summary: tails_total=%d tails_frontier_covered_single_team=%d tails_frontier_covered_competitors=%d tails_partially_covered=%d tails_exhausted=%d candidates=%d discovery_work=%d discovery_work_cap=%d",
		tailsTotal, tailsFrontierCoveredSingleTeam, tailsFrontierCoveredCompetitors, tailsPartiallyCovered, tailsExhausted, len(retained), workSpent, discoveryWorkCap)

	return retained, workSpent
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
	candidates []DiversifiedCandidate,
	validation map[string]DiversifiedValidationStats,
	scout ScoutData,
	teamGroups []TeamType,
	totalWorkBudget, scoutWork, discoveryWork, validationWork int64,
) FrozenDiversifiedDesign {
	proposals := make([]DiversifiedProposal, 0, len(candidates))
	candidateByID := make(map[string]DiversifiedCandidate, len(candidates))
	for _, c := range candidates {
		proposals = append(proposals, c.Proposal)
		candidateByID[c.Proposal.ID] = c
	}
	if len(proposals) == 0 {
		return FrozenDiversifiedDesign{TotalWork: totalWorkBudget, ScoutWork: scoutWork, DiscoveryWork: discoveryWork, ValidationWork: validationWork, UnusedWork: maxInt64(0, totalWorkBudget-scoutWork-discoveryWork-validationWork)}
	}
	available := totalWorkBudget - scoutWork - discoveryWork - validationWork
	if available < 0 {
		available = 0
	}
	minPlain := int64(float64(available) * diversifiedMinPlainFraction())
	allocated := map[string]int64{"plain_mc": minPlain}
	remaining := available - minPlain
	cells := make([][2]int, 0, len(teamGroups)*len(teamGroups))
	for _, team := range teamGroups {
		for pos := range teamGroups {
			cell := [2]int{team.Team_id, pos}
			if scout.Feasibility[cell] != "proven_impossible" {
				cells = append(cells, cell)
			}
		}
	}
	pReg := make(map[[2]int]float64, len(cells))
	prec := make(map[string]map[[2]int]float64, len(proposals))
	for _, cell := range cells {
		pReg[cell] = regularizedScoutProbability(scout, cell)
	}
	for _, prop := range proposals {
		prec[prop.ID] = map[[2]int]float64{}
		if prop.Kind == ProposalPlainMC {
			for _, cell := range cells {
				p := pReg[cell]
				v := p * (1 - p)
				if v > 0 {
					prec[prop.ID][cell] = 1 / v
				}
			}
			continue
		}
		v, ok := validation[prop.ID]
		if !ok {
			continue
		}
		c := candidateByID[prop.ID]
		// Collateral cells may improve the frozen combination, but only an
		// independently eligible intended cell can admit a proposal to production.
		intendedEligible := false
		for cell := range c.IntendedCells {
			eligible, _ := validationEligibility(v, cell, true)
			variance := v.CellVariancePerSample[cell]
			if eligible && variance > 0 && !math.IsNaN(variance) && !math.IsInf(variance, 0) {
				intendedEligible = true
				break
			}
		}
		if !intendedEligible {
			continue
		}
		for _, cell := range cells {
			intended := c.IntendedCells[cell]
			eligible, _ := validationEligibility(v, cell, intended)
			variance := v.CellVariancePerSample[cell]
			if eligible && variance > 0 && !math.IsNaN(variance) && !math.IsInf(variance, 0) {
				prec[prop.ID][cell] = 1 / variance
			}
		}
	}
	chunk := int64(DiversifiedAllocChunkWork)
	for remaining >= chunk {
		relESS := map[[2]int]float64{}
		for _, cell := range cells {
			total := 0.0
			for _, prop := range proposals {
				total += float64(allocated[prop.ID]/prop.WorkPerSample) * prec[prop.ID][cell]
			}
			p := pReg[cell]
			relESS[cell] = p * p * total
		}
		best := ""
		bestUtility := 0.0
		for _, prop := range proposals {
			n := int(chunk / prop.WorkPerSample)
			work := int64(n) * prop.WorkPerSample
			if n <= 0 || work > remaining {
				continue
			}
			u := diversifiedChunkUtility(prop, n, work, cells, relESS, pReg, prec, DiversifiedTargetESS)
			if u > bestUtility {
				best, bestUtility = prop.ID, u
			}
		}
		if best == "" {
			allocated["plain_mc"] += remaining
			remaining = 0
			break
		}
		prop := findProposalByID(proposals, best)
		work := int64(int(chunk/prop.WorkPerSample)) * prop.WorkPerSample
		if work <= 0 || work > remaining {
			break
		}
		allocated[best] += work
		remaining -= work
	}
	if remaining > 0 {
		allocated["plain_mc"] += remaining
		remaining = 0
	}
	batches := make([]FrozenProductionBatch, 0, len(proposals))
	productionWork := int64(0)
	for _, prop := range proposals {
		n := int(allocated[prop.ID] / prop.WorkPerSample)
		if n <= 0 {
			continue
		}
		work := int64(n) * prop.WorkPerSample
		productionWork += work
		batches = append(batches, FrozenProductionBatch{Proposal: prop, Samples: n, Work: work, WorkPerSample: prop.WorkPerSample})
	}
	betas := make(map[[2]int][]float64, len(cells))
	validationESS := map[string]map[[2]int]float64{}
	intendedCells := map[string]map[[2]int]bool{}
	for _, prop := range proposals {
		validationESS[prop.ID] = map[[2]int]float64{}
		intendedCells[prop.ID] = candidateByID[prop.ID].IntendedCells
		for cell, ess := range validation[prop.ID].CellEventESS {
			validationESS[prop.ID][cell] = ess
		}
	}
	for _, cell := range cells {
		betas[cell] = combineValidationBetas(batches, candidateByID, validation, cell, pReg[cell])
	}
	unused := totalWorkBudget - scoutWork - discoveryWork - validationWork - productionWork
	if unused < 0 {
		panic("diversified work budget exceeded")
	}
	if scoutWork+discoveryWork+validationWork+productionWork > totalWorkBudget {
		panic("diversified work budget exceeded")
	}
	workSpent := scoutWork + discoveryWork + validationWork + productionWork
	remainingWork := totalWorkBudget - workSpent
	if remainingWork < 0 {
		panic("diversified remaining work became negative")
	}
	log.Printf("rare-position-diversified-work-budget: total_work_limit=%d scout_work=%d discovery_work=%d validation_work=%d production_work=%d work_spent=%d remaining_work=%d", totalWorkBudget, scoutWork, discoveryWork, validationWork, productionWork, workSpent, remainingWork)
	log.Printf("rare-position-diversified-freeze: scout_work=%d discovery_work=%d validation_work=%d production_work=%d unused_work=%d", scoutWork, discoveryWork, validationWork, productionWork, unused)
	varianceMap := make(map[string]map[[2]int]float64, len(validation)+1)
	for id, stats := range validation {
		varianceMap[id] = stats.CellVariancePerSample
	}
	varianceMap["plain_mc"] = make(map[[2]int]float64, len(cells))
	for _, cell := range cells {
		p := pReg[cell]
		varianceMap["plain_mc"][cell] = p * (1 - p)
	}
	return FrozenDiversifiedDesign{Batches: batches, CellCombinationWeights: betas, CellValidationVarPerSample: varianceMap, CellValidationESS: validationESS, IntendedCells: intendedCells, TotalWork: totalWorkBudget, ScoutWork: scoutWork, DiscoveryWork: discoveryWork, ValidationWork: validationWork, ProductionWork: productionWork, UnusedWork: unused}
}

func maxInt64(a, b int64) int64 {
	if a > b {
		return a
	}
	return b
}

func findProposalByID(proposals []DiversifiedProposal, id string) DiversifiedProposal {
	for _, p := range proposals {
		if p.ID == id {
			return p
		}
	}
	return proposals[0]
}

func simulateDiversifiedMixtureBatch(
	proposal DiversifiedProposal,
	samples int,
	seed int64,
	baseCampaign []*TeamCampaign,
	games []*GameType,
	originalMeans []GameProposalMeans,
	table *Table,
	sortOrder []SortType,
	teamGroups []TeamType,
) DiversifiedValidationStats {
	stats := DiversifiedValidationStats{ProposalID: proposal.ID, Samples: samples, CellHits: map[[2]int]int{}, CellSumY: map[[2]int]float64{}, CellSumY2: map[[2]int]float64{}, CellMaxY: map[[2]int]float64{}, CellEventESS: map[[2]int]float64{}, CellVariancePerSample: map[[2]int]float64{}, ComponentSampleCount: map[int]int{}, ComponentRankHist: map[int]map[int][]int{}}
	if proposal.WorkPerSample > 0 {
		stats.Work = int64(samples) * proposal.WorkPerSample
	}
	if samples <= 0 {
		return stats
	}
	rng := rand.New(rand.NewSource(seed))
	components := []ProposalComponent{{Name: "P", Weight: DiversifiedDefensiveMixtureEpsilon, Means: originalMeans}, {Name: "Q", Weight: 1 - DiversifiedDefensiveMixtureEpsilon, Means: proposal.Means}}
	weights := []float64{DiversifiedDefensiveMixtureEpsilon, 1 - DiversifiedDefensiveMixtureEpsilon}
	if proposal.Kind == ProposalPlainMC {
		components = []ProposalComponent{{Name: "P", Weight: 1, Means: originalMeans}}
		weights = []float64{1}
	}
	for k := range components {
		stats.ComponentRankHist[k] = make(map[int][]int, len(teamGroups))
		for _, team := range teamGroups {
			stats.ComponentRankHist[k][team.Team_id] = make([]int, len(teamGroups))
		}
	}
	simCampaign := make([]*TeamCampaign, len(baseCampaign))
	teamSlice := make([]*TeamCampaign, len(teamGroups))
	logQ := make([]float64, len(components))
	for s := 0; s < samples; s++ {
		for i, v := range baseCampaign {
			if v != nil {
				simCampaign[i] = v.clone()
			} else {
				simCampaign[i] = nil
			}
		}
		for i := range logQ {
			logQ[i] = 0
		}
		chosen := 0
		if len(components) > 1 && rng.Float64() > DiversifiedDefensiveMixtureEpsilon {
			chosen = 1
		}
		active := components[chosen].Means
		stats.ComponentSampleCount[chosen]++
		for i, g := range games {
			if g.Played {
				continue
			}
			hs := poissonRand(rng, active[i].Home)
			as := poissonRand(rng, active[i].Away)
			for k, component := range components {
				logQ[k] += logPoissonQOverP(hs, originalMeans[i].Home, component.Means[i].Home)
				logQ[k] += logPoissonQOverP(as, originalMeans[i].Away, component.Means[i].Away)
			}
			home, away := g.home_table_index, g.away_table_index
			if simCampaign[home] != nil {
				simCampaign[home].add_game(&GameType{g.Id, g.HomeId, g.AwayId, hs, as, 0, 0, true, home, away})
			}
			if simCampaign[away] != nil {
				simCampaign[away].add_game(&GameType{g.Id, g.HomeId, g.AwayId, hs, as, 0, 0, true, home, away})
			}
		}
		w := mixtureImportanceWeightMulti(logQ, weights)
		idx := 0
		for _, tg := range teamGroups {
			if c := simCampaign[table.Query(uint32(tg.Team_id))]; c != nil {
				teamSlice[idx] = c
				idx++
			}
		}
		sort.Sort(TeamCampaignSorted{t: teamSlice[:idx], sort: sortOrder, rng: rng})
		for rank, team := range teamSlice[:idx] {
			cell := [2]int{team.id, rank}
			stats.ComponentRankHist[chosen][team.id][rank]++
			stats.CellHits[cell]++
			stats.CellSumY[cell] += w
			stats.CellSumY2[cell] += w * w
			if w > stats.CellMaxY[cell] {
				stats.CellMaxY[cell] = w
			}
		}
	}
	for cell, sumY := range stats.CellSumY {
		sumY2 := stats.CellSumY2[cell]
		mean := sumY / float64(samples)
		if samples > 1 {
			variance := (sumY2 - float64(samples)*mean*mean) / float64(samples-1)
			if variance < 0 {
				variance = 0
			}
			stats.CellVariancePerSample[cell] = variance
		}
		if sumY2 > 0 {
			stats.CellEventESS[cell] = sumY * sumY / sumY2
		}
	}
	return stats
}

func validationEligibility(stats DiversifiedValidationStats, cell [2]int, intended bool) (bool, float64) {
	ess := stats.CellEventESS[cell]
	if intended {
		if ess < 2 {
			return false, 0
		}
		if ess < 4 {
			return true, .1
		}
		if ess < 8 {
			return true, .5
		}
		return true, 1
	}
	if ess < 8 {
		return false, 0
	}
	if ess < 15 {
		return true, .1
	}
	if ess < 25 {
		return true, .5
	}
	return true, 1
}

func combineValidationBetas(batches []FrozenProductionBatch, candidates map[string]DiversifiedCandidate, validation map[string]DiversifiedValidationStats, cell [2]int, plainProbability float64) []float64 {
	betas := make([]float64, len(batches))
	sum := 0.0
	plainIndex := -1
	for i, batch := range batches {
		if batch.Proposal.Kind == ProposalPlainMC {
			plainIndex = i
			variance := plainProbability * (1 - plainProbability)
			if variance > 0 {
				betas[i] = float64(batch.Samples) / variance
				sum += betas[i]
			}
			continue
		}
		v := validation[batch.Proposal.ID]
		variance := v.CellVariancePerSample[cell]
		if variance <= 0 || math.IsNaN(variance) || math.IsInf(variance, 0) {
			continue
		}
		eligible, cap := validationEligibility(v, cell, candidates[batch.Proposal.ID].IntendedCells[cell])
		if !eligible {
			continue
		}
		betas[i] = float64(batch.Samples) / variance
		sum += betas[i]
		_ = cap
	}
	if sum > 0 {
		for i := range betas {
			betas[i] /= sum
		}
	} else if plainIndex >= 0 {
		betas[plainIndex] = 1
	}
	targeted := 0.0
	for i, batch := range batches {
		if batch.Proposal.Kind == ProposalPlainMC {
			continue
		}
		_, cap := validationEligibility(validation[batch.Proposal.ID], cell, candidates[batch.Proposal.ID].IntendedCells[cell])
		if betas[i] > cap {
			betas[i] = cap
		}
		targeted += betas[i]
	}
	if targeted > 1 {
		for i, batch := range batches {
			if batch.Proposal.Kind != ProposalPlainMC {
				betas[i] /= targeted
			}
		}
		targeted = 1
	}
	if plainIndex >= 0 {
		betas[plainIndex] = 1 - targeted
	}
	return betas
}

func validateDiversifiedCandidates(candidates []DiversifiedCandidate, validationCap int64, minSamples int, validationSeed int64, baseCampaign []*TeamCampaign, games []*GameType, originalMeans []GameProposalMeans, table *Table, sortOrder []SortType, teamGroups []TeamType) (map[string]DiversifiedValidationStats, int64, []DiversifiedCandidate) {
	requestedCandidates := len(candidates)
	if len(candidates) == 0 || validationCap <= 0 {
		return map[string]DiversifiedValidationStats{}, 0, nil
	}
	if minSamples < 1 {
		minSamples = 1
	}
	active := append([]DiversifiedCandidate(nil), candidates...)
	for len(active) > 0 {
		minimum := int64(0)
		for _, c := range active {
			minimum += int64(minSamples) * c.Proposal.WorkPerSample
		}
		if minimum <= validationCap {
			break
		}
		active = active[:len(active)-1]
	}
	stats := make(map[string]DiversifiedValidationStats, len(active))
	spent := int64(0)
	for i, c := range active {
		remainingCandidates := len(active) - i
		remaining := validationCap - spent
		minimumRemaining := int64(0)
		for _, future := range active[i:] {
			minimumRemaining += int64(minSamples) * future.Proposal.WorkPerSample
		}
		extraShare := int64(0)
		if remaining > minimumRemaining {
			extraShare = (remaining - minimumRemaining) / int64(remainingCandidates)
		}
		samples := minSamples + int(extraShare/c.Proposal.WorkPerSample)
		seed := deriveRarePositionSeed(validationSeed, "diversified-validation-"+c.Proposal.ID)
		v := simulateDiversifiedMixtureBatch(c.Proposal, samples, seed, baseCampaign, games, originalMeans, table, sortOrder, teamGroups)
		if v.Work > remaining {
			panic("validation work exceeded cap")
		}
		spent += v.Work
		stats[c.Proposal.ID] = v
		log.Printf("rare-position-diversified-validation: proposal=%s samples=%d work=%d", c.Proposal.ID, v.Samples, v.Work)
	}
	log.Printf("rare-position-diversified-validation-summary: shortlisted=%d candidates_fitting_minimum=%d validated=%d reduced_for_budget=%d validation_work=%d validation_cap=%d", requestedCandidates, len(active), len(stats), requestedCandidates-len(active), spent, validationCap)
	return stats, spent, active
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
		seed := deriveRarePositionSeed(masterSeed, fmt.Sprintf("diversified-prod-batch-%d-%s", j, batch.Proposal.ID))
		batchStats := simulateDiversifiedMixtureBatch(batch.Proposal, batch.Samples, seed, baseCampaign, games, originalMeans, table, sortOrder, teamGroups)
		batchHits[j] = batchStats.CellHits
		batchSumY[j] = batchStats.CellSumY
		batchSumY2[j] = batchStats.CellSumY2
		for component, count := range batchStats.ComponentSampleCount {
			for team, hist := range batchStats.ComponentRankHist[component] {
				log.Printf("rare-position-diversified-component-hist: prop_id=%s component=%d component_samples=%d team=%d rank_hist=%v", batch.Proposal.ID, component, count, team, hist)
			}
		}
	}

	// Compute predicted vs realized variance calibration ratios separately for Plain P and targeted M batches
	var pVarRatios []float64
	var targetedVarRatios []float64
	validationESSBuckets := map[string]int{"2_4": 0, "4_8": 0, "ge_8": 0}

	for j, batch := range design.Batches {
		for cell, hits := range batchHits[j] {
			if hits > 1 {
				n := batch.Samples
				pBatch := batchSumY[j][cell] / float64(n)
				s2Batch := (batchSumY2[j][cell] - float64(n)*pBatch*pBatch) / float64(n-1)
				if s2Batch > 0 {
					predVar := design.CellValidationVarPerSample[batch.Proposal.ID][cell]
					if batch.Proposal.Kind != ProposalPlainMC {
						ess := design.CellValidationESS[batch.Proposal.ID][cell]
						intended := design.IntendedCells[batch.Proposal.ID][cell]
						if eligible, _ := validationEligibility(DiversifiedValidationStats{CellEventESS: map[[2]int]float64{cell: ess}}, cell, intended); !eligible {
							continue
						}
						if ess < 4 {
							validationESSBuckets["2_4"]++
						} else if ess < 8 {
							validationESSBuckets["4_8"]++
						} else {
							validationESSBuckets["ge_8"]++
						}
					}
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
	log.Printf("rare-position-diversified-validation-eligibility: ess_2_4=%d ess_4_8=%d ess_ge_8=%d", validationESSBuckets["2_4"], validationESSBuckets["4_8"], validationESSBuckets["ge_8"])

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
				Probability:        pHatCombined,
				StdErr:             stdErr,
				RelativeSE:         relativeSEPointer(relSE),
				ESS:                relativeESS,
				ESSPerWork:         essPerWork,
				RawHits:            totHits,
				Samples:            totSamples,
				WorkSpent:          totWork,
				Available:          totSamples > 0,
				MeetsPrecisionGoal: estimateMeetsPrecisionGoal(relativeESS, relSE),
				ZeroHitUpper95:     upper95,
				Design:             "diversified_importance_sampling",
				BatchProbabilities: batchProbs,
				BatchVariances:     batchVars,
				CombinationBetas:   betas,
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
	return runDiversifiedSearchAndProductionDetailed(group, campaign, table, sortOrder, teamOdds, totalWorkLimit, masterSeed, nil)
}

func runDiversifiedSearchAndProductionDetailed(
	group *GroupType,
	campaign []*TeamCampaign,
	table *Table,
	sortOrder []SortType,
	teamOdds []OddsType,
	totalWorkLimit int64,
	masterSeed int64,
	diagnostics *DiversifiedRunDiagnostics,
) map[int]map[int]ProductionEstimate {
	logDiversifiedEnvironment()
	if raw := os.Getenv("RARE_POSITION_POINT_HYBRID_GROUP"); raw != "" {
		if groupID, err := strconv.Atoi(raw); err == nil && groupID == group.Id {
			teamID, teamErr := strconv.Atoi(os.Getenv("RARE_POSITION_POINT_HYBRID_TEAM"))
			if teamErr == nil {
				maxRank := len(group.Team_groups) - 5
				if parsed, rankErr := strconv.Atoi(os.Getenv("RARE_POSITION_POINT_HYBRID_MAX_RANK")); rankErr == nil {
					maxRank = parsed
				}
				hybridEstimates, hybrid, ok := runPointHybrid(group, campaign, table, sortOrder, totalWorkLimit, masterSeed, teamID, maxRank)
				if ok {
					unplayed := 0
					for _, game := range group.Games {
						if !game.Played {
							unplayed++
						}
					}
					recordPointHybridDiagnostics(diagnostics, hybrid, teamID, maxRank,
						estimateSeasonWork(unplayed, 1, len(group.Team_groups)), estimateSeasonWork(unplayed, 2, len(group.Team_groups)), totalWorkLimit)
					log.Printf("rare-position-point-hybrid: group=%d team=%d %s", group.Id, teamID, pointHybridDescription(hybrid))
					return hybridEstimates
				}
			}
		}
	}
	scoutSeed := deriveRarePositionSeed(masterSeed, "diversified-scout")
	discoverySeed := deriveRarePositionSeed(masterSeed, "diversified-discovery")
	validationSeed := deriveRarePositionSeed(masterSeed, "diversified-validation")
	prodSeed := deriveRarePositionSeed(masterSeed, "diversified-production")

	scoutRNG := rand.New(rand.NewSource(scoutSeed))
	discoveryRNG := rand.New(rand.NewSource(discoverySeed))

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

	scoutSamples := affordableSamples(diversifiedScoutSamples(), totalWorkLimit, plainWorkPerSample)
	log.Printf("rare-position-diversified-scout: group=%d scout_samples=%d scout_work=%d",
		group.Id, scoutSamples, int64(scoutSamples)*plainWorkPerSample)

	scout := runPlainMCScout(campaign, group.Games, table, sortOrder, group.Team_groups, scoutSamples, plainWorkPerSample, scoutRNG)

	remainingAfterScout := totalWorkLimit - scout.Work
	if remainingAfterScout < 0 {
		remainingAfterScout = 0
	}
	discoveryCapWork := diversifiedWorkCap("RARE_POSITION_DIVERSIFIED_DISCOVERY_EQ", int64(DiversifiedDefaultDiscoveryWorkCap/350), plainWorkPerSample)
	validationCapWork := diversifiedWorkCap("RARE_POSITION_DIVERSIFIED_VALIDATION_EQ", int64(DiversifiedDefaultValidationWorkCap/350), plainWorkPerSample)
	// Preserve room for validation and production even when the overall budget is
	// smaller than the two configured exploration caps combined.
	capShare := remainingAfterScout / 3
	if discoveryCapWork > capShare {
		discoveryCapWork = capShare
	}
	if validationCapWork > capShare {
		validationCapWork = capShare
	}
	discoveryBudget := totalWorkLimit - scout.Work - validationCapWork
	if discoveryBudget < 0 {
		discoveryBudget = 0
	}
	if discoveryCapWork > discoveryBudget {
		discoveryCapWork = discoveryBudget
	}
	candidates, discoveryWork := discoverDiversifiedProposals(scout, campaign, group.Games, originalMeans, table, sortOrder, group.Team_groups, discoveryCapWork, discoveryRNG)
	shortlist := shortlistDiversifiedCandidates(candidates, diversifiedMaxValidatedProposals())
	for rank, candidate := range shortlist {
		if candidate.Proposal.Kind != ProposalPlainMC {
			log.Printf("rare-position-diversified-shortlist: rank=%d proposal=%s team=%d direction=%s frontier=%d discovery_score=%.4f priority=%.4f frontier_covered=%t", rank, candidate.Proposal.ID, candidate.Tail.TeamID, directionName(candidate.Tail.Direction), candidate.Tail.FrontierRank, candidate.DiscoveryScore, discoveryCandidatePriority(candidate), candidate.FrontierCovered)
		}
	}
	plainCandidate := candidates[0]
	targeted := make([]DiversifiedCandidate, 0, len(shortlist))
	for _, c := range shortlist {
		if c.Proposal.Kind != ProposalPlainMC {
			targeted = append(targeted, c)
		}
	}
	validationBudget := totalWorkLimit - scout.Work - discoveryWork
	if validationBudget < 0 {
		validationBudget = 0
	}
	if validationCapWork > validationBudget {
		validationCapWork = validationBudget
	}
	validationStats, validationWork, validated := validateDiversifiedCandidates(targeted, validationCapWork, diversifiedValidationMinSamples(), validationSeed, campaign, group.Games, originalMeans, table, sortOrder, group.Team_groups)
	proposals := make([]DiversifiedCandidate, 0, len(validated)+1)
	proposals = append(proposals, plainCandidate)
	proposals = append(proposals, validated...)
	for _, candidate := range validated {
		v := validationStats[candidate.Proposal.ID]
		log.Printf("rare-position-diversified-validation-selection: proposal=%s team=%d position=%d samples=%d work=%d intended_cells=%d discovery_priority=%.4f", candidate.Proposal.ID, candidate.Tail.TeamID, candidate.Tail.FrontierRank, v.Samples, v.Work, len(candidate.IntendedCells), discoveryCandidatePriority(candidate))
		for cell := range candidate.IntendedCells {
			hits := v.CellHits[cell]
			rate := float64(hits) / float64(v.Samples)
			log.Printf("rare-position-diversified-validation-cell: proposal=%s team=%d position=%d intended=true hits=%d rate=%.6f event_ess=%.3f variance=%.6g", candidate.Proposal.ID, cell[0], cell[1], hits, rate, v.CellEventESS[cell], v.CellVariancePerSample[cell])
		}
	}
	frozenDesign := freezeDiversifiedDesign(proposals, validationStats, scout, group.Team_groups, totalWorkLimit, scout.Work, discoveryWork, validationWork)
	if diagnostics != nil {
		diagnostics.Design = frozenDesign
		diagnostics.Feasibility = make(map[[2]int]string, len(scout.Feasibility))
		for cell, status := range scout.Feasibility {
			diagnostics.Feasibility[cell] = status
		}
		diagnostics.Candidates = len(candidates) - 1
		diagnostics.Shortlisted = len(targeted)
		diagnostics.Validated = len(validated)
		for _, candidate := range targeted {
			diagnostics.Shortlist = append(diagnostics.Shortlist, candidate.Proposal)
		}
		diagnostics.ValidationSamples = make(map[string]int, len(validated))
		for _, candidate := range validated {
			stats := validationStats[candidate.Proposal.ID]
			diagnostics.ValidationSamples[candidate.Proposal.ID] = stats.Samples
			for cell, ess := range stats.CellEventESS {
				intended := candidate.IntendedCells[cell]
				eligible, _ := validationEligibility(stats, cell, intended)
				if intended || ess > 0 {
					diagnostics.ValidationCells = append(diagnostics.ValidationCells, DiversifiedValidationCell{
						Proposal: candidate.Proposal.ID, Team: cell[0], Position: cell[1], Intended: intended, ESS: ess, Eligible: eligible,
						VariancePerSample: stats.CellVariancePerSample[cell],
					})
				}
			}
		}
		sort.Slice(diagnostics.ValidationCells, func(i, j int) bool {
			a, b := diagnostics.ValidationCells[i], diagnostics.ValidationCells[j]
			if a.Proposal != b.Proposal {
				return a.Proposal < b.Proposal
			}
			if a.Team != b.Team {
				return a.Team < b.Team
			}
			return a.Position < b.Position
		})
	}

	log.Printf("rare-position-diversified-freeze: group=%d proposals=%d scout_work=%d discovery_work=%d validation_work=%d production_work=%d unused_work=%d",
		group.Id, len(frozenDesign.Batches), frozenDesign.ScoutWork, frozenDesign.DiscoveryWork, frozenDesign.ValidationWork, frozenDesign.ProductionWork, frozenDesign.UnusedWork)

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
