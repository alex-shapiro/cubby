BEGIN;

CREATE TABLE metadata (
    local_id INTEGER NOT NULL
);

CREATE TABLE peers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    public_id BLOB NOT NULL UNIQUE,
    bookmark INTEGER NOT NULL
);

CREATE TABLE bitmap_state (
    peer_id INTEGER PRIMARY KEY AUTOINCREMENT,
    state BLOB NOT NULL
);

CREATE TABLE entries (
    key BLOB NOT NULL PRIMARY KEY,
    value BLOB NOT NULL,
    peer_id INTEGER NOT NULL,
    hlc INTEGER NOT NULL
);

CREATE INDEX kvstore_edit ON entries(peer_id, hlc);

COMMIT;
