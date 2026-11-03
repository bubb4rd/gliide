//! Settings → Usage: every model call's tokens and what it would cost at API prices, recorded as it ends
//! ([`Metered`]), then summed per day, feature, agent and model ([`usage`]) or written out per call ([`export_usage`]).
//!
//! Claude Code says what each call cost; Codex and Antigravity report tokens only, so theirs is estimated from the
//! list prices in `resume_llm::pricing` when shown, never stored, so a corrected price applies to past calls too.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::Arc;

use chrono::Local;
use resume_llm::pricing::{self, PRICES_AS_OF, Tokens};
use resume_llm::{Agent, Attempt, Call, Llm, LlmError, Schema, Structured};
use resume_store::UsageRow;

use crate::dto::{Feature, UsageDay, UsageView};
use crate::error::Result;
use crate::state::{App, lock};

/// The agent the settings run, recording each call's usage under `feature` once it ends: a call that failed the
/// schema after its repair turn too (its turns were spent), never one that never got an answer (the agent didn't
/// start, timed out or was stopped: it reported nothing).
pub struct Metered {
    agent: Agent,
    app: Arc<App>,
    feature: Feature,
}

impl Metered {
    pub fn new(agent: Agent, app: Arc<App>, feature: Feature) -> Self {
        Self {
            agent,
            app,
            feature,
        }
    }
}

impl Llm for Metered {
    async fn structured(
        &self,
        call: Call<'_>,
        schema: &Schema,
    ) -> std::result::Result<Structured, LlmError> {
        let out = self.agent.structured(call, schema).await;
        let attempts = match &out {
            Ok(s) => Some(&s.attempts),
            Err(LlmError::Invalid { attempts, .. }) => Some(attempts),
            Err(_) => None,
        };
        if let Some(attempts) = attempts {
            self.record(call.model, attempts);
        }
        out
    }
}

impl Metered {
    /// Records the call; a store error is logged, never fails the call.
    fn record(&self, asked: Option<&str>, attempts: &[Attempt]) {
        let kind = self.agent.kind();
        let asked = asked.unwrap_or(kind.default_model());
        let tokens = Tokens::of(attempts);
        let row = UsageRow {
            at: String::new(),
            day: Local::now().format("%Y-%m-%d").to_string(),
            feature: name(&self.feature),
            agent: name(&crate::dto::Agent::from(kind)),
            model: resume_llm::served(attempts, asked).unwrap_or(asked).to_owned(),
            input_tokens: tokens.input,
            output_tokens: tokens.output,
            cache_write_tokens: tokens.cache_write,
            cache_read_tokens: tokens.cache_read,
            cost_usd: pricing::reported_cost(attempts),
        };
        if let Err(e) = lock(&self.app.store).record_usage(&row) {
            eprintln!("gliide: could not record a model call's usage: {e}");
        }
    }
}

/// A unit enum's serde name (`feed-check`, `claude`).
fn name<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// The serde value named `s`; `None` for a name this build doesn't know.
fn parse<T: serde::de::DeserializeOwned>(s: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(s.to_owned())).ok()
}

/// Every call recorded, summed per day, feature, agent and model, oldest day first.
pub fn usage(app: &App) -> Result<UsageView> {
    let calls = lock(&app.store).usage()?;
    Ok(UsageView {
        since: calls.iter().map(|c| c.day.clone()).min(),
        today: Local::now().format("%Y-%m-%d").to_string(),
        prices_as_of: PRICES_AS_OF.to_owned(),
        rows: summed(&calls),
    })
}

fn summed(calls: &[UsageRow]) -> Vec<UsageDay> {
    let mut days: BTreeMap<(String, String, String, String), UsageDay> = BTreeMap::new();
    for c in calls {
        let (Some(feature), Some(agent)) = (parse(&c.feature), parse(&c.agent)) else {
            continue;
        };
        let d = days
            .entry((c.day.clone(), c.feature.clone(), c.agent.clone(), c.model.clone()))
            .or_insert_with(|| UsageDay {
                day: c.day.clone(),
                feature,
                agent,
                model: c.model.clone(),
                calls: 0,
                input_tokens: 0,
                output_tokens: 0,
                cache_write_tokens: 0,
                cache_read_tokens: 0,
                reported_cost: 0.0,
                reported_calls: 0,
                estimated_cost: 0.0,
                estimated_calls: 0,
                unpriced_calls: 0,
                cost_by_kind: [0.0; 4],
            });
        let tokens = Tokens {
            input: c.input_tokens,
            output: c.output_tokens,
            cache_write: c.cache_write_tokens,
            cache_read: c.cache_read_tokens,
        };
        d.calls += 1;
        d.input_tokens += tokens.input;
        d.output_tokens += tokens.output;
        d.cache_write_tokens += tokens.cache_write;
        d.cache_read_tokens += tokens.cache_read;
        let listed = pricing::price(&c.model).map(|p| p.cost(&tokens));
        let split = match (c.cost_usd, listed) {
            (Some(cost), listed) => {
                d.reported_cost += cost;
                d.reported_calls += 1;
                // The list price's split, scaled to what the agent said the whole call cost.
                listed.map(|kinds| {
                    let sum: f64 = kinds.iter().sum();
                    kinds.map(|k| if sum > 0.0 { k * cost / sum } else { 0.0 })
                })
            }
            (None, Some(kinds)) => {
                d.estimated_cost += kinds.iter().sum::<f64>();
                d.estimated_calls += 1;
                Some(kinds)
            }
            (None, None) => {
                d.unpriced_calls += 1;
                None
            }
        };
        for (total, k) in d.cost_by_kind.iter_mut().zip(split.unwrap_or_default()) {
            *total += k;
        }
    }
    days.into_values().collect()
}

