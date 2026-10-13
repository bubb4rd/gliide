//! Suggest runs: Suggest through `claude -p` on fixture postings, sequentially, for one bank and master.
//!
//! `cargo run -p resume-llm --example suggest -- --bank <bank.json> --master <master.json> [--candidates id,id]
//! [--only <posting-id>[,<posting-id>…]] [--out <dir>] [--golden <dir>] [--model sonnet] [--report]`
//!
//! Each `fixtures/postings/<id>.md` runs with its requirements from `fixtures/requirements/sonnet/<id>.json`. With
//! `--candidates` the run is bank-aware (those bank entries are offered), else master-only. Writes each run (raw
//! replies, latency, usage, output) to `<out>/<id>.json`, default `local/m3/suggest/<master-only|bank-aware>`, copies
//! each valid output to `<golden>/<id>.json` when given, and rewrites `<out>/report.md` from every run recorded there:
//! outcome, latency, tokens, cost, counts per field, and what code can check (ids exist, `before` copied exactly,
//! line budget, hits are requirement terms, evidence covers every skill in order). `--report` only rewrites the
//! report.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use resume_core::suggestions::{BankOp, BulletOp, Have, KeywordOp, SkillOp, Suggestions};
use resume_core::{Bank, Master, Posting, Requirements, Resume, resolve};
use resume_llm::suggest::{self, DEFAULT_SUGGEST_MODEL, SuggestInput};
use resume_llm::{Attempt, ClaudeCli, LlmError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const TIMEOUT: Duration = Duration::from_secs(300);
/// Characters per printed line, as the prompt's line budget assumes.
const LINE: usize = 100;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Outcome {
    Valid,
    Repaired,
    Failed,
}

/// One run as recorded in `<out>/<id>.json`.
#[derive(Serialize, Deserialize)]
struct Record {
    id: String,
    model: String,
    bank: String,
    master: String,
    candidates: Vec<String>,
    outcome: Outcome,
    error: Option<String>,
    latency_ms: u64,
    input_chars: usize,
    cost_usd: Option<f64>,
    output_tokens: Option<u64>,
    output: Option<Value>,
    attempts: Vec<Value>,
}

struct Args {
    bank: PathBuf,
    master: PathBuf,
    candidates: Vec<String>,
    only: Vec<String>,
    out: Option<PathBuf>,
    golden: Option<PathBuf>,
    model: String,
    report_only: bool,
}

fn args() -> Args {
    let usage = "usage: suggest --bank <bank.json> --master <master.json> [--candidates id,id] [--only id,id] \
                 [--out <dir>] [--golden <dir>] [--model sonnet] [--report]";
    let list = |s: String| -> Vec<String> {
        s.split(',')
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(str::to_owned)
            .collect()
    };
    let (mut bank, mut master) = (None, None);
    let mut a = Args {
        bank: PathBuf::new(),
        master: PathBuf::new(),
        candidates: Vec::new(),
        only: Vec::new(),
        out: None,
        golden: None,
        model: DEFAULT_SUGGEST_MODEL.to_owned(),
        report_only: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .unwrap_or_else(|| panic!("{arg} needs a value; {usage}"))
        };
        match arg.as_str() {
            "--bank" => bank = Some(PathBuf::from(value())),
            "--master" => master = Some(PathBuf::from(value())),
            "--candidates" => a.candidates = list(value()),
            "--only" => a.only = list(value()),
            "--out" => a.out = Some(PathBuf::from(value())),
            "--golden" => a.golden = Some(PathBuf::from(value())),
            "--model" => a.model = value(),
            "--report" => a.report_only = true,
            _ => panic!("unknown argument {arg}; {usage}"),
        }
    }
    a.bank = bank.unwrap_or_else(|| panic!("--bank is required; {usage}"));
    a.master = master.unwrap_or_else(|| panic!("--master is required; {usage}"));
    a
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let a = args();
    let bank: Bank = read_json(&a.bank);
    let master: Master = read_json(&a.master);
    let resume = resolve(&bank, &master).unwrap_or_else(|e| panic!("{}: {e}", a.master.display()));
    for c in &a.candidates {
        assert!(
            bank.sections
                .iter()
                .any(|s| s.entries.iter().any(|e| e.id == *c)),
            "candidate `{c}` is not a bank entry"
        );
    }
    let mode = if a.candidates.is_empty() {
        "master-only"
    } else {
        "bank-aware"
    };
    let out = a
        .out
        .clone()
        .unwrap_or_else(|| root.join("local/m3/suggest").join(mode));
    std::fs::create_dir_all(&out).unwrap();

    let req_path = |id: &str| root.join(format!("fixtures/requirements/sonnet/{id}.json"));
    // A posting runs only once Extract has recorded its requirements.
    let postings: Vec<(String, Posting)> = postings(&root.join("fixtures/postings"))
        .into_iter()
        .filter(|(id, _)| {
            let has = req_path(id).exists();
            if !has {
                eprintln!("skipping {id}: no fixtures/requirements/sonnet/{id}.json");
            }
            has
        })
        .collect();
    let chosen: Vec<&(String, Posting)> = postings
        .iter()
        .filter(|(id, _)| a.only.is_empty() || a.only.contains(id))
        .collect();
    assert!(!chosen.is_empty(), "no posting matches");
    let requirements = |id: &str| -> Requirements { read_json(&req_path(id)) };

    if !a.report_only {
        let provider = match ClaudeCli::discover().await {
            Ok(p) => p.with_timeout(TIMEOUT),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        };
        eprintln!("claude: {}", provider.bin().display());
        let schema = suggest::schema();
        if let Some(g) = &a.golden {
            std::fs::create_dir_all(g).unwrap();
        }
        for (id, posting) in &chosen {
            let req = requirements(id);
            let input = SuggestInput {
                posting,
                requirements: &req,
                resume: &resume,
                bank: &bank,
                candidates: &a.candidates,
            };
            eprint!("{} {mode} {id} … ", a.model);
            let started = Instant::now();
            let result = suggest::suggest(&provider, &schema, &input, &a.model).await;
            let latency = started.elapsed();
            let (outcome, output, attempts, error) = match result {
                Ok((_, s)) => {
                    let outcome = if s.repaired() {
                        Outcome::Repaired
                    } else {
                        Outcome::Valid
                    };
                    (outcome, Some(s.value), s.attempts, None)
                }
                Err(e) => {
                    let msg = e.to_string();
                    match e {
                        LlmError::Invalid { attempts, raw, .. } => (
                            Outcome::Failed,
                            None,
                            attempts,
                            Some(format!("{msg}\n{raw}")),
                        ),
                        LlmError::Decode { output, .. } => (
                            Outcome::Failed,
                            Some(output.value),
                            output.attempts,
                            Some(msg),
                        ),
                        _ => (Outcome::Failed, None, Vec::new(), Some(msg)),
                    }
                }
            };
            eprintln!(
                "{} {:.1}s{}",
                label_of(outcome),
                latency.as_secs_f64(),
                error
                    .as_deref()
                    .and_then(|e| e.lines().next())
                    .map(|e| format!(" ({e})"))
                    .unwrap_or_default()
            );
            if let (Some(g), Some(v), true) = (&a.golden, &output, outcome != Outcome::Failed) {
                write_json(&g.join(format!("{id}.json")), v);
            }
            let costs: Vec<f64> = attempts.iter().filter_map(|t| t.usage.cost_usd).collect();
            let tokens: Vec<u64> = attempts
                .iter()
                .filter_map(|t| t.usage.output_tokens)
                .collect();
            let record = Record {
                id: id.clone(),
                model: a.model.clone(),
                bank: a.bank.display().to_string(),
                master: a.master.display().to_string(),
                candidates: a.candidates.clone(),
                outcome,
                error,
                latency_ms: latency.as_millis() as u64,
                input_chars: suggest::input(&input).chars().count(),
                cost_usd: (!costs.is_empty()).then(|| costs.iter().sum()),
                output_tokens: (!tokens.is_empty()).then(|| tokens.iter().sum()),
                output,
                attempts: attempts.iter().map(attempt_json).collect(),
            };
            write_json(
                &out.join(format!("{id}.json")),
                &serde_json::to_value(&record).unwrap(),
            );
        }
    }

    let records = recorded(&out);
    let md = report(
        &display(&root, &out),
        &records,
        &bank,
        &resume,
        &postings,
        |id| requirements(id),
    );
    std::fs::write(out.join("report.md"), &md).unwrap();
    println!("{md}");
}

