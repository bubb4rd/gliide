//! The Coach role (§3.4): Prep for a technical round per posting, with no earlier debriefs or stories, from the
//! resolved `swe` master; and Dig's questions per bank entry, with the terms your postings ask for that the entry
//! doesn't state. Both are built as the app builds them (`app/src-tauri/src/prep.rs`, `dig.rs`), minimally. Validity,
//! latency, question counts after the app's checks, and a blind sheet.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

use resume_core::Requirements;
use resume_core::bank::{BankEntry, SectionKind};
use resume_core::matcher::{Page, find_literal, weight};
use resume_llm::dig::{self, Questions, QuestionsInput};
use resume_llm::prep::{self, Prep, PrepInput, SentResume};
use serde::Deserialize;
use serde_json::json;

use super::report::{
    PerPosting, Section, by_route, common, common_metrics, errors, m, rates, write_blind,
};
use super::{Fixtures, Run, RunArgs, call, date, each, mean};

const TIMEOUT: Duration = Duration::from_secs(300);
/// The round every prep is for.
const ROUND: &str = "technical";
/// Dig's item ids: `dig-<entry id>`.
const DIG: &str = "dig-";

enum Item<'a> {
    Prep {
        company: String,
        role: String,
        req: Option<&'a Requirements>,
    },
    Dig {
        entry: &'a BankEntry,
        masters: Vec<String>,
        terms: Vec<(String, usize)>,
    },
}

pub async fn run(fx: &Fixtures, args: &RunArgs, out: &Path) -> anyhow::Result<()> {
    let sent = SentResume::tagged(&fx.resume, &fx.bank);
    let today = date();
    let (prep_schema, dig_schema) = (prep::schema(), dig::questions_schema());
    let mut items: Vec<(String, Item)> = fx
        .chosen(&args.only)
        .into_iter()
        .map(|f| {
            let (company, role) = super::write::job(fx, f);
            let req = fx.requirements(&f.id).map(|(r, _)| r);
            (f.id.clone(), Item::Prep { company, role, req })
        })
        .collect();
    let reqs: Vec<&Requirements> = fx
        .postings
        .iter()
        .filter_map(|f| fx.requirements(&f.id).map(|(r, _)| r))
        .collect();
    for e in entries(fx) {
        let id = format!("{DIG}{}", e.id);
        if !args.only.is_empty() && !args.only.contains(&id) {
            continue;
        }
        let shown = fx
            .master
            .sections
            .iter()
            .any(|s| s.entries.iter().any(|x| x.id == e.id));
        items.push((
            id,
            Item::Dig {
                entry: e,
                masters: if shown {
                    vec![fx.master.title.clone()]
                } else {
                    vec![]
                },
                terms: unstated_terms(e, &reqs),
            },
        ));
    }
    each(
        "coach",
        args,
        1,
        TIMEOUT,
        out,
        &items,
        async |llm, route, item| match item {
            Item::Prep { company, role, req } => {
                let input = PrepInput {
                    kind: ROUND,
                    with: "",
                    notes: "",
                    role,
                    company,
                    requirements: *req,
                    sent: &sent,
                    stories: &[],
                    debriefs: &[],
                    today: &today,
                };
                let c = call(prep::write(llm, &prep_schema, &input, &route.model)).await;
                (
                    c,
                    json!({ "kind": "prep", "round": ROUND, "company": company, "role": role }),
                )
            }
            Item::Dig {
                entry,
                masters,
                terms,
            } => {
                let input = QuestionsInput {
                    entry,
                    masters,
                    terms,
                    today: &today,
                };
                let c = call(dig::ask(llm, &dig_schema, &input, &route.model)).await;
                (
                    c,
                    json!({ "kind": "dig", "entry": entry.id, "terms": terms }),
                )
            }
        },
    )
    .await
}

/// The bank entries Dig asks about: every entry outside the summary.
fn entries(fx: &Fixtures) -> impl Iterator<Item = &BankEntry> {
    fx.bank
        .sections
        .iter()
        .filter(|s| s.kind != SectionKind::Summary)
        .flat_map(|s| &s.entries)
}

