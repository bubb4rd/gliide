//! The benchmark: each role's calls on the fixture postings, on any route, and the score code gives them. `ez eval`
//! records runs on disk and reports on them; the app's Settings → Benchmark runs the same calls and keeps the runs in
//! its store. Neither computes a score itself: this crate does.
//!
//! A run is one call as the app makes it ([`run_one`]): Read extracts a posting's requirements; Tailor suggests
//! changes to the trimmed `swe` master (bank-aware, so one bank entry is left to suggest), then asks once to fit each
//! rewrite that wraps long; Write writes a cover letter from the resolved `swe` master; Coach writes the prep for a
//! technical round. Every input other than the posting is its gold requirements. [`metrics`] measures a run's output
//! on its own; [`aggregate`] sums a route's runs; [`score`] turns that into the 0–100 numbers the pane shows.
//!
//! # Scoring
//!
//! - Quality, 0–100, from each route's valid runs (valid first try or after the repair turn):
//!   - Read: 100 × the mean of term recall, term precision and must-have recall against gold, each a mean over runs.
//!   - Tailor (the code checks): 100 × the mean of grounded (1 − Σflagged ÷ Σchanges), fits (1 − Σleft out by the
//!     fit check ÷ Σchanges) and lift (the mean of projected − now over 20 points, held to 0–1). Nothing proposed is
//!     grounded and fits.
//!   - Coach: Prep's grounded share. The unit is a claim tied to the resume: a line id a question or gap cites, a
//!     gap-answer sentence, a story named. Kept: those left after `prep::checked`; dropped: the line ids, sentences
//!     and stories it took out. A gap dropped for not being a must-have the page lacks is relevance, not grounding,
//!     and counts on neither side. No claim at all is no quality.
//!   - Write: none yet.
//! - Reliability: 100 × (valid first try + ½ × repaired) ÷ runs; a failed run counts 0.
//! - Speed: 100 while the p50 (nearest rank over every run, failed included) is at most [`target_s`], then falling in
//!   a line to 0 at four times it.
//! - Composite: 0.6 × quality + 0.2 × reliability + 0.2 × speed; none without a quality.
//! - Kept edits (Tailor only, from the app's exports): [`kept_factor`] pulls a route's kept share toward your average
//!   over every route by 10 decisions' worth of it, and divides by the average; quality is the code checks ×
//!   (0.75 + 0.25 × factor), at most 100.
//! - Ranking ([`rank`]): by the unrounded composite; routes without one last, by p50.
//!
//! # Against the build plan's contract
//!
//! - [`run_one`] takes the run number `n` too, so the [`Run`] it returns is whole.
//! - [`rank`] returns `(id, rank)` in display order, the rank `None` for a route without a composite, so equal
//!   composites can share one.
//! - [`RouteResult`] has `items`, the postings its runs covered (the reference has routes run on 4 of 12).
//! - [`Part`]'s id is a `String`, so a [`Reference`] parses.

mod coach;
mod fixtures;
mod read;
mod reference;
mod run;
mod score;
mod tailor;
mod write;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use resume_core::ResolveError;
use resume_pipeline::PipelineError;
use resume_tailor::TailorError;
use serde::{Deserialize, Serialize};

pub use coach::{ROUND, checked as prep_checked, grounding as prep_grounding};
pub use fixtures::{Fixtures, GOLD_DIR, Item};
pub use read::{Overlap, overlap, same_skill};
pub use reference::{Reference, reference};
pub use run::{Called, Outcome, Run, TokenCounts, call, run_one};
pub use score::{
    Kept, Part, Quality, RouteResult, RunMetrics, RunSummary, Score, aggregate, kept_factor, mean,
    metrics, percentile, rank, score,
};
pub use tailor::{LeftOut, Reviewed, master_of, review, suggest};
pub use write::{job, words};

/// What a route is benchmarked on: the app's four kinds of work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Read,
    Tailor,
    Write,
    Coach,
}

impl Role {
    pub const ALL: [Role; 4] = [Role::Read, Role::Tailor, Role::Write, Role::Coach];

    /// `read`, `tailor`, `write` or `coach`: its folder in `ez eval`'s runs.
    pub fn name(self) -> &'static str {
        match self {
            Role::Read => "read",
            Role::Tailor => "tailor",
            Role::Write => "write",
            Role::Coach => "coach",
        }
    }
}

/// The p50 in seconds at which a role's speed starts to drop below 100: about what the app's screen can wait for.
pub fn target_s(role: Role) -> f64 {
    match role {
        Role::Read => 8.0,
        Role::Tailor => 30.0,
        Role::Write => 15.0,
        Role::Coach => 20.0,
    }
}

/// How long one call may take before the agent is stopped: Read's posting is short, the others write at length.
pub fn timeout(role: Role) -> Duration {
    match role {
        Role::Read => Duration::from_secs(180),
        _ => Duration::from_secs(300),
    }
}

/// How much a benchmark runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Plan {
    /// [`QUICK`]'s 4 postings, once each.
    Quick,
    /// Every posting, 3 runs for Read and Tailor (their outputs vary most), 1 for Write and Coach.
    Full,
}

/// The postings a quick plan runs: the 4 the reference's slowest routes ran on, so their numbers compare.
pub const QUICK: [&str; 4] = [
    "hooli-fullstack-swe",
    "northwind-jr-swe",
    "stark-ux-engineer",
    "vandelay-systems-automation",
];

impl Plan {
    /// Runs per posting for `role`.
    pub fn runs(self, role: Role) -> u32 {
        match (self, role) {
            (Plan::Quick, _) | (Plan::Full, Role::Write | Role::Coach) => 1,
            (Plan::Full, Role::Read | Role::Tailor) => 3,
        }
    }
}

/// What `plan` runs for `role`: each posting id and its runs.
pub fn items(fx: &Fixtures, role: Role, plan: Plan) -> Vec<(String, u32)> {
    let runs = plan.runs(role);
    fx.postings
        .iter()
        .filter(|p| plan == Plan::Full || QUICK.contains(&p.id.as_str()))
        .map(|p| (p.id.clone(), runs))
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum BenchError {
    #[error("fixture {name}: {message}")]
    Fixture { name: String, message: String },
    #[error("no posting `{0}` with gold requirements")]
    UnknownItem(String),
    #[error("output does not decode: {0}")]
    Output(#[from] serde_json::Error),
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    Pipeline(#[from] PipelineError),
    #[error(transparent)]
    Tailor(#[from] TailorError),
}

/// Today in UTC, `YYYY-MM-DD`: what Prep is told the date is.
pub fn date() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400) as i64;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm), as `resume_pipeline::today` computes it.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}
