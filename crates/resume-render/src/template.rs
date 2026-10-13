//! Templates: a `.typ` source plus its manifest (ARCHITECTURE.md §6, template contract).

use std::path::Path;

use serde::{Deserialize, Serialize};

/// What a template declares about itself in `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub name: String,
    /// Single column of real text that ATS parsers read in order.
    pub ats_safe: bool,
    pub columns: u32,
}

/// A template's source and manifest, held in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    pub source: String,
    pub manifest: Manifest,
}

impl Template {
    /// `templates/classic-serif`, built in.
    pub fn classic_serif() -> Self {
        Self {
            source: include_str!("../../../templates/classic-serif/resume.typ").to_owned(),
            manifest: serde_json::from_str(include_str!(
                "../../../templates/classic-serif/manifest.json"
            ))
            .expect("classic-serif manifest is valid"),
        }
    }

    /// The template in `dir`: `resume.typ` and `manifest.json`.
    pub fn load(dir: &Path) -> std::io::Result<Self> {
        let source = std::fs::read_to_string(dir.join("resume.typ"))?;
        let manifest = serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json"))?)?;
        Ok(Self { source, manifest })
    }
}
