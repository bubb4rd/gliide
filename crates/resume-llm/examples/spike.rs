//! M0 provider spike: Extract through `claude -p` on every fixture posting, once per model, sequentially.
//!
//! `cargo run -p resume-llm --example spike -- [--models haiku,sonnet] [--only <id>]`
//!
//! A model may name a `claude --effort` level: `haiku:low` runs haiku with `--effort low`, labeled `haiku-low`.
//! Writes each run to `local/m0/<label>/<id>.json`, copies schema-valid outputs to
//! `fixtures/requirements/<label>/<id>.json` (golden-test inputs), and rewrites `local/m0/report.md` from every run
//! recorded under `local/m0/`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use resume_llm::{Attempt, ClaudeCli, LlmError, extract};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const TIMEOUT: Duration = Duration::from_secs(180);

struct Posting {
    id: String,
    location: String,
    body: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Outcome {
    Valid,
    Repaired,
    Failed,
}

/// One run's measurements, as recorded in `local/m0/<label>/<id>.json` beside its output and attempts.
#[derive(Serialize, Deserialize)]
struct Run {
    id: String,
    label: String,
    model: String,
    effort: Option<String>,
    outcome: Outcome,
    error: Option<String>,
    latency_ms: u64,
    cost_usd: Option<f64>,
    output_tokens: Option<u64>,
    skills: usize,
    literal_rate: Option<f64>,
    non_literal: Vec<String>,
    flagged: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Record {
    #[serde(flatten)]
    run: Run,
    output: Option<Value>,
    attempts: Vec<Value>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut models = vec!["haiku".to_owned(), "sonnet".to_owned()];
    let mut only = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--models" => {
                let list = args.next().expect("--models needs a list");
                models = list.split(',').map(str::to_owned).collect();
            }
            "--only" => only = Some(args.next().expect("--only needs an id")),
            _ => panic!(
                "unknown argument {a}; usage: spike [--models haiku,sonnet:low] [--only <id>]"
            ),
        }
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let postings: Vec<Posting> = postings(&root.join("fixtures/postings"))
        .into_iter()
        .filter(|p| only.as_ref().is_none_or(|o| *o == p.id))
        .collect();
    assert!(!postings.is_empty(), "no posting matches");

    let base = match ClaudeCli::discover().await {
        Ok(p) => p.with_timeout(TIMEOUT),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    eprintln!("claude: {}", base.bin().display());
    let schema = extract::schema();
    let out_dir = root.join("local/m0");

    for spec in &models {
        let (model, effort) = match spec.split_once(':') {
            Some((m, e)) => (m, Some(e)),
            None => (spec.as_str(), None),
        };
        let label = effort.map_or(model.to_owned(), |e| format!("{model}-{e}"));
        let provider = effort.map_or(base.clone(), |e| base.clone().with_effort(e));
        let raw_dir = out_dir.join(&label);
        let golden_dir = root.join("fixtures/requirements").join(&label);
        std::fs::create_dir_all(&raw_dir).unwrap();
        std::fs::create_dir_all(&golden_dir).unwrap();
        for p in &postings {
            eprint!("{label} {} … ", p.id);
            let started = Instant::now();
            let result = extract::extract(&provider, &schema, &p.body, Some(model)).await;
            let latency = started.elapsed();

            let (outcome, output, attempts, error) = match result {
                Ok(s) => {
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
                        _ => (Outcome::Failed, None, Vec::new(), Some(msg)),
                    }
                }
            };
            let mut run = measure(p, output.as_ref(), &attempts);
            run.label.clone_from(&label);
            run.model = model.to_owned();
            run.effort = effort.map(str::to_owned);
            run.outcome = outcome;
            run.error = error;
            run.latency_ms = latency.as_millis() as u64;
            let first = run.error.as_deref().and_then(|e| e.lines().next());
            eprintln!(
                "{} {:.1}s{}",
                label_of(outcome),
                latency.as_secs_f64(),
                first.map(|e| format!(" ({e})")).unwrap_or_default()
            );

            if let Some(v) = &output {
                write_json(&golden_dir.join(format!("{}.json", p.id)), v);
            }
            let record = Record {
                run,
                output,
                attempts: attempts.iter().map(attempt_json).collect(),
            };
            write_json(
                &raw_dir.join(format!("{}.json", p.id)),
                &serde_json::to_value(&record).unwrap(),
            );
        }
    }

