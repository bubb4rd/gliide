//! The Tailor role: Suggest on every posting (§3.3). Its requirements input is fixed (gold, else sonnet's recorded
//! ones), so only the route varies. Each run takes the app's step after Suggest too (`resume_pipeline::shorten`: a
//! rewrite that wraps long is asked once to fit), its turns and time counted with the run's. Each valid run goes
//! through `changes_of` and `resume_tailor::tailor` as the app's review does: grounding findings, lift, fit-loop drops.
//! The Suggest example's code checks are kept.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Context;
use resume_core::change::{Draft, Mode, State};
use resume_core::changes::{changes_of, draft_changes};
use resume_core::suggestions::{BankOp, BulletOp, Have, KeywordOp, SkillOp, Suggestions};
use resume_core::{Bank, Master, Posting, Requirements, Resume, resolve};
use resume_llm::rewrite;
use resume_llm::suggest::{self, SuggestInput};
use resume_llm::{Agent, Route};
use resume_pipeline::shorten;
use resume_pipeline::{candidates, mode_name, template, today};
use resume_render::{FontOptions, Renderer};
use resume_tailor::tailor;
use serde::Deserialize;
use serde_json::json;

use super::report::{
    Bar, PerPosting, Section, by_route, common, common_metrics, errors, m, rates, write_blind,
};
use super::{Called, Fixtures, ModeArg, Run, TRIM_MASTER, TailorArgs, call, each, mean};

const TIMEOUT: Duration = Duration::from_secs(300);

/// The master a mode's runs tailor: the trimmed one for bank-aware (so a bank entry is left to suggest), the full
/// `swe` master for master-only.
fn master_of(fx: &Fixtures, mode: Mode) -> anyhow::Result<&Master> {
    match mode {
        Mode::BankAware => fx.trim.as_ref().with_context(|| {
            format!("{TRIM_MASTER} is missing: add it, or run with --mode master-only")
        }),
        Mode::MasterOnly => Ok(&fx.master),
    }
}

fn mode_of(role: &str) -> Mode {
    if role == "tailor" {
        Mode::BankAware
    } else {
        Mode::MasterOnly
    }
}

/// One posting's input.
struct Item<'a> {
    posting: &'a Posting,
    req: &'a Requirements,
    /// `gold` or `sonnet`.
    source: &'static str,
    candidates: Vec<String>,
}

pub async fn run(fx: &Fixtures, args: &TailorArgs, out: &Path) -> anyhow::Result<()> {
    let (role, mode) = match args.mode {
        ModeArg::BankAware => ("tailor", Mode::BankAware),
        ModeArg::MasterOnly => ("tailor-master-only", Mode::MasterOnly),
    };
    let master = master_of(fx, mode)?;
    let resume = resolve(&fx.bank, master)?;
    let schema = suggest::schema();
    let mut renderer = Renderer::new(&template(master)?, FontOptions::default());
    let mut items = Vec::new();
    for f in fx.chosen(&args.run.only) {
        let Some((req, source)) = fx.requirements(&f.id) else {
            eprintln!("skipping {}: no gold or sonnet requirements", f.id);
            continue;
        };
        let item = Item {
            posting: &f.posting,
            req,
            source,
            candidates: candidates(&fx.bank, master, req, mode),
        };
        items.push((f.id.clone(), item));
    }
    each(
        role,
        &args.run,
        3,
        TIMEOUT,
        out,
        &items,
        async |llm, route, it| {
            let input = SuggestInput {
                posting: it.posting,
                requirements: it.req,
                resume: &resume,
                bank: &fx.bank,
                candidates: &it.candidates,
            };
            let mut c = call(suggest::suggest(llm, &schema, &input, &route.model)).await;
            let shortened = fit(&mut c, llm, route, &resume, &fx.bank, it.req, &mut renderer).await;
            let meta = json!({
                "mode": mode_name(mode),
                "master": master.id,
                "candidates": it.candidates,
                "requirements": it.source,
                "input_chars": suggest::input(&input).chars().count(),
                "shortened": shortened,
            });
            (c, meta)
        },
    )
    .await
}

