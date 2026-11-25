use reqwest::Method;
use serde::Serialize;
use serde_json::Value;

use crate::cloud::apply;
use crate::cloud::client::CloudClient;
use crate::cloud::config;
use crate::cloud::import;
use crate::cloud::mapping::{self, ASSIGNMENT, DUTY_ROSTER, EMPLOYEE, EVENT, SETTING, SHIFT};
use crate::cloud::outbox;
use crate::db::{lock_conn, now_iso, Db};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub pushed: i64,
    pub pulled: i64,
    pub conflicts: i64,
    pub cursor: String,
    pub synced_at: String,
}

fn delete_ignoring_missing(client: &CloudClient, path: &str) -> AppResult<()> {
    match client.request::<()>(Method::DELETE, path, &[], None, None) {
        Ok(_) => Ok(()),
        Err(AppError::CloudHttp { status: 404, .. }) => Ok(()),
        Err(error) => Err(error),
    }
}

fn version_of(value: &Value) -> Option<String> {
    value
        .get("updated_at")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn push_item(client: &CloudClient, kind: &str, operation: &str, payload: &Value) -> AppResult<Option<String>> {
    let remote_id = payload
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    match (kind, operation) {
        (EMPLOYEE, "upsert") => {
            let path = format!("/api/v1/employees/{remote_id}");
            let response: Value = client.send_json(Method::PUT, &path, payload, None)?;
            Ok(version_of(&response))
        }
        (SHIFT, "upsert") => {
            let path = format!("/api/v1/shifts/{remote_id}");
            let response: Value = client.send_json(Method::PUT, &path, payload, None)?;
            Ok(version_of(&response))
        }
        (ASSIGNMENT, "upsert") => {
            let path = format!("/api/v1/assignments/{remote_id}");
            let response: Value = client.send_json(Method::PUT, &path, payload, None)?;
            Ok(version_of(&response))
        }
        (DUTY_ROSTER, "upsert") => {
            let path = format!("/api/v1/duty-rosters/{remote_id}");
            let response: Value = client.send_json(Method::PUT, &path, payload, None)?;
            Ok(version_of(&response))
        }
        (EMPLOYEE, "delete") => {
            delete_ignoring_missing(client, &format!("/api/v1/employees/{remote_id}"))?;
            Ok(None)
        }
        (SHIFT, "delete") => {
            delete_ignoring_missing(client, &format!("/api/v1/shifts/{remote_id}"))?;
            Ok(None)
        }
        (ASSIGNMENT, "delete") => {
            delete_ignoring_missing(client, &format!("/api/v1/assignments/{remote_id}"))?;
            Ok(None)
        }
        (DUTY_ROSTER, "delete") => {
            delete_ignoring_missing(client, &format!("/api/v1/duty-rosters/{remote_id}"))?;
            Ok(None)
        }
        (EVENT, "create") => {
            client.send_json::<_, Value>(Method::POST, "/api/v1/events", payload, None)?;
            Ok(None)
        }
        (EVENT, "delete") => {
            delete_ignoring_missing(client, &format!("/api/v1/events/{remote_id}"))?;
            Ok(None)
        }
        (SETTING, "upsert") => {
            let key = payload
                .get("key")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let base = payload
                .get("updated_at")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let body = serde_json::json!({ "value": payload.get("value").cloned().unwrap_or(Value::Null) });
            client.send_json::<_, Value>(
                Method::PUT,
                &format!("/api/v1/settings/{key}"),
                &body,
                Some(base),
            )?;
            Ok(None)
        }
        _ => Err(AppError::Cloud(format!(
            "Unknown outbox operation: {kind}.{operation}"
        ))),
    }
}

fn push_pending(db: &Db, client: &CloudClient) -> AppResult<(i64, i64)> {
    let items = {
        let conn = lock_conn(db)?;
        outbox::list_pending(&conn)?
    };
    let mut pushed = 0;
    let mut conflicts = 0;

    for item in items {
        let payload: Value = serde_json::from_str(&item.payload).unwrap_or(Value::Null);
        match push_item(client, &item.kind, &item.operation, &payload) {
            Ok(version) => {
                let conn = lock_conn(db)?;
                outbox::mark_done(&conn, item.id)?;
                if let Some(version) = version {
                    let kind = item.kind.as_str();
                    if kind != SETTING {
                        mapping::set_version(&conn, kind, item.local_id, &version)?;
                    }
                }
                pushed += 1;
            }
            Err(AppError::CloudConflict { message }) => {
                let conn = lock_conn(db)?;
                outbox::mark_failed(&conn, item.id, &message, true)?;
                outbox::record_conflict(&conn, &item.kind, item.local_id, &message)?;
                if item.kind == EVENT {
                    conn.execute(
                        "DELETE FROM time_events WHERE id = ?1",
                        rusqlite::params![item.local_id],
                    )?;
                    mapping::remove(&conn, EVENT, item.local_id)?;
                }
                conflicts += 1;
            }
            Err(error @ AppError::CloudHttp { status, .. }) if status < 500 => {
                let conn = lock_conn(db)?;
                outbox::mark_failed(&conn, item.id, &error.to_string(), false)?;
            }
            Err(error) => {
                let conn = lock_conn(db)?;
                outbox::mark_failed(&conn, item.id, &error.to_string(), false)?;
                return Err(error);
            }
        }
    }
    Ok((pushed, conflicts))
}

fn apply_changes(conn: &rusqlite::Connection, response: &Value) -> AppResult<i64> {
    let changes = response
        .get("changes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for change in &changes {
        let entity = change
            .get("entity")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let operation = change
            .get("operation")
            .and_then(Value::as_str)
            .unwrap_or("upsert");
        let payload = change.get("payload").unwrap_or(&Value::Null);
        apply::apply_change(conn, entity, operation, payload)?;
    }
    Ok(changes.len() as i64)
}

fn pull(db: &Db, client: &CloudClient) -> AppResult<(i64, String)> {
    let (imported, cursor) = {
        let conn = lock_conn(db)?;
        (
            config::get_meta(&conn, "cloud_imported")? == "1",
            config::get_meta(&conn, "cloud_cursor")?,
        )
    };

    if !imported {
        let snapshot: Value = client.get_json("/api/v1/bootstrap", &[])?;
        let cursor = snapshot
            .get("cursor")
            .and_then(Value::as_str)
            .unwrap_or("0")
            .to_string();
        let conn = lock_conn(db)?;
        let pulled = apply::apply_bootstrap(&conn, &snapshot)?;
        config::set_meta(&conn, "cloud_cursor", &cursor)?;
        config::set_meta(&conn, "cloud_imported", "1")?;
        return Ok((pulled, cursor));
    }

    let response: Value = client.get_json(
        "/api/v1/changes",
        &[("since", cursor.clone())],
    )?;
    let cursor = response
        .get("cursor")
        .and_then(Value::as_str)
        .unwrap_or(&cursor)
        .to_string();
    let conn = lock_conn(db)?;
    let pulled = apply_changes(&conn, &response)?;
    config::set_meta(&conn, "cloud_cursor", &cursor)?;
    Ok((pulled, cursor))
}

pub fn sync_now(db: &Db) -> AppResult<SyncResult> {
    let credentials = {
        let conn = lock_conn(db)?;
        config::credentials(&conn)?
    };
    let Some(credentials) = credentials else {
        return Err(AppError::Cloud("Cloud sync is not configured".to_string()));
    };
    let client = CloudClient::new(credentials)?;
    {
        let conn = lock_conn(db)?;
        outbox::retry_failed(&conn)?;
    }
    let (pushed, conflicts) = push_pending(db, &client)?;
    let (pulled, cursor) = pull(db, &client)?;
    let synced_at = now_iso();
    {
        let conn = lock_conn(db)?;
        config::set_meta(&conn, "cloud_last_sync", &synced_at)?;
        config::set_meta(&conn, "cloud_last_error", "")?;
    }
    Ok(SyncResult {
        pushed,
        pulled,
        conflicts,
        cursor,
        synced_at,
    })
}

pub fn configure(db: &Db, api_url: &str, api_key: &str, import_existing: bool) -> AppResult<SyncResult> {
    {
        let conn = lock_conn(db)?;
        config::save(&conn, api_url, api_key)?;
    }
    if import_existing {
        let credentials = {
            let conn = lock_conn(db)?;
            config::credentials(&conn)?.ok_or_else(|| {
                AppError::Cloud("Cloud sync is not configured".to_string())
            })?
        };
        let outcome = import::import_local(db, &credentials)?;
        if outcome.cursor.is_empty() {
            return Err(AppError::Cloud("Server did not return a sync cursor".to_string()));
        }
    } else {
        let conn = lock_conn(db)?;
        apply::clear_cache(&conn)?;
        // The local cache was discarded, so a changes-since request would miss
        // everything already on the server. Force a fresh bootstrap pull.
        config::set_meta(&conn, "cloud_cursor", "0")?;
        config::set_meta(&conn, "cloud_imported", "")?;
        config::set_meta(&conn, "cloud_last_sync", "")?;
    }
    sync_now(db)
}

pub fn sync_if_configured(db: &Db) -> AppResult<Option<SyncResult>> {
    if !{
        let conn = lock_conn(db)?;
        config::is_configured(&conn)?
    } {
        return Ok(None);
    }
    sync_now(db).map(Some)
}

pub fn fail(db: &Db, error: &str) {
    if let Ok(conn) = lock_conn(db) {
        let _ = config::set_meta(&conn, "cloud_last_error", error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_requires_configuration() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../db/migrations.sql")).unwrap();
        let db = Db {
            conn: std::sync::Arc::new(std::sync::Mutex::new(conn)),
        };
        assert!(sync_now(&db).is_err());
    }
}
