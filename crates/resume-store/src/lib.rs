//! The app's store: one SQLite file holding bank and master revisions, postings, Extract and Suggest output, drafts,
//! cover letters, applications and their interview rounds, company logos, the job feed and settings (ARCHITECTURE.md
//! §8).
//!
//! Revisions are immutable and keyed by content hash, so an analysis records exactly the bank and master it ran
//! against. Rows carry UUIDs and RFC 3339 `updated_at` so sync can be added without a migration. JSON columns hold
//! the serde JSON of the `resume-core` types.

mod csv;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use resume_core::change::{Draft, DraftEvent, Mode};
use resume_core::feed::FeedJob;
use resume_core::posting::text_hash;
use resume_core::salary::{self, Salary};
use resume_core::suggestions::Suggestions;
use resume_core::{Bank, Master, Posting, Requirements};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use csv::{applications_csv, parse_ats_fit_csv};

/// Schema migrations in order; `PRAGMA user_version` counts the ones applied.
const MIGRATIONS: &[&str] = &[
    include_str!("schema.sql"),
    include_str!("covers.sql"),
    include_str!("companies.sql"),
    include_str!("salary.sql"),
    include_str!("feed.sql"),
    include_str!("logos.sql"),
    include_str!("places.sql"),
    include_str!("interviews.sql"),
    include_str!("rounds.sql"),
    include_str!("practice.sql"),
];

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("store: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("store JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("store folder: {0}")]
    Io(#[from] std::io::Error),
    #[error("no {what} `{id}` in the store")]
    NotFound { what: &'static str, id: String },
    #[error(
        "the data is from a newer gliide (schema version {found}; this app reads up to {known}): install the latest version"
    )]
    TooNew { found: i64, known: i64 },
}

/// A bank or master revision: the full lowercase hex SHA-256 of its JSON. Displays as its first 12 digits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rev(pub String);

impl Rev {
    /// The revision of a document: the hash of its serde JSON (fields in the type's order, so equal documents hash
    /// equal).
    pub fn of<T: Serialize>(doc: &T) -> Result<Self, StoreError> {
        Ok(Self(text_hash(&serde_json::to_string(doc)?)))
    }
}

impl fmt::Display for Rev {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.get(..12).unwrap_or(&self.0))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostingRow {
    pub id: String,
    pub posting: Posting,
    pub created_at: String,
    /// When it was last read; a re-read of the same text refreshes it.
    pub updated_at: String,
    /// The salary range you set: `None` when you haven't, `Some(None)` when you removed it.
    pub your_salary: Option<Option<Salary>>,
}

impl PostingRow {
    /// The posting's salary range: yours once you've set or removed it, else the source's, else the one its text
    /// states.
    pub fn salary(&self) -> Option<Salary> {
        match &self.your_salary {
            Some(yours) => yours.clone(),
            None => self
                .posting
                .salary
                .clone()
                .or_else(|| salary::find(&self.posting.text)),
        }
    }
}

/// A Suggest result to store as an analysis.
#[derive(Debug, Clone, Copy)]
pub struct NewAnalysis<'a> {
    pub posting_id: &'a str,
    pub bank_rev: &'a Rev,
    pub master_id: &'a str,
    pub master_rev: &'a Rev,
    pub mode: Mode,
    /// `resume_pipeline::suggest_key` of the Suggest input.
    pub key: &'a str,
    pub model: &'a str,
    pub suggestions: &'a Suggestions,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisRow {
    pub id: String,
    pub posting_id: String,
    pub bank_rev: Rev,
    pub master_id: String,
    pub master_rev: Rev,
    pub mode: Mode,
    pub model: String,
    pub suggestions: Suggestions,
    /// The projected score, for the posting's fit score; `None` until the review computed it.
    pub projected: Option<f64>,
    pub created_at: String,
    /// When the review was last opened.
    pub opened_at: String,
}

/// A draft as stored: its event log and the undo cursor. `events[..cursor]` are in effect; the rest are redo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DraftLog {
    pub events: Vec<DraftEvent>,
    pub cursor: usize,
}

impl DraftLog {
    /// The draft the events in effect make.
    pub fn draft(&self) -> Draft {
        Draft::replay(&self.events[..self.cursor.min(self.events.len())])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppStatus {
    Saved,
    Applied,
    Interviewing,
    Offer,
    Rejected,
}

impl AppStatus {
    /// The lowercase name, as stored and in `applications.csv`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Saved => "saved",
            Self::Applied => "applied",
            Self::Interviewing => "interviewing",
            Self::Offer => "offer",
            Self::Rejected => "rejected",
        }
    }

    /// The status named `name` (lowercase, as [`AppStatus::as_str`] writes it).
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "saved" => Self::Saved,
            "applied" => Self::Applied,
            "interviewing" => Self::Interviewing,
            "offer" => Self::Offer,
            "rejected" => Self::Rejected,
            _ => return None,
        })
    }
}

/// A job applied to (or about to be). Scores are percentages: `now` the master as is, `projected` with the draft,
/// `on_file` read back from the exported PDF.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Application {
    pub id: String,
    /// `None` for rows imported from an existing `applications.csv`, or once the posting is deleted.
    pub posting_id: Option<String>,
    pub analysis_id: Option<String>,
    /// `YYYY-MM-DD`.
    pub date: String,
    pub company: String,
    pub role: String,
    pub url: Option<String>,
    pub status: AppStatus,
    pub now: Option<f64>,
    pub projected: Option<f64>,
    pub on_file: Option<f64>,
    pub resume_path: Option<String>,
    pub cover_path: Option<String>,
    pub notes: String,
    pub updated_at: String,
}

/// An application's status from `at` (RFC 3339) on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusChange {
    pub status: AppStatus,
    pub at: String,
}

/// An application's interview round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Round {
    pub id: String,
    pub application_id: String,
    /// The app's name for its kind (`technical`, `recruiter-screen`, …).
    pub kind: String,
    /// When it starts, RFC 3339 with the offset it was given in; `None` while not scheduled.
    pub starts_at: Option<String>,
    /// Its length; `None` when not known (an hour, where its end matters).
    pub minutes: Option<u32>,
    pub with: String,
    /// A meeting link or an address.
    pub place: String,
    pub notes: String,
    pub state: RoundState,
    /// Set by the store on every write (the one given is ignored).
    pub updated_at: String,
}

/// A round's state: upcoming (scheduled or not; one whose end has passed is over), or cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoundState {
    Upcoming,
    Cancelled,
}

impl RoundState {
    /// The lowercase name, as stored.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Upcoming => "upcoming",
            Self::Cancelled => "cancelled",
        }
    }

    /// The state named `name` (as [`RoundState::as_str`] writes it).
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "upcoming" => Self::Upcoming,
            "cancelled" => Self::Cancelled,
            _ => return None,
        })
    }
}

