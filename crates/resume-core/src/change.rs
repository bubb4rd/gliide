//! Changes: edits to a resolved master, each one a toggle with its own wording (ARCHITECTURE.md §5).
//!
//! A tailored resume is `apply(bank, master, changes that are in)`. Suggestions become changes in `changes_of`; your
//! decisions, wording and own edits live in a [`Draft`] keyed by [`Change::key`], so they survive a re-run that
//! proposes the same change.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::posting::text_hash;

/// Where suggestions may draw from (ARCHITECTURE.md §2, "Master only or the whole bank").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// Only what the master shows.
    #[default]
    MasterOnly,
    /// The master plus bank entries outside it, ranked by requirement-term overlap.
    BankAware,
}

/// One edit. Ids are the tagged resume's: entry `acme`, bullet `acme.2`, note line `state-u#2`, skills line `tools`,
/// section `work`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    /// The bullet's text becomes `text` (its old text stays as `orig`, the line budget).
    Rewrite {
        bullet: String,
        text: String,
    },
    /// The bullet shows one of its own variants.
    UseVariant {
        bullet: String,
        variant: usize,
    },
    Cut {
        bullet: String,
    },
    /// A new bullet at the end of the entry.
    Add {
        entry: String,
        text: String,
    },
    SkillAdd {
        group: String,
        item: String,
    },
    SkillRemove {
        group: String,
        item: String,
    },
    SkillRename {
        group: String,
        from: String,
        to: String,
    },
    /// `item` leaves skills line `from` for skills line `to`.
    SkillMove {
        from: String,
        to: String,
        item: String,
    },
    /// `term` appended to a note line (`, term`).
    NoteAdd {
        note: String,
        term: String,
    },
    /// `from` replaced by `to` inside a bullet or note line; `to` empty cuts the words.
    Swap {
        line: String,
        from: String,
        to: String,
    },
    /// A bullet or note line becomes `text` (your wording of any line edit).
    Set {
        line: String,
        text: String,
    },
    /// A bank entry outside the master shown at the end of its section (all bullets when `bullets` is empty).
    Include {
        entry: String,
        bullets: Vec<String>,
    },
    /// A master entry hidden for this posting.
    Exclude {
        entry: String,
    },
    /// Master entry `out` replaced in place by bank entry `entry`.
    SwapEntry {
        out: String,
        entry: String,
        bullets: Vec<String>,
    },
    /// The section's entries in this order.
    Reorder {
        section: String,
        entries: Vec<String>,
    },
    /// The summary line (a summary section is added when the master has none).
    Summary {
        text: String,
    },
}

impl Op {
    /// The op's name and the line it targets: what a change's key is made of, with the text it replaces.
    pub fn target(&self) -> (&'static str, String) {
        match self {
            Op::Rewrite { bullet, .. } => ("rewrite", bullet.clone()),
            Op::UseVariant { bullet, .. } => ("use_variant", bullet.clone()),
            Op::Cut { bullet } => ("cut", bullet.clone()),
            Op::Add { entry, .. } => ("add", entry.clone()),
            Op::SkillAdd { group, item } => {
                ("skill_add", format!("{group}/{}", item.to_lowercase()))
            }
            Op::SkillRemove { group, item } => {
                ("skill_remove", format!("{group}/{}", item.to_lowercase()))
            }
            Op::SkillRename { group, from, .. } => {
                ("skill_rename", format!("{group}/{}", from.to_lowercase()))
            }
            Op::SkillMove { from, item, .. } => {
                ("skill_move", format!("{from}/{}", item.to_lowercase()))
            }
            Op::NoteAdd { note, term } => ("note_add", format!("{note}/{}", term.to_lowercase())),
            Op::Swap { line, from, .. } => ("swap", format!("{line}/{}", from.to_lowercase())),
            Op::Set { line, .. } => ("set", line.clone()),
            Op::Include { entry, .. } => ("include", entry.clone()),
            Op::Exclude { entry } => ("exclude", entry.clone()),
            Op::SwapEntry { out, entry, .. } => ("swap_entry", format!("{out}>{entry}")),
            Op::Reorder { section, .. } => ("reorder", section.clone()),
            Op::Summary { .. } => ("summary", String::new()),
        }
    }
}

/// Who proposed a change. Yours are never left out by the fit check; they only warn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Suggested,
    Yours,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Change {
    /// `hash(op name, target, before)`: stable across re-runs that propose the same edit, whatever its wording.
    pub key: String,
    pub op: Op,
    pub origin: Origin,
    /// The text it replaces, as on the master (`""` for additions).
    pub before: String,
    /// Requirement terms it puts on the page.
    pub hits: Vec<String>,
    pub why: String,
    /// 1 most important … 3; the fit check leaves out the least important first.
    pub priority: u8,
    /// Where the review lists it: a section id, or `bank` for bank edits.
    pub section: String,
    /// Grounding validator findings, as sentences (`"40%" isn't in your resume`). Non-empty: starts left out.
    pub flags: Vec<String>,
}

