//! The Read role: Extract on every posting (§3.2). Quality is measured against the gold requirements: term recall and
//! precision, the must-have subset's recall, exact fields, and how far the run's requirements move the `swe` master's
//! score from gold's. The Extract spike's checks (literal terms, priority-1 share, soft and person-describing terms)
//! are kept.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

use resume_core::matcher::{Page, find_literal};
use resume_core::requirements::Skill;
use resume_core::{PageFacts, Requirements, Resume, score};
use resume_llm::extract;
use resume_pipeline::{master_page, today};
use serde::Deserialize;
use serde_json::{Value, json};

use super::report::{Bar, PerPosting, Section, by_route, common, common_metrics, m};
use super::{Fixtures, Run, RunArgs, call, each, mean, stdev};

const TIMEOUT: Duration = Duration::from_secs(180);

pub async fn run(fx: &Fixtures, args: &RunArgs, out: &Path) -> anyhow::Result<()> {
    let schema = extract::schema();
    let items: Vec<(String, &resume_core::Posting)> = fx
        .chosen(&args.only)
        .into_iter()
        .map(|f| (f.id.clone(), &f.posting))
        .collect();
    each(
        "read",
        args,
        3,
        TIMEOUT,
        out,
        &items,
        async |llm, route, posting| {
            let c = call(extract::extract(llm, &schema, posting, &route.model)).await;
            let meta = json!({ "input_chars": extract::input(posting).chars().count() });
            (c, meta)
        },
    )
    .await
}

// ── Against gold ────────────────────────────────────────────────────────────────────────────────────────────────────

/// Whether two requirement skills name the same thing: either's term or one of its aliases (its own and the alias
/// dictionary's) appears as a whole term in the other's term or aliases, as `resume_core::matcher` finds terms on a
/// page. So `Python` matches `Python 3`, and `JS` matches `JavaScript` through the dictionary.
pub fn same_skill(a: &Skill, b: &Skill) -> bool {
    let page = |s: &Skill| {
        Page::new(
            std::iter::once(s.term.as_str())
                .chain(s.aliases.iter().map(String::as_str))
                .map(|t| (None, t)),
        )
    };
    find_literal(&page(b), &a.term, &a.aliases).is_some()
        || find_literal(&page(a), &b.term, &b.aliases).is_some()
}

/// A run's skills against gold's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overlap {
    /// Gold units found among the run's skills, of all gold units. A one-of group is one unit, found when any of its
    /// members is.
    pub recall: Option<f64>,
    /// The run's skills that match some gold skill (of any unit), of all the run's skills.
    pub precision: Option<f64>,
    /// Recall over the must-have units: those with a member that is priority 1 or required.
    pub must_recall: Option<f64>,
}

pub fn overlap(gold: &[Skill], run: &[Skill]) -> Overlap {
    let mut units: Vec<Vec<&Skill>> = Vec::new();
    let mut groups: BTreeMap<&str, usize> = BTreeMap::new();
    for g in gold {
        match g.group.as_deref() {
            Some(id) => match groups.get(id) {
                Some(&i) => units[i].push(g),
                None => {
                    groups.insert(id, units.len());
                    units.push(vec![g]);
                }
            },
            None => units.push(vec![g]),
        }
    }
    let found = |u: &[&Skill]| u.iter().any(|g| run.iter().any(|r| same_skill(g, r)));
    let share = |hit: usize, of: usize| (of > 0).then(|| hit as f64 / of as f64);
    let must: Vec<&Vec<&Skill>> = units
        .iter()
        .filter(|u| u.iter().any(|g| g.priority == 1 || g.required))
        .collect();
    Overlap {
        recall: share(units.iter().filter(|u| found(u)).count(), units.len()),
        precision: share(
            run.iter()
                .filter(|r| gold.iter().any(|g| same_skill(g, r)))
                .count(),
            run.len(),
        ),
        must_recall: share(must.iter().filter(|u| found(u)).count(), must.len()),
    }
}

/// The `now` score of `resume` against `run`'s requirements minus against `gold`'s: positive when the route's reading
/// makes the master look better than it is.
pub fn drift(
    resume: &Resume,
    facts: &PageFacts,
    run: &Requirements,
    gold: &Requirements,
    today: (u32, u32),
) -> f64 {
    score(resume, run, facts, today).total - score(resume, gold, facts, today).total
}

