package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"math/bits"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"
	"time"
)

// Offline necessary capacity cut: shared fixtures must fit the SUM of a
// subset's caps as well as each individual team's cap. Negative gains encode
// minimum-points constraints. No probability or tiebreak assumptions are made.
func referenceCapCapacityConsistent(problem *jointPointCapProblem, cap int, exempt uint64) bool {
	domains := make([]uint8, len(problem.games))
	for i, game := range problem.games {
		for o, p := range game.prob {
			if p > 0 {
				domains[i] |= 1 << o
			}
		}
	}
	domains, lower, ok := problem.jointPointCapPropagate(cap, exempt, domains)
	if !ok {
		return false
	}
	var constrained []int
	for i := range problem.base {
		if exempt&(uint64(1)<<i) == 0 {
			constrained = append(constrained, i)
		}
	}
	for _, useBase := range []bool{false, true} {
		sort.Slice(constrained, func(i, j int) bool {
			a, b := constrained[i], constrained[j]
			if useBase {
				return problem.base[a] > problem.base[b]
			}
			return lower[a] > lower[b]
		})
		var mask uint64
		base := 0
		for _, team := range constrained {
			mask |= uint64(1) << team
			base += problem.base[team]
			minimum := base
			for i, game := range problem.games {
				gain := int(^uint(0) >> 1)
				for o := 0; o < 3; o++ {
					if domains[i]&(1<<o) == 0 {
						continue
					}
					v := 0
					if mask&(uint64(1)<<game.home) != 0 {
						v += game.homeGain[o]
					}
					if mask&(uint64(1)<<game.away) != 0 {
						v += game.awayGain[o]
					}
					gain = min(gain, v)
				}
				minimum += gain
			}
			if minimum > bits.OnesCount64(mask)*cap {
				return false
			}
		}
	}
	return true
}

// Enumerate the possible above-cap exemptions. A consistent relaxation or a
// budget exhaustion is undecided. Only exhaustive rejection proves impossible.
func referenceCapImpossible(problem *jointPointCapProblem, target int32, rank, cap, budget int) (bool, int) {
	exempt := uint64(1) << target
	mandatory := 0
	minimum := append([]int(nil), problem.base...)
	for _, g := range problem.games {
		minimum[g.home] += min(g.homeGain[0], min(g.homeGain[1], g.homeGain[2]))
		minimum[g.away] += min(g.awayGain[0], min(g.awayGain[1], g.awayGain[2]))
	}
	var candidates []int
	for i, points := range minimum {
		if !problem.ranked[i] {
			exempt |= uint64(1) << i
			continue
		}
		if int32(i) == target {
			continue
		}
		if points > cap {
			exempt |= uint64(1) << i
			mandatory++
		} else {
			candidates = append(candidates, i)
		}
	}
	if mandatory > rank {
		return true, 0
	}
	nodes := 0
	var visit func(uint64, int, int) bool
	visit = func(mask uint64, start, slots int) bool {
		if nodes >= budget {
			return false
		}
		nodes++
		if referenceCapCapacityConsistent(problem, cap, mask) {
			return false
		}
		if slots == 0 {
			return true
		}
		for i := start; i < len(candidates); i++ {
			if !visit(mask|(uint64(1)<<candidates[i]), i+1, slots-1) {
				return false
			}
		}
		return true
	}
	return visit(exempt, 0, rank-mandatory), nodes
}

