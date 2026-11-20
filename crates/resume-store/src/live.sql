-- Version 19: live interviews (ARCHITECTURE.md §8). `live_runs`: one session for a round, the prep's questions asked in
-- order; `ended_at` NULL while live; `active_ms`: the session's clock as the app kept it (waits excluded), set at its
-- end; `questions`: the prep's question count when it started. `live_turns`: what was asked, in order (`seq` from 0):
-- `kind` `ask` (the prep question `question`, verbatim) or `follow` (a follow-up on your answer, `picked`: the phrase of
-- it the follow-up took up); `answer` NULL while unanswered or `skipped`; `spoken_*` as in `practice`. `practice.live_id`:
-- the run a practice row was read from (the readback).
CREATE TABLE live_runs (
    id TEXT PRIMARY KEY,
    round_id TEXT NOT NULL REFERENCES rounds (id) ON DELETE CASCADE,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    active_ms INTEGER,
    questions INTEGER NOT NULL
);
CREATE INDEX live_runs_round ON live_runs (round_id);
CREATE TABLE live_turns (
    run_id TEXT NOT NULL REFERENCES live_runs (id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    question TEXT NOT NULL,
    kind TEXT NOT NULL,
    asked TEXT NOT NULL,
    picked TEXT,
    answer TEXT,
    skipped INTEGER NOT NULL DEFAULT 0,
    spoken_ms INTEGER,
    spoken_words INTEGER,
    at TEXT NOT NULL,
    PRIMARY KEY (run_id, seq)
);
ALTER TABLE practice ADD COLUMN live_id TEXT REFERENCES live_runs (id) ON DELETE SET NULL;
CREATE INDEX practice_live ON practice (live_id);
