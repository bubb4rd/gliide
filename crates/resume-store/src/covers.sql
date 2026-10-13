-- Version 2: cover letters, one per analysis (the letter goes with that tailored resume). `paragraphs` is a JSON array
-- of strings.
CREATE TABLE covers (
    id TEXT PRIMARY KEY,
    analysis_id TEXT NOT NULL UNIQUE REFERENCES analyses (id) ON DELETE CASCADE,
    greeting TEXT NOT NULL,
    paragraphs TEXT NOT NULL,
    closing TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
