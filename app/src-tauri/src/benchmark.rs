//! Settings → Benchmark: each role's routes ranked by `resume-bench`'s score, from three sources: the reference shipped
//! with this release, the runs made on this Mac (`bench_runs`), and your own calls (`usage`'s wall times, and for
//! Tailoring what you kept of a route's suggestions at export). A route's quality comes from whichever of the
//! reference and this Mac has more runs (this Mac on a tie); its reliability and speed from this Mac whenever it has
//! runs for the role, else from the reference.
//!
//! A run makes the plan's calls one at a time on a task ([`start`], [`resume`], [`stop`]), each route's agent built as
//! [`App::llm`] builds it but never through `Metered`: benchmark calls stay out of `usage`. Its plan, state and time
//! spent are the `benchmark_run` setting, so a run gliide quit during reloads as stopped, and resumes skipping the runs
//! it recorded. Every call's end and every change of state is sent as the `benchmark` event.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, LazyLock};
use std::time::Instant;

use chrono::{SecondsFormat, Utc};
use resume_bench::{self as bench, Called, Fixtures, Kept, Outcome, RouteResult, RunSummary};
use resume_llm::{AgentKind, Route};
use resume_store::{BenchRunRow, KeptRow, RouteTimes};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::task::AbortHandle;

use crate::dto::{
    Agent, BenchEstimate, BenchPart, BenchPlan, BenchResult, BenchRole, BenchRoute, BenchRunStatus,
    BenchScore, BenchSource, BenchState, BenchStep, BenchmarkView, KeptShare, Role, Settings,
    YourUse,
};
use crate::error::{AppError, Result};
use crate::state::{App, Event, blocking, lock};
use crate::usage::name;

/// The run's settings key in the store ([`Record`]).
const RUN: &str = "benchmark_run";
/// Timed calls your use needs before its median says anything (as Settings → Agents).
const MIN_TIMED_CALLS: u64 = 5;
/// Failure messages a result lists at most.
const MAX_ERRORS: usize = 3;

/// A route as the store and the reference key it: the agent's name (`claude`) and the model as asked (`sonnet`).
type RouteKey = (String, String);
/// A run as `bench_runs` keys it: role, agent, model, posting, run number.
type RunKey = (String, String, String, String, u32);

/// The benchmark's run, in `App.benchmark`; what isn't kept in the store.
#[derive(Default)]
pub struct BenchRun {
    /// The running task: `stop` aborts it.
    task: Option<AbortHandle>,
    /// Counts runs launched: a task only touches the run while it's still the newest.
    generation: u64,
    /// The call in flight.
    now: Option<BenchStep>,
    /// Seconds spent running before this stretch began.
    base_s: f64,
    /// When this stretch began.
    since: Option<Instant>,
}

impl BenchRun {
    fn elapsed(&self) -> f64 {
        self.base_s + self.since.map_or(0.0, |s| s.elapsed().as_secs_f64())
    }

    /// `generation`'s task is the one running.
    fn is(&self, generation: u64) -> bool {
        self.generation == generation && self.task.is_some()
    }
}

/// The run as the `benchmark_run` setting keeps it. `Running` with no task means gliide quit during it: stopped.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    plan: BenchPlan,
    state: BenchState,
    started_at: String,
    /// Seconds spent running, as of the last call recorded or the last stop.
    elapsed_s: f64,
    error: Option<String>,
}

/// The fixtures, parsed once.
fn fixtures() -> Result<&'static Fixtures> {
    static FIXTURES: LazyLock<std::result::Result<Fixtures, String>> =
        LazyLock::new(|| Fixtures::embedded().map_err(|e| e.to_string()));
    FIXTURES
        .as_ref()
        .map_err(|e| AppError::other(format!("The benchmark's postings didn't load: {e}")))
}

fn bench_role(role: Role) -> bench::Role {
    match role {
        Role::Read => bench::Role::Read,
        Role::Tailor => bench::Role::Tailor,
        Role::Write => bench::Role::Write,
        Role::Coach => bench::Role::Coach,
    }
}

fn stamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true)
}

// ── The plan ──

/// One run a plan makes.
#[derive(Debug, Clone, PartialEq)]
struct Planned {
    role: Role,
    route: Route,
    item: String,
    /// From 1.
    n: u32,
    /// Runs on this posting.
    runs: u32,
}

impl Planned {
    fn key(&self) -> RunKey {
        (
            bench_role(self.role).name().to_owned(),
            self.route.agent.to_string(),
            self.route.model.clone(),
            self.item.clone(),
            self.n,
        )
    }
}

/// Every run `plan` makes, in order: its roles × its routes × the role's postings × their runs. A role or route named
/// twice runs once.
fn planned(fx: &Fixtures, plan: &BenchPlan) -> Vec<Planned> {
    let size = if plan.quick {
        bench::Plan::Quick
    } else {
        bench::Plan::Full
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for &role in &plan.roles {
        for id in &plan.routes {
            if !seen.insert((role, id.clone())) {
                continue;
            }
            let route = Route {
                agent: id.agent.into(),
                model: id.model.clone(),
            };
            for (item, runs) in bench::items(fx, bench_role(role), size) {
                out.extend((1..=runs).map(|n| Planned {
                    role,
                    route: route.clone(),
                    item: item.clone(),
                    n,
                    runs,
                }));
            }
        }
    }
    out
}

/// The runs of `all` not yet `recorded`, in order.
fn pending<'a>(all: &'a [Planned], recorded: &HashSet<RunKey>) -> Vec<&'a Planned> {
    all.iter()
        .filter(|p| !recorded.contains(&p.key()))
        .collect()
}