/// A round's prep as written: its JSON, the model that wrote it, when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prep {
    /// What it was written from, hashed (the caller's).
    pub key: String,
    pub round_id: String,
    pub json: String,
    pub model: String,
    /// Set by the store when written (the one given is ignored).
    pub at: String,
}

/// One practiced answer and the feedback that came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PracticeRow {
    pub id: String,
    pub round_id: String,
    /// The prep question's id.
    pub question: String,
    /// The question as asked.
    pub asked: String,
    pub answer: String,
    /// The feedback, JSON.
    pub feedback: String,
    /// Set by the store when added (the one given is ignored).
    pub at: String,
}

/// An analysis's cover letter: its parts as last written or edited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cover {
    pub id: String,
    pub analysis_id: String,
    pub greeting: String,
    pub paragraphs: Vec<String>,
    pub closing: String,
    pub updated_at: String,
}

/// A company's logo as stored, by company key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Logo {
    pub bytes: Vec<u8>,
    /// `image/png` or `image/svg+xml`.
    pub media_type: String,
    /// When it was set; every new image gets a new one.
    pub updated_at: String,
    pub source: LogoSource,
}

/// Where a stored logo came from: you, or a fetch (freehire's logo for the name, or the company's own site).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogoSource {
    Yours,
    Freehire,
    Site,
}

impl LogoSource {
    /// The lowercase name, as stored.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Yours => "yours",
            Self::Freehire => "freehire",
            Self::Site => "site",
        }
    }
}

/// What a company's last logo fetch found; `Removed`: you removed the logo it fetched, so it isn't fetched again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogoFound {
    Freehire,
    Site,
    #[serde(rename = "none")]
    Nothing,
    Removed,
}

impl LogoFound {
    /// The lowercase name, as stored.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Freehire => "freehire",
            Self::Site => "site",
            Self::Nothing => "none",
            Self::Removed => "removed",
        }
    }
}

/// A company's last logo fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogoLookup {
    pub found: LogoFound,
    /// RFC 3339.
    pub tried_at: String,
}

/// A geocoding query's last lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceLookup {
    /// The place found, as JSON; `None` when nothing was.
    pub json: Option<String>,
    /// RFC 3339.
    pub tried_at: String,
}

/// Where a feed job is: found and waiting for its check, left out by the local check or a failed read, over the day's
/// cap, checked, or dismissed by you.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeedState {
    New,
    Skipped,
    Waiting,
    Checked,
    Dismissed,
}

impl FeedState {
    /// The lowercase name, as stored.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Skipped => "skipped",
            Self::Waiting => "waiting",
            Self::Checked => "checked",
            Self::Dismissed => "dismissed",
        }
    }
}

/// A feed job as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedRow {
    pub job: FeedJob,
    pub state: FeedState,
    /// The local check's order (higher first).
    pub overlap: f64,
    pub posting_id: Option<String>,
    /// Best master's score now, once checked.
    pub score: Option<f64>,
    pub master_id: Option<String>,
    pub checked_at: Option<String>,
    /// You opened its posting: from then on it's in Recent jobs and Applications.
    pub opened_at: Option<String>,
    /// Why it was skipped, or why its check failed.
    pub reason: Option<String>,
    pub created_at: String,
}

/// A job a pull or a search found, with the local check's verdict.
#[derive(Debug, Clone, PartialEq)]
pub struct NewFeedJob {
    pub job: FeedJob,
    pub state: FeedState,
    pub overlap: f64,
    pub reason: Option<String>,
}