func TestReferenceCapacityProofRetainsFeasibleAssignments(t *testing.T) {
	problem := &jointPointCapProblem{base: []int{0, 1, 2}, ranked: []bool{true, true, true}, games: []jointPointCapGame{
		{home: 0, away: 1, prob: [3]float64{.2, .3, .5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
		{home: 1, away: 2, prob: [3]float64{.2, .3, .5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
		{home: 2, away: 0, prob: [3]float64{.2, .3, .5}, homeGain: [3]int{0, 1, 3}, awayGain: [3]int{3, 1, 0}},
	}}
	for _, p := range []*jointPointCapProblem{problem, problem.negated()} {
		for cap := -8; cap <= 8; cap++ {
			for mask := uint64(0); mask < 8; mask++ {
				feasible := false
				for code := 0; code < 27; code++ {
					points := append([]int(nil), p.base...)
					q := code
					for _, g := range p.games {
						o := q % 3
						q /= 3
						points[g.home] += g.homeGain[o]
						points[g.away] += g.awayGain[o]
					}
					valid := true
					for i, v := range points {
						if mask&(1<<i) == 0 && v > cap {
							valid = false
						}
					}
					feasible = feasible || valid
				}
				if feasible && !referenceCapCapacityConsistent(p, cap, mask) {
					t.Fatalf("valid assignments removed: cap=%d mask=%d", cap, mask)
				}
			}
			for target := int32(0); target < 3; target++ {
				for rank := 0; rank < 3; rank++ {
					impossible, _ := referenceCapImpossible(p, target, rank, cap, 100)
					if !impossible {
						continue
					}
					for code := 0; code < 27; code++ {
						points := append([]int(nil), p.base...)
						q := code
						for _, g := range p.games {
							o := q % 3
							q /= 3
							points[g.home] += g.homeGain[o]
							points[g.away] += g.awayGain[o]
						}
						above := 0
						for i, v := range points {
							if int32(i) != target && v > cap {
								above++
							}
						}
						if above <= rank {
							t.Fatalf("false impossibility: cap=%d target=%d rank=%d", cap, target, rank)
						}
					}
				}
			}
		}
	}
}

func TestRarePositionReferenceDeepProof(t *testing.T) {
	paths, output := os.Getenv("RARE_POSITION_BUDGET_EXPERIMENT_REQUESTS"), os.Getenv("RARE_POSITION_REFERENCE_PROOF_OUTPUT")
	if paths == "" || output == "" {
		t.Skip("set saved requests and absolute proof output directory")
	}
	if !filepath.IsAbs(output) {
		t.Fatal("absolute output required")
	}
	if err := os.MkdirAll(output, 0700); err != nil {
		t.Fatal(err)
	}
	for _, path := range strings.Split(paths, ",") {
		data, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		var input GroupType
		if err := json.Unmarshal(data, &input); err != nil {
			t.Fatal(err)
		}
		group := cloneGroupForBenchmark(input)
		campaign, table, order := diversifiedBenchmarkSetup(group)
		if len(order) == 0 || order[0] != PT {
			continue
		}
		problem := newJointPointCapProblem(group, campaign)
		if problem == nil {
			continue
		}
		// Encode wins only when allowed in the ordered prefix. This permits
		// sound impossibility proofs while retaining all later tiebreak ties.
		stride := conditionedRankWinStride(order, group, campaign, table, "lookahead")
		for i, c := range campaign {
			if c != nil {
				problem.base[i] = c.points * stride
				if stride > 1 {
					problem.base[i] += c.wins
				}
			}
		}
		for i := range problem.games {
			g := &problem.games[i]
			for o := 0; o < 3; o++ {
				g.homeGain[o] *= stride
				g.awayGain[o] *= stride
			}
			if stride > 1 {
				g.homeGain[2]++
				g.awayGain[0]++
			}
		}
		dual := problem.negated()
		type proof struct {
			Team       int    `json:"team"`
			Position   int    `json:"position"`
			Impossible bool   `json:"impossible"`
			Nodes      int    `json:"nodes"`
			Method     string `json:"method"`
		}
		var proofs []proof
		started := time.Now()
		for _, tg := range group.Team_groups {
			target := table.Query(uint32(tg.Team_id))
			maxValue, minValue := problem.base[target], problem.base[target]
			for _, g := range problem.games {
				var gains [3]int
				if g.home == target {
					gains = g.homeGain
				} else if g.away == target {
					gains = g.awayGain
				} else {
					continue
				}
				maxValue += max(gains[0], max(gains[1], gains[2]))
				minValue += min(gains[0], min(gains[1], gains[2]))
			}
			for rank := range group.Team_groups {
				ok, nodes := referenceCapImpossible(problem, target, rank, maxValue, 200000)
				method := "joint_maximum_capacity"
				if !ok {
					floor, spent := referenceCapImpossible(dual, target, len(group.Team_groups)-1-rank, -minValue, 200000)
					ok = floor
					nodes += spent
					method = "joint_minimum_capacity"
				}
				proofs = append(proofs, proof{tg.Team_id, rank + 1, ok, nodes, method})
				if ok {
					t.Logf("group=%d team=%d rank=%d method=%s nodes=%d", group.Id, tg.Team_id, rank+1, method, nodes)
				}
			}
		}
		encoded, err := json.MarshalIndent(proofs, "", "  ")
		if err != nil {
			t.Fatal(err)
		}
		// Snapshot filenames avoid collisions between two requests for one group.
		hash := fmt.Sprintf("%x", sha256.Sum256(data))
		name := fmt.Sprintf("proof-%d-%s.json", input.Id, hash[:8])
		encoded, err = json.MarshalIndent(map[string]interface{}{"group": input.Id, "input_sha256": hash, "stride": stride,
			"nodes_per_case": 200000, "elapsed_ms": float64(time.Since(started).Microseconds()) / 1000, "cells": proofs}, "", "  ")
		if err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(output, name), encoded, 0600); err != nil {
			t.Fatal(err)
		}
		t.Logf("group=%d proofs=%d elapsed=%s", group.Id, len(proofs), time.Since(started))
	}
}
