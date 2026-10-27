-- Version 11: stories and Dig (ARCHITECTURE.md §8). `stories`: a story's JSON (`schemas/story.schema.json`), outside
-- the bank so editing one never writes a bank revision. `practice` is rebuilt so a row can belong to a story told alone
-- (`round_id` NULL) and name the story it told (`story_id`, no foreign key: a story removed and brought back by Undo
-- finds its tellings again). `digs`: an entry's Dig questions, keyed by the prompt and the entry's content. `dig_answers`:
-- the answers one Dig capture read (JSON), which ground the stories it proposes (`Story.source.from` is its id).
CREATE TABLE stories (
    id TEXT PRIMARY KEY,
    json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE practice_v11 (
    id TEXT PRIMARY KEY,
    round_id TEXT REFERENCES rounds (id) ON DELETE CASCADE,
    story_id TEXT,
    question TEXT NOT NULL,
    asked TEXT NOT NULL,
    answer TEXT NOT NULL,
    feedback TEXT NOT NULL,
    at TEXT NOT NULL
);
INSERT INTO practice_v11 (id, round_id, question, asked, answer, feedback, at)
    SELECT id, round_id, question, asked, answer, feedback, at FROM practice ORDER BY rowid;
DROP TABLE practice;
ALTER TABLE practice_v11 RENAME TO practice;
CREATE INDEX practice_round ON practice (round_id);
CREATE INDEX practice_story ON practice (story_id);
CREATE TABLE digs (
    key TEXT PRIMARY KEY,
    entry TEXT NOT NULL,
    json TEXT NOT NULL,
    model TEXT NOT NULL,
    at TEXT NOT NULL
);
CREATE TABLE dig_answers (
    id TEXT PRIMARY KEY,
    entry TEXT NOT NULL,
    json TEXT NOT NULL,
    at TEXT NOT NULL
);