/// The requirement terms `reqs` ask for that entry `e` doesn't state anywhere, most asked first (then most weighed), at
/// most [`dig::MAX_TERMS`]: `(term, postings asking)`. As the app's `dig::unstated_terms`.
fn unstated_terms(e: &BankEntry, reqs: &[&Requirements]) -> Vec<(String, usize)> {
    let mut text = vec![e.name.clone(), e.subtitle.clone()];
    text.extend(e.notes.iter().map(|n| format!("{}: {}", n.label, n.text)));
    for b in &e.bullets {
        text.push(b.text.clone());
        text.extend(b.variants.iter().cloned());
        text.extend(b.skills.iter().cloned());
    }
    text.extend(e.fact_lines().map(str::to_owned));
    let page = Page::of_text(&text.join("\n"));
    let norm = |t: &str| {
        t.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    // key → (term as first written, postings, weight).
    let mut counts: HashMap<String, (String, usize, f64)> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for r in reqs {
        let mut seen: Vec<String> = Vec::new();
        for k in &r.skills {
            let key = norm(&k.term);
            if key.is_empty() || seen.contains(&key) {
                continue;
            }
            seen.push(key.clone());
            if find_literal(&page, &k.term, &k.aliases).is_some() {
                continue;
            }
            let c = counts.entry(key.clone()).or_insert_with(|| {
                order.push(key.clone());
                (k.term.clone(), 0, 0.0)
            });
            c.1 += 1;
            c.2 += weight(k.priority, k.required);
        }
    }
    let mut out: Vec<(String, usize, f64)> = order
        .into_iter()
        .filter_map(|k| counts.remove(&k))
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.total_cmp(&a.2)));
    out.into_iter()
        .take(dig::MAX_TERMS)
        .map(|(t, n, _)| (t, n))
        .collect()
}

fn quote(s: &str) -> String {
    s.replace('\n', " ")
}