fn row_key(r: &BenchRunRow) -> RunKey {
    (
        r.role.clone(),
        r.agent.clone(),
        r.model.clone(),
        r.item.clone(),
        r.n,
    )
}

fn outcome_name(o: Outcome) -> &'static str {
    match o {
        Outcome::Valid => "valid",
        Outcome::Repaired => "repaired",
        Outcome::Failed => "failed",
    }
}

fn outcome_of(s: &str) -> Option<Outcome> {
    match s {
        "valid" => Some(Outcome::Valid),
        "repaired" => Some(Outcome::Repaired),
        "failed" => Some(Outcome::Failed),
        _ => None,
    }
}

/// What the store keeps of `run`: never its output or turns.
fn row(run: &bench::Run) -> BenchRunRow {
    BenchRunRow {
        role: run.role.name().to_owned(),
        agent: run.route.agent.to_string(),
        model: run.route.model.clone(),
        item: run.item.clone(),
        n: run.n,
        outcome: outcome_name(run.outcome).to_owned(),
        error: run.error.clone(),
        latency_ms: run.latency_ms,
        input_tokens: run.tokens.input,
        output_tokens: run.tokens.output,
        cache_write_tokens: run.tokens.cache_write,
        cache_read_tokens: run.tokens.cache_read,
        cost_usd: run.cost_usd,
        served: run.served.clone(),
        metrics: run
            .metrics
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok()),
        at: String::new(),
    }
}

// ── The run ──

/// The run on this Mac, as of now; `None` before any.
pub fn status(app: &App) -> Result<Option<BenchRunStatus>> {
    let run = lock(&app.benchmark);
    let (record, recorded) = {
        let store = lock(&app.store);
        let Some(record) = store.setting::<Record>(RUN)? else {
            return Ok(None);
        };
        let recorded: HashSet<RunKey> = store.bench_runs()?.iter().map(row_key).collect();
        (record, recorded)
    };
    let all = planned(fixtures()?, &record.plan);
    let running = run.task.is_some();
    Ok(Some(BenchRunStatus {
        state: match record.state {
            BenchState::Running if !running => BenchState::Stopped,
            s => s,
        },
        done: all.iter().filter(|p| recorded.contains(&p.key())).count() as u32,
        total: all.len() as u32,
        started_at: record.started_at,
        elapsed_s: if running {
            run.elapsed()
        } else {
            record.elapsed_s
        },
        now: run.now.clone().filter(|_| running),
        error: record.error,
        plan: record.plan,
    }))
}

/// Sends the `benchmark` event with the run as of now.
fn changed(app: &App) {
    match status(app) {
        Ok(Some(s)) => (app.emit)(Event::Benchmark(&s)),
        Ok(None) => {}
        Err(e) => eprintln!("gliide: could not read the benchmark's run: {}", e.message),
    }
}

/// Runs `plan`, this Mac's earlier runs of its roles and routes deleted first. Fails while a run is going.
pub async fn start(app: &Arc<App>, plan: BenchPlan) -> Result<BenchRunStatus> {
    let a = Arc::clone(app);
    blocking(app, move |_| launch(&a, Some(plan))).await?;
    started(app).await
}

/// Goes on with the stopped run (or the failed one), skipping the runs it recorded.
pub async fn resume(app: &Arc<App>) -> Result<BenchRunStatus> {
    let a = Arc::clone(app);
    blocking(app, move |_| launch(&a, None)).await?;
    started(app).await
}

async fn started(app: &Arc<App>) -> Result<BenchRunStatus> {
    let status = blocking(app, status).await?;
    let status = status.ok_or_else(|| AppError::other("The benchmark's run wasn't recorded"))?;
    (app.emit)(Event::Benchmark(&status));
    Ok(status)
}

/// Records the run as running (`fresh`: a new plan; `None`: the one recorded) and starts its task.
fn launch(app: &Arc<App>, fresh: Option<BenchPlan>) -> Result<()> {
    let fx = fixtures()?;
    let mut run = lock(&app.benchmark);
    if run.task.is_some() {
        return Err(AppError::other("A benchmark is running; stop it first."));
    }
    let record = {
        let store = lock(&app.store);
        let record = match fresh {
            Some(plan) => {
                if plan.roles.is_empty() || plan.routes.is_empty() {
                    return Err(AppError::other(
                        "Choose at least one role and one route to run.",
                    ));
                }
                for &role in &plan.roles {
                    for id in &plan.routes {
                        store.delete_bench_runs(
                            bench_role(role).name(),
                            &name(&id.agent),
                            &id.model,
                        )?;
                    }
                }
                Record {
                    plan,
                    state: BenchState::Running,
                    started_at: stamp(),
                    elapsed_s: 0.0,
                    error: None,
                }
            }
            None => {
                let mut record = store
                    .setting::<Record>(RUN)?
                    .ok_or_else(|| AppError::missing("There's no benchmark run to go on with."))?;
                if record.state == BenchState::Done {
                    return Err(AppError::other(
                        "This benchmark run is done; start it again to run it anew.",
                    ));
                }
                record.state = BenchState::Running;
                record.error = None;
                record
            }
        };
        store.put_setting(RUN, &record)?;
        record
    };
    run.generation += 1;
    run.base_s = record.elapsed_s;
    run.since = Some(Instant::now());
    run.now = None;
    let task = tokio::spawn(drive(Arc::clone(app), fx, record.plan, run.generation));
    run.task = Some(task.abort_handle());
    Ok(())
}

