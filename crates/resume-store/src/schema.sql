-- Version 1 (ARCHITECTURE.md §8). Every row has an id and an RFC 3339 `updated_at`, so sync can come later without a
-- migration. JSON columns hold the serde JSON of the `resume-core` types.

-- Immutable bank documents by content hash; exactly one is current.
CREATE TABLE bank_revs (
    id TEXT PRIMARY KEY,
    hash TEXT NOT NULL UNIQUE,
    json TEXT NOT NULL,
    current INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX bank_revs_current ON bank_revs (current) WHERE current = 1;

-- One row per master (`Master.id`), pointing at its current revision.
CREATE TABLE masters (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    rev TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE master_revs (
    id TEXT PRIMARY KEY,
    master_id TEXT NOT NULL REFERENCES masters (id),
    hash TEXT NOT NULL,
    json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (master_id, hash)
);

-- Postings by text hash: the same text read twice is the same posting.
CREATE TABLE postings (
    id TEXT PRIMARY KEY,
    hash TEXT NOT NULL UNIQUE,
    json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Extract output by its cache key and model.
CREATE TABLE requirements (
    id TEXT PRIMARY KEY,
    posting_id TEXT NOT NULL REFERENCES postings (id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    model TEXT NOT NULL,
    json TEXT NOT NULL,
    latency_ms INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (key, model)
);
CREATE INDEX requirements_posting ON requirements (posting_id, updated_at);

-- Suggest output for one posting × master revision × mode; `cursor` is the draft's undo position.
CREATE TABLE analyses (
    id TEXT PRIMARY KEY,
    posting_id TEXT NOT NULL REFERENCES postings (id) ON DELETE CASCADE,
    bank_rev TEXT NOT NULL REFERENCES bank_revs (hash),
    master_id TEXT NOT NULL,
    master_rev TEXT NOT NULL,
    mode TEXT NOT NULL,
    key TEXT NOT NULL,
    model TEXT NOT NULL,
    suggestions TEXT NOT NULL,
    latency_ms INTEGER NOT NULL,
    projected REAL,
    cursor INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    opened_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (master_id, master_rev) REFERENCES master_revs (master_id, hash)
);
CREATE INDEX analyses_key ON analyses (key, model);
CREATE INDEX analyses_posting ON analyses (posting_id, opened_at);

-- A draft's append-only event log; `seq` is the event's position, 0-based.
CREATE TABLE draft_events (
    id TEXT PRIMARY KEY,
    analysis_id TEXT NOT NULL REFERENCES analyses (id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (analysis_id, seq)
);

-- Applications outlive their posting: deleting it only clears the links.
CREATE TABLE applications (
    id TEXT PRIMARY KEY,
    posting_id TEXT REFERENCES postings (id) ON DELETE SET NULL,
    analysis_id TEXT REFERENCES analyses (id) ON DELETE SET NULL,
    date TEXT NOT NULL,
    company TEXT NOT NULL,
    role TEXT NOT NULL,
    url TEXT,
    status TEXT NOT NULL,
    now REAL,
    projected REAL,
    on_file REAL,
    resume_path TEXT,
    cover_path TEXT,
    notes TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX applications_posting ON applications (posting_id) WHERE posting_id IS NOT NULL;
CREATE INDEX applications_analysis ON applications (analysis_id);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
