use crate::lookahead::RankGame;

#[derive(Clone, Debug)]
pub(super) struct RootShare {
    pub(super) prior: f64,
    pub(super) count_chance: f64,
    pub(super) raw_score: f64,
    pub(super) normalized_share: f64,
}

pub(super) struct Screen {
    pub(super) roots: Vec<RootShare>,
    pub(super) work: usize,
    pub(super) reason: Option<&'static str>,
    pub(super) leaf_comparators: Option<Vec<Vec<[f64; 3]>>>,
    pub(super) leaf_work: usize,
    pub(super) leaf_reason: Option<&'static str>,
}

pub(super) fn normalized(raw: &[f64]) -> Vec<f64> {
    if raw.is_empty() {
        return Vec::new();
    }
    let max = raw
        .iter()
        .copied()
        .filter(|x| x.is_finite())
        .fold(0.0, f64::max);
    if max <= 0.0 {
        return vec![1.0 / raw.len() as f64; raw.len()];
    }
    let scaled: Vec<_> = raw
        .iter()
        .map(|x| {
            if x.is_finite() {
                (x / max).max(0.0)
            } else {
                0.0
            }
        })
        .collect();
    let total: f64 = scaled.iter().sum();
    if !total.is_finite() || total <= 0.0 {
        return vec![1.0 / raw.len() as f64; raw.len()];
    }
    scaled.iter().map(|x| x / total).collect()
}

pub(super) fn mix_shares(raw: &[f64], uniform_weight: f64) -> Vec<f64> {
    let base = normalized(raw);
    if base.is_empty() {
        return base;
    }
    let u = uniform_weight.clamp(0.0, 1.0) / base.len() as f64;
    base.into_iter()
        .map(|x| x * (1.0 - uniform_weight.clamp(0.0, 1.0)) + u)
        .collect()
}

pub(super) fn increment(work: &mut usize, limit: usize, amount: usize) -> bool {
    if work.saturating_add(amount) > limit {
        *work = limit;
        false
    } else {
        *work += amount;
        true
    }
}

fn log_add(a: f64, b: f64) -> f64 {
    if a == f64::NEG_INFINITY {
        return b;
    }
    if b == f64::NEG_INFINITY {
        return a;
    }
    let max = a.max(b);
    max + ((a - max).exp() + (b - max).exp()).ln()
}

fn log_probability(p: f64) -> f64 {
    if p > 0.0 && p.is_finite() {
        p.ln()
    } else {
        f64::NEG_INFINITY
    }
}

pub(super) fn leaf_log_mass(
    comparators: &[[f64; 3]],
    rank: usize,
    above: bool,
    mask: u64,
    selected: u64,
    target_team: usize,
    work: &mut usize,
    limit: usize,
) -> Option<f64> {
    let n = comparators.len() + 1;
    let target_bit = 1u64.checked_shl(target_team as u32).unwrap_or(0);
    let mut dp = vec![f64::NEG_INFINITY; rank.saturating_add(1).min(n)];
    if rank >= dp.len() {
        return Some(f64::NEG_INFINITY);
    }
    dp[0] = 0.0;
    let mut seen = 0usize;
    let mut rival = 0usize;
    for team in 0..n {
        if target_bit & (1 << team) != 0 {
            continue;
        }
        let [less, equal, greater] = comparators[rival];
        rival += 1;
        if !increment(work, limit, dp.len()) {
            return None;
        }
        let bit = 1u64 << team;
        let restricted = selected & bit != 0;
        let strict = mask & bit != 0;
        let (ahead, behind) = if !restricted {
            (equal + greater, less)
        } else if above {
            if strict {
                (greater, 0.0)
            } else {
                (equal, less)
            }
        } else if strict {
            (0.0, less)
        } else {
            (equal + greater, 0.0)
        };
        let la = log_probability(ahead);
        let lb = log_probability(behind);
        for k in (0..=(seen + 1).min(rank)).rev() {
            let no = if k < dp.len() {
                dp[k] + lb
            } else {
                f64::NEG_INFINITY
            };
            let yes = if k > 0 {
                dp[k - 1] + la
            } else {
                f64::NEG_INFINITY
            };
            if k < dp.len() {
                dp[k] = log_add(no, yes);
            }
        }
        seen += 1;
    }
    Some(dp[rank])
}

