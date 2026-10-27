//! Dig: gliide asks about one bank entry and turns your answers into facts and additions (ARCHITECTURE.md §3).
//!
//! Two calls. **Questions** sees the entry tagged as the bank holds it (header, note lines, bullets with their variants
//! and skills, facts), the masters that show it, the requirement terms your saved postings ask for that the entry
//! doesn't state (counted in code), and today's date; it writes a read of the entry and 4–6 questions, each about a
//! bullet or the entry. **Capture** sees the entry, each question with your answer (or that you skipped it, or that it
//! doesn't apply) and a free note; it writes facts in your words, additive ops (a variant, a new bullet, skills, a
//! story: never a rewrite of a bullet's text), follow-ups for vague answers, and what it couldn't use.
//!
//! The prompts ask for honesty; [`questions_checked`], [`facts_checked`] and [`ops_checked`] hold the replies to what
//! the entry and your answers back.

use std::fmt::Write as _;

use resume_core::bank::{Bank, BankEntry};
use resume_core::story::StoryTheme;
use resume_core::validate::{Grounding, numbers, same_text};
use serde::{Deserialize, Serialize};

use crate::practice::StoryDraft;
use crate::suggest::write_entry;
use crate::{Call, Llm, LlmError, Schema, Structured};

/// The Questions instructions (`prompts/dig-questions.md`).
pub const QUESTIONS_PROMPT: &str = include_str!("../../../prompts/dig-questions.md");
/// Their reply's schema (`prompts/dig-questions.schema.json`): see [`Questions`].
pub const QUESTIONS_SCHEMA: &str = include_str!("../../../prompts/dig-questions.schema.json");
/// The Capture instructions (`prompts/dig-capture.md`).
pub const CAPTURE_PROMPT: &str = include_str!("../../../prompts/dig-capture.md");
/// Its reply's schema (`prompts/dig-capture.schema.json`): see [`Capture`].
pub const CAPTURE_SCHEMA: &str = include_str!("../../../prompts/dig-capture.schema.json");
/// The most questions a set keeps.
pub const MAX_QUESTIONS: usize = 6;
/// The most requirement terms Questions reads.
pub const MAX_TERMS: usize = 8;
/// The most facts, and ops, a capture keeps.
pub const MAX_ITEMS: usize = 8;
/// The most follow-ups a capture keeps.
pub const MAX_FOLLOW_UPS: usize = 3;
/// The most of one answer (and of the note) the model reads.
pub const MAX_ANSWER_CHARS: usize = 2_000;
/// How the free note is cited.
pub const NOTE: &str = "note";

pub fn questions_schema() -> Schema {
    Schema::parse(QUESTIONS_SCHEMA).expect("prompts/dig-questions.schema.json is a valid schema")
}

pub fn capture_schema() -> Schema {
    Schema::parse(CAPTURE_SCHEMA).expect("prompts/dig-capture.schema.json is a valid schema")
}

// ── Questions ───────────────────────────────────────────────────────────────────────────────────────────────────────

/// What Questions reads.
#[derive(Clone, Copy, Debug)]
pub struct QuestionsInput<'a> {
    pub entry: &'a BankEntry,
    /// The titles of the masters that show it.
    pub masters: &'a [String],
    /// Requirement terms your postings ask for that the entry doesn't state, `(term, postings asking)`, most asked
    /// first (at most [`MAX_TERMS`] are read).
    pub terms: &'a [(String, usize)],
    /// `YYYY-MM-DD`.
    pub today: &'a str,
}

