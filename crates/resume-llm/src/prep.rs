//! Prep: what one interview round will likely test, the questions it will likely ask, the posting's requirements the
//! resume you sent doesn't show with an honest answer for each, and questions to ask them (ARCHITECTURE.md §3).
//!
//! The model sees the round (its kind, who you meet, its notes), the job, the posting's requirement skills, the resume
//! as sent tagged with its line ids (the summary, note lines, bullets) with each entry's facts from the bank under it,
//! and today's date. Without the sent resume's JSON (exported before the app kept it), it sees the exported PDF's text,
//! which has no ids. [`checked`] holds the reply to what the resume and the bank back: the prompt asks for honesty,
//! the code checks it.

use std::fmt::Write as _;

use resume_core::bank::{BankEntry, SectionKind};
use resume_core::matcher::{Page, all_aliases, find_literal};
use resume_core::resume::Section;
use resume_core::validate::unsourced_numbers;
use resume_core::{Bank, Requirements, Resume};
use serde::{Deserialize, Serialize};

use crate::suggest::write_facts;
use crate::{Call, Llm, LlmError, Schema, Structured};

/// The Prep instructions (`prompts/prep.md`).
pub const PROMPT: &str = include_str!("../../../prompts/prep.md");
/// The reply's schema (`prompts/prep.schema.json`): see [`Prep`].
pub const SCHEMA: &str = include_str!("../../../prompts/prep.schema.json");
/// The most likely questions a prep keeps.
pub const MAX_QUESTIONS: usize = 10;
/// The most gaps a prep keeps.
pub const MAX_GAPS: usize = 6;
/// The most questions to ask them a prep keeps.
pub const MAX_ASK: usize = 5;
/// The id of the summary line.
pub const SUMMARY: &str = "summary";

pub fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("prompts/prep.schema.json is a valid schema")
}

/// One line of the resume as sent: the summary, a note line (`acme#1`) or a bullet (`acme.2`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub id: String,
    /// The entry it sits under (`acme`); the summary's section id for the summary.
    pub entry: String,
    /// That entry's name; empty for the summary.
    pub entry_name: String,
    pub text: String,
}

/// The resume as sent, as Prep and Practice read it and check against.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SentResume {
    /// What the model reads: the resume tagged with line ids, each entry followed by its facts; or the PDF's text.
    pub text: String,
    /// Its lines; empty for the PDF.
    pub lines: Vec<Line>,
    /// What the page prints, without ids or facts: where a requirement term "shows".
    pub printed: String,
    /// The bank entries the resume shows, `(id, name)`, in page order.
    pub entries: Vec<(String, String)>,
    /// Their facts, `(entry id, fact id, text)`.
    pub facts: Vec<(String, String, String)>,
}