pub(super) fn normalized_log_shares(log_scores: &[f64]) -> Vec<f64> {
    if log_scores.is_empty() {
        return Vec::new();
    }
    let max = log_scores
        .iter()
        .copied()
        .filter(|score| score.is_finite())
        .fold(f64::NEG_INFINITY, f64::max);
    if !max.is_finite() {
        return vec![1.0 / log_scores.len() as f64; log_scores.len()];
    }
    let scaled: Vec<_> = log_scores
        .iter()
        .map(|score| {
            if score.is_finite() {
                (*score - max).exp()
            } else {
                0.0
            }
        })
        .collect();
    let sum: f64 = scaled.iter().sum();
    if sum <= 0.0 || !sum.is_finite() {
        return vec![1.0 / log_scores.len() as f64; log_scores.len()];
    }
    scaled.into_iter().map(|score| score / sum).collect()
}

fn poisson_binomial_exact(q: &[f64], wanted: usize, work: &mut usize, limit: usize) -> Option<f64> {
    let mut dp = vec![0.0; wanted.saturating_add(1).min(q.len() + 1)];
    if wanted >= dp.len() {
        return Some(0.0);
    }
    dp[0] = 1.0;
    let mut seen = 0usize;
    for &p in q {
        if !increment(work, limit, dp.len()) {
            return None;
        }
        let p = p.clamp(0.0, 1.0);
        for k in (0..=(seen + 1).min(wanted)).rev() {
            let stay = dp[k] * (1.0 - p);
            dp[k] = stay + if k > 0 { dp[k - 1] * p } else { 0.0 };
        }
        seen += 1;
    }
    Some(dp[wanted])
}

