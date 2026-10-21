# Group 17045: reachable cells without probability estimates

This records the baseline before the recovery expansion. Implementation and
new measurements are in
[the follow-up report](2026-10-21-group-17045-expanded-recovery.md).

The uploaded request is preserved byte-for-byte in
`odds-rust/tests/fixtures/group-17045-2026-10-20.json`. It describes the
2026–27 Champions League league phase: 36 teams, 144 matches, 45 played,
99 remaining, and five overlapping qualification zones. Every team has eight
scheduled matches and five or six remaining. The sort order is
`pt,gd,gf,g_away,w,name`.

## Reproduction

Build the current source; the pre-existing executable in the workspace was stale.

```sh
cargo build --release --locked --manifest-path odds-rust/Cargo.toml
odds-rust/target/release/golaberto-odds estimate \
  odds-rust/tests/fixtures/group-17045-2026-10-20.json \
  /tmp/group-17045-response.json 808 4 2>/tmp/group-17045.log
cargo test --release --locked --manifest-path odds-rust/Cargo.toml --test group_17045
```

At seed 808, current source produces 1,282 positive estimates and 14 zero
estimates. All 1,296 cells have public reachability `reachable`; none is
`impossible` or `undecided`. The following positions are one-based (response
rank keys are zero-based). The request contains team IDs but no team names.

| Team ID | Positions with no probability estimate |
| --- | --- |
| 27 | 36 |
| 36 | 32–36 |
| 401 | 36 |
| 855 | 32–36 |
| 1455 | 1–2 |

The complete seed-808 response is identical with one and four workers. Seed
809 leaves 12 reachable zero cells: 27/36; 36/33–36; 401/36; 637/35;
855/33–36; and 1455/1. Thus the exact missing set depends on sampling seed.
At seed 808, the maximum row or column error in the percentage matrix is
approximately 4.3e-14. Matrix reconciliation reports no rollback.

## Why the estimates are missing

1. **The ordinary sampling pool misses rare rank events.** The initial
   100,000-season matched point pool leaves 25 zero cells at seed 808. Search
   reduces this to 14. Conditioning on the target team's points does not ensure
   that enough of its 35 rivals finish on the required side. For example,
   teams 36 and 855 already have nine points but are missing bottom-five
   estimates; team 1455 has zero points and a goal difference of -12 but is
   missing top-two estimates. Many simultaneous rival outcomes and sometimes
   goal-difference reversals are needed.
2. **Finding a legal season does not estimate its probability.** Constructed
   seasons and search witnesses establish reachability. Many probability pilots
   still have no hits or inadequate effective sample size. At seed 808,
   team 1455/1 has one pilot hit (ESS 1), team 36/36 has three (ESS about 1.01),
   and team 855/36 has none. A candidate can therefore be reachable while
   failing the independent main/check publication criteria in
   `odds-rust/src/rare_tail.rs` (`accepted` and `publishable`). A stored zero
   and zero standard error do not constitute an impossibility proof.
3. **Several recovery paths explicitly reject 36 teams.**
   `LazyJoint::configured` in
   `odds-rust/proposals/src/joint_caps/propagated/lazy.rs` rejects `m.n > 32`.
   Complete target-tree recovery bootstraps through this constructor. The
   seed-808 `rust_odds_target_overflow_tree_summary` reports 14 root-count
   attempts, each returning `unsupported tree model`, zero supported root
   counts, and zero added estimates. The constructor backs selected-team and
   exception sets with `u32` masks, including shifts indexed by team number.
   Removing the guard alone would be unsafe.
4. **Other fallbacks also lack support for this request.** Target-total
   certification (`core/src/target_limits.rs`) and native order-cohort
   construction (`proposals/src/joint_caps/propagated/lazy/order_cohorts.rs`)
   impose 32-team limits. Exact-cap recovery (`proposals/src/joint_caps/exact.rs`)
   additionally requires points followed immediately by wins, whereas this
   request uses goal difference second; it reports `no_eligible_cells`.
   The final packed-witness fallback rejects this score-key suffix, which
   includes away goals and wins after goal difference, and reports
   `supported: false`. Native full-world complement reports
   `no_saturated_title_or_target_cap`.

These are sampling and implementation-support gaps. The Rails UI correctly
marks a reachable zero with `*` and the explanation “Reachable; no probability
estimate found.” Its display is based on the normalized public reachability
status, so changing formatting would not supply the missing estimates.

## Follow-up direction

Expand selected-team/exception masks and all connected proposal operations to
support at least 36 teams, then review the target-limit and order-cohort gates
and bound setup/sampling costs on this fixture. Preserve the actual
goal-difference-first and away-goal sorting semantics. Exact-cap recovery
needs separate support for goal-difference-first requests; lifting its team
limit is insufficient. Evaluate independent-seed quality before publishing
new tiny probabilities. The regression tests deliberately characterize the
current gap and should change when these recovery paths gain support.

This change imports the request and tests the observed behavior; production
estimation behavior is unchanged.

Validation: the release build succeeded, all three `group_17045` tests passed,
and the rebuilt CLI completed the seed-808 one/four-worker comparison and the
seed-809 run. Existing compiler warnings remain unchanged.
