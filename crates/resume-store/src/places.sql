-- Version 7: geocoded places for the review's map (ARCHITECTURE.md §7). `key`: the geocoding query, lowercased with
-- its whitespace collapsed; `json`: the place found (the app's `Place` as JSON), NULL when nothing was; `tried_at`:
-- when it was looked up (RFC 3339).
CREATE TABLE places (
    key TEXT PRIMARY KEY,
    json TEXT,
    tried_at TEXT NOT NULL
);