impl Change {
    /// A change with its key computed.
    pub fn new(
        op: Op,
        origin: Origin,
        before: String,
        hits: Vec<String>,
        why: String,
        priority: u8,
        section: String,
    ) -> Self {
        let key = key(&op, &before);
        Self {
            key,
            op,
            origin,
            before,
            hits,
            why,
            priority,
            section,
            flags: Vec::new(),
        }
    }
}

/// `hash(op name, target, before)`, 16 hex digits.
pub fn key(op: &Op, before: &str) -> String {
    let (name, target) = op.target();
    text_hash(&format!("{name}\u{1f}{target}\u{1f}{}", before.trim()))[..16].to_owned()
}

/// Your explicit choice on a change. No entry means the default: in, unless the validator or fit check leaves it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    /// On, even if the fit check would leave it out (`Forced` when it would).
    On,
    Off,
}

/// Your work on top of the suggestions, persisted per analysis.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub decisions: BTreeMap<String, Decision>,
    /// Your wording for a change, by key: a rewrite or swap you edited becomes a `set` of the line.
    pub wording: BTreeMap<String, String>,
    /// Your own edits (`origin: yours`).
    pub custom: Vec<Change>,
}

/// One step of your work on a draft. A draft is stored as its append-only event log, so undo and redo move a cursor
/// over the log and [`Draft::replay`] rebuilds the draft from the events before it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum DraftEvent {
    /// Your choice on a change; `None` goes back to the default.
    Decide {
        key: String,
        decision: Option<Decision>,
    },
    /// Your wording for a change; `None` goes back to the suggested wording.
    Word { key: String, text: Option<String> },
    /// One of your own edits (`origin: yours`), added in; an edit with the same key is replaced where it stands.
    AddCustom { change: Change },
    /// One of your own edits removed, with your decision and wording on it.
    RemoveCustom { key: String },
    /// Several events from one action of yours (a skill chip that turns off every change adding it), so one undo
    /// takes them all back.
    Batch { events: Vec<DraftEvent> },
}

impl Draft {
    /// The draft `events` make, in order, from an empty one.
    pub fn replay(events: &[DraftEvent]) -> Self {
        let mut draft = Self::default();
        for e in events {
            draft.apply(e);
        }
        draft
    }

    /// Applies one event.
    pub fn apply(&mut self, event: &DraftEvent) {
        match event {
            DraftEvent::Decide { key, decision } => match decision {
                Some(d) => {
                    self.decisions.insert(key.clone(), *d);
                }
                None => {
                    self.decisions.remove(key);
                }
            },
            DraftEvent::Word { key, text } => match text {
                Some(t) => {
                    self.wording.insert(key.clone(), t.clone());
                }
                None => {
                    self.wording.remove(key);
                }
            },
            DraftEvent::AddCustom { change } => {
                match self.custom.iter_mut().find(|c| c.key == change.key) {
                    Some(c) => *c = change.clone(),
                    None => self.custom.push(change.clone()),
                }
            }
            DraftEvent::RemoveCustom { key } => {
                self.custom.retain(|c| &c.key != key);
                self.decisions.remove(key);
                self.wording.remove(key);
            }
            DraftEvent::Batch { events } => {
                for e in events {
                    self.apply(e);
                }
            }
        }
    }
}

/// Whether a change is in the tailored resume, and why not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum State {
    In,
    /// You turned it off.
    Off,
    /// The validator or the fit check kept the original; `reason` says why.
    LeftOut {
        reason: String,
    },
    /// You turned it on although it would be left out; never removed again. `warning` when it runs long.
    Forced {
        warning: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom(item: &str) -> Change {
        Change::new(
            Op::SkillAdd {
                group: "tools".into(),
                item: item.into(),
            },
            Origin::Yours,
            String::new(),
            vec![item.into()],
            String::new(),
            1,
            "skills".into(),
        )
    }

    #[test]
    fn replay_rebuilds_each_step_and_removal_forgets_its_choices() {
        let kafka = custom("Kafka");
        let events = [
            DraftEvent::Decide {
                key: "a".into(),
                decision: Some(Decision::Off),
            },
            DraftEvent::AddCustom {
                change: kafka.clone(),
            },
            DraftEvent::Word {
                key: kafka.key.clone(),
                text: Some("Apache Kafka".into()),
            },
            DraftEvent::Decide {
                key: kafka.key.clone(),
                decision: Some(Decision::On),
            },
            DraftEvent::RemoveCustom {
                key: kafka.key.clone(),
            },
            DraftEvent::Decide {
                key: "a".into(),
                decision: None,
            },
        ];

        // Undo to after the wording: the custom change and its wording, `a` still off.
        let mid = Draft::replay(&events[..3]);
        assert_eq!(mid.custom, vec![kafka.clone()]);
        assert_eq!(mid.wording[&kafka.key], "Apache Kafka");
        assert_eq!(mid.decisions["a"], Decision::Off);

        // The whole log: the removed change takes its decision and wording with it; `a` is back to the default.
        assert_eq!(Draft::replay(&events), Draft::default());
    }
}
