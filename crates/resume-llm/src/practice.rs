//! Practice: one typed answer to a prep question read back, never graded (ARCHITECTURE.md §3): which lines of the
//! resume it drew on, which parts of situation, task, action and result it has, what it says differently from the
//! resume you sent, what it claims that the bank doesn't hold yet, a tighter telling, the likely follow-up, and the
//! answer as a story to keep.
//!
//! The model sees the question (its text, why it's likely, the lines it's about, the story it means to tell), the
//! answer, the posting's requirement terms and the resume as sent, tagged with its facts as Prep reads it. A story
//! told alone (Tell it) reads the same way, against its entry as the bank holds it ([`SentResume::entry`]).
//! [`checked`] holds the reply to what the resume, the bank and the answer back.

use std::fmt::Write as _;

use resume_core::story::{Story, StoryTheme};
use resume_core::validate::unsourced_numbers;
use serde::{Deserialize, Serialize};

use crate::prep::{SUMMARY, SentResume};
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
    /// The story the answer means to tell (Tell it, or the prep question's story), as you saved it.
    pub story: Option<&'a Story>,
}

/// The Practice call's input: `<question>` (its text, why, the lines it's about with their text, and the story it
/// means to tell), `<answer>` (at most [`MAX_ANSWER_CHARS`]), `<terms>` (one a line), `<resume>`.
pub fn input(i: &PracticeInput<'_>) -> String {
    let answer = clipped(i.answer);
    let mut s = String::with_capacity(i.sent.text.len() + answer.len() + 1024);
    write_question(&mut s, i.question, i.why, i.lines, i.sent, i.story);
    let _ = writeln!(s, "<answer>\n{answer}\n</answer>");
    write_terms_and_resume(&mut s, i.terms, i.sent);
    s
}

/// `text` trimmed, at most [`MAX_ANSWER_CHARS`].
pub(crate) fn clipped(text: &str) -> &str {
    let text = text.trim();
    match text.char_indices().nth(MAX_ANSWER_CHARS) {
        Some((at, _)) => &text[..at],
        None => text,
    }
}

/// `<question>`: the question, why it's likely, the lines it's about with their text, and the story it means to tell
/// with its parts.
pub(crate) fn write_question(
    s: &mut String,
    question: &str,
    why: &str,
    lines: &[String],
    sent: &SentResume,
    story: Option<&Story>,
) {
    let _ = writeln!(s, "<question>\n{}", question.trim());
    if !why.trim().is_empty() {
        let _ = writeln!(s, "(why) {}", why.trim());
    }
    for id in lines {
        if let Some(l) = sent.line(id) {
            let _ = writeln!(s, "(about) [{}] {}", l.id, l.text);
        }
    }
    if let Some(story) = story {
        let _ = writeln!(s, "(story) {}", story.title.trim());
        for (part, text) in story_parts(story) {
            if !text.trim().is_empty() {
                let _ = writeln!(s, "  ({part}) {}", text.trim());
            }
        }
    }
    s.push_str("</question>\n");
}

/// A story's situation, task, action and result, by name.
pub(crate) fn story_parts(story: &Story) -> [(&'static str, &str); 4] {
    [
        ("situation", story.situation.as_str()),
        ("task", story.task.as_str()),
        ("action", story.action.as_str()),
        ("result", story.result.as_str()),
    ]
}

/// `<terms>` (one a line), then `<resume>` (`<resume ids="none">` for the PDF's text).
pub(crate) fn write_terms_and_resume(s: &mut String, terms: &[String], sent: &SentResume) {
    s.push_str("<terms>\n");
    for t in terms {
        s.push_str(t);
        s.push('\n');
    }
    s.push_str("</terms>\n");
    if sent.lines.is_empty() {
        s.push_str("<resume ids=\"none\">\n");
    } else {
        s.push_str("<resume>\n");
    }
    s.push_str(sent.text.trim());
    s.push_str("\n</resume>\n");
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

/// The answer as a story to keep, in the candidate's words: `entry`, `bullets` and `facts` name what it happened in
/// and draws on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryDraft {
    pub title: String,
    pub entry: Option<String>,
    pub bullets: Vec<String>,
    pub facts: Vec<String>,
    pub situation: String,
    pub task: String,
    pub action: String,
    pub result: String,
    pub skills: Vec<String>,
    pub themes: Vec<StoryTheme>,
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
    /// The answer as a story; `None` when it tells none (and in feedback from before stories).
    #[serde(default)]
    pub story: Option<StoryDraft>,
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
    /// Why the story was dropped: the numbers neither the answer, its bullets nor its facts state.
    pub story: Vec<String>,
}

impl Dropped {
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
            && self.mismatches.is_empty()
            && self.new_claims.is_empty()
            && self.tighter.is_empty()
            && self.story.is_empty()
    }
}

