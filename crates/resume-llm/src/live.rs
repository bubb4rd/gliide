//! Live: the interviewer's turn in a live interview (ARCHITECTURE.md §3). After each answer to a prep question, or
//! reply to a follow-up, one call decides one follow-up on what you just said, or none; it only asks: no praise, no
//! bridge, no grade. The prep question itself is asked verbatim, with no call, and the app stops after
//! [`MAX_FOLLOW_UPS`].
//!
//! The model sees Practice's question (its text, why, the lines it's about, the story it means to tell), the exchange
//! so far (your answer, then each follow-up and your reply), the posting's requirement terms and the resume as sent,
//! tagged with its facts. [`checked`] keeps the phrase it picked up only when it's in your last reply.

use std::fmt::Write as _;

use resume_core::story::Story;
use serde::{Deserialize, Serialize};

use crate::practice::{clipped, write_question, write_terms_and_resume};
use crate::prep::SentResume;
use crate::{Call, Llm, LlmError, Schema, Structured};

/// The Live instructions (`prompts/live.md`).
pub const PROMPT: &str = include_str!("../../../prompts/live.md");
/// The reply's schema (`prompts/live.schema.json`): see [`FollowUp`].
pub const SCHEMA: &str = include_str!("../../../prompts/live.schema.json");
/// The most follow-ups one question gets: after the last one's reply, the next question, with no call.
pub const MAX_FOLLOW_UPS: usize = 2;

pub fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("prompts/live.schema.json is a valid schema")
}

/// What Live reads.
#[derive(Clone, Copy, Debug)]
pub struct LiveInput<'a> {
    /// The question as asked.
    pub question: &'a str,
    /// Why it's likely.
    pub why: &'a str,
    /// The ids of the lines it's about.
    pub lines: &'a [String],
    /// Your answer to the question.
    pub answer: &'a str,
    /// Each follow-up asked so far and your reply, in order.
    pub follow_ups: &'a [(String, String)],
    /// The posting's requirement terms, most important first.
    pub terms: &'a [String],
    pub sent: &'a SentResume,
    /// The story the question links, as you saved it.
    pub story: Option<&'a Story>,
}

/// The Live call's input: Practice's `<question>`, `<exchange>` (your answer, then each follow-up and reply, each at
/// most [`crate::practice::MAX_ANSWER_CHARS`]), `<terms>`, `<resume>`.
pub fn input(i: &LiveInput<'_>) -> String {
    let said: usize = i.answer.len()
        + i.follow_ups
            .iter()
            .map(|(a, r)| a.len() + r.len())
            .sum::<usize>();
    let mut s = String::with_capacity(i.sent.text.len() + said + 1024);
    write_question(&mut s, i.question, i.why, i.lines, i.sent, i.story);
    let _ = writeln!(s, "<exchange>\n(answer) {}", clipped(i.answer));
    for (asked, reply) in i.follow_ups {
        let _ = writeln!(
            s,
            "(follow-up) {}\n(reply) {}",
            asked.trim(),
            clipped(reply)
        );
    }
    s.push_str("</exchange>\n");
    write_terms_and_resume(&mut s, i.terms, i.sent);
    s
}

/// The interviewer's turn: one follow-up and the phrase of your last reply it takes up, or none.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowUp {
    pub follow_up: Option<String>,
    pub picked: Option<String>,
}

/// `f` held to your words: the follow-up trimmed, none when empty; `picked` kept only with a follow-up and when it's
/// in `last` (your last answer or reply) word for word, ignoring case and runs of whitespace (quotes around it
/// trimmed), as `last` writes it.
pub fn checked(f: FollowUp, last: &str) -> FollowUp {
    let follow_up = f
        .follow_up
        .map(|q| q.trim().to_owned())
        .filter(|q| !q.is_empty());
    let picked = follow_up
        .as_ref()
        .and(f.picked)
        .and_then(|p| found(last, &p).map(str::to_owned));
    FollowUp { follow_up, picked }
}

/// `phrase` (trimmed of whitespace and quotes) as it stands in `text`: the first span of `text` equal to it ignoring
/// case and runs of whitespace; `None` when there's none, or the phrase is empty.
fn found<'t>(text: &'t str, phrase: &str) -> Option<&'t str> {
    let phrase = phrase.trim_matches(|c: char| c.is_whitespace() || "\"“”'‘’".contains(c));
    let (needle, _) = folded(phrase);
    if needle.is_empty() {
        return None;
    }
    let (hay, starts) = folded(text);
    let at = hay
        .windows(needle.len())
        .position(|w| w == needle.as_slice())?;
    let start = starts[at];
    let last = starts[at + needle.len() - 1];
    let end = last + text[last..].chars().next().map_or(0, char::len_utf8);
    Some(&text[start..end])
}