impl SentResume {
    /// The resume as exported, with its entries' facts from `bank`.
    pub fn tagged(resume: &Resume, bank: &Bank) -> Self {
        let lines = sent_lines(resume);
        let mut text = String::new();
        let mut printed = String::new();
        let mut entries = Vec::new();
        let mut facts = Vec::new();
        text.push_str(&resume.name);
        text.push('\n');
        for section in &resume.sections {
            let _ = writeln!(text, "\n## {}", section.title);
            let _ = writeln!(printed, "{}", section.title);
            let summary = summary_entry(section);
            for (i, e) in section.entries.iter().enumerate() {
                let bank_entry = find_entry(bank, &e.id);
                if summary.is_some() && i == 0 {
                    // The summary line by its own id; the entry has no header.
                } else {
                    let _ = write!(text, "[{}] {}", e.id, e.name);
                    if !e.place.is_empty() {
                        let _ = write!(text, ", {}", e.place);
                    }
                    if !e.dates.is_empty() {
                        let _ = write!(text, " · {}", e.dates);
                    }
                    text.push('\n');
                    if !e.subtitle.is_empty() {
                        let _ = writeln!(text, "{}", e.subtitle);
                    }
                    let _ = writeln!(printed, "{} {} {} {}", e.name, e.place, e.dates, e.subtitle);
                    if bank_entry.is_some() && !entries.iter().any(|(id, _)| *id == e.id) {
                        entries.push((e.id.clone(), e.name.clone()));
                    }
                }
                for l in lines.iter().filter(|l| l.entry == e.id) {
                    let _ = writeln!(printed, "{}", l.text);
                    if e.bullets.iter().any(|b| b.id == l.id) {
                        let _ = writeln!(text, "- [{}] {}", l.id, l.text);
                    } else {
                        let _ = writeln!(text, "[{}] {}", l.id, l.text);
                    }
                }
                if let Some(be) = bank_entry
                    && !(summary.is_some() && i == 0)
                {
                    write_facts(&mut text, be);
                    for f in &be.facts {
                        facts.push((e.id.clone(), f.id.clone(), f.text.clone()));
                    }
                }
            }
            for g in &section.groups {
                let _ = writeln!(text, "{}: {}", g.label, g.items.join(", "));
                let _ = writeln!(printed, "{}: {}", g.label, g.items.join(", "));
            }
        }
        Self {
            text,
            lines,
            printed,
            entries,
            facts,
        }
    }

    /// The exported PDF's text: no ids, no facts.
    pub fn pdf(text: &str) -> Self {
        Self {
            text: text.trim().to_owned(),
            printed: text.trim().to_owned(),
            ..Self::default()
        }
    }

    pub fn line(&self, id: &str) -> Option<&Line> {
        self.lines.iter().find(|l| l.id == id)
    }

    /// The name of entry `id` when the resume shows it and the bank holds it.
    pub fn entry_name(&self, id: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(e, _)| e == id)
            .map(|(_, n)| n.as_str())
    }

    /// The texts of `ids`' lines (the PDF's whole text when it has no lines), then the facts of the entries they sit
    /// under: what a model-written sentence citing them may take its numbers from.
    pub fn sources<'s>(&'s self, ids: &[String]) -> Vec<&'s str> {
        if self.lines.is_empty() {
            return vec![self.text.as_str()];
        }
        let lines: Vec<&Line> = ids.iter().filter_map(|id| self.line(id)).collect();
        let mut out: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        out.extend(
            self.facts
                .iter()
                .filter(|(entry, _, _)| lines.iter().any(|l| l.entry == *entry))
                .map(|(_, _, text)| text.as_str()),
        );
        out
    }

    /// Every fact of the resume's entries.
    pub fn all_facts(&self) -> impl Iterator<Item = &str> {
        self.facts.iter().map(|(_, _, t)| t.as_str())
    }

    /// `ids` that name a line of the resume, each once, in order; the others go on `dropped` (each once).
    pub fn keep(&self, ids: Vec<String>, dropped: &mut Vec<String>) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for id in ids {
            let id = id.trim();
            if self.line(id).is_none() {
                if !dropped.iter().any(|d| d == id) {
                    dropped.push(id.to_owned());
                }
            } else if !out.iter().any(|o| o == id) {
                out.push(id.to_owned());
            }
        }
        out
    }
}

/// The resume's lines in page order: the summary (`summary`), then each entry's note lines (`acme#1`, from 1) and
/// bullets (`acme.2`). Skills lines aren't lines here.
pub fn sent_lines(resume: &Resume) -> Vec<Line> {
    let mut out = Vec::new();
    for section in &resume.sections {
        let summary = summary_entry(section);
        for (i, e) in section.entries.iter().enumerate() {
            let is_summary = summary.is_some() && i == 0;
            let name = if is_summary { "" } else { e.name.as_str() };
            let line = |id: String, text: &str| Line {
                id,
                entry: e.id.clone(),
                entry_name: name.to_owned(),
                text: text.to_owned(),
            };
            for (j, n) in e.notes.iter().enumerate() {
                let id = if is_summary && j == 0 {
                    SUMMARY.to_owned()
                } else {
                    format!("{}#{}", e.id, j + 1)
                };
                let text = if n.label.is_empty() {
                    n.text.clone()
                } else {
                    format!("{}: {}", n.label, n.text)
                };
                out.push(line(id, &text));
            }
            for (j, b) in e.bullets.iter().enumerate() {
                let id = if is_summary && e.notes.is_empty() && j == 0 {
                    SUMMARY.to_owned()
                } else {
                    b.id.clone()
                };
                out.push(line(id, &b.text));
            }
        }
    }
    out
}

