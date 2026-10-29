//! Debrief: what you wrote down after a round (the questions they asked, what you got stuck on, your notes) read for
//! what to dig into in your bank and what the next round's prep should know (ARCHITECTURE.md §3). Never graded.
//!
//! One call. It sees the round's kind, the job, the posting's requirement skills, the resume as sent (tagged, as Prep
//! reads it), every bank entry tagged with its facts, and what you wrote. It writes, per topic you got stuck on that
//! some entry plausibly touched, 2–3 Dig questions about that entry, and 1–3 notes for the next prep. [`checked`]
//! holds the reply to what the bank and your words back. [`Earlier`] is how a later round's Prep reads a debrief.

use std::fmt::Write as _;

use resume_core::bank::{Bank, BankEntry};
use resume_core::validate::{Grounding, same_text};
use resume_core::{Requirements, matcher};
use serde::{Deserialize, Serialize};

use crate::dig::{Question, about};
use crate::prep::{SentResume, write_requirements};
use crate::suggest::write_entry;
use crate::{Call, Llm, LlmError, Schema, Structured};

/// The Debrief instructions (`prompts/debrief.md`).
pub const PROMPT: &str = include_str!("../../../prompts/debrief.md");
/// The reply's schema (`prompts/debrief.schema.json`): see [`Reply`].
pub const SCHEMA: &str = include_str!("../../../prompts/debrief.schema.json");
/// The most questions one dig keeps.
pub const MAX_DIG_QUESTIONS: usize = 3;
/// The most prep notes a read keeps.
pub const MAX_NOTES: usize = 3;
/// The most of your notes the model reads.
pub const MAX_NOTES_CHARS: usize = 2_000;

pub fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("prompts/debrief.schema.json is a valid schema")
}

/// What you wrote down after the round.
#[derive(Clone, Copy, Debug)]
pub struct Said<'a> {
    /// The questions they asked, as you recorded them (from the prep, or typed).
    pub asked: &'a [String],
    /// What you got stuck on, a few words each.
    pub stuck_on: &'a [String],
    pub notes: &'a str,
}

impl Said<'_> {
    /// Every piece of it, notes clipped as the model reads them.
    fn texts(&self) -> Vec<&str> {
        self.asked
            .iter()
            .chain(self.stuck_on)
            .map(String::as_str)
            .chain(Some(clip(self.notes.trim())).filter(|n| !n.is_empty()))
            .collect()
    }
}

/// What Debrief reads.
#[derive(Clone, Copy, Debug)]
pub struct DebriefInput<'a> {
    /// The round's kind as the app names it (`technical`, `recruiter-screen`, …).
    pub kind: &'a str,
    pub role: &'a str,
    pub company: &'a str,
    /// The posting's requirements; `None` when the application has no posting read.
    pub requirements: Option<&'a Requirements>,
    pub sent: &'a SentResume,
    pub bank: &'a Bank,
    pub said: Said<'a>,
}

/// The Debrief call's input: `<round>`, `<job>`, `<requirements>`, `<resume>` (tagged; `ids="none"` for the PDF's
/// text), `<bank>` (every entry, tagged, with its facts), `<asked>` and `<stuck_on>` (one a line), `<notes>`.
pub fn input(i: &DebriefInput<'_>) -> String {
    let mut s = String::with_capacity(i.sent.text.len() + 8192);
    let _ = writeln!(s, "<round>\nkind: {}\n</round>", i.kind);
    let _ = writeln!(s, "<job>{} at {}</job>", i.role.trim(), i.company.trim());
    s.push_str("<requirements>\n");
    if let Some(req) = i.requirements {
        write_requirements(&mut s, req);
    }
    s.push_str("</requirements>\n");
    if i.sent.lines.is_empty() {
        s.push_str("<resume ids=\"none\">\n");
    } else {
        s.push_str("<resume>\n");
    }
    s.push_str(i.sent.text.trim());
    s.push_str("\n</resume>\n<bank>\n");
    for e in entries(i.bank) {
        write_entry(&mut s, e);
    }
    s.push_str("</bank>\n<asked>\n");
    for a in i.said.asked {
        let _ = writeln!(s, "- {}", tidy(a));
    }
    s.push_str("</asked>\n<stuck_on>\n");
    for t in i.said.stuck_on {
        let _ = writeln!(s, "- {}", tidy(t));
    }
    let _ = writeln!(
        s,
        "</stuck_on>\n<notes>{}</notes>",
        clip(i.said.notes.trim())
    );
    s
}

