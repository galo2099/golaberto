package main

import (
	"fmt"
	"log"
	"math/bits"
	"math/rand"
	"sort"
)

// This is a scheduling score, not a probability or a reachability proof.
// Probe target-only assignments and count the rival outcomes removed by
// necessary point/win constraints. No full seasons or goal scores are drawn.
func constraintPeerScore(candidate directionalPeerTarget, group *GroupType,
	campaign []*TeamCampaign, table *Table, order []SortType, seed int64) (float64, int64) {
	problem := newJointPointCapProblem(group, campaign)
	if problem == nil || len(order) == 0 || order[0] != PT {
		return 0, 0
	}
	stride := conditionedRankWinStride(order, group, campaign, table, "lookahead")
	for i, team := range campaign {
		if team != nil {
			problem.base[i] *= stride
			if stride > 1 {
				problem.base[i] += team.wins
			}
		}
	}
	target := table.Query(uint32(candidate.cell.id))
	var rivals []int32
	for _, team := range group.Team_groups {
		index := table.Query(uint32(team.Team_id))
		if index != target {
			rivals = append(rivals, index)
		}
	}
	var selected, remaining []conditionedPointOutcomeGame
	options := 0
	for _, game := range problem.games {
		g := conditionedPointOutcomeGame{index: game.index, home: game.home, away: game.away,
			prob: game.prob, homeGain: game.homeGain, awayGain: game.awayGain}
		for o := range g.homeGain {
			g.homeGain[o] *= stride
			g.awayGain[o] *= stride
		}
		if stride > 1 {
			g.homeGain[2]++
			g.awayGain[0]++
		}
		if g.home == target || g.away == target {
			selected = append(selected, g)
		} else {
			remaining = append(remaining, g)
			for _, p := range g.prob {
				if p > 0 {
					options++
				}
			}
		}
	}
	if options == 0 {
		return 0, 0
	}
	backward := newConditionedPointBackwardSampler(candidate.event)
	rng := rand.New(rand.NewSource(seed))
	outcomes := make([]uint8, len(group.Games))
	points := make([]int, len(campaign))
	removed := 0
	for draw := 0; draw < constraintPeerProbeAssignments; draw++ {
		backward.sampleOutcomes(rng, outcomes)
		copy(points, problem.base)
		for _, g := range selected {
			o := outcomes[g.index]
			points[g.home] += g.homeGain[o]
			points[g.away] += g.awayGain[o]
		}
		domains := propagateConditionedRankDomains(remaining, points, rivals, candidate.cell.rank, points[target])
		if !domains.feasible {
			continue
		}
		retained := 0
		for _, domain := range domains.domains {
			retained += bits.OnesCount8(domain)
		}
		removed += options - retained
	}
	return float64(removed) / float64(options*constraintPeerProbeAssignments),
		int64(len(problem.games) * constraintPeerProbeAssignments)
}

func runConstraintPeerRescue(targets []directionalPeerTarget, current map[int]int,
	group *GroupType, campaign []*TeamCampaign, table *Table, order []SortType,
	seed int64, bounds pointRankBounds, samplers []conditionedScoreSampler,
	estimates map[int]map[int]ProductionEstimate) (int, int64) {
	// Keep the primary search intact. Limit preprocessing even in large tables.
	sort.Slice(targets, func(i, j int) bool {
		a, b := targets[i], targets[j]
		if a.upper != b.upper {
			return a.upper > b.upper
		}
		if a.event.mass != b.event.mass {
			return a.event.mass > b.event.mass
		}
		if a.cell.id != b.cell.id {
			return a.cell.id < b.cell.id
		}
		return a.cell.rank < b.cell.rank
	})
	var best directionalPeerTarget
	bestScore, work, screened := 0.0, int64(0), 0
	for _, c := range targets {
		if estimates[c.cell.id][c.cell.rank].Probability > 0 ||
			!conditionedDomainUniformSide(c.event, c.cell, group, campaign, table, order, bounds) {
			continue
		}
		if screened == constraintPeerProbeCells {
			break
		}
		screened++
		stream := deriveRarePositionSeed(seed, fmt.Sprintf("constraint-peer-probe-%d-%d", c.cell.id, c.cell.rank))
		score, probeWork := constraintPeerScore(c, group, campaign, table, order, stream)
		work += probeWork
		if score > bestScore {
			best, bestScore = c, score
		}
	}
	if bestScore == 0 {
		return 0, work
	}
	pointTilt := .5
	if best.cell.rank > current[best.cell.id] {
		pointTilt = -.5
	}
	pilotSeed := deriveRarePositionSeed(seed, fmt.Sprintf("directional-peer-pilot-%d-%d", best.cell.id, best.cell.rank))
	pilot, _ := sampleConditionedZeroRankWithDomains(best.event, best.cell.id, best.cell.rank,
		group, campaign, table, order, bounds, samplers, directionalPeerPilotSamples, pilotSeed, 3, pointTilt)
	work += pilot.work
	if pilot.hits == 0 {
		log.Printf("rare-position-constraint-peer: group=%d screened=%d team=%d rank=%d score=%g pilot_hits=0 accepted=false work=%d",
			group.Id, screened, best.cell.id, best.cell.rank+1, bestScore, work)
		return 0, work
	}
	est := estimates[best.cell.id][best.cell.rank]
	if est.Reachability == "undecided" || est.Reachability == "" {
		est.Reachability = "reachable_by_construction"
		estimates[best.cell.id][best.cell.rank] = est
	}
	stream := deriveRarePositionSeed(seed, fmt.Sprintf("directional-peer-rescue-%d-%d", best.cell.id, best.cell.rank))
	result, _ := sampleConditionedZeroRankWithDomains(best.event, best.cell.id, best.cell.rank,
		group, campaign, table, order, bounds, samplers, constraintPeerSamples, stream, 3, pointTilt)
	work += result.work
	accepted := conditionedPointTiltResultValid(result)
	log.Printf("rare-position-constraint-peer: group=%d screened=%d team=%d rank=%d score=%g pilot_hits=%d hits=%d ess=%g p=%g relative_se=%g batch_gap=%g accepted=%t work=%d",
		group.Id, screened, best.cell.id, best.cell.rank+1, bestScore, pilot.hits, result.hits,
		result.ess, result.probability, result.stdErr/result.probability, result.batchGap, accepted, work)
	if !accepted {
		return 0, work
	}
	estimates[best.cell.id][best.cell.rank] = applyConditionedPointTiltEstimate(
		estimates[best.cell.id][best.cell.rank], result, "matched_point_pool_conditioned_constraint_peer")
	return 1, work
}