/// The app's step after Suggest (`resume_pipeline::shorten`) on a valid run: each rewrite that wraps long, asked once
/// to fit; the turns and time join the run's, a rewrite that now fits replaces the one in its output. How many it
/// replaced.
async fn fit(
    c: &mut Called,
    llm: &Agent,
    route: &Route,
    resume: &Resume,
    bank: &Bank,
    req: &Requirements,
    renderer: &mut Renderer,
) -> usize {
    let Some(Ok(mut s)) = c.output.as_ref().map(Suggestions::deserialize) else {
        return 0;
    };
    let started = Instant::now();
    let long = match shorten::long_rewrites(&s, resume, bank, req, renderer) {
        Ok(long) => long,
        Err(e) => {
            eprint!("(measure failed: {e}) ");
            return 0;
        }
    };
    let mut replaced = 0;
    for job in &long {
        match rewrite::rewrite(llm, &rewrite::schema(), &job.input(), &route.model).await {
            Ok((text, out)) => {
                c.attempts.extend(out.attempts);
                if shorten::apply(&mut s, job, &text, renderer).unwrap_or(false) {
                    replaced += 1;
                }
            }
            Err(e) => eprint!("(shorten failed: {e}) "),
        }
    }
    c.latency += started.elapsed();
    if replaced > 0 {
        c.output = Some(serde_json::to_value(&s).expect("Suggestions serialize"));
    }
    replaced
}

/// What one valid run's suggestions come to.
#[derive(Default)]
struct Tally {
    changes: usize,
    flagged: usize,
    left_validator: usize,
    /// Left out by the fit check for taking more lines than the line it replaces (as the template wraps it).
    left_lines: usize,
    /// Left out by the fit check for the page running over.
    left_page: usize,
    left_apply: usize,
    now: f64,
    projected: f64,
    ceiling: f64,
    problems: Vec<String>,
    kinds: BTreeMap<String, usize>,
    /// One `- [ ] keep — …` line per change, for the blind sheet.
    blind: String,
}

