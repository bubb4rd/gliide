//! `ez eval`: each role's calls on the fixture postings, on every route asked for, one call at a time, then a report
//! of how the routes compare (`local/routing/PLAN.md` §3).
//!
//! Each run is recorded as `<out>/<role>/<agent>-<model>/<posting>/<n>.json`: the outcome, wall time, tokens, cost,
//! the output, and every turn's raw reply. A run already recorded is skipped, so an interrupted eval resumes where it
//! stopped. `report.md` and `routes.json` are rebuilt from the recorded runs alone, after every role command and by
//! `ez eval report`. Paths are relative to the working folder: run it from the repo root.

pub mod coach;
pub mod read;
pub mod report;
pub mod tailor;
pub mod write;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use clap::Subcommand;
use resume_core::{Bank, Master, Method, Posting, Requirements, Resume, migrate_v1, resolve};
use resume_llm::pricing::{Tokens, price, reported_cost};
use resume_llm::{Agent, Attempt, LlmError, Route, Structured};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::job::runtime;

/// The fixtures the eval reads.
const POSTINGS: &str = "fixtures/postings";
const BANK_V1: &str = "fixtures/banks/jane-doe.v1.json";
/// The `swe` master without its project, so bank-aware Suggest has a candidate.
const TRIM_MASTER: &str = "fixtures/banks/jane-doe.swe-trim.master.json";
/// Hand-checked requirements per posting; the Read eval scores against them.
const GOLD: &str = "fixtures/requirements/gold";
/// Sonnet's recorded requirements: the Tailor, Write and Coach input when a posting has no gold yet.
const SONNET: &str = "fixtures/requirements/sonnet";

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Extract on every posting: validity, latency, cost, recall and precision against gold, score drift.
    Read(RunArgs),
    /// Suggest on every posting with its gold requirements: validity, latency, grounding, lift, code checks, and a
    /// blind sheet for keep-or-drop.
    Tailor(TailorArgs),
    /// A cover letter per posting from the resolved `swe` master: validity, latency, length, a blind sheet.
    Write(RunArgs),
    /// Prep for a technical round per posting, and Dig questions per bank entry: validity, latency, question counts,
    /// a blind sheet.
    Coach(RunArgs),
    /// Rebuild report.md and routes.json from the runs recorded under --out.
    Report(OutArg),
}

/// What every role command takes.
#[derive(clap::Args)]
pub struct RunArgs {
    /// Routes to compare, comma-separated `agent:model` (`claude:sonnet,codex:gpt-6-luna`); `agent` alone is its
    /// default model.
    #[arg(long, value_delimiter = ',', required = true)]
    pub routes: Vec<Route>,
    /// Runs per posting and route: 3 for read and tailor, 1 for write and coach, unless given.
    #[arg(long)]
    pub runs: Option<u32>,
    /// Only these posting ids, comma-separated (Dig's entries are `dig-<entry id>`).
    #[arg(long, value_delimiter = ',')]
    pub only: Vec<String>,
    #[command(flatten)]
    pub out: OutArg,
}

