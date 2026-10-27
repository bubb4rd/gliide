-- Version 10: a round's prep and its practiced answers (ARCHITECTURE.md §8). `preps`: the prep JSON as written, keyed
-- by what it was written from (the prompt, the posting, the resume as sent, the bank revision, the round); `model`:
-- the model that wrote it. `practice`: one typed answer to a prep question (`question`: its id, `asked`: its text) and
-- the feedback JSON that came back.
CREATE TABLE preps (
    key TEXT PRIMARY KEY,
    round_id TEXT NOT NULL REFERENCES rounds (id) ON DELETE CASCADE,
    json TEXT NOT NULL,
    model TEXT NOT NULL,
    at TEXT NOT NULL
);
CREATE INDEX preps_round ON preps (round_id);
CREATE TABLE practice (
    id TEXT PRIMARY KEY,
    round_id TEXT NOT NULL REFERENCES rounds (id) ON DELETE CASCADE,
    question TEXT NOT NULL,
    asked TEXT NOT NULL,
    answer TEXT NOT NULL,
    feedback TEXT NOT NULL,
    at TEXT NOT NULL
);
CREATE INDEX practice_round ON practice (round_id);
