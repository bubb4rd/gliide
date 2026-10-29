//! Nudge: small suggestions for one practice question while you answer it, never the answer written for you
//! (ARCHITECTURE.md §3): with nothing typed, where to start; with a draft, what to add or change next; after an
//! answer, what it missed. Each nudge may point to lines of the resume and carry an example you could say, with `…`
//! where only you know the words.
//!
//! The model sees Practice's question (its text, why, the lines it's about, the story it means to tell), your draft,
//! your last answer to the question with what its reading found, the posting's requirement terms and the resume as
//! sent, tagged with its facts. [`checked`] holds the reply to what the resume, the bank and your words back.

use std::fmt::Write as _;

use resume_core::story::Story;
use resume_core::validate::unsourced_numbers;
use serde::{Deserialize, Serialize};

use crate::practice::{StarPart, clipped, story_parts, write_question, write_terms_and_resume};
use crate::prep::SentResume;
use crate::{Call, Llm, LlmError, Schema, Structured};

/// The Nudge instructions (`prompts/nudge.md`).
pub const PROMPT: &str = include_str!("../../../prompts/nudge.md");
/// The reply's schema (`prompts/nudge.schema.json`): see [`Nudge`].
pub const SCHEMA: &str = include_str!("../../../prompts/nudge.schema.json");
/// The most nudges a reply keeps.
pub const MAX_NUDGES: usize = 3;

pub fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("prompts/nudge.schema.json is a valid schema")
}

/// Your last answer to the question and what its reading found.
#[derive(Clone, Copy, Debug)]
pub struct LastAnswer<'a> {
    pub answer: &'a str,
    pub situation: StarPart,
    pub task: StarPart,
    pub action: StarPart,
    pub result: StarPart,
    /// The reading's note on the weakest part.
    pub note: Option<&'a str>,
}

/// What Nudge reads.
#[derive(Clone, Copy, Debug)]
pub struct NudgeInput<'a> {
    /// The question as asked.
    pub question: &'a str,
    /// Why it's likely.
    pub why: &'a str,
    /// The ids of the lines it's about.
    pub lines: &'a [String],
    /// What you've typed so far; may be empty.
    pub draft: &'a str,
    pub last: Option<LastAnswer<'a>>,
    /// The posting's requirement terms, most important first.
    pub terms: &'a [String],
    pub sent: &'a SentResume,
    /// The story the question links, as you saved it.
    pub story: Option<&'a Story>,
}

/// The Nudge call's input: Practice's `<question>`, `<draft>` (at most [`crate::practice::MAX_ANSWER_CHARS`]),
/// `<earlier>` (only with a last answer), `<terms>`, `<resume>`.
pub fn input(i: &NudgeInput<'_>) -> String {
    let draft = clipped(i.draft);
    let mut s = String::with_capacity(i.sent.text.len() + draft.len() + 1024);
    write_question(&mut s, i.question, i.why, i.lines, i.sent, i.story);
    s.push_str("<draft>\n");
    if !draft.is_empty() {
        s.push_str(draft);
        s.push('\n');
    }
    s.push_str("</draft>\n");
    if let Some(last) = &i.last {
        let word = |p: StarPart| match p {
            StarPart::Yes => "yes",
            StarPart::Thin => "thin",
            StarPart::No => "no",
        };
        let _ = writeln!(
            s,
            "<earlier>\n{}\n(read) situation {}, task {}, action {}, result {}",
            clipped(last.answer),
            word(last.situation),
            word(last.task),
            word(last.action),
            word(last.result)
        );
        if let Some(note) = last.note.map(str::trim).filter(|n| !n.is_empty()) {
            let _ = writeln!(s, "(note) {note}");
        }
        s.push_str("</earlier>\n");
    }
    write_terms_and_resume(&mut s, i.terms, i.sent);
    s
}

/// One nudge: a direction or a question (`text`), the resume lines it points to, and what you could say for it, with
/// `…` where only you know the words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nudge {
    pub text: String,
    pub lines: Vec<String>,
    pub example: Option<String>,
}

#[derive(Deserialize)]
struct Reply {
    nudges: Vec<Nudge>,
}

/// What [`checked`] took out, for the log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dropped {
    /// Line ids no line of the resume has.
    pub lines: Vec<String>,
    /// Nudges with nothing to say, or past [`MAX_NUDGES`].
    pub nudges: usize,
    /// Why examples were dropped: the numbers your words, the story, the lines and the facts don't state.
    pub examples: Vec<String>,
}

impl Dropped {
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.nudges == 0 && self.examples.is_empty()
    }
}