/// The store: one SQLite connection. Not `Sync`; share it behind a mutex.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Opens (or creates) the store at `path`, creating its folders, and migrates it to the current schema.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    /// An empty store in memory, for tests.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, StoreError> {
        conn.busy_timeout(Duration::from_secs(5))?;
        // In memory, journal_mode stays `memory`; the answer doesn't matter.
        conn.pragma_update_and_check(None, "journal_mode", "wal", |r| r.get::<_, String>(0))?;
        conn.pragma_update(None, "synchronous", "normal")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), StoreError> {
        let version: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?;
        let known = MIGRATIONS.len() as i64;
        if version > known {
            return Err(StoreError::TooNew {
                found: version,
                known,
            });
        }
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            let tx = self.conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", i as i64 + 1)?;
            tx.commit()?;
        }
        Ok(())
    }

    // Bank and masters.

    /// Stores `bank` as the current bank. An identical document returns its existing revision.
    pub fn put_bank(&self, bank: &Bank) -> Result<Rev, StoreError> {
        let rev = Rev::of(bank)?;
        let tx = self.conn.unchecked_transaction()?;
        let current: Option<String> = tx
            .query_row("SELECT hash FROM bank_revs WHERE current = 1", [], |r| {
                r.get(0)
            })
            .optional()?;
        if current.as_deref() != Some(&rev.0) {
            let now = now();
            tx.execute("UPDATE bank_revs SET current = 0 WHERE current = 1", [])?;
            tx.execute(
                "INSERT INTO bank_revs (id, hash, json, current, created_at, updated_at) VALUES (?1, ?2, ?3, 1, ?4, ?4)
                 ON CONFLICT (hash) DO UPDATE SET current = 1, updated_at = excluded.updated_at",
                params![new_id(), rev.0, serde_json::to_string(bank)?, now],
            )?;
        }
        tx.commit()?;
        Ok(rev)
    }

    /// The current bank.
    pub fn bank(&self) -> Result<Option<(Rev, Bank)>, StoreError> {
        let row: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT hash, json FROM bank_revs WHERE current = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        row.map(|(hash, json)| Ok((Rev(hash), from_json(&json)?)))
            .transpose()
    }

    /// When the current bank became current (RFC 3339).
    pub fn bank_updated_at(&self) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT updated_at FROM bank_revs WHERE current = 1",
                [],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Stores `master` as the current revision of `master.id`. An identical document returns its existing revision.
    pub fn put_master(&self, master: &Master) -> Result<Rev, StoreError> {
        let rev = Rev::of(master)?;
        let now = now();
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO masters (id, title, rev, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT (id) DO UPDATE SET title = excluded.title, rev = excluded.rev, updated_at = excluded.updated_at
             WHERE masters.rev <> excluded.rev",
            params![master.id, master.title, rev.0, now],
        )?;
        tx.execute(
            "INSERT INTO master_revs (id, master_id, hash, json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT (master_id, hash) DO NOTHING",
            params![new_id(), master.id, rev.0, serde_json::to_string(master)?, now],
        )?;
        tx.commit()?;
        Ok(rev)
    }

    /// The current revision of every master, by title.
    pub fn masters(&self) -> Result<Vec<(Rev, Master)>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT m.rev, r.json FROM masters m JOIN master_revs r ON r.master_id = m.id AND r.hash = m.rev
             ORDER BY m.title, m.id",
        )?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(hash, json)| Ok((Rev(hash), from_json(&json)?)))
            .collect()
    }

    /// The current revision of master `id`.
    pub fn master(&self, id: &str) -> Result<Option<(Rev, Master)>, StoreError> {
        let row: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT m.rev, r.json FROM masters m JOIN master_revs r ON r.master_id = m.id AND r.hash = m.rev
                 WHERE m.id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        row.map(|(hash, json)| Ok((Rev(hash), from_json(&json)?)))
            .transpose()
    }

    /// When master `id`'s current revision became current (RFC 3339).
    pub fn master_updated_at(&self, id: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT updated_at FROM masters WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()?)
    }

    // Postings.

    /// Stores `posting` and returns its id. The same text (by `posting.hash`) is the same posting: a re-read keeps
    /// its id and refreshes its metadata and `updated_at`.
    pub fn put_posting(&self, posting: &Posting) -> Result<String, StoreError> {
        Ok(self.conn.query_row(
            "INSERT INTO postings (id, hash, json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT (hash) DO UPDATE SET json = excluded.json, updated_at = excluded.updated_at
             RETURNING id",
            params![new_id(), posting.hash, serde_json::to_string(posting)?, now()],
            |r| r.get(0),
        )?)
    }

    pub fn posting(&self, id: &str) -> Result<Option<PostingRow>, StoreError> {
        let row = self
            .conn
            .query_row(
                "SELECT id, json, created_at, updated_at, salary FROM postings WHERE id = ?1",
                [id],
                RawPosting::from_row,
            )
            .optional()?;
        row.map(RawPosting::parse).transpose()
    }

    /// Every posting, most recently read first.
    pub fn postings(&self) -> Result<Vec<PostingRow>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, json, created_at, updated_at, salary FROM postings ORDER BY updated_at DESC, rowid DESC",
        )?;
        let rows = stmt
            .query_map([], RawPosting::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(RawPosting::parse).collect()
    }

    /// Sets the posting's salary range as yours (`None`: removed). Its place in the lists stays: `updated_at` is
    /// when it was read.
    pub fn set_posting_salary(&self, id: &str, salary: Option<&Salary>) -> Result<(), StoreError> {
        let changed = self.conn.execute(
            "UPDATE postings SET salary = ?2 WHERE id = ?1",
            params![id, serde_json::to_string(&salary)?],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound {
                what: "posting",
                id: id.to_owned(),
            });
        }
        Ok(())
    }

    /// Deletes a posting with its requirements, analyses and drafts. Its application stays, without the links.
    pub fn delete_posting(&self, id: &str) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM postings WHERE id = ?1", [id])?;
        Ok(())
    }

    // Requirements.

    /// Extract output by `resume_pipeline::extract_key` and model.
    pub fn requirements(&self, key: &str, model: &str) -> Result<Option<Requirements>, StoreError> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM requirements WHERE key = ?1 AND model = ?2",
                [key, model],
                |r| r.get(0),
            )
            .optional()?;
        json.map(|j| from_json(&j)).transpose()
    }

    /// Stores Extract output; replaces any under the same key and model.
    pub fn put_requirements(
        &self,
        posting_id: &str,
        key: &str,
        model: &str,
        req: &Requirements,
        latency_ms: u64,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO requirements (id, posting_id, key, model, json, latency_ms, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
             ON CONFLICT (key, model) DO UPDATE SET posting_id = excluded.posting_id, json = excluded.json,
                 latency_ms = excluded.latency_ms, updated_at = excluded.updated_at",
            params![
                new_id(),
                posting_id,
                key,
                model,
                serde_json::to_string(req)?,
                to_i64(latency_ms),
                now()
            ],
        )?;
        Ok(())
    }

    /// The posting's most recently stored requirements, whatever their key and model.
    pub fn latest_requirements(
        &self,
        posting_id: &str,
    ) -> Result<Option<Requirements>, StoreError> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM requirements WHERE posting_id = ?1 ORDER BY updated_at DESC, rowid DESC LIMIT 1",
                [posting_id],
                |r| r.get(0),
            )
            .optional()?;
        json.map(|j| from_json(&j)).transpose()
    }

    // Analyses.

    /// The newest analysis stored under `resume_pipeline::suggest_key` and model.
    pub fn analysis(&self, key: &str, model: &str) -> Result<Option<AnalysisRow>, StoreError> {
        self.analysis_where(
            "key = ?1 AND model = ?2 ORDER BY created_at DESC, rowid DESC",
            &[key, model],
        )
    }

    /// Stores Suggest output as a new analysis, opened now, with an empty draft. Another analysis under the same key
    /// and model may exist (a fresh re-run); [`Store::analysis`] returns the newest.
    pub fn put_analysis(&self, new: &NewAnalysis) -> Result<AnalysisRow, StoreError> {
        let row = AnalysisRow {
            id: new_id(),
            posting_id: new.posting_id.to_owned(),
            bank_rev: new.bank_rev.clone(),
            master_id: new.master_id.to_owned(),
            master_rev: new.master_rev.clone(),
            mode: new.mode,
            model: new.model.to_owned(),
            suggestions: new.suggestions.clone(),
            projected: None,
            created_at: now(),
            opened_at: String::new(),
        };
        self.conn.execute(
            "INSERT INTO analyses (id, posting_id, bank_rev, master_id, master_rev, mode, key, model, suggestions,
                 latency_ms, created_at, opened_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, ?11)",
            params![
                row.id,
                row.posting_id,
                row.bank_rev.0,
                row.master_id,
                row.master_rev.0,
                mode_name(row.mode),
                new.key,
                row.model,
                serde_json::to_string(new.suggestions)?,
                to_i64(new.latency_ms),
                row.created_at,
            ],
        )?;
        Ok(AnalysisRow {
            opened_at: row.created_at.clone(),
            ..row
        })
    }

    pub fn analysis_by_id(&self, id: &str) -> Result<Option<AnalysisRow>, StoreError> {
        self.analysis_where("id = ?1", &[id])
    }

    /// The posting's most recently opened analysis.
    pub fn latest_analysis(&self, posting_id: &str) -> Result<Option<AnalysisRow>, StoreError> {
        self.analysis_where(
            "posting_id = ?1 ORDER BY opened_at DESC, rowid DESC",
            &[posting_id],
        )
    }

    /// Every analysis of the posting, most recently opened first.
    pub fn analyses(&self, posting_id: &str) -> Result<Vec<AnalysisRow>, StoreError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ANALYSIS_COLUMNS} FROM analyses WHERE posting_id = ?1 ORDER BY opened_at DESC, rowid DESC"
        ))?;
        let rows = stmt
            .query_map([posting_id], RawAnalysis::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(RawAnalysis::parse).collect()
    }

    /// Marks the analysis opened now, so [`Store::latest_analysis`] returns it.
    pub fn touch_analysis(&self, id: &str) -> Result<(), StoreError> {
        let now = now();
        self.update_analysis(
            id,
            "UPDATE analyses SET opened_at = ?2, updated_at = ?2 WHERE id = ?1",
            params![id, now],
        )
    }

    /// Records the analysis's projected score, for the posting's fit score.
    pub fn set_projected(&self, id: &str, projected: f64) -> Result<(), StoreError> {
        self.update_analysis(
            id,
            "UPDATE analyses SET projected = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, projected, now()],
        )
    }

    fn update_analysis(
        &self,
        id: &str,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> Result<(), StoreError> {
        if self.conn.execute(sql, params)? == 0 {
            return Err(not_found("analysis", id));
        }
        Ok(())
    }

    fn analysis_where(
        &self,
        clause: &str,
        args: &[&str],
    ) -> Result<Option<AnalysisRow>, StoreError> {
        let row = self
            .conn
            .query_row(
                &format!("SELECT {ANALYSIS_COLUMNS} FROM analyses WHERE {clause} LIMIT 1"),
                rusqlite::params_from_iter(args),
                RawAnalysis::from_row,
            )
            .optional()?;
        row.map(RawAnalysis::parse).transpose()
    }

    // Drafts.

    /// The analysis's draft log: empty with cursor 0 until something is pushed.
    pub fn draft(&self, analysis_id: &str) -> Result<DraftLog, StoreError> {
        read_draft(&self.conn, analysis_id)
    }

    /// Appends `event` at the cursor: events at or past it (the redo tail) are dropped first.
    pub fn push_draft_event(
        &self,
        analysis_id: &str,
        event: &DraftEvent,
    ) -> Result<DraftLog, StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        let cursor = draft_cursor(&tx, analysis_id)?;
        let now = now();
        tx.execute(
            "DELETE FROM draft_events WHERE analysis_id = ?1 AND seq >= ?2",
            params![analysis_id, cursor],
        )?;
        tx.execute(
            "INSERT INTO draft_events (id, analysis_id, seq, json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![
                new_id(),
                analysis_id,
                cursor,
                serde_json::to_string(event)?,
                now
            ],
        )?;
        tx.execute(
            "UPDATE analyses SET cursor = ?2, updated_at = ?3 WHERE id = ?1",
            params![analysis_id, cursor + 1, now],
        )?;
        let log = read_draft(&tx, analysis_id)?;
        tx.commit()?;
        Ok(log)
    }

    /// Moves the cursor (undo/redo), clamped to the log's length.
    pub fn set_draft_cursor(
        &self,
        analysis_id: &str,
        cursor: usize,
    ) -> Result<DraftLog, StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        draft_cursor(&tx, analysis_id)?;
        let len: i64 = tx.query_row(
            "SELECT count(*) FROM draft_events WHERE analysis_id = ?1",
            [analysis_id],
            |r| r.get(0),
        )?;
        tx.execute(
            "UPDATE analyses SET cursor = ?2, updated_at = ?3 WHERE id = ?1",
            params![analysis_id, to_i64(cursor as u64).min(len), now()],
        )?;
        let log = read_draft(&tx, analysis_id)?;
        tx.commit()?;
        Ok(log)
    }

    // Cover letters.

    /// The analysis's cover letter, `None` until one is written.
    pub fn cover(&self, analysis_id: &str) -> Result<Option<Cover>, StoreError> {
        let row = self
            .conn
            .query_row(
                "SELECT id, analysis_id, greeting, paragraphs, closing, updated_at FROM covers
                 WHERE analysis_id = ?1",
                [analysis_id],
                |r| {
                    Ok((
                        Cover {
                            id: r.get(0)?,
                            analysis_id: r.get(1)?,
                            greeting: r.get(2)?,
                            paragraphs: Vec::new(),
                            closing: r.get(4)?,
                            updated_at: r.get(5)?,
                        },
                        r.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(c, paragraphs)| {
            Ok(Cover {
                paragraphs: from_json(&paragraphs)?,
                ..c
            })
        })
        .transpose()
    }

    /// Sets the analysis's cover letter (written, rewritten or edited): one per analysis, keeping its id.
    pub fn put_cover(
        &self,
        analysis_id: &str,
        greeting: &str,
        paragraphs: &[String],
        closing: &str,
    ) -> Result<Cover, StoreError> {
        let now = now();
        self.conn.execute(
            "INSERT INTO covers (id, analysis_id, greeting, paragraphs, closing, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT (analysis_id) DO UPDATE SET greeting = excluded.greeting,
                 paragraphs = excluded.paragraphs, closing = excluded.closing, updated_at = excluded.updated_at",
            params![
                new_id(),
                analysis_id,
                greeting,
                serde_json::to_string(paragraphs)?,
                closing,
                now
            ],
        )?;
        self.cover(analysis_id)?
            .ok_or_else(|| not_found("cover", analysis_id))
    }

    // Company logos.

    /// Every company with a logo: its key and when the logo was set.
    pub fn logos(&self) -> Result<Vec<(String, String)>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, updated_at FROM companies ORDER BY key")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn logo(&self, key: &str) -> Result<Option<Logo>, StoreError> {
        let row = self
            .conn
            .query_row(
                "SELECT logo, logo_type, updated_at, source FROM companies WHERE key = ?1",
                [key],
                |r| {
                    Ok((
                        r.get::<_, Vec<u8>>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(bytes, media_type, updated_at, source)| {
            Ok(Logo {
                bytes,
                media_type,
                updated_at,
                source: serde_json::from_value(serde_json::Value::String(source))?,
            })
        })
        .transpose()
    }

    /// Sets the company's logo as yours, replacing any; returns when it was set.
    pub fn put_logo(&self, key: &str, bytes: &[u8], media_type: &str) -> Result<String, StoreError> {
        let now = now();
        self.conn.execute(
            "INSERT INTO companies (id, key, logo, logo_type, source, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'yours', ?5, ?5)
             ON CONFLICT (key) DO UPDATE SET logo = excluded.logo, logo_type = excluded.logo_type,
                 source = 'yours', updated_at = excluded.updated_at",
            params![new_id(), key, bytes, media_type, now],
        )?;
        Ok(now)
    }

    /// Stores a fetched logo (`source` freehire or site), replacing a fetched one but never yours; whether it was
    /// stored.
    pub fn put_fetched_logo(
        &self,
        key: &str,
        bytes: &[u8],
        media_type: &str,
        source: LogoSource,
    ) -> Result<bool, StoreError> {
        let changed = self.conn.execute(
            "INSERT INTO companies (id, key, logo, logo_type, source, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT (key) DO UPDATE SET logo = excluded.logo, logo_type = excluded.logo_type,
                 source = excluded.source, updated_at = excluded.updated_at
             WHERE companies.source <> 'yours'",
            params![new_id(), key, bytes, media_type, source.as_str(), now()],
        )?;
        Ok(changed > 0)
    }

    /// Removes the company's logo; nothing when it has none.
    pub fn remove_logo(&self, key: &str) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM companies WHERE key = ?1", [key])?;
        Ok(())
    }

    /// The company's last logo fetch, if any.
    pub fn logo_lookup(&self, key: &str) -> Result<Option<LogoLookup>, StoreError> {
        let row = self
            .conn
            .query_row(
                "SELECT found, tried_at FROM logo_lookups WHERE key = ?1",
                [key],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?;
        row.map(|(found, tried_at)| {
            Ok(LogoLookup {
                found: serde_json::from_value(serde_json::Value::String(found))?,
                tried_at,
            })
        })
        .transpose()
    }

    /// Records a logo fetch for the company (now), replacing the last.
    pub fn put_logo_lookup(&self, key: &str, found: LogoFound) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO logo_lookups (key, found, tried_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (key) DO UPDATE SET found = excluded.found, tried_at = excluded.tried_at",
            params![key, found.as_str(), now()],
        )?;
        Ok(())
    }

    // Geocoded places.

    /// The geocoding query's last lookup, if any; `key` as [`put_place_lookup`](Self::put_place_lookup) stored it.
    pub fn place_lookup(&self, key: &str) -> Result<Option<PlaceLookup>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT json, tried_at FROM places WHERE key = ?1",
                [key],
                |r| {
                    Ok(PlaceLookup {
                        json: r.get(0)?,
                        tried_at: r.get(1)?,
                    })
                },
            )
            .optional()?)
    }

    /// Records a geocoding lookup (now): the place found as JSON, or `None`; replaces the last.
    pub fn put_place_lookup(&self, key: &str, json: Option<&str>) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO places (key, json, tried_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (key) DO UPDATE SET json = excluded.json, tried_at = excluded.tried_at",
            params![key, json, now()],
        )?;
        Ok(())
    }

    // Applications.

    /// Every application, newest first (by date, then the most recently added).
    pub fn applications(&self) -> Result<Vec<Application>, StoreError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {APPLICATION_COLUMNS} FROM applications ORDER BY date DESC, rowid DESC"
        ))?;
        let rows = stmt
            .query_map([], RawApplication::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(RawApplication::parse).collect()
    }

    pub fn application_for_posting(
        &self,
        posting_id: &str,
    ) -> Result<Option<Application>, StoreError> {
        self.application_where("posting_id = ?1", posting_id)
    }

    /// Inserts or updates the application by `a.id`; sets its `updated_at` (the one given is ignored) and logs its
    /// status when new or changed. A posting has at most one application.
    pub fn put_application(&self, a: &Application) -> Result<(), StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        insert_application(&tx, a)?;
        tx.commit()?;
        Ok(())
    }

    /// Stores applications listed newest first (as [`parse_ats_fit_csv`] returns them), so [`Store::applications`]
    /// lists same-day rows in the same order. Their statuses are logged as of the import.
    pub fn import_applications(&self, apps: &[Application]) -> Result<(), StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        for a in apps.iter().rev() {
            insert_application(&tx, a)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Sets every application in `ids` to `status`, in one transaction: an unknown id changes none of them. Logs the
    /// change for each whose status differed. Returns the applications as saved, in `ids`' order.
    pub fn set_application_status(
        &self,
        ids: &[&str],
        status: AppStatus,
    ) -> Result<Vec<Application>, StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        let at = now();
        for id in ids {
            let was = stored_status(&tx, id)?.ok_or_else(|| not_found("application", id))?;
            tx.execute(
                "UPDATE applications SET status = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, status.as_str(), at],
            )?;
            if was != status.as_str() {
                log_status(&tx, id, status, &at)?;
            }
        }
        tx.commit()?;
        ids.iter()
            .map(|id| {
                self.application_where("id = ?1", id)?
                    .ok_or_else(|| not_found("application", id))
            })
            .collect()
    }

    /// The application's status changes, oldest first: the status it was first stored with, then each change.
    pub fn status_history(&self, application_id: &str) -> Result<Vec<StatusChange>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT status, at FROM status_changes WHERE application_id = ?1 ORDER BY at, rowid",
        )?;
        let rows = stmt
            .query_map([application_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(status, at)| {
                let status = serde_json::from_value(serde_json::Value::String(status))?;
                Ok(StatusChange { status, at })
            })
            .collect()
    }

    /// Stores the tailored resume JSON as exported for the application, replacing any earlier one.
    pub fn put_sent(&self, application_id: &str, resume_json: &str) -> Result<(), StoreError> {
        let changed = self.conn.execute(
            "UPDATE applications SET sent = ?2 WHERE id = ?1",
            params![application_id, resume_json],
        )?;
        if changed == 0 {
            return Err(not_found("application", application_id));
        }
        Ok(())
    }

    /// The tailored resume JSON as last exported for the application; `None` when it was never exported since
    /// schema version 8 (or was imported from `applications.csv`).
    pub fn sent(&self, application_id: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT sent FROM applications WHERE id = ?1",
                [application_id],
                |r| r.get(0),
            )
            .optional()?
            .flatten())
    }

    // Rounds.

    /// The application's rounds: scheduled ones by when they start, then the unscheduled by when they were added.
    pub fn rounds(&self, application_id: &str) -> Result<Vec<Round>, StoreError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ROUND_COLUMNS} FROM rounds WHERE application_id = ?1
             ORDER BY starts_at IS NULL, julianday(starts_at), created_at, rowid"
        ))?;
        let rows = stmt
            .query_map([application_id], RawRound::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(RawRound::parse).collect()
    }

    pub fn round(&self, id: &str) -> Result<Option<Round>, StoreError> {
        round_by_id(&self.conn, id)
    }

    /// Inserts or updates the round by `r.id` (an id not stored is inserted, as when Undo brings back one removed);
    /// sets its `updated_at`. Returns it as saved; errors when its application doesn't exist.
    pub fn put_round(&self, r: &Round) -> Result<Round, StoreError> {
        if stored_status(&self.conn, &r.application_id)?.is_none() {
            return Err(not_found("application", &r.application_id));
        }
        self.conn.execute(
            "INSERT INTO rounds (id, application_id, kind, starts_at, minutes, with_whom, place, notes, state,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
             ON CONFLICT (id) DO UPDATE SET application_id = excluded.application_id, kind = excluded.kind,
                 starts_at = excluded.starts_at, minutes = excluded.minutes, with_whom = excluded.with_whom,
                 place = excluded.place, notes = excluded.notes, state = excluded.state,
                 updated_at = excluded.updated_at",
            params![
                r.id,
                r.application_id,
                r.kind,
                r.starts_at,
                r.minutes,
                r.with,
                r.place,
                r.notes,
                r.state.as_str(),
                now()
            ],
        )?;
        round_by_id(&self.conn, &r.id)?.ok_or_else(|| not_found("round", &r.id))
    }

    /// Deletes the round; returns it as it was, `None` when there was none.
    pub fn delete_round(&self, id: &str) -> Result<Option<Round>, StoreError> {
        let round = round_by_id(&self.conn, id)?;
        if round.is_some() {
            self.conn
                .execute("DELETE FROM rounds WHERE id = ?1", [id])?;
        }
        Ok(round)
    }

    /// Sets every round in `ids` to `state`, in one transaction: an unknown id changes none of them. Returns the rounds
    /// as saved, in `ids`' order.
    pub fn set_round_state(
        &self,
        ids: &[&str],
        state: RoundState,
    ) -> Result<Vec<Round>, StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        let at = now();
        for id in ids {
            let changed = tx.execute(
                "UPDATE rounds SET state = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, state.as_str(), at],
            )?;
            if changed == 0 {
                return Err(not_found("round", id));
            }
        }
        tx.commit()?;
        ids.iter()
            .map(|id| round_by_id(&self.conn, id)?.ok_or_else(|| not_found("round", id)))
            .collect()
    }

    /// Cancels the applications' upcoming rounds that haven't ended (the unscheduled too; one that's over stays as it
    /// is). Returns the ids cancelled.
    pub fn cancel_upcoming_rounds(
        &self,
        application_ids: &[&str],
    ) -> Result<Vec<String>, StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        let at = now();
        let mut cancelled = Vec::new();
        {
            let mut stmt = tx.prepare(&format!(
                "UPDATE rounds SET state = 'cancelled', updated_at = ?2
                 WHERE application_id = ?1 AND state = 'upcoming' AND {NOT_ENDED} RETURNING id"
            ))?;
            for id in application_ids {
                let ids = stmt
                    .query_map(params![id, at], |r| r.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                cancelled.extend(ids);
            }
        }
        tx.commit()?;
        Ok(cancelled)
    }

    /// Each application's next round, by application id: the soonest upcoming one that is scheduled and hasn't ended
    /// (its start plus its length, an hour when unknown, is after now).
    pub fn next_rounds(&self) -> Result<HashMap<String, Round>, StoreError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ROUND_COLUMNS} FROM (
                 SELECT *, ROW_NUMBER() OVER (
                     PARTITION BY application_id ORDER BY julianday(starts_at), created_at, rowid
                 ) AS n
                 FROM rounds WHERE state = 'upcoming' AND starts_at IS NOT NULL AND {NOT_ENDED}
             ) WHERE n = 1"
        ))?;
        let rows = stmt
            .query_map([], RawRound::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|raw| {
                let r = raw.parse()?;
                Ok((r.application_id.clone(), r))
            })
            .collect()
    }

    /// One application's next round, as [`Store::next_rounds`] finds it.
    pub fn next_round(&self, application_id: &str) -> Result<Option<Round>, StoreError> {
        let row = self
            .conn
            .query_row(
                &format!(
                    "SELECT {ROUND_COLUMNS} FROM rounds
                     WHERE application_id = ?1 AND state = 'upcoming' AND starts_at IS NOT NULL AND {NOT_ENDED}
                     ORDER BY julianday(starts_at), created_at, rowid LIMIT 1"
                ),
                [application_id],
                RawRound::from_row,
            )
            .optional()?;
        row.map(RawRound::parse).transpose()
    }

    // Preps and practice.

    /// The prep stored under `key`.
    pub fn prep(&self, key: &str) -> Result<Option<Prep>, StoreError> {
        self.prep_where("key = ?1", key)
    }

    /// The round's prep written last, whatever it was written from.
    pub fn latest_prep(&self, round_id: &str) -> Result<Option<Prep>, StoreError> {
        self.prep_where("round_id = ?1 ORDER BY at DESC, rowid DESC LIMIT 1", round_id)
    }

    fn prep_where(&self, clause: &str, arg: &str) -> Result<Option<Prep>, StoreError> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT key, round_id, json, model, at FROM preps WHERE {clause}"),
                [arg],
                |r| {
                    Ok(Prep {
                        key: r.get(0)?,
                        round_id: r.get(1)?,
                        json: r.get(2)?,
                        model: r.get(3)?,
                        at: r.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    /// Stores `p` under its key, written now; replaces any prep under the same key. Returns it as stored; errors
    /// when its round doesn't exist.
    pub fn put_prep(&self, p: &Prep) -> Result<Prep, StoreError> {
        if round_by_id(&self.conn, &p.round_id)?.is_none() {
            return Err(not_found("round", &p.round_id));
        }
        let at = now();
        self.conn.execute(
            "INSERT INTO preps (key, round_id, json, model, at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (key) DO UPDATE SET round_id = excluded.round_id, json = excluded.json,
                 model = excluded.model, at = excluded.at",
            params![p.key, p.round_id, p.json, p.model, at],
        )?;
        Ok(Prep { at, ..p.clone() })
    }

    /// Adds a practiced answer, at now; returns it as stored. Errors when its round doesn't exist.
    pub fn add_practice(&self, p: &PracticeRow) -> Result<PracticeRow, StoreError> {
        if round_by_id(&self.conn, &p.round_id)?.is_none() {
            return Err(not_found("round", &p.round_id));
        }
        let at = now();
        self.conn.execute(
            "INSERT INTO practice (id, round_id, question, asked, answer, feedback, at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![p.id, p.round_id, p.question, p.asked, p.answer, p.feedback, at],
        )?;
        Ok(PracticeRow { at, ..p.clone() })
    }

    /// The round's practiced answers, oldest first.
    pub fn practice(&self, round_id: &str) -> Result<Vec<PracticeRow>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, round_id, question, asked, answer, feedback, at FROM practice
             WHERE round_id = ?1 ORDER BY at, rowid",
        )?;
        let rows = stmt
            .query_map([round_id], |r| {
                Ok(PracticeRow {
                    id: r.get(0)?,
                    round_id: r.get(1)?,
                    question: r.get(2)?,
                    asked: r.get(3)?,
                    answer: r.get(4)?,
                    feedback: r.get(5)?,
                    at: r.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// The application the round belongs to.
    pub fn application(&self, id: &str) -> Result<Option<Application>, StoreError> {
        self.application_where("id = ?1", id)
    }

    fn application_where(
        &self,
        clause: &str,
        arg: &str,
    ) -> Result<Option<Application>, StoreError> {
        let row = self
            .conn
            .query_row(
                &format!("SELECT {APPLICATION_COLUMNS} FROM applications WHERE {clause}"),
                [arg],
                RawApplication::from_row,
            )
            .optional()?;
        row.map(RawApplication::parse).transpose()
    }

    // Feed.

    /// Adds jobs whose slug the feed doesn't have; existing rows are left as they are. Returns how many were added.
    pub fn add_feed_jobs(&self, jobs: &[NewFeedJob]) -> Result<usize, StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        let mut added = 0;
        for j in jobs {
            added += tx.execute(
                "INSERT INTO feed (id, slug, json, found_at, posted_at, state, overlap, reason, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
                 ON CONFLICT (slug) DO NOTHING",
                params![
                    new_id(),
                    j.job.slug,
                    serde_json::to_string(&j.job)?,
                    j.job.found_at,
                    j.job.posted_at,
                    j.state.as_str(),
                    j.overlap,
                    j.reason,
                    now()
                ],
            )?;
        }
        tx.commit()?;
        Ok(added)
    }

    /// Every row, most recently found first.
    pub fn feed(&self) -> Result<Vec<FeedRow>, StoreError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {FEED_COLUMNS} FROM feed ORDER BY found_at DESC, rowid DESC"
        ))?;
        let rows = stmt
            .query_map([], RawFeed::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(RawFeed::parse).collect()
    }

    pub fn feed_row(&self, slug: &str) -> Result<Option<FeedRow>, StoreError> {
        let row = self
            .conn
            .query_row(
                &format!("SELECT {FEED_COLUMNS} FROM feed WHERE slug = ?1"),
                [slug],
                RawFeed::from_row,
            )
            .optional()?;
        row.map(RawFeed::parse).transpose()
    }

    pub fn set_feed_state(
        &self,
        slug: &str,
        state: FeedState,
        reason: Option<&str>,
    ) -> Result<(), StoreError> {
        self.update_feed(
            slug,
            "state = ?2, reason = ?3",
            params![slug, state.as_str(), reason, now()],
        )
    }

    pub fn set_feed_posting(&self, slug: &str, posting_id: &str) -> Result<(), StoreError> {
        self.update_feed(slug, "posting_id = ?2", params![slug, posting_id, now()])
    }

    /// State `checked`, `checked_at` now.
    pub fn set_feed_checked(
        &self,
        slug: &str,
        score: Option<f64>,
        master_id: Option<&str>,
    ) -> Result<(), StoreError> {
        self.update_feed(
            slug,
            "state = 'checked', score = ?2, master_id = ?3, reason = NULL, checked_at = ?4",
            params![slug, score, master_id, now()],
        )
    }

    /// Marks the feed row holding this posting opened (nothing when none does).
    pub fn set_feed_opened(&self, posting_id: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE feed SET opened_at = ?2, updated_at = ?2 WHERE posting_id = ?1 AND opened_at IS NULL",
            params![posting_id, now()],
        )?;
        Ok(())
    }

    /// Postings the feed read that you haven't opened: Recent jobs and Applications leave them out.
    pub fn feed_unopened_postings(&self) -> Result<HashSet<String>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT posting_id FROM feed WHERE posting_id IS NOT NULL AND opened_at IS NULL",
        )?;
        let ids = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<HashSet<String>, _>>()?;
        Ok(ids)
    }

    /// Checks since `since` (RFC 3339, any offset), for the daily cap.
    pub fn feed_checked_since(&self, since: &str) -> Result<u32, StoreError> {
        let n: i64 = self.conn.query_row(
            "SELECT count(*) FROM feed WHERE checked_at IS NOT NULL AND julianday(checked_at) >= julianday(?1)",
            [since],
            |r| r.get(0),
        )?;
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
    }

    /// Sets `columns` (from `?2`; the last parameter is `updated_at`) on the row `slug` (`?1`).
    fn update_feed(
        &self,
        slug: &str,
        columns: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> Result<(), StoreError> {
        let sql = format!(
            "UPDATE feed SET {columns}, updated_at = ?{} WHERE slug = ?1",
            params.len()
        );
        if self.conn.execute(&sql, params)? == 0 {
            return Err(not_found("feed job", slug));
        }
        Ok(())
    }

    // Settings.

    /// The setting under `key`, `None` when unset.
    pub fn setting<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, StoreError> {
        let json: Option<String> = self
            .conn
            .query_row("SELECT json FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?;
        json.map(|j| from_json(&j)).transpose()
    }

    pub fn put_setting<T: Serialize>(&self, key: &str, value: &T) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO settings (key, json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (key) DO UPDATE SET json = excluded.json, updated_at = excluded.updated_at",
            params![key, serde_json::to_string(value)?, now()],
        )?;
        Ok(())
    }
}

