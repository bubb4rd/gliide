//! Rewrite with a note: one resume line rewritten the way you ask ("shorter", "mention Postgres"), from its own
//! entry's facts only, in the posting's words, within the line's budget (ARCHITECTURE.md §5).
//!
//! The model sees the line as it reads now, the facts it may use (the line's bank entry, tagged like Suggest's input,
//! or the whole resume for a summary), the posting's requirement terms, the note and a character budget. The caller
//! runs the grounding validator on the result: the prompt asks for honesty, the validator checks it.

use std::fmt::Write as _;

use resume_core::bank::BankEntry;
use serde::Deserialize;

use crate::suggest::{LINE_CHARS, MAX_BULLET_CHARS, write_facts};
use crate::{Call, Llm, LlmError, Schema, Structured};

/// The Rewrite instructions (`prompts/rewrite.md`).
pub const PROMPT: &str = include_str!("../../../prompts/rewrite.md");
/// The reply's schema (`prompts/rewrite.schema.json`): `{ "text": string }`.
pub const SCHEMA: &str = include_str!("../../../prompts/rewrite.schema.json");
/// The longest a summary may be (Suggest's limit).
pub const SUMMARY_CHARS: usize = 200;

pub fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("prompts/rewrite.schema.json is a valid schema")
}

/// What Rewrite reads.
#[derive(Clone, Copy, Debug)]
pub struct RewriteInput<'a> {
    /// The line as it reads now (your wording or the suggestion's).
    pub line: &'a str,
    /// The only facts it may use: [`entry_facts`] of the line's entry, or the tagged resume for a summary.
    pub facts: &'a str,
    /// The posting's requirement terms, most important first.
    pub terms: &'a [String],
    /// What you want changed; may be empty (a better version of the same line).
    pub note: &'a str,
    /// The most characters the line may take ([`budget`]).
    pub budget: usize,
}

/// The characters a rewrite of `before` (the master's text of the line, its line budget) may take: the lines it
/// wraps to at [`LINE_CHARS`] a line, at most [`MAX_BULLET_CHARS`], as Suggest's budget.
pub fn budget(before: &str) -> usize {
    let lines = before.chars().count().div_ceil(LINE_CHARS).max(1);
    (lines * LINE_CHARS).min(MAX_BULLET_CHARS)
}

/// A bank entry as Rewrite reads it: `[id] Name, place · dates`, the subtitle, `[id#n] Label: text` note lines, every
/// bullet `- [id] text` with its variants and skills, then its facts.
pub fn entry_facts(e: &BankEntry) -> String {
    let mut s = String::new();
    let _ = write!(s, "[{}] {}", e.id, e.name);
    if !e.place.is_empty() {
        let _ = write!(s, ", {}", e.place);
    }
    if !e.dates.is_empty() {
        let _ = write!(s, " · {}", e.dates);
    }
    s.push('\n');
    if !e.subtitle.is_empty() {
        s.push_str(&e.subtitle);
        s.push('\n');
    }
    for (i, n) in e.notes.iter().enumerate() {
        let _ = write!(s, "[{}#{}] ", e.id, i + 1);
        if !n.label.is_empty() {
            let _ = write!(s, "{}: ", n.label);
        }
        s.push_str(&n.text);
        s.push('\n');
    }
    for b in &e.bullets {
        let _ = writeln!(s, "- [{}] {}", b.id, b.text);
        for (i, v) in b.variants.iter().enumerate() {
            let _ = writeln!(s, "  (variant {i}) {v}");
        }
        if !b.skills.is_empty() {
            let _ = writeln!(s, "  (skills) {}", b.skills.join(", "));
        }
    }
    write_facts(&mut s, e);
    s
}

/// The Rewrite call's input: `<line>`, `<facts>`, `<terms>` (one per line), `<note>`, `<budget>`.
pub fn input(i: &RewriteInput<'_>) -> String {
    let mut s = String::with_capacity(i.facts.len() + 1024);
    let _ = write!(s, "<line>\n{}\n</line>\n", i.line.trim());
    let _ = write!(s, "<facts>\n{}\n</facts>\n", i.facts.trim());
    s.push_str("<terms>\n");
    for t in i.terms {
        s.push_str(t);
        s.push('\n');
    }
    s.push_str("</terms>\n");
    let _ = writeln!(s, "<note>{}</note>", i.note.trim());
    let _ = writeln!(s, "<budget>{} characters</budget>", i.budget);
    s
}

#[derive(Deserialize)]
struct Reply {
    text: String,
}

/// Runs Rewrite with `model`: the new line, trimmed.
pub async fn rewrite(
    llm: &impl Llm,
    schema: &Schema,
    input: &RewriteInput<'_>,
    model: &str,
) -> Result<(String, Structured), LlmError> {
    let text = self::input(input);
    let output = llm
        .structured(
            Call {
                system: PROMPT,
                input: &text,
                model: Some(model),
            },
            schema,
        )
        .await?;
    match Reply::deserialize(&output.value) {
        Ok(r) => Ok((r.text.trim().to_owned(), output)),
        Err(e) => Err(LlmError::Decode {
            target: "Rewrite",
            message: e.to_string(),
            output,
        }),
    }
}

#[cfg(test)]
mod tests {
    use resume_core::bank::{BankBullet, BankEntry, Fact, FactKind, FactSource, Note};

    use super::{RewriteInput, budget, entry_facts, input};

    #[test]
    fn schema_compiles() {
        super::schema();
    }

    #[test]
    fn budget_is_whole_lines_capped() {
        assert_eq!(budget("short"), 100);
        assert_eq!(budget(&"x".repeat(150)), 200);
        assert_eq!(budget(&"x".repeat(250)), 220);
    }

    #[test]
    fn input_carries_every_part() {
        let e = BankEntry {
            id: "acme".into(),
            name: "Acme".into(),
            url: String::new(),
            place: "Springfield".into(),
            dates: "2025".into(),
            start: None,
            end: None,
            subtitle: "Intern".into(),
            tags: vec![],
            notes: vec![Note {
                label: "Stack".into(),
                text: "Rust".into(),
            }],
            bullets: vec![BankBullet {
                id: "acme.1".into(),
                text: "Built a service".into(),
                variants: vec!["Wrote a service".into()],
                skills: vec!["Rust".into()],
            }],
            facts: vec![Fact {
                id: "acme~1".into(),
                text: "The service handled 2,000 orders a day".into(),
                skills: vec!["Kafka".into()],
                source: FactSource {
                    kind: FactKind::Review,
                    at: "2026-10-27T10:00:00Z".into(),
                    posting: None,
                    company: None,
                },
            }],
        };
        let facts = entry_facts(&e);
        assert_eq!(
            facts,
            "[acme] Acme, Springfield · 2025\nIntern\n[acme#1] Stack: Rust\n- [acme.1] Built a service\n  \
             (variant 0) Wrote a service\n  (skills) Rust\n[acme~1] (fact) The service handled 2,000 orders a \
             day\n  (skills) Kafka\n"
        );
        let terms = ["Rust".to_owned()];
        let text = input(&RewriteInput {
            line: "Built a service ",
            facts: &facts,
            terms: &terms,
            note: " shorter ",
            budget: 100,
        });
        assert!(text.starts_with("<line>\nBuilt a service\n</line>\n<facts>\n[acme] Acme"));
        assert!(text.ends_with(
            "<terms>\nRust\n</terms>\n<note>shorter</note>\n<budget>100 characters</budget>\n"
        ));
    }
}
