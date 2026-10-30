//! `AntigravityCli` against a fake `agy` (`fake::Fake`) printing `agy -p --output-format json` envelopes.
#![cfg(unix)]

mod fake;

use std::time::Duration;

use fake::Fake;
use resume_llm::{AntigravityCli, Call, Llm, LlmError, Schema};
use serde_json::{Value, json};

const SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["greeting"],"properties":{"greeting":{"type":"string"}}}"#;

const CALL: Call<'static> = Call {
    system: "Return a greeting.",
    input: "<posting>\nhi\n</posting>\n",
    model: None,
};

fn provider(fake: &Fake) -> AntigravityCli {
    AntigravityCli::new(fake.bin())
        .with_model("gemini-test-low")
        .with_timeout(Duration::from_secs(10))
}

async fn run(fake: &Fake) -> Result<resume_llm::Structured, LlmError> {
    let schema = Schema::parse(SCHEMA).unwrap();
    provider(fake).structured(CALL, &schema).await
}

fn envelope(structured: &Value) -> String {
    json!({
        "conversation_id": "c1",
        "status": "SUCCESS",
        "response": format!("{structured}\n"),
        "duration_seconds": 1.5,
        "num_turns": 1,
        "structured_output": structured,
        "usage": { "input_tokens": 10, "output_tokens": 20, "thinking_tokens": 5, "cache_read_tokens": 4, "total_tokens": 30 }
    })
    .to_string()
}

#[tokio::test]
async fn structured_output_is_the_reply() {
    let fake = Fake::new("agy");
    fake.reply(None, &envelope(&json!({ "greeting": "hello" })));
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
    assert_eq!(usage.models, ["gemini-test-low"]);
}

#[tokio::test]
async fn the_prompt_opens_with_the_instructions_and_every_tool_is_denied() {
    let fake = Fake::new("agy");
    fake.reply(None, &envelope(&json!({ "greeting": "hello" })));
    run(&fake).await.unwrap();
    let args = fake.read("args.1");
    assert!(
        args.starts_with(
            "-p\n<instructions>\nReturn a greeting.\n</instructions>\n\n<posting>\nhi\n</posting>\n"
        ),
        "{args}"
    );
    let args = fake.args(1);
    let at = |flag: &str| {
        let i = args
            .iter()
            .position(|a| a == flag)
            .unwrap_or_else(|| panic!("{flag} in {args:?}"));
        args[i + 1].clone()
    };
    let schema: Value = serde_json::from_str(&at("--json-schema")).unwrap();
    assert_eq!(schema, serde_json::from_str::<Value>(SCHEMA).unwrap());
    assert_eq!(at("--model"), "gemini-test-low");
    let settings: Value = serde_json::from_str(&at("--settings")).unwrap();
    let denied: Vec<&str> = settings["permissions"]["deny"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for action in [
        "read_file(*)",
        "write_file(*)",
        "read_url(*)",
        "execute_url(*)",
        "command(*)",
        "mcp(*)",
    ] {
        assert!(denied.contains(&action), "{action} in {denied:?}");
    }
    assert!(
        args.iter().any(|a| a == "--disable-slash-commands"),
        "{args:?}"
    );
}

#[tokio::test]
async fn an_error_status_is_an_agent_error() {
    let fake = Fake::new("agy");
    let body = json!({ "conversation_id": "c1", "status": "ERROR", "response": "", "error": "quota exhausted\nretry later", "num_turns": 1 });
    fake.reply(None, &body.to_string());
    let err = run(&fake).await.unwrap_err();
    assert!(
        matches!(&err, LlmError::Agent { message, .. } if message == "quota exhausted"),
        "{err:?}"
    );
}

#[tokio::test]
async fn non_zero_exit_says_why_from_the_envelope() {
    let fake = Fake::new("agy");
    let body = json!({ "conversation_id": "", "status": "ERROR", "response": "", "error": "invalid model selection (--model \"nope\")\nAvailable models:\n  Gemini" });
    fake.reply(None, &body.to_string()).status(1);
    let err = run(&fake).await.unwrap_err();
    let LlmError::Exit { code, message, .. } = err else {
        panic!("{err:?}")
    };
    assert_eq!(code, Some(1));
    assert_eq!(message, "invalid model selection (--model \"nope\")");
}

#[tokio::test]
async fn prose_gets_one_repair() {
    let fake = Fake::new("agy");
    let prose = json!({ "conversation_id": "c1", "status": "SUCCESS", "response": "Sure! Here is a greeting.", "num_turns": 1 });
    fake.reply(Some(1), &prose.to_string());
    fake.reply(Some(2), &envelope(&json!({ "greeting": "hello" })));
    let out = run(&fake).await.unwrap();
    assert!(out.repaired());
    let repair = fake.read("args.2");
    assert!(repair.contains("Sure! Here is a greeting."), "{repair}");
}
