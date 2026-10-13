//! Cover letters (ATS Fit's `cover.md` and `cover-paragraph.md`): a one-page letter written from the tailored resume
//! as the draft has it (your toggles and edits included) and the posting, and one paragraph of it rewritten with a
//! note. The prompts ask for honesty (nothing the resume doesn't show, never what you lack); there's no validator for
//! prose, so the letter is yours to edit.

use std::fmt::Write as _;

use resume_core::Resume;
use serde::{Deserialize, Serialize};

use crate::{Call, Llm, LlmError, Schema, Structured};

/// The letter instructions (`prompts/cover.md`).
pub const PROMPT: &str = include_str!("../../../prompts/cover.md");
/// The letter's schema (`prompts/cover.schema.json`): greeting, 3–4 paragraphs, closing.
pub const SCHEMA: &str = include_str!("../../../prompts/cover.schema.json");
/// The paragraph-rewrite instructions (`prompts/cover-paragraph.md`).
pub const PARAGRAPH_PROMPT: &str = include_str!("../../../prompts/cover-paragraph.md");
/// The paragraph rewrite's schema (`prompts/cover-paragraph.schema.json`): `{ "paragraph": string }`.
pub const PARAGRAPH_SCHEMA: &str = include_str!("../../../prompts/cover-paragraph.schema.json");

pub fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("prompts/cover.schema.json is a valid schema")
}

pub fn paragraph_schema() -> Schema {
    Schema::parse(PARAGRAPH_SCHEMA).expect("prompts/cover-paragraph.schema.json is a valid schema")
}

/// A letter's parts, as the model writes them and you edit them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Letter {
    pub greeting: String,
    pub paragraphs: Vec<String>,
    pub closing: String,
}

impl Letter {
    /// Every part trimmed.
    fn trimmed(self) -> Self {
        Self {
            greeting: self.greeting.trim().to_owned(),
            paragraphs: self
                .paragraphs
                .into_iter()
                .map(|p| p.trim().to_owned())
                .collect(),
            closing: self.closing.trim().to_owned(),
        }
    }
}

/// What both calls read: the job, the tailored resume and the posting.
#[derive(Clone, Copy, Debug)]
pub struct CoverInput<'a> {
    pub company: &'a str,
    pub role: &'a str,
    /// The tailored resume as the draft has it now.
    pub resume: &'a Resume,
    /// The posting's text.
    pub posting: &'a str,
}

/// `resume` as plain text, the way ATS Fit hands it to the letter: name, contact line, then `## Title` per section,
/// `Name, place · dates`, subtitle, `Label: text` note lines, `- bullet` lines and `Label: item, item` skills lines.
pub fn resume_text(resume: &Resume) -> String {
    let mut s = String::new();
    s.push_str(&resume.name);
    s.push('\n');
    let contact: Vec<&str> = resume.contact.iter().map(|c| c.text.as_str()).collect();
    s.push_str(&contact.join(" | "));
    for section in &resume.sections {
        let _ = write!(s, "\n\n## {}", section.title);
        for e in &section.entries {
            let _ = write!(s, "\n{}", e.name);
            if !e.place.is_empty() {
                let _ = write!(s, ", {}", e.place);
            }
            if !e.dates.is_empty() {
                let _ = write!(s, " · {}", e.dates);
            }
            if !e.subtitle.is_empty() {
                let _ = write!(s, "\n{}", e.subtitle);
            }
            for n in &e.notes {
                s.push('\n');
                if !n.label.is_empty() {
                    let _ = write!(s, "{}: ", n.label);
                }
                s.push_str(&n.text);
            }
            for b in &e.bullets {
                let _ = write!(s, "\n- {}", b.text);
            }
        }
        for g in &section.groups {
            let _ = write!(s, "\n{}: {}", g.label, g.items.join(", "));
        }
    }
    s
}

/// The letter call's input: `<job>` (company and role), `<resume>`, `<posting>`.
pub fn input(i: &CoverInput<'_>) -> String {
    let mut s = String::with_capacity(i.posting.len() + 4096);
    let _ = write!(
        s,
        "<job>\nCompany: {}\nRole: {}\n</job>\n<resume>\n{}\n</resume>\n<posting>\n{}\n</posting>\n",
        i.company.trim(),
        i.role.trim(),
        resume_text(i.resume),
        i.posting.trim()
    );
    s
}

/// The paragraph call's input: [`input`], then `<letter>` with the paragraphs numbered from 1, `<which>` (`which` is
/// 0-based; the model sees it numbered from 1) and `<note>`.
pub fn paragraph_input(i: &CoverInput<'_>, letter: &Letter, which: usize, note: &str) -> String {
    let mut s = input(i);
    s.push_str("<letter>\n");
    for (n, p) in letter.paragraphs.iter().enumerate() {
        if n > 0 {
            s.push_str("\n\n");
        }
        let _ = write!(s, "{}. {}", n + 1, p.trim());
    }
    let _ = write!(
        s,
        "\n</letter>\n<which>{}</which>\n<note>{}</note>\n",
        which + 1,
        note.trim()
    );
    s
}

/// Writes the letter with `model`; every part trimmed.
pub async fn write(
    llm: &impl Llm,
    schema: &Schema,
    input: &CoverInput<'_>,
    model: &str,
) -> Result<(Letter, Structured), LlmError> {
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
    match Letter::deserialize(&output.value) {
        Ok(letter) => Ok((letter.trimmed(), output)),
        Err(e) => Err(LlmError::Decode {
            target: "Letter",
            message: e.to_string(),
            output,
        }),
    }
}

#[derive(Deserialize)]
struct Paragraph {
    paragraph: String,
}

/// Rewrites paragraph `which` (0-based) of `letter` per `note` (may be empty: a better version of the same paragraph)
/// with `model`: the new paragraph, trimmed.
pub async fn rewrite_paragraph(
    llm: &impl Llm,
    schema: &Schema,
    input: &CoverInput<'_>,
    letter: &Letter,
    which: usize,
    note: &str,
    model: &str,
) -> Result<(String, Structured), LlmError> {
    let text = paragraph_input(input, letter, which, note);
    let output = llm
        .structured(
            Call {
                system: PARAGRAPH_PROMPT,
                input: &text,
                model: Some(model),
            },
            schema,
        )
        .await?;
    match Paragraph::deserialize(&output.value) {
        Ok(p) if !p.paragraph.trim().is_empty() => Ok((p.paragraph.trim().to_owned(), output)),
        Ok(_) => Err(LlmError::Agent {
            agent: "claude".to_owned(),
            message: "returned an empty paragraph".to_owned(),
        }),
        Err(e) => Err(LlmError::Decode {
            target: "Paragraph",
            message: e.to_string(),
            output,
        }),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn schemas_compile() {
        super::schema();
        super::paragraph_schema();
    }
}
