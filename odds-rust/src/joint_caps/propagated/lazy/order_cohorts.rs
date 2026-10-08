use super::*;

#[cfg(test)]
mod tests;

fn expected_raw_points(
    m: &Model,
    target: &[RankGame],
    remaining: &[RankGame],
    root: &[u8],
) -> Vec<f64> {
    let mut points: Vec<f64> = m
        .base
        .iter()
        .map(|standing| f64::from(standing.points))
        .collect();
    for (game, &outcome) in target.iter().zip(root) {
        points[game.home] += [0.0, 1.0, 3.0][outcome as usize];
        points[game.away] += [3.0, 1.0, 0.0][outcome as usize];
    }
    for game in remaining {
        let home: f64 = [0.0, 1.0, 3.0]
            .into_iter()
            .zip(game.prob)
            .map(|(points, probability)| points * probability)
            .sum();
        let away: f64 = [3.0, 1.0, 0.0]
            .into_iter()
            .zip(game.prob)
            .map(|(points, probability)| points * probability)
            .sum();
        points[game.home] += home;
        points[game.away] += away;
    }
    points
}

impl LazyJoint {
    /// Conservative preflight for constructing the native order base and
    /// cloning it once for one cohort arm. The common constructor allowance
    /// bounds target rows and any copied goal tables. At most 33 sets of root
    /// pattern metadata, one fallback guide, and the base/target/remaining
    /// vectors are copied separately; the final term bounds those structures.
    pub(crate) fn order_base_constructor_and_clone_allowance(m: &Model, cell: Cell) -> usize {
        let native_setup_bound = crate::rare_tail::budget::setup_cost(60_000, 4_000_000);
        let constructor = Self::order_constructor_allowance(m, cell);
        let common = constructor.saturating_sub(native_setup_bound);
        let g = m.fixtures.len();
        let n = m.n;
        let clone_bound = common
            .saturating_add(
                33usize.saturating_mul(
                    7usize
                        .saturating_mul(g)
                        .saturating_add(3usize.saturating_mul(n)),
                ),
            )
            .saturating_add(n)
            .saturating_add(2usize.saturating_mul(g));
        constructor.saturating_add(clone_bound)
    }

