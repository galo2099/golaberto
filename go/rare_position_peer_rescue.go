package main

import (
	"fmt"
	"log"
	"math/rand"
	"os"
	"sort"
	"strings"
)

const (
	directionalPeerMaximumProbability = 1e-6
	directionalPeerMinimumPointMass   = 1e-4
	directionalPeerPilotSamples       = 5000
	directionalPeerSamples            = 50000
	constraintPeerSamples             = directionalPeerSamples - directionalPeerPilotSamples
	constraintPeerProbeAssignments    = 32
	constraintPeerProbeCells          = 8
)

func directionalPeerSupports(targetStart, peerStart, targetRank int) bool {
	return targetRank > targetStart && peerStart < targetStart ||
		targetRank < targetStart && peerStart > targetStart
}

// Another fresh confirmation is useful when the existing sample has enough
// effective evidence but its two halves disagree. Other failures retain the
// current budget. The retry must pass the unchanged full validity gate.
func directionalPeerRetryEligible(result conditionedZeroResult) bool {
	if !(result.batchGap > 1) {
		return false
	}
	result.batchGap = 0
	return conditionedPointTiltResultValid(result)
}

type directionalPeerTarget struct {
	cell  conditionedZeroCell
	event *conditionedPointEvent
	upper float64
}

// A rarer-ranked peer can identify a zero worth another independent search.
// Current standing is used only to prioritize a target; it never transfers
// probability from one team's fixtures to another's.
func runDirectionalPeerRescue(group *GroupType, campaign []*TeamCampaign,
	table *Table, sortOrder []SortType, seed int64, bounds pointRankBounds,
	samplers []conditionedScoreSampler, estimates map[int]map[int]ProductionEstimate,
) (int, int64) {
	if os.Getenv("RARE_POSITION_DIRECTIONAL_PEER_RESCUE") == "0" {
		return 0, 0
	}
	standing := make([]*TeamCampaign, 0, len(group.Team_groups))
	for _, tg := range group.Team_groups {
		standing = append(standing, campaign[table.Query(uint32(tg.Team_id))])
	}
	sort.Sort(TeamCampaignSorted{t: standing, sort: sortOrder, rng: rand.New(rand.NewSource(1))})
	currentRank := make(map[int]int, len(standing))
	for rank, team := range standing {
		currentRank[team.id] = rank
	}
	var best directionalPeerTarget
	var targets []directionalPeerTarget
	found := false
	eligible := 0
	for _, team := range standing {
		id := team.id
		from := currentRank[id]
		for rank := range standing {
			est := estimates[id][rank]
			if rank == from || est.Probability > 0 || strings.HasPrefix(est.Reachability, "impossible") {
				continue
			}
			peer := false
			for _, other := range standing {
				p := estimates[other.id][rank].Probability
				if p <= 0 || p >= directionalPeerMaximumProbability {
					continue
				}
				if directionalPeerSupports(from, currentRank[other.id], rank) {
					peer = true
					break
				}
			}
			if !peer {
				continue
			}
			event, ok := buildConditionedPointEvent(id, rank, nil, group, campaign, table, bounds)
			if !ok || event.mass < directionalPeerMinimumPointMass {
				continue
			}
			eligible++
			targets = append(targets, directionalPeerTarget{conditionedZeroCell{id, rank}, event, est.ZeroHitUpper95})
			if !found || est.ZeroHitUpper95 > best.upper ||
				est.ZeroHitUpper95 == best.upper && event.mass > best.event.mass ||
				est.ZeroHitUpper95 == best.upper && event.mass == best.event.mass &&
					(id < best.cell.id || id == best.cell.id && rank < best.cell.rank) {
				best = directionalPeerTarget{conditionedZeroCell{id, rank}, event, est.ZeroHitUpper95}
				found = true
			}
		}
	}
	if !found {
		return 0, 0
	}
	pointTilt := 0.5
	if best.cell.rank > currentRank[best.cell.id] {
		pointTilt = -0.5
	}
	pilotSeed := deriveRarePositionSeed(seed, fmt.Sprintf(
		"directional-peer-pilot-%d-%d", best.cell.id, best.cell.rank))
	pilot, _ := sampleConditionedZeroRankLookaheadWithPointTilt(
		best.event, best.cell.id, best.cell.rank, group, campaign, table,
		sortOrder, bounds, samplers, directionalPeerPilotSamples, pilotSeed, 3, pointTilt)
	if pilot.hits == 0 {
		log.Printf("rare-position-directional-peer: group=%d eligible=%d team=%d rank=%d mass=%g pilot_hits=0 accepted=false work=%d",
			group.Id, eligible, best.cell.id, best.cell.rank+1, best.event.mass, pilot.work)
		return 0, pilot.work
	}
	if est := estimates[best.cell.id][best.cell.rank]; est.Reachability == "undecided" || est.Reachability == "" {
		est.Reachability = "reachable_by_construction"
		estimates[best.cell.id][best.cell.rank] = est
	}
	freshSeed := deriveRarePositionSeed(seed, fmt.Sprintf(
		"directional-peer-rescue-%d-%d", best.cell.id, best.cell.rank))
	result, _ := sampleConditionedZeroRankLookaheadWithPointTilt(
		best.event, best.cell.id, best.cell.rank, group, campaign, table,
		sortOrder, bounds, samplers, directionalPeerSamples, freshSeed, 3, pointTilt)
	accepted := conditionedPointTiltResultValid(result)
	work := pilot.work + result.work
	firstBatchGap, retryDraws := result.batchGap, 0
	if !accepted && directionalPeerRetryEligible(result) && os.Getenv("RARE_POSITION_DIRECTIONAL_PEER_RETRY") != "0" {
		retrySeed := deriveRarePositionSeed(seed, fmt.Sprintf(
			"directional-peer-fallback-%d-%d", best.cell.id, best.cell.rank))
		retry, _ := sampleConditionedZeroRankLookaheadWithPointTilt(
			best.event, best.cell.id, best.cell.rank, group, campaign, table,
			sortOrder, bounds, samplers, directionalPeerSamples, retrySeed, 3, pointTilt)
		work += retry.work
		retryDraws = retry.samples
		if conditionedPointTiltResultValid(retry) {
			// A fresh result must also agree in order of magnitude with the
			// first, sufficiently informative confirmation.
			result, accepted, _ = crossCheckExtremePointTilt(retry, result)
		}
	}
	if accepted {
		estimates[best.cell.id][best.cell.rank] = applyConditionedPointTiltEstimate(
			estimates[best.cell.id][best.cell.rank], result,
			"matched_point_pool_conditioned_point_tilt_peer")
	}
	log.Printf("rare-position-directional-peer: group=%d eligible=%d team=%d rank=%d mass=%g pilot_hits=%d hits=%d ess=%.1f probability=%g relative_se=%g max_weight_share=%g batch_gap=%g first_batch_gap=%g retry_draws=%d accepted=%t work=%d",
		group.Id, eligible, best.cell.id, best.cell.rank+1, best.event.mass,
		pilot.hits, result.hits, result.ess, result.probability, result.stdErr/result.probability,
		result.maxWeightShare, result.batchGap, firstBatchGap, retryDraws, accepted, work)
	if accepted {
		// A successful first confirmation leaves its retry allowance unused.
		// Spend that allowance on a second, demonstrably constrained cell.
		if retryDraws == 0 && os.Getenv("RARE_POSITION_CONSTRAINT_PEER_RESCUE") != "0" {
			added, extraWork := runConstraintPeerRescue(targets, currentRank, group, campaign,
				table, sortOrder, seed, bounds, samplers, estimates)
			return 1 + added, work + extraWork
		}
		return 1, work
	}
	return 0, work
}
