use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::error::AppResult;

pub const EMPLOYEE: &str = "employee";
pub const SHIFT: &str = "shift";
pub const ASSIGNMENT: &str = "assignment";
pub const DUTY_ROSTER: &str = "duty_roster";
pub const EVENT: &str = "event";
pub const SETTING: &str = "setting";

#[derive(Debug, Clone)]
pub struct RemoteRef {
    pub remote_id: String,
    pub version: String,
}

pub fn get(conn: &Connection, kind: &str, local_id: i64) -> AppResult<Option<RemoteRef>> {
    let found = conn
        .query_row(
            "SELECT remote_id, version FROM cloud_ids WHERE kind = ?1 AND local_id = ?2",
            params![kind, local_id],
            |row| {
                Ok(RemoteRef {
                    remote_id: row.get(0)?,
                    version: row.get(1)?,
                })
            },
        )
        .optional()?;
    Ok(found)
}

pub fn get_by_remote(conn: &Connection, kind: &str, remote_id: &str) -> AppResult<Option<i64>> {
    let found = conn
        .query_row(
            "SELECT local_id FROM cloud_ids WHERE kind = ?1 AND remote_id = ?2 LIMIT 1",
            params![kind, remote_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found)
}

pub fn set(conn: &Connection, kind: &str, local_id: i64, remote_id: &str, version: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM cloud_ids WHERE kind = ?1 AND remote_id = ?2 AND local_id <> ?3",
        params![kind, remote_id, local_id],
    )?;
    conn.execute(
        "INSERT INTO cloud_ids(kind, local_id, remote_id, version) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(kind, local_id) DO UPDATE SET remote_id = excluded.remote_id, version = excluded.version",
        params![kind, local_id, remote_id, version],
    )?;
    Ok(())
}

pub fn set_version(conn: &Connection, kind: &str, local_id: i64, version: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE cloud_ids SET version = ?3 WHERE kind = ?1 AND local_id = ?2",
        params![kind, local_id, version],
    )?;
    Ok(())
}

pub fn remove(conn: &Connection, kind: &str, local_id: i64) -> AppResult<()> {
    conn.execute(
        "DELETE FROM cloud_ids WHERE kind = ?1 AND local_id = ?2",
        params![kind, local_id],
    )?;
    Ok(())
}

/// Gives every local record a stable UUID before it is first uploaded. The same
/// UUID is used as the event idempotency key on the server.
pub fn ensure_uuid(conn: &Connection, kind: &str, local_id: i64) -> AppResult<String> {
    if let Some(existing) = get(conn, kind, local_id)? {
        return Ok(existing.remote_id);
    }
    let remote_id = Uuid::new_v4().to_string();
    set(conn, kind, local_id, &remote_id, "")?;
    Ok(remote_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn test_db() -> Db {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../db/migrations.sql")).unwrap();
        Db {
            conn: std::sync::Arc::new(std::sync::Mutex::new(conn)),
        }
    }

    #[test]
    fn uuids_are_stable_and_rebind_to_server_ids() {
        let db = test_db();
        let conn = crate::db::lock_conn(&db).unwrap();
        let first = ensure_uuid(&conn, EMPLOYEE, 1).unwrap();
        assert_eq!(first, ensure_uuid(&conn, EMPLOYEE, 1).unwrap());
        assert_ne!(first, ensure_uuid(&conn, EMPLOYEE, 2).unwrap());

        set(&conn, EMPLOYEE, 1, "server-id", "2026-11-25T00:00:00Z").unwrap();
        assert_eq!(get_by_remote(&conn, EMPLOYEE, "server-id").unwrap(), Some(1));
        let reference = get(&conn, EMPLOYEE, 1).unwrap().unwrap();
        assert_eq!(reference.remote_id, "server-id");
        assert!(!reference.version.is_empty());

        remove(&conn, EMPLOYEE, 1).unwrap();
        assert!(get(&conn, EMPLOYEE, 1).unwrap().is_none());
    }
}