/// The summary section's first entry, when `section` is the summary.
fn summary_entry(section: &Section) -> Option<&resume_core::resume::Entry> {
    (section.kind == Some(SectionKind::Summary))
        .then(|| section.entries.first())
        .flatten()
}

/// The bank entry `id`, in any section.
fn find_entry<'b>(bank: &'b Bank, id: &str) -> Option<&'b BankEntry> {
    bank.sections
        .iter()
        .flat_map(|s| &s.entries)
        .find(|e| e.id == id)
}

/// What Prep reads.
#[derive(Clone, Copy, Debug)]
pub struct PrepInput<'a> {
    /// The round's kind as the app names it (`technical`, `recruiter-screen`, …).
    pub kind: &'a str,
    /// Who you meet.
    pub with: &'a str,
    /// The round's notes.
    pub notes: &'a str,
    pub role: &'a str,
    pub company: &'a str,
    /// The posting's requirements; `None` when the application has no posting read.
    pub requirements: Option<&'a Requirements>,
    pub sent: &'a SentResume,
    /// `YYYY-MM-DD`.
    pub today: &'a str,
}

/// The Prep call's input: `<today>`, `<round>`, `<job>`, `<requirements>` (one skill a line, most important first),
/// `<resume>` (tagged; `ids="none"` for the PDF's text).
pub fn input(i: &PrepInput<'_>) -> String {
    let mut s = String::with_capacity(i.sent.text.len() + 2048);
    let _ = writeln!(s, "<today>{}</today>", i.today);
    let _ = writeln!(s, "<round>\nkind: {}", i.kind);
    if !i.with.trim().is_empty() {
        let _ = writeln!(s, "with: {}", i.with.trim());
    }
    if !i.notes.trim().is_empty() {
        let _ = writeln!(s, "notes: {}", i.notes.trim());
    }
    s.push_str("</round>\n");
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
    s.push_str("\n</resume>\n");
    s
}

/// `- term (kind; required|preferred|nice to have)`, one a line; then the years and degree asked for.
pub fn write_requirements(s: &mut String, req: &Requirements) {
    for k in &req.skills {
        let weight = if k.required || k.priority == 1 {
            "required"
        } else if k.priority == 2 {
            "preferred"
        } else {
            "nice to have"
        };
        let kind = serde_json::to_value(k.kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        let _ = writeln!(s, "- {} ({kind}; {weight})", k.term);
    }
    if let Some(y) = req.years_min {
        let _ = writeln!(s, "years: {y}+");
    }
    if let Some(d) = &req.degree {
        let level = serde_json::to_value(d.level)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        let _ = writeln!(s, "degree: {level} {}", d.fields.join(", "));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    Resume,
    Skills,
    Behavioral,
    Logistics,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    /// `q1`, `q2`…: set by [`checked`].
    #[serde(default)]
    pub id: String,
    pub group: Group,
    pub text: String,
    pub why: String,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gap {
    pub term: String,
    pub required: bool,
    pub answer: String,
    pub lines: Vec<String>,
}

/// A round's prep.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prep {
    pub focus: String,
    pub questions: Vec<Question>,
    pub gaps: Vec<Gap>,
    pub ask_them: Vec<String>,
}

/// What [`checked`] took out, for the log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dropped {
    /// Line ids no line of the resume has.
    pub lines: Vec<String>,
    /// Gap terms dropped whole: not a required or priority-1 requirement, shown on the resume, or no sentence left.
    pub gaps: Vec<String>,
    /// Gap-answer sentences dropped for a number neither their lines nor those entries' facts state.
    pub sentences: Vec<String>,
}

impl Dropped {
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.gaps.is_empty() && self.sentences.is_empty()
    }
}

