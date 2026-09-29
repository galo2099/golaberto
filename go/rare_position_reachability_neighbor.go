package main

import (
	"log"
	"math/rand"
	"os"
	"sort"
	"strconv"
)

const reachabilityNeighborCandidateBudget = 10000
const reachabilityNeighborTieChecks = 500
const reachabilityNeighborWalkRounds = 2
const reachabilityNeighborWalkBudget = 4000

// The first neighborhood search stops after two fixture changes. Feeding its
// verified assignments back into the same bounded search reaches schedules
// that need several changes, without trusting an unverified intermediate rank.
func extendReachabilityNeighbors(group *GroupType, campaign []*TeamCampaign,
	table *Table, sortOrder []SortType, cells []conditionedZeroCell,
	assignments [][]uint8, estimates map[int]map[int]ProductionEstimate,
	rounds, budget int) (int, int) {
	seeds := make([]conditionedZeroSearchResult, 0, len(assignments))
	for _, outcomes := range assignments {
		seeds = append(seeds, conditionedZeroSearchResult{witnessOutcomes: outcomes})
	}
	totalProofs, totalAttempts := 0, 0
	for round := 0; round < rounds && len(seeds) > 0; round++ {
		proofs, attempts := searchReachabilityNeighbors(group, campaign, table,
			sortOrder, cells, seeds, estimates, budget)
		totalAttempts += attempts
		totalProofs += len(proofs)
		seeds = seeds[:0]
		for _, proof := range proofs {
			est := estimates[proof.cell.id][proof.cell.rank]
			est.Reachability = "reachable_by_construction"
			estimates[proof.cell.id][proof.cell.rank] = est
			seeds = append(seeds, conditionedZeroSearchResult{witnessOutcomes: proof.outcomes})
		}
	}
	return totalProofs, totalAttempts
}

type reachabilityNeighborProof struct {
	cell     conditionedZeroCell
	outcomes []uint8
}

type reachabilityNeighborGame struct {
	index                  int
	home, away             int32
	prob                   [3]float64
	homePoints, awayPoints [3]int
}

func reachabilityNeighborEnabled() bool {
	return os.Getenv("RARE_POSITION_NEIGHBORHOOD_SEARCH") != "0"
}

func reachabilityNeighborBudget() int {
	if value, err := strconv.Atoi(os.Getenv("RARE_POSITION_NEIGHBORHOOD_BUDGET")); err == nil && value > 0 && value <= 250000 {
		return value
	}
	return reachabilityNeighborCandidateBudget
}

