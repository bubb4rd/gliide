-- Version 12: debriefs and the notifications sent (ARCHITECTURE.md §8). `debriefs`: a round's debrief as you saved it
-- (JSON: asked, stuck_on, notes), what reading it found (JSON, NULL until it's read; cleared when the debrief changes)
-- and when it was saved. `notified`: the banners shown for a round (`kind`: `day`, `hour` or `debrief`), each once.
CREATE TABLE debriefs (
    round_id TEXT PRIMARY KEY REFERENCES rounds (id) ON DELETE CASCADE,
    debrief TEXT NOT NULL,
    read TEXT,
    at TEXT NOT NULL
);
CREATE TABLE notified (
    round_id TEXT NOT NULL REFERENCES rounds (id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    at TEXT NOT NULL,
    PRIMARY KEY (round_id, kind)
);
