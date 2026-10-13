//! Extract: what a posting asks for, independent of any resume (ARCHITECTURE.md §3).

use resume_core::{Posting, Requirements};
use serde::Deserialize;

use crate::{Call, Llm, LlmError, Schema, Structured};

/// The Extract instructions (`prompts/extract.md`).
pub const PROMPT: &str = include_str!("../../../prompts/extract.md");
/// The `Requirements` schema (`prompts/extract.schema.json`).
pub const SCHEMA: &str = include_str!("../../../prompts/extract.schema.json");
/// The model Extract runs on unless the user picks another: the fastest in the M0 spike and the best at priorities.
pub const DEFAULT_EXTRACT_MODEL: &str = "sonnet";

pub fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("prompts/extract.schema.json is a valid schema")
}

/// The Extract call's input: `<hints>` with the title, company and location the source gave (omitted when it gave
/// none), then the posting text.
pub fn input(posting: &Posting) -> String {
    let hints = [
        ("title", &posting.title),
        ("company", &posting.company),
        ("location", &posting.location),
    ];
    let mut s = String::with_capacity(posting.text.len() + 256);
    let mut open = false;
    for (key, value) in hints {
        let Some(value) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) else {
            continue;
        };
        if !open {
            s.push_str("<hints>\n");
            open = true;
        }
        s.push_str(key);
        s.push_str(": ");
        s.push_str(value);
        s.push('\n');
    }
    if open {
        s.push_str("</hints>\n");
    }
    s.push_str("<posting>\n");
    s.push_str(posting.text.trim());
    s.push_str("\n</posting>\n");
    s
}

/// Runs Extract on `posting` with `model` ([`DEFAULT_EXTRACT_MODEL`] unless the user picked another).
pub async fn extract(
    llm: &impl Llm,
    schema: &Schema,
    posting: &Posting,
    model: &str,
) -> Result<(Requirements, Structured), LlmError> {
    let input = input(posting);
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
    match Requirements::deserialize(&output.value) {
        Ok(requirements) => Ok((requirements, output)),
        Err(e) => Err(LlmError::Decode {
            target: "Requirements",
            message: e.to_string(),
            output,
        }),
    }
}

#[cfg(test)]
mod tests {
    use resume_core::{Method, Posting};

    use super::input;

    #[test]
    fn schema_compiles() {
        super::schema();
    }

    #[test]
    fn pasted_text_has_no_hints() {
        let p = Posting::pasted("  Build things.\n".to_owned());
        assert_eq!(input(&p), "<posting>\nBuild things.\n</posting>\n");
    }

    #[test]
    fn hints_carry_what_the_source_gave() {
        let p = Posting::new(
            Method::Greenhouse,
            None,
            None,
            Some("Junior Software Engineer".to_owned()),
            Some(" ".to_owned()),
            Some("US - Remote".to_owned()),
            "Build things.".to_owned(),
        );
        assert_eq!(
            input(&p),
            "<hints>\ntitle: Junior Software Engineer\nlocation: US - Remote\n</hints>\n\
             <posting>\nBuild things.\n</posting>\n"
        );
    }
}