#[derive(clap::Args)]
pub struct TailorArgs {
    #[command(flatten)]
    pub run: RunArgs,
    /// What Suggest may draw from: the trimmed master plus its bank candidates (recorded under `tailor/`), or the
    /// full `swe` master alone (recorded under `tailor-master-only/`).
    #[arg(long, value_enum, default_value = "bank-aware")]
    pub mode: ModeArg,
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum ModeArg {
    BankAware,
    MasterOnly,
}

#[derive(clap::Args)]
pub struct OutArg {
    /// Folder for the runs, report.md and routes.json: `local/eval/<YYYY-MM-DD>` unless given.
    #[arg(long)]
    pub out: Option<PathBuf>,
}

impl OutArg {
    pub fn dir(&self) -> PathBuf {
        self.out
            .clone()
            .unwrap_or_else(|| Path::new("local/eval").join(date()))
    }
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let fx = Fixtures::load()?;
    let out = match &args.command {
        Command::Read(a) | Command::Write(a) | Command::Coach(a) => a.out.dir(),
        Command::Tailor(a) => a.run.out.dir(),
        Command::Report(o) => o.dir(),
    };
    let rt = runtime()?;
    match &args.command {
        Command::Read(a) => rt.block_on(read::run(&fx, a, &out))?,
        Command::Tailor(a) => rt.block_on(tailor::run(&fx, a, &out))?,
        Command::Write(a) => rt.block_on(write::run(&fx, a, &out))?,
        Command::Coach(a) => rt.block_on(coach::run(&fx, a, &out))?,
        Command::Report(_) => {}
    }
    report::run(&fx, &out)
}

// ── Fixtures ────────────────────────────────────────────────────────────────────────────────────────────────────────

/// One posting fixture.
pub struct Fixture {
    pub id: String,
    pub posting: Posting,
}

/// Everything the roles read: the postings, the bank migrated from the v1 fixture with its `swe` master, the trimmed
/// master when it exists, and the gold and sonnet requirements by posting id.
pub struct Fixtures {
    pub postings: Vec<Fixture>,
    pub bank: Bank,
    pub master: Master,
    /// `master` resolved.
    pub resume: Resume,
    /// The `swe` master without its project; `None` until the fixture exists.
    pub trim: Option<Master>,
    pub gold: BTreeMap<String, Requirements>,
    pub sonnet: BTreeMap<String, Requirements>,
}

impl Fixtures {
    pub fn load() -> anyhow::Result<Self> {
        let v1: Resume = super::read_json(Path::new(BANK_V1))?;
        let m = migrate_v1(&v1);
        let resume = resolve(&m.bank, &m.master)
            .with_context(|| format!("resolving the `swe` master of {BANK_V1}"))?;
        let trim_path = Path::new(TRIM_MASTER);
        let trim = if trim_path.exists() {
            let t: Master = super::read_json(trim_path)?;
            resolve(&m.bank, &t).with_context(|| format!("resolving {TRIM_MASTER}"))?;
            Some(t)
        } else {
            None
        };
        Ok(Self {
            postings: postings(Path::new(POSTINGS))?,
            bank: m.bank,
            master: m.master,
            resume,
            trim,
            gold: requirements_in(Path::new(GOLD))?,
            sonnet: requirements_in(Path::new(SONNET))?,
        })
    }

    /// The postings `only` names, or all of them.
    pub fn chosen(&self, only: &[String]) -> Vec<&Fixture> {
        self.postings
            .iter()
            .filter(|f| only.is_empty() || only.contains(&f.id))
            .collect()
    }

    /// The requirements a role other than Read gets for posting `id`: gold, else sonnet's; with which it is.
    pub fn requirements(&self, id: &str) -> Option<(&Requirements, &'static str)> {
        self.gold
            .get(id)
            .map(|r| (r, "gold"))
            .or_else(|| self.sonnet.get(id).map(|r| (r, "sonnet")))
    }

    /// Posting `id`.
    pub fn posting(&self, id: &str) -> Option<&Fixture> {
        self.postings.iter().find(|f| f.id == id)
    }
}

/// Every posting fixture as a `Posting`, in id order: front matter `method: pasted text`, or none, is pasted text with
/// no metadata; any other method passes the front matter's title, company and location as hints (as the Extract
/// spike built them).
fn postings(dir: &Path) -> anyhow::Result<Vec<Fixture>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|e| e == "md")
                && p.file_name().is_some_and(|n| n != "README.md")
        })
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let (front, body) = split_front_matter(&text);
            let field = |key: &str| {
                front
                    .lines()
                    .find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix(':')))
                    .map(|v| v.trim().to_owned())
                    .filter(|v| !v.is_empty())
            };
            let id = field("id").unwrap_or_else(|| {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
            let method = match field("method") {
                None => Method::Pasted,
                Some(m) => method_of(&m).with_context(|| format!("{}", path.display()))?,
            };
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
            Ok(Fixture { id, posting })
        })
        .collect()
}

