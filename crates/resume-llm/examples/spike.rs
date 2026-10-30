//! Extract spike: Extract through an agent CLI (`claude -p` unless `--agent` names another) on every fixture posting,
//! sequentially, compared with a baseline run.
//!
//! `cargo run -p resume-llm --example spike -- [--agent claude|codex|antigravity] [--model <the agent's default>]
//! [--only <id>] [--baseline <dir>] [--out <dir>] [--no-golden] [--report]`
//!
//! Each fixture becomes a `Posting`: front matter `method: pasted text`, or no front matter, is pasted text with no
//! metadata; any other method passes the front matter's title, company and location as hints. Writes each run to
//! `<out>/<model>/<id>.json` (default out `local/m2/extract`), copies its output to
//! `fixtures/requirements/<model>/<id>.json` (golden-test inputs) unless `--no-golden`, and rewrites
//! `<out>/report.md`, comparing the runs recorded there with the baseline's (default `local/m0/sonnet`, the M0
//! spike's sonnet runs). `--report` only rewrites the report.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use resume_core::{Method, Posting};
use resume_llm::extract;
use resume_llm::{Agent, AgentKind, Attempt, LlmError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const TIMEOUT: Duration = Duration::from_secs(180);

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

struct Fixture {
    id: String,
    posting: Posting,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Outcome {
    Valid,
    Repaired,
    Failed,
}

/// One run as recorded in `<dir>/<id>.json`. Reads the M0 spike's records too (their extra fields are ignored).
#[derive(Serialize, Deserialize)]
struct Record {
    id: String,
    model: String,
    outcome: Outcome,
    error: Option<String>,
    latency_ms: u64,
    cost_usd: Option<f64>,
    output_tokens: Option<u64>,
    output: Option<Value>,
    attempts: Vec<Value>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let mut agent = AgentKind::Claude;
    let mut model = None;
    let mut only = None;
    let mut baseline = root.join("local/m0/sonnet");
    let mut out_dir = root.join("local/m2/extract");
    let mut golden = true;
    let mut report_only = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--agent" => agent = args.next().expect("--agent needs a name").parse().unwrap(),
            "--model" => model = Some(args.next().expect("--model needs a name")),
            "--only" => only = Some(args.next().expect("--only needs an id")),
            "--baseline" => baseline = PathBuf::from(args.next().expect("--baseline needs a dir")),
            "--out" => out_dir = PathBuf::from(args.next().expect("--out needs a dir")),
            "--no-golden" => golden = false,
            "--report" => report_only = true,
            _ => panic!(
                "unknown argument {a}; usage: spike [--agent claude|codex|antigravity] [--model <name>] [--only <id>] \
                 [--baseline <dir>] [--out <dir>] [--no-golden] [--report]"
            ),
        }
    }
    let model = model.unwrap_or_else(|| agent.default_model().to_owned());

    let fixtures = fixtures(&root.join("fixtures/postings"));
    let raw_dir = out_dir.join(&model);
    std::fs::create_dir_all(&raw_dir).unwrap();

    if !report_only {
        let chosen: Vec<&Fixture> = fixtures
            .iter()
            .filter(|f| only.as_ref().is_none_or(|o| *o == f.id))
            .collect();
        assert!(!chosen.is_empty(), "no posting matches");
        let provider = match Agent::discover(agent).await {
            Ok(p) => p.with_timeout(TIMEOUT),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        };
        eprintln!("{}: {}", agent.bin(), provider.bin().display());
        let schema = extract::schema();
        let golden_dir = root.join("fixtures/requirements").join(&model);
        if golden {
            std::fs::create_dir_all(&golden_dir).unwrap();
        }
        for f in chosen {
            eprint!("{model} {} … ", f.id);
            let started = Instant::now();
            let result = extract::extract(&provider, &schema, &f.posting, &model).await;
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
            let first = error.as_deref().and_then(|e| e.lines().next());
            eprintln!(
                "{} {:.1}s{}",
                label_of(outcome),
                latency.as_secs_f64(),
                first.map(|e| format!(" ({e})")).unwrap_or_default()
            );
            if let (Some(v), true, true) = (&output, outcome != Outcome::Failed, golden) {
                write_json(&golden_dir.join(format!("{}.json", f.id)), v);
            }
            let costs: Vec<f64> = attempts.iter().filter_map(|a| a.usage.cost_usd).collect();
            let tokens: Vec<u64> = attempts
                .iter()
                .filter_map(|a| a.usage.output_tokens)
                .collect();
            let record = Record {
                id: f.id.clone(),
                model: model.clone(),
                outcome,
                error,
                latency_ms: latency.as_millis() as u64,
                cost_usd: (!costs.is_empty()).then(|| costs.iter().sum()),
                output_tokens: (!tokens.is_empty()).then(|| tokens.iter().sum()),
                output,
                attempts: attempts.iter().map(attempt_json).collect(),
            };
            write_json(
                &raw_dir.join(format!("{}.json", f.id)),
                &serde_json::to_value(&record).unwrap(),
            );
        }
    }

    let md = render_report(
        &fixtures,
        (&display(&root, &baseline), &recorded(&baseline)),
        (&display(&root, &raw_dir), &recorded(&raw_dir)),
    );
    std::fs::write(out_dir.join("report.md"), &md).unwrap();
    println!("{md}");
}

