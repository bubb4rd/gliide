//! The resolved resume: the v1 master JSON shape, plus the build-only fields a template reads for the fit
//! check and the review's highlight. Templates read this as `sys.inputs.data`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use crate::bank::{Contact, SectionKind};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Resume {
    pub name: String,
    pub contact: Vec<Contact>,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub id: String,
    pub title: String,
    pub entries: Vec<Entry>,
    pub groups: Vec<Group>,
    /// Scoring only (templates ignore it): the bank section's kind, filled by `resolve`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<SectionKind>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub url: String,
    pub place: String,
    pub dates: String,
    pub subtitle: String,
    pub notes: Vec<Note>,
    pub bullets: Vec<Bullet>,
    /// Scoring only (templates ignore it): the bank entry's first month, `YYYY-MM`, filled by `resolve`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// Scoring only (templates ignore it): the bank entry's last month, `YYYY-MM` or `present`, filled by `resolve`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub label: String,
    pub text: String,
    /// Build only: the tailoring changed this line (highlighted in the preview).
    #[serde(default, skip_serializing_if = "is_false")]
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Bullet {
    pub id: String,
    pub text: String,
    /// Build only: the master text this bullet replaces, the line budget the fit check measures against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orig: Option<String>,
    /// Build only: the tailoring changed this bullet (highlighted in the preview).
    #[serde(default, skip_serializing_if = "is_false")]
    pub changed: bool,
    /// Build only: a bullet the tailoring added.
    #[serde(default, skip_serializing_if = "is_false")]
    pub new: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub id: String,
    pub label: String,
    pub items: Vec<String>,
    /// Build only: the master's items, when the tailoring edited this line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orig: Option<Vec<String>>,
    /// Build only: items the tailoring added (highlighted in the preview).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added: Vec<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}
