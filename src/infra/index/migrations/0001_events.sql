-- Every event taken in from the spool, by id, so taking a line twice keeps
-- one row. `source` and `kind` are the spool's names for them.
CREATE TABLE events (
    id      TEXT PRIMARY KEY,
    v       INTEGER NOT NULL,
    at      TEXT NOT NULL,
    source  TEXT NOT NULL,
    kind    TEXT NOT NULL,
    session TEXT,
    cwd     TEXT,
    branch  TEXT,
    head    TEXT,
    data    TEXT
);
CREATE INDEX events_at ON events (at);
CREATE INDEX events_session ON events (session, at);

-- How far into each spool file the index has read, in bytes.
CREATE TABLE ingest_offsets (
    file   TEXT PRIMARY KEY,
    offset INTEGER NOT NULL
);
