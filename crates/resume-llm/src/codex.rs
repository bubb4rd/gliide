//! `codex exec`: the Codex CLI, signed in to the user's own ChatGPT plan. Schema-enforced output
//! (`--output-schema`), the instructions in place of Codex's own (`model_instructions_file`), no shell, images,
//! sub-agents or web search, no user config or rules, no session files.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use tokio::process::Command;

use crate::{
    AgentKind, Attempt, Call, Llm, LlmError, Reply, Schema, Structured, Turn, Usage, cli,
    first_line, with_repair,
};

const AGENT: AgentKind = AgentKind::Codex;

/// The Codex features a call turns off: both shells, image viewing and sub-agents. Codex only warns about a name it
/// doesn't know, so one that a later version drops doesn't stop the call.
const FEATURES_OFF: [&str; 4] = ["shell_tool", "unified_exec", "view_image", "multi_agent"];

/// The `codex exec` provider.
#[derive(Clone, Debug)]
pub struct CodexCli {
    bin: PathBuf,
    model: String,
    timeout: Duration,
    cwd: PathBuf,
}

impl CodexCli {
    /// The model calls run on unless the user picks another.
    pub const DEFAULT_MODEL: &str = "gpt-6-luna";
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(240);

    /// The provider for the `codex` binary at `bin`.
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self {
            bin: bin.into(),
            model: Self::DEFAULT_MODEL.to_owned(),
            timeout: Self::DEFAULT_TIMEOUT,
            cwd: std::env::temp_dir().join("resume-llm"),
        }
    }

    /// The model calls use when they name none: a `codex --model` slug.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Time allowed per turn; a repair is a second turn with its own allowance.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn bin(&self) -> &Path {
        &self.bin
    }
}

impl Turn for CodexCli {
    /// One `codex exec` run: the input on stdin, its events as JSON Lines on stdout.
    async fn turn(&self, call: Call<'_>, schema: &Schema) -> Result<(Reply, Attempt), LlmError> {
        cli::working_dir(AGENT, &self.cwd)?;
        // Codex reads the schema and the instructions from files, kept apart from the working folder.
        let files = tempfile::Builder::new()
            .prefix("resume-llm-codex-")
            .tempdir()
            .map_err(|source| LlmError::Spawn {
                agent: AGENT,
                bin: std::env::temp_dir(),
                source,
            })?;
        let schema_file = files.path().join("schema.json");
        let instructions = files.path().join("instructions.md");
        for (path, text) in [(&schema_file, schema.text()), (&instructions, call.system)] {
            std::fs::write(path, text).map_err(|source| LlmError::Spawn {
                agent: AGENT,
                bin: path.clone(),
                source,
            })?;
        }
        let model = call.model.unwrap_or(&self.model);
        let mut cmd = Command::new(&self.bin);
        cmd.args([
            "exec",
            "--json",
            "--ephemeral",
            "--skip-git-repo-check",
            "--ignore-user-config",
            "--ignore-rules",
            "--sandbox",
            "read-only",
        ])
        .arg("--output-schema")
        .arg(&schema_file)
        .arg("-c")
        .arg(format!(
            "model_instructions_file={}",
            toml_string(&instructions.to_string_lossy())
        ))
        .args(["-c", "web_search=\"disabled\""])
        .args(
            FEATURES_OFF
                .iter()
                .flat_map(|f| ["-c".to_owned(), format!("features.{f}=false")]),
        )
        .args(["--model", model, "-"])
        .current_dir(&self.cwd);
        let (out, latency_ms) = cli::run(
            AGENT,
            &self.bin,
            cmd,
            Some(call.input.as_bytes()),
            self.timeout,
        )
        .await?;
        drop(files);

        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr);
        let events = Events::read(&stdout);
        if !out.status.success() {
            let text = [events.error.as_deref().unwrap_or(""), &stderr, &stdout]
                .into_iter()
                .find(|s| !s.trim().is_empty())
                .unwrap_or("");
            return Err(LlmError::Exit {
                agent: AGENT,
                code: out.status.code(),
                message: first_line(text).to_owned(),
            });
        }
        if let Some(error) = &events.error {
            return Err(LlmError::Agent {
                agent: AGENT,
                message: first_line(error).to_owned(),
            });
        }
        let Some(text) = &events.message else {
            return Err(LlmError::Agent {
                agent: AGENT,
                message: "no answer".to_owned(),
            });
        };
        let reply = Reply::of_text(text);
        let usage = events.usage(model);
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

impl Llm for CodexCli {
    async fn structured(&self, call: Call<'_>, schema: &Schema) -> Result<Structured, LlmError> {
        with_repair(self, call, schema).await
    }
}

/// What a `codex exec --json` run's events say: its last message, the error that ended it, and its usage.
#[derive(Debug, Default)]
struct Events {
    message: Option<String>,
    /// A failed turn's or the stream's error. Error items (unknown settings, a tool that's off) are warnings and
    /// don't count.
    error: Option<String>,
    usage: Option<Value>,
}

impl Events {
    fn read(stdout: &str) -> Self {
        let mut events = Self::default();
        for line in stdout.lines() {
            let Ok(event) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(str::to_owned);
            match event.get("type").and_then(Value::as_str) {
                Some("item.completed") => {
                    let item = &event["item"];
                    if item.get("type").and_then(Value::as_str) == Some("agent_message") {
                        events.message = text(item, "text");
                    }
                }
                Some("turn.completed") => events.usage = event.get("usage").cloned(),
                Some("turn.failed") => events.error = text(&event["error"], "message"),
                Some("error") => events.error = text(&event, "message"),
                _ => {}
            }
        }
        events
    }

    /// The turn's token counts; `model` served it (Codex fails a call rather than switch models).
    fn usage(&self, model: &str) -> Usage {
        let n = |key: &str| {
            self.usage
                .as_ref()
                .and_then(|u| u.get(key))
                .and_then(Value::as_u64)
        };
        Usage {
            input_tokens: n("input_tokens"),
            output_tokens: n("output_tokens"),
            cache_creation_input_tokens: n("cache_write_input_tokens"),
            cache_read_input_tokens: n("cached_input_tokens"),
            cost_usd: None,
            api_ms: None,
            turns: self.usage.is_some().then_some(1),
            models: vec![model.to_owned()],
        }
    }
}

/// `s` as a TOML basic string (a JSON string is one).
fn toml_string(s: &str) -> String {
    serde_json::to_string(s).expect("a string serializes")
}
