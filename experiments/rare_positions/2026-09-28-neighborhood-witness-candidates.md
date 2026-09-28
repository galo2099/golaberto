# Neighborhood witnesses as probability candidates

## Current reachability passes

The matched-point pipeline has five reachability mechanisms: a hard individual-points bound; conditioned point-event simulation (which can prove an event empty or produce a sampled rank witness); the neighborhood search that changes one or two fixtures from a conditioned season; joint fixture point-cap impossibility propagation; and the constructive full-fixture witness search. The first and fourth can prove impossibility. The second, third, and fifth can prove reachability with a complete season. Only an actual sampled rank hit supplies a probability estimate.

The neighborhood pass produced Avaí (team 68) 2nd- and 3rd-place proofs in group 16653. `searchReachabilityNeighbors` already returns a full win/draw/loss vector for each proof, but `runConditionedZeroSearch` currently consumes only the team and rank when it marks `reachable_by_construction`. The matched-point estimator has no proposal-candidate interface. The diversified importance sampler does have one, but its candidates are vectors of Poisson scoring means rather than fixture-outcome witnesses.

## Candidate projection experiment

With the saved group 16653 request and seed 808, the two Avaí witnesses differ at one of 85 unplayed fixtures: game 364696, Fortaleza (22) vs team 550, is a home loss in the 2nd-place witness and a draw in the 3rd-place witness. The other 84 win/draw/loss outcomes match.

I measured single-fixture sensitivity by replacing each witness outcome with each alternative and checking Avaí's rank with the full standings sorter. The 2nd-place witness has 62 sensitive fixtures; the 3rd-place witness has 56. I used the most sensitive 8, 16, 32, and then all sensitive fixtures to build Poisson-mean proposals compatible with `simulateDiversifiedMixtureBatch`: home-win and away-win witnesses multiply the favored team's mean by 1.8 and divide the other's by 1.8; draws multiply both means by 0.7. The existing 5% original-model defensive mixture and likelihood weights remained in use.

| Target | Changed fixtures | Validation samples | Hits | Event ESS |
| --- | ---: | ---: | ---: | ---: |
| Avaí 2nd | 8, 16, 32, 62 | 5,000 each | 0 each | 0 |
| Avaí 3rd | 8, 16, 32, 56 | 5,000 each | 0 each | 0 |
| Avaí 2nd | 62 | 100,000 | 0 | 0 |
| Avaí 3rd | 56 | 100,000 | 0 | 0 |

The 5,000-sample validations took about 80–100 ms each. The two 100,000-sample checks took 1.66 and 1.69 seconds. These candidates would be rejected by the existing validation gate, so the experiment did not change production estimates.

## Consequence

The reachability search can mechanically generate candidate inputs, and its witness vectors are useful search evidence. Directly projecting many fixture outcomes into independent Poisson means did not produce a useful proposal for Avaí. The next proposal family should condition on a selected subset of fixture outcomes and compute exact `P/Q` mixture weights, with more than one witness component when available. It must demonstrate event hits and stable weighted effective sample size on held-out samples before being used for the odds matrix. The original-model component should retain support for seasons outside the selected witness neighborhoods.