/// `p` held to what the resume as sent and the bank back: unknown line ids dropped; a gap kept only for a required or
/// priority-1 requirement skill the page doesn't show (named and weighed as the posting does), its answer without any
/// sentence whose numbers neither its lines nor their entries' facts state (the gap dropped when none is left);
/// questions numbered `q1`…; every list capped.
pub fn checked(p: Prep, sent: &SentResume, req: Option<&Requirements>) -> (Prep, Dropped) {
    let mut dropped = Dropped::default();
    let questions = p
        .questions
        .into_iter()
        .filter(|q| !q.text.trim().is_empty())
        .take(MAX_QUESTIONS)
        .enumerate()
        .map(|(i, q)| Question {
            id: format!("q{}", i + 1),
            group: q.group,
            text: q.text.trim().to_owned(),
            why: q.why.trim().to_owned(),
            lines: sent.keep(q.lines, &mut dropped.lines),
        })
        .collect();
    let printed = Page::of_text(&sent.printed);
    let mut gaps: Vec<Gap> = Vec::new();
    for g in p.gaps {
        let skill = req.and_then(|r| {
            r.skills.iter().find(|k| {
                let term = g.term.trim().to_lowercase();
                k.term.to_lowercase() == term
                    || all_aliases(&k.term, &k.aliases)
                        .iter()
                        .any(|a| a.to_lowercase() == term)
            })
        });
        let Some(skill) = skill.filter(|k| k.required || k.priority == 1) else {
            dropped.gaps.push(g.term);
            continue;
        };
        if find_literal(&printed, &skill.term, &skill.aliases).is_some()
            || gaps.iter().any(|x| x.term == skill.term)
        {
            dropped.gaps.push(g.term);
            continue;
        }
        let lines = sent.keep(g.lines, &mut dropped.lines);
        let sources = sent.sources(&lines);
        let mut kept = Vec::new();
        for sentence in sentences(&g.answer) {
            if unsourced_numbers(sentence, &sources).is_empty() {
                kept.push(sentence);
            } else {
                dropped.sentences.push(sentence.to_owned());
            }
        }
        if kept.is_empty() {
            dropped.gaps.push(g.term);
            continue;
        }
        gaps.push(Gap {
            term: skill.term.clone(),
            required: skill.required,
            answer: kept.join(" "),
            lines,
        });
        if gaps.len() == MAX_GAPS {
            break;
        }
    }
    let prep = Prep {
        focus: p.focus.trim().to_owned(),
        questions,
        gaps,
        ask_them: p
            .ask_them
            .into_iter()
            .map(|a| a.trim().to_owned())
            .filter(|a| !a.is_empty())
            .take(MAX_ASK)
            .collect(),
    };
    (prep, dropped)
}

/// `text`'s sentences, trimmed: each ends at `.`, `!` or `?` followed by a space (or the end).
pub fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    for (i, c) in text.char_indices() {
        if matches!(c, '.' | '!' | '?') && bytes.get(i + 1).is_none_or(|b| b.is_ascii_whitespace()) {
            let s = text[start..=i].trim();
            if !s.is_empty() {
                out.push(s);
            }
            start = i + 1;
        }
    }
    let rest = text[start..].trim();
    if !rest.is_empty() {
        out.push(rest);
    }
    out
}