/// Knockout kinds, sorted: the field compared, since the text is free.
fn knockout_kinds(r: &Requirements) -> Vec<String> {
    let mut k: Vec<String> = r
        .knockouts
        .iter()
        .map(|k| format!("{:?}", k.kind))
        .collect();
    k.sort();
    k
}

// ── The spike's checks ──────────────────────────────────────────────────────────────────────────────────────────────

/// Words that make a skill term describe a person rather than name a skill.
const PERSON_NOUNS: [&str; 14] = [
    "communicator",
    "starter",
    "player",
    "solver",
    "learner",
    "technologist",
    "thinker",
    "go-getter",
    "self-starter",
    "individual",
    "candidate",
    "person",
    "achiever",
    "leader",
];
const PERSON_ADJECTIVES: [&str; 12] = [
    "strong",
    "pragmatic",
    "skilled",
    "passionate",
    "motivated",
    "self-motivated",
    "driven",
    "excellent",
    "proactive",
    "curious",
    "eager",
    "detail-oriented",
];

/// What the Extract spike measured in one output.
struct Spike {
    skills: usize,
    /// Terms not found case-insensitively in the posting text.
    non_literal: usize,
    p1: usize,
    /// Priority 1 but not required.
    p1_optional: usize,
    soft: Vec<String>,
    /// A soft skill listed before a hard, tool or domain one.
    soft_early: bool,
    person: Vec<String>,
    groups: usize,
    knockouts: usize,
}

fn spike(text: &str, v: &Value) -> Spike {
    let text = text.to_lowercase();
    let skills = v["skills"].as_array().cloned().unwrap_or_default();
    let term = |k: &Value| k["term"].as_str().unwrap_or("").to_owned();
    let is_soft = |k: &Value| k["kind"] == "soft";
    let first_soft = skills.iter().position(is_soft);
    let last_hard = skills.iter().rposition(|k| !is_soft(k));
    let mut groups: Vec<&str> = skills.iter().filter_map(|k| k["group"].as_str()).collect();
    groups.sort();
    groups.dedup();
    Spike {
        skills: skills.len(),
        non_literal: skills
            .iter()
            .filter(|k| !text.contains(&term(k).to_lowercase()))
            .count(),
        p1: skills.iter().filter(|k| k["priority"] == 1).count(),
        p1_optional: skills
            .iter()
            .filter(|k| k["priority"] == 1 && k["required"] != true)
            .count(),
        soft: skills.iter().filter(|k| is_soft(k)).map(term).collect(),
        soft_early: matches!((first_soft, last_hard), (Some(f), Some(l)) if f < l),
        person: skills
            .iter()
            .map(term)
            .filter(|t| describes_person(t))
            .collect(),
        groups: groups.len(),
        knockouts: v["knockouts"].as_array().map_or(0, Vec::len),
    }
}

/// Whether a skill term describes the candidate ("Strong communicators", "Pragmatic self-starter").
fn describes_person(term: &str) -> bool {
    let t = term.to_lowercase();
    let words: Vec<&str> = t.split_whitespace().collect();
    let noun = |w: &str| {
        PERSON_NOUNS
            .iter()
            .any(|n| w == *n || w.strip_suffix('s') == Some(n))
    };
    words.iter().any(|w| noun(w)) || words.first().is_some_and(|w| PERSON_ADJECTIVES.contains(w))
}

// ── Report ──────────────────────────────────────────────────────────────────────────────────────────────────────────

/// One valid run, measured.
struct Measured<'a> {
    run: &'a Run,
    spike: Spike,
    overlap: Option<Overlap>,
    now: Option<f64>,
    drift: Option<f64>,
    /// Knockout kinds, seniority, years, degree: equal to gold's.
    fields: Option<[bool; 4]>,
}