    let md = render_report(&recorded(&out_dir));
    std::fs::write(out_dir.join("report.md"), &md).unwrap();
    println!("{md}");
}

/// Every posting fixture, front matter stripped.
fn postings(dir: &Path) -> Vec<Posting> {
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
                    .unwrap_or_default()
            };
            let id = field("id");
            let id = if id.is_empty() {
                path.file_stem().unwrap().to_string_lossy().into_owned()
            } else {
                id
            };
            Posting {
                id,
                location: field("location"),
                body: body.to_owned(),
            }
        })
        .collect()
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

/// The output's measurements; the caller fills in who ran it and how it went.
fn measure(p: &Posting, output: Option<&Value>, attempts: &[Attempt]) -> Run {
    let costs: Vec<f64> = attempts.iter().filter_map(|a| a.usage.cost_usd).collect();
    let tokens: Vec<u64> = attempts
        .iter()
        .filter_map(|a| a.usage.output_tokens)
        .collect();
    let terms: Vec<&str> = output
        .and_then(|v| v["skills"].as_array())
        .map(|s| s.iter().filter_map(|k| k["term"].as_str()).collect())
        .unwrap_or_default();
    let body = p.body.to_lowercase();
    let non_literal: Vec<String> = terms
        .iter()
        .filter(|t| !body.contains(&t.to_lowercase()))
        .map(|t| (*t).to_owned())
        .collect();
    let flagged = terms
        .iter()
        .filter_map(|t| flag(t, &p.location).map(|why| format!("{t} ({why})")))
        .collect();
    Run {
        id: p.id.clone(),
        label: String::new(),
        model: String::new(),
        effort: None,
        outcome: Outcome::Failed,
        error: None,
        latency_ms: 0,
        cost_usd: (!costs.is_empty()).then(|| costs.iter().sum()),
        output_tokens: (!tokens.is_empty()).then(|| tokens.iter().sum()),
        skills: terms.len(),
        literal_rate: (!terms.is_empty())
            .then(|| 1.0 - non_literal.len() as f64 / terms.len() as f64),
        non_literal,
        flagged,
    }
}

/// Why a skill term breaks the "never a sentence, years, degree or location" rule, if it does.
fn flag(term: &str, location: &str) -> Option<&'static str> {
    let t = term.to_lowercase();
    if term.split_whitespace().count() > 5 || t.ends_with('.') || t.contains(';') {
        return Some("sentence");
    }
    if (t.contains("year") || t.contains("yrs")) && t.chars().any(|c| c.is_ascii_digit()) {
        return Some("years");
    }
    const DEGREE: [&str; 9] = [
        "degree",
        "bachelor",
        "master's",
        "masters",
        "b.s.",
        "m.s.",
        "phd",
        "ph.d",
        "associate's",
    ];
    if DEGREE.iter().any(|d| t.contains(d)) {
        return Some("degree");
    }
    const PLACE: [&str; 6] = [
        "remote",
        "on-site",
        "onsite",
        "hybrid",
        "relocat",
        "in-office",
    ];
    let city = location
        .split([',', '-', '(', ')', '/'])
        .map(|s| s.trim().to_lowercase())
        .filter(|s| s.len() >= 3 && !["remote", "fully remote", "us", "usa"].contains(&s.as_str()))
        .any(|s| t.contains(&s));
    if PLACE.iter().any(|w| t.contains(w)) || city {
        return Some("location");
    }
    None
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