pub fn section(
    fx: &Fixtures,
    runs: &[Run],
    role: &'static str,
    dir: &Path,
) -> anyhow::Result<Vec<Section>> {
    let mode = mode_of(role);
    let master = master_of(fx, mode)?;
    let resume = resolve(&fx.bank, master)?;
    let mut renderer = Renderer::new(&template(master)?, FontOptions::default());
    let today = today();
    let fallback: BTreeSet<&str> = runs
        .iter()
        .map(|r| r.posting.as_str())
        .filter(|p| !fx.gold.contains_key(*p))
        .collect();
    let mut s = Section::new(
        role,
        if role == "tailor" {
            "Tailor: Suggest, bank-aware (trimmed master)"
        } else {
            "Tailor: Suggest, master-only (swe master)"
        },
        format!(
            "Requirements in: gold{}. Each valid run goes through `changes_of` and `tailor` with an empty draft. \
             Flagged: changes the grounding validator flagged. Left out: by the validator (the change has flags, so \
             it starts left out), by the fit check (no flags: it takes more lines than the line it replaces, as the \
             template wraps it, or the page runs over), or because it doesn't apply to the master (`doesn't apply: \
             …`). Lift: projected − now; ceiling lift: ceiling − now. Code checks: the Suggest example's (ids exist, \
             `before` exact, hits are terms, evidence covers every skill in order), total problems. Keep rate: the \
             share of changes marked `[x]` in \
             `{role}/blind.md`, once you've marked it.\n",
            if fallback.is_empty() {
                String::new()
            } else {
                format!(
                    "; sonnet's recorded requirements for {} (no gold yet)",
                    fallback
                        .iter()
                        .map(|p| format!("`{p}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        ),
    );
    let mut metrics = common_metrics(Some(0.95), Some(30.0), None);
    metrics.extend([
        m("changes", "Changes proposed, mean", 1, None),
        m("flagged", "Flagged by the validator, total", 0, None),
        m(
            "left_validator",
            "Left out by the validator, total",
            0,
            None,
        ),
        m(
            "left_lines",
            "Left out: takes more lines, total",
            0,
            None,
        ),
        m("left_page", "Left out: page runs over, total", 0, None),
        m("left_apply", "Left out, doesn't apply, total", 0, None),
        m("now", "Now, mean", 1, None),
        m("projected", "Projected, mean", 1, None),
        m("ceiling", "Ceiling, mean", 1, None),
        m("lift", "Lift (projected − now), mean", 1, None),
        m("ceiling_lift", "Ceiling lift, mean", 1, None),
        m(
            "problems",
            "Code-check problems, total",
            0,
            Some(Bar::AtMost(0.0)),
        ),
        m("keep_rate", "Blind keep rate", 2, None),
    ]);
    s.metrics = metrics;
    let keep = rates(dir)?;

    let mut lift_pp = PerPosting::new("Lift", 1);
    let mut lat_pp = PerPosting::new("Latency s", 1);
    let mut kinds: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut details = String::new();
    let mut blind: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (route, rs) in by_route(runs) {
        let mut n = common(&rs);
        let mut tallies: Vec<(&Run, Tally)> = Vec::new();
        for r in rs.iter().filter(|r| r.ok()) {
            let Some(out) = &r.output else { continue };
            let Some((req, _)) = fx.requirements(&r.posting) else {
                continue;
            };
            let Ok(sg) = Suggestions::deserialize(out) else {
                continue;
            };
            let cands: Vec<String> = r.meta["candidates"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|c| c.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            let text = fx
                .posting(&r.posting)
                .map_or("", |f| f.posting.text.as_str());
            let mut t = Tally {
                problems: problems(&sg, &Lines::new(&resume, &fx.bank, &cands), req, text),
                ..Tally::default()
            };
            let changes = draft_changes(
                &changes_of(&sg, &fx.bank, master, req, mode),
                &Draft::default(),
            );
            t.changes = changes.len();
            t.flagged = changes.iter().filter(|c| !c.flags.is_empty()).count();
            for c in &changes {
                let kind = serde_json::to_value(&c.op)?["op"]
                    .as_str()
                    .unwrap_or("?")
                    .to_owned();
                *t.kinds.entry(kind).or_default() += 1;
            }
            match tailor(
                &fx.bank,
                master,
                req,
                &changes,
                &Draft::default(),
                &mut renderer,
                today,
            ) {
                Ok(tl) => {
                    for c in &changes {
                        if let Some(State::LeftOut { reason }) = tl.states.get(&c.key) {
                            if !c.flags.is_empty() {
                                t.left_validator += 1;
                            } else if reason.starts_with("doesn't apply") {
                                t.left_apply += 1;
                            } else if reason.starts_with("takes ") {
                                t.left_lines += 1;
                            } else {
                                t.left_page += 1;
                            }
                        }
                        let line = super::super::tailor::summary(
                            c,
                            tl.after.get(&c.key).map(String::as_str),
                        );
                        let _ = writeln!(t.blind, "- [ ] keep — {}", line.replace('\n', " "));
                    }
                    t.now = tl.now.total;
                    t.projected = tl.projected.total;
                    t.ceiling = tl.ceiling.total;
                }
                Err(e) => {
                    let _ = writeln!(
                        details,
                        "- `{route}` `{}` #{}: tailor failed: {e}",
                        r.posting, r.n
                    );
                    continue;
                }
            }
            tallies.push((r, t));
        }
        let sum = |f: fn(&Tally) -> usize| tallies.iter().map(|(_, t)| f(t)).sum::<usize>() as f64;
        let avg = |f: fn(&Tally) -> f64| mean(tallies.iter().map(|(_, t)| f(t)));
        n.insert("flagged", sum(|t| t.flagged));
        n.insert("left_validator", sum(|t| t.left_validator));
        n.insert("left_apply", sum(|t| t.left_apply));
        n.insert("left_lines", sum(|t| t.left_lines));
        n.insert("left_page", sum(|t| t.left_page));
        n.insert("problems", sum(|t| t.problems.len()));
        for (k, v) in [
            ("changes", avg(|t| t.changes as f64)),
            ("now", avg(|t| t.now)),
            ("projected", avg(|t| t.projected)),
            ("ceiling", avg(|t| t.ceiling)),
            ("lift", avg(|t| t.projected - t.now)),
            ("ceiling_lift", avg(|t| t.ceiling - t.now)),
            (
                "keep_rate",
                keep.as_ref().and_then(|k| k.get(&route).copied()),
            ),
        ] {
            if let Some(v) = v {
                n.insert(k, v);
            }
        }
        for r in &rs {
            lat_pp.push(&r.posting, &route, r.secs());
        }
        for (r, t) in &tallies {
            lift_pp.push(&r.posting, &route, t.projected - t.now);
            for (k, c) in &t.kinds {
                *kinds
                    .entry(k.clone())
                    .or_default()
                    .entry(route.clone())
                    .or_default() += c;
            }
            if !t.problems.is_empty() {
                let _ = writeln!(
                    details,
                    "- `{route}` `{}` #{}: {}",
                    r.posting,
                    r.n,
                    t.problems.join("; ").replace('|', "\\|")
                );
            }
            // The sheet shows each route's first valid run of a posting.
            let sheet = blind.entry(r.posting.clone()).or_default();
            if !sheet.contains_key(&route) {
                let body = if t.blind.is_empty() {
                    "(no changes)\n- [ ] keep — nothing".to_owned()
                } else {
                    t.blind.clone()
                };
                sheet.insert(route.clone(), body);
            }
        }
        s.routes.insert(route, n);
    }
    s.per_posting = vec![lift_pp, lat_pp];
    if !kinds.is_empty() {
        let routes: Vec<&String> = s.routes.keys().collect();
        let _ = write!(s.extra, "\n### Changes by kind, total\n\n| Kind |");
        for r in &routes {
            let _ = write!(s.extra, " `{r}` |");
        }
        let _ = write!(s.extra, "\n|---|{}\n", "---|".repeat(routes.len()));
        for (k, by) in &kinds {
            let _ = write!(s.extra, "| {k} |");
            for r in &routes {
                let _ = write!(s.extra, " {} |", by.get(*r).copied().unwrap_or(0));
            }
            s.extra.push('\n');
        }
    }
    if !details.is_empty() {
        let _ = write!(s.extra, "\n### Problems code can see\n\n{details}");
    }
    s.extra.push_str(&errors(runs));
    write_blind(
        dir,
        s.title,
        "Each route's changes for a posting (its first valid run). Mark `[x] keep` on each change you'd keep.",
        &blind,
    )?;
    Ok(vec![s])
}

// ── Code checks (from the Suggest example) ─────────────────────────────────────────────────────────────────────────

/// The lines Suggest could name: id → its text (`""` for headings and entries), skills line → items, and the entry
/// each entry, note and bullet belongs to.
struct Lines {
    text: BTreeMap<String, String>,
    groups: BTreeMap<String, Vec<String>>,
    /// Section id → its entry ids on the resume.
    sections: BTreeMap<String, Vec<String>>,
    entry_of: BTreeMap<String, String>,
}

impl Lines {
    fn new(resume: &Resume, bank: &Bank, candidates: &[String]) -> Self {
        let mut l = Lines {
            text: BTreeMap::new(),
            groups: BTreeMap::new(),
            sections: BTreeMap::new(),
            entry_of: BTreeMap::new(),
        };
        for s in &resume.sections {
            l.text.insert(s.id.clone(), String::new());
            l.sections.insert(
                s.id.clone(),
                s.entries.iter().map(|e| e.id.clone()).collect(),
            );
            for e in &s.entries {
                let facts = bank
                    .sections
                    .iter()
                    .flat_map(|s| &s.entries)
                    .find(|b| b.id == e.id)
                    .map(|b| b.facts.as_slice())
                    .unwrap_or_default();
                l.entry(
                    &e.id,
                    e.notes.iter().map(|n| n.text.as_str()),
                    e.bullets.iter().map(|b| (b.id.as_str(), b.text.as_str())),
                    facts.iter().map(|f| (f.id.as_str(), f.text.as_str())),
                );
            }
            for g in &s.groups {
                l.text.insert(g.id.clone(), g.items.join(", "));
                l.groups.insert(g.id.clone(), g.items.clone());
            }
        }
        for s in &bank.sections {
            for e in s.entries.iter().filter(|e| candidates.contains(&e.id)) {
                l.entry(
                    &e.id,
                    e.notes.iter().map(|n| n.text.as_str()),
                    e.bullets.iter().map(|b| (b.id.as_str(), b.text.as_str())),
                    e.facts.iter().map(|f| (f.id.as_str(), f.text.as_str())),
                );
            }
        }
        l
    }

    /// Entry `id`'s lines: its notes, bullets and facts, each by its tag.
    fn entry<'a>(
        &mut self,
        id: &str,
        notes: impl Iterator<Item = &'a str>,
        bullets: impl Iterator<Item = (&'a str, &'a str)>,
        facts: impl Iterator<Item = (&'a str, &'a str)>,
    ) {
        let mut put = |line: String, text: &str| {
            self.entry_of.insert(line.clone(), id.to_owned());
            self.text.insert(line, text.to_owned());
        };
        put(id.to_owned(), "");
        for (j, n) in notes.enumerate() {
            put(format!("{id}#{}", j + 1), n);
        }
        for (line, text) in bullets.chain(facts) {
            put(line.to_owned(), text);
        }
    }
}

/// What code can check in one output; each entry is one problem.
fn problems(s: &Suggestions, lines: &Lines, req: &Requirements, posting: &str) -> Vec<String> {
    let mut p = Vec::new();
    let terms: BTreeSet<&str> = req.skills.iter().map(|k| k.term.as_str()).collect();
    let posting = posting.to_lowercase();
    let known = |id: &str, what: &str, p: &mut Vec<String>| {
        if !lines.text.contains_key(id) {
            p.push(format!("{what}: unknown id `{id}`"));
        }
    };
    let skill_ok = |skill: &str, what: &str, p: &mut Vec<String>| {
        if !skill.is_empty() && !terms.contains(skill) {
            p.push(format!("{what}: `{skill}` is not a requirement term"));
        }
    };
    let ev: Vec<&str> = s.evidence.iter().map(|e| e.skill.as_str()).collect();
    let want: Vec<&str> = req.skills.iter().map(|k| k.term.as_str()).collect();
    if ev != want {
        p.push(format!("evidence: {ev:?} ≠ requirement terms {want:?}"));
    }
    for e in &s.evidence {
        for id in &e.ids {
            known(id, "evidence", &mut p);
        }
        if e.have == Have::No && !e.ids.is_empty() {
            p.push(format!("evidence `{}`: no with ids", e.skill));
        }
        if e.have != Have::No && e.ids.is_empty() {
            p.push(format!("evidence `{}`: backed with no ids", e.skill));
        }
    }
    for k in &s.keywords {
        known(&k.line, "keyword", &mut p);
        skill_ok(&k.skill, "keyword skill", &mut p);
        let text = lines.text.get(&k.line).map_or("", String::as_str);
        if lines.groups.contains_key(&k.line) {
            p.push(format!("keyword on skills line `{}`", k.line));
        }
        // The posting's words: the whole term in the posting, or it holds the requirement skill it names (`TOTP MFA`).
        let posting_words = |term: &str| {
            let term = term.to_lowercase();
            posting.contains(&term)
                || req.skills.iter().filter(|r| r.term == k.skill).any(|r| {
                    std::iter::once(&r.term)
                        .chain(&r.aliases)
                        .any(|t| term.contains(&t.to_lowercase()))
                })
        };
        match k.op {
            KeywordOp::Edit if !text.contains(&k.from) => {
                p.push(format!("keyword edit: `{}` not in `{}`", k.from, k.line))
            }
            KeywordOp::Edit if k.from == k.term => {
                p.push(format!("keyword edit on `{}` changes nothing", k.line))
            }
            KeywordOp::Drop if !text.contains(&k.term) => {
                p.push(format!("keyword drop: `{}` not in `{}`", k.term, k.line))
            }
            KeywordOp::Add | KeywordOp::Edit if !posting_words(&k.term) => {
                p.push(format!("keyword term `{}` not in the posting", k.term))
            }
            _ => {}
        }
    }
    for b in &s.bullets {
        known(&b.id, "bullet", &mut p);
        for id in &b.sources {
            known(id, "bullet source", &mut p);
        }
        for h in &b.hits {
            skill_ok(h, "hit", &mut p);
        }
        match b.op {
            BulletOp::Rewrite | BulletOp::Cut => {
                let text = lines.text.get(&b.id).map_or("", String::as_str);
                if b.before != text {
                    p.push(format!("bullet `{}`: before is not the line's text", b.id));
                }
            }
            BulletOp::Add => {}
        }
        if b.op == BulletOp::Rewrite && b.sources.first() != Some(&b.id) {
            p.push(format!("bullet `{}`: sources don't start with it", b.id));
        }
        if b.op != BulletOp::Cut && b.sources.is_empty() {
            p.push(format!("bullet `{}`: no sources", b.id));
        }
        let own = lines.entry_of.get(&b.id);
        for src in &b.sources {
            if lines.entry_of.get(src) != own {
                p.push(format!(
                    "bullet `{}`: source `{src}` is another entry's",
                    b.id
                ));
            }
        }
    }
    for k in &s.skills {
        if !lines.groups.contains_key(&k.group) {
            p.push(format!("skills: unknown line `{}`", k.group));
        }
        skill_ok(&k.skill, "skills skill", &mut p);
        let items = lines.groups.get(&k.group);
        let has = items.is_some_and(|i| i.contains(&k.item));
        match k.op {
            SkillOp::Add if has => p.push(format!("skills add: `{}` already there", k.item)),
            SkillOp::Add if !posting.contains(&k.item.to_lowercase()) => {
                p.push(format!("skills add `{}` not in the posting", k.item))
            }
            SkillOp::Remove | SkillOp::Rename | SkillOp::Move if !has => p.push(format!(
                "skills {:?}: `{}` not on `{}`",
                k.op, k.item, k.group
            )),
            _ => {}
        }
        if k.op == SkillOp::Move && !lines.groups.contains_key(&k.to) {
            p.push(format!("skills move: unknown line `{}`", k.to));
        }
        if let Some(e) = s.evidence.iter().find(|e| e.skill == k.skill)
            && e.have == Have::No
        {
            p.push(format!("skills edit for `{}`, evidence no", k.skill));
        }
    }
    for b in &s.bank {
        known(&b.entry, "bank entry", &mut p);
        for id in &b.bullets {
            known(id, "bank bullet", &mut p);
        }
        if b.op == BankOp::Swap {
            known(&b.replaces, "bank replaces", &mut p);
        }
        for k in &b.skills {
            skill_ok(k, "bank skill", &mut p);
        }
    }
    for o in &s.order {
        match lines.sections.get(&o.section) {
            None => p.push(format!("order: unknown section `{}`", o.section)),
            Some(have) => {
                let (a, b): (BTreeSet<_>, BTreeSet<_>) =
                    (have.iter().collect(), o.entries.iter().collect());
                if a != b || have.len() != o.entries.len() {
                    p.push(format!("order `{}`: not the section's entries", o.section));
                }
            }
        }
    }
    for g in &s.gaps {
        skill_ok(&g.skill, "gap skill", &mut p);
    }
    p
}