pub fn section(fx: &Fixtures, runs: &[Run], dir: &Path) -> anyhow::Result<Vec<Section>> {
    let sent = SentResume::tagged(&fx.resume, &fx.bank);
    let pref = rates(dir)?;
    let (preps, digs): (Vec<&Run>, Vec<&Run>) =
        runs.iter().partition(|r| !r.posting.starts_with(DIG));
    let mut blind: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();

    let mut prep_s = Section::new(
        "coach-prep",
        "Coach: Prep (technical round)",
        "Per posting: a technical round, no earlier debriefs, no stories, the `swe` master as sent. Counts are after \
         `prep::checked` (as the app keeps them); dropped: line ids, gaps, sentences and stories it took out. \
         Preferred: the share of items whose `[x] best` in `coach/blind.md` is this route's, once marked (Prep and \
         Dig together).\n"
            .to_owned(),
    );
    prep_s.metrics = common_metrics(None, None, None);
    prep_s.metrics.extend([
        m("questions", "Questions, mean", 1, None),
        m("gaps", "Gaps, mean", 1, None),
        m("ask_them", "Questions to ask them, mean", 1, None),
        m("dropped", "Dropped by the checks, mean", 1, None),
        m("preferred", "Blind preference", 2, None),
    ]);
    let mut q_pp = PerPosting::new("Questions", 0);
    let mut lat_pp = PerPosting::new("Latency s", 1);
    for (route, rs) in by_route(preps.iter().copied()) {
        let mut n = common(&rs);
        let mut kept: Vec<(&Run, Prep, usize)> = Vec::new();
        for r in rs.iter().filter(|r| r.ok()) {
            let Some(p) = r.output.as_ref().and_then(|o| Prep::deserialize(o).ok()) else {
                continue;
            };
            let req = fx.requirements(&r.posting).map(|(q, _)| q);
            let (p, d) = prep::checked(p, &sent, req, &[]);
            let dropped = d.lines.len() + d.gaps.len() + d.sentences.len() + d.stories.len();
            kept.push((r, p, dropped));
        }
        for (k, v) in [
            (
                "questions",
                mean(kept.iter().map(|(_, p, _)| p.questions.len() as f64)),
            ),
            (
                "gaps",
                mean(kept.iter().map(|(_, p, _)| p.gaps.len() as f64)),
            ),
            (
                "ask_them",
                mean(kept.iter().map(|(_, p, _)| p.ask_them.len() as f64)),
            ),
            ("dropped", mean(kept.iter().map(|(_, _, d)| *d as f64))),
            (
                "preferred",
                pref.as_ref().and_then(|p| p.get(&route).copied()),
            ),
        ] {
            if let Some(v) = v {
                n.insert(k, v);
            }
        }
        for r in &rs {
            lat_pp.push(&r.posting, &route, r.secs());
        }
        for (r, p, _) in &kept {
            q_pp.push(&r.posting, &route, p.questions.len() as f64);
            let sheet = blind.entry(r.posting.clone()).or_default();
            if !sheet.contains_key(&route) {
                let mut body = format!("> **Focus:** {}\n>\n", quote(&p.focus));
                for q in &p.questions {
                    let _ = writeln!(body, "> - {} _({})_", quote(&q.text), quote(&q.why));
                }
                for g in &p.gaps {
                    let _ = writeln!(body, "> - gap **{}**: {}", g.term, quote(&g.answer));
                }
                body.push_str("\n- [ ] best\n");
                sheet.insert(route.clone(), body);
            }
        }
        prep_s.routes.insert(route, n);
    }
    prep_s.per_posting = vec![q_pp, lat_pp];
    prep_s.extra = errors(preps.iter().copied());

    let mut dig_s = Section::new(
        "coach-dig",
        "Coach: Dig questions",
        "Per bank entry (`dig-<entry>`): its questions, with the terms the postings' requirements (gold, else sonnet's) \
         ask for that it doesn't state. Counts are after `dig::questions_checked`.\n"
            .to_owned(),
    );
    dig_s.metrics = common_metrics(None, None, None);
    dig_s.metrics.extend([
        m("questions", "Questions, mean", 1, None),
        m("preferred", "Blind preference", 2, None),
    ]);
    let mut q_pp = PerPosting::new("Questions", 0);
    let mut lat_pp = PerPosting::new("Latency s", 1);
    for (route, rs) in by_route(digs.iter().copied()) {
        let mut n = common(&rs);
        let mut kept: Vec<(&Run, Questions)> = Vec::new();
        for r in rs.iter().filter(|r| r.ok()) {
            let entry = entries(fx).find(|e| r.posting.strip_prefix(DIG) == Some(e.id.as_str()));
            let (Some(q), Some(e)) = (
                r.output
                    .as_ref()
                    .and_then(|o| Questions::deserialize(o).ok()),
                entry,
            ) else {
                continue;
            };
            kept.push((r, dig::questions_checked(q, e)));
        }
        for (k, v) in [
            (
                "questions",
                mean(kept.iter().map(|(_, q)| q.questions.len() as f64)),
            ),
            (
                "preferred",
                pref.as_ref().and_then(|p| p.get(&route).copied()),
            ),
        ] {
            if let Some(v) = v {
                n.insert(k, v);
            }
        }
        for r in &rs {
            lat_pp.push(&r.posting, &route, r.secs());
        }
        for (r, q) in &kept {
            q_pp.push(&r.posting, &route, q.questions.len() as f64);
            let sheet = blind.entry(r.posting.clone()).or_default();
            if !sheet.contains_key(&route) {
                let mut body = format!("> **Read:** {}\n>\n", quote(&q.read));
                for x in &q.questions {
                    let _ = writeln!(
                        body,
                        "> - ({}) {} _({})_",
                        x.about_label,
                        quote(&x.text),
                        quote(&x.why)
                    );
                }
                body.push_str("\n- [ ] best\n");
                sheet.insert(route.clone(), body);
            }
        }
        dig_s.routes.insert(route, n);
    }
    dig_s.per_posting = vec![q_pp, lat_pp];
    dig_s.extra = errors(digs.iter().copied());

    write_blind(
        dir,
        "Coach",
        "Each route's prep for a posting, or questions for a bank entry. Mark `[x] best` on the one per item you'd \
         rather have.",
        &blind,
    )?;
    let mut out = Vec::new();
    if !preps.is_empty() {
        out.push(prep_s);
    }
    if !digs.is_empty() {
        out.push(dig_s);
    }
    Ok(out)
}