/// Cheap optimistic packed-score ordering estimate; it never removes roots.
pub(super) fn screen(
    team: usize,
    rank: usize,
    base: &[i32],
    target_games: &[RankGame],
    remaining: &[RankGame],
    roots: &[Vec<u8>],
    limit: usize,
    collect_leaf_pilot: bool,
    leaf_pilot_limit: usize,
) -> Screen {
    let mut work = 0usize;
    let mut leaf_work = 0usize;
    let mut leaf_reason = None;
    let n = base.len();
    let mut pmfs = vec![vec![1.0f64]; n];
    for game in remaining {
        let max_h = *game.hg.iter().max().unwrap_or(&0).max(&0) as usize;
        let max_a = *game.ag.iter().max().unwrap_or(&0).max(&0) as usize;
        for (t, gains) in [(game.home, &game.hg), (game.away, &game.ag)] {
            let old = &pmfs[t];
            let span = old
                .len()
                .saturating_add(if t == game.home { max_h } else { max_a });
            let mut next = vec![0.0; span];
            for (i, mass) in old.iter().enumerate() {
                if !increment(&mut work, limit, 3) {
                    return fallback(roots.len(), work, "screen work cap", leaf_work, leaf_reason);
                }
                for o in 0..3 {
                    let index = i.saturating_add(gains[o] as usize);
                    if index < next.len() {
                        next[index] += mass * game.prob[o];
                    }
                }
            }
            pmfs[t] = next;
        }
    }
    // Reverse summation keeps rare tails intact; no subtraction from one.
    let mut tails = Vec::with_capacity(n);
    let mut prefixes = collect_leaf_pilot.then(|| Vec::with_capacity(n));
    for pmf in &pmfs {
        if !increment(&mut work, limit, pmf.len()) {
            return fallback(roots.len(), work, "screen work cap", leaf_work, leaf_reason);
        }
        let mut tail = vec![0.0; pmf.len()];
        let mut sum = 0.0;
        for i in (0..pmf.len()).rev() {
            sum += pmf[i];
            tail[i] = sum;
        }
        tails.push(tail);
        if let Some(prefixes) = prefixes.as_mut().filter(|_| leaf_reason.is_none()) {
            // Separate leaf-pilot fee; it cannot make the existing root screen
            // hit its cap or alter split ordering.
            if !increment(&mut leaf_work, leaf_pilot_limit, pmf.len()) {
                leaf_reason = Some("leaf pilot PMF prefix work cap");
                prefixes.clear();
                continue;
            }
            let mut prefix = vec![0.0; pmf.len()];
            let mut sum = 0.0;
            for (i, mass) in pmf.iter().enumerate() {
                sum += mass;
                prefix[i] = sum;
            }
            prefixes.push(prefix);
        }
    }
    let mut raw = Vec::with_capacity(roots.len());
    let mut priors = Vec::with_capacity(roots.len());
    let mut chances = Vec::with_capacity(roots.len());
    let mut leaf_comparators = Vec::with_capacity(roots.len());
    for root in roots {
        if !increment(&mut work, limit, target_games.len().saturating_add(1)) {
            return fallback(roots.len(), work, "screen work cap", leaf_work, leaf_reason);
        }
        let mut fixed = vec![0i32; n];
        let mut prior = 1.0;
        for (game, &outcome) in target_games.iter().zip(root) {
            let o = outcome as usize;
            prior *= game.prob[o];
            fixed[game.home] += game.hg[o];
            fixed[game.away] += game.ag[o];
        }
        let target = base[team] + fixed[team];
        let mut q = Vec::with_capacity(n.saturating_sub(1));
        let mut comparators = Vec::with_capacity(n.saturating_sub(1));
        for rival in 0..n {
            if rival == team {
                continue;
            }
            if !increment(&mut work, limit, 1) {
                return fallback(roots.len(), work, "screen work cap", leaf_work, leaf_reason);
            }
            let threshold = target - (base[rival] + fixed[rival]);
            let chance = if threshold <= 0 {
                1.0
            } else {
                let threshold = threshold as usize;
                tails[rival].get(threshold).copied().unwrap_or(0.0)
            };
            q.push(chance.clamp(0.0, 1.0));
            if collect_leaf_pilot && leaf_reason.is_none() {
                if !increment(&mut leaf_work, leaf_pilot_limit, 1) {
                    leaf_reason = Some("leaf pilot root comparator work cap");
                } else {
                    let less = if threshold <= 0 {
                        0.0
                    } else {
                        prefixes
                            .as_ref()
                            .and_then(|v| v.get(rival))
                            .map_or(0.0, |prefix| {
                                prefix
                                    .get((threshold as usize).saturating_sub(1))
                                    .copied()
                                    .or_else(|| prefix.last().copied())
                                    .unwrap_or(0.0)
                            })
                    };
                    let equal = if threshold < 0 {
                        0.0
                    } else {
                        pmfs[rival].get(threshold as usize).copied().unwrap_or(0.0)
                    };
                    let greater = if threshold < 0 {
                        tails[rival].first().copied().unwrap_or(1.0)
                    } else {
                        tails[rival]
                            .get((threshold as usize).saturating_add(1))
                            .copied()
                            .unwrap_or(0.0)
                    };
                    comparators.push([less, equal, greater]);
                }
            }
        }
        if collect_leaf_pilot && leaf_reason.is_none() {
            leaf_comparators.push(comparators);
        }
        let Some(count_chance) = poisson_binomial_exact(&q, rank, &mut work, limit) else {
            return fallback(roots.len(), work, "screen work cap", leaf_work, leaf_reason);
        };
        let score = prior * count_chance;
        priors.push(prior);
        chances.push(count_chance);
        raw.push(if score.is_finite() {
            score.max(0.0)
        } else {
            0.0
        });
    }
    if !increment(&mut work, limit, roots.len().saturating_mul(2)) {
        return fallback(roots.len(), work, "screen work cap", leaf_work, leaf_reason);
    }
    let share = normalized(&raw);
    let rows = roots
        .iter()
        .enumerate()
        .map(|(i, _root)| RootShare {
            prior: priors[i],
            count_chance: chances[i],
            raw_score: raw[i],
            normalized_share: share[i],
        })
        .collect();
    let leaf_comparators = if collect_leaf_pilot && leaf_reason.is_none() {
        Some(leaf_comparators)
    } else {
        None
    };
    Screen {
        roots: rows,
        work,
        reason: None,
        leaf_comparators,
        leaf_work,
        leaf_reason,
    }
}

