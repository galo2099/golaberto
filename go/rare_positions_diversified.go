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

type ProposalProbeStats struct {
	ProposalID     string
	Samples        int
	Work           int64
	RankHistograms map[int][]int        // teamID -> rank histogram
	CellHits       map[[2]int]int       // (teamID, pos) -> raw hits
	CellSumY       map[[2]int]float64
	CellSumY2      map[[2]int]float64
	CellESS        map[[2]int]float64
}

type FrozenProductionBatch struct {
	Proposal      DiversifiedProposal `json:"proposal"`
	Samples       int                 `json:"samples"`
	Work          int64               `json:"work"`
	WorkPerSample int64               `json:"work_per_sample"`
}

type FrozenDiversifiedDesign struct {
	Batches                []FrozenProductionBatch    `json:"batches"`
	CellCombinationWeights map[[2]int][]float64        `json:"-"` // (teamID, pos) -> beta per batch
	TotalWork              int64                      `json:"total_work"`
	ScoutWork              int64                      `json:"scout_work"`
	SearchWork             int64                      `json:"search_work"`
	ProductionWork         int64                      `json:"production_work"`
	UnusedWork             int64                      `json:"unused_work"`
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
	TeamID                int
	Direction             RareDirection
	UnseenFeasibleCount   int
	FirstUnseenPosition   int
	ExtremityScore        float64
}

