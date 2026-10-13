//! The experience bank: everything you've done, in one document (ARCHITECTURE.md §2).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The bank format version this crate reads and writes.
pub const BANK_SCHEMA: u32 = 2;

/// Every job, project, school, certification and skills line you have, whether or not a master shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Bank {
    /// Format version, always 2.
    #[schemars(range(min = 2, max = 2))]
    pub schema: u32,
    pub name: String,
    pub contact: Vec<Contact>,
    pub sections: Vec<BankSection>,
}

/// One item of the contact line: shown text, and a link (`""` for none, `mailto:` for an email).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Contact {
    pub text: String,
    pub url: String,
}

/// Where a section's content lives for scoring and the standard-heading check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SectionKind {
    Summary,
    Experience,
    Projects,
    Education,
    Skills,
    Certifications,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BankSection {
    pub id: String,
    pub kind: SectionKind,
    /// The printed heading, unless a master overrides it.
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<BankEntry>,
    /// Skills lines.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<BankGroup>,
}

/// A job, project, school or certification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BankEntry {
    pub id: String,
    pub name: String,
    /// Link on the name, `""` for none.
    pub url: String,
    pub place: String,
    /// The dates as printed, e.g. `Aug 2022 – May 2026`.
    pub dates: String,
    /// First month, `YYYY-MM`; null when the printed dates couldn't be read.
    #[serde(default)]
    #[schemars(regex(pattern = r"^[0-9]{4}-(0[1-9]|1[0-2])$"))]
    pub start: Option<String>,
    /// Last month, `YYYY-MM`, or `present`; null when the printed dates couldn't be read.
    #[serde(default)]
    #[schemars(regex(pattern = r"^([0-9]{4}-(0[1-9]|1[0-2])|present)$"))]
    pub end: Option<String>,
    pub subtitle: String,
    /// Tracks this entry belongs to (`swe`, `design`, `it`…): editor filters and a hint for candidates.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Labeled lines under the entry (an education entry's honors or coursework).
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default)]
    pub bullets: Vec<BankBullet>,
}

/// A line under an entry, `label: text` (or just `text` when the label is `""`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub label: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BankBullet {
    pub id: String,
    pub text: String,
    /// Other phrasings you wrote and approved; a master picks one by index.
    #[serde(default)]
    pub variants: Vec<String>,
    /// Skills you say this bullet demonstrates.
    #[serde(default)]
    pub skills: Vec<String>,
}

/// A skills line: `label: item, item, …`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BankGroup {
    pub id: String,
    pub label: String,
    pub items: Vec<String>,
}
