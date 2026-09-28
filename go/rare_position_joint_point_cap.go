package main

import (
	"log"
	"os"
	"time"
)

const (
	jointPointCapNodesPerCell = 500
	jointPointCapTotalNodes   = 10000
)

type jointPointCapGame struct {
	home, away int32
	homeGain   [3]int
	awayGain   [3]int
}

type jointPointCapProblem struct {
	base   []int
	ranked []bool
	games  []jointPointCapGame
}

func jointPointCapEnabled() bool {
	return os.Getenv("RARE_POSITION_JOINT_POINT_CAP") != "0"
}

func newJointPointCapProblem(group *GroupType, campaign []*TeamCampaign) *jointPointCapProblem {
	if len(campaign) == 0 || len(campaign) > 64 {
		return nil
	}
	problem := &jointPointCapProblem{base: make([]int, len(campaign)), ranked: make([]bool, len(campaign))}
	for index, team := range campaign {
		if team != nil {
			problem.base[index] = team.points
			problem.ranked[index] = true
		}
	}
	for _, game := range group.Games {
		if game.Played {
			continue
		}
		home, away := game.home_table_index, game.away_table_index
		if home < 0 || away < 0 || int(home) >= len(campaign) || int(away) >= len(campaign) || home == away {
			return nil
		}
		gain := func(team *TeamCampaign) (int, int, int) {
			if team != nil {
				return team.points_loss, team.points_draw, team.points_win
			}
			points := group.Phase.Championship
			return points.Point_loss, points.Point_draw, points.Point_win
		}
		hLoss, hDraw, hWin := gain(campaign[home])
		aLoss, aDraw, aWin := gain(campaign[away])
		problem.games = append(problem.games, jointPointCapGame{
			home: home, away: away,
			homeGain: [3]int{hLoss, hDraw, hWin},
			awayGain: [3]int{aWin, aDraw, aLoss},
		})
	}
	return problem
}

func minJointPointGain(domain uint8, gain [3]int) int {
	minimum := int(^uint(0) >> 1)
	for outcome := 0; outcome < 3; outcome++ {
		if domain&(1<<outcome) != 0 && gain[outcome] < minimum {
			minimum = gain[outcome]
		}
	}
	return minimum
}

// jointPointCapConsistent is a relaxation: every team outside exempt must
// finish on at most cap points. It removes an outcome only if the team's
// minimum possible final points would already exceed that cap.
func (problem *jointPointCapProblem) jointPointCapConsistent(cap int, exempt uint64) bool {
	domains := make([]uint8, len(problem.games))
	for index := range domains {
		domains[index] = 0b111
	}
	lower := make([]int, len(problem.base))
	homeMin := make([]int, len(problem.games))
	awayMin := make([]int, len(problem.games))
	for {
		copy(lower, problem.base)
		for index, game := range problem.games {
			if domains[index] == 0 {
				return false
			}
			homeMin[index] = minJointPointGain(domains[index], game.homeGain)
			awayMin[index] = minJointPointGain(domains[index], game.awayGain)
			lower[game.home] += homeMin[index]
			lower[game.away] += awayMin[index]
		}
		for index, points := range lower {
			if exempt&(uint64(1)<<index) == 0 && points > cap {
				return false
			}
		}
		changed := false
		for index, game := range problem.games {
			domain := domains[index]
			for outcome := 0; outcome < 3; outcome++ {
				bit := uint8(1 << outcome)
				if domain&bit == 0 {
					continue
				}
				if exempt&(uint64(1)<<game.home) == 0 &&
					lower[game.home]-homeMin[index]+game.homeGain[outcome] > cap ||
					exempt&(uint64(1)<<game.away) == 0 &&
						lower[game.away]-awayMin[index]+game.awayGain[outcome] > cap {
					domain &^= bit
					changed = true
				}
			}
			if domain == 0 {
				return false
			}
			domains[index] = domain
		}
		if !changed {
			return true
		}
	}
}

// proveJointPointCapImpossible certifies that every way to spend the
// above-cap rank slots conflicts with the remaining fixtures. A false result
// can mean either feasible relaxation or an exhausted node budget.
func (problem *jointPointCapProblem) proveJointPointCapImpossible(target int32, rank, cap, budget int) (bool, int) {
	if problem == nil || target < 0 || int(target) >= len(problem.base) || budget <= 0 {
		return false, 0
	}
	exempt := uint64(1) << target
	mandatory := 0
	for index, points := range problem.base {
		if len(problem.ranked) > 0 && !problem.ranked[index] {
			exempt |= uint64(1) << index
			continue
		}
		if index != int(target) && points > cap {
			exempt |= uint64(1) << index
			mandatory++
		}
	}
	if mandatory > rank {
		return true, 0
	}
	spare := rank - mandatory
	visited := make(map[uint64]bool)
	nodes := 0
	var search func(uint64, int) int
	search = func(mask uint64, slots int) int {
		if visited[mask] {
			return 2
		}
		visited[mask] = true
		if nodes >= budget {
			return 0
		}
		nodes++
		if problem.jointPointCapConsistent(cap, mask) {
			return 1
		}
		if slots == 0 {
			return 2
		}
		for index := range problem.base {
			bit := uint64(1) << index
			if mask&bit != 0 || len(problem.ranked) > 0 && !problem.ranked[index] {
				continue
			}
			result := search(mask|bit, slots-1)
			if result != 2 {
				return result
			}
		}
		return 2
	}
	return search(exempt, spare) == 2, nodes
}

func runJointPointCapProofs(group *GroupType, campaign []*TeamCampaign, table *Table, sortOrder []SortType,
	cells []conditionedZeroCell, estimates map[int]map[int]ProductionEstimate) (int, int) {
	if len(sortOrder) == 0 || sortOrder[0] != PT {
		return 0, 0
	}
	start := time.Now()
	problem := newJointPointCapProblem(group, campaign)
	if problem == nil {
		return 0, 0
	}
	proofs, nodes, considered := 0, 0, 0
	for _, cell := range cells {
		est := estimates[cell.id][cell.rank]
		if est.Probability != 0 || est.Reachability != "undecided" {
			continue
		}
		if nodes >= jointPointCapTotalNodes {
			break
		}
		considered++
		target := table.Query(uint32(cell.id))
		maxPoints := problem.base[target]
		for _, game := range problem.games {
			if game.home == target {
				maxPoints += max(game.homeGain[0], max(game.homeGain[1], game.homeGain[2]))
			} else if game.away == target {
				maxPoints += max(game.awayGain[0], max(game.awayGain[1], game.awayGain[2]))
			}
		}
		budget := min(jointPointCapNodesPerCell, jointPointCapTotalNodes-nodes)
		impossible, spent := problem.proveJointPointCapImpossible(target, cell.rank, maxPoints, budget)
		nodes += spent
		if impossible {
			est.Reachability = "impossible_by_joint_points"
			est.ZeroHitUpper95 = 0
			estimates[cell.id][cell.rank] = est
			proofs++
			log.Printf("rare-position-joint-point-cap-proof: group=%d team=%d rank=%d cap=%d nodes=%d",
				group.Id, cell.id, cell.rank+1, maxPoints, spent)
		}
	}
	log.Printf("rare-position-joint-point-cap: group=%d cells=%d proofs=%d nodes=%d elapsed=%s",
		group.Id, considered, proofs, nodes, time.Since(start))
	return proofs, nodes
}