/// The Questions call's input: `<today>`, `<entry>` (tagged), `<masters>` (titles, or `none`), `<terms>` (`- term
/// (asked in N postings)`, one a line).
pub fn questions_input(i: &QuestionsInput<'_>) -> String {
    let mut s = String::with_capacity(2048);
    let _ = writeln!(s, "<today>{}</today>", i.today);
    s.push_str("<entry>\n");
    write_entry(&mut s, i.entry);
    s.push_str("</entry>\n");
    if i.masters.is_empty() {
        s.push_str("<masters>none</masters>\n");
    } else {
        let _ = writeln!(s, "<masters>{}</masters>", i.masters.join(", "));
    }
    s.push_str("<terms>\n");
    for (term, n) in i.terms.iter().take(MAX_TERMS) {
        let plural = if *n == 1 { "" } else { "s" };
        let _ = writeln!(s, "- {term} (asked in {n} posting{plural})");
    }
    s.push_str("</terms>\n");
    s
}

/// One question about the entry (or a follow-up).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    /// `d1`, `d2`… (follow-ups `f1`…): set by [`questions_checked`].
    #[serde(default)]
    pub id: String,
    /// A bullet's id, or the entry's.
    pub about: String,
    /// "Bullet 2", "The entry": set by [`questions_checked`].
    #[serde(default)]
    pub about_label: String,
    pub text: String,
    pub why: String,
}

/// An entry's read and its questions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Questions {
    pub read: String,
    pub questions: Vec<Question>,
}

/// `q` held to the entry: empty questions dropped, at most [`MAX_QUESTIONS`], numbered `d1`…, each about one of the
/// entry's bullets or the entry itself (anything else reads as the entry), labelled by the bullet's place.
pub fn questions_checked(q: Questions, entry: &BankEntry) -> Questions {
    Questions {
        read: q.read.trim().to_owned(),
        questions: numbered(q.questions, entry, "d", MAX_QUESTIONS),
    }
}

fn numbered(qs: Vec<Question>, entry: &BankEntry, prefix: &str, max: usize) -> Vec<Question> {
    qs.into_iter()
        .filter(|q| !q.text.trim().is_empty())
        .take(max)
        .enumerate()
        .map(|(i, q)| {
            let (about, about_label) = about(entry, q.about.trim());
            Question {
                id: format!("{prefix}{}", i + 1),
                about,
                about_label,
                text: q.text.trim().to_owned(),
                why: q.why.trim().to_owned(),
            }
        })
        .collect()
}

/// What a question is about and its label: bullet `id` of the entry ("Bullet 2", by its place), else the entry.
pub fn about(entry: &BankEntry, id: &str) -> (String, String) {
    match entry.bullets.iter().position(|b| b.id == id) {
        Some(i) => (id.to_owned(), format!("Bullet {}", i + 1)),
        None => (entry.id.clone(), "The entry".to_owned()),
    }
}

/// Runs Questions with `model`: the reply as written (unchecked: see [`questions_checked`]).
pub async fn ask(
    llm: &impl Llm,
    schema: &Schema,
    input: &QuestionsInput<'_>,
    model: &str,
) -> Result<(Questions, Structured), LlmError> {
    let text = questions_input(input);
    let output = llm
        .structured(
            Call {
                system: QUESTIONS_PROMPT,
                input: &text,
                model: Some(model),
            },
            schema,
        )
        .await?;
    match Questions::deserialize(&output.value) {
        Ok(q) => Ok((q, output)),
        Err(e) => Err(LlmError::Decode {
            target: "Questions",
            message: e.to_string(),
            output,
        }),
    }
}

// ── Capture ─────────────────────────────────────────────────────────────────────────────────────────────────────────

/// One question as you answered it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answered {
    pub question: Question,
    /// `None`: skipped.
    pub answer: Option<String>,
    pub not_applicable: bool,
}

impl Answered {
    /// The answer's words, when you gave any.
    pub fn said(&self) -> Option<&str> {
        if self.not_applicable {
            return None;
        }
        self.answer
            .as_deref()
            .map(str::trim)
            .filter(|a| !a.is_empty())
    }
}

/// What Capture reads.
#[derive(Clone, Copy, Debug)]
pub struct CaptureInput<'a> {
    pub entry: &'a BankEntry,
    pub answers: &'a [Answered],
    /// Anything else you wrote.
    pub note: &'a str,
    /// `YYYY-MM-DD`.
    pub today: &'a str,
}