const FEED_COLUMNS: &str = "json, state, overlap, posting_id, score, master_id, checked_at, opened_at, reason, \
                            created_at";

const ANALYSIS_COLUMNS: &str = "id, posting_id, bank_rev, master_id, master_rev, mode, model, suggestions, projected, \
                                created_at, opened_at";

const APPLICATION_COLUMNS: &str = "id, posting_id, analysis_id, date, company, role, url, status, now, projected, \
                                   on_file, resume_path, cover_path, notes, updated_at";

const ROUND_COLUMNS: &str =
    "id, application_id, kind, starts_at, minutes, with_whom, place, notes, state, updated_at";
/// A round that hasn't ended: unscheduled, or its end (start plus its length, an hour when unknown) after now.
const NOT_ENDED: &str = "(starts_at IS NULL OR julianday(starts_at) + COALESCE(minutes, 60) / 1440.0 > julianday('now'))";

struct RawPosting {
    id: String,
    json: String,
    created_at: String,
    updated_at: String,
    salary: Option<String>,
}

impl RawPosting {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            json: r.get(1)?,
            created_at: r.get(2)?,
            updated_at: r.get(3)?,
            salary: r.get(4)?,
        })
    }

    fn parse(self) -> Result<PostingRow, StoreError> {
        Ok(PostingRow {
            id: self.id,
            posting: from_json(&self.json)?,
            created_at: self.created_at,
            updated_at: self.updated_at,
            your_salary: self.salary.as_deref().map(from_json).transpose()?,
        })
    }
}

