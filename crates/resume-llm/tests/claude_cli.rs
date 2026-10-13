//! `ClaudeCli` against a fake `claude`: a shell script that records each call's arguments and stdin, then prints
//! the reply queued for that call and exits with its status.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use resume_llm::{Call, ClaudeCli, Llm, LlmError, Schema};
use serde_json::{Value, json};

const SCHEMA: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "required": ["greeting"],
  "properties": { "greeting": { "type": "string" } }
}"#;

const SCRIPT: &str = r#"#!/bin/sh
dir="$(dirname "$0")"
n=$(( $(cat "$dir/count" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$dir/count"
for a in "$@"; do printf '%s\n' "$a"; done > "$dir/args.$n"
cat > "$dir/stdin.$n"
if [ -f "$dir/sleep" ]; then sleep "$(cat "$dir/sleep")"; fi
reply="$dir/reply.$n"; [ -f "$reply" ] || reply="$dir/reply"
cat "$reply"
status="$dir/status.$n"; [ -f "$status" ] || status="$dir/status"
exit "$(cat "$status" 2>/dev/null || echo 0)"
"#;

struct Fake {
    dir: tempfile::TempDir,
}

impl Fake {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("claude");
        std::fs::write(&bin, SCRIPT).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Queues `body` for call `n` (or every call, with `None`).
    fn reply(&self, n: Option<u32>, body: &str) -> &Self {
        let name = n.map_or("reply".to_owned(), |n| format!("reply.{n}"));
        std::fs::write(self.path(&name), body).unwrap();
        self
    }

    fn status(&self, code: i32) -> &Self {
        std::fs::write(self.path("status"), code.to_string()).unwrap();
        self
    }

    fn sleep(&self, secs: u32) -> &Self {
        std::fs::write(self.path("sleep"), secs.to_string()).unwrap();
        self
    }

    fn calls(&self) -> u32 {
        std::fs::read_to_string(self.path("count")).map_or(0, |s| s.trim().parse().unwrap())
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.path(name)).unwrap()
    }

    fn provider(&self) -> ClaudeCli {
        ClaudeCli::new(self.path("claude"))
            .with_model("haiku")
            .with_timeout(Duration::from_secs(10))
    }
}

fn envelope(structured: Value) -> String {
    json!({
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "duration_api_ms": 812,
        "num_turns": 2,
        "result": structured.to_string(),
        "structured_output": structured,
        "total_cost_usd": 0.0012,
        "usage": { "input_tokens": 10, "output_tokens": 20, "cache_creation_input_tokens": 30, "cache_read_input_tokens": 40 },
        "modelUsage": { "claude-haiku-test": { "costUSD": 0.0012 } }
    })
    .to_string()
}

const CALL: Call<'static> = Call {
    system: "Return a greeting.",
    input: "<posting>\nhi\n</posting>\n",
    model: None,
};

async fn run(fake: &Fake) -> Result<resume_llm::Structured, LlmError> {
    let schema = Schema::parse(SCHEMA).unwrap();
    fake.provider().structured(CALL, &schema).await
}

#[tokio::test]
async fn valid_output_passes_in_one_call() {
    let fake = Fake::new();
    fake.reply(None, &envelope(json!({ "greeting": "hello" })));
    let out = run(&fake).await.unwrap();
    assert_eq!(out.value, json!({ "greeting": "hello" }));
    assert!(!out.repaired());
    assert_eq!(fake.calls(), 1);

    let usage = &out.attempts[0].usage;
    assert_eq!(usage.cost_usd, Some(0.0012));
    assert_eq!(
        (usage.input_tokens, usage.output_tokens),
        (Some(10), Some(20))
    );
    assert_eq!(
        (
            usage.cache_creation_input_tokens,
            usage.cache_read_input_tokens
        ),
        (Some(30), Some(40))
    );
    assert_eq!((usage.api_ms, usage.turns), (Some(812), Some(2)));
    assert_eq!(usage.models, ["claude-haiku-test"]);

    // ATS Fit's call: input on stdin, schema and prompt as flags, no tools, settings, MCP or session.
    assert_eq!(fake.read("stdin.1"), CALL.input);
    let args: Vec<String> = fake.read("args.1").lines().map(str::to_owned).collect();
    let schema_text = SCHEMA.lines().collect::<Vec<_>>();
    let mut expected: Vec<&str> = vec!["-p", "--output-format", "json", "--json-schema"];
    expected.extend(&schema_text);
    expected.extend([
        "--system-prompt",
        "Return a greeting.",
        "--model",
        "haiku",
        "--tools",
        "",
        "--setting-sources",
        "",
        "--strict-mcp-config",
        "--no-session-persistence",
    ]);
    assert_eq!(args, expected);
}

#[tokio::test]
async fn effort_follows_the_model() {
    let fake = Fake::new();
    fake.reply(None, &envelope(json!({ "greeting": "hello" })));
    let schema = Schema::parse(SCHEMA).unwrap();
    let call = Call {
        model: Some("sonnet"),
        ..CALL
    };
    fake.provider()
        .with_effort("low")
        .structured(call, &schema)
        .await
        .unwrap();
    let args = fake.read("args.1");
    assert!(
        args.contains("--model\nsonnet\n--effort\nlow\n--tools\n"),
        "{args}"
    );
}

#[tokio::test]
async fn invalid_output_gets_exactly_one_repair() {
    let fake = Fake::new();
    fake.reply(Some(1), &envelope(json!({ "greeting": 7 })));
    fake.reply(Some(2), &envelope(json!({ "greeting": "fixed" })));
    let out = run(&fake).await.unwrap();
    assert_eq!(out.value, json!({ "greeting": "fixed" }));
    assert!(out.repaired());
    assert_eq!(out.attempts.len(), 2);
    assert_eq!(fake.calls(), 2);

    let repair = fake.read("stdin.2");
    assert!(repair.starts_with(CALL.input.trim_end()), "{repair}");
    assert!(repair.contains("<previous_output>"), "{repair}");
    assert!(repair.contains("\"greeting\": 7"), "{repair}");
    assert!(
        repair.contains("- /greeting: 7 is not of type \"string\""),
        "{repair}"
    );
}

#[tokio::test]
async fn still_invalid_after_repair_fails_with_raw_text() {
    let fake = Fake::new();
    fake.reply(None, &envelope(json!({ "greeting": "hi", "extra": true })));
    let err = run(&fake).await.unwrap_err();
    assert_eq!(fake.calls(), 2);
    let LlmError::Invalid {
        errors,
        raw,
        attempts,
    } = err
    else {
        panic!("{err:?}")
    };
    assert_eq!(attempts.len(), 2);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(raw.contains("\"extra\": true"), "{raw}");
}

#[tokio::test]
async fn non_json_text_gets_one_repair() {
    let fake = Fake::new();
    let text =
        json!({ "type": "result", "is_error": false, "result": "Sure! Here is a greeting." })
            .to_string();
    fake.reply(Some(1), &text);
    fake.reply(Some(2), &envelope(json!({ "greeting": "hello" })));
    let out = run(&fake).await.unwrap();
    assert!(out.repaired());
    assert!(fake.read("stdin.2").contains("Sure! Here is a greeting."));
}

#[tokio::test]
async fn non_zero_exit_is_an_error() {
    let fake = Fake::new();
    fake.reply(None, "Invalid API key · Please run /login\n")
        .status(1);
    let err = run(&fake).await.unwrap_err();
    assert_eq!(fake.calls(), 1);
    let LlmError::Exit { code, message, .. } = err else {
        panic!("{err:?}")
    };
    assert_eq!(code, Some(1));
    assert_eq!(message, "Invalid API key · Please run /login");
}

#[tokio::test]
async fn reported_error_is_an_error() {
    let fake = Fake::new();
    let body =
        json!({ "type": "result", "subtype": "error_max_turns", "is_error": true }).to_string();
    fake.reply(None, &body);
    let err = run(&fake).await.unwrap_err();
    assert_eq!(fake.calls(), 1);
    assert!(
        matches!(&err, LlmError::Agent { message, .. } if message == "error_max_turns"),
        "{err:?}"
    );
}

#[tokio::test]
async fn timeout_is_a_typed_error() {
    let fake = Fake::new();
    fake.reply(None, &envelope(json!({ "greeting": "late" })))
        .sleep(5);
    let schema = Schema::parse(SCHEMA).unwrap();
    let provider = fake.provider().with_timeout(Duration::from_millis(300));
    let err = provider.structured(CALL, &schema).await.unwrap_err();
    assert!(
        matches!(err, LlmError::Timeout { after } if after == Duration::from_millis(300)),
        "{err:?}"
    );
}

#[tokio::test]
async fn missing_binary_is_a_typed_error() {
    let schema = Schema::parse(SCHEMA).unwrap();
    let provider = ClaudeCli::new(Path::new("/nonexistent/claude"));
    let err = provider.structured(CALL, &schema).await.unwrap_err();
    assert!(matches!(err, LlmError::NotFound(_)), "{err:?}");
}
