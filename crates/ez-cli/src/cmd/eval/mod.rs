//! `ez eval`: each role's calls on the fixture postings, on every route asked for, one call at a time, then a report
//! of how the routes compare (`local/routing/PLAN.md` §3). The calls, fixtures and numbers are `resume_bench`'s, as
//! the app's Benchmark runs them; this records and reports.
//!
//! Each run is recorded as `<out>/<role>/<agent>-<model>/<posting>/<n>.json`: the outcome, wall time, tokens, cost,
//! the output, and every turn's raw reply. A run already recorded is skipped, so an interrupted eval resumes where it
//! stopped. `report.md` and `routes.json` are rebuilt from the recorded runs alone, after every role command and by
//! `ez eval report`. `ez eval reference` turns recorded runs into the reference the app ships. Paths are relative to
//! the working folder: run it from the repo root.

pub mod coach;
pub mod read;
pub mod report;
pub mod tailor;
pub mod write;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::Subcommand;
use resume_bench::{
    Fixtures, Plan, Reference, Role, RouteResult, Run, aggregate, date, mean, metrics, score,
};
use resume_llm::{Agent, Route};

use super::job::runtime;

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
    /// Build the app's reference (`crates/resume-bench/reference.json`) from recorded runs: each role's routes from
    /// the first of its folders that has runs of them.
    Reference(ReferenceArgs),
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

/// Each role's eval folders, most preferred first: a route takes its runs from the first that has any. Each folder's
/// name starts with its date (`local/eval/2026-11-17-read`).
#[derive(clap::Args)]
pub struct ReferenceArgs {
    #[arg(long, required = true)]
    pub read: Vec<PathBuf>,
    /// Bank-aware runs (`<dir>/tailor`).
    #[arg(long, required = true)]
    pub tailor: Vec<PathBuf>,
    #[arg(long, required = true)]
    pub write: Vec<PathBuf>,
    /// Prep runs only; Dig's are left out.
    #[arg(long, required = true)]
    pub coach: Vec<PathBuf>,
    #[arg(long, default_value = "crates/resume-bench/reference.json")]
    pub out: PathBuf,
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let fx = Fixtures::embedded()?;
    let out = match &args.command {
        Command::Read(a) | Command::Write(a) | Command::Coach(a) => a.out.dir(),
        Command::Tailor(a) => a.run.out.dir(),
        Command::Report(o) => o.dir(),
        Command::Reference(a) => return reference(&fx, a),
    };
    let rt = runtime()?;
    match &args.command {
        Command::Read(a) => rt.block_on(read::run(&fx, a, &out))?,
        Command::Tailor(a) => rt.block_on(tailor::run(&fx, a, &out))?,
        Command::Write(a) => rt.block_on(write::run(&fx, a, &out))?,
        Command::Coach(a) => rt.block_on(coach::run(&fx, a, &out))?,
        Command::Report(_) | Command::Reference(_) => {}
    }
    report::run(&fx, &out)
}

// ── Runs ────────────────────────────────────────────────────────────────────────────────────────────────────────────

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

fn run_path(out: &Path, dir: &str, route: &Route, item: &str, n: u32) -> PathBuf {
    out.join(dir)
        .join(route_dir(route))
        .join(item)
        .join(format!("{n}.json"))
}

/// Runs `f` for every route, item and run number not recorded yet under `<out>/<dir>`, one call at a time, and records
/// each. `role`'s plan sets the runs unless `--runs` does, and its timeout holds each call. A route whose agent isn't
/// installed is skipped with a message; the others still run.
pub async fn each(
    dir: &str,
    role: Role,
    args: &RunArgs,
    out: &Path,
    items: &[String],
    mut f: impl AsyncFnMut(&Agent, &Route, &str, u32) -> Run,
) -> anyhow::Result<()> {
    if items.is_empty() {
        bail!("no posting matches");
    }
    let runs = args.runs.unwrap_or(Plan::Full.runs(role)).max(1);
    for route in &args.routes {
        let todo = |id: &str, n| !run_path(out, dir, route, id, n).exists();
        if !items.iter().any(|id| (1..=runs).any(|n| todo(id, n))) {
            eprintln!("{dir} {route}: every run recorded");
            continue;
        }
        let llm = match Agent::discover(route.agent).await {
            Ok(a) => a.with_timeout(resume_bench::timeout(role)),
            Err(e) => {
                eprintln!("{dir} {route}: {e}; skipped");
                continue;
            }
        };
        for id in items {
            for n in (1..=runs).filter(|&n| todo(id, n)) {
                eprint!("{dir} {route} {id} #{n} … ");
                let run = f(&llm, route, id, n).await;
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
                super::write_json(&run_path(out, dir, route, id, n), &run)?;
            }
        }
    }
    Ok(())
}

