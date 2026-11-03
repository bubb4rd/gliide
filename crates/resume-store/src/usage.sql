-- Version 14: each model call's usage (Settings → Usage), a row per call, its repair turn included: when it ran (`at`)
-- and the local day it ran on, what it was for (`feature`), the agent and the model that served it, its tokens (input
-- read fresh; cache writes and reads apart from it) and what it cost at API prices when the agent said so (Claude Code
-- does; NULL: estimated from list prices when shown, so a corrected price list applies to past calls too).
CREATE TABLE usage (
    id INTEGER PRIMARY KEY,
    at TEXT NOT NULL,
    day TEXT NOT NULL,
    feature TEXT NOT NULL,
    agent TEXT NOT NULL,
    model TEXT NOT NULL,
    input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    cache_write_tokens INTEGER NOT NULL,
    cache_read_tokens INTEGER NOT NULL,
    cost_usd REAL
);