/// Every run recorded under `dir`, by label, in posting order.
fn recorded(dir: &Path) -> BTreeMap<String, Vec<Run>> {
    let mut by_label: BTreeMap<String, Vec<Run>> = BTreeMap::new();
    for label_dir in std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
    {
        for file in std::fs::read_dir(&label_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
        {
            if file.extension().is_some_and(|e| e == "json") {
                let record: Record = serde_json::from_str(&std::fs::read_to_string(&file).unwrap())
                    .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
                by_label
                    .entry(record.run.label.clone())
                    .or_default()
                    .push(record.run);
            }
        }
    }
    for runs in by_label.values_mut() {
        runs.sort_by(|a, b| a.id.cmp(&b.id));
    }
    by_label
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

fn render_report(report: &BTreeMap<String, Vec<Run>>) -> String {
    let mut md = String::from("# M0 spike: Extract through `claude -p`\n\n");
    md.push_str(
        "Latency is wall time per run (both turns when repaired). Cost is what Claude Code reports at API prices; on \
         a subscription it is a measure, not a bill. Literal-term rate: share of `skills[].term` found \
         case-insensitively in the posting. A label `<model>-<level>` ran with `claude --effort <level>`.\n\n",
    );
    md.push_str(
        "| Model | Runs | Valid first try | After repair | Failed | p50 s | p90 s | Mean cost $ | Mean output tokens \
         | Mean skills | Mean literal rate | Runs with flagged skills |\n|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for (label, runs) in report {
        let count = |o| runs.iter().filter(|r| r.outcome == o).count();
        let mut lat: Vec<f64> = runs.iter().map(|r| r.latency_ms as f64 / 1000.0).collect();
        lat.sort_by(f64::total_cmp);
        let ok: Vec<&Run> = runs
            .iter()
            .filter(|r| r.outcome != Outcome::Failed)
            .collect();
        writeln!(
            md,
            "| {label} | {} | {} | {} | {} | {:.1} | {:.1} | {} | {} | {} | {} | {} |",
            runs.len(),
            count(Outcome::Valid),
            count(Outcome::Repaired),
            count(Outcome::Failed),
            percentile(&lat, 0.5),
            percentile(&lat, 0.9),
            opt(mean(runs.iter().filter_map(|r| r.cost_usd)), 4),
            opt(
                mean(
                    runs.iter()
                        .filter_map(|r| r.output_tokens.map(|t| t as f64))
                ),
                0
            ),
            opt(mean(ok.iter().map(|r| r.skills as f64)), 1),
            opt(mean(ok.iter().filter_map(|r| r.literal_rate)), 3),
            ok.iter().filter(|r| !r.flagged.is_empty()).count(),
        )
        .unwrap();
    }
    for (label, runs) in report {
        writeln!(
            md,
            "\n## {label}\n\n| Posting | Outcome | Latency s | Cost $ | Skills | Literal rate | Not in posting | Flagged |\n\
             |---|---|---|---|---|---|---|---|"
        )
        .unwrap();
        for r in runs {
            writeln!(
                md,
                "| `{}` | {} | {:.1} | {} | {} | {} | {} | {} |",
                r.id,
                label_of(r.outcome),
                r.latency_ms as f64 / 1000.0,
                opt(r.cost_usd, 4),
                r.skills,
                opt(r.literal_rate, 2),
                cell(&r.non_literal),
                cell(&r.flagged),
            )
            .unwrap();
        }
        for r in runs.iter().filter(|r| r.error.is_some()) {
            let e = r.error.as_deref().unwrap_or("");
            writeln!(md, "\n`{}` error: {}", r.id, e.lines().next().unwrap_or("")).unwrap();
        }
    }
    md
}

fn cell(xs: &[String]) -> String {
    if xs.is_empty() {
        String::new()
    } else {
        xs.join(", ").replace('|', "\\|")
    }
}