/// The front matter's `method` ("greenhouse board API", "applicantpro JSON-LD", "pasted text") as a `Method`.
fn method_of(s: &str) -> anyhow::Result<Method> {
    let s = s.to_lowercase();
    let has = |k: &str| s.contains(k);
    Ok(if has("pasted") {
        Method::Pasted
    } else if has("greenhouse") {
        Method::Greenhouse
    } else if has("lever") {
        Method::Lever
    } else if has("ashby") {
        Method::Ashby
    } else if has("linkedin") {
        Method::Linkedin
    } else if has("workday") {
        Method::Workday
    } else if has("json-ld") {
        Method::JsonLd
    } else if has("html") {
        Method::Html
    } else {
        bail!("unknown method `{s}`")
    })
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

/// Every `<id>.json` requirements file in `dir`, by id; none when `dir` doesn't exist.
fn requirements_in(dir: &Path) -> anyhow::Result<BTreeMap<String, Requirements>> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(BTreeMap::new());
    };
    let mut out = BTreeMap::new();
    for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
        if path.extension().is_some_and(|e| e == "json")
            && let Some(id) = path.file_stem().and_then(|s| s.to_str())
        {
            out.insert(id.to_owned(), super::read_json(&path)?);
        }
    }
    Ok(out)
}

// ── Runs ────────────────────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// Schema-valid on the first turn.
    Valid,
    /// Valid after the repair turn.
    Repaired,
    Failed,
}

/// Token counts as [`Tokens`] has them, every turn together.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct TokenCounts {
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
}

/// One recorded run: `<out>/<role>/<agent>-<model>/<posting>/<n>.json`.
#[derive(Serialize, Deserialize)]
pub struct Run {
    pub role: String,
    pub route: Route,
    pub posting: String,
    pub n: u32,
    pub outcome: Outcome,
    pub error: Option<String>,
    /// Wall time of the call, both turns when repaired.
    pub latency_ms: u64,
    /// The model id the agent says served the last turn.
    pub served: Option<String>,
    pub tokens: TokenCounts,
    /// What the agent reported, else the tokens at the model's API list price; `None` when neither is known.
    pub cost_usd: Option<f64>,
    /// `reported` or `list price`.
    pub cost_from: Option<String>,
    /// What the role's call read beyond the posting (mode, candidates, which requirements, input size).
    #[serde(default)]
    pub meta: Value,
    pub output: Option<Value>,
    /// Every turn: its latency, usage and raw reply.
    pub attempts: Vec<Value>,
}

impl Run {
    pub fn ok(&self) -> bool {
        self.outcome != Outcome::Failed
    }

    pub fn secs(&self) -> f64 {
        self.latency_ms as f64 / 1000.0
    }
}

/// A call's result, whatever it was.
pub struct Called {
    pub outcome: Outcome,
    pub output: Option<Value>,
    pub attempts: Vec<Attempt>,
    pub error: Option<String>,
    pub latency: Duration,
}

/// Runs `call`, timing it, and keeps whatever it returned: the output and turns of a valid call; the turns and error
/// of one that failed (and the output, when it was schema-valid but didn't decode).
pub async fn call<T>(call: impl Future<Output = Result<(T, Structured), LlmError>>) -> Called {
    let started = Instant::now();
    let result = call.await;
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
    Called {
        outcome,
        output,
        attempts,
        error,
        latency,
    }
}

impl Run {
    fn new(role: &str, route: &Route, posting: &str, n: u32, c: Called, meta: Value) -> Self {
        let t = Tokens::of(&c.attempts);
        let served = resume_llm::served(&c.attempts, &route.model).map(str::to_owned);
        let (cost_usd, cost_from) = match reported_cost(&c.attempts) {
            Some(cost) if !c.attempts.is_empty() => (Some(cost), Some("reported")),
            _ => match price(served.as_deref().unwrap_or(&route.model)) {
                Some(p) if !c.attempts.is_empty() => {
                    (Some(p.cost(&t).iter().sum()), Some("list price"))
                }
                _ => (None, None),
            },
        };
        Self {
            role: role.to_owned(),
            route: route.clone(),
            posting: posting.to_owned(),
            n,
            outcome: c.outcome,
            error: c.error,
            latency_ms: c.latency.as_millis() as u64,
            served,
            tokens: TokenCounts {
                input: t.input,
                output: t.output,
                cache_write: t.cache_write,
                cache_read: t.cache_read,
            },
            cost_usd,
            cost_from: cost_from.map(str::to_owned),
            meta,
            output: c.output,
            attempts: c
                .attempts
                .iter()
                .map(|a| {
                    json!({
                        "latency_ms": a.latency_ms,
                        "usage": a.usage,
                        "raw": serde_json::from_str::<Value>(&a.raw)
                            .unwrap_or_else(|_| Value::String(a.raw.clone())),
                    })
                })
                .collect(),
        }
    }
}

