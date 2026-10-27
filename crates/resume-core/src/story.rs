//! Stories: what you tell in interviews, situation, task, action and result in your words (ARCHITECTURE.md §2). A
//! story is interview material, not resume content: it never prints and changes as you practice, so it lives in its own
//! record (the store's `stories` table, `schemas/story.schema.json`) and refers to the bank by id instead of being part
//! of a bank revision.
//!
//! Grounding: a story drawn from an answer (`source.from` set: a practice turn or a Dig capture) may state only the
//! numbers its bullets (text and variants), its facts or that answer state ([`unsourced`]). A story you type is your
//! word, as a fact is, and isn't checked.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::bank::{Bank, BankBullet, Fact};
use crate::validate::unsourced_numbers;

/// Words a minute a story is told at.
pub const WORDS_PER_MINUTE: u32 = 150;

/// A story you tell in interviews: situation, task, action, result, in your words. Never printed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Story {
    /// A UUID.
    pub id: String,
    /// "Caught 14 regressions before release".
    pub title: String,
    /// The bank entry it happened in; null when it's off the resume (a class project's conflict).
    #[serde(default)]
    pub entry: Option<String>,
    /// The bank bullets it expands (`acme.2`).
    #[serde(default)]
    pub bullets: Vec<String>,
    /// The facts it draws on (`acme~1`).
    #[serde(default)]
    pub facts: Vec<String>,
    pub situation: String,
    pub task: String,
    pub action: String,
    pub result: String,
    /// Terms it shows, in requirement words ("GitHub Actions", "Integration testing").
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub themes: Vec<StoryTheme>,
    pub source: StorySource,
    /// Last changed, RFC 3339.
    pub updated_at: String,
}

/// What kind of question a story answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum StoryTheme {
    Conflict,
    Failure,
    Leadership,
    Ambiguity,
    Deadline,
    Impact,
    Learning,
    Teamwork,
    Initiative,
}

impl StoryTheme {
    pub const ALL: [Self; 9] = [
        Self::Conflict,
        Self::Failure,
        Self::Leadership,
        Self::Ambiguity,
        Self::Deadline,
        Self::Impact,
        Self::Learning,
        Self::Teamwork,
        Self::Initiative,
    ];

    /// The lowercase name, as stored and as the prompts write it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Conflict => "conflict",
            Self::Failure => "failure",
            Self::Leadership => "leadership",
            Self::Ambiguity => "ambiguity",
            Self::Deadline => "deadline",
            Self::Impact => "impact",
            Self::Learning => "learning",
            Self::Teamwork => "teamwork",
            Self::Initiative => "initiative",
        }
    }
}

/// Where a story came from and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StorySource {
    pub kind: StoryKind,
    /// RFC 3339.
    pub at: String,
    /// The answer it was drawn from: a practice turn's id or a Dig capture's id. Its words ground the story's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum StoryKind {
    /// Proposed by Dig from your answers about an entry.
    Dig,
    /// Drawn from a practiced answer ("Save as story").
    Practice,
    /// Typed in the story editor.
    Edit,
}

impl Story {
    /// Title, situation, task, action, result: the words a story says.
    pub fn parts(&self) -> [&str; 5] {
        [
            &self.title,
            &self.situation,
            &self.task,
            &self.action,
            &self.result,
        ]
    }

    /// About how long it takes to tell, in seconds: its situation, task, action and result at [`WORDS_PER_MINUTE`].
    pub fn seconds(&self) -> u32 {
        let words: usize = [&self.situation, &self.task, &self.action, &self.result]
            .iter()
            .map(|p| p.split_whitespace().count())
            .sum();
        (words as u32 * 60).div_ceil(WORDS_PER_MINUTE)
    }

    /// Points every link to bullet `from` at `into` instead (`from` folded into `into`), each bullet once. True when
    /// anything moved.
    pub fn move_bullet(&mut self, from: &str, into: &str) -> bool {
        if !self.bullets.iter().any(|b| b == from) {
            return false;
        }
        let mut out: Vec<String> = Vec::with_capacity(self.bullets.len());
        for b in self.bullets.drain(..) {
            let b = if b == from { into.to_owned() } else { b };
            if !out.contains(&b) {
                out.push(b);
            }
        }
        self.bullets = out;
        true
    }
}

/// Findings for the numbers `story` states that none of its bullets (text and variants), its facts nor `answer` (the
/// text it was drawn from) state. Links the bank no longer holds ground nothing.
pub fn unsourced(story: &Story, bank: &Bank, answer: &str) -> Vec<String> {
    let mut sources: Vec<&str> = vec![answer];
    for b in &story.bullets {
        if let Some(b) = bullet(bank, b) {
            sources.push(&b.text);
            sources.extend(b.variants.iter().map(String::as_str));
        }
    }
    for f in &story.facts {
        if let Some(f) = fact(bank, f) {
            sources.push(&f.text);
        }
    }
    let mut out: Vec<String> = Vec::new();
    for part in story.parts() {
        for finding in unsourced_numbers(part, &sources) {
            if !out.contains(&finding) {
                out.push(finding);
            }
        }
    }
    out
}

