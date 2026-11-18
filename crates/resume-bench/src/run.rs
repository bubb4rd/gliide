//! One run: a role's call on a route, timed, with what it cost and returned.

use std::time::{Duration, Instant};

use resume_core::change::Mode;
use resume_llm::cover::{self, CoverInput};
use resume_llm::prep::{self, PrepInput, SentResume};
use resume_llm::pricing::{Tokens, price, reported_cost};
use resume_llm::{Attempt, Llm, LlmError, Route, Structured, extract};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::score::{RunMetrics, RunSummary, metrics};
use crate::{BenchError, Fixtures, Role, coach, date, job, tailor};

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

/// One run, as `ez eval` records it in `<out>/<role>/<agent>-<model>/<posting>/<n>.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    pub role: Role,
    pub route: Route,
    /// The posting id (`ez eval`'s Dig runs: `dig-<entry id>`).
    #[serde(rename = "posting")]
    pub item: String,
    pub n: u32,
    pub outcome: Outcome,
    pub error: Option<String>,
    /// Wall time of the call, both turns when repaired (and Tailor's fit step).
    pub latency_ms: u64,
    /// The model id the agent says served the last turn.
    pub served: Option<String>,
    pub tokens: TokenCounts,
    /// What the agent reported, else the tokens at the model's API list price; `None` when neither is known.
    pub cost_usd: Option<f64>,
    /// `reported` or `list price`.
    pub cost_from: Option<String>,
    /// What the call read beyond the posting (mode, candidates, input size).
    #[serde(default)]
    pub meta: Value,
    pub output: Option<Value>,
    /// Every turn: its latency, usage and raw reply.
    pub attempts: Vec<Value>,
    /// [`metrics`] of the output; `None` for a failed run or one its role's checks couldn't measure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<RunMetrics>,
}

impl Run {
    pub fn ok(&self) -> bool {
        self.outcome != Outcome::Failed
    }

    pub fn secs(&self) -> f64 {
        self.latency_ms as f64 / 1000.0
    }

    /// What [`crate::aggregate`] reads of it.
    pub fn summary(&self) -> RunSummary {
        RunSummary {
            item: self.item.clone(),
            outcome: self.outcome,
            latency_ms: self.latency_ms,
            cost_usd: self.cost_usd,
            metrics: self.metrics.clone(),
        }
    }

    /// The run `c` was, without metrics: cost from what the agent reported, else the served model's list price.
    pub fn new(role: Role, route: &Route, item: &str, n: u32, c: Called, meta: Value) -> Self {
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
            role,
            route: route.clone(),
            item: item.to_owned(),
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
            metrics: None,
        }
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

impl Called {
    /// A call that never reached the agent.
    pub fn failed(error: String) -> Self {
        Self {
            outcome: Outcome::Failed,
            output: None,
            attempts: Vec::new(),
            error: Some(error),
            latency: Duration::ZERO,
        }
    }
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

/// Run `n` of `role` on posting `item` with `route`: one call as the app makes it (Tailor: Suggest, bank-aware, and
/// its fit step), measured by [`metrics`] when it is valid. Nothing touches the filesystem; an item the fixtures don't
/// have is a failed run that never called.
pub async fn run_one(
    llm: &impl Llm,
    route: &Route,
    role: Role,
    fx: &Fixtures,
    item: &str,
    n: u32,
) -> Run {
    let (c, meta) = match call_role(llm, route, role, fx, item).await {
        Ok(x) => x,
        Err(e) => (Called::failed(e.to_string()), Value::Null),
    };
    let mut run = Run::new(role, route, item, n, c, meta);
    if run.ok() {
        run.metrics = run
            .output
            .as_ref()
            .and_then(|o| metrics(role, fx, o, item).ok());
    }
    run
}

async fn call_role(
    llm: &impl Llm,
    route: &Route,
    role: Role,
    fx: &Fixtures,
    item: &str,
) -> Result<(Called, Value), BenchError> {
    let (f, req) = fx.item(item)?;
    Ok(match role {
        Role::Read => {
            let c = call(extract::extract(
                llm,
                &extract::schema(),
                &f.posting,
                &route.model,
            ))
            .await;
            let meta = json!({ "input_chars": extract::input(&f.posting).chars().count() });
            (c, meta)
        }
        Role::Tailor => tailor::suggest(llm, route, fx, Mode::BankAware, item).await?,
        Role::Write => {
            let (company, role) = job(f, req);
            let input = CoverInput {
                company: &company,
                role: &role,
                resume: &fx.resume,
                posting: &f.posting.text,
            };
            let c = call(cover::write(llm, &cover::schema(), &input, &route.model)).await;
            let meta = json!({ "company": company, "role": role, "master": fx.master.id });
            (c, meta)
        }
        Role::Coach => {
            let (company, role) = job(f, req);
            let sent = SentResume::tagged(&fx.resume, &fx.bank);
            let today = date();
            let input = PrepInput {
                kind: coach::ROUND,
                with: "",
                notes: "",
                role: &role,
                company: &company,
                requirements: Some(req),
                sent: &sent,
                stories: &[],
                debriefs: &[],
                today: &today,
            };
            let c = call(prep::write(llm, &prep::schema(), &input, &route.model)).await;
            let meta =
                json!({ "kind": "prep", "round": coach::ROUND, "company": company, "role": role });
            (c, meta)
        }
    })
}