pub fn section(fx: &Fixtures, runs: &[Run]) -> anyhow::Result<Vec<Section>> {
    let (resume, facts) = master_page(&fx.bank, &fx.master)?;
    let today = today();
    let mut sections = Section::new(
        "read",
        "Read: Extract",
        format!(
            "Gold metrics compare each valid run with `{}/<posting>.json` ({} of {} postings have one; `–` where none \
             does). Terms match as `resume_core::matcher` finds them (whole terms, the term's aliases and the alias \
             dictionary), either way round; a one-of group is one unit, recalled when any member is. Must-have recall \
             counts the units with a priority-1 or required member. Score drift: the `swe` master's `now` with the \
             run's requirements minus with gold's (positive: the route flatters the master); `now` spread is the \
             stdev across a posting's runs, averaged over postings. Knockouts compare their kinds; degree its level \
             and `equivalent`. Literal-term rate, priority-1 share, soft and person-describing terms are the Extract \
             spike's checks.\n",
            super::GOLD,
            fx.gold.len(),
            fx.postings.len()
        ),
    );
    let mut metrics = common_metrics(Some(0.95), Some(8.0), Some(12.0));
    metrics.extend([
        m("gold_runs", "Runs scored against gold", 0, None),
        m("recall", "Term recall", 2, Some(Bar::AtLeast(0.90))),
        m("precision", "Term precision", 2, Some(Bar::AtLeast(0.85))),
        m(
            "must_recall",
            "Recall, priority 1 + required",
            2,
            Some(Bar::AtLeast(0.95)),
        ),
        m("knockouts_match", "Knockout kinds = gold", 2, None),
        m("seniority_match", "Seniority = gold", 2, None),
        m("years_match", "Years = gold", 2, None),
        m("degree_match", "Degree = gold", 2, None),
        m(
            "drift_abs",
            "Score drift, mean |Δ|",
            1,
            Some(Bar::AtMost(3.0)),
        ),
        m("drift", "Score drift, mean Δ", 1, None),
        m("now", "Mean `now` of the swe master", 1, None),
        m("now_stdev", "`now` spread across runs (stdev)", 2, None),
        m(
            "p1_share",
            "Priority-1 share, mean",
            2,
            Some(Bar::AtMost(0.33)),
        ),
        m("p1_share_max", "Priority-1 share, worst run", 2, None),
        m("literal_rate", "Literal-term rate", 3, None),
        m("skills", "Mean skills", 1, None),
        m("p1_optional", "Priority 1 but not required, mean", 1, None),
        m("soft", "Soft skills, mean", 1, None),
        m(
            "soft_early",
            "Runs with a soft skill before a technical one",
            0,
            None,
        ),
        m("person", "Person-describing terms", 0, None),
        m("groups", "One-of groups, mean", 1, None),
        m("knockouts", "Knockouts, mean", 1, None),
    ]);
    sections.metrics = metrics;

    let mut recall_pp = PerPosting::new("Term recall vs gold", 2);
    let mut now_pp = PerPosting::new("`now` of the swe master", 1);
    let mut lat_pp = PerPosting::new("Latency s", 1);
    let mut review = String::new();
    for (route, rs) in by_route(runs) {
        let mut n = common(&rs);
        let ms: Vec<Measured> = rs
            .iter()
            .filter(|r| r.ok())
            .filter_map(|r| {
                let out = r.output.as_ref()?;
                let text = fx
                    .posting(&r.posting)
                    .map_or("", |f| f.posting.text.as_str());
                let req = Requirements::deserialize(out).ok();
                let gold = fx.gold.get(&r.posting);
                let now = req.as_ref().map(|q| score(&resume, q, &facts, today).total);
                Some(Measured {
                    run: r,
                    spike: spike(text, out),
                    overlap: req
                        .as_ref()
                        .zip(gold)
                        .map(|(q, g)| overlap(&g.skills, &q.skills)),
                    drift: req
                        .as_ref()
                        .zip(gold)
                        .map(|(q, g)| drift(&resume, &facts, q, g, today)),
                    now,
                    fields: req.as_ref().zip(gold).map(|(q, g)| {
                        [
                            knockout_kinds(q) == knockout_kinds(g),
                            q.seniority == g.seniority,
                            q.years_min == g.years_min,
                            q.degree.as_ref().map(|d| (d.level, d.equivalent))
                                == g.degree.as_ref().map(|d| (d.level, d.equivalent)),
                        ]
                    }),
                })
            })
            .collect();
        let mut put = |k: &'static str, v: Option<f64>| {
            if let Some(v) = v {
                n.insert(k, v);
            }
        };
        let gold: Vec<&Measured> = ms.iter().filter(|x| x.overlap.is_some()).collect();
        put("gold_runs", Some(gold.len() as f64));
        put(
            "recall",
            mean(gold.iter().filter_map(|x| x.overlap?.recall)),
        );
        put(
            "precision",
            mean(gold.iter().filter_map(|x| x.overlap?.precision)),
        );
        put(
            "must_recall",
            mean(gold.iter().filter_map(|x| x.overlap?.must_recall)),
        );
        for (i, k) in [
            "knockouts_match",
            "seniority_match",
            "years_match",
            "degree_match",
        ]
        .into_iter()
        .enumerate()
        {
            put(
                k,
                mean(
                    ms.iter()
                        .filter_map(|x| x.fields.map(|f| f64::from(u8::from(f[i])))),
                ),
            );
        }
        put(
            "drift_abs",
            mean(ms.iter().filter_map(|x| x.drift.map(f64::abs))),
        );
        put("drift", mean(ms.iter().filter_map(|x| x.drift)));
        put("now", mean(ms.iter().filter_map(|x| x.now)));
        let mut by_posting: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
        for x in &ms {
            if let Some(now) = x.now {
                by_posting.entry(&x.run.posting).or_default().push(now);
            }
        }
        put(
            "now_stdev",
            mean(by_posting.values().filter_map(|v| stdev(v))),
        );
        let share = |x: &Measured| x.spike.p1 as f64 / x.spike.skills.max(1) as f64;
        put("p1_share", mean(ms.iter().map(share)));
        put("p1_share_max", ms.iter().map(share).reduce(f64::max));
        let skills: usize = ms.iter().map(|x| x.spike.skills).sum();
        let non_literal: usize = ms.iter().map(|x| x.spike.non_literal).sum();
        put(
            "literal_rate",
            (skills > 0).then(|| 1.0 - non_literal as f64 / skills as f64),
        );
        put("skills", mean(ms.iter().map(|x| x.spike.skills as f64)));
        put(
            "p1_optional",
            mean(ms.iter().map(|x| x.spike.p1_optional as f64)),
        );
        put("soft", mean(ms.iter().map(|x| x.spike.soft.len() as f64)));
        put(
            "soft_early",
            Some(ms.iter().filter(|x| x.spike.soft_early).count() as f64),
        );
        put(
            "person",
            Some(ms.iter().map(|x| x.spike.person.len()).sum::<usize>() as f64),
        );
        put("groups", mean(ms.iter().map(|x| x.spike.groups as f64)));
        put(
            "knockouts",
            mean(ms.iter().map(|x| x.spike.knockouts as f64)),
        );

        for x in &ms {
            if let Some(r) = x.overlap.and_then(|o| o.recall) {
                recall_pp.push(&x.run.posting, &route, r);
            }
            if let Some(now) = x.now {
                now_pp.push(&x.run.posting, &route, now);
            }
        }
        for r in &rs {
            lat_pp.push(&r.posting, &route, r.secs());
        }

        let mut terms = String::new();
        for x in &ms {
            if x.spike.soft.is_empty() && x.spike.person.is_empty() {
                continue;
            }
            let _ = writeln!(
                terms,
                "- `{}` #{}: soft {}{}",
                x.run.posting,
                x.run.n,
                list(&x.spike.soft),
                if x.spike.person.is_empty() {
                    String::new()
                } else {
                    format!("; person-describing {}", list(&x.spike.person))
                }
            );
        }
        if !terms.is_empty() {
            let _ = write!(review, "\n**{route}**\n\n{terms}");
        }
        sections.routes.insert(route, n);
    }
    if !review.is_empty() {
        sections.extra = format!("\n### Soft and person-describing terms, for review\n{review}");
    }
    sections.extra.push_str(&super::report::errors(runs));
    sections.per_posting = vec![recall_pp, now_pp, lat_pp];
    Ok(vec![sections])
}