struct RawAnalysis {
    id: String,
    posting_id: String,
    bank_rev: String,
    master_id: String,
    master_rev: String,
    mode: String,
    model: String,
    suggestions: String,
    projected: Option<f64>,
    created_at: String,
    opened_at: String,
}

impl RawAnalysis {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            posting_id: r.get(1)?,
            bank_rev: r.get(2)?,
            master_id: r.get(3)?,
            master_rev: r.get(4)?,
            mode: r.get(5)?,
            model: r.get(6)?,
            suggestions: r.get(7)?,
            projected: r.get(8)?,
            created_at: r.get(9)?,
            opened_at: r.get(10)?,
        })
    }

    fn parse(self) -> Result<AnalysisRow, StoreError> {
        Ok(AnalysisRow {
            id: self.id,
            posting_id: self.posting_id,
            bank_rev: Rev(self.bank_rev),
            master_id: self.master_id,
            master_rev: Rev(self.master_rev),
            mode: serde_json::from_value(serde_json::Value::String(self.mode))?,
            model: self.model,
            suggestions: from_json(&self.suggestions)?,
            projected: self.projected,
            created_at: self.created_at,
            opened_at: self.opened_at,
        })
    }
}

struct RawFeed {
    json: String,
    state: String,
    overlap: f64,
    posting_id: Option<String>,
    score: Option<f64>,
    master_id: Option<String>,
    checked_at: Option<String>,
    opened_at: Option<String>,
    reason: Option<String>,
    created_at: String,
}

