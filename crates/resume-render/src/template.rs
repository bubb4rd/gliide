//! Templates: a `.typ` source plus its manifest (ARCHITECTURE.md §6, template contract). A built-in also has a cover
//! letter, `letter.typ`, under the same letterhead.

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

/// A built-in's files: `resume.typ`, `letter.typ`, `manifest.json`.
struct Files {
    resume: &'static str,
    letter: &'static str,
    manifest: &'static str,
}

macro_rules! files {
    ($name:literal) => {
        Files {
            resume: include_str!(concat!("../../../templates/", $name, "/resume.typ")),
            letter: include_str!(concat!("../../../templates/", $name, "/letter.typ")),
            manifest: include_str!(concat!("../../../templates/", $name, "/manifest.json")),
        }
    };
}

fn files(name: &str) -> Option<Files> {
    match name {
        "classic-serif" => Some(files!("classic-serif")),
        "modern-sans" => Some(files!("modern-sans")),
        _ => None,
    }
}

impl Template {
    /// The built-in templates' names, the default first.
    pub const BUILT_IN: [&str; 2] = ["classic-serif", "modern-sans"];

    /// The built-in template named `name`.
    pub fn built_in(name: &str) -> Option<Self> {
        let f = files(name)?;
        Some(Self::of(f.resume, f.manifest))
    }

    /// The cover letter of the built-in template named `name`: its letterhead, its type. Its data is
    /// [`LetterData`](crate::cover::LetterData).
    pub fn built_in_letter(name: &str) -> Option<Self> {
        let f = files(name)?;
        Some(Self::of(f.letter, f.manifest))
    }

    /// `templates/classic-serif`, built in.
    pub fn classic_serif() -> Self {
        Self::built_in("classic-serif").expect("classic-serif is built in")
    }

    fn of(source: &str, manifest: &str) -> Self {
        Self {
            source: source.to_owned(),
            manifest: serde_json::from_str(manifest).expect("a built-in manifest is valid"),
        }
    }

    /// The template in `dir`: `resume.typ` and `manifest.json`.
    pub fn load(dir: &Path) -> std::io::Result<Self> {
        let source = std::fs::read_to_string(dir.join("resume.typ"))?;
        let manifest = serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json"))?)?;
        Ok(Self { source, manifest })
    }
}
