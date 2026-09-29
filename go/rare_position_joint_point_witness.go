package main

import (
	"log"
	"math/bits"
	"os"
	"sort"
	"time"
)

const (
	jointPointWitnessNodesPerCell = 100
	jointPointWitnessTotalNodes   = 3200
)

func jointPointWitnessEnabled() bool {
	return os.Getenv("RARE_POSITION_JOINT_POINT_WITNESS") != "0"
}

type jointPointWitnessSearch struct {
	problem       *jointPointCapProblem
	group         *GroupType
	campaign      []*TeamCampaign
	table         *Table
	order         []SortType
	cell          conditionedZeroCell
	cap           int
	nodes         int
	maxNodes      int
	verified      int
	initial       []uint8
	proof         []uint8
	targetMinimum bool
}

func (search *jointPointWitnessSearch) complete(domains []uint8) bool {
	outcomes := make([]uint8, len(search.group.Games))
	for index, game := range search.problem.games {
		domain := domains[index]
		if bits.OnesCount8(domain) != 1 {
			return false
		}
		outcomes[game.index] = uint8(bits.TrailingZeros8(domain))
	}
	search.verified++
	if canonicalReachabilityRank(search.group, search.campaign, search.table,
		search.order, outcomes, search.cell.id) != search.cell.rank {
		return false
	}
	search.proof = outcomes
	return true
}

func (search *jointPointWitnessSearch) assign(domains []uint8, exempt uint64) bool {
	if search.nodes >= search.maxNodes {
		return false
	}
	search.nodes++
	domains, lower, ok := search.problem.jointPointCapPropagate(search.cap, exempt, domains)
	if !ok {
		return false
	}
	best, bestSize, bestSlack := -1, 4, int(^uint(0)>>1)
	for index, game := range search.problem.games {
		size := bits.OnesCount8(domains[index])
		if size <= 1 {
			continue
		}
		homeSlack, awaySlack := 1000000, 1000000
		if exempt&(uint64(1)<<game.home) == 0 {
			homeSlack = search.cap - lower[game.home]
		}
		if exempt&(uint64(1)<<game.away) == 0 {
			awaySlack = search.cap - lower[game.away]
		}
		slack := min(homeSlack, awaySlack)
		if size < bestSize || size == bestSize && slack < bestSlack {
			best, bestSize, bestSlack = index, size, slack
		}
	}
	if best < 0 {
		return search.complete(domains)
	}
	game := search.problem.games[best]
	options := make([]int, 0, 3)
	for outcome := 0; outcome < 3; outcome++ {
		if domains[best]&(1<<outcome) != 0 {
			options = append(options, outcome)
		}
	}
	penalty := func(outcome int) float64 {
		value := 0.0
		if exempt&(uint64(1)<<game.home) == 0 {
			value += float64(game.homeGain[outcome]) / float64(max(1, search.cap-lower[game.home]+1))
		}
		if exempt&(uint64(1)<<game.away) == 0 {
			value += float64(game.awayGain[outcome]) / float64(max(1, search.cap-lower[game.away]+1))
		}
		return value
	}
	sort.Slice(options, func(i, j int) bool { return penalty(options[i]) < penalty(options[j]) })
	for _, outcome := range options {
		if search.nodes >= search.maxNodes {
			break
		}
		next := append([]uint8(nil), domains...)
		next[best] = 1 << outcome
		if search.assign(next, exempt) {
			return true
		}
	}
	return false
}