impl RawFeed {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            json: r.get(0)?,
            state: r.get(1)?,
            overlap: r.get(2)?,
            posting_id: r.get(3)?,
            score: r.get(4)?,
            master_id: r.get(5)?,
            checked_at: r.get(6)?,
            opened_at: r.get(7)?,
            reason: r.get(8)?,
            created_at: r.get(9)?,
        })
    }

    fn parse(self) -> Result<FeedRow, StoreError> {
        Ok(FeedRow {
            job: from_json(&self.json)?,
            state: serde_json::from_value(serde_json::Value::String(self.state))?,
            overlap: self.overlap,
            posting_id: self.posting_id,
            score: self.score,
            master_id: self.master_id,
            checked_at: self.checked_at,
            opened_at: self.opened_at,
            reason: self.reason,
            created_at: self.created_at,
        })
    }
}

struct RawRound {
    r: Round,
    state: String,
}

impl RawRound {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            r: Round {
                id: r.get(0)?,
                application_id: r.get(1)?,
                kind: r.get(2)?,
                starts_at: r.get(3)?,
                minutes: r.get(4)?,
                with: r.get(5)?,
                place: r.get(6)?,
                notes: r.get(7)?,
                state: RoundState::Upcoming,
                updated_at: r.get(9)?,
            },
            state: r.get(8)?,
        })
    }

    fn parse(self) -> Result<Round, StoreError> {
        let state = serde_json::from_value(serde_json::Value::String(self.state))?;
        Ok(Round { state, ..self.r })
    }
}

