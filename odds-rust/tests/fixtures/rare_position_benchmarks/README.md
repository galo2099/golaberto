# Synthetic finishing-position regression fixtures

These ten JSON files were moved unchanged from
`go/testdata/rare_position_benchmarks`. The five requests cover tight standings,
wide strength differences, a dominant team, a weak team, and a cluster of
blocking rivals. Each has 20 teams, 50 played games, and 330 remaining games.
`script/generate_rare_position_benchmarks.py` generates the request scenarios.

The corresponding `.reference.json` files contain 5,000,000-season Monte Carlo
results from the retired Go estimator. Their metadata records the original
seed and SHA-256 of the exact request bytes. Cells use team IDs and zero-based
positions, with `p = count / samples`. The reference sampled full Poisson
scorelines and sorted by points, goal difference, goals scored, then bias.

`odds-rust/tests/rare_position_benchmarks.rs` validates each reference's input
hash, dimensions, probability/count consistency, and team/position totals.
Default regression checks run 10,000 Rust scout seasons per scenario with a
fixed seed. Every cell is compared with the frozen probability using six
combined standard errors plus three scout counts of allowance. A second default
test checks the current 100,000-season production pool's matrix and compares
cells with at least 25 reference hits using six combined standard errors plus
three production counts of allowance. This is an empirical regression gate for
these fixed cases; jackknife errors do not quantify pooling bias. See
`odds-rust/README.md` for commands.

The references never enter the estimator. They are noisy test expectations,
not exact probabilities: a zero reference count does not prove impossibility.
These synthetic scenarios supplement real-group tests and do not establish
accuracy for the rare-event fallback's extreme tails.