/// `nudges` held to what the resume as sent, the bank and your words back: unknown line ids dropped; a nudge with no
/// text dropped, the rest capped at [`MAX_NUDGES`]; an example dropped (its nudge kept) when it states a number neither
/// `draft`, `last` (your last answer), `told` (the question's story), the lines it and the question cite, nor the
/// facts state. `question_lines`: the ids of the lines the question was about.
pub fn checked(
    nudges: Vec<Nudge>,
    sent: &SentResume,
    question_lines: &[String],
    draft: &str,
    last: Option<&str>,
    told: Option<&Story>,
) -> (Vec<Nudge>, Dropped) {
    let mut dropped = Dropped::default();
    let mut out = Vec::new();
    for n in nudges {
        let text = n.text.trim();
        if text.is_empty() || out.len() == MAX_NUDGES {
            dropped.nudges += 1;
            continue;
        }
        let lines = sent.keep(n.lines, &mut dropped.lines);
        let example = n
            .example
            .map(|e| e.trim().to_owned())
            .filter(|e| !e.is_empty())
            .filter(|e| {
                let cited: Vec<String> = lines.iter().chain(question_lines).cloned().collect();
                let mut sources = vec![draft];
                sources.extend(last);
                if let Some(story) = told {
                    sources.extend(story_parts(story).map(|(_, t)| t));
                }
                sources.extend(sent.sources(&cited));
                sources.extend(sent.all_facts());
                let unsourced = unsourced_numbers(e, &sources);
                let ok = unsourced.is_empty();
                dropped.examples.extend(unsourced);
                ok
            });
        out.push(Nudge {
            text: text.to_owned(),
            lines,
            example,
        });
    }
    (out, dropped)
}

/// Runs Nudge with `model`: the nudges as written (unchecked: see [`checked`]).
pub async fn read(
    llm: &impl Llm,
    schema: &Schema,
    input: &NudgeInput<'_>,
    model: &str,
) -> Result<(Vec<Nudge>, Structured), LlmError> {
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
        Ok(r) => Ok((r.nudges, output)),
        Err(e) => Err(LlmError::Decode {
            target: "Nudge",
            message: e.to_string(),
            output,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{LastAnswer, MAX_NUDGES, Nudge, NudgeInput, checked, input};
    use crate::practice::StarPart;
    use crate::prep::SentResume;
    use crate::prep::tests::{jane, stories};

    #[test]
    fn schema_compiles() {
        super::schema();
    }

    fn nudge(text: &str, lines: &[&str], example: Option<&str>) -> Nudge {
        Nudge {
            text: text.into(),
            lines: lines.iter().map(|l| (*l).to_owned()).collect(),
            example: example.map(Into::into),
        }
    }

    #[test]
    fn input_carries_the_draft_and_the_last_answer() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let lines = ["acme.2".to_owned()];
        let text = input(&NudgeInput {
            question: "Tell me about the regressions you caught.",
            why: "",
            lines: &lines,
            draft: "  ",
            last: Some(LastAnswer {
                answer: "I wrote tests at Acme.",
                situation: StarPart::Yes,
                task: StarPart::Thin,
                action: StarPart::Yes,
                result: StarPart::No,
                note: Some("Say how it came out."),
            }),
            terms: &[],
            sent: &sent,
            story: None,
        });
        assert!(text.contains(
            "</question>\n<draft>\n</draft>\n<earlier>\nI wrote tests at Acme.\n\
             (read) situation yes, task thin, action yes, result no\n(note) Say how it came out.\n</earlier>\n<terms>\n"
        ));
    }

    #[test]
    fn checking_keeps_what_the_resume_backs() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let (out, dropped) = checked(
            vec![
                nudge(" Open with the order API tests. ", &["acme.2", "acme.9"], Some(" ")),
                nudge("  ", &[], None),
                nudge("Say what broke before them.", &[], Some("Releases kept breaking because …")),
                nudge("Name your part.", &[], None),
                nudge("One too many.", &[], None),
            ],
            &sent,
            &[],
            "",
            None,
            None,
        );
        assert_eq!(out.len(), MAX_NUDGES);
        assert_eq!(out[0].text, "Open with the order API tests.");
        assert_eq!(out[0].lines, ["acme.2"]);
        assert_eq!(out[0].example, None);
        assert_eq!(out[1].example.as_deref(), Some("Releases kept breaking because …"));
        assert_eq!(dropped.lines, ["acme.9"]);
        assert_eq!(dropped.nudges, 2);
    }

    #[test]
    fn an_unsourced_number_drops_the_example_not_the_nudge() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let invented = || vec![nudge("Say how much it cut.", &[], Some("It cut release bugs by 60%."))];
        let (out, dropped) = checked(invented(), &sent, &[], "I wrote tests.", None, None);
        assert_eq!(out[0].text, "Say how much it cut.");
        assert_eq!(out[0].example, None);
        assert_eq!(dropped.examples.len(), 1);
        // Your draft, your last answer and the question's story ground it.
        assert!(checked(invented(), &sent, &[], "They cut bugs by 60%.", None, None).0[0].example.is_some());
        assert!(checked(invented(), &sent, &[], "", Some("Bugs fell 60%."), None).0[0].example.is_some());
        let mut told = stories().remove(0);
        told.result = "Release bugs fell by 60%.".into();
        assert!(checked(invented(), &sent, &[], "", None, Some(&told)).0[0].example.is_some());
        // So do the lines it points to, and the facts.
        let cited = vec![nudge("Use the tests.", &["acme.2"], Some("They caught 14 regressions."))];
        assert!(checked(cited, &sent, &[], "", None, None).0[0].example.is_some());
        let fact = vec![nudge("Say where it ran.", &[], Some("The suite ran on 3 services."))];
        assert!(checked(fact, &sent, &[], "", None, None).0[0].example.is_some());
    }
}
