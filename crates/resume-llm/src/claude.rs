//! `claude -p`: the unmodified Claude Code binary, signed in to the user's own subscription. Schema-enforced JSON
//! output, a replaced system prompt, no tools, no settings, no MCP, no session.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::{
    Attempt, Call, Llm, LlmError, Reply, Schema, Structured, Usage, check, first_line, repair_input,
};

const AGENT: &str = "claude";

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

    /// The provider for the `claude` binary [`find_claude`] finds.
    pub async fn discover() -> Result<Self, LlmError> {
        find_claude()
            .await
            .map(Self::new)
            .ok_or_else(|| LlmError::NotFound(AGENT.to_owned()))
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

    /// One `claude -p` run.
    async fn turn(&self, call: Call<'_>, schema: &Schema) -> Result<(Reply, Attempt), LlmError> {
        let spawn_err = |source: std::io::Error| {
            if source.kind() == std::io::ErrorKind::NotFound {
                LlmError::NotFound(self.bin.display().to_string())
            } else {
                LlmError::Spawn {
                    bin: self.bin.clone(),
                    source,
                }
            }
        };
        // An empty working folder: nothing for the agent to pick up as context.
        std::fs::create_dir_all(&self.cwd).map_err(|source| LlmError::Spawn {
            bin: self.cwd.clone(),
            source,
        })?;
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
        .current_dir(&self.cwd)
        .env("PATH", path_with(&self.bin))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

        let started = Instant::now();
        let mut child = cmd.spawn().map_err(spawn_err)?;
        let mut stdin = child.stdin.take().expect("stdin is piped");
        let input = call.input.as_bytes();
        let run = async {
            let write = async move {
                // A child that exits without reading closes the pipe; its output says why, so the error is moot.
                let _ = stdin.write_all(input).await;
            };
            tokio::join!(write, child.wait_with_output()).1
        };
        // On timeout, dropping `run` drops the child, which kills it.
        let out = tokio::time::timeout(self.timeout, run)
            .await
            .map_err(|_| LlmError::Timeout {
                after: self.timeout,
            })?
            .map_err(|source| LlmError::Spawn {
                bin: self.bin.clone(),
                source,
            })?;
        let latency_ms = started.elapsed().as_millis() as u64;

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
                agent: AGENT.to_owned(),
                code: out.status.code(),
                message: message(),
            });
        }
        let Some(envelope) = &envelope else {
            return Err(LlmError::Agent {
                agent: AGENT.to_owned(),
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
                agent: AGENT.to_owned(),
                message: message.to_owned(),
            });
        }
        let reply = match envelope.get("structured_output") {
            Some(v) if !v.is_null() => Reply::Json(v.clone()),
            _ => match envelope.get("result").and_then(Value::as_str) {
                Some(text) => serde_json::from_str(text)
                    .map_or_else(|_| Reply::Text(text.to_owned()), Reply::Json),
                None => {
                    return Err(LlmError::Agent {
                        agent: AGENT.to_owned(),
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
        let (reply, attempt) = self.turn(call, schema).await?;
        let mut attempts = vec![attempt];
        let rejected = match check(schema, reply) {
            Ok(value) => return Ok(Structured { value, attempts }),
            Err(r) => r,
        };
        let input = repair_input(call.input, &rejected);
        let (reply, attempt) = self
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
}

/// `PATH` with the binary's own folder first, so an nvm install's `node` shebang resolves.
fn path_with(bin: &Path) -> OsString {
    let mut dirs: Vec<PathBuf> = bin.parent().map(Path::to_path_buf).into_iter().collect();
    match std::env::var_os("PATH") {
        Some(path) => dirs.extend(std::env::split_paths(&path)),
        None => dirs.extend(["/usr/bin", "/bin"].map(PathBuf::from)),
    }
    std::env::join_paths(dirs).unwrap_or_default()
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

/// The `claude` binary: the usual install locations (`~/.local/bin`, `~/.claude/local`, Homebrew, `/usr/local/bin`,
/// then nvm's Node versions, newest first), else whatever a login shell finds.
pub async fn find_claude() -> Option<PathBuf> {
    find_cli(AGENT).await
}

async fn find_cli(name: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut candidates = Vec::new();
    if let Some(home) = &home {
        candidates.push(home.join(".local/bin").join(name));
        candidates.push(home.join(".claude/local").join(name));
    }
    candidates.push(Path::new("/opt/homebrew/bin").join(name));
    candidates.push(Path::new("/usr/local/bin").join(name));
    if let Some(home) = &home {
        candidates.extend(nvm_bins(&home.join(".nvm/versions/node"), name));
    }
    if let Some(found) = candidates.into_iter().find(|c| c.is_file()) {
        return Some(found);
    }

    let mut shell = Command::new("/bin/zsh");
    shell
        .args(["-lic", &format!("command -v {name}")])
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let out = tokio::time::timeout(Duration::from_secs(10), shell.output())
        .await
        .ok()?
        .ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let last = stdout.trim_end().lines().last()?.trim();
    (last.starts_with('/') && last.ends_with(&format!("/{name}"))).then(|| PathBuf::from(last))
}

/// `<nvm>/<version>/bin/<name>` for every installed Node version, newest first.
fn nvm_bins(nvm: &Path, name: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(nvm) else {
        return Vec::new();
    };
    let mut versions: Vec<(Vec<u64>, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|e| {
            let key = e
                .file_name()
                .to_string_lossy()
                .trim_start_matches('v')
                .split('.')
                .map(|p| p.parse().unwrap_or(0))
                .collect();
            (key, e.path().join("bin").join(name))
        })
        .collect();
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    versions.into_iter().map(|(_, p)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvm_versions_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        for v in ["v9.11.2", "v20.19.0", "v18.2.0"] {
            std::fs::create_dir_all(dir.path().join(v).join("bin")).unwrap();
        }
        let found: Vec<String> = nvm_bins(dir.path(), "claude")
            .iter()
            .map(|p| {
                p.parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(found, ["v20.19.0", "v18.2.0", "v9.11.2"]);
    }
}
