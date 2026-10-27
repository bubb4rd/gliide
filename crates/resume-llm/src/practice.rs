//! Practice: one typed answer to a prep question read back, never graded (ARCHITECTURE.md §3): which lines of the
//! resume it drew on, which parts of situation, task, action and result it has, what it says differently from the
//! resume you sent, what it claims that the bank doesn't hold yet, a tighter telling, and the likely follow-up.
//!
//! The model sees the question (its text, why it's likely, the lines it's about), the answer, the posting's
//! requirement terms and the resume as sent, tagged with its facts as Prep reads it. [`checked`] holds the reply to
//! what the resume, the bank and the answer back.

use std::fmt::Write as _;

use resume_core::validate::unsourced_numbers;
use serde::{Deserialize, Serialize};

use crate::prep::SentResume;
use crate::{Call, Llm, LlmError, Schema, Structured};

/// The Practice instructions (`prompts/practice.md`).
pub const PROMPT: &str = include_str!("../../../prompts/practice.md");
/// The reply's schema (`prompts/practice.schema.json`): see [`Feedback`].
pub const SCHEMA: &str = include_str!("../../../prompts/practice.schema.json");
/// The most of an answer the model reads.
pub const MAX_ANSWER_CHARS: usize = 6_000;
/// The most mismatches, and new claims, a reading keeps.
pub const MAX_ITEMS: usize = 5;

pub fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("prompts/practice.schema.json is a valid schema")
}

/// What Practice reads.
#[derive(Clone, Copy, Debug)]
pub struct PracticeInput<'a> {
    /// The question as asked.
    pub question: &'a str,
    /// Why it's likely.
    pub why: &'a str,
    /// The ids of the lines it's about.
    pub lines: &'a [String],
    pub answer: &'a str,
    /// The posting's requirement terms, most important first.
    pub terms: &'a [String],
    pub sent: &'a SentResume,
}

/// The Practice call's input: `<question>` (its text, why, and the lines it's about with their text), `<answer>` (at
/// most [`MAX_ANSWER_CHARS`]), `<terms>` (one a line), `<resume>`.
pub fn input(i: &PracticeInput<'_>) -> String {
    let answer = i.answer.trim();
    let answer = match answer.char_indices().nth(MAX_ANSWER_CHARS) {
        Some((at, _)) => &answer[..at],
        None => answer,
    };
    let mut s = String::with_capacity(i.sent.text.len() + answer.len() + 1024);
    let _ = writeln!(s, "<question>\n{}", i.question.trim());
    if !i.why.trim().is_empty() {
        let _ = writeln!(s, "(why) {}", i.why.trim());
    }
    for id in i.lines {
        if let Some(l) = i.sent.line(id) {
            let _ = writeln!(s, "(about) [{}] {}", l.id, l.text);
        }
    }
    s.push_str("</question>\n");
    let _ = writeln!(s, "<answer>\n{answer}\n</answer>");
    s.push_str("<terms>\n");
    for t in i.terms {
        s.push_str(t);
        s.push('\n');
    }
    s.push_str("</terms>\n");
    if i.sent.lines.is_empty() {
        s.push_str("<resume ids=\"none\">\n");
    } else {
        s.push_str("<resume>\n");
    }
    s.push_str(i.sent.text.trim());
    s.push_str("\n</resume>\n");
    s
}

/// Whether an answer has a part of situation, task, action and result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StarPart {
    Yes,
    Thin,
    No,
}

/// A number, name, title or date the answer says differently from a line of the resume or a fact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mismatch {
    pub said: String,
    pub resume: String,
    /// The line (or fact) id that says it.
    pub line: String,
}

/// A concrete claim of the answer that neither the resume nor the facts hold, offered as a fact of `entry`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewClaim {
    pub text: String,
    pub entry: String,
    /// Set by [`checked`].
    #[serde(default)]
    pub entry_name: String,
}

/// What an answer did; never a grade.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feedback {
    pub lines: Vec<String>,
    pub situation: StarPart,
    pub task: StarPart,
    pub action: StarPart,
    pub result: StarPart,
    pub star_note: Option<String>,
    pub mismatches: Vec<Mismatch>,
    pub new_claims: Vec<NewClaim>,
    pub tighter: Option<String>,
    pub follow_up: Option<String>,
}