fn list(xs: &[String]) -> String {
    if xs.is_empty() {
        "–".to_owned()
    } else {
        xs.join("; ")
    }
}

#[cfg(test)]
mod tests {
    use resume_core::migrate_v1;
    use resume_core::requirements::{Seniority, SkillKind};

    use super::*;

    fn skill(
        term: &str,
        aliases: &[&str],
        priority: u8,
        required: bool,
        group: Option<&str>,
    ) -> Skill {
        Skill {
            term: term.into(),
            aliases: aliases.iter().map(|a| (*a).into()).collect(),
            kind: SkillKind::Hard,
            priority,
            required,
            group: group.map(str::to_owned),
        }
    }

    fn req(skills: Vec<Skill>) -> Requirements {
        Requirements {
            role: "Software Engineer".into(),
            company: "Acme".into(),
            short_role: "swe".into(),
            location: String::new(),
            seniority: Seniority::Entry,
            skills,
            title_terms: vec!["Software Engineer".into()],
            years_min: None,
            degree: None,
            certifications: vec![],
            knockouts: vec![],
        }
    }

    #[test]
    fn aliases_match_either_way() {
        let gold = skill("Kubernetes", &["K8s"], 2, false, None);
        assert!(same_skill(&gold, &skill("k8s", &[], 2, false, None)));
        assert!(same_skill(&skill("K8s", &[], 2, false, None), &gold));
        assert!(same_skill(
            &gold,
            &skill("Kubernetes clusters", &[], 2, false, None)
        ));
        assert!(!same_skill(&gold, &skill("Docker", &[], 2, false, None)));
        // A whole term only: `Java` is not in `JavaScript`.
        assert!(!same_skill(
            &skill("Java", &[], 2, false, None),
            &skill("JavaScript", &[], 2, false, None)
        ));
    }