/// Every run recorded under `<out>/<dir>`, by route, posting and run number.
pub fn recorded(out: &Path, dir: &str) -> anyhow::Result<Vec<Run>> {
    let mut runs = Vec::new();
    let Ok(routes) = std::fs::read_dir(out.join(dir)) else {
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
        (a.route.to_string(), &a.item, a.n).cmp(&(b.route.to_string(), &b.item, b.n))
    });
    Ok(runs)
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

// ── Reference ───────────────────────────────────────────────────────────────────────────────────────────────────────

/// Writes the reference from the runs `a` names: per role and route, the runs with their metrics measured again
/// from each output, summed. Its date is the newest folder's that gave runs. Prints each route's score before kept
/// edits, for a look.
fn reference(fx: &Fixtures, a: &ReferenceArgs) -> anyhow::Result<()> {
    let mut roles = BTreeMap::new();
    let mut dates = Vec::new();
    for (role, dirs) in [
        (Role::Read, &a.read),
        (Role::Tailor, &a.tailor),
        (Role::Write, &a.write),
        (Role::Coach, &a.coach),
    ] {
        let mut routes: BTreeMap<String, RouteResult> = BTreeMap::new();
        for dir in dirs {
            let runs = recorded(dir, role.name())?;
            let mut used = false;
            for (route, rs) in report::by_route(runs.iter().filter(|r| !coach::is_dig(&r.item))) {
                if routes.contains_key(&route) {
                    continue;
                }
                let summaries: Vec<_> = rs
                    .iter()
                    .map(|r| {
                        let mut s = r.summary();
                        s.metrics = r.output.as_ref().filter(|_| r.ok()).and_then(|o| {
                            metrics(role, fx, o, &r.item)
                                .inspect_err(|e| {
                                    eprintln!("{} {route} {} #{}: {e}", dir.display(), r.item, r.n)
                                })
                                .ok()
                        });
                        s
                    })
                    .collect();
                routes.insert(route, aggregate(role, &summaries));
                used = true;
            }
            if used {
                dates.push(dir_date(dir)?);
            }
        }
        roles.insert(role, routes);
    }
    let reference = Reference {
        date: dates
            .into_iter()
            .max()
            .context("no runs recorded in any folder")?,
        roles,
    };
    super::write_json(&a.out, &reference)?;
    println!("{} ({})", a.out.display(), reference.date);
    for (role, routes) in &reference.roles {
        for (route, r) in routes {
            let s = score(*role, r, None);
            let n = |x: Option<f64>| x.map_or("–".to_owned(), |x| format!("{x:.1}"));
            println!(
                "{:<7} {route:<36} composite {:>5}  quality {:>5}  reliability {:>5.1}  speed {:>5.1}",
                role.name(),
                n(s.composite),
                n(s.quality),
                s.reliability,
                s.speed
            );
        }
    }
    Ok(())
}

/// The `YYYY-MM-DD` a run folder's name starts with.
fn dir_date(dir: &Path) -> anyhow::Result<String> {
    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let date = name.get(..10).unwrap_or("");
    let ok = date.len() == 10
        && date.bytes().enumerate().all(|(i, b)| match i {
            4 | 7 => b == b'-',
            _ => b.is_ascii_digit(),
        });
    if !ok {
        bail!(
            "{}: the folder's name doesn't start with its date",
            dir.display()
        );
    }
    Ok(date.to_owned())
}
