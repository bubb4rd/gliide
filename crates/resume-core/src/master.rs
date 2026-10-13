//! A master: a selection from the bank, holding no text of its own (ARCHITECTURE.md §2).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which sections, entries and bullets a resume shows, in what order, which variant of each bullet, which skills, under
/// what title and template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Master {
    pub id: String,
    /// The track, e.g. `Software Engineering`.
    pub title: String,
    /// Template name, e.g. `classic-serif`.
    pub template: String,
    #[schemars(range(min = 1))]
    pub page_limit: u32,
    pub sections: Vec<MasterSection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MasterSection {
    /// A bank section id.
    pub id: String,
    /// Heading override; the bank section's title when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<MasterEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<MasterGroup>,
}

/// An entry of the section, with the bullets shown under it (none when empty). Its notes are always shown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MasterEntry {
    pub id: String,
    #[serde(default)]
    pub bullets: Vec<MasterBullet>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MasterBullet {
    pub id: String,
    /// Index into the bullet's `variants`; its `text` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<usize>,
}

/// A skills line of the section with the items shown, in order; each must be one of the bank line's items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MasterGroup {
    pub id: String,
    pub items: Vec<String>,
}
