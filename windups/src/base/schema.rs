//! Schema versions: `PRAGMA user_version` is the number of migrations applied. Migrations run
//! forward only, in order, one transaction each, so a failed one leaves the database as it was.
use rusqlite::Connection;

pub fn version(c: &Connection) -> Result<i64, String> {
    c.query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())
}

/// Applies every migration after the current version. `what` names the database in messages.
/// Returns the versions applied (empty when already current).
pub fn migrate(c: &mut Connection, what: &str, migrations: &[&str]) -> Result<Vec<i64>, String> {
    let current = version(c)?;
    let latest = migrations.len() as i64;
    if current > latest {
        return Err(format!(
            "{what} is version {current}, newer than this windup knows ({latest}); please update"
        ));
    }
    let mut applied = Vec::new();
    for (i, sql) in migrations.iter().enumerate().skip(current as usize) {
        let v = (i + 1) as i64;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(sql)
            .map_err(|e| format!("{what} migration {v}: {e}"))?;
        tx.pragma_update(None, "user_version", v)
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        applied.push(v);
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: &[&str] = &[
        "CREATE TABLE a(x INTEGER);",
        "ALTER TABLE a ADD COLUMN y TEXT;",
        "CREATE TABLE b(z INTEGER);",
    ];

    #[test]
    fn applies_in_order_and_only_once() {
        let mut c = Connection::open_in_memory().unwrap();
        assert_eq!(migrate(&mut c, "t.db", &M[..2]).unwrap(), vec![1, 2]);
        c.execute("INSERT INTO a(x, y) VALUES (1, 'kept')", [])
            .unwrap();
        assert_eq!(
            migrate(&mut c, "t.db", M).unwrap(),
            vec![3],
            "forward from 2"
        );
        assert!(migrate(&mut c, "t.db", M).unwrap().is_empty());
        assert_eq!(version(&c).unwrap(), 3);
        let y: String = c.query_row("SELECT y FROM a", [], |r| r.get(0)).unwrap();
        assert_eq!(y, "kept");
    }

    #[test]
    fn a_failed_migration_rolls_back_and_newer_is_refused() {
        let mut c = Connection::open_in_memory().unwrap();
        let bad = [M[0], "CREATE TABLE c(q); THIS IS NOT SQL;"];
        let e = migrate(&mut c, "t.db", &bad).unwrap_err();
        assert!(e.contains("t.db migration 2"), "{e}");
        assert_eq!(version(&c).unwrap(), 1);
        let n: i64 = c
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'c'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0, "half a migration is never left behind");
        migrate(&mut c, "t.db", M).unwrap();
        let e = migrate(&mut c, "t.db", &M[..1]).unwrap_err();
        assert!(e.contains("newer than this windup knows"), "{e}");
    }
}
