//! Suggest: what to change in one master for one posting (ARCHITECTURE.md §3). Master-dependent.
//!
//! The model sees the requirements, the resolved master as tagged text (every section, entry, note line, bullet and
//! skills line carries its id in brackets, so an edit can name the line it changes), in bank-aware mode the candidate
//! bank entries tagged the same way, and the posting.

use std::fmt::Write as _;

use resume_core::bank::{BankBullet, BankEntry};
use resume_core::resume::Section;
use resume_core::suggestions::Suggestions;
use resume_core::{Bank, Posting, Requirements, Resume};
use serde::Deserialize;

use crate::{Call, Llm, LlmError, Schema, Structured};

/// The Suggest instructions (`prompts/suggest.md`).
pub const PROMPT: &str = include_str!("../../../prompts/suggest.md");
/// The `Suggestions` schema (`prompts/suggest.schema.json`).
pub const SCHEMA: &str = include_str!("../../../prompts/suggest.schema.json");
/// The model Suggest runs on unless the user picks another.
pub const DEFAULT_SUGGEST_MODEL: &str = "sonnet";
/// Characters per printed line the line budget assumes (the one-page layout).
pub const LINE_CHARS: usize = 100;
/// The longest a bullet rewrite may be, whatever the line it replaces.
pub const MAX_BULLET_CHARS: usize = 220;

pub fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("prompts/suggest.schema.json is a valid schema")
}

/// What Suggest reads.
#[derive(Clone, Copy, Debug)]
pub struct SuggestInput<'a> {
    pub posting: &'a Posting,
    pub requirements: &'a Requirements,
    /// The resolved master.
    pub resume: &'a Resume,
    /// The bank the master resolves from: its bullets' variants and skills, and the candidates' text.
    pub bank: &'a Bank,
    /// Bank entry ids outside the master the model may propose (bank-aware mode); empty for master-only.
    pub candidates: &'a [String],
}

/// The Suggest call's input: `<requirements>` as JSON, `<resume>` as tagged text with each bullet's line budget and
/// its variants and skills from the bank, `<bank>` with the candidate entries (only when there are any), then
/// `<posting>`.
pub fn input(i: &SuggestInput<'_>) -> String {
    let mut s = String::with_capacity(i.posting.text.len() + 8192);
    s.push_str("<requirements>\n");
    s.push_str(&serde_json::to_string(i.requirements).expect("Requirements serialize"));
    s.push_str("\n</requirements>\n<resume>\n");
    write_resume(&mut s, i.resume, Some(i.bank));
    s.push_str("</resume>\n");
    if !i.candidates.is_empty() {
        s.push_str("<bank>\n");
        write_candidates(&mut s, i.bank, i.candidates);
        s.push_str("</bank>\n");
    }
    s.push_str("<posting>\n");
    s.push_str(i.posting.text.trim());
    s.push_str("\n</posting>\n");
    s
}

/// `resume` as the text Suggest reads, each line tagged with its id: `## [work] TITLE`, `[acme] Name, place · dates`,
/// `[state-u#2] Label: text` (notes numbered from 1), `- [acme.2] text`, `[tools] Label: item, item`. A resume carries
/// no variants; [`input`] adds them from the bank.
pub fn tagged_text(resume: &Resume) -> String {
    let mut s = String::new();
    write_resume(&mut s, resume, None);
    s
}

/// `resume` tagged as Suggest's `<resume>` reads it: [`tagged_text`] with each bullet's line budget, then its other
/// variants and its skills from `bank`.
pub fn tagged_with_bank(resume: &Resume, bank: &Bank) -> String {
    let mut s = String::new();
    write_resume(&mut s, resume, Some(bank));
    s
}

/// Runs Suggest with `model` ([`DEFAULT_SUGGEST_MODEL`] unless the user picked another).
pub async fn suggest(
    llm: &impl Llm,
    schema: &Schema,
    input: &SuggestInput<'_>,
    model: &str,
) -> Result<(Suggestions, Structured), LlmError> {
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
    match Suggestions::deserialize(&output.value) {
        Ok(suggestions) => Ok((suggestions, output)),
        Err(e) => Err(LlmError::Decode {
            target: "Suggestions",
            message: e.to_string(),
            output,
        }),
    }
}

/// The resume, tagged; with `bank` (the model's input), each bullet is followed by its line budget, then its other
/// variants and its skills when the bank holds it.
fn write_resume(s: &mut String, resume: &Resume, bank: Option<&Bank>) {
    s.push_str(&resume.name);
    s.push('\n');
    let contact: Vec<&str> = resume.contact.iter().map(|c| c.text.as_str()).collect();
    s.push_str(&contact.join(" | "));
    s.push('\n');
    for section in &resume.sections {
        heading(s, &section.id, &section.title);
        for e in &section.entries {
            let bank_entry = bank.and_then(|b| find_entry(b, section, &e.id));
            header(s, &e.id, &e.name, &e.place, &e.dates, &e.subtitle);
            for (j, n) in e.notes.iter().enumerate() {
                note(s, &e.id, j, &n.label, &n.text);
            }
            for b in &e.bullets {
                let _ = writeln!(s, "- [{}] {}", b.id, b.text);
                if bank.is_none() {
                    continue;
                }
                budget(s, &b.text);
                if let Some(bb) = bank_entry.and_then(|be| be.bullets.iter().find(|x| x.id == b.id))
                {
                    extras(s, bb, &b.text);
                }
            }
        }
        for g in &section.groups {
            let _ = writeln!(s, "[{}] {}: {}", g.id, g.label, g.items.join(", "));
        }
    }
}

