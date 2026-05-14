use minion_db::Result;

pub fn run(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(MIGRATIONS).map_err(|e: rusqlite::Error| minion_db::Error::Migration(e.to_string()))?;
    Ok(())
}

const MIGRATIONS: &str = "
CREATE TABLE IF NOT EXISTS presentations (
    id             TEXT PRIMARY KEY,
    title          TEXT NOT NULL,
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL,
    bundle_path    TEXT NOT NULL,
    thumbnail      BLOB,
    schema_version TEXT NOT NULL DEFAULT '1.0'
);

CREATE TABLE IF NOT EXISTS generation_sessions (
    id               TEXT PRIMARY KEY,
    presentation_id  TEXT REFERENCES presentations(id) ON DELETE CASCADE,
    status           TEXT NOT NULL CHECK(status IN ('running','completed','failed','interrupted')),
    started_at       INTEGER NOT NULL,
    completed_at     INTEGER,
    last_checkpoint  TEXT,
    error            TEXT
);

CREATE TABLE IF NOT EXISTS slide_results (
    session_id   TEXT REFERENCES generation_sessions(id) ON DELETE CASCADE,
    slide_index  INTEGER NOT NULL,
    slide_id     TEXT NOT NULL,
    status       TEXT NOT NULL CHECK(status IN ('pending','completed','failed')),
    deck_patch   TEXT,
    PRIMARY KEY (session_id, slide_index)
);

CREATE TABLE IF NOT EXISTS user_assets (
    id          TEXT PRIMARY KEY,
    filename    TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK(kind IN ('image','svg','font','video')),
    checksum    TEXT NOT NULL,
    size_bytes  INTEGER NOT NULL,
    stored_at   TEXT NOT NULL,
    created_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_presentations_updated ON presentations(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_presentation ON generation_sessions(presentation_id);
";