/// `f` held to what the resume as sent, the bank and the answer back: unknown line ids dropped; a mismatch kept only
/// when it cites a line (or a fact) of the resume whose text states every number it quotes from the resume, and the
/// answer states every number it says; a new claim only for one of the resume's bank entries (its name set); the
/// tighter telling dropped when it states a number neither the answer, the lines it and the question cite, nor the
/// facts state; the story kept with only the resume's entries, bullets and facts it names, and dropped when it states
/// a number neither the answer, those bullets and facts, nor the story told (`told`) state, or says nothing.
/// `question_lines`: the ids of the lines the question was about.
pub fn checked(
    f: Feedback,
    sent: &SentResume,
    question_lines: &[String],
    answer: &str,
    told: Option<&Story>,
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
    let story = f
        .story
        .and_then(|s| story_checked(s, sent, answer, told, &mut dropped.story));
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
        story,
        ..f
    };
    (feedback, dropped)
}

/// `s` with only the entry, bullets and facts of the resume it names (its entry, when it names none, the one its first
/// bullet sits under), every part trimmed; `None` when it says nothing, or states a number neither the answer, those
/// bullets and facts, nor `told` (the saved story the answer told) state (the findings go on `dropped`).
fn story_checked(
    s: StoryDraft,
    sent: &SentResume,
    answer: &str,
    told: Option<&Story>,
    dropped: &mut Vec<String>,
) -> Option<StoryDraft> {
    let trim = |t: String| t.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut bullets: Vec<String> = Vec::new();
    for b in s.bullets {
        let b = b.trim();
        let bullet = sent
            .line(b)
            .filter(|l| l.id != SUMMARY && !l.id.contains('#'));
        if bullet.is_some() && !bullets.iter().any(|x| x == b) {
            bullets.push(b.to_owned());
        }
    }
    let mut facts: Vec<String> = Vec::new();
    for f in s.facts {
        let f = f.trim();
        if sent.facts.iter().any(|(_, id, _)| id == f) && !facts.iter().any(|x| x == f) {
            facts.push(f.to_owned());
        }
    }
    let entry = s
        .entry
        .map(|e| e.trim().to_owned())
        .filter(|e| sent.entry_name(e).is_some())
        .or_else(|| {
            bullets
                .first()
                .and_then(|b| sent.line(b))
                .map(|l| l.entry.clone())
        });
    let mut themes: Vec<StoryTheme> = Vec::new();
    for t in s.themes {
        if !themes.contains(&t) {
            themes.push(t);
        }
    }
    let mut skills: Vec<String> = Vec::new();
    for k in s.skills.into_iter().map(trim) {
        if !k.is_empty() && !skills.iter().any(|x| x.eq_ignore_ascii_case(&k)) {
            skills.push(k);
        }
    }
    let story = StoryDraft {
        title: trim(s.title),
        entry,
        bullets,
        facts,
        situation: trim(s.situation),
        task: trim(s.task),
        action: trim(s.action),
        result: trim(s.result),
        skills,
        themes,
    };
    let parts = [
        &story.title,
        &story.situation,
        &story.task,
        &story.action,
        &story.result,
    ];
    if parts[1..].iter().all(|p| p.is_empty()) {
        return None;
    }
    let mut sources: Vec<&str> = vec![answer];
    sources.extend(story.bullets.iter().filter_map(|b| sent.line(b)).map(|l| l.text.as_str()));
    sources.extend(
        sent.facts
            .iter()
            .filter(|(_, id, _)| story.facts.contains(id))
            .map(|(_, _, t)| t.as_str()),
    );
    if let Some(t) = told {
        sources.extend(t.parts());
    }
    for p in parts {
        for finding in unsourced_numbers(p, &sources) {
            if !dropped.contains(&finding) {
                dropped.push(finding);
            }
        }
    }
    dropped.is_empty().then_some(story)
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
    use super::{
        Feedback, Mismatch, NewClaim, PracticeInput, StarPart, StoryDraft, checked, input,
    };
    use resume_core::story::StoryTheme;
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
            story: None,
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
            story: None,
        }
    }

    #[test]
    fn checking_keeps_what_the_resume_and_bank_back() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let (f, dropped) = checked(feedback(), &sent, &[], ANSWER, None);
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
        let (f, dropped) = checked(f, &sent, &[], ANSWER, None);
        assert_eq!(f.tighter, None);
        assert_eq!(dropped.tighter.len(), 1);
        // A number from a fact of the resume's entries is grounded.
        let f = Feedback {
            tighter: Some("The suite ran on 3 services.".into()),
            ..feedback()
        };
        assert!(checked(f, &sent, &[], ANSWER, None).0.tighter.is_some());
    }

    #[test]
    fn a_full_star_has_no_note() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let f = Feedback {
            situation: StarPart::Yes,
            ..feedback()
        };
        assert_eq!(checked(f, &sent, &[], ANSWER, None).0.star_note, None);
    }

    fn draft() -> StoryDraft {
        StoryDraft {
            title: " Caught 20 regressions before release ".into(),
            entry: Some("globex".into()),
            bullets: vec!["acme.2".into(), "acme.9".into(), "acme.2".into()],
            facts: vec!["acme~1".into(), "acme~7".into()],
            situation: "Releases kept breaking the order API.".into(),
            task: "I owned the  tests.".into(),
            action: "I wrote integration tests across 3 services.".into(),
            result: "They caught 20 regressions before release.".into(),
            skills: vec!["GitHub Actions".into(), "github actions".into(), " ".into()],
            themes: vec![StoryTheme::Impact, StoryTheme::Impact],
        }
    }

    #[test]
    fn a_story_keeps_what_the_resume_names_and_the_answer_grounds() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let f = Feedback {
            story: Some(draft()),
            ..feedback()
        };
        let (f, dropped) = checked(f, &sent, &[], ANSWER, None);
        let s = f.story.unwrap();
        assert_eq!(s.title, "Caught 20 regressions before release");
        // globex isn't on the resume: the entry is the first bullet's.
        assert_eq!(s.entry.as_deref(), Some("acme"));
        assert_eq!(s.bullets, ["acme.2"]);
        assert_eq!(s.facts, ["acme~1"]);
        assert_eq!(s.task, "I owned the tests.");
        assert_eq!(s.skills, ["GitHub Actions"]);
        assert_eq!(s.themes, [StoryTheme::Impact]);
        assert!(dropped.story.is_empty());
    }

    #[test]
    fn an_unsourced_number_drops_the_story() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let f = Feedback {
            story: Some(StoryDraft {
                result: "Pages fell by 40%.".into(),
                ..draft()
            }),
            ..feedback()
        };
        let (checked_f, dropped) = checked(f.clone(), &sent, &[], ANSWER, None);
        assert_eq!(checked_f.story, None);
        assert_eq!(dropped.story, ["“40%” is not in the lines it draws on"]);
        // The story the answer told grounds it.
        let mut told = crate::prep::tests::stories().remove(0);
        told.result = "Pages fell by 40%.".into();
        assert!(checked(f, &sent, &[], ANSWER, Some(&told)).0.story.is_some());
        // A story that says nothing is none.
        let empty = Feedback {
            story: Some(StoryDraft {
                title: "Tests".into(),
                ..StoryDraft::default()
            }),
            ..feedback()
        };
        assert_eq!(checked(empty, &sent, &[], ANSWER, None).0.story, None);
    }

    #[test]
    fn a_told_story_is_in_the_question() {
        let (bank, _) = jane();
        let sent = SentResume::entry(&bank, "acme");
        let stories = crate::prep::tests::stories();
        let lines = ["acme.2".to_owned()];
        let text = input(&PracticeInput {
            question: "Tell me about the work you're proudest of.",
            why: "",
            lines: &lines,
            answer: "I wrote tests.",
            terms: &[],
            sent: &sent,
            story: Some(&stories[0]),
        });
        assert!(text.contains(
            "(story) Caught 14 regressions before release\n  (situation) Releases kept breaking the order API.\n"
        ));
        assert!(text.contains("(about) [acme.2] "));
        assert!(text.contains("<resume>\n[acme] Acme Widgets Co."));
    }
}
