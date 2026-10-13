//! Model access: the [`Llm`] trait, the `claude -p` provider and the built-in prompts.
//!
//! Every call is stateless: instructions, input and a JSON Schema in, JSON valid against that schema out. Output is
//! validated here, whatever the provider promises; an invalid reply gets one repair turn that quotes the validation
//! errors, then the call fails with the raw text.

mod claude;
pub mod extract;

use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

pub use claude::{ClaudeCli, find_claude};

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
}

/// One turn with the agent.
#[derive(Clone, Debug, Serialize)]
pub struct Attempt {
    /// Wall time from spawn to exit.
    pub latency_ms: u64,
    pub usage: Usage,
    /// The agent's raw output (for `claude -p`, its JSON result envelope).
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
    #[error("`{0}` CLI not found; install it or set its path")]
    NotFound(String),
    #[error("could not start {}: {source}", bin.display())]
    Spawn {
        bin: PathBuf,
        source: std::io::Error,
    },
    #[error("model timed out after {}s", after.as_secs())]
    Timeout { after: Duration },
    #[error("{agent} exited {}: {message}", code.map_or("by signal".to_owned(), |c| c.to_string()))]
    Exit {
        agent: String,
        code: Option<i32>,
        message: String,
    },
    /// The agent ran but reported an error or returned no output.
    #[error("{agent}: {message}")]
    Agent { agent: String, message: String },
    #[error("invalid schema: {0}")]
    Schema(String),
    /// Still invalid after the repair turn; `raw` is the last reply as text.
    #[error("output failed the schema after one repair: {}", errors.join("; "))]
    Invalid {
        errors: Vec<String>,
        raw: String,
        attempts: Vec<Attempt>,
    },
}

/// What one turn produced, before validation.
pub(crate) enum Reply {
    Json(Value),
    /// Output that isn't JSON.
    Text(String),
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
