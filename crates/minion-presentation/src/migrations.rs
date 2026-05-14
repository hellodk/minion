use minion_db::Result;

pub fn run(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(MIGRATIONS).map_err(|e: rusqlite::Error| minion_db::Error::Migration(e.to_string()))?;
    Ok(())
}

const MIGRATIONS: &str = "
-- placeholder: filled in Task 5
";