/// The Capture call's input: `<today>`, `<entry>` (tagged), `<answers>` (`[d1] (about acme.2) question`, then the
/// answer, `(skipped)` or `(doesn't apply)`), `<note>`.
pub fn capture_input(i: &CaptureInput<'_>) -> String {
    let mut s = String::with_capacity(4096);
    let _ = writeln!(s, "<today>{}</today>", i.today);
    s.push_str("<entry>\n");
    write_entry(&mut s, i.entry);
    s.push_str("</entry>\n<answers>\n");
    for a in i.answers {
        let q = &a.question;
        let _ = writeln!(s, "[{}] (about {}) {}", q.id, q.about, q.text.trim());
        match (a.not_applicable, a.said()) {
            (true, _) => s.push_str("(doesn't apply)\n"),
            (false, None) => s.push_str("(skipped)\n"),
            (false, Some(answer)) => {
                let _ = writeln!(s, "{}", clip(answer));
            }
        }
    }
    s.push_str("</answers>\n");
    let _ = writeln!(s, "<note>{}</note>", clip(i.note.trim()));
    s
}

fn clip(text: &str) -> &str {
    match text.char_indices().nth(MAX_ANSWER_CHARS) {
        Some((at, _)) => &text[..at],
        None => text,
    }
}

/// A claim of your answers, in your words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedFact {
    pub text: String,
    pub skills: Vec<String>,
    /// The answers it comes from: question ids, or [`NOTE`].
    pub from: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpKind {
    AddVariant,
    AddBullet,
    TagSkills,
    AddStory,
}

/// An addition as written: which fields count depends on `op`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Op {
    pub op: OpKind,
    /// `add_variant`, `tag_skills`: the bullet.
    pub bullet: String,
    /// `add_variant`, `add_bullet`: the line.
    pub text: String,
    /// `tag_skills`: the skills to add.
    pub skills: Vec<String>,
    /// `add_story`: the story; its facts name new ones by place (`new:1`).
    pub story: Option<StoryDraft>,
    pub why: String,
}

/// Something Capture couldn't use, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skip {
    pub what: String,
    pub why: String,
}

/// What your answers became, as written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capture {
    pub reply: String,
    pub facts: Vec<CapturedFact>,
    pub ops: Vec<Op>,
    pub follow_ups: Vec<Question>,
    pub skipped: Vec<Skip>,
}

/// A checked addition, its links resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Checked {
    AddVariant {
        bullet: String,
        text: String,
        why: String,
    },
    AddBullet {
        text: String,
        why: String,
    },
    TagSkills {
        bullet: String,
        skills: Vec<String>,
        why: String,
    },
    /// The story with this entry, its bullets and its facts (new ones by their saved ids).
    AddStory { story: StoryDraft, why: String },
}

/// The facts of `facts` your answers back, in order (at most [`MAX_ITEMS`]), text and skills tidied, each with its
/// place in `facts`; one stating a number the answers it cites don't state goes to the skipped list instead.
pub fn facts_checked(
    facts: Vec<CapturedFact>,
    answers: &[Answered],
    note: &str,
) -> (Vec<(usize, CapturedFact)>, Vec<Skip>) {
    let mut kept = Vec::new();
    let mut skipped = Vec::new();
    for (i, f) in facts.into_iter().enumerate() {
        let text = tidy(&f.text);
        if text.is_empty() || kept.len() == MAX_ITEMS {
            continue;
        }
        let cited: Vec<&str> = f
            .from
            .iter()
            .filter_map(|id| {
                let id = id.trim();
                if id == NOTE {
                    Some(note)
                } else {
                    answers
                        .iter()
                        .find(|a| a.question.id == id)
                        .and_then(Answered::said)
                }
            })
            .collect();
        let missing = unsaid(&text, &cited);
        if !missing.is_empty() {
            skipped.push(Skip {
                what: text,
                why: not_said(&missing),
            });
            continue;
        }
        kept.push((
            i,
            CapturedFact {
                text,
                skills: tidy_list(&f.skills),
                from: f.from.iter().map(|s| s.trim().to_owned()).collect(),
            },
        ));
    }
    (kept, skipped)
}

