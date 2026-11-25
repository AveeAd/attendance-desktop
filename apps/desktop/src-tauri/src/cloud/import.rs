use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};

use crate::cloud::apply;
use crate::cloud::client::CloudClient;
use crate::cloud::config::{self, Credentials};
use crate::cloud::mapping::{self, ASSIGNMENT, DUTY_ROSTER, EMPLOYEE, EVENT, SHIFT};
use crate::db::{lock_conn, mask_to_days, now_iso, settings, Db};
use crate::error::AppResult;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: i64,
    pub cursor: String,
}

fn rows(conn: &Connection, sql: &str, map: impl Fn(&rusqlite::Row<'_>) -> AppResult<Value>) -> AppResult<Vec<Value>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map([], |row| Ok(map(row)))?
        .collect::<rusqlite::Result<Vec<AppResult<Value>>>>()?
        .into_iter()
        .collect::<AppResult<Vec<Value>>>()?;
    Ok(rows)
}

fn optional_string(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<String>> {
    row.get::<_, Option<String>>(index)
}

fn build_payload(conn: &Connection) -> AppResult<Value> {
    let updated_at = now_iso();

    let employees = rows(
        conn,
        "SELECT id, code, name, department, role, active, pin_hash, created_at FROM employees ORDER BY id",
        |row| {
            let id: i64 = row.get(0)?;
            let remote_id = mapping::ensure_uuid(conn, EMPLOYEE, id)?;
            Ok(json!({
                "id": remote_id,
                "code": row.get::<_, String>(1)?,
                "name": row.get::<_, String>(2)?,
                "department": optional_string(row, 3)?,
                "role": optional_string(row, 4)?,
                "active": row.get::<_, i64>(5)? != 0,
                "pin_hash": row.get::<_, String>(6)?,
                "created_at": row.get::<_, String>(7)?,
                "updated_at": updated_at,
            }))
        },
    )?;

    let shifts = rows(
        conn,
        "SELECT id, name, start_time, end_time, days_mask, break_minutes, active FROM shifts ORDER BY id",
        |row| {
            let id: i64 = row.get(0)?;
            let remote_id = mapping::ensure_uuid(conn, SHIFT, id)?;
            let mask: i64 = row.get(4)?;
            Ok(json!({
                "id": remote_id,
                "name": row.get::<_, String>(1)?,
                "start_time": row.get::<_, String>(2)?,
                "end_time": row.get::<_, String>(3)?,
                "days": mask_to_days(mask),
                "break_minutes": row.get::<_, i64>(5)?,
                "active": row.get::<_, i64>(6)? != 0,
                "updated_at": updated_at,
            }))
        },
    )?;

    let assignments = rows(
        conn,
        "SELECT id, employee_id, shift_id, effective_from, effective_to FROM shift_assignments ORDER BY id",
        |row| {
            let id: i64 = row.get(0)?;
            let remote_id = mapping::ensure_uuid(conn, ASSIGNMENT, id)?;
            let employee_id = mapping::ensure_uuid(conn, EMPLOYEE, row.get::<_, i64>(1)?)?;
            let shift_id = mapping::ensure_uuid(conn, SHIFT, row.get::<_, i64>(2)?)?;
            Ok(json!({
                "id": remote_id,
                "employee_id": employee_id,
                "shift_id": shift_id,
                "effective_from": row.get::<_, String>(3)?,
                "effective_to": optional_string(row, 4)?,
                "updated_at": updated_at,
            }))
        },
    )?;

    let duty_rosters = rows(
        conn,
        "SELECT id, employee_id, duty_date, shift_id, duty, notes, source, created_at FROM duty_rosters ORDER BY id",
        |row| {
            let id: i64 = row.get(0)?;
            let remote_id = mapping::ensure_uuid(conn, DUTY_ROSTER, id)?;
            let employee_id = mapping::ensure_uuid(conn, EMPLOYEE, row.get::<_, i64>(1)?)?;
            let shift_id = match row.get::<_, Option<i64>>(3)? {
                Some(shift_id) => Some(mapping::ensure_uuid(conn, SHIFT, shift_id)?),
                None => None,
            };
            Ok(json!({
                "id": remote_id,
                "employee_id": employee_id,
                "duty_date": row.get::<_, String>(2)?,
                "shift_id": shift_id,
                "duty": row.get::<_, String>(4)?,
                "notes": optional_string(row, 5)?,
                "source": row.get::<_, String>(6)?,
                "created_at": row.get::<_, String>(7)?,
                "updated_at": updated_at,
            }))
        },
    )?;

    let events = rows(
        conn,
        "SELECT id, employee_id, event_type, occurred_at, occurred_at_epoch, source, note FROM time_events ORDER BY id",
        |row| {
            let id: i64 = row.get(0)?;
            let remote_id = mapping::ensure_uuid(conn, EVENT, id)?;
            let employee_id = mapping::ensure_uuid(conn, EMPLOYEE, row.get::<_, i64>(1)?)?;
            Ok(json!({
                "id": remote_id,
                "employee_id": employee_id,
                "event_type": row.get::<_, String>(2)?,
                "occurred_at": row.get::<_, String>(3)?,
                "occurred_at_epoch": row.get::<_, i64>(4)?,
                "source": row.get::<_, String>(5)?,
                "note": optional_string(row, 6)?,
            }))
        },
    )?;

    let mut setting_values = serde_json::Map::new();
    for key in ["company_name", "overtime_daily_hours", "overtime_weekly_hours", "idle_lock_minutes"] {
        let value = settings::get_value(conn, key)?;
        if !value.trim().is_empty() {
            setting_values.insert(key.to_string(), Value::String(value));
        }
    }

    Ok(json!({
        "employees": employees,
        "shifts": shifts,
        "assignments": assignments,
        "duty_rosters": duty_rosters,
        "events": events,
        "settings": setting_values,
    }))
}

pub fn import_local(db: &Db, credentials: &Credentials) -> AppResult<ImportResult> {
    let client = CloudClient::new(credentials.clone())?;
    let payload = {
        let conn = lock_conn(db)?;
        build_payload(&conn)?
    };
    let result: Value = client.send_json(reqwest::Method::POST, "/api/v1/import", &payload, None)?;
    let imported = result
        .get("imported")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let snapshot: Value = client.get_json("/api/v1/bootstrap", &[])?;
    let cursor = snapshot
        .get("cursor")
        .and_then(Value::as_str)
        .unwrap_or("0")
        .to_string();
    {
        let conn = lock_conn(db)?;
        apply::apply_bootstrap(&conn, &snapshot)?;
        config::set_meta(&conn, "cloud_cursor", &cursor)?;
        config::set_meta(&conn, "cloud_imported", "1")?;
    }
    Ok(ImportResult { imported, cursor })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use rusqlite::params;
    use std::sync::{Arc, Mutex};

    fn test_db() -> Db {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../db/migrations.sql")).unwrap();
        Db {
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    #[test]
    fn import_payload_maps_local_ids_to_stable_uuids() {
        let db = test_db();
        let conn = lock_conn(&db).unwrap();
        conn.execute(
            "INSERT INTO employees(code, name, qr_token, created_at) VALUES ('E1', 'Jane', 'token', '2026-01-01T00:00:00Z')",
            params![],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO shifts(name, start_time, end_time, days_mask) VALUES ('Morning', '09:00', '17:00', 127)",
            params![],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO shift_assignments(employee_id, shift_id, effective_from) VALUES (1, 1, '2026-01-01')",
            params![],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO time_events(employee_id, event_type, occurred_at, occurred_at_epoch, source, created_at)
             VALUES (1, 'clock_in', '2026-01-01T09:00:00Z', 1767248400, 'scan', '2026-01-01T09:00:00Z')",
            params![],
        )
        .unwrap();

        let payload = build_payload(&conn).unwrap();
        let employee = &payload["employees"][0];
        let shift = &payload["shifts"][0];
        let assignment = &payload["assignments"][0];
        let event = &payload["events"][0];
        assert_eq!(employee["code"], "E1");
        assert_eq!(assignment["employee_id"], employee["id"]);
        assert_eq!(assignment["shift_id"], shift["id"]);
        assert_eq!(event["employee_id"], employee["id"]);
        assert_eq!(event["id"].as_str().unwrap().len(), 36);
        assert!(payload["settings"].as_object().unwrap().contains_key("company_name"));
        assert!(payload["settings"].as_object().unwrap().get("admin_pin_hash").is_none());
    }
}
