# Group 16498: fresh 100× reproduction and backward analysis

Written October 5, 2026 before the new high-work runs.

Reproduce R67's per-cell experiment on Flamengo/13th and Palmeiras/15th–16th
for the original and refreshed two-game fixtures. This measures six cells,
not a 100× resimulation of the entire odds matrix. Probabilities are fractions.

Freeze one release example, its source and hashes. Run requests serially with
four workers, clearing inherited experiment controls through the existing R67
runner. Do not compile during measured experiments. Every branch is sampled;
bound-based branch omission remains off. Acceptance gates and importance
weights remain unchanged.

| Cell | Finals per MAIN and CHECK | Pilot draws per branch | Floor | Proposal |
| --- | ---: | ---: | ---: | --- |
| Flamengo13 | 300,000 | 2,500 | 10 | Complete tree, 64 leaves, one message |
| Palmeiras15 | 600,000 | 500 | 2 | Complete tree, 256 leaves, three messages, generic rank priority |
| Palmeiras16 | 2,500,000 | 5,000 | 200 | Complete secondary-refined branches |

Construction seed is 808. Final seeds are 60013, 60017 and 60029. Run each cell
first with pilot seed 808, then with fresh pilot seed 60031. Reuse final seeds
across the pilot controls to isolate training/proposal changes. MAIN and CHECK
use separate derived streams; final observations never alter proposals or
allocation. The 36 pairs are not 36 independent training configurations.

Analyze estimates, concentration and acceptance before interpreting family
contributions. Join branch metadata to probability contributions, then group by
semantic target paths and rival-set constraints rather than branch indices.
Branch probabilities already sum to the total; do not multiply by draw share.
Compare discovery evidence with final allocation and measured contribution.
First-hit outcome witnesses illustrate sampled points/wins and W/D/L paths;
actual goal scores are absent, so these records cannot replay goal tiebreaks or
certify exact probabilities.

The after fixture includes the two results and refreshed ratings. It does not
isolate their causal effects. Report wall time, CPU and complete versus partial
modeled accounting separately. Compare results with the previous provisional
R67 references and production estimates; do not certify either as truth.

Finish with a concrete generic algorithm design and requirements for a bounded
follow-up. Account for R68's unsuccessful retunes and unaffordable conservative
family reserves. Do not activate experimental production defaults, commit,
push or write the database.
