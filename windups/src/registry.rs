//! Windups introduce themselves: each one's profile (`<name> -h -j`) is kept in the
//! `windups` table, so others (Walter, Margaret) can find who does what.
use rusqlite::{Connection, params};

pub fn register(c: &Connection, name: &str, version: &str, profile: &str) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO windups(name, version, profile) VALUES (?1, ?2, ?3)
         ON CONFLICT(name) DO UPDATE SET version = excluded.version, profile = excluded.profile, updated_at = CURRENT_TIMESTAMP",
        params![name, version, profile],
    )?;
    Ok(())
}

/// (name, profile) of every registered windup.
pub fn all(c: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    c.prepare("SELECT name, profile FROM windups ORDER BY name")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}
