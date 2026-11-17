-- Version 15: each model call's wall time from spawn to answer, its repair turn included (milliseconds), so Settings
-- can say how fast each role's agent and model answer on this Mac. NULL: recorded before it was kept.
ALTER TABLE usage ADD COLUMN latency_ms INTEGER;