/// Stops the run: its task aborted, which drops the call in flight and kills the agent's process.
pub fn stop(app: &App) -> Result<()> {
    {
        let mut run = lock(&app.benchmark);
        let Some(task) = run.task.take() else {
            return Ok(());
        };
        task.abort();
        let elapsed = run.elapsed();
        run.now = None;
        run.since = None;
        run.base_s = elapsed;
        let store = lock(&app.store);
        if let Some(mut record) = store.setting::<Record>(RUN)? {
            record.state = BenchState::Stopped;
            record.elapsed_s = elapsed;
            store.put_setting(RUN, &record)?;
        }
    }
    changed(app);
    Ok(())
}

/// Clears the run's task however it ends (a panic included: the record still says running, so it reads as stopped).
struct Running {
    app: Arc<App>,
    generation: u64,
}

impl Drop for Running {
    fn drop(&mut self) {
        let cleared = {
            let mut run = lock(&self.app.benchmark);
            let ours = run.is(self.generation);
            if ours {
                run.base_s = run.elapsed();
                run.task = None;
                run.now = None;
                run.since = None;
            }
            ours
        };
        if cleared {
            changed(&self.app);
        }
    }
}

async fn drive(app: Arc<App>, fx: &'static Fixtures, plan: BenchPlan, generation: u64) {
    let running = Running {
        app: Arc::clone(&app),
        generation,
    };
    let result = calls(&app, fx, &plan, generation).await;
    finish(&app, generation, result);
    drop(running);
    changed(&app);
}

/// Makes the plan's runs not yet recorded, one at a time. An agent that can't be found fails each of its runs with
/// why, without looking again.
async fn calls(
    app: &Arc<App>,
    fx: &'static Fixtures,
    plan: &BenchPlan,
    generation: u64,
) -> Result<()> {
    let recorded = blocking(app, |app| {
        Ok(lock(&app.store)
            .bench_runs()?
            .iter()
            .map(row_key)
            .collect::<HashSet<_>>())
    })
    .await?;
    let all = planned(fx, plan);
    let mut agents: HashMap<AgentKind, std::result::Result<resume_llm::Agent, String>> =
        HashMap::new();
    for p in pending(&all, &recorded) {
        let kind = p.route.agent;
        if let std::collections::hash_map::Entry::Vacant(e) = agents.entry(kind) {
            e.insert(agent(app, kind).await);
        }
        let role = bench_role(p.role);
        let run = match &agents[&kind] {
            Ok(cli) => {
                step(app, fx, p, generation);
                changed(app);
                let cli = cli.clone().with_timeout(bench::timeout(role));
                bench::run_one(&cli, &p.route, role, fx, &p.item, p.n).await
            }
            Err(e) => bench::Run::new(
                role,
                &p.route,
                &p.item,
                p.n,
                Called::failed(e.clone()),
                Value::Null,
            ),
        };
        let row = row(&run);
        if !blocking(app, move |app| record_run(app, generation, &row)).await? {
            return Ok(());
        }
        changed(app);
    }
    Ok(())
}

/// `kind`'s agent at the path the settings name for it, else the one found, unmetered.
async fn agent(app: &App, kind: AgentKind) -> std::result::Result<resume_llm::Agent, String> {
    let path = lock(&app.settings)
        .agents
        .get(kind.into())
        .path
        .clone()
        .filter(|p| !p.trim().is_empty());
    match path {
        Some(p) => Ok(resume_llm::Agent::new(kind, p)),
        None => resume_llm::Agent::discover(kind).await.map_err(|_| {
            format!(
                "gliide can't find {} on this Mac. Install it, or set its path in Settings.",
                kind.name()
            )
        }),
    }
}

/// Marks `p` as the call in flight.
fn step(app: &App, fx: &Fixtures, p: &Planned, generation: u64) {
    let mut run = lock(&app.benchmark);
    if !run.is(generation) {
        return;
    }
    run.now = Some(BenchStep {
        role: p.role,
        agent: p.route.agent.into(),
        model: p.route.model.clone(),
        item: p.item.clone(),
        company: fx
            .item(&p.item)
            .map(|(item, req)| bench::job(item, req).0)
            .unwrap_or_default(),
        n: p.n,
        runs: p.runs,
        started_at: stamp(),
    });
}

/// Stores a finished run and the time spent so far, unless the run was stopped or replaced meanwhile (false).
fn record_run(app: &App, generation: u64, row: &BenchRunRow) -> Result<bool> {
    let mut run = lock(&app.benchmark);
    if !run.is(generation) {
        return Ok(false);
    }
    run.now = None;
    let store = lock(&app.store);
    store.put_bench_run(row)?;
    if let Some(mut record) = store.setting::<Record>(RUN)? {
        record.elapsed_s = run.elapsed();
        store.put_setting(RUN, &record)?;
    }
    Ok(true)
}

/// Records how the task ended: done, or failed with why.
fn finish(app: &App, generation: u64, result: Result<()>) {
    let mut run = lock(&app.benchmark);
    if !run.is(generation) {
        return;
    }
    let elapsed = run.elapsed();
    run.base_s = elapsed;
    run.task = None;
    run.now = None;
    run.since = None;
    let store = lock(&app.store);
    let saved = store.setting::<Record>(RUN).and_then(|record| {
        let Some(mut record) = record else {
            return Ok(());
        };
        record.elapsed_s = elapsed;
        match result {
            Ok(()) => record.state = BenchState::Done,
            Err(e) => {
                record.state = BenchState::Failed;
                record.error = Some(e.message);
            }
        }
        store.put_setting(RUN, &record)
    });
    if let Err(e) = saved {
        eprintln!("gliide: could not record the benchmark's end: {e}");
    }
}

