//! A run's metrics, a route's runs summed, and the score: code computes every part of it.

use std::collections::BTreeSet;

use resume_core::Requirements;
use resume_core::change::Mode;
use resume_core::suggestions::Suggestions;
use resume_llm::cover::Letter;
use resume_llm::prep::{Prep, SentResume};
use resume_pipeline::template;
use resume_render::{FontOptions, Renderer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tailor::{LeftOut, master_of, review};
use crate::{BenchError, Fixtures, Outcome, Role, coach, overlap, target_s, words};

/// What one valid run's output comes to, by role.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum RunMetrics {
    /// Against gold; `None` where gold has nothing to measure (no must-have unit, say).
    Read {
        recall: Option<f64>,
        precision: Option<f64>,
        must_recall: Option<f64>,
    },
    /// The review of its suggestions: changes proposed, those with grounding flags, those the fit check left out
    /// (lines or page), and the score before and after.
    Tailor {
        changes: u32,
        flagged: u32,
        left_fit: u32,
        now: f64,
        projected: f64,
    },
    Write {
        words: u32,
    },
    /// Claims tied to the resume that `prep::checked` kept and dropped (see [`coach::grounding`]).
    Coach {
        kept_items: u32,
        dropped_items: u32,
    },
}

/// Measures one run's `output` of `role` on posting `item`, as the app would take it in; no call, no filesystem.
pub fn metrics(
    role: Role,
    fx: &Fixtures,
    output: &Value,
    item: &str,
) -> Result<RunMetrics, BenchError> {
    let (_, req) = fx.item(item)?;
    Ok(match role {
        Role::Read => {
            let run = Requirements::deserialize(output)?;
            let o = overlap(&req.skills, &run.skills);
            RunMetrics::Read {
                recall: o.recall,
                precision: o.precision,
                must_recall: o.must_recall,
            }
        }
        Role::Tailor => {
            let s = Suggestions::deserialize(output)?;
            let mode = Mode::BankAware;
            let mut renderer =
                Renderer::new(&template(master_of(fx, mode))?, FontOptions::default());
            let r = review(fx, mode, req, &s, &mut renderer)?;
            let count = |f: &dyn Fn(&resume_core::change::Change) -> bool| {
                r.changes.iter().filter(|c| f(c)).count() as u32
            };
            RunMetrics::Tailor {
                changes: r.changes.len() as u32,
                flagged: count(&|c| !c.flags.is_empty()),
                left_fit: count(&|c| matches!(r.left_out(c), Some(LeftOut::Lines | LeftOut::Page))),
                now: r.tailored.now.total,
                projected: r.tailored.projected.total,
            }
        }
        Role::Write => RunMetrics::Write {
            words: words(&Letter::deserialize(output)?) as u32,
        },
        Role::Coach => {
            let sent = SentResume::tagged(&fx.resume, &fx.bank);
            let (p, d) = coach::checked(&sent, req, Prep::deserialize(output)?);
            let (kept_items, dropped_items) = coach::grounding(&p, &d);
            RunMetrics::Coach {
                kept_items,
                dropped_items,
            }
        }
    })
}

/// What [`aggregate`] reads of a run: what the app keeps of it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunSummary {
    pub item: String,
    pub outcome: Outcome,
    pub latency_ms: u64,
    pub cost_usd: Option<f64>,
    pub metrics: Option<RunMetrics>,
}

/// One route's runs of a role, summed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouteResult {
    pub runs: u32,
    /// Valid on the first turn.
    pub valid: u32,
    /// Valid after the repair turn.
    pub repaired: u32,
    pub failed: u32,
    /// The postings the runs covered.
    pub items: u32,
    /// Nearest-rank percentiles of every run's wall time, failed runs included.
    pub p50_s: Option<f64>,
    pub p90_s: Option<f64>,
    /// Mean cost of the runs whose cost is known.
    pub cost_usd: Option<f64>,
    pub quality: Quality,
}

/// A route's quality and what it's made of.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Quality {
    /// 0–100; for Tailor the code checks alone (kept edits apply in [`score`]); `None` for Write, or when no valid run
    /// could be measured.
    pub value: Option<f64>,
    /// Read: `recall`, `precision`, `must_recall` (shares 0–1). Tailor: `grounded`, `fits`, `lift` (shares 0–1),
    /// `lift_pts` (mean projected − now), and the sums `changes`, `flagged`, `left_fit`. Coach: `grounded` (share 0–1),
    /// and the sums `kept_items`, `dropped_items`.
    pub parts: Vec<Part>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub id: String,
    pub value: f64,
}

/// The lift that counts in full: 20 points of projected over now.
const FULL_LIFT: f64 = 20.0;