/// `text` lowercased with each run of whitespace one space and none at the ends, as chars, and for each the byte
/// offset in `text` of the char it came from (a run's first).
fn folded(text: &str) -> (Vec<char>, Vec<usize>) {
    let (mut chars, mut starts) = (
        Vec::with_capacity(text.len()),
        Vec::with_capacity(text.len()),
    );
    let mut space = None;
    for (at, c) in text.char_indices() {
        if c.is_whitespace() {
            space = space.or(Some(at));
            continue;
        }
        if let Some(s) = space.take()
            && !chars.is_empty()
        {
            chars.push(' ');
            starts.push(s);
        }
        for l in c.to_lowercase() {
            chars.push(l);
            starts.push(at);
        }
    }
    (chars, starts)
}

/// Runs Live with `model`: the reply as written (unchecked: see [`checked`]).
pub async fn read(
    llm: &impl Llm,
    schema: &Schema,
    input: &LiveInput<'_>,
    model: &str,
) -> Result<(FollowUp, Structured), LlmError> {
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
    match FollowUp::deserialize(&output.value) {
        Ok(f) => Ok((f, output)),
        Err(e) => Err(LlmError::Decode {
            target: "FollowUp",
            message: e.to_string(),
            output,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{FollowUp, LiveInput, checked, input};
    use crate::prep::SentResume;
    use crate::prep::tests::jane;

    #[test]
    fn schema_compiles() {
        super::schema();
    }

    #[test]
    fn input_carries_the_exchange_in_order() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let lines = ["acme.2".to_owned()];
        let follow_ups = [(
            " Who wrote the consumer? ".to_owned(),
            "\n I did, in Rust. \n".to_owned(),
        )];
        let terms = ["Rust".to_owned()];
        let text = input(&LiveInput {
            question: "Tell me about the regressions you caught.",
            why: "",
            lines: &lines,
            answer: " We read order events from a Kafka topic. ",
            follow_ups: &follow_ups,
            terms: &terms,
            sent: &sent,
            story: None,
        });
        assert!(text.starts_with(
            "<question>\nTell me about the regressions you caught.\n(about) [acme.2] "
        ));
        assert!(text.contains(
            "</question>\n<exchange>\n(answer) We read order events from a Kafka topic.\n\
             (follow-up) Who wrote the consumer?\n(reply) I did, in Rust.\n</exchange>\n<terms>\nRust\n</terms>\n<resume>\n"
        ));
        assert!(text.ends_with("\n</resume>\n"));
    }

    fn reply(follow_up: Option<&str>, picked: Option<&str>) -> FollowUp {
        FollowUp {
            follow_up: follow_up.map(Into::into),
            picked: picked.map(Into::into),
        }
    }

    const LAST: &str = "We  read order events\nfrom a Kafka topic, then wrote them to Postgres.";

    #[test]
    fn picked_is_kept_as_the_reply_writes_it() {
        // Case and whitespace runs aside, and quotes around it trimmed.
        let f = checked(
            reply(
                Some(" Who wrote the consumer? "),
                Some("“Read order events from a kafka topic”"),
            ),
            LAST,
        );
        assert_eq!(f.follow_up.as_deref(), Some("Who wrote the consumer?"));
        assert_eq!(
            f.picked.as_deref(),
            Some("read order events\nfrom a Kafka topic")
        );
        let f = checked(reply(Some("Why Postgres?"), Some("postgres.")), LAST);
        assert_eq!(f.picked.as_deref(), Some("Postgres."));
    }

    #[test]
    fn picked_not_in_the_reply_is_dropped() {
        // Paraphrased, or from an earlier answer.
        let f = checked(
            reply(
                Some("Who wrote the consumer?"),
                Some("consumed Kafka events"),
            ),
            LAST,
        );
        assert_eq!(f.follow_up.as_deref(), Some("Who wrote the consumer?"));
        assert_eq!(f.picked, None);
        assert_eq!(
            checked(reply(Some("Why?"), Some(" “” ")), LAST).picked,
            None
        );
    }

    #[test]
    fn an_empty_follow_up_is_none() {
        assert_eq!(
            checked(reply(Some("  "), Some("Kafka topic")), LAST),
            FollowUp::default()
        );
        assert_eq!(
            checked(reply(None, Some("Kafka topic")), LAST),
            FollowUp::default()
        );
        assert_eq!(checked(reply(None, None), LAST), FollowUp::default());
    }
}