func (search *jointPointWitnessSearch) find(target int32) bool {
	if search.problem == nil || search.maxNodes <= 0 || target < 0 || int(target) >= len(search.problem.base) {
		return false
	}
	mandatory := uint64(1) << target
	mandatoryCount := 0
	for index, points := range search.problem.base {
		if !search.problem.ranked[index] {
			mandatory |= uint64(1) << index
		} else if index != int(target) && points > search.cap {
			mandatory |= uint64(1) << index
			mandatoryCount++
		}
	}
	spare := search.cell.rank - mandatoryCount
	if spare < 0 {
		return false
	}
	search.initial = make([]uint8, len(search.problem.games))
	for index, game := range search.problem.games {
		domain := uint8(0)
		bestGain := -int(^uint(0)>>1) - 1
		if search.targetMinimum {
			bestGain = int(^uint(0) >> 1)
		}
		if game.home == target {
			for outcome := 0; outcome < 3; outcome++ {
				better := game.homeGain[outcome] > bestGain
				if search.targetMinimum {
					better = game.homeGain[outcome] < bestGain
				}
				if game.prob[outcome] > 0 && better {
					bestGain = game.homeGain[outcome]
				}
			}
		} else if game.away == target {
			for outcome := 0; outcome < 3; outcome++ {
				better := game.awayGain[outcome] > bestGain
				if search.targetMinimum {
					better = game.awayGain[outcome] < bestGain
				}
				if game.prob[outcome] > 0 && better {
					bestGain = game.awayGain[outcome]
				}
			}
		}
		for outcome := 0; outcome < 3; outcome++ {
			if game.prob[outcome] <= 0 {
				continue
			}
			if game.home == target && game.homeGain[outcome] != bestGain ||
				game.away == target && game.awayGain[outcome] != bestGain {
				continue
			}
			domain |= 1 << outcome
		}
		if domain == 0 {
			return false
		}
		search.initial[index] = domain
	}
	candidates := make([]int, 0, len(search.problem.base))
	for index := range search.problem.base {
		if mandatory&(uint64(1)<<index) == 0 && search.problem.ranked[index] {
			candidates = append(candidates, index)
		}
	}
	sort.Slice(candidates, func(i, j int) bool {
		a, b := candidates[i], candidates[j]
		if search.problem.base[a] != search.problem.base[b] {
			return search.problem.base[a] > search.problem.base[b]
		}
		return a < b
	})
	var choose func(uint64, int, int) bool
	choose = func(mask uint64, start, left int) bool {
		if search.nodes >= search.maxNodes {
			return false
		}
		if left == 0 {
			return search.assign(append([]uint8(nil), search.initial...), mask)
		}
		for index := start; index <= len(candidates)-left; index++ {
			if choose(mask|(uint64(1)<<candidates[index]), index+1, left-1) {
				return true
			}
			if search.nodes >= search.maxNodes {
				break
			}
		}
		return false
	}
	for extra := 0; extra <= spare && extra <= len(candidates); extra++ {
		if choose(mandatory, 0, extra) {
			return true
		}
		if search.nodes >= search.maxNodes {
			break
		}
	}
	return false
}

// Return complete verified assignments so later reachability searches can
// explore nearby schedules without repeating the fixture-domain search.
func runJointPointWitnessSearch(group *GroupType, campaign []*TeamCampaign, table *Table,
	order []SortType, cells []conditionedZeroCell, estimates map[int]map[int]ProductionEstimate) (int, int, [][]uint8) {
	if len(order) == 0 || order[0] != PT {
		return 0, 0, nil
	}
	problem := newJointPointCapProblem(group, campaign)
	if problem == nil {
		return 0, 0, nil
	}
	start := time.Now()
	proofs, nodes, considered := 0, 0, 0
	var assignments [][]uint8
	for _, cell := range cells {
		est := estimates[cell.id][cell.rank]
		if est.Probability != 0 || est.Reachability != "undecided" || nodes >= jointPointWitnessTotalNodes {
			continue
		}
		considered++
		target := table.Query(uint32(cell.id))
		cap := problem.base[target]
		for _, game := range problem.games {
			if game.home == target {
				cap += max(game.homeGain[0], max(game.homeGain[1], game.homeGain[2]))
			} else if game.away == target {
				cap += max(game.awayGain[0], max(game.awayGain[1], game.awayGain[2]))
			}
		}
		search := jointPointWitnessSearch{
			problem: problem, group: group, campaign: campaign, table: table, order: order,
			cell: cell, cap: cap, maxNodes: min(jointPointWitnessNodesPerCell, jointPointWitnessTotalNodes-nodes),
		}
		found := search.find(target)
		nodes += search.nodes
		if found {
			assignments = append(assignments, search.proof)
			est.Reachability = "reachable_by_construction"
			estimates[cell.id][cell.rank] = est
			proofs++
			log.Printf("rare-position-joint-point-witness: group=%d team=%d rank=%d nodes=%d verified=%d",
				group.Id, cell.id, cell.rank+1, search.nodes, search.verified)
		}
	}
	log.Printf("rare-position-joint-point-witness: group=%d cells=%d proofs=%d nodes=%d elapsed=%s",
		group.Id, considered, proofs, nodes, time.Since(start))
	return proofs, nodes, assignments
}
