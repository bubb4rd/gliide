//! Import (`prompts/import.md`): a resume file's text and links turned into a resume, word for word, in the v1 master
//! JSON shape (`resume_core::Resume`); `resume_core::import` merges it into the bank. Nothing checks the
//! copy against the file: the import review shows it before anything is saved.

use std::fmt::Write as _;

use resume_core::Resume;
use serde::Deserialize;

use crate::{Call, Llm, LlmError, Schema, Structured};

/// The import instructions (`prompts/import.md`).
pub const PROMPT: &str = include_str!("../../../prompts/import.md");
/// The reply's schema (`prompts/import.schema.json`): the v1 master JSON shape.
pub const SCHEMA: &str = include_str!("../../../prompts/import.schema.json");
/// The model Import runs on unless the user picks another.
pub const DEFAULT_IMPORT_MODEL: &str = "sonnet";

pub fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("prompts/import.schema.json is a valid schema")
}

/// A link of the file: the text it sits on, and where it goes.
#[derive(Clone, Copy, Debug)]
pub struct Link<'a> {
    pub text: &'a str,
    pub url: &'a str,
}

/// The call's input: `<resume>` with the file's text, `<links>` with one `anchor text -> URL` per line.
pub fn input(text: &str, links: &[Link<'_>]) -> String {
    let mut out = String::with_capacity(text.len() + 64 * links.len() + 48);
    let _ = writeln!(out, "<resume>\n{}\n</resume>\n<links>", text.trim());
    for l in links {
        let _ = writeln!(out, "{} -> {}", l.text.trim(), l.url.trim());
    }
    out.push_str("</links>\n");
    out
}

/// Runs Import with `model`: the resume as the file shows it.
pub async fn import(
    llm: &impl Llm,
    schema: &Schema,
    text: &str,
    links: &[Link<'_>],
    model: &str,
) -> Result<(Resume, Structured), LlmError> {
    let input = input(text, links);
    let output = llm
        .structured(
            Call {
                system: PROMPT,
                input: &input,
                model: Some(model),
            },
            schema,
        )
        .await?;
    match Resume::deserialize(&output.value) {
        Ok(r) => Ok((r, output)),
        Err(e) => Err(LlmError::Decode {
            target: "Resume",
            message: e.to_string(),
            output,
        }),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn schema_compiles() {
        super::schema();
    }
}
