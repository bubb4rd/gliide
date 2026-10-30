//! `agy -p`: the Antigravity CLI, signed in to the user's own Google AI plan. Schema-enforced output
//! (`--json-schema`, as Gemini takes a schema), every tool denied (session-only deny rules, which hold in every
//! permission mode), no slash commands. It takes no system prompt, so the instructions open the prompt; and it reads
//! no prompt from stdin, so the prompt is an argument.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use tokio::process::Command;

use crate::{
    AgentKind, Attempt, Call, Llm, LlmError, Reply, Schema, Structured, Turn, Usage, cli,
    first_line, with_repair,
};

const AGENT: AgentKind = AgentKind::Antigravity;

/// `--settings` for one call, never saved: every action Antigravity's tools take, denied.
const DENY_ALL: &str = r#"{"permissions":{"deny":["read_file(*)","write_file(*)","read_url(*)","execute_url(*)","command(*)","mcp(*)"]}}"#;

/// The `agy -p` provider.
#[derive(Clone, Debug)]
pub struct AntigravityCli {
    bin: PathBuf,
    model: String,
    timeout: Duration,
    cwd: PathBuf,
}

impl AntigravityCli {
    /// The model calls run on unless the user picks another (`agy models` lists the slugs; the suffix is its thinking
    /// level). On the fixture postings' Extract, `-medium` thought for about 8,800 tokens a call and took 64–142 s;
    /// `-low` took 8.5 s at p50, every reply valid.
    pub const DEFAULT_MODEL: &str = "gemini-3.8-flash-low";
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(240);

    /// The provider for the `agy` binary at `bin`.
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self {
            bin: bin.into(),
            model: Self::DEFAULT_MODEL.to_owned(),
            timeout: Self::DEFAULT_TIMEOUT,
            cwd: std::env::temp_dir().join("resume-llm"),
        }
    }

    /// The model calls use when they name none: an `agy --model` slug.
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