// searchReachabilityNeighbors reuses outcome vectors observed by conditional
// samplers. It changes at most two remaining fixtures. A points/wins prefix
// establishes ranks without a tie; otherwise the full sorter verifies them.
func searchReachabilityNeighbors(group *GroupType, campaign []*TeamCampaign,
	table *Table, sortOrder []SortType, cells []conditionedZeroCell,
	results []conditionedZeroSearchResult, estimates map[int]map[int]ProductionEstimate,
	budget int) ([]reachabilityNeighborProof, int) {
	if len(sortOrder) == 0 || sortOrder[0] != PT || budget <= 0 {
		return nil, 0
	}
	games := make([]reachabilityNeighborGame, 0, len(group.Games))
	for index, game := range group.Games {
		if game.Played {
			continue
		}
		home, away := game.home_table_index, game.away_table_index
		if home == away || home < 0 || away < 0 ||
			int(home) >= len(campaign) || int(away) >= len(campaign) ||
			campaign[home] == nil || campaign[away] == nil {
			return nil, 0
		}
		h, a := campaign[home], campaign[away]
		games = append(games, reachabilityNeighborGame{
			index: index, home: home, away: away,
			prob:       targetOutcomeProbabilities(game),
			homePoints: [3]int{h.points_loss, h.points_draw, h.points_win},
			awayPoints: [3]int{a.points_win, a.points_draw, a.points_loss},
		})
	}
	if len(games) == 0 {
		return nil, 0
	}
	type targetCell struct {
		cell  conditionedZeroCell
		index int32
	}
	unresolved := make([]targetCell, 0, len(cells))
	for _, cell := range cells {
		if est := estimates[cell.id][cell.rank]; est.Probability == 0 && est.Reachability == "undecided" {
			unresolved = append(unresolved, targetCell{cell, table.Query(uint32(cell.id))})
		}
	}
	if len(unresolved) == 0 {
		return nil, 0
	}
	seeds := make([][]uint8, 0, len(results))
	seen := make(map[string]bool, len(results))
	for _, result := range results {
		outcomes := result.witnessOutcomes
		if len(outcomes) != len(group.Games) || seen[string(outcomes)] {
			continue
		}
		seen[string(outcomes)] = true
		seeds = append(seeds, outcomes)
	}
	if len(seeds) == 0 {
		return nil, 0
	}
	log.Printf("rare-position-neighborhood: group=%d distinct_seeds=%d unresolved=%d",
		group.Id, len(seeds), len(unresolved))
	perSeedBudget := budget / len(seeds)
	if perSeedBudget < 1 {
		perSeedBudget = 1
	}
	perSeedTieChecks := reachabilityNeighborTieChecks / len(seeds)
	if perSeedTieChecks < 1 {
		perSeedTieChecks = 1
	}
	teamIndices := make([]int32, len(group.Team_groups))
	for index, team := range group.Team_groups {
		teamIndices[index] = table.Query(uint32(team.Team_id))
	}
	useWins := len(sortOrder) > 1 && sortOrder[1] == W
	proofs := make([]reachabilityNeighborProof, 0, len(unresolved))
	points := make([]int, len(campaign))
	wins := make([]int, len(campaign))
	attempted := 0
	totalTieChecks := 0
	for _, seed := range seeds {
		if attempted >= budget || len(unresolved) == 0 {
			break
		}
		seedLimit := attempted + perSeedBudget
		if seedLimit > budget {
			seedLimit = budget
		}
		copyBase := func() bool {
			for i, team := range campaign {
				if team != nil {
					points[i], wins[i] = team.points, team.wins
				}
			}
			for _, game := range games {
				outcome := seed[game.index]
				if outcome > 2 || game.prob[outcome] <= 0 {
					return false
				}
				points[game.home] += game.homePoints[outcome]
				points[game.away] += game.awayPoints[outcome]
				if outcome == 0 {
					wins[game.away]++
				} else if outcome == 2 {
					wins[game.home]++
				}
			}
			return true
		}
		if !copyBase() {
			continue
		}
		candidate := append([]uint8(nil), seed...)
		tieChecks := 0
		check := func() {
			attempted++
			remaining := unresolved[:0]
			for _, target := range unresolved {
				above, tied := 0, 0
				p, w := points[target.index], wins[target.index]
				for _, index := range teamIndices {
					if index == target.index {
						continue
					}
					if points[index] > p || useWins && points[index] == p && wins[index] > w {
						above++
					} else if points[index] == p && (!useWins || wins[index] == w) {
						tied++
					}
				}
				if target.cell.rank < above || target.cell.rank > above+tied {
					remaining = append(remaining, target)
					continue
				}
				verified := tied == 0 && above == target.cell.rank
				if !verified && tieChecks < perSeedTieChecks && totalTieChecks < reachabilityNeighborTieChecks {
					tieChecks++
					totalTieChecks++
					verified = canonicalReachabilityRank(group, campaign, table, sortOrder,
						candidate, target.cell.id) == target.cell.rank
				}
				if verified {
					proofs = append(proofs, reachabilityNeighborProof{
						cell: target.cell, outcomes: append([]uint8(nil), candidate...),
					})
				} else {
					remaining = append(remaining, target)
				}
			}
			unresolved = remaining
		}
		apply := func(game reachabilityNeighborGame, next uint8) {
			old := candidate[game.index]
			points[game.home] += game.homePoints[next] - game.homePoints[old]
			points[game.away] += game.awayPoints[next] - game.awayPoints[old]
			if old == 2 {
				wins[game.home]--
			}
			if old == 0 {
				wins[game.away]--
			}
			if next == 2 {
				wins[game.home]++
			}
			if next == 0 {
				wins[game.away]++
			}
			candidate[game.index] = next
		}
		check()
		for i, game := range games {
			if attempted >= seedLimit || len(unresolved) == 0 {
				break
			}
			old := candidate[game.index]
			for next := uint8(0); next < 3 && attempted < seedLimit; next++ {
				if next == old || game.prob[next] <= 0 {
					continue
				}
				apply(game, next)
				check()
				if len(unresolved) == 0 {
					return proofs, attempted
				}
				apply(game, old)
			}
			for j := i + 1; j < len(games) && attempted < seedLimit && len(unresolved) > 0; j++ {
				other := games[j]
				otherOld := candidate[other.index]
				for next := uint8(0); next < 3; next++ {
					if next == old || game.prob[next] <= 0 {
						continue
					}
					apply(game, next)
					for otherNext := uint8(0); otherNext < 3; otherNext++ {
						if otherNext == otherOld || other.prob[otherNext] <= 0 {
							continue
						}
						apply(other, otherNext)
						check()
						apply(other, otherOld)
						if attempted >= seedLimit || len(unresolved) == 0 {
							break
						}
					}
					apply(game, old)
					if attempted >= seedLimit || len(unresolved) == 0 {
						break
					}
				}
			}
		}
	}
	return proofs, attempted
}

func canonicalReachabilityRank(group *GroupType, campaign []*TeamCampaign,
	table *Table, sortOrder []SortType, outcomes []uint8, target int) int {
	sim := make([]*TeamCampaign, len(campaign))
	for index, team := range campaign {
		if team != nil {
			sim[index] = team.clone()
		}
	}
	simGames := make([]GameType, len(group.Games))
	for index, game := range group.Games {
		if game.Played {
			continue
		}
		score := &simGames[index]
		*score = GameType{Id: game.Id, HomeId: game.HomeId, AwayId: game.AwayId,
			Played: true, home_table_index: game.home_table_index,
			away_table_index: game.away_table_index}
		switch outcomes[index] {
		case 0:
			score.AwayScore = 1
		case 1:
		case 2:
			score.HomeScore = 1
		default:
			return -1
		}
		addSimulatedGame(sim[game.home_table_index], sim[game.away_table_index], score)
	}
	teams := make([]*TeamCampaign, len(group.Team_groups))
	for index, team := range group.Team_groups {
		teams[index] = sim[table.Query(uint32(team.Team_id))]
	}
	sort.Sort(TeamCampaignSorted{t: teams, sort: sortOrder,
		rng: rand.New(rand.NewSource(1))})
	for rank, team := range teams {
		if team.id == target {
			return rank
		}
	}
	return -1
}