    /// Build a native 32-root proposal for later independent cohort fitting.
    /// Admission uses the full native setup reservation before construction.
    pub(crate) fn new_order_base(
        m: &Model,
        cell: Cell,
        seed: i64,
        work_limit: usize,
    ) -> (Option<Self>, usize, serde_json::Value) {
        let valid_model = m.keys.first() == Some(&Key::Pt)
            && m.request.phase.bonus_points == 0
            && (
                m.request.phase.championship.point_win,
                m.request.phase.championship.point_draw,
                m.request.phase.championship.point_loss,
            ) == (3, 1, 0)
            && m.ids.len() == m.n
            && m.n <= 32
            && m.fixtures.len() <= 256
            && m.fixtures.iter().all(|g| g.home != g.away)
            && cell.team < m.n
            && cell.rank < m.n;
        if !valid_model {
            return (
                None,
                0,
                json!({"status":"declined","reason":"unsupported_model","work":0,"allowance":0}),
            );
        }
        let native_setup_bound = crate::rare_tail::budget::setup_cost(60_000, 4_000_000);
        let total_allowance = Self::order_constructor_allowance(m, cell);
        let common_allowance = total_allowance.saturating_sub(native_setup_bound);
        let admission = common_allowance.saturating_add(native_setup_bound);
        if work_limit < admission {
            let preflight_charge = work_limit.min(1);
            return (
                None,
                preflight_charge,
                json!({"status":"declined","reason":"constructor_budget","work":preflight_charge,"requested_allowance":admission}),
            );
        }
        let plan = Self::configured(m, cell, seed, &[], 32, false, false, None, Some(4));
        let charged = plan.as_ref().map_or(admission, |plan| {
            common_allowance.saturating_add(plan.setup_work())
        });
        let diagnostics = json!({
            "status": if plan.is_some() {"ready"} else {"declined"},
            "reason": if plan.is_some() {""} else {"native_build_failed"},
            "roots": plan.as_ref().map_or(0, |p| p.roots.len()),
            "native_width": 4,
            "work": charged,
            "allowance": admission,
            "requested_allowance": admission
        });
        (plan, charged, diagnostics)
    }
    fn order_constructor_allowance(m: &Model, cell: Cell) -> usize {
        let n = m.n;
        let f = m.fixtures.len();
        let wins = m.keys.get(1) == Some(&Key::W);
        let mut d = vec![0usize; n];
        for game in &m.fixtures {
            d[game.home] = d[game.home].saturating_add(1);
            d[game.away] = d[game.away].saturating_add(1);
        }
        let stride = if wins {
            m.base
                .iter()
                .zip(&d)
                .map(|(b, &x)| b.wins.saturating_add(x as i32))
                .max()
                .unwrap_or(0)
                .saturating_add(1)
                .max(1) as usize
        } else {
            1
        };
        let gain = 3usize
            .saturating_mul(stride)
            .saturating_add(usize::from(wins));
        let spans: Vec<usize> = d
            .iter()
            .map(|&x| gain.saturating_mul(x).saturating_add(1))
            .collect();
        let dt = d.get(cell.team).copied().unwrap_or(0);
        let st = spans.get(cell.team).copied().unwrap_or(1);
        let mut b = f
            .saturating_mul(n.saturating_add(4))
            .saturating_add(n.saturating_mul(n.saturating_add(1)));
        b = b
            .saturating_add(dt.saturating_add(1).saturating_mul(st))
            .saturating_add(3usize.saturating_mul(dt).saturating_mul(st));
        if crate::rare_tail::value("RUST_ODDS_LAZY_GOALS") == "1" {
            let gd = if wins { 2 } else { 1 };
            if m.keys.get(gd) == Some(&Key::Gd)
                && !m
                    .fixtures
                    .iter()
                    .any(|x| x.home_sampler.mean > 32. || x.away_sampler.mean > 32.)
            {
                for x in m
                    .fixtures
                    .iter()
                    .filter(|x| x.home == cell.team || x.away == cell.team)
                {
                    let support = |mean: f64| {
                        if mean <= 0. {
                            1
                        } else {
                            ((mean.ceil() as usize).saturating_mul(4).saturating_add(64)).min(1000)
                        }
                    };
                    b = b.saturating_add(
                        9usize
                            .saturating_mul(support(x.home_sampler.mean))
                            .saturating_mul(support(x.away_sampler.mean))
                            .saturating_mul(6),
                    );
                }
            } else {
                b = b.saturating_add(f.saturating_add(n).max(1));
            }
        }
        b = b.saturating_add(2usize.saturating_mul(n).saturating_mul(st));
        for t in 0..n {
            let mut row = f;
            for k in 0..d[t] {
                row = row
                    .saturating_add(4usize.saturating_mul(gain.saturating_mul(k).saturating_add(1)))
                    .saturating_add(gain.saturating_mul(k.saturating_add(1)).saturating_add(1));
            }
            b = b.saturating_add(row);
        }
        let a = (if wins {
            dt.saturating_add(1).saturating_mul(dt.saturating_add(2)) / 2
        } else {
            3usize.saturating_mul(dt).saturating_add(1)
        })
        .min(st);
        let mut term = spans
            .iter()
            .enumerate()
            .filter(|(t, _)| *t != cell.team)
            .fold(0usize, |s, (_, v)| s.saturating_add(*v));
        for c in 0..n.saturating_sub(1) {
            let tri = (c + 1).saturating_mul(c + 2) / 2;
            term = term.saturating_add(
                6usize
                    .saturating_mul(tri)
                    .saturating_add(n.saturating_mul(n)),
            );
        }
        term = term.saturating_add(n.saturating_mul(n));
        b = b.saturating_add(a.saturating_mul(term));
        b.saturating_add(dt.saturating_add(1).saturating_mul(st))
            .saturating_add(3usize.saturating_mul(dt).saturating_mul(st))
            .saturating_add(
                4usize
                    .saturating_mul(32)
                    .saturating_mul(10usize.saturating_mul(dt).saturating_add(1)),
            )
            .saturating_add(crate::rare_tail::budget::setup_cost(60_000, 4_000_000))
    }

