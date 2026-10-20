//! Instruct: one free-form instruction ("lean harder into backend and databases") turned into a few proposed edits
//! to the open review, each from the facts its line may use, in the posting's words, within its line's budget
//! (ARCHITECTURE.md §5).
//!
//! The model sees the tailored resume as it reads now (tagged like Suggest's input, with budgets, variants and
//! skills from the bank), the posting's requirement terms marked by whether the page states them, the scope the
//! edits may touch and the instruction. The caller places each edit, runs the grounding validator on it and lands it
//! pending: the prompt asks for honesty, the validator checks it.

use std::fmt::Write as _;

use resume_core::matcher::{Page, find_literal};
use resume_core::{Bank, Requirements, Resume};
use serde::Deserialize;

use crate::suggest::tagged_with_bank;
use crate::{Call, Llm, LlmError, Schema, Structured};

/// The Instruct instructions (`prompts/instruct.md`).
pub const PROMPT: &str = include_str!("../../../prompts/instruct.md");
/// The reply's schema (`prompts/instruct.schema.json`): `{ reply, edits, skipped }`.
pub const SCHEMA: &str = include_str!("../../../prompts/instruct.schema.json");

pub fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("prompts/instruct.schema.json is a valid schema")
}

/// What Instruct reads.
#[derive(Clone, Copy, Debug)]
pub struct InstructInput<'a> {
    /// The tailored resume as it reads now.
    pub resume: &'a Resume,
    /// The bank it resolves from: its bullets' variants and skills.
    pub bank: &'a Bank,
    /// The posting's requirement terms, most important first, each with whether the page states it ([`terms`]).
    pub terms: &'a [(String, bool)],
    /// The ids the edits may touch; empty for anywhere.
    pub scope: &'a [String],
    /// Skills the candidate confirms having though the resume doesn't show them.
    pub confirmed: &'a [String],
    pub instruction: &'a str,
}

/// The posting's requirement terms, most important first (priority, then the posting's order), each with whether
/// `page` states it (the term or an alias, literally).
pub fn terms(req: &Requirements, page: &Resume) -> Vec<(String, bool)> {
    let page = Page::of_resume(page);
    let mut skills: Vec<_> = req.skills.iter().collect();
    skills.sort_by_key(|k| k.priority);
    skills
        .into_iter()
        .map(|k| {
            (
                k.term.clone(),
                find_literal(&page, &k.term, &k.aliases).is_some(),
            )
        })
        .collect()
}

/// The Instruct call's input: `<resume>` tagged with budgets, variants and skills, `<terms>` (one per line, each
/// marked on or not on the page), `<confirmed>` (one skill per line, or `none`), `<scope>` (one id per line, or
/// `anywhere`), `<instruction>`.
pub fn input(i: &InstructInput<'_>) -> String {
    let mut s = String::with_capacity(8192);
    s.push_str("<resume>\n");
    s.push_str(&tagged_with_bank(i.resume, i.bank));
    s.push_str("</resume>\n<terms>\n");
    for (t, on) in i.terms {
        let mark = if *on {
            "on the page"
        } else {
            "not on the page"
        };
        let _ = writeln!(s, "{t} ({mark})");
    }
    s.push_str("</terms>\n<confirmed>\n");
    if i.confirmed.is_empty() {
        s.push_str("none\n");
    }
    for c in i.confirmed {
        s.push_str(c);
        s.push('\n');
    }
    s.push_str("</confirmed>\n<scope>\n");
    if i.scope.is_empty() {
        s.push_str("anywhere\n");
    }
    for id in i.scope {
        s.push_str(id);
        s.push('\n');
    }
    s.push_str("</scope>\n");
    let _ = writeln!(s, "<instruction>{}</instruction>", i.instruction.trim());
    s
}

/// What Instruct proposes.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Reply {
    /// 1–2 sentences on what it did, for the candidate.
    pub reply: String,
    pub edits: Vec<Edit>,
    /// Parts of the instruction it couldn't do honestly.
    pub skipped: Vec<Skip>,
}

/// One proposed edit, as the model named it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Edit {
    pub kind: EditKind,
    /// A bullet id (`line`), an entry id (`bullet`), a skills line id (`skill`), `""` for the summary.
    pub id: String,
    /// The line's new wording, the new summary or bullet, or the skill to add.
    pub text: String,
    pub why: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditKind {
    Line,
    Summary,
    Bullet,
    Skill,
}

/// A part of the instruction left undone, and why.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Skip {
    pub what: String,
    pub why: String,
    /// The skill the resume doesn't show that blocked this part ("Bash"); `""` for any other reason.
    #[serde(default)]
    pub term: String,
}

/// Runs Instruct with `model`: the reply, its text trimmed.
pub async fn instruct(
    llm: &impl Llm,
    schema: &Schema,
    input: &InstructInput<'_>,
    model: &str,
) -> Result<(Reply, Structured), LlmError> {
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
        Ok(mut r) => {
            r.reply = r.reply.trim().to_owned();
            for e in &mut r.edits {
                e.id = e.id.trim().to_owned();
                e.text = e.text.trim().to_owned();
                e.why = e.why.trim().to_owned();
            }
            for s in &mut r.skipped {
                s.term = s.term.trim().to_owned();
            }
            Ok((r, output))
        }
        Err(e) => Err(LlmError::Decode {
            target: "Instruct",
            message: e.to_string(),
            output,
        }),
    }
}

