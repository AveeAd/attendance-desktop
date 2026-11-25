use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;

use crate::db::now_iso;
use crate::error::AppResult;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxItemView {
    pub id: i64,
    pub kind: String,
    pub operation: String,
    pub local_id: i64,
    pub status: String,
    pub last_error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudConflict {
    pub id: i64,
    pub kind: String,
    pub local_id: i64,
    pub message: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone)]
pub struct OutboxRow {
    pub id: i64,
    pub kind: String,
    pub operation: String,
    pub local_id: i64,
    pub payload: String,
}

/// Duplicate keys are ignored so retries and repeated LAN scans cannot create
/// the same server event twice.
pub fn enqueue(
    conn: &Connection,
    idempotency_key: &str,
    kind: &str,
    operation: &str,
    local_id: i64,
    payload: &Value,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO outbox(idempotency_key, kind, operation, local_id, payload, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(idempotency_key) DO NOTHING",
        params![
            idempotency_key,
            kind,
            operation,
            local_id,
            payload.to_string(),
            now_iso()
        ],
    )?;
    Ok(())
}

pub fn list_pending(conn: &Connection) -> AppResult<Vec<OutboxRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, operation, local_id, payload FROM outbox
         WHERE status = 'pending' ORDER BY id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(OutboxRow {
                id: row.get(0)?,
                kind: row.get(1)?,
                operation: row.get(2)?,
                local_id: row.get(3)?,
                payload: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn mark_done(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM outbox WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn mark_failed(conn: &Connection, id: i64, error: &str, is_conflict: bool) -> AppResult<()> {
    let status = if is_conflict { "conflict" } else { "failed" };
    conn.execute(
        "UPDATE outbox SET status = ?2, attempts = attempts + 1, last_error = ?3 WHERE id = ?1",
        params![id, status, error],
    )?;
    Ok(())
}

pub fn record_conflict(conn: &Connection, kind: &str, local_id: i64, message: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO cloud_conflicts(kind, local_id, message, occurred_at) VALUES (?1, ?2, ?3, ?4)",
        params![kind, local_id, message, now_iso()],
    )?;
    Ok(())
}

pub fn list_conflicts(conn: &Connection) -> AppResult<Vec<CloudConflict>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, local_id, message, occurred_at FROM cloud_conflicts ORDER BY id DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(CloudConflict {
                id: row.get(0)?,
                kind: row.get(1)?,
                local_id: row.get(2)?,
                message: row.get(3)?,
                occurred_at: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn acknowledge(conn: &Connection, ids: &[i64]) -> AppResult<()> {
    if ids.is_empty() {
        conn.execute("DELETE FROM cloud_conflicts", [])?;
        return Ok(());
    }
    for id in ids {
        conn.execute("DELETE FROM cloud_conflicts WHERE id = ?1", params![id])?;
    }
    Ok(())
}

pub fn retry_failed(conn: &Connection) -> AppResult<i64> {
    let changed = conn.execute(
        "UPDATE outbox SET status = 'pending', last_error = NULL WHERE status = 'failed'",
        [],
    )?;
    Ok(changed as i64)
}

pub fn list_all(conn: &Connection) -> AppResult<Vec<OutboxItemView>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, operation, local_id, status, last_error, created_at FROM outbox ORDER BY id DESC LIMIT 200",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(OutboxItemView {
                id: row.get(0)?,
                kind: row.get(1)?,
                operation: row.get(2)?,
                local_id: row.get(3)?,
                status: row.get(4)?,
                last_error: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
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
    fn duplicate_idempotency_keys_are_ignored_and_failed_items_can_retry() {
        let db = test_db();
        let conn = crate::db::lock_conn(&db).unwrap();
        let payload = serde_json::json!({ "id": "event-1" });
        enqueue(&conn, "event-1", "event", "create", 7, &payload).unwrap();
        enqueue(&conn, "event-1", "event", "create", 7, &payload).unwrap();
        enqueue(&conn, "event-2", "event", "create", 8, &payload).unwrap();
        assert_eq!(list_pending(&conn).unwrap().len(), 2);

        let id = list_pending(&conn).unwrap()[0].id;
        mark_failed(&conn, id, "online again", false).unwrap();
        assert_eq!(list_pending(&conn).unwrap().len(), 1);
        assert_eq!(retry_failed(&conn).unwrap(), 1);
        assert_eq!(list_pending(&conn).unwrap().len(), 2);

        mark_done(&conn, id).unwrap();
        assert_eq!(list_pending(&conn).unwrap().len(), 1);
    }

    #[test]
    fn conflicts_are_durable_until_acknowledged() {
        let db = test_db();
        let conn = crate::db::lock_conn(&db).unwrap();
        enqueue(&conn, "key", "employee", "upsert", 1, &serde_json::json!({})).unwrap();
        let id = list_pending(&conn).unwrap()[0].id;
        mark_failed(&conn, id, "stale version", true).unwrap();
        record_conflict(&conn, "employee", 1, "stale version").unwrap();

        let conflicts = list_conflicts(&conn).unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].kind, "employee");
        acknowledge(&conn, &[conflicts[0].id]).unwrap();
        assert!(list_conflicts(&conn).unwrap().is_empty());
        assert!(list_all(&conn).unwrap().iter().any(|item| item.id == id));
    }
}