fn entries(bank: &Bank) -> impl Iterator<Item = &BankEntry> {
    bank.sections.iter().flat_map(|s| &s.entries)
}

/// `text` cut to [`MAX_NOTES_CHARS`] at a character boundary.
fn clip(text: &str) -> &str {
    match text.char_indices().nth(MAX_NOTES_CHARS) {
        Some((i, _)) => &text[..i],
        None => text,
    }
}

/// One dig as written: the topic you got stuck on, the entry it touched, questions about that entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DigReply {
    pub stuck_on: String,
    pub entry: String,
    pub questions: Vec<Question>,
}

/// The reply as written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub digs: Vec<DigReply>,
    pub prep_notes: Vec<String>,
}

/// Dig questions about one entry, from a topic you got stuck on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dig {
    /// The topic exactly as your debrief lists it.
    pub topic: String,
    pub entry: String,
    pub entry_name: String,
    /// Numbered `b1`… across the read, each labelled by what it's about.
    pub questions: Vec<Question>,
}

/// A debrief read, checked.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Read {
    pub digs: Vec<Dig>,
    pub prep_notes: Vec<String>,
}

/// What [`checked`] took out and why, for the log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dropped(pub Vec<(String, String)>);

impl Dropped {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn push(&mut self, what: impl Into<String>, why: impl Into<String>) {
        self.0.push((what.into(), why.into()));
    }
}

/// `reply` held to the bank and what you said:
/// - a dig only for a topic your debrief lists (named as you wrote it; one dig a topic) and an entry the bank has;
/// - its questions about one of that entry's bullets or the entry itself (labelled by the bullet's place), at most
///   [`MAX_DIG_QUESTIONS`], none repeated, each stating only numbers the entry or your words state and only names the
///   bank, your words, the job or its requirement skills hold, and none grading; a dig left with none is dropped;
/// - questions numbered `b1`… across the read;
/// - prep notes stating only numbers your words state and names those sources hold, none grading, at most
///   [`MAX_NOTES`].
pub fn checked(
    reply: Reply,
    bank: &Bank,
    said: &Said<'_>,
    job: &[&str],
    req: Option<&Requirements>,
) -> (Read, Dropped) {
    let mut dropped = Dropped::default();
    let words = said.texts();
    let mut vouched: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
    vouched.extend(job.iter().map(|j| (*j).to_owned()));
    if let Some(r) = req {
        for k in &r.skills {
            vouched.push(k.term.clone());
            vouched.extend(matcher::all_aliases(&k.term, &k.aliases));
        }
    }
    let grounding = Grounding::of_bank(bank).confirmed(&vouched);

    let mut digs: Vec<Dig> = Vec::new();
    let mut n = 0;
    for d in reply.digs {
        let named = tidy(&d.stuck_on);
        let Some(topic) = said
            .stuck_on
            .iter()
            .map(|t| tidy(t))
            .find(|t| t.eq_ignore_ascii_case(&named))
        else {
            dropped.push(named, "not a topic the debrief lists");
            continue;
        };
        if digs.iter().any(|x| x.topic == topic) {
            dropped.push(topic, "a second dig for the topic");
            continue;
        }
        let Some(e) = entries(bank).find(|e| e.id == d.entry.trim()) else {
            dropped.push(topic, format!("no entry `{}`", d.entry.trim()));
            continue;
        };
        let mut sources: Vec<&str> = words.clone();
        sources.extend([e.name.as_str(), e.subtitle.as_str(), e.dates.as_str()]);
        sources.extend(e.notes.iter().map(|n| n.text.as_str()));
        for b in &e.bullets {
            sources.push(&b.text);
            sources.extend(b.variants.iter().map(String::as_str));
        }
        sources.extend(e.facts.iter().map(|f| f.text.as_str()));
        let mut questions: Vec<Question> = Vec::new();
        for q in d.questions {
            let text = tidy(&q.text);
            if text.is_empty() || questions.iter().any(|x| same_text(&x.text, &text)) {
                continue;
            }
            if questions.len() == MAX_DIG_QUESTIONS {
                break;
            }
            let mut findings = grounding.check(&text, &sources);
            findings.extend(grades(&text));
            if !findings.is_empty() {
                dropped.push(text, findings.join("; "));
                continue;
            }
            let (about_id, about_label) = about(e, q.about.trim());
            n += 1;
            questions.push(Question {
                id: format!("b{n}"),
                about: about_id,
                about_label,
                text,
                why: tidy(&q.why),
            });
        }
        if questions.is_empty() {
            dropped.push(topic, "no question held");
            continue;
        }
        digs.push(Dig {
            topic,
            entry: e.id.clone(),
            entry_name: e.name.clone(),
            questions,
        });
    }

    let mut prep_notes: Vec<String> = Vec::new();
    for note in reply.prep_notes {
        let note = tidy(&note);
        if note.is_empty() || prep_notes.iter().any(|x| same_text(x, &note)) {
            continue;
        }
        if prep_notes.len() == MAX_NOTES {
            break;
        }
        let mut findings = grounding.check(&note, &words);
        findings.extend(grades(&note));
        if findings.is_empty() {
            prep_notes.push(note);
        } else {
            dropped.push(note, findings.join("; "));
        }
    }
    (Read { digs, prep_notes }, dropped)
}