/// `runs` of one route on `role`, summed.
pub fn aggregate(role: Role, runs: &[RunSummary]) -> RouteResult {
    let count = |o| runs.iter().filter(|r| r.outcome == o).count() as u32;
    let lat: Vec<f64> = runs.iter().map(|r| r.latency_ms as f64 / 1000.0).collect();
    let measured: Vec<&RunMetrics> = runs
        .iter()
        .filter(|r| r.outcome != Outcome::Failed)
        .filter_map(|r| r.metrics.as_ref())
        .collect();
    RouteResult {
        runs: runs.len() as u32,
        valid: count(Outcome::Valid),
        repaired: count(Outcome::Repaired),
        failed: count(Outcome::Failed),
        items: runs
            .iter()
            .map(|r| r.item.as_str())
            .collect::<BTreeSet<_>>()
            .len() as u32,
        p50_s: percentile(&lat, 0.5),
        p90_s: percentile(&lat, 0.9),
        cost_usd: mean(runs.iter().filter_map(|r| r.cost_usd)),
        quality: quality(role, &measured),
    }
}

fn quality(role: Role, ms: &[&RunMetrics]) -> Quality {
    let part = |id: &str, value: f64| Part {
        id: id.to_owned(),
        value,
    };
    match role {
        Role::Read => {
            let of = |f: fn(&RunMetrics) -> Option<f64>| mean(ms.iter().filter_map(|m| f(m)));
            let recall = of(|m| match m {
                RunMetrics::Read { recall, .. } => *recall,
                _ => None,
            });
            let precision = of(|m| match m {
                RunMetrics::Read { precision, .. } => *precision,
                _ => None,
            });
            let must_recall = of(|m| match m {
                RunMetrics::Read { must_recall, .. } => *must_recall,
                _ => None,
            });
            let parts: Vec<Part> = [
                ("recall", recall),
                ("precision", precision),
                ("must_recall", must_recall),
            ]
            .into_iter()
            .filter_map(|(id, v)| Some(part(id, v?)))
            .collect();
            let value = match (recall, precision, must_recall) {
                (Some(r), Some(p), Some(m)) => Some(100.0 * (r + p + m) / 3.0),
                _ => None,
            };
            Quality { value, parts }
        }
        Role::Tailor => {
            let mut sums = (0u32, 0u32, 0u32);
            let mut lifts = Vec::new();
            for m in ms {
                if let RunMetrics::Tailor {
                    changes,
                    flagged,
                    left_fit,
                    now,
                    projected,
                } = m
                {
                    sums.0 += changes;
                    sums.1 += flagged;
                    sums.2 += left_fit;
                    lifts.push(projected - now);
                }
            }
            let Some(lift_pts) = mean(lifts) else {
                return Quality::default();
            };
            let (changes, flagged, left_fit) = sums;
            // Nothing proposed: nothing ungrounded, nothing that didn't fit.
            let share_ok = |bad: u32| {
                if changes == 0 {
                    1.0
                } else {
                    1.0 - f64::from(bad) / f64::from(changes)
                }
            };
            let grounded = share_ok(flagged);
            let fits = share_ok(left_fit);
            let lift = (lift_pts / FULL_LIFT).clamp(0.0, 1.0);
            Quality {
                value: Some(100.0 * (grounded + fits + lift) / 3.0),
                parts: vec![
                    part("grounded", grounded),
                    part("fits", fits),
                    part("lift", lift),
                    part("lift_pts", lift_pts),
                    part("changes", f64::from(changes)),
                    part("flagged", f64::from(flagged)),
                    part("left_fit", f64::from(left_fit)),
                ],
            }
        }
        Role::Write => Quality::default(),
        Role::Coach => {
            let (kept, dropped) = ms.iter().fold((0u32, 0u32), |(k, d), m| match m {
                RunMetrics::Coach {
                    kept_items,
                    dropped_items,
                } => (k + kept_items, d + dropped_items),
                _ => (k, d),
            });
            if kept + dropped == 0 {
                return Quality::default();
            }
            let grounded = f64::from(kept) / f64::from(kept + dropped);
            Quality {
                value: Some(100.0 * grounded),
                parts: vec![
                    part("grounded", grounded),
                    part("kept_items", f64::from(kept)),
                    part("dropped_items", f64::from(dropped)),
                ],
            }
        }
    }
}

/// A route's score on a role, every part 0–100.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Score {
    /// 0.6 × quality + 0.2 × reliability + 0.2 × speed; `None` without a quality.
    pub composite: Option<f64>,
    /// Tailor: the code checks scaled by kept edits.
    pub quality: Option<f64>,
    pub reliability: f64,
    pub speed: f64,
}