impl Turn for AntigravityCli {
    /// One `agy -p` run.
    async fn turn(&self, call: Call<'_>, schema: &Schema) -> Result<(Reply, Attempt), LlmError> {
        cli::working_dir(AGENT, &self.cwd)?;
        let model = call.model.unwrap_or(&self.model);
        let mut cmd = Command::new(&self.bin);
        cmd.arg("-p")
            .arg(prompt(call.system, call.input))
            .args([
                "--output-format",
                "json",
                "--json-schema",
                &gemini_schema(schema),
                "--disable-slash-commands",
                "--settings",
                DENY_ALL,
                "--model",
                model,
            ])
            .current_dir(&self.cwd);
        let (out, latency_ms) = cli::run(AGENT, &self.bin, cmd, None, self.timeout).await?;

        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr);
        let envelope = serde_json::from_str::<Value>(stdout.trim())
            .ok()
            .filter(Value::is_object);
        let error = envelope
            .as_ref()
            .and_then(|e| e.get("error"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let message = || {
            let text = [error, &stderr, &stdout]
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
        let status = envelope.get("status").and_then(Value::as_str).unwrap_or("");
        if status != "SUCCESS" {
            let message = match first_line(error) {
                "" if status.is_empty() => "no status".to_owned(),
                "" => status.to_lowercase(),
                e => e.to_owned(),
            };
            return Err(LlmError::Agent {
                agent: AGENT,
                message,
            });
        }
        let reply = match envelope.get("structured_output") {
            Some(v) if !v.is_null() => Reply::Json(v.clone()),
            _ => match envelope.get("response").and_then(Value::as_str) {
                Some(text) => Reply::of_text(text),
                None => {
                    return Err(LlmError::Agent {
                        agent: AGENT,
                        message: "no structured output".to_owned(),
                    });
                }
            },
        };
        let usage = usage_of(envelope, model);
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

impl Llm for AntigravityCli {
    async fn structured(&self, call: Call<'_>, schema: &Schema) -> Result<Structured, LlmError> {
        with_repair(self, call, schema).await
    }
}

/// The instructions, then the input: the prompt `agy` gets in place of a system prompt.
fn prompt(system: &str, input: &str) -> String {
    let mut s = String::with_capacity(system.len() + input.len() + 40);
    s.push_str("<instructions>\n");
    s.push_str(system.trim());
    s.push_str("\n</instructions>\n\n");
    s.push_str(input);
    s
}

/// The usage fields of an `agy -p --output-format json` envelope; `model` served it (`agy` fails a pinned model
/// it doesn't know rather than fall back).
fn usage_of(envelope: &Value, model: &str) -> Usage {
    let u = envelope.get("usage");
    let n = |key: &str| u.and_then(|u| u.get(key)).and_then(Value::as_u64);
    Usage {
        input_tokens: n("input_tokens"),
        output_tokens: n("output_tokens"),
        cache_creation_input_tokens: None,
        cache_read_input_tokens: n("cache_read_tokens"),
        cost_usd: None,
        api_ms: None,
        turns: envelope.get("num_turns").and_then(Value::as_u64),
        models: vec![model.to_owned()],
    }
}

/// `schema` as Gemini takes it. Antigravity hands the schema to Gemini as a tool's parameters, whose `enum` holds
/// strings only: a `null` in an enum becomes a nullable `type`, and numbers become their type and range. The reply is
/// still checked against the schema as written.
fn gemini_schema(schema: &Schema) -> String {
    let mut v: Value = serde_json::from_str(schema.text()).expect("a parsed schema is JSON");
    strings_only_enums(&mut v);
    v.to_string()
}

fn strings_only_enums(v: &mut Value) {
    match v {
        Value::Object(o) => {
            if let Some(Value::Array(values)) = o.get("enum")
                && values.iter().any(|e| !e.is_string())
            {
                let nullable = values.iter().any(Value::is_null);
                let rest: Vec<Value> = values.iter().filter(|e| !e.is_null()).cloned().collect();
                let inferred = if rest.iter().all(Value::is_string) {
                    "string"
                } else if rest.iter().all(|e| e.is_i64() || e.is_u64()) {
                    "integer"
                } else if rest.iter().all(Value::is_number) {
                    "number"
                } else if rest.iter().all(Value::is_boolean) {
                    "boolean"
                } else {
                    ""
                };
                if inferred == "string" && !rest.is_empty() {
                    o.insert("enum".to_owned(), Value::Array(rest.clone()));
                } else {
                    o.remove("enum");
                }
                let numbers = rest.iter().filter(|e| e.is_number());
                let by = |a: &&Value, b: &&Value| {
                    a.as_f64()
                        .partial_cmp(&b.as_f64())
                        .expect("JSON numbers compare")
                };
                if let (Some(min), Some(max)) = (numbers.clone().min_by(by), numbers.max_by(by)) {
                    o.insert("minimum".to_owned(), min.clone());
                    o.insert("maximum".to_owned(), max.clone());
                }
                let ty = match o.get("type").and_then(Value::as_str) {
                    Some(t) => t.to_owned(),
                    None if rest.is_empty() => "null".to_owned(),
                    None => inferred.to_owned(),
                };
                if !ty.is_empty() {
                    let ty = if nullable && ty != "null" {
                        serde_json::json!([ty, "null"])
                    } else {
                        Value::String(ty)
                    };
                    o.insert("type".to_owned(), ty);
                }
            }
            o.values_mut().for_each(strings_only_enums);
        }
        Value::Array(a) => a.iter_mut().for_each(strings_only_enums),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn translated(schema: Value) -> Value {
        let schema = Schema::parse(&schema.to_string()).unwrap();
        serde_json::from_str(&gemini_schema(&schema)).unwrap()
    }

    #[test]
    fn number_enums_become_a_range_and_null_a_nullable_type() {
        let out = translated(json!({
            "type": "object",
            "properties": {
                "priority": { "enum": [1, 2, 3] },
                "kind": { "enum": ["technical", "other", null] },
                "tier": { "type": "integer", "enum": [3, 1] },
                "word": { "type": "string", "enum": ["a", "b"] }
            }
        }));
        let p = &out["properties"];
        assert_eq!(
            p["priority"],
            json!({ "type": "integer", "minimum": 1, "maximum": 3 })
        );
        assert_eq!(
            p["kind"],
            json!({ "type": ["string", "null"], "enum": ["technical", "other"] })
        );
        assert_eq!(
            p["tier"],
            json!({ "type": "integer", "minimum": 1, "maximum": 3 })
        );
        assert_eq!(p["word"], json!({ "type": "string", "enum": ["a", "b"] }));
    }

    #[test]
    fn every_built_in_schema_keeps_string_only_enums() {
        fn check(v: &Value, at: &str) {
            match v {
                Value::Object(o) => {
                    if let Some(Value::Array(e)) = o.get("enum") {
                        assert!(e.iter().all(Value::is_string), "{at}: {e:?}");
                    }
                    o.iter().for_each(|(k, v)| check(v, &format!("{at}/{k}")));
                }
                Value::Array(a) => a.iter().for_each(|v| check(v, at)),
                _ => {}
            }
        }
        for (name, text) in [
            ("extract", crate::extract::SCHEMA),
            ("suggest", crate::suggest::SCHEMA),
            ("invite", crate::invite::SCHEMA),
        ] {
            let schema = Schema::parse(text).unwrap();
            check(
                &serde_json::from_str(&gemini_schema(&schema)).unwrap(),
                name,
            );
        }
    }
}