    #[test]
    fn a_one_of_group_is_one_unit() {
        let gold = vec![
            skill("Python", &[], 1, true, None),
            skill("React", &[], 2, false, Some("ui")),
            skill("Vue", &[], 2, false, Some("ui")),
            skill("Svelte", &[], 2, false, Some("ui")),
            skill("Terraform", &[], 3, false, None),
        ];
        // Python and one member of the group: 2 of 3 units; Terraform missing.
        let run = vec![
            skill("python", &[], 1, true, None),
            skill("Vue.js", &["Vue"], 2, false, None),
            skill("Cobol", &[], 3, false, None),
        ];
        let o = overlap(&gold, &run);
        assert_eq!(o.recall, Some(2.0 / 3.0));
        // Cobol matches nothing gold has.
        assert_eq!(o.precision, Some(2.0 / 3.0));
        // Only Python is a must-have, and it is found.
        assert_eq!(o.must_recall, Some(1.0));
    }

    #[test]
    fn must_recall_misses_a_required_skill() {
        let gold = vec![
            skill("Go", &[], 2, true, None),
            skill("SQL", &[], 1, false, None),
            skill("Rust", &[], 3, false, None),
        ];
        let o = overlap(&gold, &[skill("SQL", &[], 1, false, None)]);
        assert_eq!(o.must_recall, Some(0.5));
        assert_eq!(o.recall, Some(1.0 / 3.0));
        assert_eq!(o.precision, Some(1.0));
        assert_eq!(overlap(&[], &[]).recall, None);
    }

    #[test]
    fn drift_is_positive_when_the_run_flatters_the_master() {
        let v1: Resume = serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/banks/jane-doe.v1.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let mig = migrate_v1(&v1);
        let resume = resume_core::resolve(&mig.bank, &mig.master).unwrap();
        let facts = PageFacts {
            pages: 1,
            page_limit: 1,
            ats_safe: true,
            pdf_text: None,
        };
        // What the resume shows, against what it doesn't.
        let shown: Vec<Skill> = resume
            .sections
            .iter()
            .flat_map(|s| &s.groups)
            .flat_map(|g| &g.items)
            .take(4)
            .map(|i| skill(i, &[], 1, true, None))
            .collect();
        assert!(!shown.is_empty());
        let absent = vec![
            skill("COBOL", &[], 1, true, None),
            skill("Fortran 77", &[], 1, true, None),
        ];
        let d = drift(
            &resume,
            &facts,
            &req(shown.clone()),
            &req(absent.clone()),
            (2026, 11),
        );
        assert!(d > 0.0, "{d}");
        let d = drift(&resume, &facts, &req(absent), &req(shown), (2026, 11));
        assert!(d < 0.0, "{d}");
    }
}
