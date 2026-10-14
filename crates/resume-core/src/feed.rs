//! A job as the feed's sources list it, shared by the readers and the store (ARCHITECTURE.md §7, Job feed).

use serde::{Deserialize, Serialize};

use crate::salary::Salary;

/// Where the feed found a job. Rows stored before there was more than freehire are freehire's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    /// freehire's public API: every board it collects.
    #[default]
    Freehire,
    /// LinkedIn's guest job search.
    Linkedin,
    /// A company's own board, polled directly (Settings → Feed → Watchlist).
    Watchlist,
    /// Fantastic.jobs' career-site index, through its Apify actor.
    Fantastic,
}

impl Origin {
    /// Every origin, in the order the feed lists them.
    pub const ALL: [Self; 4] = [
        Self::Freehire,
        Self::Linkedin,
        Self::Watchlist,
        Self::Fantastic,
    ];

    /// The name a person reads ("LinkedIn", "Watchlist").
    pub fn label(self) -> &'static str {
        match self {
            Self::Freehire => "freehire",
            Self::Linkedin => "LinkedIn",
            Self::Watchlist => "Watchlist",
            Self::Fantastic => "Fantastic.jobs",
        }
    }
}

/// One job as a source lists it (ARCHITECTURE.md §7, Job feed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedJob {
    #[serde(default)]
    pub origin: Origin,
    /// The feed's key: freehire's public slug (never has a `:`), else `{source}:{board}:{id}` for a board's job and
    /// `{origin}:{id}` otherwise (`linkedin:4477858480`, `fantastic:1234`).
    pub slug: String,
    pub title: String,
    pub company: String,
    /// freehire's company (`/companies/{slug}`: its website, for the logo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub company_slug: Option<String>,
    pub location: Option<String>,
    /// The employer's link, `utm_*` parameters removed. LinkedIn's job page for a LinkedIn job: its guest page doesn't
    /// show the employer's.
    pub url: String,
    /// The site or ATS the job is on (`greenhouse`, `workday`, `linkedin`).
    pub source: String,
    /// `{board}:{id}` on the job's ATS when the origin names it (freehire's `wing:8872476002`).
    pub external_id: Option<String>,
    /// Skills in freehire's vocabulary (`cpp`, `nextjs`); empty when the origin lists none.
    pub skills: Vec<String>,
    /// The employer's posting date (RFC 3339), when the source states one.
    pub posted_at: Option<String>,
    /// When the origin first recorded it (RFC 3339): freehire's and Fantastic.jobs' own times, else when a pull
    /// first saw it.
    pub found_at: String,
    pub seniority: Option<String>,
    pub employment_type: Option<String>,
    pub work_mode: Option<String>,
    pub years_min: Option<u32>,
    pub salary: Option<Salary>,
    /// The posting's text as the origin lists it (Fantastic.jobs), read when the employer's page can't be.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl FeedJob {
    /// The job's page at its origin: freehire's page, LinkedIn's job page, else the employer's link.
    pub fn page_url(&self) -> String {
        match self.origin {
            Origin::Freehire => format!("https://freehire.me/jobs/{}", self.slug),
            Origin::Linkedin | Origin::Watchlist | Origin::Fantastic => self.url.clone(),
        }
    }
}
