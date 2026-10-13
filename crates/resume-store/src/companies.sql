-- Version 3: companies by key (the name lowercased, each run of anything but letters and digits as one `_`), so a
-- posting and an application with the same name share one row. Holds the logo you set: the image as stored and its
-- media type (ARCHITECTURE.md §7, Company logos).
CREATE TABLE companies (
    id TEXT PRIMARY KEY,
    key TEXT NOT NULL UNIQUE,
    logo BLOB NOT NULL,
    logo_type TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
