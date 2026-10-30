//! Model access: the [`Llm`] trait, the agent CLI providers (`claude -p`, `codex exec`, `agy -p`) and the built-in
//! prompts.
//!
//! Every call is stateless: instructions, input and a JSON Schema in, JSON valid against that schema out. Output is
//! validated here, whatever the provider promises; an invalid reply gets one repair turn that quotes the validation
//! errors, then the call fails with the raw text.

mod agent;
mod antigravity;
mod claude;
mod cli;
mod codex;
pub mod cover;
pub mod debrief;
pub mod dig;
pub mod extract;
pub mod import;
pub mod instruct;
pub mod invite;
pub mod nudge;
pub mod practice;
pub mod prep;
pub mod rewrite;
pub mod suggest;

use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

pub use agent::{Agent, AgentKind};
pub use antigravity::AntigravityCli;
pub use claude::ClaudeCli;
pub use codex::CodexCli;

/// One stateless call: no tools, no history.
#[derive(Clone, Copy, Debug)]
pub struct Call<'a> {
    /// The instructions (system prompt).
    pub system: &'a str,
    /// The user turn.
    pub input: &'a str,
    /// A model alias or full name; `None` uses the provider's default.
    pub model: Option<&'a str>,
}

/// A model provider.
pub trait Llm {
    /// One stateless call: instructions + input → JSON valid against `schema`.
    fn structured(
        &self,
        call: Call<'_>,
        schema: &Schema,
    ) -> impl Future<Output = Result<Structured, LlmError>> + Send;
}

/// One turn with an agent CLI: its reply before validation, and the turn's record.
pub(crate) trait Turn: Sync {
    fn turn(
        &self,
        call: Call<'_>,
        schema: &Schema,
    ) -> impl Future<Output = Result<(Reply, Attempt), LlmError>> + Send;
}

/// `agent`'s turn, and when its reply fails `schema`, exactly one repair turn quoting the errors.
pub(crate) async fn with_repair(
    agent: &impl Turn,
    call: Call<'_>,
    schema: &Schema,
) -> Result<Structured, LlmError> {
    let (reply, attempt) = agent.turn(call, schema).await?;
    let mut attempts = vec![attempt];
    let rejected = match check(schema, reply) {
        Ok(value) => return Ok(Structured { value, attempts }),
        Err(r) => r,
    };
    let input = repair_input(call.input, &rejected);
    let (reply, attempt) = agent
        .turn(
            Call {
                input: &input,
                ..call
            },
            schema,
        )
        .await?;
    attempts.push(attempt);
    match check(schema, reply) {
        Ok(value) => Ok(Structured { value, attempts }),
        Err(r) => Err(LlmError::Invalid {
            errors: r.errors,
            raw: r.raw,
            attempts,
        }),
    }
}

/// A JSON Schema: its text (handed to providers that enforce it) and its compiled validator.
pub struct Schema {
    text: String,
    validator: jsonschema::Validator,
}

impl Schema {
    pub fn parse(text: &str) -> Result<Self, LlmError> {
        let json: Value =
            serde_json::from_str(text).map_err(|e| LlmError::Schema(e.to_string()))?;
        let validator =
            jsonschema::validator_for(&json).map_err(|e| LlmError::Schema(e.to_string()))?;
        Ok(Self {
            text: text.to_owned(),
            validator,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Every validation error in `value`, as `<json pointer>: <message>`; empty when valid.
    pub fn errors(&self, value: &Value) -> Vec<String> {
        self.validator
            .iter_errors(value)
            .map(|e| {
                let at = e.instance_path().to_string();
                format!("{}: {e}", if at.is_empty() { "/" } else { &at })
            })
            .collect()
    }
}

impl std::fmt::Debug for Schema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Schema")
            .field("text", &self.text)
            .finish_non_exhaustive()
    }
}

/// A schema-valid reply and the turns it took (two when the first needed a repair).
#[derive(Clone, Debug, Serialize)]
pub struct Structured {
    pub value: Value,
    pub attempts: Vec<Attempt>,
}

impl Structured {
    pub fn repaired(&self) -> bool {
        self.attempts.len() > 1
    }

    /// Wall time of every turn together.
    pub fn latency(&self) -> Duration {
        self.attempts
            .iter()
            .map(|a| Duration::from_millis(a.latency_ms))
            .sum()
    }