/// What [`checked`] took out, for the log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dropped {
    /// Line ids no line of the resume has.
    pub lines: Vec<String>,
    /// Mismatches whose line isn't on the resume or doesn't state what they quote, or whose numbers the answer
    /// doesn't say.
    pub mismatches: Vec<Mismatch>,
    /// New claims whose entry isn't one of the resume's bank entries.
    pub new_claims: Vec<NewClaim>,
    /// Why the tighter telling was dropped: the numbers neither the answer, the lines nor the facts state.
    pub tighter: Vec<String>,
}

impl Dropped {
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
            && self.mismatches.is_empty()
            && self.new_claims.is_empty()
            && self.tighter.is_empty()
    }
}

/// `f` held to what the resume as sent, the bank and the answer back: unknown line ids dropped; a mismatch kept only
/// when it cites a line (or a fact) of the resume whose text states every number it quotes from the resume, and the
/// answer states every number it says; a new claim only for one of the resume's bank entries (its name set); the
/// tighter telling dropped when it states a number neither the answer, the lines it and the question cite, nor the
/// facts state. `question_lines`: the ids of the lines the question was about.
pub fn checked(
    f: Feedback,
    sent: &SentResume,
    question_lines: &[String],
    answer: &str,
) -> (Feedback, Dropped) {
    let mut dropped = Dropped::default();
    let lines = sent.keep(f.lines, &mut dropped.lines);
    let mut mismatches = Vec::new();
    for m in f.mismatches {
        let line = m.line.trim();
        let stated = sent.line(line).map(|l| l.text.as_str()).or_else(|| {
            sent.facts
                .iter()
                .find(|(_, id, _)| id == line)
                .map(|(_, _, t)| t.as_str())
        });
        let ok = stated.is_some_and(|text| {
            unsourced_numbers(&m.resume, &[text]).is_empty()
                && unsourced_numbers(&m.said, &[answer]).is_empty()
        });
        if ok && mismatches.len() < MAX_ITEMS {
            mismatches.push(Mismatch {
                said: m.said.trim().to_owned(),
                resume: m.resume.trim().to_owned(),
                line: line.to_owned(),
            });
        } else {
            dropped.mismatches.push(m);
        }
    }
    let mut new_claims = Vec::new();
    for c in f.new_claims {
        match sent.entry_name(c.entry.trim()) {
            Some(name) if !c.text.trim().is_empty() && new_claims.len() < MAX_ITEMS => {
                new_claims.push(NewClaim {
                    text: c.text.trim().to_owned(),
                    entry: c.entry.trim().to_owned(),
                    entry_name: name.to_owned(),
                });
            }
            _ => dropped.new_claims.push(c),
        }
    }
    let tighter = some(f.tighter).filter(|t| {
        let cited: Vec<String> = lines
            .iter()
            .chain(question_lines)
            .cloned()
            .collect::<Vec<_>>();
        let mut sources = vec![answer];
        sources.extend(sent.sources(&cited));
        sources.extend(sent.all_facts());
        dropped.tighter = unsourced_numbers(t, &sources);
        dropped.tighter.is_empty()
    });
    let all_there = [f.situation, f.task, f.action, f.result]
        .iter()
        .all(|p| *p == StarPart::Yes);
    let feedback = Feedback {
        lines,
        star_note: some(f.star_note).filter(|_| !all_there),
        mismatches,
        new_claims,
        tighter,
        follow_up: some(f.follow_up),
        ..f
    };
    (feedback, dropped)
}