/// The bank entries `ids` names, under their bank sections' headings, in bank order. Ids the bank doesn't hold are
/// skipped.
fn write_candidates(s: &mut String, bank: &Bank, ids: &[String]) {
    let mut first = true;
    for section in &bank.sections {
        let entries: Vec<&BankEntry> = ids
            .iter()
            .filter_map(|id| section.entries.iter().find(|e| e.id == *id))
            .collect();
        if entries.is_empty() {
            continue;
        }
        if !first {
            s.push('\n');
        }
        first = false;
        let _ = writeln!(s, "## [{}] {}", section.id, section.title);
        for e in entries {
            header(s, &e.id, &e.name, &e.place, &e.dates, &e.subtitle);
            for (j, n) in e.notes.iter().enumerate() {
                note(s, &e.id, j, &n.label, &n.text);
            }
            for b in &e.bullets {
                let _ = writeln!(s, "- [{}] {}", b.id, b.text);
                extras(s, b, &b.text);
            }
        }
    }
}

fn heading(s: &mut String, id: &str, title: &str) {
    let _ = write!(s, "\n## [{id}] {title}\n");
}

/// `[id] Name, place · dates`, then the subtitle on its own line.
fn header(s: &mut String, id: &str, name: &str, place: &str, dates: &str, subtitle: &str) {
    let _ = write!(s, "[{id}] {name}");
    if !place.is_empty() {
        let _ = write!(s, ", {place}");
    }
    if !dates.is_empty() {
        let _ = write!(s, " · {dates}");
    }
    s.push('\n');
    if !subtitle.is_empty() {
        s.push_str(subtitle);
        s.push('\n');
    }
}

/// `[entry#n] Label: text`, `n` counting from 1.
fn note(s: &mut String, entry: &str, index: usize, label: &str, text: &str) {
    let _ = write!(s, "[{entry}#{}] ", index + 1);
    if !label.is_empty() {
        let _ = write!(s, "{label}: ");
    }
    s.push_str(text);
    s.push('\n');
}

/// `(budget) N lines, M characters`: the lines `text` wraps to at [`LINE_CHARS`] a line, and the most a rewrite of it
/// may take (those lines, at most [`MAX_BULLET_CHARS`]).
fn budget(s: &mut String, text: &str) {
    let lines = text.chars().count().div_ceil(LINE_CHARS).max(1);
    let chars = (lines * LINE_CHARS).min(MAX_BULLET_CHARS);
    let plural = if lines == 1 { "" } else { "s" };
    let _ = writeln!(s, "  (budget) {lines} line{plural}, {chars} characters");
}

/// A bullet's variants other than the one shown (`(variant N)`, N its index in the bank) and its skills.
fn extras(s: &mut String, b: &BankBullet, shown: &str) {
    for (i, v) in b.variants.iter().enumerate() {
        if v != shown {
            let _ = writeln!(s, "  (variant {i}) {v}");
        }
    }
    if !b.skills.is_empty() {
        let _ = writeln!(s, "  (skills) {}", b.skills.join(", "));
    }
}

/// The bank entry a resume entry resolves from: same section id, same entry id.
fn find_entry<'b>(bank: &'b Bank, section: &Section, id: &str) -> Option<&'b BankEntry> {
    bank.sections
        .iter()
        .find(|s| s.id == section.id)?
        .entries
        .iter()
        .find(|e| e.id == id)
}

#[cfg(test)]
mod tests {
    use resume_core::bank::Bank;
    use resume_core::master::Master;
    use resume_core::suggestions::{
        BankEdit, BankOp, BulletEdit, BulletOp, Evidence, Gap, Have, Keyword, KeywordOp, Order,
        SkillEdit, SkillOp, Suggestions, Verdict,
    };
    use resume_core::{Posting, Requirements, Resume, migrate_v1, resolve};

    use super::{SuggestInput, input, tagged_text};

    /// The synthetic jane-doe bank, migrated, and its `swe` master.
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
    fn tagged_text_tags_every_line() {
        let (bank, master) = jane();
        let text = tagged_text(&resolve(&bank, &master).unwrap());
        for line in [
            "Jane Q. Doe\njane.doe@example.com | (555) 010-0199 | ",
            "\n\n## [work] WORK EXPERIENCE\n[acme] Acme Widgets Co., Springfield, IL · June 2025 – Sep 2025\n\
             Software Engineering Intern\n- [acme.1] Built a Rust service",
            "\n[state-u#1] Relevant Coursework: Data Structures, Operating Systems, Databases, Computer Networks\n\
             [state-u#2] Honors: Dean's List (6 semesters)\n- [state-u.1] Teaching assistant",
            "\n[languages] Languages: Rust, TypeScript, Python, SQL, Java\n",
        ] {
            assert!(text.contains(line), "missing {line:?} in\n{text}");
        }
        assert!(!text.contains("(variant"));
    }

