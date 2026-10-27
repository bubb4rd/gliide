-- Version 9: interview rounds (ARCHITECTURE.md §8). `kind`: the round's kind (`technical`, `recruiter-screen`, …);
-- `starts_at`: RFC 3339 with the offset it was given in, NULL while not scheduled; `minutes`: its length, NULL when
-- not known; `state`: `upcoming` or `cancelled`.
CREATE TABLE rounds (
    id TEXT PRIMARY KEY,
    application_id TEXT NOT NULL REFERENCES applications (id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    starts_at TEXT,
    minutes INTEGER,
    with_whom TEXT NOT NULL DEFAULT '',
    place TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX rounds_application ON rounds (application_id);
