//! Extract: what a posting asks for, independent of any resume (ARCHITECTURE.md §3).

use crate::{Call, Llm, LlmError, Schema, Structured};

/// The Extract instructions (`prompts/extract.md`).
pub const PROMPT: &str = include_str!("../../../prompts/extract.md");
/// The `Requirements` schema (`prompts/extract.schema.json`).
pub const SCHEMA: &str = include_str!("../../../prompts/extract.schema.json");

pub fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("prompts/extract.schema.json is a valid schema")
}

/// The Extract call's input: the posting text, as received.
pub fn input(posting: &str) -> String {
    format!("<posting>\n{}\n</posting>\n", posting.trim())
}

/// Runs Extract on `posting` with `model` (the provider's default when `None`).
pub async fn extract(
    llm: &impl Llm,
    schema: &Schema,
    posting: &str,
    model: Option<&str>,
) -> Result<Structured, LlmError> {
    let input = input(posting);
    llm.structured(
        Call {
            system: PROMPT,
            input: &input,
            model,
        },
        schema,
    )
    .await
}

#[cfg(test)]
mod tests {
    #[test]
    fn schema_compiles() {
        super::schema();
    }
}
