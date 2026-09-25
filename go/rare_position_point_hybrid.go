package main

import (
	"fmt"
	"math"
	"math/rand"
	"os"
	"sort"
	"strconv"
)

type PointHybridDiagnostics struct {
	ScoutWork          int64
	DiscoveryWork      int64
	ValidationWork     int64
	ProductionWork     int64
	PlainSamples       int
	StratumSamples     int
	StratumMass        float64
	PointsTailMass     float64
	MinAddedPoints     int
	Threshold          int
	DiscoveryHits      int
	ValidationHits     int
	ValidationRankHits []int
	ValidationSamples  int
	Feasibility        map[[2]int]string
}

// simulatePointHybridSeasons samples either P or P conditioned on a finite
// target-team W/D/L stratum. It never uses the offline reference matrix.
func simulatePointHybridSeasons(
	base []*TeamCampaign, games []*GameType, table *Table, order []SortType,
	teams []TeamType, stratum *PointStratum, conditional bool,
	samples int, seed int64,
) (map[[2]int]int, []int) {
	universe := stratum.Universe
	rng := rand.New(rand.NewSource(seed))
	counts := make(map[[2]int]int, len(teams)*len(teams))
	complement := make([]int, len(teams))
	campaign := make([]*TeamCampaign, len(base))
	sorted := make([]*TeamCampaign, len(teams))
	for sample := 0; sample < samples; sample++ {
		for i, c := range base {
			if c != nil {
				campaign[i] = c.clone()
			} else {
				campaign[i] = nil
			}
		}
		chosenCode := uint64(0)
		if conditional {
			chosenCode = stratum.samplePattern(rng)
		}
		observedCode := uint64(0)
		for i, game := range games {
			if game.Played {
				continue
			}
			hs, as := 0, 0
			if slot, ok := universe.GameSlots[i]; ok && conditional {
				digit := pointStratumDigit(chosenCode, slot)
				hs, as = samplePointStratumScore(rng, game, GameProposalMeans{game.HomePower, game.AwayPower}, universe.TargetTeam, digit)
			} else {
				hs = poissonRand(rng, game.HomePower)
				as = poissonRand(rng, game.AwayPower)
			}
			if slot, ok := universe.GameSlots[i]; ok {
				digit := targetOutcomeDigit(game.HomeId == universe.TargetTeam, hs, as)
				observedCode += uint64(digit) * universe.GamePowers[slot]
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
		if conditional && !stratum.contains(observedCode) {
			panic("conditional point-stratum sample escaped its stratum")
		}
		idx := 0
		for _, team := range teams {
			if c := campaign[table.Query(uint32(team.Team_id))]; c != nil {
				sorted[idx] = c
				idx++
			}
		}
		sort.Sort(TeamCampaignSorted{t: sorted[:idx], sort: order, rng: rng})
		for rank, c := range sorted[:idx] {
			counts[[2]int{c.id, rank}]++
			if c.id == universe.TargetTeam && !conditional && !stratum.contains(observedCode) {
				complement[rank]++
			}
		}
	}
	return counts, complement
}

func pointHybridEstimate(probability, variance float64, hits, samples int, work int64, design string) ProductionEstimate {
	if variance < 0 {
		variance = 0
	}
	se := math.Sqrt(variance)
	ess := 0.0
	if variance > 0 {
		ess = probability * probability / variance
	} else if probability == 1 {
		ess = float64(samples)
	}
	rel := math.Inf(1)
	if probability > 0 {
		rel = se / probability
	}
	upper := probability + 1.96*se
	if hits == 0 {
		upper = zeroHitUpper95(samples)
	}
	return ProductionEstimate{Probability: probability, StdErr: se, RelativeSE: relativeSEPointer(rel), ESS: ess,
		Hits: hits, Samples: samples, WorkSpent: work, MeanWeight: 1,
		Available: samples > 0, MeetsPrecisionGoal: estimateMeetsPrecisionGoal(ess, rel), ZeroHitUpper95: upper, Design: design}
}

func pointHybridDescription(diag PointHybridDiagnostics) string {
	return fmt.Sprintf("point stratum: P(points>=minimum)=%.9g P(A)=%.9g minimum=%d threshold=%d validation_hits=%d P_samples=%d Q_samples=%d",
		diag.PointsTailMass, diag.StratumMass, diag.MinAddedPoints, diag.Threshold, diag.ValidationHits, diag.PlainSamples, diag.StratumSamples)
}

func recordPointHybridDiagnostics(
	diagnostics *DiversifiedRunDiagnostics, hybrid PointHybridDiagnostics,
	targetTeam, maxRank int, plainCost, pointCost, limit int64,
) {
	if diagnostics == nil {
		return
	}
	proposalID := pointStratumProposalID(targetTeam, maxRank, hybrid.Threshold)
	proposal := DiversifiedProposal{ID: proposalID, Kind: "point_outcome_stratum", TargetTeam: targetTeam,
		Strength: float64(hybrid.Threshold), KL: -math.Log(hybrid.StratumMass), WorkPerSample: pointCost}
	diagnostics.Feasibility = hybrid.Feasibility
	diagnostics.Candidates, diagnostics.Shortlisted, diagnostics.Validated = 1, 1, 1
	diagnostics.Shortlist = []DiversifiedProposal{proposal}
	diagnostics.ValidationSamples = map[string]int{proposalID: hybrid.ValidationSamples}
	for pos, hits := range hybrid.ValidationRankHits {
		p := float64(hits) / float64(hybrid.ValidationSamples)
		diagnostics.ValidationCells = append(diagnostics.ValidationCells, DiversifiedValidationCell{
			Proposal: proposalID, Team: targetTeam, Position: pos, Intended: true, ESS: float64(hits),
			Eligible: hits >= 2, VariancePerSample: hybrid.StratumMass * hybrid.StratumMass * p * (1 - p),
		})
	}
	diagnostics.Design = FrozenDiversifiedDesign{TotalWork: limit, ScoutWork: hybrid.ScoutWork,
		DiscoveryWork: hybrid.DiscoveryWork, ValidationWork: hybrid.ValidationWork, ProductionWork: hybrid.ProductionWork,
		UnusedWork: limit - hybrid.ScoutWork - hybrid.DiscoveryWork - hybrid.ValidationWork - hybrid.ProductionWork,
		Batches: []FrozenProductionBatch{{Proposal: DiversifiedProposal{ID: "plain_mc", Kind: ProposalPlainMC, WorkPerSample: plainCost},
			Samples: hybrid.PlainSamples, Work: int64(hybrid.PlainSamples) * plainCost, WorkPerSample: plainCost}}}
	if hybrid.StratumSamples > 0 {
		diagnostics.Design.Batches = append(diagnostics.Design.Batches,
			FrozenProductionBatch{Proposal: proposal, Samples: hybrid.StratumSamples,
				Work: int64(hybrid.StratumSamples) * pointCost, WorkPerSample: pointCost})
	}
}

// runPointHybrid keeps the broad matrix on P and estimates selected target
// cells by disjoint P-outside-A and P(A)*P(rank|A) contributions. The stratum
// and sample counts are frozen before either production stream starts.
func runPointHybrid(
	group *GroupType, base []*TeamCampaign, table *Table, order []SortType,
	workLimit, masterSeed int64, targetTeam, maxRank int,
) (map[int]map[int]ProductionEstimate, PointHybridDiagnostics, bool) {
	diag := PointHybridDiagnostics{}
	unplayed := 0
	for _, game := range group.Games {
		if !game.Played {
			unplayed++
		}
	}
	plainCost := estimateSeasonWork(unplayed, 1, len(group.Team_groups))
	stratumCost := estimateSeasonWork(unplayed, 2, len(group.Team_groups))
	if plainCost <= 0 || stratumCost <= 0 {
		return nil, diag, false
	}
	universe, ok := pointOutcomeUniverse(targetTeam, base, table, group.Games, 39)
	if !ok {
		return nil, diag, false
	}
	minimum, ok := minimumPointsForRank(targetTeam, maxRank, base)
	if !ok {
		return nil, diag, false
	}
	diag.MinAddedPoints = minimum
	minimumTail, ok := makePointTailStratum(universe, minimum)
	if !ok {
		return nil, diag, false
	}
	diag.PointsTailMass = minimumTail.Mass
	threshold := minimum + 6
	if raw := os.Getenv("RARE_POSITION_POINT_HYBRID_TARGET_MASS"); raw != "" {
		targetMass, err := strconv.ParseFloat(raw, 64)
		if err != nil || targetMass <= 0 || targetMass >= 1 {
			return nil, diag, false
		}
		maxAdded := 0
		for _, gains := range universe.OutcomeGains {
			maxGain := gains[0]
			for _, gain := range gains[1:] {
				if gain > maxGain {
					maxGain = gain
				}
			}
			maxAdded += maxGain
		}
		bestDistance := math.Inf(1)
		for candidate := minimum; candidate <= maxAdded; candidate++ {
			tail, exists := makePointTailStratum(universe, candidate)
			if !exists {
				continue
			}
			distance := math.Abs(math.Log(tail.Mass / targetMass))
			if distance < bestDistance {
				bestDistance = distance
				threshold = candidate
			}
		}
	}
	stratum, ok := makePointTailStratum(universe, threshold)
	if !ok {
		return nil, diag, false
	}
	diag.Threshold = threshold
	diag.StratumMass = stratum.Mass
	// Charge the outcome probability table and points dynamic program as
	// design work. The ternary outcome code supports at most 39 fixtures.
	diag.DiscoveryWork = int64(len(universe.GameIndices) * 1000)
	probeSamples := 200
	diag.DiscoveryWork += int64(probeSamples) * stratumCost
	scoutSamples := affordableSamples(1000, workLimit, plainCost)
	diag.ScoutWork = int64(scoutSamples) * plainCost
	validationSamples := 1000
	diag.ValidationSamples = validationSamples
	diag.ValidationWork = int64(validationSamples) * stratumCost
	if scoutSamples == 0 || diag.ScoutWork+diag.DiscoveryWork+diag.ValidationWork+4*plainCost > workLimit {
		return nil, diag, false
	}
	scout := runPlainMCScout(base, group.Games, table, order, group.Team_groups,
		scoutSamples, plainCost, rand.New(rand.NewSource(deriveRarePositionSeed(masterSeed, "point-hybrid-scout"))))
	diag.Feasibility = scout.Feasibility
	probe, _ := simulatePointHybridSeasons(base, group.Games, table, order, group.Team_groups,
		stratum, true, probeSamples, deriveRarePositionSeed(masterSeed, "point-hybrid-discovery"))
	for pos := 0; pos <= maxRank; pos++ {
		diag.DiscoveryHits += probe[[2]int{targetTeam, pos}]
	}
	validation, _ := simulatePointHybridSeasons(base, group.Games, table, order, group.Team_groups,
		stratum, true, validationSamples, deriveRarePositionSeed(masterSeed, "point-hybrid-validation"))
	diag.ValidationRankHits = make([]int, maxRank+1)
	for pos := 0; pos <= maxRank; pos++ {
		hits := validation[[2]int{targetTeam, pos}]
		diag.ValidationRankHits[pos] = hits
		diag.ValidationHits += hits
	}
	remaining := workLimit - diag.ScoutWork - diag.DiscoveryWork - diag.ValidationWork
	stratumSamples := 0
	if diag.ValidationHits >= 2 {
		qPercent := int64(2)
		if raw := os.Getenv("RARE_POSITION_POINT_HYBRID_Q_PERCENT"); raw != "" {
			if parsed, err := strconv.ParseInt(raw, 10, 64); err == nil && parsed >= 0 && parsed <= 50 {
				qPercent = parsed
			}
		}
		stratumSamples = int((remaining * qPercent / 100) / stratumCost)
	}
	stratumWork := int64(stratumSamples) * stratumCost
	plainSamples := int((remaining - stratumWork) / plainCost)
	if plainSamples <= 1 {
		stratumSamples = 0
		stratumWork = 0
		plainSamples = int(remaining / plainCost)
		if plainSamples <= 1 {
			panic("point hybrid preflight left insufficient production work")
		}
	}
	diag.PlainSamples = plainSamples
	diag.StratumSamples = stratumSamples
	diag.ProductionWork = int64(plainSamples)*plainCost + stratumWork
	if diag.ScoutWork+diag.DiscoveryWork+diag.ValidationWork+diag.ProductionWork > workLimit {
		panic("point hybrid work budget exceeded")
	}
	plainCounts, outsideCounts := simulatePointHybridSeasons(base, group.Games, table, order, group.Team_groups,
		stratum, false, plainSamples, deriveRarePositionSeed(masterSeed, "point-hybrid-production-P"))
	qCounts := map[[2]int]int{}
	if stratumSamples > 0 {
		qCounts, _ = simulatePointHybridSeasons(base, group.Games, table, order, group.Team_groups,
			stratum, true, stratumSamples, deriveRarePositionSeed(masterSeed, "point-hybrid-production-Q"))
	}
	estimates := make(map[int]map[int]ProductionEstimate, len(group.Team_groups))
	for _, team := range group.Team_groups {
		id := team.Team_id
		estimates[id] = make(map[int]ProductionEstimate, len(group.Team_groups))
		for pos := range group.Team_groups {
			cell := [2]int{id, pos}
			plainHits := plainCounts[cell]
			pPlain := float64(plainHits) / float64(plainSamples)
			p, variance, hits, samples := pPlain, pPlain*(1-pPlain)/float64(plainSamples), plainHits, plainSamples
			if id == targetTeam && pos <= maxRank && stratumSamples > 0 {
				outsideHits := outsideCounts[pos]
				pOutside := float64(outsideHits) / float64(plainSamples)
				insideHits := qCounts[cell]
				pInside := float64(insideHits) / float64(stratumSamples)
				p = pOutside + stratum.Mass*pInside
				variance = pOutside*(1-pOutside)/float64(plainSamples) +
					stratum.Mass*stratum.Mass*pInside*(1-pInside)/float64(stratumSamples)
				hits = outsideHits + insideHits
				samples = plainSamples + stratumSamples
			}
			e := pointHybridEstimate(p, variance, hits, samples, diag.ProductionWork, "point_stratum_hybrid")
			if id == targetTeam && pos <= maxRank && stratumSamples > 0 && hits == 0 {
				// Bonferroni combines two zero-count 97.5% upper limits.
				e.ZeroHitUpper95 = math.Min(1, -math.Log(.025)/float64(plainSamples)+stratum.Mass*(-math.Log(.025))/float64(stratumSamples))
			}
			estimates[id][pos] = e
		}
	}
	return estimates, diag, true
}