/// Words and phrases that grade the round or the candidate.
const GRADING: &[&str] = &[
    "score",
    "scored",
    "scores",
    "grade",
    "graded",
    "rating",
    "rated",
    "nailed",
    "impressive",
    "excellent",
    "poorly",
    "out of 10",
    "/10",
    "well done",
    "great job",
    "good job",
    "did well",
    "did great",
    "did poorly",
    "went well",
    "went badly",
    "went poorly",
    "strong answer",
    "weak answer",
    "good answer",
    "great answer",
    "bad answer",
];

/// A finding when `text` grades the round or you ("You did well", "7/10"), else none.
pub fn grades(text: &str) -> Option<String> {
    let lower = format!(
        " {} ",
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && c != '/')
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    );
    GRADING
        .iter()
        .find(|g| {
            if g.starts_with('/') {
                lower.contains(*g)
            } else {
                lower.contains(&format!(" {g} "))
            }
        })
        .map(|g| format!("grades the round (“{g}”)"))
}

fn tidy(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Runs Debrief with `model`: the reply as written (unchecked: see [`checked`]).
pub async fn read(
    llm: &impl Llm,
    schema: &Schema,
    input: &DebriefInput<'_>,
    model: &str,
) -> Result<(Reply, Structured), LlmError> {
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
        Ok(r) => Ok((r, output)),
        Err(e) => Err(LlmError::Decode {
            target: "Debrief",
            message: e.to_string(),
            output,
        }),
    }
}

// ── For a later round's prep ────────────────────────────────────────────────────────────────────────────────────────

/// An earlier round's debrief as a later round's Prep reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Earlier {
    /// The round's kind as the app names it.
    pub kind: String,
    /// When it was, `YYYY-MM-DD`; empty when it wasn't scheduled.
    pub date: String,
    pub asked: Vec<String>,
    pub stuck_on: Vec<String>,
    pub notes: String,
    /// What reading it found for the next prep (empty until it's read).
    pub prep_notes: Vec<String>,
}

