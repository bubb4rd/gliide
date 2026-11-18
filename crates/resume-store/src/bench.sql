-- Version 16: Settings → Benchmark. `analyses.agent`: the agent of the Suggest route that wrote the analysis (as
-- `usage.agent` names it), so what you kept can be told apart by route; '' for analyses written before it was kept.
-- `change_outcomes`: what became of each change of an exported resume, a row per change, replaced on every export of
-- the application: `in`, `off`, `forced`, or left out by the validator (`left_out_validator`) or the fit check
-- (`left_out_fit`); `explicit` when you decided it yourself, `edited` when you reworded it. Rows outlive their
-- analysis (`analysis_id` cleared) but not their application. `bench_runs`: each benchmark call's result, one per
-- role, route, item and run number, replaced when run again; `metrics` is the run's measures as JSON (NULL when it
-- failed).
ALTER TABLE analyses ADD COLUMN agent TEXT NOT NULL DEFAULT '';
CREATE TABLE change_outcomes (
    id INTEGER PRIMARY KEY,
    application_id TEXT NOT NULL REFERENCES applications (id) ON DELETE CASCADE,
    analysis_id TEXT REFERENCES analyses (id) ON DELETE SET NULL,
    key TEXT NOT NULL,
    agent TEXT NOT NULL,
    model TEXT NOT NULL,
    op TEXT NOT NULL,
    origin TEXT NOT NULL,
    outcome TEXT NOT NULL,
    explicit INTEGER NOT NULL,
    edited INTEGER NOT NULL,
    at TEXT NOT NULL,
    UNIQUE (application_id, key)
);
CREATE INDEX change_outcomes_route ON change_outcomes (agent, model);
CREATE INDEX change_outcomes_analysis ON change_outcomes (analysis_id);
CREATE TABLE bench_runs (
    id INTEGER PRIMARY KEY,
    role TEXT NOT NULL,
    agent TEXT NOT NULL,
    model TEXT NOT NULL,
    item TEXT NOT NULL,
    n INTEGER NOT NULL,
    outcome TEXT NOT NULL,
    error TEXT,
    latency_ms INTEGER NOT NULL,
    input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    cache_write_tokens INTEGER NOT NULL,
    cache_read_tokens INTEGER NOT NULL,
    cost_usd REAL,
    served TEXT,
    metrics TEXT,
    at TEXT NOT NULL,
    UNIQUE (role, agent, model, item, n)
);