fn round_by_id(conn: &Connection, id: &str) -> Result<Option<Round>, StoreError> {
    let row = conn
        .query_row(
            &format!("SELECT {ROUND_COLUMNS} FROM rounds WHERE id = ?1"),
            [id],
            RawRound::from_row,
        )
        .optional()?;
    row.map(RawRound::parse).transpose()
}

struct RawApplication {
    a: Application,
    status: String,
}

impl RawApplication {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            a: Application {
                id: r.get(0)?,
                posting_id: r.get(1)?,
                analysis_id: r.get(2)?,
                date: r.get(3)?,
                company: r.get(4)?,
                role: r.get(5)?,
                url: r.get(6)?,
                status: AppStatus::Saved,
                now: r.get(8)?,
                projected: r.get(9)?,
                on_file: r.get(10)?,
                resume_path: r.get(11)?,
                cover_path: r.get(12)?,
                notes: r.get(13)?,
                updated_at: r.get(14)?,
            },
            status: r.get(7)?,
        })
    }

    fn parse(self) -> Result<Application, StoreError> {
        let status = serde_json::from_value(serde_json::Value::String(self.status))?;
        Ok(Application { status, ..self.a })
    }
}

/// Upserts the application and logs its status when the row is new or its status changed. Run it in a transaction.
fn insert_application(conn: &Connection, a: &Application) -> Result<(), StoreError> {
    let at = now();
    let was = stored_status(conn, &a.id)?;
    conn.execute(
        "INSERT INTO applications (id, posting_id, analysis_id, date, company, role, url, status, now, projected,
             on_file, resume_path, cover_path, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)
         ON CONFLICT (id) DO UPDATE SET posting_id = excluded.posting_id, analysis_id = excluded.analysis_id,
             date = excluded.date, company = excluded.company, role = excluded.role, url = excluded.url,
             status = excluded.status, now = excluded.now, projected = excluded.projected,
             on_file = excluded.on_file, resume_path = excluded.resume_path, cover_path = excluded.cover_path,
             notes = excluded.notes, updated_at = excluded.updated_at",
        params![
            a.id,
            a.posting_id,
            a.analysis_id,
            a.date,
            a.company,
            a.role,
            a.url,
            a.status.as_str(),
            a.now,
            a.projected,
            a.on_file,
            a.resume_path,
            a.cover_path,
            a.notes,
            at
        ],
    )?;
    if was.as_deref() != Some(a.status.as_str()) {
        log_status(conn, &a.id, a.status, &at)?;
    }
    Ok(())
}