/// `s` trimmed; `None` when empty.
fn some(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

/// Runs Practice with `model`: the reply as written (unchecked: see [`checked`]).
pub async fn read(
    llm: &impl Llm,
    schema: &Schema,
    input: &PracticeInput<'_>,
    model: &str,
) -> Result<(Feedback, Structured), LlmError> {
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
    match Feedback::deserialize(&output.value) {
        Ok(f) => Ok((f, output)),
        Err(e) => Err(LlmError::Decode {
            target: "Feedback",
            message: e.to_string(),
            output,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{Feedback, Mismatch, NewClaim, PracticeInput, StarPart, checked, input};
    use crate::prep::SentResume;
    use crate::prep::tests::jane;

    #[test]
    fn schema_compiles() {
        super::schema();
    }

    #[test]
    fn input_carries_every_part() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let terms = ["Rust".to_owned(), "CI/CD".to_owned()];
        let lines = ["acme.2".to_owned(), "gone".to_owned()];
        let text = input(&PracticeInput {
            question: " Tell me about the regressions you caught. ",
            why: "A number invites a follow-up",
            lines: &lines,
            answer: "\n I caught 20 regressions. \n",
            terms: &terms,
            sent: &sent,
        });
        assert!(text.starts_with(
            "<question>\nTell me about the regressions you caught.\n(why) A number invites a follow-up\n\
             (about) [acme.2] Wrote integration tests in GitHub Actions for the order API, catching 14 \
             regressions before release\n</question>\n<answer>\nI caught 20 regressions.\n</answer>\n\
             <terms>\nRust\nCI/CD\n</terms>\n<resume>\n"
        ));
        assert!(text.contains("[acme~1] (fact) The test suite ran on 3 services"));
        assert!(text.ends_with("\n</resume>\n"));
    }

    const ANSWER: &str = "At Acme I ran the on-call rotation and wrote integration tests for the order API in \
                          GitHub Actions. They caught 20 regressions before release.";

    fn feedback() -> Feedback {
        Feedback {
            lines: vec!["acme.2".into(), "acme.9".into()],
            situation: StarPart::Thin,
            task: StarPart::Yes,
            action: StarPart::Yes,
            result: StarPart::Yes,
            star_note: Some(" Say why the tests were needed. ".into()),
            mismatches: vec![
                Mismatch {
                    said: "20 regressions".into(),
                    resume: "14 regressions".into(),
                    line: "acme.2".into(),
                },
                // The line doesn't say 30.
                Mismatch {
                    said: "20 regressions".into(),
                    resume: "30 regressions".into(),
                    line: "acme.2".into(),
                },
                Mismatch {
                    said: "20 regressions".into(),
                    resume: "14 regressions".into(),
                    line: "acme.7".into(),
                },
            ],
            new_claims: vec![
                NewClaim {
                    text: "Ran the on-call rotation".into(),
                    entry: "acme".into(),
                    entry_name: String::new(),
                },
                NewClaim {
                    text: "Mentored interns".into(),
                    entry: "globex".into(),
                    entry_name: String::new(),
                },
            ],
            tighter: Some(
                "At Acme I wrote integration tests for the order API that caught 14 regressions before release."
                    .into(),
            ),
            follow_up: Some("  ".into()),
        }
    }

    #[test]
    fn checking_keeps_what_the_resume_and_bank_back() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let (f, dropped) = checked(feedback(), &sent, &[], ANSWER);
        assert_eq!(f.lines, ["acme.2"]);
        assert_eq!(dropped.lines, ["acme.9"]);
        assert_eq!(f.mismatches.len(), 1);
        assert_eq!(f.mismatches[0].resume, "14 regressions");
        assert_eq!(dropped.mismatches.len(), 2);
        assert_eq!(f.new_claims.len(), 1);
        assert_eq!(f.new_claims[0].entry, "acme");
        assert_eq!(f.new_claims[0].entry_name, "Acme Widgets Co.");
        assert_eq!(dropped.new_claims[0].entry, "globex");
        assert!(f.tighter.is_some());
        assert_eq!(f.star_note.as_deref(), Some("Say why the tests were needed."));
        assert_eq!(f.follow_up, None);
        assert!(dropped.tighter.is_empty());
    }

    #[test]
    fn an_unsourced_number_drops_the_tighter_telling() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let f = Feedback {
            tighter: Some("I wrote tests that cut release bugs by 60%.".into()),
            ..feedback()
        };
        let (f, dropped) = checked(f, &sent, &[], ANSWER);
        assert_eq!(f.tighter, None);
        assert_eq!(dropped.tighter.len(), 1);
        // A number from a fact of the resume's entries is grounded.
        let f = Feedback {
            tighter: Some("The suite ran on 3 services.".into()),
            ..feedback()
        };
        assert!(checked(f, &sent, &[], ANSWER).0.tighter.is_some());
    }

    #[test]
    fn a_full_star_has_no_note() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let f = Feedback {
            situation: StarPart::Yes,
            ..feedback()
        };
        assert_eq!(checked(f, &sent, &[], ANSWER).0.star_note, None);
    }
}
