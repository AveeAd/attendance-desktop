use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

use crate::db::settings;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudConfigView {
    pub configured: bool,
    pub api_url: String,
    pub device_id: String,
    pub key_hint: String,
    pub imported: bool,
    pub last_sync: Option<String>,
    pub last_error: Option<String>,
    pub pending: i64,
    pub conflicts: i64,
}

#[derive(Debug, Clone)]
pub struct Credentials {
    pub api_url: String,
    pub api_key: String,
    pub device_id: String,
}

fn optional(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn hint(key: &str) -> String {
    let key = key.trim();
    if key.len() <= 8 {
        return "key".to_string();
    }
    format!("{}…{}", &key[..4], &key[key.len() - 4..])
}

pub fn ensure_device_id(conn: &Connection) -> AppResult<String> {
    let existing = settings::get_value(conn, "cloud_device_id")?;
    if !existing.trim().is_empty() {
        return Ok(existing);
    }
    let device_id = Uuid::new_v4().to_string();
    settings::set_value(conn, "cloud_device_id", &device_id)?;
    Ok(device_id)
}

pub fn credentials(conn: &Connection) -> AppResult<Option<Credentials>> {
    let api_url = settings::get_value(conn, "cloud_api_url")?;
    let api_key = settings::get_value(conn, "cloud_api_key")?;
    if api_url.trim().is_empty() || api_key.trim().is_empty() {
        return Ok(None);
    }
    let device_id = ensure_device_id(conn)?;
    Ok(Some(Credentials {
        api_url: api_url.trim().trim_end_matches('/').to_string(),
        api_key: api_key.trim().to_string(),
        device_id,
    }))
}

pub fn save(conn: &Connection, api_url: &str, api_key: &str) -> AppResult<CloudConfigView> {
    let url = api_url.trim().trim_end_matches('/');
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(AppError::InvalidInput(
            "Server URL must start with http:// or https://".to_string(),
        ));
    }
    let key = api_key.trim();
    if key.is_empty() {
        return Err(AppError::InvalidInput("API key is required".to_string()));
    }

    let previous_url = settings::get_value(conn, "cloud_api_url")?;
    let previous_key = settings::get_value(conn, "cloud_api_key")?;
    if previous_url.trim() != url || previous_key.trim() != key {
        settings::set_value(conn, "cloud_cursor", "0")?;
        settings::set_value(conn, "cloud_imported", "")?;
    }
    settings::set_value(conn, "cloud_api_url", url)?;
    settings::set_value(conn, "cloud_api_key", key)?;
    settings::set_value(conn, "cloud_last_error", "")?;
    ensure_device_id(conn)?;
    view(conn)
}

pub fn disconnect(conn: &Connection) -> AppResult<CloudConfigView> {
    settings::set_value(conn, "cloud_api_url", "")?;
    settings::set_value(conn, "cloud_api_key", "")?;
    settings::set_value(conn, "cloud_last_error", "")?;
    view(conn)
}

pub fn view(conn: &Connection) -> AppResult<CloudConfigView> {
    let api_url = settings::get_value(conn, "cloud_api_url")?;
    let api_key = settings::get_value(conn, "cloud_api_key")?;
    let device_id = settings::get_value(conn, "cloud_device_id")?;
    let pending = conn.query_row(
        // Failed network writes are automatically retried on the next sync,
        // so they are still pending from the user's point of view.
        "SELECT COUNT(*) FROM outbox WHERE status IN ('pending', 'failed')",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    let conflicts = conn.query_row(
        "SELECT COUNT(*) FROM cloud_conflicts",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(CloudConfigView {
        configured: !api_url.trim().is_empty() && !api_key.trim().is_empty(),
        api_url,
        device_id,
        key_hint: if api_key.trim().is_empty() {
            String::new()
        } else {
            hint(&api_key)
        },
        imported: settings::get_value(conn, "cloud_imported")?.trim() == "1",
        last_sync: optional(settings::get_value(conn, "cloud_last_sync")?),
        last_error: optional(settings::get_value(conn, "cloud_last_error")?),
        pending,
        conflicts,
    })
}

pub fn is_configured(conn: &Connection) -> AppResult<bool> {
    Ok(credentials(conn)?.is_some())
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    settings::set_value(conn, key, value)
}

pub fn get_meta(conn: &Connection, key: &str) -> AppResult<String> {
    settings::get_value(conn, key)
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
    fn config_keeps_api_key_out_of_the_view() {
        let db = test_db();
        let conn = crate::db::lock_conn(&db).unwrap();
        let view = save(&conn, "https://api.example.com/", "ak_live_1234567890abcdef").unwrap();
        assert!(view.configured);
        assert_eq!(view.api_url, "https://api.example.com");
        assert_eq!(view.key_hint, "ak_l…cdef");
        assert!(!serde_json::to_string(&view).unwrap().contains("1234567890abcdef"));

        let creds = credentials(&conn).unwrap().unwrap();
        assert_eq!(creds.api_key, "ak_live_1234567890abcdef");
        assert_eq!(creds.device_id.len(), 36);
        assert_eq!(ensure_device_id(&conn).unwrap(), creds.device_id);
    }

    #[test]
    fn invalid_urls_are_rejected() {
        let db = test_db();
        let conn = crate::db::lock_conn(&db).unwrap();
        assert!(save(&conn, "api.example.com", "key").is_err());
        assert!(save(&conn, "https://api.example.com", "").is_err());
    }

    #[test]
    fn failed_writes_still_count_as_pending() {
        let db = test_db();
        let conn = crate::db::lock_conn(&db).unwrap();
        conn.execute(
            "INSERT INTO outbox(idempotency_key, kind, operation, local_id, payload, status, created_at)
             VALUES ('pending', 'event', 'create', 1, '{}', 'pending', ?1)",
            [crate::db::now_iso()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO outbox(idempotency_key, kind, operation, local_id, payload, status, created_at)
             VALUES ('failed', 'event', 'create', 2, '{}', 'failed', ?1)",
            [crate::db::now_iso()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO outbox(idempotency_key, kind, operation, local_id, payload, status, created_at)
             VALUES ('conflict', 'event', 'create', 3, '{}', 'conflict', ?1)",
            [crate::db::now_iso()],
        )
        .unwrap();
        assert_eq!(view(&conn).unwrap().pending, 2);
    }
}