/// Runs Prep with `model`: the reply as written (unchecked: see [`checked`]).
pub async fn write(
    llm: &impl Llm,
    schema: &Schema,
    input: &PrepInput<'_>,
    model: &str,
) -> Result<(Prep, Structured), LlmError> {
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
    match Prep::deserialize(&output.value) {
        Ok(p) => Ok((p, output)),
        Err(e) => Err(LlmError::Decode {
            target: "Prep",
            message: e.to_string(),
            output,
        }),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use resume_core::bank::{Fact, FactKind, FactSource};
    use resume_core::requirements::{Seniority, Skill, SkillKind};
    use resume_core::{Bank, Requirements, Resume, migrate_v1, resolve};

    use super::{Gap, Group, Prep, PrepInput, Question, SentResume, checked, input, sentences};

    /// The synthetic jane-doe bank, migrated, with one fact on `acme`, and its master resolved.
    pub(crate) fn jane() -> (Bank, Resume) {
        let v1: Resume =
            serde_json::from_str(include_str!("../../../fixtures/banks/jane-doe.v1.json")).unwrap();
        let mut m = migrate_v1(&v1);
        let acme = m
            .bank
            .sections
            .iter_mut()
            .flat_map(|s| &mut s.entries)
            .find(|e| e.id == "acme")
            .unwrap();
        acme.facts.push(Fact {
            id: "acme~1".into(),
            text: "The test suite ran on 3 services".into(),
            skills: vec!["Jest".into()],
            source: FactSource {
                kind: FactKind::Edit,
                at: "2026-10-27T10:00:00Z".into(),
                posting: None,
                company: None,
            },
        });
        let resume = resolve(&m.bank, &m.master).unwrap();
        (m.bank, resume)
    }

    pub(crate) fn requirements() -> Requirements {
        let skill = |term: &str, priority: u8, required: bool| Skill {
            term: term.into(),
            aliases: vec![],
            kind: SkillKind::Tool,
            priority,
            required,
            group: None,
        };
        Requirements {
            role: "Software Engineer".into(),
            company: "Acme".into(),
            short_role: "swe".into(),
            location: String::new(),
            seniority: Seniority::Entry,
            skills: vec![
                skill("Kubernetes", 1, true),
                skill("Terraform", 2, false),
                skill("Kotlin", 1, true),
            ],
            title_terms: vec![],
            years_min: Some(1),
            degree: None,
            certifications: vec![],
            knockouts: vec![],
        }
    }

    #[test]
    fn schema_compiles() {
        super::schema();
    }

    #[test]
    fn the_sent_resume_is_tagged_with_its_lines_and_facts() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        assert!(sent.text.contains("- [acme.1] "), "{}", sent.text);
        assert!(sent.text.contains("[acme~1] (fact) The test suite ran on 3 services\n  (skills) Jest\n"));
        assert!(sent.line("acme.1").is_some());
        assert_eq!(sent.line("acme.1").unwrap().entry, "acme");
        assert!(!sent.line("acme.1").unwrap().entry_name.is_empty());
        assert!(sent.entry_name("acme").is_some());
        assert!(!sent.printed.contains("(fact)"));
        // The facts come after the entry's bullets, before the next entry.
        let fact = sent.text.find("[acme~1]").unwrap();
        let last_bullet = sent.text.rfind("- [acme.").unwrap();
        assert!(fact > last_bullet);
    }

    #[test]
    fn input_carries_every_part() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let req = requirements();
        let text = input(&PrepInput {
            kind: "technical",
            with: " Priya Shah ",
            notes: "",
            role: "Software Engineer",
            company: "Acme",
            requirements: Some(&req),
            sent: &sent,
            today: "2026-10-27",
        });
        assert!(text.starts_with(
            "<today>2026-10-27</today>\n<round>\nkind: technical\nwith: Priya Shah\n</round>\n\
             <job>Software Engineer at Acme</job>\n<requirements>\n- Kubernetes (tool; required)\n\
             - Terraform (tool; preferred)\n- Kotlin (tool; required)\nyears: 1+\n</requirements>\n<resume>\n"
        ));
        assert!(text.contains("[acme~1] (fact)"));
        assert!(text.ends_with("\n</resume>\n"));
        let pdf = SentResume::pdf("Jane Doe\nAcme");
        let text = input(&PrepInput {
            kind: "recruiter-screen",
            with: "",
            notes: "Bring questions",
            role: "SWE",
            company: "Acme",
            requirements: None,
            sent: &pdf,
            today: "2026-10-27",
        });
        assert!(text.contains(
            "kind: recruiter-screen\nnotes: Bring questions\n</round>\n<job>SWE at Acme</job>\n\
             <requirements>\n</requirements>\n<resume ids=\"none\">\nJane Doe\nAcme\n</resume>\n"
        ));
    }

    #[test]
    fn sentences_split_at_their_ends() {
        assert_eq!(
            sentences("Not in production. I used 2.5 GB! Why? ok"),
            ["Not in production.", "I used 2.5 GB!", "Why?", "ok"]
        );
    }

    fn question(lines: &[&str]) -> Question {
        Question {
            id: "x".into(),
            group: Group::Resume,
            text: "Tell me about it".into(),
            why: "A number invites a follow-up".into(),
            lines: lines.iter().map(|l| (*l).to_owned()).collect(),
        }
    }

    #[test]
    fn checking_drops_unknown_lines_and_unsourced_gap_sentences() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let req = requirements();
        let p = Prep {
            focus: " Depth on testing ".into(),
            questions: (0..12).map(|_| question(&["acme.1", "acme.99", "acme.1"])).collect(),
            gaps: vec![
                Gap {
                    term: "kubernetes".into(),
                    required: false,
                    answer: "I haven't run Kubernetes. The suite ran on 3 services. It cut deploys by 75%."
                        .into(),
                    lines: vec!["acme.1".into(), "nope".into()],
                },
                // Priority 2: not a gap prep names.
                Gap {
                    term: "Terraform".into(),
                    required: false,
                    answer: "Not yet.".into(),
                    lines: vec![],
                },
                // Every sentence unsourced: the gap goes.
                Gap {
                    term: "Kotlin".into(),
                    required: true,
                    answer: "I wrote 12 scripts.".into(),
                    lines: vec![],
                },
            ],
            ask_them: (0..7).map(|i| format!("Ask {i}")).collect(),
        };
        let (p, dropped) = checked(p, &sent, Some(&req));
        assert_eq!(p.focus, "Depth on testing");
        assert_eq!(p.questions.len(), 10);
        assert_eq!(p.questions[0].id, "q1");
        assert_eq!(p.questions[9].id, "q10");
        assert_eq!(p.questions[0].lines, ["acme.1"]);
        assert_eq!(p.ask_them.len(), 5);
        assert_eq!(p.gaps.len(), 1);
        let g = &p.gaps[0];
        assert_eq!((g.term.as_str(), g.required), ("Kubernetes", true));
        assert_eq!(
            g.answer,
            "I haven't run Kubernetes. The suite ran on 3 services."
        );
        assert_eq!(g.lines, ["acme.1"]);
        assert_eq!(dropped.lines, ["acme.99", "nope"]);
        assert_eq!(dropped.gaps, ["Terraform", "Kotlin"]);
        assert_eq!(
            dropped.sentences,
            ["It cut deploys by 75%.", "I wrote 12 scripts."]
        );
    }

    #[test]
    fn a_term_the_page_shows_is_no_gap() {
        let (bank, resume) = jane();
        let sent = SentResume::tagged(&resume, &bank);
        let shown = sent
            .printed
            .split_whitespace()
            .find(|w| w.len() > 3 && w.chars().all(char::is_alphabetic))
            .unwrap()
            .to_owned();
        let mut req = requirements();
        req.skills[0].term = shown.clone();
        let p = Prep {
            focus: String::new(),
            questions: vec![],
            gaps: vec![Gap {
                term: shown,
                required: true,
                answer: "Not in production.".into(),
                lines: vec![],
            }],
            ask_them: vec![],
        };
        let (p, _) = checked(p, &sent, Some(&req));
        assert!(p.gaps.is_empty());
    }
}
