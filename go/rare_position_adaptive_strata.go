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

type AdaptiveStratumAdmitted struct {
	TeamID         int     `json:"team_id"`
	MaxRank        int     `json:"max_rank"`
	Threshold      int     `json:"threshold"`
	Mass           float64 `json:"mass"`
	ValidationHits int     `json:"validation_hits"`
	Samples        int     `json:"samples"`
	Work           int64   `json:"work"`
}

type AdaptiveStratumDiagnostics struct {
	ScoutWork        int64                       `json:"scout_work"`
	DiscoveryWork    int64                       `json:"discovery_work"`
	ValidationWork   int64                       `json:"validation_work"`
	ProductionWork   int64                       `json:"production_work"`
	TotalWork        int64                       `json:"total_work"`
	TotalWorkLimit   int64                       `json:"total_work_limit"`
	PlainSamples     int                         `json:"plain_samples"`
	Feasibility      map[[2]int]string           `json:"feasibility,omitempty"`
	UpperBounds      map[[2]int]float64          `json:"upper_bounds,omitempty"`
	AdmittedStrata   []AdaptiveStratumAdmitted   `json:"admitted_strata,omitempty"`
	ReconciledMatrix map[int]map[int]float64     `json:"reconciled_matrix,omitempty"`
}

type candidateStratumTarget struct {
	TeamID      int
	MaxRank     int
	MinPoints   int
	Universe    *PointStratumUniverse
	Stratum     *PointStratum
	UpperBounds map[int]float64
}

func simulateAdaptivePlainSeasons(
	base []*TeamCampaign, games []*GameType, table *Table, order []SortType,
	teamGroups []TeamType, strata map[int]*PointStratum,
	samples int, seed int64,
) (plainCounts map[[2]int]int, outsideCounts map[[2]int]int) {
	rng := rand.New(rand.NewSource(seed))
	plainCounts = make(map[[2]int]int, len(teamGroups)*len(teamGroups))
	outsideCounts = make(map[[2]int]int, len(teamGroups)*len(teamGroups))

	type teamUniverseInfo struct {
		universe *PointStratumUniverse
		stratum  *PointStratum
	}
	infoByTeam := make(map[int]teamUniverseInfo, len(strata))
	for teamID, stratum := range strata {
		if stratum != nil && stratum.Universe != nil {
			infoByTeam[teamID] = teamUniverseInfo{
				universe: stratum.Universe,
				stratum:  stratum,
			}
		}
	}

	campaign := make([]*TeamCampaign, len(base))
	teamSlice := make([]*TeamCampaign, len(teamGroups))

	for s := 0; s < samples; s++ {
		for i, c := range base {
			if c != nil {
				campaign[i] = c.clone()
			} else {
				campaign[i] = nil
			}
		}

		observedCodes := make(map[int]uint64, len(infoByTeam))

		for i, game := range games {
			if game.Played {
				continue
			}
			hs := poissonRand(rng, game.HomePower)
			as := poissonRand(rng, game.AwayPower)

			for teamID, info := range infoByTeam {
				if slot, ok := info.universe.GameSlots[i]; ok {
					digit := targetOutcomeDigit(game.HomeId == teamID, hs, as)
					observedCodes[teamID] += uint64(digit) * info.universe.GamePowers[slot]
				}
			}

			home, away := game.home_table_index, game.away_table_index
			played := &GameType{game.Id, game.HomeId, game.AwayId, hs, as, 0, 0, true, home, away}
			if campaign[home] != nil {
				campaign[home].add_game(played)
			}
			if campaign[away] != nil {
				campaign[away].add_game(played)
			}
		}

		idx := 0
		for _, team := range teamGroups {
			if c := campaign[table.Query(uint32(team.Team_id))]; c != nil {
				teamSlice[idx] = c
				idx++
			}
		}
		sort.Sort(TeamCampaignSorted{t: teamSlice[:idx], sort: order, rng: rng})

		for rank, c := range teamSlice[:idx] {
			cell := [2]int{c.id, rank}
			plainCounts[cell]++
			if info, ok := infoByTeam[c.id]; ok {
				code := observedCodes[c.id]
				if !info.stratum.contains(code) {
					outsideCounts[cell]++
				}
			}
		}
	}

	return plainCounts, outsideCounts
}