// ── The view ──

/// Your timed calls on a route for a role's features: percentiles in ms.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Times {
    calls: u64,
    p50_ms: f64,
    p90_ms: f64,
}

/// What the view and the estimate read.
struct Sources {
    settings: Settings,
    /// This Mac's runs.
    mac: Vec<BenchRunRow>,
    /// Your calls' times, by role (in `Role::ALL`'s order), under the model names Settings uses.
    yours: Vec<BTreeMap<RouteKey, Times>>,
    /// What you kept of each Tailoring route's suggestions, under the model names Settings uses.
    kept: BTreeMap<RouteKey, KeptShare>,
}

fn load(app: &App) -> Result<Sources> {
    let settings = app.settings();
    let asked = app.served_models()?;
    let (mac, kept, times) = {
        let store = lock(&app.store);
        let times = Role::ALL
            .into_iter()
            .map(|role| {
                let features: Vec<String> = role.features().iter().map(name).collect();
                let features: Vec<&str> = features.iter().map(String::as_str).collect();
                store.latencies_by_route(&features)
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        (store.bench_runs()?, store.kept_by_route()?, times)
    };
    let known = known(&settings, bench::reference(), &mac);
    Ok(Sources {
        yours: times
            .iter()
            .map(|t| your_times(t, &known, &asked))
            .collect(),
        kept: kept_by_route(&kept, &known, &asked),
        settings,
        mac,
    })
}

/// Every route some source names a model for: the reference's, this Mac's runs' and the settings' (every agent's model
/// for every role).
fn known(
    settings: &Settings,
    reference: &bench::Reference,
    mac: &[BenchRunRow],
) -> BTreeSet<RouteKey> {
    let mut known: BTreeSet<RouteKey> = reference
        .roles
        .values()
        .flat_map(|routes| routes.keys())
        .filter_map(|k| parsed(k))
        .collect();
    known.extend(mac.iter().map(|r| (r.agent.clone(), r.model.clone())));
    for kind in AgentKind::ALL {
        let models = &settings.agents.get(kind.into()).models;
        known.extend(Role::ALL.map(|role| (kind.to_string(), models.get(role).to_owned())));
    }
    known
}

/// A reference route (`claude:sonnet`) as a key.
fn parsed(key: &str) -> Option<RouteKey> {
    Route::from_str(key)
        .ok()
        .map(|r| (r.agent.to_string(), r.model))
}

/// The model Settings names for `served` on `agent`: itself when a route names it, else the alias that was asked
/// for it (`asked`: alias → the id that last served it), one a route names first; `None` when nothing asked for it.
fn alias(
    agent: &str,
    served: &str,
    known: &BTreeSet<RouteKey>,
    asked: &HashMap<String, String>,
) -> Option<String> {
    let is_known = |m: &str| known.contains(&(agent.to_owned(), m.to_owned()));
    if is_known(served) {
        return Some(served.to_owned());
    }
    let mut aliases: Vec<&String> = asked
        .iter()
        .filter(|(_, s)| *s == served)
        .map(|(a, _)| a)
        .collect();
    aliases.sort();
    aliases
        .iter()
        .find(|a| is_known(a))
        .or(aliases.first())
        .map(|a| (*a).clone())
}

/// Usage's routes (by served model) under the names Settings uses, rows that land on one route merged: their calls
/// added, their percentiles averaged weighted by calls (the store keeps no merged distribution). A served id nothing
/// asked for stays its own route with 5 calls or more, and is left out below that.
fn your_times(
    rows: &[RouteTimes],
    known: &BTreeSet<RouteKey>,
    asked: &HashMap<String, String>,
) -> BTreeMap<RouteKey, Times> {
    let mut out: BTreeMap<RouteKey, Times> = BTreeMap::new();
    for r in rows {
        let model = match alias(&r.agent, &r.model, known, asked) {
            Some(m) => m,
            None if r.calls >= MIN_TIMED_CALLS => r.model.clone(),
            None => continue,
        };
        let t = out.entry((r.agent.clone(), model)).or_default();
        let calls = t.calls + r.calls;
        if calls == 0 {
            continue;
        }
        let weigh = |was: f64, now: u64| {
            (was * t.calls as f64 + now as f64 * r.calls as f64) / calls as f64
        };
        *t = Times {
            calls,
            p50_ms: weigh(t.p50_ms, r.p50_ms),
            p90_ms: weigh(t.p90_ms, r.p90_ms),
        };
    }
    out
}

/// What you kept, by route under the names Settings uses (analyses record the alias asked, so this rarely moves one).
fn kept_by_route(
    rows: &[KeptRow],
    known: &BTreeSet<RouteKey>,
    asked: &HashMap<String, String>,
) -> BTreeMap<RouteKey, KeptShare> {
    let mut out: BTreeMap<RouteKey, KeptShare> = BTreeMap::new();
    for r in rows {
        let model = alias(&r.agent, &r.model, known, asked).unwrap_or_else(|| r.model.clone());
        let k = out.entry((r.agent.clone(), model)).or_insert(KeptShare {
            kept: 0,
            decided: 0,
            postings: 0,
        });
        k.kept += r.kept as u32;
        k.decided += r.decided as u32;
        k.postings += r.postings as u32;
    }
    out
}

/// Every route's kept share pooled; `None` before any decision.
fn kept_average(kept: &BTreeMap<RouteKey, KeptShare>) -> Option<KeptShare> {
    let all = kept.values().fold(
        KeptShare {
            kept: 0,
            decided: 0,
            postings: 0,
        },
        |a, k| KeptShare {
            kept: a.kept + k.kept,
            decided: a.decided + k.decided,
            postings: a.postings + k.postings,
        },
    );
    (all.decided > 0).then_some(all)
}

/// Settings → Benchmark.
pub fn view(app: &App) -> Result<BenchmarkView> {
    let src = load(app)?;
    let reference = bench::reference();
    let roles = Role::ALL
        .into_iter()
        .zip(&src.yours)
        .map(|(role, yours)| {
            let current = src.settings.route(role);
            board(
                role,
                reference.roles.get(&bench_role(role)),
                &reference.date,
                &src.mac,
                yours,
                &src.kept,
                &(current.agent.to_string(), current.model),
            )
        })
        .collect();
    Ok(BenchmarkView {
        roles,
        reference_date: reference.date.clone(),
        run: status(app)?,
        kept_average: kept_average(&src.kept),
    })
}

/// One role's routes, ranked: every route the reference, this Mac's runs, your calls (and, for Tailoring, your kept
/// edits) know, and `current`.
fn board(
    role: Role,
    reference: Option<&BTreeMap<String, RouteResult>>,
    reference_date: &str,
    mac: &[BenchRunRow],
    yours: &BTreeMap<RouteKey, Times>,
    kept: &BTreeMap<RouteKey, KeptShare>,
    current: &RouteKey,
) -> BenchRole {
    let br = bench_role(role);
    let tailor = role == Role::Tailor;
    let refs: BTreeMap<RouteKey, &RouteResult> = reference
        .into_iter()
        .flatten()
        .filter_map(|(k, r)| Some((parsed(k)?, r)))
        .collect();
    let mut runs: BTreeMap<RouteKey, Vec<&BenchRunRow>> = BTreeMap::new();
    for r in mac.iter().filter(|r| r.role == br.name()) {
        runs.entry((r.agent.clone(), r.model.clone()))
            .or_default()
            .push(r);
    }
    let mut keys: BTreeSet<RouteKey> = refs.keys().cloned().collect();
    keys.extend(runs.keys().cloned());
    keys.extend(yours.keys().cloned());
    if tailor {
        keys.extend(kept.keys().cloned());
    }
    keys.insert(current.clone());
    let all_kept: Vec<Kept> = kept
        .values()
        .map(|k| Kept {
            kept: k.kept,
            decided: k.decided,
        })
        .collect();
    let average = kept_average(kept).map(|k| f64::from(k.kept) / f64::from(k.decided));

    let mut routes = Vec::new();
    let mut ranked = Vec::new();
    for key in keys {
        let Ok(kind) = AgentKind::from_str(&key.0) else {
            continue;
        };
        let reference = refs.get(&key).copied();
        let mine = runs.get(&key).map(|rows| this_mac(br, rows));
        let scored = scored_on(br, reference, mine.as_ref().map(|(r, _)| r));
        let quality_from = scored.as_ref().and_then(|s| s.1);
        let timing_from = scored.as_ref().map(|s| s.2);
        let scored = scored.as_ref().map(|s| &s.0);
        let route_kept = kept.get(&key).copied().filter(|_| tailor);
        let factor = if tailor {
            let mine = route_kept.map_or(Kept::default(), |k| Kept {
                kept: k.kept,
                decided: k.decided,
            });
            bench::kept_factor(mine, &all_kept)
        } else {
            None
        };
        let score = scored.map(|r| {
            let s = bench::score(br, r, factor);
            let without = bench::score(br, r, None).composite;
            BenchScore {
                composite: s.composite,
                composite_without_kept: without,
                quality: s.quality,
                code_checks: r.quality.value.filter(|_| tailor),
                kept_factor: factor,
                reliability: s.reliability,
                speed: s.speed,
            }
        });
        ranked.push((
            routes.len(),
            score.map_or(
                bench::Score {
                    composite: None,
                    quality: None,
                    reliability: 0.0,
                    speed: 0.0,
                },
                |s| bench::Score {
                    composite: s.composite,
                    quality: s.quality,
                    reliability: s.reliability,
                    speed: s.speed,
                },
            ),
            scored.and_then(|r| r.p50_s),
        ));
        let times = yours.get(&key);
        let yours = (times.is_some() || route_kept.is_some()).then(|| {
            let enough = |t: &&Times| t.calls >= MIN_TIMED_CALLS;
            YourUse {
                calls: times.map_or(0, |t| t.calls as u32),
                p50_s: times.filter(enough).map(|t| t.p50_ms / 1000.0),
                p90_s: times.filter(enough).map(|t| t.p90_ms / 1000.0),
                kept: route_kept,
                kept_counted: route_kept.and(factor).zip(average).map(|(f, avg)| f * avg),
            }
        });
        routes.push(BenchRoute {
            agent: kind.into(),
            current: key == *current,
            model: key.1,
            rank: None,
            quality_from,
            timing_from,
            score,
            reference: reference.map(|r| result(r, reference_date.to_owned(), Vec::new())),
            this_mac: mine.map(|(r, (at, errors))| result(&r, at, errors)),
            yours,
        });
    }
    let mut slots: Vec<Option<BenchRoute>> = routes.into_iter().map(Some).collect();
    let routes = bench::rank(&ranked)
        .into_iter()
        .filter_map(|(i, rank)| {
            let mut r = slots.get_mut(i)?.take()?;
            r.rank = rank;
            Some(r)
        })
        .collect();
    BenchRole {
        role,
        target_s: bench::target_s(br),
        routes,
    }
}

/// The result a route is scored on: quality (its value and parts) from the source with more runs, this Mac on a tie,
/// so a quick run on this Mac doesn't outweigh the reference's larger sample; reliability and speed (runs, outcomes,
/// percentiles, cost) from this Mac whenever it has runs, as timing is this Mac's own. With where each came from
/// (quality: `None` for Writing, which has none); `None` when neither source has runs.
fn scored_on(
    role: bench::Role,
    reference: Option<&RouteResult>,
    mine: Option<&RouteResult>,
) -> Option<(RouteResult, Option<BenchSource>, BenchSource)> {
    let (timing, timing_from) = match (mine, reference) {
        (Some(m), _) => (m, BenchSource::ThisMac),
        (None, Some(r)) => (r, BenchSource::Reference),
        (None, None) => return None,
    };
    let (quality, quality_from) = match (mine, reference) {
        (Some(m), Some(r)) if r.runs > m.runs => (r, BenchSource::Reference),
        _ => (timing, timing_from),
    };
    let merged = RouteResult {
        quality: quality.quality.clone(),
        ..timing.clone()
    };
    Some((
        merged,
        (role != bench::Role::Write).then_some(quality_from),
        timing_from,
    ))
}

/// This Mac's runs of a route summed, with the day of the newest and its distinct failures.
fn this_mac(role: bench::Role, rows: &[&BenchRunRow]) -> (RouteResult, (String, Vec<String>)) {
    let summaries: Vec<RunSummary> = rows
        .iter()
        .filter_map(|r| {
            Some(RunSummary {
                item: r.item.clone(),
                outcome: outcome_of(&r.outcome)?,
                latency_ms: r.latency_ms,
                cost_usd: r.cost_usd,
                metrics: r
                    .metrics
                    .as_deref()
                    .and_then(|m| serde_json::from_str(m).ok()),
            })
        })
        .collect();
    // The whole timestamp (UTC): the pane shows its local day, which the UTC date isn't in the evening west of UTC.
    let at = rows
        .iter()
        .map(|r| r.at.as_str())
        .max()
        .unwrap_or_default()
        .to_owned();
    let mut errors: Vec<String> = Vec::new();
    for e in rows
        .iter()
        .filter(|r| r.outcome == "failed")
        .filter_map(|r| r.error.as_deref())
    {
        let first = e.lines().next().unwrap_or(e).trim().to_owned();
        if errors.len() < MAX_ERRORS && !errors.contains(&first) {
            errors.push(first);
        }
    }
    (bench::aggregate(role, &summaries), (at, errors))
}

fn result(r: &RouteResult, at: String, errors: Vec<String>) -> BenchResult {
    BenchResult {
        runs: r.runs,
        valid: r.valid,
        repaired: r.repaired,
        failed: r.failed,
        items: r.items,
        p50_s: r.p50_s,
        p90_s: r.p90_s,
        cost_usd: r.cost_usd,
        quality: r.quality.value,
        parts: r
            .quality
            .parts
            .iter()
            .map(|p| BenchPart {
                id: p.id.clone(),
                value: p.value,
            })
            .collect(),
        at,
        errors,
    }
}

// ── The estimate ──

/// How many calls `plan` makes, about how long they take, and how many each agent makes.
pub fn estimate(app: &App, plan: &BenchPlan) -> Result<BenchEstimate> {
    let fx = fixtures()?;
    let src = load(app)?;
    let reference = bench::reference();
    let mut per_agent: HashMap<Agent, u32> = HashMap::new();
    let mut seconds = 0.0;
    let all = planned(fx, plan);
    for p in &all {
        let agent = Agent::from(p.route.agent);
        *per_agent.entry(agent).or_default() += 1;
        let key = (p.route.agent.to_string(), p.route.model.clone());
        let role_index = Role::ALL.iter().position(|r| *r == p.role).unwrap_or(0);
        let yours = src.yours[role_index]
            .get(&key)
            .filter(|t| t.calls >= MIN_TIMED_CALLS)
            .map(|t| t.p50_ms / 1000.0);
        let referenced = reference
            .roles
            .get(&bench_role(p.role))
            .and_then(|routes| routes.get(&p.route.to_string()))
            .and_then(|r| r.p50_s);
        seconds += yours
            .or(referenced)
            .unwrap_or_else(|| bench::target_s(bench_role(p.role)));
    }
    Ok(BenchEstimate {
        calls: all.len() as u32,
        seconds,
        per_agent: AgentKind::ALL
            .into_iter()
            .map(Agent::from)
            .filter_map(|a| per_agent.get(&a).map(|&n| (a, n)))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use bench::{Part, Quality};

    use super::*;
    use crate::dto::BenchRouteId;

    fn times(agent: &str, model: &str, calls: u64, p50_ms: u64) -> RouteTimes {
        RouteTimes {
            agent: agent.to_owned(),
            model: model.to_owned(),
            calls,
            p50_ms,
            p90_ms: p50_ms * 2,
        }
    }

    fn key(agent: &str, model: &str) -> RouteKey {
        (agent.to_owned(), model.to_owned())
    }

    #[test]
    fn served_ids_count_under_the_alias_settings_asked_for() {
        let known = BTreeSet::from([key("claude", "sonnet"), key("codex", "gpt-6-luna")]);
        let asked = HashMap::from([
            ("sonnet".to_owned(), "claude-sonnet-5-5".to_owned()),
            ("gpt-6-luna".to_owned(), "gpt-6-luna".to_owned()),
        ]);
        let mapped = your_times(
            &[
                // Served ids the agent reported, and calls from before it did (recorded as asked).
                times("claude", "claude-sonnet-5-5", 6, 10_000),
                times("claude", "sonnet", 2, 4_000),
                times("codex", "gpt-6-luna", 5, 3_000),
                // Nothing asked for these: a route of its own with 5 calls or more, else left out.
                times("codex", "gpt-mystery", 7, 8_000),
                times("codex", "gpt-rare", 4, 8_000),
            ],
            &known,
            &asked,
        );
        assert_eq!(
            mapped.keys().cloned().collect::<Vec<_>>(),
            [
                key("claude", "sonnet"),
                key("codex", "gpt-6-luna"),
                key("codex", "gpt-mystery")
            ]
        );
        let sonnet = mapped[&key("claude", "sonnet")];
        assert_eq!(sonnet.calls, 8);
        assert_eq!(sonnet.p50_ms, (6.0 * 10_000.0 + 2.0 * 4_000.0) / 8.0);
        // The same served id under another agent isn't that agent's alias.
        assert_eq!(
            alias("codex", "claude-sonnet-5-5", &known, &asked),
            Some("sonnet".to_owned())
        );
        assert_eq!(alias("codex", "nobody-asked", &known, &asked), None);
    }

    #[test]
    fn a_resumed_run_skips_the_runs_already_recorded() {
        let fx = fixtures().unwrap();
        let plan = BenchPlan {
            quick: true,
            roles: vec![Role::Read, Role::Tailor, Role::Read],
            routes: vec![BenchRouteId {
                agent: Agent::Claude,
                model: "sonnet".to_owned(),
            }],
        };
        let all = planned(fx, &plan);
        // Quick: 4 postings once, per role, the repeated role once.
        assert_eq!(all.len(), 8);
        // A recorded run keys as its planned run does.
        let first = &all[0];
        let done = row(&bench::Run::new(
            bench_role(first.role),
            &first.route,
            &first.item,
            first.n,
            Called::failed("not found".to_owned()),
            Value::Null,
        ));
        assert_eq!(row_key(&done), first.key());
        let recorded = HashSet::from([row_key(&done), all[5].key()]);
        let left = pending(&all, &recorded);
        assert_eq!(left.len(), 6);
        assert_eq!(left[0], &all[1]);
        assert!(!left.contains(&&all[5]));
        let full = BenchPlan {
            quick: false,
            ..plan
        };
        let all = planned(fx, &full);
        assert_eq!(all.len(), 12 * 3 * 2);
        assert_eq!((all[2].n, all[2].runs), (3, 3));
    }

    fn rr(runs: u32, quality: Option<f64>, p50_s: f64) -> RouteResult {
        RouteResult {
            runs,
            valid: runs,
            repaired: 0,
            failed: 0,
            items: runs,
            p50_s: Some(p50_s),
            p90_s: Some(p50_s),
            cost_usd: None,
            quality: Quality {
                value: quality,
                parts: vec![Part {
                    id: "recall".to_owned(),
                    value: 0.9,
                }],
            },
        }
    }

    fn mac_row(
        role: &str,
        agent: &str,
        model: &str,
        item: &str,
        outcome: &str,
        latency_ms: u64,
    ) -> BenchRunRow {
        BenchRunRow {
            role: role.to_owned(),
            agent: agent.to_owned(),
            model: model.to_owned(),
            item: item.to_owned(),
            n: 1,
            outcome: outcome.to_owned(),
            error: (outcome == "failed").then(|| "timed out\nraw reply".to_owned()),
            latency_ms,
            input_tokens: 0,
            output_tokens: 0,
            cache_write_tokens: 0,
            cache_read_tokens: 0,
            cost_usd: None,
            served: None,
            metrics: (outcome != "failed").then(|| {
                serde_json::to_string(&bench::RunMetrics::Read {
                    recall: Some(0.5),
                    precision: Some(0.5),
                    must_recall: Some(0.5),
                })
                .unwrap()
            }),
            at: "2026-11-18T09:00:00.000000Z".to_owned(),
        }
    }

    fn read(reference: &BTreeMap<String, RouteResult>, mac: &[BenchRunRow]) -> BenchRole {
        board(
            Role::Read,
            Some(reference),
            "2026-11-17",
            mac,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &key("claude", "haiku"),
        )
    }

    #[test]
    fn a_quick_run_on_this_mac_times_a_route_but_the_larger_reference_sets_its_quality() {
        let reference = BTreeMap::from([
            ("claude:sonnet".to_owned(), rr(36, Some(90.0), 20.0)),
            ("claude:haiku".to_owned(), rr(36, Some(80.0), 5.0)),
        ]);
        let mac = [
            mac_row("read", "claude", "sonnet", "a", "valid", 4_000),
            mac_row("read", "claude", "sonnet", "b", "valid", 4_000),
            mac_row("read", "claude", "sonnet", "c", "valid", 4_000),
            mac_row("read", "claude", "sonnet", "d", "failed", 30_000),
            // Another role's run doesn't count for Read.
            mac_row("tailor", "claude", "haiku", "a", "valid", 4_000),
        ];
        let b = read(&reference, &mac);
        let route = |m: &str| b.routes.iter().find(|r| r.model == m).unwrap();
        let sonnet = route("sonnet");
        assert_eq!(sonnet.quality_from, Some(BenchSource::Reference));
        assert_eq!(sonnet.timing_from, Some(BenchSource::ThisMac));
        let mine = sonnet.this_mac.as_ref().unwrap();
        assert_eq!(
            (mine.runs, mine.valid, mine.failed, mine.items),
            (4, 3, 1, 4)
        );
        assert_eq!(mine.quality, Some(50.0));
        // The time, not its UTC day: the pane shows the local day.
        assert_eq!(mine.at, "2026-11-18T09:00:00.000000Z");
        assert_eq!(mine.errors, ["timed out"]);
        // Quality 90 from the reference's 36 runs; reliability 75 and speed 100 (p50 4 s) from this Mac's 4, where
        // the reference's p50 of 20 s would give speed 50.
        let score = sonnet.score.unwrap();
        assert_eq!(score.quality, Some(90.0));
        assert_eq!(score.reliability, 75.0);
        assert_eq!(score.speed, 100.0);
        assert!((score.composite.unwrap() - (0.6 * 90.0 + 0.2 * 75.0 + 0.2 * 100.0)).abs() < 1e-9);
        assert_eq!(sonnet.reference.as_ref().unwrap().at, "2026-11-17");
        let haiku = route("haiku");
        assert_eq!(haiku.quality_from, Some(BenchSource::Reference));
        assert_eq!(haiku.timing_from, Some(BenchSource::Reference));
        assert!(haiku.this_mac.is_none());
        assert_eq!(haiku.score.unwrap().quality, Some(80.0));
    }

    #[test]
    fn a_full_run_on_this_mac_sets_quality_too() {
        let reference = BTreeMap::from([("claude:sonnet".to_owned(), rr(36, Some(90.0), 5.0))]);
        // Full: 12 postings × 3, as many runs as the reference, so this Mac's count wins the tie.
        let mac: Vec<BenchRunRow> = (0..36)
            .map(|i| {
                mac_row(
                    "read",
                    "claude",
                    "sonnet",
                    &format!("p{}", i % 12),
                    "valid",
                    4_000,
                )
            })
            .collect();
        let b = read(&reference, &mac);
        let sonnet = b.routes.iter().find(|r| r.model == "sonnet").unwrap();
        assert_eq!(sonnet.quality_from, Some(BenchSource::ThisMac));
        assert_eq!(sonnet.timing_from, Some(BenchSource::ThisMac));
        assert_eq!(sonnet.score.unwrap().quality, Some(50.0));
        assert_eq!(sonnet.this_mac.as_ref().unwrap().items, 12);

        // Writing has no quality to take from either source.
        let b = board(
            Role::Write,
            Some(&BTreeMap::from([(
                "claude:sonnet".to_owned(),
                rr(4, None, 5.0),
            )])),
            "2026-11-17",
            &[],
            &BTreeMap::new(),
            &BTreeMap::new(),
            &key("claude", "sonnet"),
        );
        let sonnet = &b.routes[0];
        assert_eq!(sonnet.quality_from, None);
        assert_eq!(sonnet.timing_from, Some(BenchSource::Reference));
        assert_eq!(sonnet.score.unwrap().composite, None);
    }

    #[test]
    fn routes_rank_by_composite_and_the_current_one_is_flagged() {
        let reference = BTreeMap::from([
            ("claude:sonnet".to_owned(), rr(10, Some(80.0), 5.0)),
            ("claude:opus".to_owned(), rr(10, Some(90.0), 5.0)),
            ("codex:gpt-6-luna".to_owned(), rr(10, Some(90.0), 5.0)),
        ]);
        let kept = BTreeMap::from([
            (
                key("claude", "opus"),
                KeptShare {
                    kept: 2,
                    decided: 10,
                    postings: 2,
                },
            ),
            (
                key("claude", "sonnet"),
                KeptShare {
                    kept: 9,
                    decided: 10,
                    postings: 3,
                },
            ),
        ]);
        // Tailoring: kept edits move the composites; the current route has no data and goes last, unranked.
        let b = board(
            Role::Tailor,
            Some(&reference),
            "2026-11-17",
            &[],
            &BTreeMap::new(),
            &kept,
            &key("antigravity", "gemini-new"),
        );
        let order: Vec<(&str, Option<u32>, bool)> = b
            .routes
            .iter()
            .map(|r| (r.model.as_str(), r.rank, r.current))
            .collect();
        assert_eq!(
            order,
            [
                ("gpt-6-luna", Some(1), false),
                // Sonnet's code checks are lower, but you keep most of what it suggests.
                ("sonnet", Some(2), false),
                ("opus", Some(3), false),
                ("gemini-new", None, true),
            ]
        );
        let opus = b.routes[2].score.unwrap();
        // Opus kept less than your average (0.55): its factor is below 1 and its composite below the one without.
        assert!(opus.kept_factor.unwrap() < 1.0);
        assert!(opus.composite.unwrap() < opus.composite_without_kept.unwrap());
        assert_eq!(opus.code_checks, Some(90.0));
        let luna = b.routes[0].score.unwrap();
        // No decisions on Luna: at your average.
        assert_eq!(luna.kept_factor, Some(1.0));
        let yours = b.routes[2].yours.as_ref().unwrap();
        assert_eq!(yours.calls, 0);
        assert!((yours.kept_counted.unwrap() - (2.0 + 10.0 * 0.55) / 20.0).abs() < 1e-9);

        // Read: the same scores rank equal and share a rank; no kept factor.
        let b = board(
            Role::Read,
            Some(&reference),
            "2026-11-17",
            &[],
            &BTreeMap::new(),
            &kept,
            &key("claude", "sonnet"),
        );
        let ranks: Vec<Option<u32>> = b.routes.iter().map(|r| r.rank).collect();
        assert_eq!(ranks, [Some(1), Some(1), Some(3)]);
        assert!(b.routes[2].current);
        let s = b.routes[0].score.unwrap();
        assert_eq!((s.kept_factor, s.code_checks), (None, None));
        assert_eq!(s.composite, s.composite_without_kept);
        assert!(b.routes.iter().all(|r| r.yours.is_none()));
    }
}