/// Every posting fixture as a `Posting`, in id order.
fn fixtures(dir: &Path) -> Vec<Fixture> {
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
            let (front, body) = split_front_matter(&text);
            let field = |key: &str| {
                front
                    .lines()
                    .find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix(':')))
                    .map(|v| v.trim().to_owned())
                    .filter(|v| !v.is_empty())
            };
            let id = field("id")
                .unwrap_or_else(|| path.file_stem().unwrap().to_string_lossy().into_owned());
            let method = field("method").map_or(Method::Pasted, |m| method_of(&m));
            let posting = match method {
                Method::Pasted => Posting::pasted(body.to_owned()),
                m => {
                    let source = field("source");
                    Posting::new(
                        m,
                        source.clone(),
                        source,
                        field("title"),
                        field("company"),
                        field("location"),
                        body.to_owned(),
                    )
                }
            };
            Fixture { id, posting }
        })
        .collect()
}

/// The front matter's `method` ("greenhouse board API", "applicantpro JSON-LD", "pasted text") as a `Method`.
fn method_of(s: &str) -> Method {
    let s = s.to_lowercase();
    let has = |k: &str| s.contains(k);
    if has("pasted") {
        Method::Pasted
    } else if has("greenhouse") {
        Method::Greenhouse
    } else if has("lever") {
        Method::Lever
    } else if has("ashby") {
        Method::Ashby
    } else if has("linkedin") {
        Method::Linkedin
    } else if has("json-ld") {
        Method::JsonLd
    } else if has("html") {
        Method::Html
    } else {
        panic!("unknown method `{s}`")
    }
}

