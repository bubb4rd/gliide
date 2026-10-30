//! `claude -p`: the unmodified Claude Code binary, signed in to the user's own subscription. Schema-enforced JSON
//! output, a replaced system prompt, no tools, no settings, no MCP, no session.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use tokio::process::Command;

use crate::{
    AgentKind, Attempt, Call, Llm, LlmError, Reply, Schema, Structured, Turn, Usage, cli,
    first_line, with_repair,
};

const AGENT: AgentKind = AgentKind::Claude;

/// The `claude -p` provider.
#[derive(Clone, Debug)]
pub struct ClaudeCli {
    bin: PathBuf,
    model: String,
    timeout: Duration,
    effort: Option<String>,
    cwd: PathBuf,
}

impl ClaudeCli {
    /// The model calls run on unless the user picks another: in the M0 spike, the fastest on Extract and the best at
    /// its priorities.
    pub const DEFAULT_MODEL: &str = "sonnet";
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(240);

    /// The provider for the `claude` binary at `bin`.
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self {
            bin: bin.into(),
            model: Self::DEFAULT_MODEL.to_owned(),
            timeout: Self::DEFAULT_TIMEOUT,
            effort: None,
            cwd: std::env::temp_dir().join("resume-llm"),
        }
    }

    /// The provider for the `claude` binary [`AgentKind::find`] finds.
    pub async fn discover() -> Result<Self, LlmError> {
        AGENT
            .find()
            .await
            .map(Self::new)
            .ok_or(LlmError::NotFound(AGENT))
    }

    /// The model calls use when they name none: a `claude --model` alias or full model name.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Time allowed per turn; a repair is a second turn with its own allowance.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// `claude --effort` (low, medium, high, xhigh, max); unset leaves Claude Code's default. In the M0 spike, `low`
    /// cut haiku's thinking on Extract by about 40% but did not turn it off.
    pub fn with_effort(mut self, effort: impl Into<String>) -> Self {
        self.effort = Some(effort.into());
        self
    }

    pub fn bin(&self) -> &Path {
        &self.bin
    }
}

impl Turn for ClaudeCli {
    /// One `claude -p` run.
    async fn turn(&self, call: Call<'_>, schema: &Schema) -> Result<(Reply, Attempt), LlmError> {
        cli::working_dir(AGENT, &self.cwd)?;
        let mut cmd = Command::new(&self.bin);
        cmd.args([
            "-p",
            "--output-format",
            "json",
            "--json-schema",
            schema.text(),
            "--system-prompt",
            call.system,
        ])
        .args(["--model", call.model.unwrap_or(&self.model)])
        .args(
            self.effort
                .as_deref()
                .into_iter()
                .flat_map(|e| ["--effort", e]),
        )
        .args([
            "--tools",
            "",
            "--setting-sources",
            "",
            "--strict-mcp-config",
            "--no-session-persistence",
        ])
        .current_dir(&self.cwd);
        let (out, latency_ms) = cli::run(
            AGENT,
            &self.bin,
            cmd,
            Some(call.input.as_bytes()),
            self.timeout,
        )
        .await?;

        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr);
        let envelope = serde_json::from_str::<Value>(stdout.trim())
            .ok()
            .filter(Value::is_object);
        let message = || {
            let result = envelope
                .as_ref()
                .and_then(|e| e.get("result"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let text = [result, &stderr, &stdout]
                .into_iter()
                .find(|s| !s.trim().is_empty())
                .unwrap_or("");
            first_line(text).to_owned()
        };
        if !out.status.success() {
            return Err(LlmError::Exit {
                agent: AGENT,
                code: out.status.code(),
                message: message(),
            });
        }
        let Some(envelope) = &envelope else {
            return Err(LlmError::Agent {
                agent: AGENT,
                message: format!("unreadable output: {}", message()),
            });
        };
        if envelope
            .get("is_error")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            let result = envelope
                .get("result")
                .and_then(Value::as_str)
                .map(first_line)
                .unwrap_or("");
            let subtype = envelope
                .get("subtype")
                .and_then(Value::as_str)
                .unwrap_or("error");
            let message = if result.is_empty() { subtype } else { result };
            return Err(LlmError::Agent {
                agent: AGENT,
                message: message.to_owned(),
            });
        }
        let reply = match envelope.get("structured_output") {
            Some(v) if !v.is_null() => Reply::Json(v.clone()),
            _ => match envelope.get("result").and_then(Value::as_str) {
                Some(text) => Reply::of_text(text),
                None => {
                    return Err(LlmError::Agent {
                        agent: AGENT,
                        message: "no structured output".to_owned(),
                    });
                }
            },
        };
        let usage = usage_of(envelope);
        Ok((
            reply,
            Attempt {
                latency_ms,
                usage,
                raw: stdout,
            },
        ))
    }
}

impl Llm for ClaudeCli {
    async fn structured(&self, call: Call<'_>, schema: &Schema) -> Result<Structured, LlmError> {
        with_repair(self, call, schema).await
    }
}

/// The usage fields of a `claude -p --output-format json` envelope.
fn usage_of(envelope: &Value) -> Usage {
    let u = envelope.get("usage");
    let n = |key: &str| u.and_then(|u| u.get(key)).and_then(Value::as_u64);
    Usage {
        input_tokens: n("input_tokens"),
        output_tokens: n("output_tokens"),
        cache_creation_input_tokens: n("cache_creation_input_tokens"),
        cache_read_input_tokens: n("cache_read_input_tokens"),
        cost_usd: envelope.get("total_cost_usd").and_then(Value::as_f64),
        api_ms: envelope.get("duration_api_ms").and_then(Value::as_u64),
        turns: envelope.get("num_turns").and_then(Value::as_u64),
        models: envelope
            .get("modelUsage")
            .and_then(Value::as_object)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default(),
    }
}
