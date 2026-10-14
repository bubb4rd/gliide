-- Version 5: the job feed (ARCHITECTURE.md §7, Job feed): one row per freehire job by its slug, `json` the serde JSON of
-- `resume_core::feed::FeedJob`. `state` is new, skipped, waiting, checked or dismissed; `overlap` the local check's
-- order; `score` the best master's score now once checked. `posting_id` is set once the job's text is read; deleting
-- the posting only clears it. `opened_at`: you opened the posting, so it shows in Recent jobs and Applications.
CREATE TABLE feed (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    json TEXT NOT NULL,
    found_at TEXT NOT NULL,
    posted_at TEXT,
    state TEXT NOT NULL,
    overlap REAL NOT NULL,
    posting_id TEXT REFERENCES postings (id) ON DELETE SET NULL,
    score REAL,
    master_id TEXT,
    checked_at TEXT,
    opened_at TEXT,
    reason TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX feed_found ON feed (found_at);
CREATE INDEX feed_posting ON feed (posting_id);
CREATE INDEX feed_checked ON feed (checked_at);
