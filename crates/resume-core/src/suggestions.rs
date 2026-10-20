//! What to change for one posting: the Suggest call's output (ARCHITECTURE.md §3). Master-dependent.
//!
//! Every id names a line of the tagged resume the model saw: an entry (`acme`), a bullet (`acme.2`), a note line
//! (`state-u#2`, 1-based), a skills line (`tools`), a section (`work`), or a bank entry outside the master (bank-aware
//! mode). `prompts/suggest.schema.json` must accept exactly this shape; every key is present, `""`/`null` when unused.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Suggestions {
    pub verdict: Verdict,
    /// One line, the single most useful thing to know.
    pub headline: String,
    /// One row per requirement skill: whether the resume backs it, and which lines do.
    pub evidence: Vec<Evidence>,
    /// In-place edits to a note line, or to a bullet that has no rewrite.
    pub keywords: Vec<Keyword>,
    pub bullets: Vec<BulletEdit>,
    /// Edits to Skills lines.
    pub skills: Vec<SkillEdit>,
    /// Bank-aware mode only (empty otherwise): entries outside the master worth showing.
    pub bank: Vec<BankEdit>,
    /// Entry order per section, when reordering helps; empty to keep the master's order.
    pub order: Vec<Order>,
    /// A summary line built only from facts on the resume; `None` to leave the resume without one.
    pub summary: Option<String>,
    pub gaps: Vec<Gap>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Strong fit as is.
    Apply,
    /// A fit once these edits are made.
    Tailor,
    /// Misses a must-have that edits can't fix.
    Stretch,
    /// Fails a hard requirement (years, degree, clearance, location).
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    /// A `Requirements.skills[].term`, copied exactly.
    pub skill: String,
    pub have: Have,
    /// Lines that back it (`yes`: where it's stated; `add`: what supports it in substance); empty for `no`.
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Have {
    /// The resume states it now.
    Yes,
    /// The resume backs it in substance but never states the posting's term.
    Add,
    /// The resume can't back it.
    No,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Keyword {
    pub op: KeywordOp,
    /// The note line or bullet it changes.
    pub line: String,
    /// The posting's words: appended (`add`), swapped in (`edit`), or cut (`drop`).
    pub term: String,
    /// `edit` only: the line's current words, copied exactly; `""` otherwise.
    pub from: String,
    /// The requirement skill it states, or `""`.
    pub skill: String,
    pub priority: u8,
    pub why: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum KeywordOp {
    Add,
    Edit,
    Drop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BulletEdit {
    pub op: BulletOp,
    /// The bullet rewritten or cut, or for `add` the entry it goes under.
    pub id: String,
    /// The bullet's current text, copied exactly; `""` for `add`.
    pub before: String,
    /// The new text; `""` for `cut`.
    pub after: String,
    /// Requirement terms `after` puts on the page.
    pub hits: Vec<String>,
    /// Lines whose facts `after` draws on (the grounding validator checks against them).
    pub sources: Vec<String>,
    /// Index of one of the bullet's own variants used as `after`, when the model picked one.
    pub variant: Option<u32>,
    pub priority: u8,
    pub why: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BulletOp {
    Rewrite,
    Add,
    Cut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillEdit {
    pub op: SkillOp,
    /// The skills line it changes (for `move`, the line the item leaves).
    pub group: String,
    /// The item added, removed, renamed or moved, in the posting's words when adding.
    pub item: String,
    /// `rename`: the new wording; `move`: the destination skills line; `""` otherwise.
    pub to: String,
    /// The requirement skill it states, or `""`.
    pub skill: String,
    pub priority: u8,
    pub why: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SkillOp {
    Add,
    Remove,
    Rename,
    Move,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BankEdit {
    pub op: BankOp,
    /// The bank entry to show.
    pub entry: String,
    /// Which of its bullets to show; empty for all.
    pub bullets: Vec<String>,
    /// `swap`: the master entry it replaces, in the same place; `""` for `include`.
    pub replaces: String,
    /// Requirement skills it puts on the page.
    pub skills: Vec<String>,
    pub why: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BankOp {
    Include,
    Swap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Order {
    pub section: String,
    /// Every entry of the section, in the new order.
    pub entries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Gap {
    pub gap: String,
    /// One honest thing to do before applying or to say in the interview.
    pub fix: String,
    /// The requirement skill it is (`have: no`), or `""` (years, degree, location).
    pub skill: String,
}