fn fallback(
    _n: usize,
    work: usize,
    reason: &'static str,
    leaf_work: usize,
    leaf_reason: Option<&'static str>,
) -> Screen {
    Screen {
        roots: Vec::new(),
        work,
        reason: Some(reason),
        leaf_comparators: None,
        leaf_work,
        leaf_reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_have_uniform_fallback_and_retained_subset_normalizes() {
        assert_eq!(mix_shares(&[0.0, 0.0], 0.10), vec![0.5, 0.5]);
        let raw = [0.9, 0.1, 0.0];
        let shares = mix_shares(&[raw[0], raw[2]], 0.10);
        assert!((shares[0] - 0.95).abs() < 1e-12);
        assert!((shares[1] - 0.05).abs() < 1e-12);
    }

    #[test]
    fn exact_count_mass_and_favorable_ties() {
        let mut work = 0;
        assert!(
            (poisson_binomial_exact(&[0.5, 0.5], 1, &mut work, 100).unwrap() - 0.5).abs() < 1e-12
        );
        let games = [RankGame {
            index: 0,
            home: 1,
            away: 2,
            prob: [0.3, 0.4, 0.3],
            hg: [0, 1, 3],
            ag: [3, 1, 0],
        }];
        let roots = [vec![]];
        let s = screen(0, 1, &[0, 0, -1], &[], &games, &roots, 1000, false, 0);
        assert!(s.reason.is_none());
        // Rival 1 can tie at zero and therefore always compares >= target;
        // rival 2 reaches the target only on draw/win (0.7), so P(X=1)=0.3.
        assert!((s.roots[0].count_chance - 0.3).abs() < 1e-12);

        let win_game = [RankGame {
            index: 0,
            home: 0,
            away: 1,
            prob: [0.2, 0.3, 0.5],
            hg: [0, 1, 4],
            ag: [4, 1, 0],
        }];
        let win_roots = [vec![0], vec![2]];
        let wins = screen(0, 0, &[0, 0], &win_game, &[], &win_roots, 1000, false, 0);
        assert_eq!(wins.roots[0].count_chance, 0.0);
        assert_eq!(wins.roots[1].count_chance, 1.0);
        assert!((wins.roots[1].raw_score - 0.5).abs() < 1e-12);
        let rare_game = [RankGame {
            index: 0,
            home: 1,
            away: 2,
            prob: [1.0 - 1e-20, 0.0, 1e-20],
            hg: [0, 0, 3],
            ag: [3, 0, 0],
        }];
        let rare = screen(
            0,
            1,
            &[1, 0, -100],
            &[],
            &rare_game,
            &[vec![]],
            1000,
            false,
            0,
        );
        assert!((rare.roots[0].count_chance - 1e-20).abs() < 1e-30);
    }

    #[test]
    fn screen_cap_returns_bounded_fallback() {
        let games = [RankGame {
            index: 0,
            home: 0,
            away: 1,
            prob: [0.3, 0.4, 0.3],
            hg: [0, 1, 3],
            ag: [3, 1, 0],
        }];
        let s = screen(0, 0, &[0, 0], &[], &games, &[vec![]], 1, false, 0);
        assert_eq!(s.reason, Some("screen work cap"));
        assert!(s.work <= 1);
    }

    #[test]
    fn leaf_constraint_mass_respects_ties_and_both_orientations() {
        let cmp = [[0.2, 0.3, 0.5]];
        let score = |above, mask, selected| {
            let mut work = 0;
            leaf_log_mass(&cmp, 1, above, mask, selected, 0, &mut work, 100)
                .unwrap()
                .exp()
        };
        // team 1 is selected; its tie belongs to the non-strict side.
        assert!((score(true, 0, 1 << 1) - 0.3).abs() < 1e-12);
        assert!((score(true, 1 << 1, 1 << 1) - 0.5).abs() < 1e-12);
        assert!((score(false, 0, 1 << 1) - 0.8).abs() < 1e-12);
        assert!(score(false, 1 << 1, 1 << 1) < 1e-12);
        // Unselected rivals have the same exact-rank factor in either sibling.
        let mut work = 0;
        let unselected = leaf_log_mass(&cmp, 1, true, 0, 0, 0, &mut work, 100)
            .unwrap()
            .exp();
        assert!((unselected - 0.8).abs() < 1e-12);
    }

    #[test]
    fn screen_comparator_tails_keep_boundary_and_rare_masses() {
        let game = [RankGame {
            index: 0,
            home: 1,
            away: 2,
            prob: [0.2, 0.3, 0.5],
            hg: [0, 1, 3],
            ag: [3, 1, 0],
        }];
        let get = |rival_base| {
            screen(
                0,
                1,
                &[0, rival_base, 0],
                &[],
                &game,
                &[vec![]],
                1000,
                true,
                1000,
            )
            .leaf_comparators
            .unwrap()[0][0]
        };
        assert_eq!(get(1), [0.0, 0.0, 1.0]);
        assert_eq!(get(0), [0.0, 0.2, 0.8]);
        assert_eq!(get(-1), [0.2, 0.3, 0.5]);
        assert_eq!(get(-3), [0.5, 0.5, 0.0]);
        assert_eq!(get(-4), [1.0, 0.0, 0.0]);

        let rare = [RankGame {
            index: 1,
            home: 1,
            away: 2,
            prob: [1.0 - 1e-20, 0.0, 1e-20],
            hg: [0, 0, 3],
            ag: [3, 0, 0],
        }];
        let tail = screen(0, 1, &[1, 0, -100], &[], &rare, &[vec![]], 1000, true, 1000)
            .leaf_comparators
            .unwrap()[0][0][2];
        assert!((tail - 1e-20).abs() < 1e-30);
    }

    #[test]
    fn leaf_pilot_cap_falls_back_without_changing_root_screen() {
        let game = [RankGame {
            index: 0,
            home: 1,
            away: 2,
            prob: [0.2, 0.3, 0.5],
            hg: [0, 1, 3],
            ag: [3, 1, 0],
        }];
        let roots = [vec![]];
        let baseline = screen(0, 1, &[0, 0, 0], &[], &game, &roots, 1000, false, 0);
        let capped = screen(0, 1, &[0, 0, 0], &[], &game, &roots, 1000, true, 0);
        assert_eq!(baseline.work, capped.work);
        assert_eq!(baseline.roots[0].raw_score, capped.roots[0].raw_score);
        assert_eq!(
            baseline.roots[0].normalized_share,
            capped.roots[0].normalized_share
        );
        assert_eq!(capped.reason, None);
        assert_eq!(capped.leaf_reason, Some("leaf pilot PMF prefix work cap"));
        assert!(capped.leaf_comparators.is_none());

        let late_cap = screen(0, 1, &[0, 0, 0], &[], &game, &roots, 12, true, 1000);
        assert_eq!(late_cap.reason, Some("screen work cap"));
        assert!(late_cap.leaf_work > 0);
    }
}