#[cfg(test)]
mod tests {
    use resume_core::bank::Bank;
    use resume_core::master::Master;
    use resume_core::{Requirements, Resume, migrate_v1, resolve};
    use serde::Deserialize;
    use serde_json::json;

    use super::{EditKind, InstructInput, Reply, input, terms};

    fn jane() -> (Bank, Master) {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/banks/jane-doe.v1.json"
        );
        let v1: Resume = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let m = migrate_v1(&v1);
        (m.bank, m.master)
    }

    fn requirements() -> Requirements {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/requirements/sonnet/northwind-jr-swe.json"
        );
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn schema_compiles() {
        super::schema();
    }

    #[test]
    fn terms_go_by_priority_and_mark_the_page() {
        let (bank, master) = jane();
        let resume = resolve(&bank, &master).unwrap();
        let req = requirements();
        let t = terms(&req, &resume);
        assert_eq!(t.len(), req.skills.len());
        let priority = |term: &str| req.skills.iter().find(|k| k.term == term).unwrap().priority;
        assert!(t.windows(2).all(|w| priority(&w[0].0) <= priority(&w[1].0)));
        assert!(t.contains(&("TypeScript".to_owned(), true)), "{t:?}");
        assert!(t.iter().any(|(_, on)| !on), "{t:?}");
    }

    #[test]
    fn input_carries_every_part() {
        let (mut bank, master) = jane();
        bank.sections[0].entries[0].bullets[1].variants =
            vec!["Added GitHub Actions tests".to_owned()];
        let resume = resolve(&bank, &master).unwrap();
        let terms = [("Rust".to_owned(), true), ("Kafka".to_owned(), false)];
        let scope = ["acme".to_owned(), "summary".to_owned()];
        let text = input(&InstructInput {
            resume: &resume,
            bank: &bank,
            terms: &terms,
            scope: &scope,
            confirmed: &["Bash".to_owned(), "Kafka".to_owned()],
            instruction: " lean into backend ",
        });
        assert!(text.starts_with("<resume>\nJane Q. Doe\n"));
        for part in [
            "## [work] WORK EXPERIENCE\n[acme] Acme Widgets Co.",
            "- [acme.1] Built a Rust service",
            "  (budget) 1 line, 100 characters\n",
            "  (variant 0) Added GitHub Actions tests\n",
            "[languages] Languages: Rust",
            "</resume>\n<terms>\nRust (on the page)\nKafka (not on the page)\n</terms>\n",
            "<confirmed>\nBash\nKafka\n</confirmed>\n",
            "<scope>\nacme\nsummary\n</scope>\n",
        ] {
            assert!(text.contains(part), "missing {part:?} in\n{text}");
        }
        assert!(text.ends_with("<instruction>lean into backend</instruction>\n"));

        let anywhere = input(&InstructInput {
            resume: &resume,
            bank: &bank,
            terms: &terms,
            scope: &[],
            confirmed: &[],
            instruction: "x",
        });
        assert!(anywhere.contains("<scope>\nanywhere\n</scope>\n"));
        assert!(anywhere.contains("<confirmed>\nnone\n</confirmed>\n"));
    }

    #[test]
    fn every_kind_of_reply_passes_the_schema_and_decodes() {
        let value = json!({
            "reply": "Reworded 1 bullet.",
            "edits": [
                { "kind": "line", "id": "acme.1", "text": "Built a Rust service", "why": "Backend" },
                { "kind": "summary", "id": "", "text": "Software engineer", "why": "Title" },
                { "kind": "bullet", "id": "acme", "text": "Wrote SQL", "why": "Databases" },
                { "kind": "skill", "id": "languages", "text": "SQL", "why": "Databases" }
            ],
            "skipped": [
                { "what": "distributed systems", "why": "Nothing in your bank backs it", "term": "" },
                { "what": "Bash", "why": "Nothing mentions Bash", "term": "Bash" }
            ]
        });
        assert_eq!(super::schema().errors(&value), Vec::<String>::new());
        let r = Reply::deserialize(&value).unwrap();
        let kinds: Vec<_> = r.edits.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            [
                EditKind::Line,
                EditKind::Summary,
                EditKind::Bullet,
                EditKind::Skill
            ]
        );
        let terms: Vec<_> = r.skipped.iter().map(|s| s.term.as_str()).collect();
        assert_eq!(terms, ["", "Bash"]);
        let mut bad = value.clone();
        bad["edits"][0]["kind"] = "note".into();
        assert!(!super::schema().errors(&bad).is_empty());
        let mut termless = value;
        termless["skipped"][0]
            .as_object_mut()
            .unwrap()
            .remove("term");
        assert!(!super::schema().errors(&termless).is_empty());
        assert_eq!(Reply::deserialize(&termless).unwrap().skipped[0].term, "");
    }
}
