-- Version 8: what an interview prep reads (ARCHITECTURE.md §8). `status_changes`: each application's status as first
-- stored and every change after, `at` when (RFC 3339); `applications.sent`: the tailored resume as exported (the
-- `Resume` as JSON), NULL for applications exported before this version or imported from `applications.csv`.
CREATE TABLE status_changes (
    application_id TEXT NOT NULL REFERENCES applications (id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    at TEXT NOT NULL
);
CREATE INDEX status_changes_application ON status_changes (application_id, at);

ALTER TABLE applications ADD COLUMN sent TEXT;
