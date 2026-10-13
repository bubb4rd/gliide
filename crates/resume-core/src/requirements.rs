//! What a posting asks for: the Extract call's output (ARCHITECTURE.md §3). Master-independent; cached per posting.
//!
//! `prompts/extract.schema.json` must accept exactly this shape (a test in `resume-llm` checks the recorded fixtures
//! deserialize into it).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requirements {
    pub role: String,
    pub company: String,
    /// The role's short name for file names (`swe`, `it_support`).
    pub short_role: String,
    pub location: String,
    pub seniority: Seniority,
    /// 8–20 skills, most important first, in the posting's words.
    pub skills: Vec<Skill>,
    /// Words of the job title an ATS matches on.
    pub title_terms: Vec<String>,
    pub years_min: Option<u32>,
    pub degree: Option<Degree>,
    pub certifications: Vec<String>,
    pub knockouts: Vec<Knockout>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Seniority {
    Intern,
    Entry,
    Mid,
    Senior,
    Lead,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Skill {
    /// The posting's exact words.
    pub term: String,
    /// Other forms an ATS would also match (`JS` for `JavaScript`, the noun for a verb the posting used).
    pub aliases: Vec<String>,
    pub kind: SkillKind,
    /// 1 required or repeated, 2 preferred, 3 nice to have. Weighs 3, 2, 1 in the score.
    pub priority: u8,
    pub required: bool,
    /// Skills sharing a group are one-of alternatives ("React, Vue, or Svelte"): any one of them meets the group.
    /// `None` for a skill that stands alone.
    pub group: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SkillKind {
    Hard,
    Tool,
    Domain,
    Soft,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Degree {
    pub level: DegreeLevel,
    pub fields: Vec<String>,
    /// The posting accepts equivalent experience instead.
    pub equivalent: bool,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum DegreeLevel {
    Associate,
    Bachelor,
    Master,
    Phd,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Knockout {
    pub kind: KnockoutKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum KnockoutKind {
    Clearance,
    Authorization,
    Onsite,
    Other,
}