/// `r` scored on `role`. `kept_factor` ([`kept_factor`]) scales Tailor's code checks by
/// 0.75 + 0.25 × factor, to at most 100; `None` leaves them as they are. Other roles ignore it.
pub fn score(role: Role, r: &RouteResult, kept_factor: Option<f64>) -> Score {
    let reliability = if r.runs == 0 {
        0.0
    } else {
        100.0 * (f64::from(r.valid) + 0.5 * f64::from(r.repaired)) / f64::from(r.runs)
    };
    let speed = r.p50_s.map_or(0.0, |p50| speed(p50, target_s(role)));
    let quality = r.quality.value.map(|q| match (role, kept_factor) {
        (Role::Tailor, Some(f)) => (q * (0.75 + 0.25 * f)).min(100.0),
        _ => q,
    });
    Score {
        composite: quality.map(|q| 0.6 * q + 0.2 * reliability + 0.2 * speed),
        quality,
        reliability,
        speed,
    }
}

/// 100 up to `target` seconds, then down in a line to 0 at four times it.
fn speed(p50: f64, target: f64) -> f64 {
    if p50 <= target {
        100.0
    } else {
        (100.0 * (4.0 * target - p50) / (3.0 * target)).max(0.0)
    }
}

/// A route's suggested changes you decided on at export: kept (left in or forced in) of decided (in, off or forced).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    pub kept: u32,
    pub decided: u32,
}

/// Decisions' worth of your average a route's kept share starts from.
const KEPT_PRIOR: f64 = 10.0;

/// How `route`'s kept share compares with your average over `all` routes (this one among them): 1 at the average,
/// above when you keep more. A route with few decisions sits near 1: its share is pulled toward the average by
/// [`KEPT_PRIOR`] decisions' worth of it. `None` until you've decided on any change.
pub fn kept_factor(route: Kept, all: &[Kept]) -> Option<f64> {
    let (kept, decided) = all
        .iter()
        .fold((0u32, 0u32), |(k, d), x| (k + x.kept, d + x.decided));
    if decided == 0 {
        return None;
    }
    let avg = f64::from(kept) / f64::from(decided);
    if avg == 0.0 {
        // You kept nothing anywhere, so every route is at the average.
        return Some(1.0);
    }
    let counted =
        (f64::from(route.kept) + KEPT_PRIOR * avg) / (f64::from(route.decided) + KEPT_PRIOR);
    Some(counted / avg)
}