    /// Selection work charged while building this proposal, in addition to
    /// the ordinary root and guide setup cost.
    pub(crate) fn order_cohort_setup_work(&self) -> usize {
        self.setup_work().saturating_add(self.family_selection_work)
    }

    /// Add inclusive rival-order cohorts to cached target roots. The cohorts
    /// are proposal hints only; the native target path retains full support.
    pub(crate) fn add_expected_cohorts(
        &mut self,
        m: &Model,
        rivals: usize,
        max_nodes: usize,
        max_guides: usize,
        max_selection_work: usize,
    ) -> serde_json::Value {
        let start_nodes = self.nodes;
        let start_guides = self.guide_values;
        let start_work = self.family_selection_work;
        let root_count = self.roots.len();
        let roots_available = self.roots.values().filter(|p| p.is_some()).count();
        macro_rules! report {
            ($status:expr, $covered:expr, $considered:expr, $added:expr, $work:expr, $reason:expr) => {
                json!({
                    "status": $status,
                    "mode": "expected_points",
                    "roots_covered": $covered,
                    "roots_available": roots_available,
                    "cohorts_considered": $considered,
                    "cohorts_added": $added,
                    "selection_work": $work,
                    "nodes_before": start_nodes,
                    "nodes_after": self.nodes,
                    "guides_before": start_guides,
                    "guides_after": self.guide_values,
                    "omission_enabled": self.omit,
                    "reason": $reason
                })
            };
        }
        if self.omit {
            return report!("declined", 0, 0, 0, 0, "omission_enabled");
        }
        if self.subset {
            return report!("declined", 0, 0, 0, 0, "subset_enabled");
        }
        if !self.families.is_empty() {
            return report!("declined", 0, 0, 0, 0, "families_already_present");
        }
        if rivals == 0 || max_nodes == 0 || max_guides == 0 || max_selection_work == 0 {
            return report!("declined", 0, 0, 0, 0, "zero_budget");
        }

        // Conservative preflight is deliberately cheap and precedes root
        // cloning, per-root scoring, sorting, PMF allocation, and construction.
        let teams = m.n.saturating_sub(1);
        let candidate_limit = 4usize.min(1usize.saturating_add(teams.saturating_mul(teams)));
        let estimated_work = root_count
            .saturating_mul(4usize.saturating_add(self.target.games.len()))
            .saturating_add(m.n.saturating_mul(self.remaining.len().saturating_add(1)));
        let estimated_work = estimated_work.saturating_add(
            roots_available
                .saturating_mul(m.n.saturating_mul((m.n.max(2) as f64).log2() as usize + 1)),
        );
        let estimated_work = estimated_work.saturating_add(
            roots_available.saturating_mul(candidate_limit.saturating_mul(m.n.saturating_add(1))),
        );
        let estimated_work = estimated_work.saturating_add(
            roots_available.saturating_mul(
                teams
                    .saturating_mul(teams)
                    .saturating_mul((teams.max(2) as f64).log2() as usize + 1)
                    .saturating_add(candidate_limit.saturating_mul(candidate_limit)),
            ),
        );
        // Do not reject the whole arm because every possible construction
        // would exceed the budget. Construction allowance is charged per
        // candidate below, in primary-before-swap order.
        let per_attempt =
            m.n.saturating_add(self.remaining.len())
                .saturating_mul(m.n.saturating_add(1))
                .saturating_mul(8);
        let charged = estimated_work.min(max_selection_work);
        if estimated_work > max_selection_work {
            self.family_selection_work = self.family_selection_work.saturating_add(charged);
            let mut output = report!("declined", 0, 0, 0, charged, "selection_budget");
            output["roots_selection_declined"] = json!(roots_available);
            return output;
        }
        self.family_selection_work = self.family_selection_work.saturating_add(estimated_work);

        let mut roots: Vec<(Vec<u8>, f64)> = self
            .roots
            .iter()
            .filter_map(|(key, pattern)| {
                let pattern = pattern.as_ref()?;
                let allocation = self.root_allocation.get(key).copied().unwrap_or_else(|| {
                    pattern.mass * pattern.joint.residual_hint.max(1e-80) * pattern.tilt
                });
                Some((key.clone(), allocation))
            })
            .collect();
        roots.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let mut root_data = Vec::with_capacity(roots.len());
        for (root, _) in &roots {
            let mut totals = self.base.clone();
            let raw_totals = expected_raw_points(m, &self.target.games, &self.remaining, root);
            for (game, &outcome) in self.target.games.iter().zip(root) {
                totals[game.home] += game.hg[outcome as usize];
                totals[game.away] += game.ag[outcome as usize];
            }
            let anchor = totals[self.cell.team];
            let mut rivals = Vec::with_capacity(teams);
            for team in 0..m.n {
                if team == self.cell.team {
                    continue;
                }
                rivals.push((team, raw_totals[team]));
            }
            root_data.push((root.clone(), totals, raw_totals, anchor, rivals));
        }
        let mut primary_plans = Vec::new();
        let mut swap_plans = Vec::new();
        let mut considered = 0usize;
        for (root, _totals, raw_totals, anchor, mut values) in root_data {
            values.sort_by(|a, b| {
                b.1.total_cmp(&a.1)
                    .then_with(|| raw_totals[b.0].total_cmp(&raw_totals[a.0]))
                    .then_with(|| m.ids[a.0].cmp(&m.ids[b.0]))
            });
            if self.cell.rank > values.len() {
                continue;
            }
            let primary: Vec<usize> = values
                .iter()
                .take(self.cell.rank)
                .map(|value| value.0)
                .collect();
            let mut memberships = vec![primary.clone()];
            let mut swaps = Vec::new();
            for above in 0..self.cell.rank {
                for below in self.cell.rank..values.len() {
                    swaps.push((
                        self.cell.rank - 1 - above + below - self.cell.rank,
                        above,
                        below,
                    ));
                }
            }
            swaps.sort_unstable();
            for (_, above, below) in swaps.into_iter().take(3) {
                let mut swapped = primary.clone();
                swapped.retain(|team| *team != values[above].0);
                swapped.push(values[below].0);
                swapped.sort_unstable();
                if !memberships.contains(&swapped) {
                    memberships.push(swapped);
                }
            }
            for (membership_index, membership) in memberships.into_iter().enumerate() {
                considered += 1;
                let chosen: std::collections::HashSet<_> = membership.iter().copied().collect();
                let mut lower = vec![i32::MIN / 4; m.n];
                let mut upper = vec![i32::MAX / 4; m.n];
                for (team, _) in &values {
                    if chosen.contains(team) {
                        lower[*team] = anchor;
                    } else {
                        upper[*team] = anchor;
                    }
                }
                let item = (root.clone(), membership, lower, upper);
                if membership_index == 0 {
                    primary_plans.push(item);
                } else {
                    swap_plans.push(item);
                }
            }
        }

        let old_node_limit = self.node_limit;
        let old_guide_limit = self.guide_limit;
        let old_rival_limit = self.rival_limit;
        self.node_limit = start_nodes.saturating_add(max_nodes);
        self.guide_limit = start_guides.saturating_add(max_guides);
        self.rival_limit = rivals.clamp(1, 6);
        // JointKey omits rival width, so entries prepared at native width are
        // not valid when this temporary width override is active.
        self.joint_cache.clear();
        let mut added = 0usize;
        let mut refuted = 0usize;
        let mut capped = 0usize;
        let mut selection_declined = 0usize;
        let mut covered = std::collections::HashSet::new();
        let mut refuted_roots = std::collections::HashSet::new();
        let mut capped_roots = std::collections::HashSet::new();
        let mut selection_declined_roots = std::collections::HashSet::new();
        #[cfg(test)]
        let mut root_paths = Vec::new();
        let all_plans: Vec<_> = primary_plans.into_iter().chain(swap_plans).collect();
        let scheduled = all_plans.len();
        let mut attempted = 0usize;
        let mut duplicate_patterns = 0usize;
        let mut stopped_at = None;
        'candidates: for (index, (root, _membership, lower, upper)) in all_plans.iter().enumerate()
        {
            if self.nodes >= self.node_limit || self.guide_values >= self.guide_limit {
                capped += 1;
                capped_roots.insert(root.clone());
                stopped_at = Some(index);
                break;
            }
            let spent = self.family_selection_work.saturating_sub(start_work);
            if spent.saturating_add(per_attempt) > max_selection_work {
                selection_declined += 1;
                selection_declined_roots.insert(root.clone());
                stopped_at = Some(index);
                break;
            }
            attempted += 1;
            self.family_selection_work = self.family_selection_work.saturating_add(per_attempt);
            match self.build_bounded(root, m.n, None, None, Some((lower, upper)), true) {
                Ok(Some(pattern)) => {
                    let labels = vec![2; m.n.saturating_sub(1)];
                    let families = self.families.entry(root.clone()).or_default();
                    if !families
                        .iter()
                        .any(|(_, old, _)| old.lower == pattern.lower && old.upper == pattern.upper)
                    {
                        #[cfg(test)]
                        let above_team_ids: Vec<_> =
                            _membership.iter().map(|team| m.ids[*team]).collect();
                        families.push((labels, pattern, 1.0));
                        added += 1;
                        covered.insert(root.clone());
                        #[cfg(test)]
                        root_paths.push(json!({"root": root, "above_team_ids": above_team_ids}));
                    } else {
                        duplicate_patterns += 1;
                    }
                }
                Ok(None) => {
                    refuted += 1;
                    refuted_roots.insert(root.clone());
                }
                Err(()) => {
                    capped += 1;
                    capped_roots.insert(root.clone());
                    stopped_at = Some(index + 1);
                    break 'candidates;
                }
            }
        }
        let unattempted = scheduled.saturating_sub(attempted);
        let mut unattempted_roots = std::collections::HashSet::new();
        if let Some(index) = stopped_at {
            for (root, _, _, _) in &all_plans[index..] {
                unattempted_roots.insert(root.clone());
            }
        }
        self.node_limit = old_node_limit;
        self.guide_limit = old_guide_limit;
        self.rival_limit = old_rival_limit;
        self.joint_cache.clear();
        for proposals in self.families.values_mut() {
            let total: f64 = proposals.iter().map(|(_, _, weight)| *weight).sum();
            if total > 0.0 && total.is_finite() {
                for (_, _, weight) in proposals {
                    *weight /= total;
                }
            }
        }
        let mut output = report!(
            if capped > 0 || selection_declined > 0 {
                "partial"
            } else if added > 0 {
                "added"
            } else {
                "no_feasible_cohorts"
            },
            covered.len(),
            considered,
            added,
            self.family_selection_work.saturating_sub(start_work),
            ""
        );
        #[cfg(test)]
        {
            output["root_paths"] = json!(root_paths);
        }
        output["candidates_scheduled"] = json!(scheduled);
        output["candidates_attempted"] = json!(attempted);
        output["candidates_unattempted"] = json!(unattempted);
        output["roots_unattempted"] = json!(unattempted_roots.len());
        output["duplicate_patterns"] = json!(duplicate_patterns);
        output["candidates_refuted"] = json!(refuted);
        output["roots_refuted"] = json!(refuted_roots.len());
        output["candidates_capped"] = json!(capped);
        output["roots_capped"] = json!(capped_roots.len());
        output["candidates_selection_declined"] = json!(selection_declined);
        output["roots_selection_declined"] = json!(selection_declined_roots.len());
        output
    }
}