/// `agent-model`, safe as a folder name.
pub fn route_dir(route: &Route) -> String {
    format!("{}-{}", route.agent, route.model)
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn run_path(out: &Path, role: &str, route: &Route, item: &str, n: u32) -> PathBuf {
    out.join(role)
        .join(route_dir(route))
        .join(item)
        .join(format!("{n}.json"))
}

/// Runs `f` for every route, item and run number not recorded yet, one call at a time, and records each. A route
/// whose agent isn't installed is skipped with a message; the others still run.
pub async fn each<T>(
    role: &str,
    args: &RunArgs,
    default_runs: u32,
    timeout: Duration,
    out: &Path,
    items: &[(String, T)],
    mut f: impl AsyncFnMut(&Agent, &Route, &T) -> (Called, Value),
) -> anyhow::Result<()> {
    if items.is_empty() {
        bail!("no posting matches");
    }
    let runs = args.runs.unwrap_or(default_runs).max(1);
    for route in &args.routes {
        let todo = |id: &str, n| !run_path(out, role, route, id, n).exists();
        if !items.iter().any(|(id, _)| (1..=runs).any(|n| todo(id, n))) {
            eprintln!("{role} {route}: every run recorded");
            continue;
        }
        let llm = match Agent::discover(route.agent).await {
            Ok(a) => a.with_timeout(timeout),
            Err(e) => {
                eprintln!("{role} {route}: {e}; skipped");
                continue;
            }
        };
        for (id, item) in items {
            for n in (1..=runs).filter(|&n| todo(id, n)) {
                eprint!("{role} {route} {id} #{n} … ");
                let (c, meta) = f(&llm, route, item).await;
                let run = Run::new(role, route, id, n, c, meta);
                eprintln!(
                    "{} {:.1}s{}",
                    serde_json::to_value(run.outcome)?.as_str().unwrap_or("?"),
                    run.secs(),
                    run.error
                        .as_deref()
                        .and_then(|e| e.lines().next())
                        .map(|e| format!(" ({e})"))
                        .unwrap_or_default()
                );
                super::write_json(&run_path(out, role, route, id, n), &run)?;
            }
        }
    }
    Ok(())
}

/// Every run recorded under `<out>/<role>`, by route, posting and run number.
pub fn recorded(out: &Path, role: &str) -> anyhow::Result<Vec<Run>> {
    let mut runs = Vec::new();
    let Ok(routes) = std::fs::read_dir(out.join(role)) else {
        return Ok(runs);
    };
    for route in routes.filter_map(|e| e.ok().map(|e| e.path())) {
        let Ok(items) = std::fs::read_dir(&route) else {
            continue;
        };
        for item in items.filter_map(|e| e.ok().map(|e| e.path())) {
            let Ok(files) = std::fs::read_dir(&item) else {
                continue;
            };
            for file in files.filter_map(|e| e.ok().map(|e| e.path())) {
                if file.extension().is_some_and(|e| e == "json") {
                    runs.push(super::read_json::<Run>(&file)?);
                }
            }
        }
    }
    runs.sort_by(|a, b| {
        (a.route.to_string(), &a.posting, a.n).cmp(&(b.route.to_string(), &b.posting, b.n))
    });
    Ok(runs)
}

// ── Numbers ─────────────────────────────────────────────────────────────────────────────────────────────────────────

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

/// Sample standard deviation; `None` under two values.
pub fn stdev(xs: &[f64]) -> Option<f64> {
    if xs.len() < 2 {
        return None;
    }
    let m = mean(xs.iter().copied())?;
    let var = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() - 1) as f64;
    Some(var.sqrt())
}

/// Today in UTC, `YYYY-MM-DD`.
pub fn date() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400) as i64;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm), as `resume_pipeline::today` computes it.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}