/// `## technical, 2026-10-24`, then `asked:` (one a line), `stuck on:`, `notes:` and `for this prep:` (one a line),
/// each only when it has something, per debrief.
pub fn write_earlier(s: &mut String, earlier: &[Earlier]) {
    for e in earlier {
        if e.date.is_empty() {
            let _ = writeln!(s, "## {}", e.kind);
        } else {
            let _ = writeln!(s, "## {}, {}", e.kind, e.date);
        }
        if !e.asked.is_empty() {
            s.push_str("asked:\n");
            for a in &e.asked {
                let _ = writeln!(s, "- {}", tidy(a));
            }
        }
        if !e.stuck_on.is_empty() {
            let _ = writeln!(s, "stuck on: {}", e.stuck_on.join("; "));
        }
        let notes = clip(e.notes.trim());
        if !notes.is_empty() {
            let _ = writeln!(s, "notes: {}", tidy(notes));
        }
        if !e.prep_notes.is_empty() {
            s.push_str("for this prep:\n");
            for n in &e.prep_notes {
                let _ = writeln!(s, "- {n}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prep::tests::{jane, requirements};

    fn q(about: &str, text: &str) -> Question {
        Question {
            id: String::new(),
            about: about.into(),
            about_label: String::new(),
            text: text.into(),
            why: " They pressed on it ".into(),
        }
    }

    fn said<'a>(asked: &'a [String], stuck: &'a [String], notes: &'a str) -> Said<'a> {
        Said {
            asked,
            stuck_on: stuck,
            notes,
        }
    }

    #[test]
    fn schema_compiles() {
        super::schema();
    }

    #[test]
    fn input_carries_every_part() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let req = requirements();
        let asked = vec!["How would you deploy a  Go service?".to_owned()];
        let stuck = vec!["Kubernetes deployments".to_owned()];
        let text = input(&DebriefInput {
            kind: "technical",
            role: "Software Engineer",
            company: "Acme",
            requirements: Some(&req),
            sent: &sent,
            bank: &bank,
            said: said(&asked, &stuck, " They want the final by Friday. "),
        });
        assert!(text.starts_with(
            "<round>\nkind: technical\n</round>\n<job>Software Engineer at Acme</job>\n<requirements>\n\
             - Kubernetes (tool; required)\n"
        ));
        assert!(text.contains("<resume>\n"));
        assert!(text.contains("<bank>\n[acme] "));
        assert!(text.contains("[acme~1] (fact) The test suite ran on 3 services\n"));
        assert!(text.ends_with(
            "</bank>\n<asked>\n- How would you deploy a Go service?\n</asked>\n<stuck_on>\n\
             - Kubernetes deployments\n</stuck_on>\n<notes>They want the final by Friday.</notes>\n"
        ));
    }

    #[test]
    fn digs_hold_to_the_topics_the_entries_and_your_words() {
        let (bank, _) = jane();
        let acme = entries(&bank).find(|e| e.id == "acme").unwrap();
        let second = acme.bullets[1].id.clone();
        let asked = vec!["How do your tests run in CI?".to_owned()];
        let stuck = vec![
            "Kubernetes deployments".to_owned(),
            "Table sizing".to_owned(),
        ];
        let s = said(&asked, &stuck, "They run 40 services.");
        let reply = Reply {
            digs: vec![
                DigReply {
                    stuck_on: " kubernetes  DEPLOYMENTS ".into(),
                    entry: "acme".into(),
                    questions: vec![
                        q(&second, "Did all 3 services deploy the same way?"),
                        q("acme", "Did the test suite run against all 40 services?"),
                        q("acme", "Did you run 12 clusters there?"),
                        q("nope", "Did you deploy anything to Kubernetes there?"),
                        q("acme", "Did you deploy anything to Kubernetes there?"),
                        q("acme", "Did you set up Helm charts for it?"),
                        q("acme", "What did you change in the deploy scripts?"),
                    ],
                },
                // A second dig for the same topic, a topic the debrief doesn't list, an entry the bank lacks.
                DigReply {
                    stuck_on: "Kubernetes deployments".into(),
                    entry: "acme".into(),
                    questions: vec![q("acme", "Anything else?")],
                },
                DigReply {
                    stuck_on: "Kafka".into(),
                    entry: "acme".into(),
                    questions: vec![q("acme", "Did you use Kafka?")],
                },
                DigReply {
                    stuck_on: "Table sizing".into(),
                    entry: "nowhere".into(),
                    questions: vec![q("nowhere", "How big were the tables?")],
                },
            ],
            prep_notes: vec![
                " Kubernetes came up twice. ".into(),
                "Kubernetes came up twice.".into(),
                "They asked 5 questions on deploys.".into(),
                "You did well on the testing questions.".into(),
                "They run 40 services, so expect scale questions.".into(),
                "Expect more on Kubernetes.".into(),
                "A fourth note.".into(),
            ],
        };
        let (read, dropped) = checked(reply, &bank, &s, &["Acme", "Software Engineer"], None);
        assert_eq!(read.digs.len(), 1, "{dropped:?}");
        let d = &read.digs[0];
        assert_eq!(d.topic, "Kubernetes deployments");
        assert_eq!(
            (d.entry.as_str(), d.entry_name.as_str()),
            ("acme", acme.name.as_str())
        );
        assert_eq!(
            d.questions
                .iter()
                .map(|q| (
                    q.id.as_str(),
                    q.about.as_str(),
                    q.about_label.as_str(),
                    q.text.as_str()
                ))
                .collect::<Vec<_>>(),
            [
                (
                    "b1",
                    second.as_str(),
                    "Bullet 2",
                    "Did all 3 services deploy the same way?"
                ),
                (
                    "b2",
                    "acme",
                    "The entry",
                    "Did the test suite run against all 40 services?"
                ),
                // An unknown bullet reads as the entry; the repeat is dropped; the cap is 3.
                (
                    "b3",
                    "acme",
                    "The entry",
                    "Did you deploy anything to Kubernetes there?"
                ),
            ]
        );
        assert_eq!(d.questions[0].why, "They pressed on it");
        let why = |what: &str| {
            dropped
                .0
                .iter()
                .find(|(w, _)| w == what)
                .map(|(_, y)| y.clone())
                .unwrap_or_else(|| panic!("{what} not dropped: {dropped:?}"))
        };
        assert!(why("Did you run 12 clusters there?").contains("12"));
        assert_eq!(why("Kafka"), "not a topic the debrief lists");
        assert_eq!(why("Table sizing"), "no entry `nowhere`");
        assert_eq!(why("Kubernetes deployments"), "a second dig for the topic");
        assert_eq!(
            read.prep_notes,
            [
                "Kubernetes came up twice.",
                "They run 40 services, so expect scale questions.",
                "Expect more on Kubernetes."
            ]
        );
        assert!(why("They asked 5 questions on deploys.").contains('5'));
        assert!(why("You did well on the testing questions.").contains("grades"));
    }

    #[test]
    fn names_the_bank_and_your_words_dont_hold_are_dropped() {
        let (bank, _) = jane();
        let stuck = vec!["Kafka consumer groups".to_owned()];
        let s = said(&[], &stuck, "");
        let reply = Reply {
            digs: vec![DigReply {
                stuck_on: "Kafka consumer groups".into(),
                entry: "acme".into(),
                questions: vec![
                    q("acme", "How did you tune RabbitMQ there?"),
                    q("acme", "Did anything there read from Kafka?"),
                ],
            }],
            prep_notes: vec!["Expect RabbitMQ again.".into()],
        };
        let (read, dropped) = checked(reply, &bank, &s, &[], None);
        assert_eq!(read.digs[0].questions.len(), 1);
        assert_eq!(
            read.digs[0].questions[0].text,
            "Did anything there read from Kafka?"
        );
        assert!(read.prep_notes.is_empty());
        assert_eq!(dropped.0.len(), 2, "{dropped:?}");
    }

    #[test]
    fn grading_is_found_by_word() {
        assert!(grades("I'd give it a 7/10").is_some());
        assert!(grades("That went well overall").is_some());
        assert!(grades("Your score was low").is_some());
        assert!(grades("They asked about the upgrade path and rate limits").is_none());
        assert!(grades("Kubernetes came up twice").is_none());
    }

    #[test]
    fn earlier_debriefs_are_written_for_prep() {
        let mut s = String::new();
        write_earlier(
            &mut s,
            &[
                Earlier {
                    kind: "recruiter-screen".into(),
                    date: "2026-10-24".into(),
                    asked: vec!["Why Globex?".into()],
                    stuck_on: vec!["Kafka".into(), "Kubernetes".into()],
                    notes: " Final is with the manager. ".into(),
                    prep_notes: vec!["Kubernetes came up twice.".into()],
                },
                Earlier {
                    kind: "technical".into(),
                    ..Earlier::default()
                },
            ],
        );
        assert_eq!(
            s,
            "## recruiter-screen, 2026-10-24\nasked:\n- Why Globex?\nstuck on: Kafka; Kubernetes\n\
             notes: Final is with the manager.\nfor this prep:\n- Kubernetes came up twice.\n## technical\n"
        );
    }
}