/// Bullet `id` of the bank.
pub fn bullet<'b>(bank: &'b Bank, id: &str) -> Option<&'b BankBullet> {
    bank.sections
        .iter()
        .flat_map(|s| &s.entries)
        .flat_map(|e| &e.bullets)
        .find(|b| b.id == id)
}

/// Fact `id` of the bank.
pub fn fact<'b>(bank: &'b Bank, id: &str) -> Option<&'b Fact> {
    bank.sections
        .iter()
        .flat_map(|s| &s.entries)
        .flat_map(|e| &e.facts)
        .find(|f| f.id == id)
}

/// The themes none of `stories` covers, in [`StoryTheme::ALL`]'s order.
pub fn uncovered<'s>(stories: impl IntoIterator<Item = &'s Story>) -> Vec<StoryTheme> {
    let mut covered = Vec::new();
    for s in stories {
        covered.extend(s.themes.iter().copied());
    }
    StoryTheme::ALL
        .into_iter()
        .filter(|t| !covered.contains(t))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bank::{Fact, FactKind, FactSource};
    use crate::migrate_v1;
    use crate::resume::Resume;

    fn bank() -> Bank {
        let v1: Resume =
            serde_json::from_str(include_str!("../../../fixtures/banks/jane-doe.v1.json")).unwrap();
        let mut bank = migrate_v1(&v1).bank;
        let acme = bank
            .sections
            .iter_mut()
            .flat_map(|s| &mut s.entries)
            .find(|e| e.id == "acme")
            .unwrap();
        acme.bullets[0].variants.push("Shipped 7 releases".into());
        acme.facts.push(Fact {
            id: "acme~1".into(),
            text: "The suite ran on 3 services".into(),
            skills: vec![],
            source: FactSource {
                kind: FactKind::Edit,
                at: "2026-10-27T10:00:00Z".into(),
                posting: None,
                company: None,
            },
        });
        bank
    }

    fn story() -> Story {
        Story {
            id: "s1".into(),
            title: "Caught regressions before release".into(),
            entry: Some("acme".into()),
            bullets: vec!["acme.1".into(), "acme.2".into()],
            facts: vec!["acme~1".into()],
            situation: "Releases kept breaking the order API.".into(),
            task: "I owned the test suite.".into(),
            action: "I wrote integration tests across 3 services and shipped 7 releases.".into(),
            result: "They caught 14 regressions; on-call pages fell by 40%.".into(),
            skills: vec!["Integration testing".into()],
            themes: vec![StoryTheme::Impact],
            source: StorySource {
                kind: StoryKind::Dig,
                at: "2026-10-27T10:00:00Z".into(),
                from: Some("d".into()),
            },
            updated_at: "2026-10-27T10:00:00Z".into(),
        }
    }

    #[test]
    fn numbers_come_from_bullets_variants_facts_or_the_answer() {
        let bank = bank();
        // 3 from the fact, 7 from a variant, 14 from acme.2; 40% from nowhere.
        assert_eq!(
            unsourced(&story(), &bank, "I wrote the tests."),
            ["“40%” is not in the lines it draws on"]
        );
        assert!(unsourced(&story(), &bank, "Pages fell by 40%.").is_empty());
        // A link the bank no longer holds grounds nothing.
        let s = Story {
            facts: vec!["acme~9".into()],
            ..story()
        };
        assert_eq!(unsourced(&s, &bank, "40%").len(), 1);
    }

    #[test]
    fn folding_moves_links_onto_the_bullet_kept() {
        let mut s = story();
        assert!(s.move_bullet("acme.1", "acme.2"));
        assert_eq!(s.bullets, ["acme.2"]);
        assert!(!s.move_bullet("acme.1", "acme.2"));
        let mut s = story();
        assert!(s.move_bullet("acme.2", "acme.3"));
        assert_eq!(s.bullets, ["acme.1", "acme.3"]);
    }

    #[test]
    fn telling_time_and_uncovered_themes() {
        let s = Story {
            situation: "one two three".into(),
            task: String::new(),
            action: "four".into(),
            result: "five".into(),
            ..story()
        };
        // 5 words at 150 a minute: 2 s.
        assert_eq!(s.seconds(), 2);
        let words = vec!["w"; 250].join(" ");
        assert_eq!(
            Story {
                situation: words,
                ..s.clone()
            }
            .seconds(),
            101
        );
        let u = uncovered([&story()]);
        assert_eq!(u.len(), 8);
        assert!(!u.contains(&StoryTheme::Impact));
        assert_eq!(u[0], StoryTheme::Conflict);
    }

    #[test]
    fn a_story_round_trips_and_has_a_schema() {
        let s = story();
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Story>(&json).unwrap(), s);
        let schema = serde_json::to_value(crate::story_schema()).unwrap();
        assert_eq!(schema["title"], "Story");
    }
}