/// Writes every recorded call to `path` as CSV, oldest first: when (UTC) and the local day, what for, the agent and
/// model, its tokens, its cost and where that came from (`reported` by the agent, `estimated` from list prices, or
/// `none`).
pub fn export_usage(app: &App, path: &Path) -> Result<()> {
    let calls = lock(&app.store).usage()?;
    std::fs::write(path, csv(&calls))?;
    Ok(())
}

fn csv(calls: &[UsageRow]) -> String {
    let mut out = String::from(
        "at,day,feature,agent,model,input_tokens,output_tokens,cache_write_tokens,cache_read_tokens,cost_usd,\
         cost_from\n",
    );
    for c in calls {
        let tokens = Tokens {
            input: c.input_tokens,
            output: c.output_tokens,
            cache_write: c.cache_write_tokens,
            cache_read: c.cache_read_tokens,
        };
        let (cost, from) = match c.cost_usd {
            Some(cost) => (Some(cost), "reported"),
            None => match pricing::price(&c.model) {
                Some(p) => (Some(p.cost(&tokens).iter().sum()), "estimated"),
                None => (None, "none"),
            },
        };
        let _ = writeln!(
            out,
            "{},{},{},{},{},{},{},{},{},{},{from}",
            field(&c.at),
            field(&c.day),
            field(&c.feature),
            field(&c.agent),
            field(&c.model),
            c.input_tokens,
            c.output_tokens,
            c.cache_write_tokens,
            c.cache_read_tokens,
            cost.map_or(String::new(), |c| format!("{c:.6}")),
        );
    }
    out
}

/// `s` as one CSV field: quoted when it holds a comma, quote or line break.
fn field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(day: &str, feature: &str, agent: &str, model: &str, cost: Option<f64>) -> UsageRow {
        UsageRow {
            at: format!("{day}T10:00:00.000000Z"),
            day: day.to_owned(),
            feature: feature.to_owned(),
            agent: agent.to_owned(),
            model: model.to_owned(),
            input_tokens: 1_000_000,
            output_tokens: 100_000,
            cache_write_tokens: 0,
            cache_read_tokens: 1_000_000,
            cost_usd: cost,
        }
    }

    #[test]
    fn reported_costs_stay_as_said_and_the_rest_are_estimated_or_unpriced() {
        let rows = summed(&[
            call("2026-11-02", "chat", "claude", "claude-sonnet-5-5", Some(3.0)),
            call("2026-11-02", "chat", "claude", "claude-sonnet-5-5", Some(1.0)),
            call("2026-11-02", "feed-check", "codex", "gpt-6-luna", None),
            call("2026-11-02", "feed-check", "codex", "gpt-reserve", None),
            call("2026-11-03", "chat", "claude", "claude-sonnet-5-5", None),
        ]);
        assert_eq!(rows.len(), 4);
        let chat = &rows[0];
        assert_eq!((chat.calls, chat.reported_calls), (2, 2));
        assert_eq!(chat.reported_cost, 4.0);
        assert_eq!(chat.input_tokens, 2_000_000);
        // The split follows the list price's shape but adds up to what Claude Code said.
        assert!((chat.cost_by_kind.iter().sum::<f64>() - 4.0).abs() < 1e-9);
        let luna = rows.iter().find(|r| r.model == "gpt-6-luna").unwrap();
        // $0.10 for 1M fresh input, $0.05 for 100K output, $0.01 for 1M cache reads.
        assert!((luna.estimated_cost - 0.16).abs() < 1e-9);
        assert_eq!(luna.estimated_calls, 1);
        let unknown = rows.iter().find(|r| r.model == "gpt-reserve").unwrap();
        assert_eq!((unknown.unpriced_calls, unknown.estimated_cost), (1, 0.0));
        assert_eq!(unknown.cost_by_kind, [0.0; 4]);
        // A Claude call that said nothing is estimated like any other.
        assert_eq!(rows[3].day, "2026-11-03");
        assert_eq!(rows[3].estimated_calls, 1);
    }

    #[test]
    fn calls_this_build_cant_name_are_left_out() {
        let rows = summed(&[call("2026-11-02", "telepathy", "claude", "claude-sonnet-5-5", Some(1.0))]);
        assert!(rows.is_empty());
    }

    #[test]
    fn the_csv_says_where_each_cost_came_from() {
        let text = csv(&[
            call("2026-11-02", "chat", "claude", "claude-sonnet-5-5", Some(3.0)),
            call("2026-11-02", "feed-check", "codex", "gpt-6-luna", None),
            call("2026-11-02", "feed-check", "codex", "gpt,odd", None),
        ]);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[1].ends_with(",3.000000,reported"));
        assert!(lines[2].ends_with(",0.160000,estimated"));
        assert!(lines[3].contains(",\"gpt,odd\","));
        assert!(lines[3].ends_with(",,none"));
    }
}
