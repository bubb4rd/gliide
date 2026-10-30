//! `CodexCli` against a fake `codex` (`fake::Fake`) printing `codex exec --json` events.
#![cfg(unix)]

mod fake;

use std::time::Duration;

use fake::Fake;
use resume_llm::{Call, CodexCli, Llm, LlmError, Schema};
use serde_json::{Value, json};

const SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["greeting"],"properties":{"greeting":{"type":"string"}}}"#;

const CALL: Call<'static> = Call {
    system: "Return a greeting.",
    input: "<posting>\nhi\n</posting>\n",
    model: None,
};

fn provider(fake: &Fake) -> CodexCli {
    CodexCli::new(fake.bin())
        .with_model("gpt-test")
        .with_timeout(Duration::from_secs(10))
}

async fn run(fake: &Fake) -> Result<resume_llm::Structured, LlmError> {
    let schema = Schema::parse(SCHEMA).unwrap();
    provider(fake).structured(CALL, &schema).await
}

/// A run's events: a warning item (as Codex prints for a setting it doesn't know), the message, the usage.
fn events(message: &Value) -> String {
    [
        json!({ "type": "thread.started", "thread_id": "t1" }),
        json!({ "type": "item.completed", "item": { "id": "item_0", "type": "error", "message": "Codex is ignoring 1 unrecognized configuration setting." } }),
        json!({ "type": "turn.started" }),
        json!({ "type": "item.completed", "item": { "id": "item_1", "type": "agent_message", "text": message.to_string() } }),
        json!({ "type": "turn.completed", "usage": { "input_tokens": 10, "cached_input_tokens": 4, "cache_write_input_tokens": 0, "output_tokens": 20, "reasoning_output_tokens": 3 } }),
    ]
    .map(|e| e.to_string())
    .join("\n")
}

#[tokio::test]
async fn the_last_message_is_the_reply() {
    let fake = Fake::new("codex");
    fake.reply(None, &events(&json!({ "greeting": "hello" })));
    let out = run(&fake).await.unwrap();
    assert_eq!(out.value, json!({ "greeting": "hello" }));
    assert_eq!(fake.calls(), 1);
    let usage = &out.attempts[0].usage;
    assert_eq!(
        (
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_read_input_tokens
        ),
        (Some(10), Some(20), Some(4))
    );
    assert_eq!(usage.models, ["gpt-test"]);
}

#[tokio::test]
async fn the_call_reads_input_schema_and_instructions_and_turns_tools_off() {
    let fake = Fake::new("codex");
    fake.reply(None, &events(&json!({ "greeting": "hello" })));
    run(&fake).await.unwrap();
    assert_eq!(fake.read("stdin.1"), CALL.input);
    assert_eq!(fake.read("schema.1"), SCHEMA);
    assert_eq!(fake.read("instructions.1"), CALL.system);
    let args = fake.args(1);
    let has = |pair: [&str; 2]| args.windows(2).any(|w| w == pair);
    assert_eq!(args.first().map(String::as_str), Some("exec"));
    assert!(has(["--sandbox", "read-only"]), "{args:?}");
    assert!(has(["--model", "gpt-test"]), "{args:?}");
    for off in [
        "web_search=\"disabled\"",
        "features.shell_tool=false",
        "features.unified_exec=false",
    ] {
        assert!(has(["-c", off]), "{off} in {args:?}");
    }
    for flag in ["--ephemeral", "--ignore-user-config", "--ignore-rules"] {
        assert!(args.iter().any(|a| a == flag), "{flag} in {args:?}");
    }
    assert_eq!(args.last().map(String::as_str), Some("-"));
}

#[tokio::test]
async fn a_failed_turn_is_an_agent_error() {
    let fake = Fake::new("codex");
    let body = [
        json!({ "type": "turn.started" }),
        json!({ "type": "turn.failed", "error": { "message": "You've hit your usage limit.\nTry again later." } }),
    ]
    .map(|e| e.to_string())
    .join("\n");
    fake.reply(None, &body);
    let err = run(&fake).await.unwrap_err();
    assert!(
        matches!(&err, LlmError::Agent { message, .. } if message == "You've hit your usage limit."),
        "{err:?}"
    );
}

#[tokio::test]
async fn non_zero_exit_says_why_from_stderr() {
    let fake = Fake::new("codex");
    fake.reply(None, "")
        .stderr("Not logged in. Run `codex login`.\n")
        .status(1);
    let err = run(&fake).await.unwrap_err();
    let LlmError::Exit { code, message, .. } = err else {
        panic!("{err:?}")
    };
    assert_eq!(code, Some(1));
    assert_eq!(message, "Not logged in. Run `codex login`.");
}

#[tokio::test]
async fn prose_gets_one_repair() {
    let fake = Fake::new("codex");
    fake.reply(Some(1), &events(&json!("Sure! Here is a greeting.")));
    fake.reply(Some(2), &events(&json!({ "greeting": "hello" })));
    let out = run(&fake).await.unwrap();
    assert!(out.repaired());
    assert!(fake.read("stdin.2").contains("<validation_errors>"));
}