/// `ops` held to entry `entry` of `bank` (with the kept facts saved) and your answers: each op's words may state only
/// numbers the entry's lines, its facts or the answers state, and only names the bank holds or the answers say; a
/// variant or new bullet that reads like a wording the entry has is dropped, as are skills a bullet lists already; a
/// story keeps only this entry's bullets and facts, `new:N` naming the `N`th fact Capture wrote (`new_facts[N - 1]`:
/// its saved id, `None` when it wasn't kept). Whatever doesn't hold goes on the skipped list with why; at most
/// [`MAX_ITEMS`] ops are kept.
pub fn ops_checked(
    ops: Vec<Op>,
    bank: &Bank,
    entry: &str,
    new_facts: &[Option<String>],
    answers: &[Answered],
    note: &str,
) -> (Vec<Checked>, Vec<Skip>) {
    let mut kept = Vec::new();
    let mut skipped = Vec::new();
    let Some(e) = bank
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .find(|x| x.id == entry)
    else {
        return (kept, skipped);
    };
    let said: Vec<&str> = answers
        .iter()
        .filter_map(Answered::said)
        .chain(Some(note.trim()).filter(|n| !n.is_empty()))
        .collect();
    let vouched: Vec<String> = said.iter().map(|s| (*s).to_owned()).collect();
    let grounding = Grounding::of_bank(bank).confirmed(&vouched);
    let mut sources: Vec<&str> = said.clone();
    sources.extend([e.name.as_str(), e.subtitle.as_str(), e.dates.as_str()]);
    sources.extend(e.notes.iter().map(|n| n.text.as_str()));
    for b in &e.bullets {
        sources.push(&b.text);
        sources.extend(b.variants.iter().map(String::as_str));
    }
    sources.extend(e.facts.iter().map(|f| f.text.as_str()));
    let wordings = || {
        e.bullets
            .iter()
            .flat_map(|b| std::iter::once(&b.text).chain(&b.variants))
    };
    for op in ops {
        if kept.len() == MAX_ITEMS {
            break;
        }
        let why = tidy(&op.why);
        let mut skip = |what: String, reason: String| skipped.push(Skip { what, why: reason });
        match op.op {
            OpKind::AddVariant | OpKind::AddBullet => {
                let text = tidy(&op.text);
                let bullet = op.bullet.trim();
                let label = if op.op == OpKind::AddVariant {
                    match about(e, bullet) {
                        (_, l) if l == "The entry" => {
                            skip(text, format!("Bullet `{bullet}` isn't in this entry"));
                            continue;
                        }
                        (_, l) => format!("A wording of {}", l.to_lowercase()),
                    }
                } else {
                    "A new bullet".to_owned()
                };
                if text.is_empty() {
                    continue;
                }
                if wordings().any(|w| same_text(w, &text)) {
                    skip(label, "The entry already reads that way".to_owned());
                    continue;
                }
                let findings = grounding.check(&text, &sources);
                if !findings.is_empty() {
                    skip(text, findings.join("; "));
                    continue;
                }
                kept.push(if op.op == OpKind::AddVariant {
                    Checked::AddVariant {
                        bullet: bullet.to_owned(),
                        text,
                        why,
                    }
                } else {
                    Checked::AddBullet { text, why }
                });
            }
            OpKind::TagSkills => {
                let bullet = op.bullet.trim();
                let Some(b) = e.bullets.iter().find(|b| b.id == bullet) else {
                    skip(
                        op.skills.join(", "),
                        format!("Bullet `{bullet}` isn't in this entry"),
                    );
                    continue;
                };
                let mut skills = Vec::new();
                for s in tidy_list(&op.skills) {
                    if b.skills.iter().any(|x| x.eq_ignore_ascii_case(&s)) {
                        continue;
                    }
                    let findings = grounding.check_item(&s);
                    if findings.is_empty() {
                        skills.push(s);
                    } else {
                        skip(s, findings.join("; "));
                    }
                }
                if !skills.is_empty() {
                    kept.push(Checked::TagSkills {
                        bullet: bullet.to_owned(),
                        skills,
                        why,
                    });
                }
            }
            OpKind::AddStory => {
                let Some(s) = op.story else { continue };
                let story = story_linked(s, e, new_facts);
                if [&story.situation, &story.task, &story.action, &story.result]
                    .iter()
                    .all(|p| p.is_empty())
                {
                    continue;
                }
                let mut grounds: Vec<&str> = said.clone();
                for id in &story.bullets {
                    if let Some(b) = e.bullets.iter().find(|b| b.id == *id) {
                        grounds.push(&b.text);
                        grounds.extend(b.variants.iter().map(String::as_str));
                    }
                }
                grounds.extend(
                    e.facts
                        .iter()
                        .filter(|f| story.facts.contains(&f.id))
                        .map(|f| f.text.as_str()),
                );
                let mut findings: Vec<String> = Vec::new();
                for part in [
                    &story.title,
                    &story.situation,
                    &story.task,
                    &story.action,
                    &story.result,
                ] {
                    for f in grounding.check(part, &grounds) {
                        if !findings.contains(&f) {
                            findings.push(f);
                        }
                    }
                }
                if findings.is_empty() {
                    kept.push(Checked::AddStory { story, why });
                } else {
                    skip(story.title, findings.join("; "));
                }
            }
        }
    }
    (kept, skipped)
}