    #[test]
    fn input_lists_variants_and_skills_and_omits_bank_without_candidates() {
        let (mut bank, master) = jane();
        let b = &mut bank.sections[0].entries[0].bullets[1];
        b.variants = vec!["Added GitHub Actions integration tests to the order API".to_owned()];
        b.skills = vec!["GitHub Actions".to_owned(), "integration tests".to_owned()];
        let resume = resolve(&bank, &master).unwrap();
        let posting = Posting::pasted("  Build things.\n".to_owned());
        let req = requirements();
        let text = input(&SuggestInput {
            posting: &posting,
            requirements: &req,
            resume: &resume,
            bank: &bank,
            candidates: &[],
        });
        assert!(text.starts_with("<requirements>\n{\"role\":\"Junior Software Engineer\""));
        assert!(text.contains(
            "- [acme.2] Wrote integration tests in GitHub Actions for the order API, catching 14 regressions before \
             release\n  (budget) 1 line, 100 characters\n  (variant 0) Added GitHub Actions integration tests to the \
             order API\n  (skills) GitHub Actions, integration tests\n- [acme.3] Paired with the design team to ship a \
             React dashboard that plant managers now use daily\n  (budget) 1 line, 100 characters\n"
        ));
        assert!(!text.contains("<bank>"));
        assert!(text.ends_with("</resume>\n<posting>\nBuild things.\n</posting>\n"));
    }

    #[test]
    fn candidates_render_from_the_bank() {
        let (bank, mut master) = jane();
        master.sections[0].entries.retain(|e| e.id != "library");
        let resume = resolve(&bank, &master).unwrap();
        let posting = Posting::pasted("Fix laptops.".to_owned());
        let req = requirements();
        let candidates = ["library".to_owned(), "nowhere".to_owned()];
        let text = input(&SuggestInput {
            posting: &posting,
            requirements: &req,
            resume: &resume,
            bank: &bank,
            candidates: &candidates,
        });
        assert!(!tagged_text(&resume).contains("[library]"));
        assert!(text.contains(
            "</resume>\n<bank>\n## [work] WORK EXPERIENCE\n[library] Springfield Public Library, Springfield, IL · \
             Summer 2023\nIT Assistant\n- [library.1] Imaged and deployed"
        ));
        assert!(
            text.contains("documenting fixes in a shared knowledge base\n</bank>\n<posting>\n")
        );
    }

    /// Every field of every kind of item, serialized, passes the schema: the schema accepts the type's shape.
    #[test]
    fn schema_accepts_the_type() {
        let s = Suggestions {
            verdict: Verdict::Tailor,
            headline: "Strong React match; no Elixir".to_owned(),
            evidence: vec![Evidence {
                skill: "React".to_owned(),
                have: Have::Yes,
                ids: vec!["acme.3".to_owned()],
            }],
            keywords: vec![Keyword {
                op: KeywordOp::Edit,
                line: "acme.3".to_owned(),
                term: "Partnered".to_owned(),
                from: "Paired".to_owned(),
                skill: String::new(),
                priority: 2,
                why: "posting's verb".to_owned(),
            }],
            bullets: vec![BulletEdit {
                op: BulletOp::Rewrite,
                id: "acme.2".to_owned(),
                before: "Wrote integration tests".to_owned(),
                after: "Added CI/CD integration tests".to_owned(),
                hits: vec!["CI/CD".to_owned()],
                sources: vec!["acme.2".to_owned()],
                variant: Some(0),
                priority: 1,
                why: "states CI/CD".to_owned(),
            }],
            skills: vec![SkillEdit {
                op: SkillOp::Rename,
                group: "tools".to_owned(),
                item: "Postgres".to_owned(),
                to: "PostgreSQL".to_owned(),
                skill: "PostgreSQL".to_owned(),
                priority: 1,
                why: "posting's form".to_owned(),
            }],
            bank: vec![BankEdit {
                op: BankOp::Swap,
                entry: "library".to_owned(),
                bullets: vec![],
                replaces: "acme".to_owned(),
                skills: vec!["help desk".to_owned()],
                why: "IT role".to_owned(),
            }],
            order: vec![Order {
                section: "projects".to_owned(),
                entries: vec!["b".to_owned(), "a".to_owned()],
            }],
            summary: None,
            gaps: vec![Gap {
                gap: "Elixir".to_owned(),
                fix: "Build a small Phoenix app".to_owned(),
                skill: "Elixir".to_owned(),
            }],
        };
        let value = serde_json::to_value(&s).unwrap();
        assert_eq!(super::schema().errors(&value), Vec::<String>::new());
        let mut with_summary = value;
        with_summary["summary"] = "Software engineer".into();
        assert_eq!(super::schema().errors(&with_summary), Vec::<String>::new());
    }
}