func discoverDirectionalTails(scout ScoutData, teamGroups []TeamType) []DirectionalTail {
	var tails []DirectionalTail
	numPositions := len(teamGroups)

	for _, team := range teamGroups {
		id := team.Team_id
		minObserved := scout.MinObservedRank[id]
		maxObserved := scout.MaxObservedRank[id]

		// Better tail (positions < minObserved)
		betterUnseen := 0
		firstBetter := -1
		for pos := minObserved - 1; pos >= 0; pos-- {
			if scout.Feasibility[[2]int{id, pos}] == "feasible_unseen" {
				betterUnseen++
				if firstBetter == -1 {
					firstBetter = pos
				}
			}
		}
		if betterUnseen > 0 {
			tails = append(tails, DirectionalTail{
				TeamID:              id,
				Direction:           RareBetter,
				UnseenFeasibleCount: betterUnseen,
				FirstUnseenPosition: firstBetter,
				ExtremityScore:      scout.TeamMeanRanks[id] - float64(firstBetter),
			})
		}

		// Worse tail (positions > maxObserved)
		worseUnseen := 0
		firstWorse := -1
		for pos := maxObserved + 1; pos < numPositions; pos++ {
			if scout.Feasibility[[2]int{id, pos}] == "feasible_unseen" {
				worseUnseen++
				if firstWorse == -1 {
					firstWorse = pos
				}
			}
		}
		if worseUnseen > 0 {
			tails = append(tails, DirectionalTail{
				TeamID:              id,
				Direction:           RareWorse,
				UnseenFeasibleCount: worseUnseen,
				FirstUnseenPosition: firstWorse,
				ExtremityScore:      float64(firstWorse) - scout.TeamMeanRanks[id],
			})
		}
	}

	// Sort tails by extremity / unseen count descending
	sort.Slice(tails, func(i, j int) bool {
		if tails[i].UnseenFeasibleCount != tails[j].UnseenFeasibleCount {
			return tails[i].UnseenFeasibleCount > tails[j].UnseenFeasibleCount
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
) ProposalProbeStats {
	samples := int(probeWork / proposal.WorkPerSample)
	if samples < 10 {
		samples = 10
	}
	actualWork := int64(samples) * proposal.WorkPerSample

	stats := ProposalProbeStats{
		ProposalID:     proposal.ID,
		Samples:        samples,
		Work:           actualWork,
		RankHistograms: make(map[int][]int, len(teamGroups)),
		CellHits:       make(map[[2]int]int),
		CellSumY:       make(map[[2]int]float64),
		CellSumY2:      make(map[[2]int]float64),
		CellESS:        make(map[[2]int]float64),
	}

	for _, team := range teamGroups {
		stats.RankHistograms[team.Team_id] = make([]int, len(teamGroups))
	}

	components := []ProposalComponent{
		{Name: "P", Weight: DiversifiedDefensiveMixtureEpsilon, Means: originalMeans},
		{Name: "Q", Weight: 1.0 - DiversifiedDefensiveMixtureEpsilon, Means: proposal.Means},
	}
	compWeights := []float64{DiversifiedDefensiveMixtureEpsilon, 1.0 - DiversifiedDefensiveMixtureEpsilon}

	simCampaign := make([]*TeamCampaign, len(baseCampaign))
	teamSlice := make([]*TeamCampaign, len(teamGroups))
	logQBuf := make([]float64, 2)

	for s := 0; s < samples; s++ {
		for k, v := range baseCampaign {
			if v != nil {
				simCampaign[k] = v.clone()
			} else {
				simCampaign[k] = nil
			}
		}

		logQBuf[0], logQBuf[1] = 0.0, 0.0
		// Draw from defensive mixture M = epsilon P + (1-epsilon) Q
		chosen := 1
		if rng.Float64() <= DiversifiedDefensiveMixtureEpsilon {
			chosen = 0
		}
		activeMeans := components[chosen].Means

		for i, g := range games {
			if g.Played {
				continue
			}
			hScore := poissonRand(rng, activeMeans[i].Home)
			aScore := poissonRand(rng, activeMeans[i].Away)

			logQBuf[0] += logPoissonQOverP(hScore, originalMeans[i].Home, originalMeans[i].Home)
			logQBuf[1] += logPoissonQOverP(hScore, originalMeans[i].Home, components[1].Means[i].Home)
			logQBuf[0] += logPoissonQOverP(aScore, originalMeans[i].Away, originalMeans[i].Away)
			logQBuf[1] += logPoissonQOverP(aScore, originalMeans[i].Away, components[1].Means[i].Away)

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
			teamID := teamComp.id
			stats.RankHistograms[teamID][rankPos]++
			cell := [2]int{teamID, rankPos}
			stats.CellHits[cell]++
			stats.CellSumY[cell] += w
			stats.CellSumY2[cell] += w * w
		}
	}

	for cell, sumY := range stats.CellSumY {
		sumY2 := stats.CellSumY2[cell]
		if sumY2 > 0 {
			stats.CellESS[cell] = (sumY * sumY) / sumY2
		}
	}

	return stats
}

func selectCompetitorsForTail(
	targetTeamID int,
	direction RareDirection,
	scout ScoutData,
	teamGroups []TeamType,
	limit int,
) []int {
	targetPos := scout.TeamMeanRanks[targetTeamID]
	type candidate struct {
		id        int
		relevance float64
	}
	var candidates []candidate

	for _, team := range teamGroups {
		id := team.Team_id
		if id == targetTeamID {
			continue
		}
		meanRank := scout.TeamMeanRanks[id]
		diff := meanRank - targetPos
		if direction == RareBetter && diff < 2.0 {
			rel := 1.0 / (1.0 + math.Abs(diff))
			candidates = append(candidates, candidate{id, rel})
		} else if direction == RareWorse && diff > -2.0 {
			rel := 1.0 / (1.0 + math.Abs(diff))
			candidates = append(candidates, candidate{id, rel})
		}
	}

	sort.Slice(candidates, func(i, j int) bool {
		return candidates[i].relevance > candidates[j].relevance
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
	attackTheta := map[int]float64{targetTeamID: s / 2.0}
	concedeTheta := map[int]float64{targetTeamID: -s / 2.0}

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

	attackTheta := map[int]float64{targetTeamID: s / 2.0}
	concedeTheta := map[int]float64{targetTeamID: -s / 2.0}

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
		compShift := competitorMass * normW * compSign
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
	plainWorkPerSample := estimateSeasonWork(unplayedGames, 1, len(teamGroups))
	plainProp := DiversifiedProposal{
		ID:            "plain_mc",
		Kind:          ProposalPlainMC,
		Means:         original,
		KL:            0.0,
		WorkPerSample: plainWorkPerSample,
	}
	retained = append(retained, plainProp)

	plainStats := ProposalProbeStats{
		ProposalID:     "plain_mc",
		Samples:        scout.Samples,
		Work:           scout.Work,
		RankHistograms: scout.TeamCounts,
		CellHits:       make(map[[2]int]int),
		CellSumY:       make(map[[2]int]float64),
		CellSumY2:      make(map[[2]int]float64),
		CellESS:        make(map[[2]int]float64),
	}
	for teamID, counts := range scout.TeamCounts {
		for pos, c := range counts {
			cell := [2]int{teamID, pos}
			plainStats.CellHits[cell] = c
			plainStats.CellSumY[cell] = float64(c)
			plainStats.CellSumY2[cell] = float64(c)
			if c > 0 {
				plainStats.CellESS[cell] = float64(c)
			}
		}
	}
	probeStatsMap["plain_mc"] = plainStats

	tails := discoverDirectionalTails(scout, teamGroups)
	teamIDs := teamIDsFromGroups(teamGroups)
	numTeams := len(teamGroups)

	workSpent := int64(0)
	strengthLadder := []float64{0.25, 0.50, 0.75, 1.00, 1.50, 2.00}

	for _, tail := range tails {
		if workSpent >= searchWorkCap {
			break
		}

		retainedForTail := 0
		singleTeamFailed := false

		for _, s := range strengthLadder {
			if workSpent >= searchWorkCap || retainedForTail >= DiversifiedMaxSingleTeamPerDirection {
				break
			}

			prop, ok := buildDirectTeamProposal(tail.TeamID, tail.Direction, s, original, games, teamIDs, numTeams)
			if !ok {
				singleTeamFailed = true
				continue
			}

			probeWork := int64(DiversifiedProbeChunkWork)
			if workSpent+probeWork > searchWorkCap {
				probeWork = searchWorkCap - workSpent
			}
			if probeWork < 10*prop.WorkPerSample {
				break
			}

			stats := probeProposal(prop, baseCampaign, games, original, table, sortOrder, teamGroups, probeWork, rng)
			workSpent += stats.Work

			usefulHits := 0
			for pos := 0; pos < numTeams; pos++ {
				cell := [2]int{tail.TeamID, pos}
				if scout.Feasibility[cell] == "feasible_unseen" && stats.CellHits[cell] > 0 {
					usefulHits += stats.CellHits[cell]
				}
			}

			if usefulHits > 0 {
				retained = append(retained, prop)
				probeStatsMap[prop.ID] = stats
				retainedForTail++
				log.Printf("rare-position-diversified-search: team=%d direction=%s prop=%s strength=%.2f kl=%.3f useful_hits=%d decision=retain",
					tail.TeamID, directionName(tail.Direction), prop.ID, s, prop.KL, usefulHits)
			} else if s >= 1.0 {
				singleTeamFailed = true
			}
		}

		if singleTeamFailed && retainedForTail == 0 && workSpent < searchWorkCap {
			competitors := selectCompetitorsForTail(tail.TeamID, tail.Direction, scout, teamGroups, DiversifiedMaxCompetitors)
			if len(competitors) > 0 {
				for _, s := range []float64{0.75, 1.25} {
					if workSpent >= searchWorkCap || retainedForTail >= 1 {
						break
					}

					prop, ok := buildCompetitorAssistedProposal(tail.TeamID, tail.Direction, s, competitors, 0.50, scout.TeamMeanRanks, original, games, teamIDs, numTeams)
					if !ok {
						continue
					}

					probeWork := int64(DiversifiedProbeChunkWork)
					if workSpent+probeWork > searchWorkCap {
						probeWork = searchWorkCap - workSpent
					}
					if probeWork < 10*prop.WorkPerSample {
						break
					}

					stats := probeProposal(prop, baseCampaign, games, original, table, sortOrder, teamGroups, probeWork, rng)
					workSpent += stats.Work

					usefulHits := 0
					for pos := 0; pos < numTeams; pos++ {
						cell := [2]int{tail.TeamID, pos}
						if scout.Feasibility[cell] == "feasible_unseen" && stats.CellHits[cell] > 0 {
							usefulHits += stats.CellHits[cell]
						}
					}

					if usefulHits > 0 {
						retained = append(retained, prop)
						probeStatsMap[prop.ID] = stats
						retainedForTail++
						log.Printf("rare-position-diversified-search: team=%d direction=%s prop=%s strength=%.2f competitors=%v kl=%.3f useful_hits=%d decision=retain_competitor",
							tail.TeamID, directionName(tail.Direction), prop.ID, s, competitors, prop.KL, usefulHits)
					}
				}
			}
		}

		if retainedForTail == 0 {
			log.Printf("rare-position-diversified-search: team=%d direction=%s decision=search_exhausted",
				tail.TeamID, directionName(tail.Direction))
		}
	}

	return retained, probeStatsMap, workSpent
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

	// 1. Minimum Plain-MC share
	minPlainWork := int64(float64(productionWorkBudget) * diversifiedMinPlainFraction())
	allocatedWork := make(map[string]int64, len(proposals))
	allocatedWork["plain_mc"] = minPlainWork

	remainingProdWork := productionWorkBudget - minPlainWork

	// 2. Compute search efficiency per proposal per feasible cell
	// efficiency[propID][cell] = ESS / work
	efficiencies := make(map[string]map[[2]int]float64, len(proposals))
	for _, prop := range proposals {
		stats := probeStats[prop.ID]
		effMap := make(map[[2]int]float64)
		for cell, ess := range stats.CellESS {
			if stats.Work > 0 && ess > 0 {
				effMap[cell] = ess / float64(stats.Work)
			}
		}
		efficiencies[prop.ID] = effMap
	}

	// 3. Greedy Marginal-Utility Allocation
	feasibleCells := make([][2]int, 0)
	for _, team := range teamGroups {
		for pos := 0; pos < numPositions; pos++ {
			cell := [2]int{team.Team_id, pos}
			if scout.Feasibility[cell] != "proven_impossible" {
				feasibleCells = append(feasibleCells, cell)
			}
		}
	}

	allocChunk := int64(DiversifiedAllocChunkWork)
	for remainingProdWork >= allocChunk {
		// Calculate current predicted ESS per cell
		predictedESS := make(map[[2]int]float64, len(feasibleCells))
		for _, cell := range feasibleCells {
			totESS := 0.0
			for _, prop := range proposals {
				work := allocatedWork[prop.ID]
				totESS += float64(work) * efficiencies[prop.ID][cell]
			}
			predictedESS[cell] = totESS
		}

		bestPropID := ""
		bestUtility := -1.0

		for _, prop := range proposals {
			// Utility = sum_cell min(deficit, delta_ESS) / allocChunk
			totGain := 0.0
			chunkSamples := allocChunk / prop.WorkPerSample
			actualChunkWork := chunkSamples * prop.WorkPerSample
			if actualChunkWork <= 0 || actualChunkWork > remainingProdWork {
				continue
			}

			for _, cell := range feasibleCells {
				deficit := math.Max(0, DiversifiedTargetESS-predictedESS[cell])
				deltaESS := float64(actualChunkWork) * efficiencies[prop.ID][cell]
				totGain += math.Min(deficit, deltaESS)
			}

			utility := totGain / float64(actualChunkWork)
			if utility > bestUtility {
				bestUtility = utility
				bestPropID = prop.ID
			}
		}

		if bestPropID == "" || bestUtility <= 0 {
			// Deficits saturated or no proposal offers utility; allocate rest to plain MC
			allocatedWork["plain_mc"] += remainingProdWork
			remainingProdWork = 0
			break
		}

		// Allocate chunk to best proposal
		prop := findProposalByID(proposals, bestPropID)
		chunkSamples := allocChunk / prop.WorkPerSample
		actualChunkWork := chunkSamples * prop.WorkPerSample
		allocatedWork[bestPropID] += actualChunkWork
		remainingProdWork -= actualChunkWork
	}

	// Any small leftover unallocated production work goes to plain MC
	if remainingProdWork > 0 {
		allocatedWork["plain_mc"] += remainingProdWork
		remainingProdWork = 0
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

	// 4. Compute search-derived combination weights (beta_j,A) for each cell
	cellBetas := make(map[[2]int][]float64, len(feasibleCells))
	for _, cell := range feasibleCells {
		betas := make([]float64, len(batches))
		sumScore := 0.0
		for j, batch := range batches {
			stats := probeStats[batch.Proposal.ID]
			// Regularized efficiency: search_ESS / (search_samples + 10.0)
			ess := stats.CellESS[cell]
			score := ess / (float64(stats.Samples) + 10.0)
			if batch.Proposal.Kind == ProposalPlainMC {
				score += 1e-6 // Ensure plain MC always has non-zero fallback score
			}
			betas[j] = score
			sumScore += score
		}

		if sumScore > 0 {
			for j := range betas {
				betas[j] /= sumScore
			}
			// Cap non-plain proposal beta at 0.90 and allocate excess to Plain MC
			plainIndex := -1
			excess := 0.0
			for j := range betas {
				if batches[j].Proposal.Kind == ProposalPlainMC {
					plainIndex = j
				} else if betas[j] > 0.90 {
					excess += betas[j] - 0.90
					betas[j] = 0.90
				}
			}
			if plainIndex >= 0 && excess > 0 {
				betas[plainIndex] += excess
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

	return FrozenDiversifiedDesign{
		Batches:                batches,
		CellCombinationWeights: cellBetas,
		TotalWork:              totalWorkBudget,
		ScoutWork:              scoutWork,
		SearchWork:             searchWork,
		ProductionWork:         actualProductionWork,
		UnusedWork:             unusedWork,
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