/// The application's stored status name; `None` when there is no such application.
fn stored_status(conn: &Connection, id: &str) -> Result<Option<String>, StoreError> {
    Ok(conn
        .query_row("SELECT status FROM applications WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .optional()?)
}

fn log_status(conn: &Connection, id: &str, status: AppStatus, at: &str) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO status_changes (application_id, status, at) VALUES (?1, ?2, ?3)",
        params![id, status.as_str(), at],
    )?;
    Ok(())
}

/// The analysis's draft cursor; errors when there is no such analysis.
fn draft_cursor(conn: &Connection, analysis_id: &str) -> Result<i64, StoreError> {
    conn.query_row(
        "SELECT cursor FROM analyses WHERE id = ?1",
        [analysis_id],
        |r| r.get(0),
    )
    .optional()?
    .ok_or_else(|| not_found("analysis", analysis_id))
}

fn read_draft(conn: &Connection, analysis_id: &str) -> Result<DraftLog, StoreError> {
    let cursor: i64 = conn
        .query_row(
            "SELECT cursor FROM analyses WHERE id = ?1",
            [analysis_id],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0);
    let mut stmt =
        conn.prepare_cached("SELECT json FROM draft_events WHERE analysis_id = ?1 ORDER BY seq")?;
    let jsons = stmt
        .query_map([analysis_id], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let events = jsons
        .iter()
        .map(|j| from_json(j))
        .collect::<Result<Vec<DraftEvent>, _>>()?;
    let cursor = usize::try_from(cursor).unwrap_or(0).min(events.len());
    Ok(DraftLog { events, cursor })
}

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::MasterOnly => "master-only",
        Mode::BankAware => "bank-aware",
    }
}

fn from_json<T: DeserializeOwned>(json: &str) -> Result<T, StoreError> {
    Ok(serde_json::from_str(json)?)
}

fn not_found(what: &'static str, id: &str) -> StoreError {
    StoreError::NotFound {
        what,
        id: id.to_owned(),
    }
}

fn to_i64(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

/// Now as RFC 3339 UTC with microseconds (`2026-10-12T14:03:05.123456Z`): fixed width, so the text sorts by time.
/// Strictly increasing within the process, so "most recent" orderings never tie.
fn now() -> String {
    static LAST: AtomicU64 = AtomicU64::new(0);
    let clock = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_micros() as u64);
    let prev = LAST
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |last| {
            Some(clock.max(last + 1))
        })
        .unwrap_or(0);
    let micros = clock.max(prev + 1);
    rfc3339(micros)
}

/// RFC 3339 UTC for microseconds since the Unix epoch.
fn rfc3339(micros: u64) -> String {
    let secs = (micros / 1_000_000) as i64;
    let (days, day_secs) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:06}Z",
        day_secs / 3600,
        day_secs / 60 % 60,
        day_secs % 60,
        micros % 1_000_000
    )
}