/// Every posting fixture's id and text, in id order. Suggest reads only the text.
fn postings(dir: &Path) -> Vec<(String, Posting)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "md")
                && p.file_name().is_some_and(|n| n != "README.md")
        })
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).unwrap();
            let body = text
                .strip_prefix("---\n")
                .and_then(|rest| rest.find("\n---\n").map(|end| &rest[end + 5..]))
                .unwrap_or(&text);
            let id = path.file_stem().unwrap().to_string_lossy().into_owned();
            (id, Posting::pasted(body.to_owned()))
        })
        .collect()
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn write_json(path: &Path, v: &Value) {
    let mut s = serde_json::to_string_pretty(v).unwrap();
    s.push('\n');
    std::fs::write(path, s).unwrap();
}

fn label_of(o: Outcome) -> &'static str {
    match o {
        Outcome::Valid => "valid",
        Outcome::Repaired => "repaired",
        Outcome::Failed => "failed",
    }
}

fn attempt_json(a: &Attempt) -> Value {
    json!({
        "latency_ms": a.latency_ms,
        "usage": a.usage,
        "raw": serde_json::from_str::<Value>(&a.raw).unwrap_or_else(|_| Value::String(a.raw.clone())),
    })
}

fn display(root: &Path, dir: &Path) -> String {
    dir.strip_prefix(root).unwrap_or(dir).display().to_string()
}

