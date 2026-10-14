//! A job posting as read: the text every later step works from, and where it came from (ARCHITECTURE.md §7).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::salary::Salary;

/// How the posting's text was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Method {
    /// Text the user pasted.
    Pasted,
    /// Greenhouse job board API.
    Greenhouse,
    /// Lever postings API.
    Lever,
    /// Ashby's job page (the app data it embeds: company, title, locations, description).
    Ashby,
    /// LinkedIn's public job page (guest API).
    Linkedin,
    /// Workday's career-site job data (`/wday/cxs/…`).
    Workday,
    /// SmartRecruiters' public Posting API.
    #[serde(rename = "smartrecruiters")]
    SmartRecruiters,
    /// Workable's public job API.
    Workable,
    /// Recruitee's public careers API.
    Recruitee,
    /// A schema.org `JobPosting` JSON-LD block in the page.
    JsonLd,
    /// The page's main text, without navigation, footers and forms.
    Html,
    /// freehire's copy of a posting, for sources gliide can't read itself.
    Freehire,
    /// Fantastic.jobs' copy of a posting, for career sites gliide can't read itself.
    Fantastic,
}

/// A posting snapshot. `hash` identifies the text (and keys the Extract cache), so the same text read twice is the
/// same posting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Posting {
    /// The link as the user gave it, when there was one.
    pub source: Option<String>,
    /// The canonical link the text was read from (a LinkedIn search link becomes the job's own page).
    pub url: Option<String>,
    pub method: Method,
    /// From the source's metadata when it has any; passed to Extract as hints.
    pub title: Option<String>,
    pub company: Option<String>,
    pub location: Option<String>,
    /// The pay the source states in its own fields (a JSON-LD `baseSalary`, Lever's `salaryRange`); the text's is
    /// read when it's shown ([`crate::salary::find`]). Postings read before it existed have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salary: Option<Salary>,
    /// The posting's text, as plain text with list items on their own lines.
    pub text: String,
    /// Lowercase hex SHA-256 of `text`.
    pub hash: String,
}

impl Posting {
    /// A posting from text and its metadata; computes `hash`.
    pub fn new(
        method: Method,
        source: Option<String>,
        url: Option<String>,
        title: Option<String>,
        company: Option<String>,
        location: Option<String>,
        text: String,
    ) -> Self {
        let hash = text_hash(&text);
        Self {
            source,
            url,
            method,
            title,
            company,
            location,
            salary: None,
            text,
            hash,
        }
    }

    /// Pasted text with no metadata.
    pub fn pasted(text: String) -> Self {
        Self::new(Method::Pasted, None, None, None, None, None, text)
    }
}

/// Lowercase hex SHA-256 of `text`.
pub fn text_hash(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
