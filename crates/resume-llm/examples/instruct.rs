//! Instruct runs: chat asks through `claude -p` on one bank and master, sequentially, to see what the prompt does
//! with them (what it edits, what it leaves out and why).
//!
//! `cargo run -p resume-llm --example instruct -- --bank <bank.json> --master <master.json> --cases <cases.json>
//! [--only <name>[,<name>…]] [--model sonnet]`
//!
//! `cases.json` is a list of `{ "name", "instruction", "scope": [ids], "confirmed": [skills], "terms": [[term, on the
//! page]] }` (`scope`, `confirmed` and `terms` may be left out). Prints each case's reply, edits and skipped parts.

use std::path::PathBuf;
use std::time::Duration;

use resume_core::{Bank, Master, resolve};
use resume_llm::ClaudeCli;
use resume_llm::instruct::{self, InstructInput};
use resume_llm::suggest::DEFAULT_SUGGEST_MODEL;
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    name: String,
    instruction: String,
    #[serde(default)]
    scope: Vec<String>,
    #[serde(default)]
    confirmed: Vec<String>,
    #[serde(default)]
    terms: Vec<(String, bool)>,
}

fn read_json<T: serde::de::DeserializeOwned>(path: &PathBuf) -> T {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let usage = "usage: instruct --bank <bank.json> --master <master.json> --cases <cases.json> [--only name,name] \
                 [--model sonnet]";
    let (mut bank, mut master, mut cases) = (None, None, None);
    let mut only: Vec<String> = Vec::new();
    let mut model = DEFAULT_SUGGEST_MODEL.to_owned();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().unwrap_or_else(|| panic!("{arg} needs a value; {usage}"));
        match arg.as_str() {
            "--bank" => bank = Some(PathBuf::from(value())),
            "--master" => master = Some(PathBuf::from(value())),
            "--cases" => cases = Some(PathBuf::from(value())),
            "--only" => only = value().split(',').map(|s| s.trim().to_owned()).collect(),
            "--model" => model = value(),
            _ => panic!("unknown argument {arg}; {usage}"),
        }
    }
    let bank: Bank = read_json(&bank.unwrap_or_else(|| panic!("--bank is required; {usage}")));
    let master: Master = read_json(&master.unwrap_or_else(|| panic!("--master is required; {usage}")));
    let cases: Vec<Case> = read_json(&cases.unwrap_or_else(|| panic!("--cases is required; {usage}")));
    let resume = resolve(&bank, &master).expect("the master resolves against the bank");
    let llm = ClaudeCli::discover()
        .await
        .expect("claude is installed")
        .with_timeout(Duration::from_secs(300));
    let schema = instruct::schema();

    for case in cases.iter().filter(|c| only.is_empty() || only.contains(&c.name)) {
        let input = InstructInput {
            resume: &resume,
            bank: &bank,
            terms: &case.terms,
            scope: &case.scope,
            confirmed: &case.confirmed,
            instruction: &case.instruction,
        };
        println!("── {} ──", case.name);
        println!("ask: {} (scope: {:?}, confirmed: {:?})", case.instruction, case.scope, case.confirmed);
        match instruct::instruct(&llm, &schema, &input, &model).await {
            Ok((reply, out)) => {
                println!("reply: {}", reply.reply);
                for e in &reply.edits {
                    println!("  edit {:?} {}: {}  ({})", e.kind, e.id, e.text, e.why);
                }
                for s in &reply.skipped {
                    println!("  skip {} [term: {:?}]: {}", s.what, s.term, s.why);
                }
                println!("  ({} ms)", out.latency().as_millis());
            }
            Err(e) => println!("  error: {e}"),
        }
    }
}