    /// The model id that served the last turn for `asked` (a `--model` alias or full name): the one naming it, else
    /// the first; `None` when the agent reported none.
    pub fn served(&self, asked: &str) -> Option<&str> {
        let models = &self.attempts.last()?.usage.models;
        let asked = asked.to_lowercase();
        models
            .iter()
            .find(|m| m.to_lowercase().contains(&asked))
            .or_else(|| models.first())
            .map(String::as_str)
    }
}

/// One turn with the agent.
#[derive(Clone, Debug, Serialize)]
pub struct Attempt {
    /// Wall time from spawn to exit.
    pub latency_ms: u64,
    pub usage: Usage,
    /// The agent's raw output (`claude -p`'s and `agy -p`'s JSON result envelope, `codex exec`'s JSON Lines events).
    pub raw: String,
}

/// Whatever usage the agent reports; `None` when it reports nothing.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    /// What the call would cost at API prices; on a subscription it is a measure, not a bill.
    pub cost_usd: Option<f64>,
    /// Time the agent spent waiting on the model API.
    pub api_ms: Option<u64>,
    pub turns: Option<u64>,
    /// Models that served the turn.
    pub models: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("{} (`{}`) not found; install it or set its path", .0.name(), .0.bin())]
    NotFound(AgentKind),
    #[error("could not start {}: {source}", bin.display())]
    Spawn {
        agent: AgentKind,
        bin: PathBuf,
        source: std::io::Error,
    },
    #[error("{} timed out after {}s", agent.name(), after.as_secs())]
    Timeout { agent: AgentKind, after: Duration },
    #[error("{} exited {}: {message}", agent.name(), code.map_or("by signal".to_owned(), |c| c.to_string()))]
    Exit {
        agent: AgentKind,
        code: Option<i32>,
        message: String,
    },
    /// The agent ran but reported an error or returned no output.
    #[error("{}: {message}", agent.name())]
    Agent { agent: AgentKind, message: String },
    #[error("invalid schema: {0}")]
    Schema(String),
    /// Still invalid after the repair turn; `raw` is the last reply as text.
    #[error("output failed the schema after one repair: {}", errors.join("; "))]
    Invalid {
        errors: Vec<String>,
        raw: String,
        attempts: Vec<Attempt>,
    },
    /// Schema-valid output that doesn't fit the Rust type the call returns (the schema and the type disagree).
    #[error("output does not fit {target}: {message}")]
    Decode {
        target: &'static str,
        message: String,
        output: Structured,
    },
}

impl LlmError {
    /// The agent the error came from; `None` for the schema and output-shape errors, which are gliide's own checks.
    pub fn agent(&self) -> Option<AgentKind> {
        match self {
            Self::NotFound(agent)
            | Self::Spawn { agent, .. }
            | Self::Timeout { agent, .. }
            | Self::Exit { agent, .. }
            | Self::Agent { agent, .. } => Some(*agent),
            Self::Schema(_) | Self::Invalid { .. } | Self::Decode { .. } => None,
        }
    }
}

/// What one turn produced, before validation.
pub(crate) enum Reply {
    Json(Value),
    /// Output that isn't JSON.
    Text(String),
}

impl Reply {
    /// `text` as JSON when it parses, else as text.
    pub(crate) fn of_text(text: &str) -> Self {
        serde_json::from_str(text.trim()).map_or_else(|_| Self::Text(text.to_owned()), Self::Json)
    }
}

/// A reply that failed validation.
pub(crate) struct Rejected {
    errors: Vec<String>,
    raw: String,
}

/// `reply` as a schema-valid value, or why not.
pub(crate) fn check(schema: &Schema, reply: Reply) -> Result<Value, Rejected> {
    match reply {
        Reply::Json(value) => {
            let errors = schema.errors(&value);
            if errors.is_empty() {
                Ok(value)
            } else {
                let raw = serde_json::to_string_pretty(&value).unwrap_or_default();
                Err(Rejected { errors, raw })
            }
        }
        Reply::Text(raw) => Err(Rejected {
            errors: vec!["/: not JSON".to_owned()],
            raw,
        }),
    }
}

/// The repair turn's input: the original input, the rejected reply and its validation errors.
pub(crate) fn repair_input(input: &str, rejected: &Rejected) -> String {
    let mut s = String::with_capacity(input.len() + rejected.raw.len() + 512);
    s.push_str(input.trim_end());
    s.push_str("\n\n<previous_output>\n");
    s.push_str(rejected.raw.trim());
    s.push_str("\n</previous_output>\n<validation_errors>\n");
    for e in &rejected.errors {
        s.push_str("- ");
        s.push_str(e);
        s.push('\n');
    }
    s.push_str(
        "</validation_errors>\nYour previous output failed the output schema with the errors above. Return it \
         again, corrected: fix what the errors name and keep everything else.\n",
    );
    s
}

/// First non-empty line of `s`, trimmed.
pub(crate) fn first_line(s: &str) -> &str {
    s.trim().lines().next().unwrap_or("").trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn served_by(models: &[&str]) -> Structured {
        Structured {
            value: Value::Null,
            attempts: vec![Attempt {
                latency_ms: 0,
                usage: Usage {
                    models: models.iter().map(|m| (*m).to_owned()).collect(),
                    ..Usage::default()
                },
                raw: String::new(),
            }],
        }
    }

    #[test]
    fn served_prefers_the_model_naming_the_alias() {
        let out = served_by(&["claude-haiku-4-5", "claude-sonnet-5-5-20261001"]);
        assert_eq!(out.served("Sonnet"), Some("claude-sonnet-5-5-20261001"));
        assert_eq!(out.served("claude-haiku-4-5"), Some("claude-haiku-4-5"));
    }

    #[test]
    fn served_falls_back_to_the_first_model() {
        let out = served_by(&["claude-opus-5-5", "claude-haiku-4-5"]);
        assert_eq!(out.served("sonnet"), Some("claude-opus-5-5"));
    }

    #[test]
    fn served_is_none_when_no_model_was_reported() {
        assert_eq!(served_by(&[]).served("sonnet"), None);
    }
}