/// `s` on entry `e`: its bullets only the entry's, its facts the entry's or the new ones it names by place, every part
/// tidied.
fn story_linked(s: StoryDraft, e: &BankEntry, new_facts: &[Option<String>]) -> StoryDraft {
    let mut bullets: Vec<String> = Vec::new();
    for b in s.bullets {
        let b = b.trim();
        if e.bullets.iter().any(|x| x.id == b) && !bullets.iter().any(|x| x == b) {
            bullets.push(b.to_owned());
        }
    }
    let mut facts: Vec<String> = Vec::new();
    for f in s.facts {
        let f = f.trim();
        let id = match f.strip_prefix("new:").and_then(|n| n.parse::<usize>().ok()) {
            Some(n) => n
                .checked_sub(1)
                .and_then(|i| new_facts.get(i))
                .cloned()
                .flatten(),
            None => e.facts.iter().any(|x| x.id == f).then(|| f.to_owned()),
        };
        if let Some(id) = id
            && !facts.contains(&id)
        {
            facts.push(id);
        }
    }
    let mut themes: Vec<StoryTheme> = Vec::new();
    for t in s.themes {
        if !themes.contains(&t) {
            themes.push(t);
        }
    }
    StoryDraft {
        title: tidy(&s.title),
        entry: Some(e.id.clone()),
        bullets,
        facts,
        situation: tidy(&s.situation),
        task: tidy(&s.task),
        action: tidy(&s.action),
        result: tidy(&s.result),
        skills: tidy_list(&s.skills),
        themes,
    }
}

/// Follow-ups held to the entry as questions are: numbered `f1`…, at most [`MAX_FOLLOW_UPS`].
pub fn follow_ups_checked(follow_ups: Vec<Question>, entry: &BankEntry) -> Vec<Question> {
    numbered(follow_ups, entry, "f", MAX_FOLLOW_UPS)
}

/// The numbers `text` states (as written) that none of `sources` states.
pub fn unsaid(text: &str, sources: &[&str]) -> Vec<String> {
    let known: Vec<String> = sources
        .iter()
        .flat_map(|s| numbers(s))
        .map(|n| n.norm)
        .collect();
    let mut out: Vec<String> = Vec::new();
    for n in numbers(text) {
        if !known.contains(&n.norm) && !out.contains(&n.shown) {
            out.push(n.shown);
        }
    }
    out
}