/// Every run recorded in `dir`, by posting id.
fn recorded(dir: &Path) -> BTreeMap<String, Record> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return BTreeMap::new();
    };
    entries
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .map(|file| {
            let record: Record = serde_json::from_str(&std::fs::read_to_string(&file).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            (record.id.clone(), record)
        })
        .collect()
}

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
                l.entry(
                    &e.id,
                    e.notes.iter().map(|n| n.text.as_str()),
                    e.bullets.iter().map(|b| (b.id.as_str(), b.text.as_str())),
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
                );
            }
        }
        l
    }

    fn entry<'a>(
        &mut self,
        id: &str,
        notes: impl Iterator<Item = &'a str>,
        bullets: impl Iterator<Item = (&'a str, &'a str)>,
    ) {
        let mut put = |line: String, text: &str| {
            self.entry_of.insert(line.clone(), id.to_owned());
            self.text.insert(line, text.to_owned());
        };
        put(id.to_owned(), "");
        for (j, n) in notes.enumerate() {
            put(format!("{id}#{}", j + 1), n);
        }
        for (b, text) in bullets {
            put(b.to_owned(), text);
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
        if b.op == BulletOp::Rewrite {
            let (before, after) = (b.before.chars().count(), b.after.chars().count());
            if after.div_ceil(LINE) > before.div_ceil(LINE) {
                p.push(format!(
                    "bullet `{}`: {after} chars wraps past {before} ({} lines)",
                    b.id,
                    before.div_ceil(LINE)
                ));
            }
            if b.sources.first() != Some(&b.id) {
                p.push(format!("bullet `{}`: sources don't start with it", b.id));
            }
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

fn percentile(xs: &[f64], q: f64) -> f64 {
    let rank = ((q * xs.len() as f64).ceil() as usize).clamp(1, xs.len());
    xs[rank - 1]
}

fn mean(xs: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, n) = xs.fold((0.0, 0usize), |(s, n), x| (s + x, n + 1));
    (n > 0).then(|| sum / n as f64)
}

fn opt(x: Option<f64>, digits: usize) -> String {
    x.map_or("n/a".to_owned(), |x| format!("{x:.digits$}"))
}

fn report(
    dir: &str,
    records: &BTreeMap<String, Record>,
    bank: &Bank,
    resume: &Resume,
    postings: &[(String, Posting)],
    requirements: impl Fn(&str) -> Requirements,
) -> String {
    let mut md = format!("# Suggest runs: `{dir}`\n\n");
    let n = records.len();
    let count = |o: Outcome| records.values().filter(|r| r.outcome == o).count();
    let mut lat: Vec<f64> = records
        .values()
        .map(|r| r.latency_ms as f64 / 1000.0)
        .collect();
    lat.sort_by(f64::total_cmp);
    let _ = writeln!(
        md,
        "{n} runs: {} valid first try, {} after repair, {} failed. Latency p50 {} s, p90 {} s. Mean output tokens {}, \
         mean cost ${} (API prices), mean input {} chars.\n",
        count(Outcome::Valid),
        count(Outcome::Repaired),
        count(Outcome::Failed),
        if lat.is_empty() {
            "n/a".to_owned()
        } else {
            format!("{:.1}", percentile(&lat, 0.5))
        },
        if lat.is_empty() {
            "n/a".to_owned()
        } else {
            format!("{:.1}", percentile(&lat, 0.9))
        },
        opt(
            mean(
                records
                    .values()
                    .filter_map(|r| r.output_tokens.map(|t| t as f64))
            ),
            0
        ),
        opt(mean(records.values().filter_map(|r| r.cost_usd)), 4),
        opt(mean(records.values().map(|r| r.input_chars as f64)), 0),
    );
    md.push_str(
        "| posting | outcome | s | out tok | $ | verdict | evidence y/a/n | keywords | bullets (variant) | skills | bank | \
         order | summary | gaps | problems |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    let mut details = String::new();
    for (id, r) in records {
        let parsed = r
            .output
            .as_ref()
            .and_then(|v| serde_json::from_value::<Suggestions>(v.clone()).ok());
        let row = match &parsed {
            None => format!(
                "| {id} | {} | {:.1} | {} | {} | – | – | – | – | – | – | – | – | – | – |",
                label_of(r.outcome),
                r.latency_ms as f64 / 1000.0,
                r.output_tokens.map_or("n/a".to_owned(), |t| t.to_string()),
                opt(r.cost_usd, 4),
            ),
            Some(s) => {
                let have = |h: Have| s.evidence.iter().filter(|e| e.have == h).count();
                let posting = postings
                    .iter()
                    .find(|(p, _)| p == id)
                    .map_or("", |(_, p)| p.text.as_str());
                let lines = Lines::new(resume, bank, &r.candidates);
                let probs = problems(s, &lines, &requirements(id), posting);
                if !probs.is_empty() {
                    let _ = writeln!(details, "\n### {id}\n");
                    for p in &probs {
                        let _ = writeln!(details, "- {p}");
                    }
                }
                format!(
                    "| {id} | {} | {:.1} | {} | {} | {} | {}/{}/{} | {} | {} ({}) | {} | {} | {} | {} | {} | {} |",
                    label_of(r.outcome),
                    r.latency_ms as f64 / 1000.0,
                    r.output_tokens.map_or("n/a".to_owned(), |t| t.to_string()),
                    opt(r.cost_usd, 4),
                    serde_json::to_value(s.verdict)
                        .unwrap()
                        .as_str()
                        .unwrap_or("?"),
                    have(Have::Yes),
                    have(Have::Add),
                    have(Have::No),
                    s.keywords.len(),
                    s.bullets.len(),
                    s.bullets.iter().filter(|b| b.variant.is_some()).count(),
                    s.skills.len(),
                    s.bank.len(),
                    s.order.len(),
                    if s.summary.is_some() { "yes" } else { "–" },
                    s.gaps.len(),
                    probs.len(),
                )
            }
        };
        md.push_str(&row);
        md.push('\n');
    }
    if !details.is_empty() {
        md.push_str("\n## Problems code can see\n");
        md.push_str(&details);
    }
    md
}