// runAdaptivePointStratifiedSearch automatically discovers team-position point
// strata across all teams and ranks, validates them in design, freezes work
// allocation, and performs fresh production simulation.
func runAdaptivePointStratifiedSearch(
	group *GroupType, base []*TeamCampaign, table *Table, order []SortType,
	workLimit, masterSeed int64,
) (map[int]map[int]ProductionEstimate, AdaptiveStratumDiagnostics, bool) {
	diag := AdaptiveStratumDiagnostics{
		TotalWorkLimit:   workLimit,
		Feasibility:      make(map[[2]int]string),
		UpperBounds:      make(map[[2]int]float64),
		ReconciledMatrix: make(map[int]map[int]float64),
	}

	numTeams := len(group.Team_groups)
	unplayed := 0
	for _, game := range group.Games {
		if !game.Played {
			unplayed++
		}
	}
	plainCost := estimateSeasonWork(unplayed, 1, numTeams)
	stratumCost := estimateSeasonWork(unplayed, 2, numTeams)
	if plainCost <= 0 || stratumCost <= 0 {
		return nil, diag, false
	}

	// 1. Initial plain-MC design scout run
	scoutRequested := 15000
	if raw := os.Getenv("RARE_POSITION_DIVERSIFIED_SCOUT_SAMPLES"); raw != "" {
		if parsed, err := strconv.Atoi(raw); err == nil && parsed > 0 {
			scoutRequested = parsed
		}
	}
	scoutSamples := affordableSamples(scoutRequested, workLimit, plainCost)
	if scoutSamples <= 0 {
		return nil, diag, false
	}
	diag.ScoutWork = int64(scoutSamples) * plainCost

	scoutRNG := rand.New(rand.NewSource(deriveRarePositionSeed(masterSeed, "adaptive-scout")))
	scout := runPlainMCScout(base, group.Games, table, order, group.Team_groups, scoutSamples, plainCost, scoutRNG)

	for cell, status := range scout.Feasibility {
		diag.Feasibility[cell] = status
	}

	minInterestProb := MinInterestingProbability
	if raw := os.Getenv("RARE_POSITION_MIN_INTERESTING_PROBABILITY"); raw != "" {
		if parsed, err := strconv.ParseFloat(raw, 64); err == nil && parsed > 0 {
			minInterestProb = parsed
		}
	}

	// 2 & 3 & 4. Points/rank analysis, exact PMF, and upper bounds per team
	candidateTargets := make([]candidateStratumTarget, 0, numTeams)
	discoveryWorkTotal := int64(0)

	for _, team := range group.Team_groups {
		teamID := team.Team_id
		universe, ok := pointOutcomeUniverse(teamID, base, table, group.Games, 39)
		if !ok || universe == nil {
			continue
		}
		discoveryWorkTotal += int64(len(universe.GameIndices) * 1000)

		upperBounds := make(map[int]float64, numTeams)
		maxTargetRank := -1
		minPointsForMaxRank := 0

		for pos := 0; pos < numTeams; pos++ {
			cell := [2]int{teamID, pos}
			if scout.Feasibility[cell] == "proven_impossible" {
				diag.UpperBounds[cell] = 0
				continue
			}
			if scout.Feasibility[cell] == "observed" {
				pObs := float64(scout.TeamCounts[teamID][pos]) / float64(scoutSamples)
				diag.UpperBounds[cell] = pObs
				continue
			}

			// Reachable but unobserved in scout
			minReq, ok := minimumPointsForRank(teamID, pos, base)
			if !ok {
				diag.UpperBounds[cell] = 0
				continue
			}

			tailStratum, ok := makePointTailStratum(universe, minReq)
			if !ok || tailStratum == nil {
				diag.UpperBounds[cell] = 0
				continue
			}

			ub := tailStratum.Mass
			diag.UpperBounds[cell] = ub
			upperBounds[pos] = ub

			if ub >= minInterestProb {
				if pos > maxTargetRank {
					maxTargetRank = pos
					minPointsForMaxRank = minReq
				}
			}
		}

		if maxTargetRank >= 0 {
			// Find threshold T >= minPointsForMaxRank
			threshold := minPointsForMaxRank + 6
			targetMass := 0.0015
			if raw := os.Getenv("RARE_POSITION_POINT_HYBRID_TARGET_MASS"); raw != "" {
				if parsed, err := strconv.ParseFloat(raw, 64); err == nil && parsed > 0 && parsed < 1 {
					targetMass = parsed
				}
			}

			maxPossibleAdded := 0
			for _, gains := range universe.OutcomeGains {
				maxG := gains[0]
				for _, g := range gains[1:] {
					if g > maxG {
						maxG = g
					}
				}
				maxPossibleAdded += maxG
			}

			bestDist := math.Inf(1)
			for candidateT := minPointsForMaxRank; candidateT <= maxPossibleAdded; candidateT++ {
				tail, exists := makePointTailStratum(universe, candidateT)
				if !exists || tail == nil || tail.Mass <= 0 {
					continue
				}
				dist := math.Abs(math.Log(tail.Mass / targetMass))
				if dist < bestDist {
					bestDist = dist
					threshold = candidateT
				}
			}

			stratum, ok := makePointTailStratum(universe, threshold)
			if ok && stratum != nil && stratum.Mass > 0 {
				candidateTargets = append(candidateTargets, candidateStratumTarget{
					TeamID:      teamID,
					MaxRank:     maxTargetRank,
					MinPoints:   minPointsForMaxRank,
					Universe:    universe,
					Stratum:     stratum,
					UpperBounds: upperBounds,
				})
			}
		}
	}

	diag.DiscoveryWork = discoveryWorkTotal

	// 5. Validation probes on candidate strata (Design)
	valSamplesPerStratum := 1000
	valWorkTotal := int64(len(candidateTargets) * valSamplesPerStratum) * stratumCost
	if diag.ScoutWork+diag.DiscoveryWork+valWorkTotal+4*plainCost > workLimit {
		valSamplesPerStratum = 200
		valWorkTotal = int64(len(candidateTargets) * valSamplesPerStratum) * stratumCost
		if diag.ScoutWork+diag.DiscoveryWork+valWorkTotal+4*plainCost > workLimit {
			valSamplesPerStratum = 0
			valWorkTotal = 0
		}
	}

	diag.ValidationWork = valWorkTotal

	admittedStrata := make([]candidateStratumTarget, 0, len(candidateTargets))
	valHitsMap := make(map[int]int)

	if valSamplesPerStratum > 0 {
		for _, target := range candidateTargets {
			valSeed := deriveRarePositionSeed(masterSeed, fmt.Sprintf("adaptive-val-team%d", target.TeamID))
			valCounts, _ := simulatePointHybridSeasons(base, group.Games, table, order, group.Team_groups,
				target.Stratum, true, valSamplesPerStratum, valSeed)

			hits := 0
			for pos := 0; pos <= target.MaxRank; pos++ {
				hits += valCounts[[2]int{target.TeamID, pos}]
			}
			valHitsMap[target.TeamID] = hits

			if hits >= 2 {
				admittedStrata = append(admittedStrata, target)
			}
		}
	}

	// 6. Freeze work allocation
	remainingWork := workLimit - diag.ScoutWork - diag.DiscoveryWork - diag.ValidationWork
	if remainingWork <= 0 {
		return nil, diag, false
	}

	qPercentPerStratum := int64(2)
	if raw := os.Getenv("RARE_POSITION_POINT_HYBRID_Q_PERCENT"); raw != "" {
		if parsed, err := strconv.ParseInt(raw, 10, 64); err == nil && parsed >= 0 && parsed <= 50 {
			qPercentPerStratum = parsed
		}
	}

	type admittedProdStream struct {
		target   candidateStratumTarget
		samples  int
		work     int64
		valHits  int
	}

	prodStreams := make([]admittedProdStream, 0, len(admittedStrata))
	totalStratumWork := int64(0)

	if len(admittedStrata) > 0 {
		for _, target := range admittedStrata {
			samples := int((remainingWork * qPercentPerStratum / 100) / stratumCost)
			if samples > 0 {
				work := int64(samples) * stratumCost
				totalStratumWork += work
				prodStreams = append(prodStreams, admittedProdStream{
					target:   target,
					samples:  samples,
					work:     work,
					valHits:  valHitsMap[target.TeamID],
				})
			}
		}
	}

	plainSamples := int((remainingWork - totalStratumWork) / plainCost)
	if plainSamples <= 1 {
		prodStreams = nil
		totalStratumWork = 0
		plainSamples = int(remainingWork / plainCost)
		if plainSamples <= 1 {
			return nil, diag, false
		}
	}

	diag.PlainSamples = plainSamples
	diag.ProductionWork = int64(plainSamples)*plainCost + totalStratumWork

	if diag.ScoutWork+diag.DiscoveryWork+diag.ValidationWork+diag.ProductionWork > workLimit {
		panic("adaptive stratum work budget exceeded")
	}

	for _, ps := range prodStreams {
		diag.AdmittedStrata = append(diag.AdmittedStrata, AdaptiveStratumAdmitted{
			TeamID:         ps.target.TeamID,
			MaxRank:        ps.target.MaxRank,
			Threshold:      ps.target.Stratum.Tail.Threshold,
			Mass:           ps.target.Stratum.Mass,
			ValidationHits: ps.valHits,
			Samples:        ps.samples,
			Work:           ps.work,
		})
	}

	// 7 & 8. Fresh production simulation and raw cell estimates
	strataByTeam := make(map[int]*PointStratum, len(prodStreams))
	for _, ps := range prodStreams {
		strataByTeam[ps.target.TeamID] = ps.target.Stratum
	}

	plainCounts, plainOutsideCounts := simulateAdaptivePlainSeasons(base, group.Games, table, order, group.Team_groups,
		strataByTeam, plainSamples, deriveRarePositionSeed(masterSeed, "adaptive-prod-P"))

	// Map targetTeamID -> admittedProdStream
	streamByTeam := make(map[int]admittedProdStream, len(prodStreams))
	condCountsByTeam := make(map[int]map[[2]int]int, len(prodStreams))

	for _, ps := range prodStreams {
		streamByTeam[ps.target.TeamID] = ps
		qSeed := deriveRarePositionSeed(masterSeed, fmt.Sprintf("adaptive-prod-Q-team%d", ps.target.TeamID))
		qCounts, _ := simulatePointHybridSeasons(base, group.Games, table, order, group.Team_groups,
			ps.target.Stratum, true, ps.samples, qSeed)
		condCountsByTeam[ps.target.TeamID] = qCounts
	}

	estimates := make(map[int]map[int]ProductionEstimate, numTeams)
	rawProbs := make(map[int]map[int]float64, numTeams)

	for _, team := range group.Team_groups {
		id := team.Team_id
		estimates[id] = make(map[int]ProductionEstimate, numTeams)
		rawProbs[id] = make(map[int]float64, numTeams)

		ps, hasStratum := streamByTeam[id]

		for pos := 0; pos < numTeams; pos++ {
			cell := [2]int{id, pos}
			plainHits := plainCounts[cell]
			pPlain := float64(plainHits) / float64(plainSamples)

			p := pPlain
			varCell := pPlain * (1 - pPlain) / float64(plainSamples)
			hits := plainHits
			samples := plainSamples
			designName := "plain_mc"

			if hasStratum && pos <= ps.target.MaxRank {
				designName = "adaptive_point_stratum"
				outsideHits := plainOutsideCounts[cell]
				pOutside := float64(outsideHits) / float64(plainSamples)

				qCounts := condCountsByTeam[id]
				insideHits := qCounts[cell]
				pInside := float64(insideHits) / float64(ps.samples)

				stratumMass := ps.target.Stratum.Mass
				p = pOutside + stratumMass*pInside
				varCell = pOutside*(1-pOutside)/float64(plainSamples) +
					stratumMass*stratumMass*pInside*(1-pInside)/float64(ps.samples)
				hits = outsideHits + insideHits
				samples = plainSamples + ps.samples
			}

			est := pointHybridEstimate(p, varCell, hits, samples, diag.ProductionWork, designName)

			if hasStratum && pos <= ps.target.MaxRank && hits == 0 {
				stratumMass := ps.target.Stratum.Mass
				est.ZeroHitUpper95 = math.Min(1, -math.Log(0.025)/float64(plainSamples)+stratumMass*(-math.Log(0.025))/float64(ps.samples))
			}

			estimates[id][pos] = est
			rawProbs[id][pos] = est.Probability
		}
	}

	// 9. Reporting-only reconciled matrix
	teamIDs := teamIDsFromGroups(group.Team_groups)
	diag.ReconciledMatrix = reconcileProbabilityMatrix(rawProbs, teamIDs)

	log.Printf("rare-position-adaptive-strata-summary: group=%d scout_work=%d discovery_work=%d validation_work=%d production_work=%d total_work=%d admitted_strata=%d",
		group.Id, diag.ScoutWork, diag.DiscoveryWork, diag.ValidationWork, diag.ProductionWork,
		diag.ScoutWork+diag.DiscoveryWork+diag.ValidationWork+diag.ProductionWork, len(diag.AdmittedStrata))

	return estimates, diag, true
}

// reconcileProbabilityMatrix applies Sinkhorn-Knopp doubly-stochastic matrix
// normalization to produce a separately labeled reconciled N x N matrix.
func reconcileProbabilityMatrix(raw map[int]map[int]float64, teams []int) map[int]map[int]float64 {
	n := len(teams)
	reconciled := make(map[int]map[int]float64, n)
	for _, t := range teams {
		reconciled[t] = make(map[int]float64, n)
		for r := 0; r < n; r++ {
			reconciled[t][r] = raw[t][r]
		}
	}

	for iter := 0; iter < 100; iter++ {
		// Row normalization
		for _, t := range teams {
			rowSum := 0.0
			for r := 0; r < n; r++ {
				rowSum += reconciled[t][r]
			}
			if rowSum > 0 {
				for r := 0; r < n; r++ {
					reconciled[t][r] /= rowSum
				}
			}
		}

		// Column normalization
		for r := 0; r < n; r++ {
			colSum := 0.0
			for _, t := range teams {
				colSum += reconciled[t][r]
			}
			if colSum > 0 {
				for _, t := range teams {
					reconciled[t][r] /= colSum
				}
			}
		}
	}

	return reconciled
}