/// `rows` (`(id, score, p50 seconds)`) in display order, each with its rank from 1: by the unrounded composite,
/// highest first, equal composites sharing a rank (the next one skips). Routes without a composite follow unranked,
/// fastest p50 first, those with no p50 last; otherwise the given order holds.
pub fn rank(rows: &[(usize, Score, Option<f64>)]) -> Vec<(usize, Option<u32>)> {
    let mut sorted: Vec<&(usize, Score, Option<f64>)> = rows.iter().collect();
    sorted.sort_by(|a, b| match (a.1.composite, b.1.composite) {
        (Some(x), Some(y)) => y.total_cmp(&x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => match (a.2, b.2) {
            (Some(x), Some(y)) => x.total_cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        },
    });
    let mut out = Vec::with_capacity(sorted.len());
    let mut prev: Option<(f64, u32)> = None;
    for (i, (id, s, _)) in sorted.into_iter().enumerate() {
        let rank = s.composite.map(|c| match prev {
            Some((p, r)) if p == c => r,
            _ => i as u32 + 1,
        });
        if let (Some(c), Some(r)) = (s.composite, rank) {
            prev = Some((c, r));
        }
        out.push((*id, rank));
    }
    out
}

/// Nearest-rank percentile `p` (0–1) of `xs`; `None` when empty.
pub fn percentile(xs: &[f64], p: f64) -> Option<f64> {
    let mut xs = xs.to_vec();
    xs.sort_by(f64::total_cmp);
    let rank = ((p * xs.len() as f64).ceil() as usize).clamp(1, xs.len().max(1));
    xs.get(rank - 1).copied()
}

pub fn mean(xs: impl IntoIterator<Item = f64>) -> Option<f64> {
    let (sum, n) = xs
        .into_iter()
        .fold((0.0, 0usize), |(s, n), x| (s + x, n + 1));
    (n > 0).then(|| sum / n as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(
        role_quality: Option<f64>,
        runs: u32,
        valid: u32,
        repaired: u32,
        p50: f64,
    ) -> RouteResult {
        RouteResult {
            runs,
            valid,
            repaired,
            failed: runs - valid - repaired,
            items: runs,
            p50_s: Some(p50),
            p90_s: Some(p50),
            cost_usd: None,
            quality: Quality {
                value: role_quality,
                parts: Vec::new(),
            },
        }
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} ≠ {b}");
    }

    #[test]
    fn speed_is_full_to_the_target_then_falls_to_zero_at_four_times_it() {
        let t = target_s(Role::Tailor);
        let at = |p50| score(Role::Tailor, &route(Some(80.0), 1, 1, 0, p50), None).speed;
        close(at(t * 0.5), 100.0);
        close(at(t), 100.0);
        close(at(t * 2.5), 50.0);
        close(at(t * 4.0), 0.0);
        close(at(t * 9.0), 0.0);
    }

    #[test]
    fn reliability_halves_repaired_and_zeroes_failed() {
        // 6 valid, 2 repaired, 2 failed of 10: (6 + 1) ÷ 10.
        let s = score(Role::Read, &route(Some(90.0), 10, 6, 2, 1.0), None);
        close(s.reliability, 70.0);
        let none = RouteResult {
            runs: 0,
            failed: 0,
            p50_s: None,
            ..route(Some(90.0), 0, 0, 0, 1.0)
        };
        close(score(Role::Read, &none, None).reliability, 0.0);
    }

    #[test]
    fn composite_weighs_quality_and_is_none_without_it() {
        // Quality 80, reliability 100, speed 100.
        let s = score(Role::Coach, &route(Some(80.0), 2, 2, 0, 5.0), Some(2.0));
        close(s.composite.unwrap(), 88.0);
        let w = score(Role::Write, &route(None, 2, 2, 0, 5.0), None);
        assert_eq!(w.composite, None);
        assert_eq!(w.quality, None);
        close(w.reliability, 100.0);
    }

    #[test]
    fn kept_factor_is_one_at_the_average_and_without_decisions() {
        let all = [
            Kept {
                kept: 30,
                decided: 40,
            },
            Kept {
                kept: 15,
                decided: 20,
            },
            Kept {
                kept: 0,
                decided: 0,
            },
        ];
        close(kept_factor(all[0], &all).unwrap(), 1.0);
        close(kept_factor(all[1], &all).unwrap(), 1.0);
        // A route you never decided on counts as your average.
        close(kept_factor(all[2], &all).unwrap(), 1.0);
        assert_eq!(kept_factor(all[2], &[all[2]]), None);
        // No decisions anywhere: the code checks stand alone.
        let r = route(Some(80.0), 1, 1, 0, 1.0);
        assert_eq!(score(Role::Tailor, &r, None).quality, Some(80.0));
    }

    #[test]
    fn few_decisions_sit_near_the_average() {
        // Average 0.5. Kept 2 of 2: (2 + 5) ÷ 12 ÷ 0.5; kept 50 of 50: (50 + 5) ÷ 60 ÷ 0.5.
        let all = [
            Kept {
                kept: 2,
                decided: 2,
            },
            Kept {
                kept: 50,
                decided: 50,
            },
            Kept {
                kept: 0,
                decided: 52,
            },
        ];
        let few = kept_factor(all[0], &all).unwrap();
        let many = kept_factor(all[1], &all).unwrap();
        close(few, 7.0 / 12.0 / 0.5);
        close(many, 55.0 / 60.0 / 0.5);
        assert!(1.0 < few && few < many);
        let none = kept_factor(all[2], &all).unwrap();
        close(none, 5.0 / 62.0 / 0.5);
    }

    #[test]
    fn kept_quality_is_capped_at_100() {
        let r = route(Some(95.0), 1, 1, 0, 1.0);
        // ×(0.75 + 0.25 × 1.8) = ×1.2 would be 114.
        assert_eq!(score(Role::Tailor, &r, Some(1.8)).quality, Some(100.0));
        close(
            score(Role::Tailor, &r, Some(0.0)).quality.unwrap(),
            95.0 * 0.75,
        );
        // Only Tailor counts kept edits.
        assert_eq!(score(Role::Read, &r, Some(0.0)).quality, Some(95.0));
    }

    #[test]
    fn rank_orders_by_the_unrounded_composite_and_shares_exact_ties() {
        let s = |c: Option<f64>| Score {
            composite: c,
            quality: c,
            reliability: 100.0,
            speed: 100.0,
        };
        let rows = [
            (0, s(Some(86.6)), Some(10.0)),
            (1, s(None), Some(30.0)),
            (2, s(Some(87.4)), Some(10.0)),
            (3, s(Some(86.6)), Some(5.0)),
            (4, s(None), None),
            (5, s(None), Some(3.0)),
            (6, s(Some(80.0)), Some(1.0)),
        ];
        // 87.4 and 86.6 both show 87: 87.4 still ranks first. The two exact 86.6s share 2nd; 80 is 4th.
        assert_eq!(
            rank(&rows),
            [
                (2, Some(1)),
                (0, Some(2)),
                (3, Some(2)),
                (6, Some(4)),
                (5, None),
                (1, None),
                (4, None),
            ]
        );
    }
}