fn split_front_matter(text: &str) -> (&str, &str) {
    let Some(rest) = text.strip_prefix("---\n") else {
        return ("", text);
    };
    match rest.find("\n---\n") {
        Some(end) => (&rest[..end], &rest[end + 5..]),
        None => ("", text),
    }
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

fn write_json(path: &Path, v: &Value) {
    let mut s = serde_json::to_string_pretty(v).unwrap();
    s.push('\n');
    std::fs::write(path, s).unwrap();
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

/// What the report measures in one output.
struct Metrics {
    skills: usize,
    /// Terms not found case-insensitively in the posting text.
    non_literal: Vec<String>,
    p1: usize,
    /// Priority 1 but not required.
    p1_optional: Vec<String>,
    soft: Vec<String>,
    /// A soft skill listed before a hard, tool or domain one.
    soft_early: bool,
    person: Vec<String>,
    /// Group id → members, with `R` on required ones.
    groups: BTreeMap<String, Vec<String>>,
    /// Groups whose members disagree on `required` or `priority`.
    uneven_groups: Vec<String>,
    role: String,
    title_terms: Vec<String>,
    /// Title terms found neither in the role nor in the posting text, case-insensitively.
    title_non_literal: Vec<String>,
    company: String,
    location: String,
    seniority: String,
    knockouts: Vec<String>,
    degree: String,
    years_min: String,
}

fn measure(posting: &Posting, v: &Value) -> Metrics {
    let text = posting.text.to_lowercase();
    let s = |key: &str| v[key].as_str().unwrap_or("").to_owned();
    let skills = v["skills"].as_array().cloned().unwrap_or_default();
    let term = |k: &Value| k["term"].as_str().unwrap_or("").to_owned();
    let is_soft = |k: &Value| k["kind"] == "soft";
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut group_keys: BTreeMap<String, Vec<(bool, i64)>> = BTreeMap::new();
    for k in &skills {
        if let Some(g) = k["group"].as_str() {
            let required = k["required"] == true;
            let mut t = term(k);
            if required {
                t.push_str(" (R)");
            }
            groups.entry(g.to_owned()).or_default().push(t);
            group_keys
                .entry(g.to_owned())
                .or_default()
                .push((required, k["priority"].as_i64().unwrap_or(0)));
        }
    }
    let first_soft = skills.iter().position(is_soft);
    let last_hard = skills.iter().rposition(|k| !is_soft(k));
    let degree = match &v["degree"] {
        Value::Null => "none".to_owned(),
        d => {
            let level = d["level"].as_str().unwrap_or("?");
            match d.get("equivalent") {
                Some(Value::Bool(true)) => format!("{level}, or equivalent"),
                Some(Value::Bool(false)) => level.to_owned(),
                _ => format!("{level} (no `equivalent`)"),
            }
        }
    };
    Metrics {
        skills: skills.len(),
        non_literal: skills
            .iter()
            .map(term)
            .filter(|t| !text.contains(&t.to_lowercase()))
            .collect(),
        p1: skills.iter().filter(|k| k["priority"] == 1).count(),
        p1_optional: skills
            .iter()
            .filter(|k| k["priority"] == 1 && k["required"] != true)
            .map(term)
            .collect(),
        soft: skills.iter().filter(|k| is_soft(k)).map(term).collect(),
        soft_early: matches!((first_soft, last_hard), (Some(f), Some(l)) if f < l),
        person: skills
            .iter()
            .map(term)
            .filter(|t| describes_person(t))
            .collect(),
        uneven_groups: group_keys
            .iter()
            .filter(|(_, ks)| ks.windows(2).any(|w| w[0] != w[1]))
            .map(|(g, _)| g.clone())
            .collect(),
        groups,
        title_terms: strings(&v["title_terms"]),
        title_non_literal: strings(&v["title_terms"])
            .into_iter()
            .filter(|t| {
                let t = t.to_lowercase();
                !text.contains(&t) && !s("role").to_lowercase().contains(&t)
            })
            .collect(),
        role: s("role"),
        company: s("company"),
        location: s("location"),
        seniority: s("seniority"),
        knockouts: v["knockouts"]
            .as_array()
            .map(|ks| {
                ks.iter()
                    .map(|k| {
                        format!(
                            "{}: {}",
                            k["kind"].as_str().unwrap_or("?"),
                            k["text"].as_str().unwrap_or("")
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        degree,
        years_min: v["years_min"]
            .as_u64()
            .map_or("none".to_owned(), |y| y.to_string()),
    }
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|xs| {
            xs.iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
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

/// Nearest-rank percentile of `xs` (sorted ascending).
fn percentile(xs: &[f64], p: f64) -> f64 {
    let rank = ((p * xs.len() as f64).ceil() as usize).clamp(1, xs.len());
    xs[rank - 1]
}

fn mean(xs: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, n) = xs.fold((0.0, 0usize), |(s, n), x| (s + x, n + 1));
    (n > 0).then(|| sum / n as f64)
}

fn opt(x: Option<f64>, digits: usize) -> String {
    x.map_or("n/a".to_owned(), |x| format!("{x:.digits$}"))
}

fn cell(xs: &[String]) -> String {
    if xs.is_empty() {
        "–".to_owned()
    } else {
        xs.join("; ").replace('|', "\\|")
    }
}

/// One set of runs (baseline or new), measured.
struct Set<'a> {
    label: &'a str,
    runs: &'a BTreeMap<String, Record>,
    metrics: BTreeMap<&'a str, Metrics>,
}

impl<'a> Set<'a> {
    fn new(fixtures: &'a [Fixture], label: &'a str, runs: &'a BTreeMap<String, Record>) -> Self {
        let metrics = fixtures
            .iter()
            .filter_map(|f| {
                let out = runs.get(&f.id)?.output.as_ref()?;
                Some((f.id.as_str(), measure(&f.posting, out)))
            })
            .collect();
        Self {
            label,
            runs,
            metrics,
        }
    }

    /// The summary column, one value per row of [`SUMMARY_ROWS`].
    fn summary(&self) -> Vec<String> {
        let runs: Vec<&Record> = self.runs.values().collect();
        let count = |o| runs.iter().filter(|r| r.outcome == o).count();
        let mut lat: Vec<f64> = runs.iter().map(|r| r.latency_ms as f64 / 1000.0).collect();
        lat.sort_by(f64::total_cmp);
        let ms: Vec<&Metrics> = self.metrics.values().collect();
        let skills: usize = ms.iter().map(|m| m.skills).sum();
        let non_literal: usize = ms.iter().map(|m| m.non_literal.len()).sum();
        let p1: usize = ms.iter().map(|m| m.p1).sum();
        let max_p1 = ms
            .iter()
            .map(|m| m.p1 as f64 / m.skills.max(1) as f64)
            .fold(0.0, f64::max);
        let soft: usize = ms.iter().map(|m| m.soft.len()).sum();
        let lat_p = |p| {
            if lat.is_empty() {
                "n/a".to_owned()
            } else {
                format!("{:.1}", percentile(&lat, p))
            }
        };
        vec![
            runs.len().to_string(),
            count(Outcome::Valid).to_string(),
            count(Outcome::Repaired).to_string(),
            count(Outcome::Failed).to_string(),
            lat_p(0.5),
            lat_p(0.9),
            opt(mean(runs.iter().filter_map(|r| r.cost_usd)), 4),
            opt(
                mean(
                    runs.iter()
                        .filter_map(|r| r.output_tokens.map(|t| t as f64)),
                ),
                0,
            ),
            opt(mean(ms.iter().map(|m| m.skills as f64)), 1),
            format!(
                "{:.3} ({non_literal} of {skills} not found)",
                1.0 - non_literal as f64 / skills.max(1) as f64
            ),
            format!("{:.2} (max {max_p1:.2})", p1 as f64 / skills.max(1) as f64),
            ms.iter()
                .map(|m| m.p1_optional.len())
                .sum::<usize>()
                .to_string(),
            format!(
                "{soft} (mean {:.1}, max {})",
                soft as f64 / ms.len().max(1) as f64,
                ms.iter().map(|m| m.soft.len()).max().unwrap_or(0)
            ),
            ms.iter().filter(|m| m.soft_early).count().to_string(),
            ms.iter().map(|m| m.person.len()).sum::<usize>().to_string(),
            ms.iter().map(|m| m.groups.len()).sum::<usize>().to_string(),
            ms.iter()
                .map(|m| m.knockouts.len())
                .sum::<usize>()
                .to_string(),
        ]
    }
}

const SUMMARY_ROWS: [&str; 17] = [
    "Runs",
    "Valid first try",
    "Valid after repair",
    "Failed",
    "p50 latency s",
    "p90 latency s",
    "Mean cost $",
    "Mean output tokens",
    "Mean skills",
    "Literal-term rate",
    "Share of priority 1 (worst posting)",
    "Priority 1 but not required",
    "Soft skills",
    "Runs with a soft skill before a technical one",
    "Person-describing terms",
    "One-of groups",
    "Knockouts",
];

fn render_report(
    fixtures: &[Fixture],
    baseline: (&str, &BTreeMap<String, Record>),
    new: (&str, &BTreeMap<String, Record>),
) -> String {
    let base = Set::new(fixtures, baseline.0, baseline.1);
    let new = Set::new(fixtures, new.0, new.1);
    let mut md = String::from("# M2 Extract spike: baseline vs new prompt\n\n");
    writeln!(
        md,
        "Baseline: `{}`. New: `{}`. Latency is wall time per run (both turns when repaired). Cost is what Claude \
         Code reports at API prices; on a subscription it is a measure, not a bill. Literal-term rate: share of \
         `skills[].term` found case-insensitively in the posting text. Person-describing terms: a word like \
         \"communicator\", \"self-starter\", \"solver\", or a leading \"Strong\", \"Pragmatic\", \"Skilled\"… \
         (heuristic; soft terms are listed per posting below for review).\n",
        base.label, new.label
    )
    .unwrap();
    md.push_str("| Metric | Baseline | New |\n|---|---|---|\n");
    for (row, (b, n)) in SUMMARY_ROWS
        .iter()
        .zip(base.summary().into_iter().zip(new.summary()))
    {
        writeln!(md, "| {row} | {b} | {n} |").unwrap();
    }

    let per_posting = |md: &mut String, title: &str, f: &dyn Fn(&Metrics) -> String| {
        writeln!(
            md,
            "\n## {title}\n\n| Posting | Baseline | New |\n|---|---|---|"
        )
        .unwrap();
        for fx in fixtures {
            let get = |s: &Set| {
                s.metrics
                    .get(fx.id.as_str())
                    .map_or("(no run)".to_owned(), f)
                    .replace('|', "\\|")
            };
            writeln!(md, "| `{}` | {} | {} |", fx.id, get(&base), get(&new)).unwrap();
        }
    };
    md.push_str("\n## Hints passed\n\n| Posting | Method | Hints |\n|---|---|---|\n");
    for fx in fixtures {
        let p = &fx.posting;
        let hints: Vec<String> = [
            ("title", &p.title),
            ("company", &p.company),
            ("location", &p.location),
        ]
        .into_iter()
        .filter_map(|(k, v)| v.as_ref().map(|v| format!("{k}: {v}")))
        .collect();
        writeln!(
            md,
            "| `{}` | {} | {} |",
            fx.id,
            serde_json::to_value(p.method)
                .unwrap()
                .as_str()
                .unwrap_or(""),
            cell(&hints)
        )
        .unwrap();
    }
    per_posting(&mut md, "Role · company · location", &|m| {
        format!(
            "{} · {} · {}",
            m.role,
            empty(&m.company),
            empty(&m.location)
        )
    });
    per_posting(&mut md, "Title terms · not in the role or text", &|m| {
        format!(
            "{} · not literal: {}",
            cell(&m.title_terms),
            cell(&m.title_non_literal)
        )
    });
    per_posting(&mut md, "Seniority · years · degree", &|m| {
        format!("{} · {} · {}", m.seniority, m.years_min, m.degree)
    });
    per_posting(&mut md, "Knockouts", &|m| cell(&m.knockouts));
    per_posting(&mut md, "Skills · priority 1 · not literal", &|m| {
        format!(
            "{} skills, {} p1 ({:.0}%){} · not literal: {}",
            m.skills,
            m.p1,
            100.0 * m.p1 as f64 / m.skills.max(1) as f64,
            if m.p1_optional.is_empty() {
                String::new()
            } else {
                format!(", p1 not required: {}", m.p1_optional.join(", "))
            },
            cell(&m.non_literal)
        )
    });
    per_posting(&mut md, "Soft skills · person-describing", &|m| {
        format!(
            "{}{} · person: {}",
            cell(&m.soft),
            if m.soft_early {
                " (before a technical one)"
            } else {
                ""
            },
            cell(&m.person)
        )
    });
    per_posting(&mut md, "One-of groups", &|m| {
        let gs: Vec<String> = m
            .groups
            .iter()
            .map(|(g, ts)| {
                let uneven = if m.uneven_groups.contains(g) {
                    " (members disagree)"
                } else {
                    ""
                };
                format!("{g}: {}{uneven}", ts.join(", "))
            })
            .collect();
        cell(&gs)
    });
    for (set, label) in [(&base, "Baseline"), (&new, "New")] {
        for r in set.runs.values().filter(|r| r.error.is_some()) {
            let e = r.error.as_deref().unwrap_or("");
            writeln!(
                md,
                "\n{label} `{}` error: {}",
                r.id,
                e.lines().next().unwrap_or("")
            )
            .unwrap();
        }
    }
    md
}

fn empty(s: &str) -> &str {
    if s.is_empty() { "\"\"" } else { s }
}
