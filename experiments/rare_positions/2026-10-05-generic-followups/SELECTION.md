# Policy selection before holdout

Screening complete: ten panels, all processes successful. Holdout runs have
not started. This records selection using the protocol's fixed criteria.

| P16 policy | Passing pairs / 6 | Minimum MAIN/CHECK ESS | Worst MAIN/CHECK weight share |
| --- | ---: | ---: | ---: |
| Reused adaptive baseline | 5 | 5.1992 | 42.5494% |
| Equal allocation | 5 | 2.7643 | 58.4815% |
| Pilot 50,000 | 6 | 11.6994 | 28.1715% |
| Bounds guidance | 6 | 29.2244 | 15.0464% |
| Native goals | 2 | 1.0151 | 99.2541% |

Select **bounds guidance**, keeping trained allocation, pilot 5,000 and all
other baseline settings. Run it and the baseline on both fixtures with fresh
pilot 60109 and final seeds 60101/60103/60107. No adjustment after holdout.

For P15, the larger pilot retained branches 51, 53 and 54 on both fixtures;
all six screening pairs passed. Run the protocol's three training policies on
both fixtures with the same fresh pilot and final seeds: pilot 500/three
messages, pilot 5,000/three messages, and pilot 500/four messages. Branch IDs
are diagnostic observations, not forced selections or algorithm rules.

All decisions use exploratory observed diagnostics. They do not establish
exact probabilities, production affordability, or generalization beyond the
retained fixtures. Keep failures and concentrated accepted pairs in the report.