/// "Your answers don't say “40%”" (or "… “40%” or “3”").
fn not_said(numbers: &[String]) -> String {
    let quoted: Vec<String> = numbers.iter().map(|n| format!("“{n}”")).collect();
    format!("Your answers don't say {}", quoted.join(" or "))
}

fn tidy(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Each tidied, empty ones dropped, each once ignoring case.
fn tidy_list(items: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in items.iter().map(|s| tidy(s)) {
        if !s.is_empty() && !out.iter().any(|x| x.eq_ignore_ascii_case(&s)) {
            out.push(s);
        }
    }
    out
}

/// Runs Capture with `model`: the reply as written (unchecked: see [`facts_checked`], [`ops_checked`],
/// [`follow_ups_checked`]).
pub async fn capture(
    llm: &impl Llm,
    schema: &Schema,
    input: &CaptureInput<'_>,
    model: &str,
) -> Result<(Capture, Structured), LlmError> {
    let text = capture_input(input);
    let output = llm
        .structured(
            Call {
                system: CAPTURE_PROMPT,
                input: &text,
                model: Some(model),
            },
            schema,
        )
        .await?;
    match Capture::deserialize(&output.value) {
        Ok(c) => Ok((c, output)),
        Err(e) => Err(LlmError::Decode {
            target: "Capture",
            message: e.to_string(),
            output,
        }),
    }
}

#[cfg(test)]
mod tests {
    use resume_core::bank::{BankEntry, Fact, FactKind, FactSource};
    use resume_core::story::StoryTheme;

    use super::*;
    use crate::prep::tests::jane;

    fn acme(bank: &Bank) -> &BankEntry {
        bank.sections
            .iter()
            .flat_map(|s| &s.entries)
            .find(|e| e.id == "acme")
            .unwrap()
    }

    fn q(id: &str, about: &str) -> Question {
        Question {
            id: id.into(),
            about: about.into(),
            about_label: String::new(),
            text: "How many?".into(),
            why: "A number".into(),
        }
    }

    fn answers() -> Vec<Answered> {
        vec![
            Answered {
                question: q("d1", "acme.2"),
                answer: Some("About 20 regressions, mostly in checkout. We used Kafka for events.".into()),
                not_applicable: false,
            },
            Answered {
                question: q("d2", "acme.1"),
                answer: None,
                not_applicable: false,
            },
            Answered {
                question: q("d3", "acme"),
                answer: Some("It helped a lot.".into()),
                not_applicable: true,
            },
        ]
    }

    #[test]
    fn schemas_compile() {
        questions_schema();
        capture_schema();
    }

    #[test]
    fn questions_input_carries_the_entry_masters_and_terms() {
        let (bank, _) = jane();
        let terms = vec![("Docker".to_owned(), 3), ("Kafka".to_owned(), 1)];
        let text = questions_input(&QuestionsInput {
            entry: acme(&bank),
            masters: &["SWE".to_owned(), "IT".to_owned()],
            terms: &terms,
            today: "2026-10-27",
        });
        assert!(text.starts_with("<today>2026-10-27</today>\n<entry>\n[acme] Acme Widgets Co."));
        assert!(text.contains("- [acme.2] Wrote integration tests"));
        assert!(text.contains("[acme~1] (fact) The test suite ran on 3 services\n  (skills) Jest\n</entry>\n"));
        assert!(text.ends_with(
            "<masters>SWE, IT</masters>\n<terms>\n- Docker (asked in 3 postings)\n- Kafka (asked in 1 posting)\n</terms>\n"
        ));
        let text = questions_input(&QuestionsInput {
            entry: acme(&bank),
            masters: &[],
            terms: &[],
            today: "2026-10-27",
        });
        assert!(text.ends_with("<masters>none</masters>\n<terms>\n</terms>\n"));
    }

    #[test]
    fn questions_are_numbered_and_labelled_by_their_bullet() {
        let (bank, _) = jane();
        let qs = Questions {
            read: " 3 bullets. ".into(),
            questions: vec![
                q("x", "acme.2"),
                q("x", "acme"),
                q("x", "acme~1"),
                Question {
                    text: " ".into(),
                    ..q("x", "acme.1")
                },
                q("x", "acme.1"),
                q("x", "acme.3"),
                q("x", "acme.3"),
                q("x", "acme.3"),
            ],
        };
        let qs = questions_checked(qs, acme(&bank));
        assert_eq!(qs.read, "3 bullets.");
        assert_eq!(qs.questions.len(), MAX_QUESTIONS);
        let got: Vec<(&str, &str, &str)> = qs
            .questions
            .iter()
            .map(|q| (q.id.as_str(), q.about.as_str(), q.about_label.as_str()))
            .collect();
        assert_eq!(
            got[..4],
            [
                ("d1", "acme.2", "Bullet 2"),
                ("d2", "acme", "The entry"),
                ("d3", "acme", "The entry"),
                ("d4", "acme.1", "Bullet 1"),
            ]
        );
    }

    #[test]
    fn capture_input_marks_skipped_and_not_applicable() {
        let (bank, _) = jane();
        let answers = answers();
        let text = capture_input(&CaptureInput {
            entry: acme(&bank),
            answers: &answers,
            note: " I also ran on-call. ",
            today: "2026-10-27",
        });
        assert!(text.contains(
            "<answers>\n[d1] (about acme.2) How many?\nAbout 20 regressions, mostly in checkout. We used Kafka for \
             events.\n[d2] (about acme.1) How many?\n(skipped)\n[d3] (about acme) How many?\n(doesn't apply)\n\
             </answers>\n<note>I also ran on-call.</note>\n"
        ));
    }

    #[test]
    fn a_fact_needs_its_numbers_in_the_answers_it_cites() {
        let fact = |text: &str, from: &[&str]| CapturedFact {
            text: text.into(),
            skills: vec!["Integration testing".into(), " integration  testing".into()],
            from: from.iter().map(|f| (*f).to_owned()).collect(),
        };
        let (kept, skipped) = facts_checked(
            vec![
                fact("The tests caught about 20 regressions", &["d1"]),
                fact("Ran 3 on-call rotations", &["note"]),
                fact("Cut checkout bugs by 40%", &["d1"]),
                fact("Used  Kafka for events", &["d1"]),
                fact("Ran 3 on-call rotations", &["d3"]),
            ],
            &answers(),
            "I ran 3 on-call rotations.",
        );
        let got: Vec<(usize, &str)> = kept.iter().map(|(i, f)| (*i, f.text.as_str())).collect();
        assert_eq!(
            got,
            [
                (0, "The tests caught about 20 regressions"),
                (1, "Ran 3 on-call rotations"),
                (3, "Used Kafka for events"),
            ]
        );
        assert_eq!(kept[0].1.skills, ["Integration testing"]);
        // 40% isn't said; a doesn't-apply answer grounds nothing.
        assert_eq!(
            skipped,
            [
                Skip {
                    what: "Cut checkout bugs by 40%".into(),
                    why: "Your answers don't say “40%”".into()
                },
                Skip {
                    what: "Ran 3 on-call rotations".into(),
                    why: "Your answers don't say “3”".into()
                },
            ]
        );
    }

    fn op(kind: OpKind, bullet: &str, text: &str) -> Op {
        Op {
            op: kind,
            bullet: bullet.into(),
            text: text.into(),
            skills: vec![],
            story: None,
            why: " Adds the number ".into(),
        }
    }

    /// Jane's bank with the kept fact saved as `acme~2`.
    fn saved() -> Bank {
        let (mut bank, _) = jane();
        let acme = bank
            .sections
            .iter_mut()
            .flat_map(|s| &mut s.entries)
            .find(|e| e.id == "acme")
            .unwrap();
        acme.facts.push(Fact {
            id: "acme~2".into(),
            text: "The tests caught about 20 regressions".into(),
            skills: vec![],
            source: FactSource {
                kind: FactKind::Dig,
                at: "2026-10-27T10:00:00Z".into(),
                posting: None,
                company: None,
            },
        });
        bank
    }

    #[test]
    fn ops_hold_to_the_entry_and_the_answers() {
        let bank = saved();
        let ops = vec![
            op(
                OpKind::AddVariant,
                "acme.2",
                "Wrote integration tests in GitHub Actions that caught 20 regressions before release",
            ),
            op(OpKind::AddVariant, "acme.9", "Wrote tests"),
            op(OpKind::AddVariant, "acme.2", "Cut checkout bugs by 40%"),
            op(OpKind::AddBullet, "", "Published order events to Kafka"),
            op(OpKind::AddBullet, "", "Deployed the API on Kubernetes"),
            // The entry already reads this way.
            op(
                OpKind::AddBullet,
                "",
                "Wrote integration tests in GitHub Actions for the order API, catching 14 regressions before release",
            ),
            Op {
                skills: vec!["Kafka".into(), "Terraform".into()],
                ..op(OpKind::TagSkills, "acme.2", "")
            },
            Op {
                story: Some(StoryDraft {
                    title: "Caught 20 regressions before release".into(),
                    entry: None,
                    bullets: vec!["acme.2".into(), "library.1".into()],
                    facts: vec!["new:1".into(), "new:2".into(), "acme~1".into(), "acme~9".into()],
                    situation: "Checkout kept breaking.".into(),
                    task: "I owned the tests.".into(),
                    action: "I wrote integration tests across 3 services.".into(),
                    result: "They caught about 20 regressions.".into(),
                    skills: vec![],
                    themes: vec![StoryTheme::Impact],
                }),
                ..op(OpKind::AddStory, "", "")
            },
        ];
        let answers = answers();
        let new_facts = [Some("acme~2".to_owned()), None];
        let (kept, skipped) = ops_checked(ops, &bank, "acme", &new_facts, &answers, "");
        assert_eq!(kept.len(), 4, "{kept:#?}\n{skipped:#?}");
        assert!(matches!(&kept[0], Checked::AddVariant { bullet, why, .. } if bullet == "acme.2" && why == "Adds the number"));
        assert!(matches!(&kept[1], Checked::AddBullet { text, .. } if text.contains("Kafka")));
        assert!(matches!(&kept[2], Checked::TagSkills { skills, .. } if skills == &["Kafka"]));
        let Checked::AddStory { story, .. } = &kept[3] else {
            panic!("{:?}", kept[3]);
        };
        assert_eq!(story.entry.as_deref(), Some("acme"));
        assert_eq!(story.bullets, ["acme.2"]);
        assert_eq!(story.facts, ["acme~2", "acme~1"]);
        let whats: Vec<&str> = skipped.iter().map(|s| s.what.as_str()).collect();
        assert_eq!(
            whats,
            [
                "Wrote tests",
                "Cut checkout bugs by 40%",
                "Deployed the API on Kubernetes",
                "A new bullet",
                "Terraform",
            ]
        );
        assert!(skipped[2].why.contains("Kubernetes"));
    }

    #[test]
    fn a_story_with_an_unsourced_number_is_skipped() {
        let bank = saved();
        let ops = vec![Op {
            story: Some(StoryDraft {
                title: "Cut pages".into(),
                result: "Pages fell by 60%.".into(),
                ..StoryDraft::default()
            }),
            ..op(OpKind::AddStory, "", "")
        }];
        let (kept, skipped) = ops_checked(ops, &bank, "acme", &[], &answers(), "");
        assert!(kept.is_empty());
        assert_eq!(skipped[0].what, "Cut pages");
        assert!(skipped[0].why.contains("60%"));
    }

    #[test]
    fn follow_ups_are_numbered_apart() {
        let (bank, _) = jane();
        let f = follow_ups_checked(vec![q("x", "acme.3"); 5], acme(&bank));
        assert_eq!(f.len(), MAX_FOLLOW_UPS);
        assert_eq!((f[0].id.as_str(), f[0].about_label.as_str()), ("f1", "Bullet 3"));
    }
}
