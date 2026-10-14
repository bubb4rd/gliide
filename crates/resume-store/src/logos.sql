-- Version 6: fetched logos (ARCHITECTURE.md §7, Company logos). `companies.source`: yours (you set it), freehire
-- (freehire's logo for the name) or site (the company's own site's icon); a fetched logo never replaces yours. Rows
-- from before are yours. `logo_lookups`: each company key's last fetch and what it found: freehire, site, none, or
-- removed (you removed a fetched logo, so it isn't fetched again).
ALTER TABLE companies ADD COLUMN source TEXT NOT NULL DEFAULT 'yours';
CREATE TABLE logo_lookups (
    key TEXT PRIMARY KEY,
    found TEXT NOT NULL,
    tried_at TEXT NOT NULL
);
