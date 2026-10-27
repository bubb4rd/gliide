//! Invite: a pasted interview invite (an email, a calendar notice) read into one round's fields, each only when the
//! text states it (ARCHITECTURE.md §3).
//!
//! The model sees the text, the candidate's date, UTC offset and time zone (to resolve "tomorrow at 2" and invites
//! without a zone) and the job's company and role. The caller checks `starts_at` parses as RFC 3339.

use std::fmt::Write as _;

use serde::Deserialize;

use crate::{Call, Llm, LlmError, Schema, Structured};

/// The Invite instructions (`prompts/invite.md`).
pub const PROMPT: &str = include_str!("../../../prompts/invite.md");
/// The reply's schema (`prompts/invite.schema.json`): see [`Invite`].
pub const SCHEMA: &str = include_str!("../../../prompts/invite.schema.json");
/// The most of the pasted text the model reads.
pub const MAX_CHARS: usize = 12_000;

pub fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("prompts/invite.schema.json is a valid schema")
}

/// What Invite reads.
#[derive(Clone, Copy, Debug)]
pub struct InviteInput<'a> {
    /// The pasted invite.
    pub text: &'a str,
    /// The candidate's date now, `YYYY-MM-DD`.
    pub today: &'a str,
    /// Its weekday ("Tuesday").
    pub weekday: &'a str,
    /// The candidate's UTC offset now (`-07:00`).
    pub offset: &'a str,
    /// The candidate's IANA time zone (`America/Los_Angeles`), when known.
    pub zone: Option<&'a str>,
    pub company: &'a str,
    pub role: &'a str,
}

/// The round the invite describes; each field as the invite states it, `None` or empty when it doesn't.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Invite {
    /// A round kind as the app names it (`technical`, `recruiter-screen`, …).
    pub kind: Option<String>,
    /// RFC 3339 with an offset, as the model wrote it: unchecked.
    pub starts_at: Option<String>,
    pub minutes: Option<u32>,
    pub with: String,
    pub place: String,
    pub notes: String,
}

/// The Invite call's input: `<today>`, `<job>`, `<invite>` (the text, at most [`MAX_CHARS`]).
pub fn input(i: &InviteInput<'_>) -> String {
    let text = i.text.trim();
    let text = match text.char_indices().nth(MAX_CHARS) {
        Some((at, _)) => &text[..at],
        None => text,
    };
    let mut s = String::with_capacity(text.len() + 256);
    let _ = write!(
        s,
        "<today>{} ({}), UTC offset {}",
        i.today, i.weekday, i.offset
    );
    if let Some(zone) = i.zone {
        let _ = write!(s, ", time zone {zone}");
    }
    s.push_str("</today>\n");
    let _ = writeln!(s, "<job>{} at {}</job>", i.role.trim(), i.company.trim());
    let _ = write!(s, "<invite>\n{text}\n</invite>\n");
    s
}

/// Runs Invite with `model`: the round's fields, text trimmed.
pub async fn read(
    llm: &impl Llm,
    schema: &Schema,
    input: &InviteInput<'_>,
    model: &str,
) -> Result<(Invite, Structured), LlmError> {
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
    match Invite::deserialize(&output.value) {
        Ok(i) => Ok((
            Invite {
                starts_at: i.starts_at.map(|s| s.trim().to_owned()),
                with: i.with.trim().to_owned(),
                place: i.place.trim().to_owned(),
                notes: i.notes.trim().to_owned(),
                ..i
            },
            output,
        )),
        Err(e) => Err(LlmError::Decode {
            target: "Invite",
            message: e.to_string(),
            output,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{Invite, InviteInput, MAX_CHARS, input};

    #[test]
    fn schema_compiles() {
        super::schema();
    }

    #[test]
    fn input_carries_every_part() {
        let i = InviteInput {
            text: "\n Hi Jane, can you do Tuesday at 2pm ET? \n",
            today: "2026-10-27",
            weekday: "Tuesday",
            offset: "-07:00",
            zone: Some("America/Los_Angeles"),
            company: "Acme ",
            role: " Software Engineer",
        };
        assert_eq!(
            input(&i),
            "<today>2026-10-27 (Tuesday), UTC offset -07:00, time zone America/Los_Angeles</today>\n\
             <job>Software Engineer at Acme</job>\n\
             <invite>\nHi Jane, can you do Tuesday at 2pm ET?\n</invite>\n"
        );
        let unzoned = input(&InviteInput { zone: None, ..i });
        assert!(unzoned.starts_with("<today>2026-10-27 (Tuesday), UTC offset -07:00</today>\n"));
        let long = "é".repeat(MAX_CHARS + 10);
        let capped = input(&InviteInput { text: &long, ..i });
        assert!(capped.contains(&format!("<invite>\n{}\n</invite>", "é".repeat(MAX_CHARS))));
    }

    #[test]
    fn a_reply_of_nulls_reads() {
        let reply = serde_json::json!({
            "kind": null, "starts_at": null, "minutes": null, "with": "", "place": "", "notes": ""
        });
        assert!(super::schema().errors(&reply).is_empty());
        let invite: Invite = serde_json::from_value(reply).unwrap();
        assert_eq!(invite, Invite::default());
    }
}
