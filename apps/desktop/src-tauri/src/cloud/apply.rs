use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use uuid::Uuid;

use crate::cloud::mapping::{self, ASSIGNMENT, DUTY_ROSTER, EMPLOYEE, EVENT, SHIFT};
use crate::db::{days_to_mask, now_iso};
use crate::error::AppResult;

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn text_or(value: &Value, key: &str, fallback: &str) -> String {
    text(value, key).unwrap_or_else(|| fallback.to_string())
}

fn integer(value: &Value, key: &str) -> Option<i64> {
    match value.get(key) {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(raw)) => raw.parse().ok(),
        _ => None,
    }
}

fn flag(value: &Value, key: &str, default: bool) -> bool {
    match value.get(key) {
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(value)) => value.as_i64().unwrap_or(0) != 0,
        _ => default,
    }
}

fn days(value: &Value) -> Vec<i64> {
    value
        .get("days")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(|day| day.as_i64()).collect())
        .unwrap_or_default()
}

fn new_qr_token() -> String {
    Uuid::new_v4().simple().to_string()
}

fn local_employee_id(conn: &Connection, payload: &Value) -> AppResult<i64> {
    if let Some(remote_id) = text(payload, "id") {
        if let Some(local_id) = mapping::get_by_remote(conn, EMPLOYEE, &remote_id)? {
            return Ok(local_id);
        }
    }
    if let Some(code) = text(payload, "code") {
        let found = conn
            .query_row(
                "SELECT id FROM employees WHERE lower(code) = lower(?1) LIMIT 1",
                params![code],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(local_id) = found {
            return Ok(local_id);
        }
    }
    if let Some(remote_id) = text(payload, "id") {
        conn.execute(
            "INSERT INTO employees(code, name, department, role, active, pin_hash, qr_token, created_at)
             VALUES (?1, ?2, NULL, NULL, 1, '', ?3, ?4)",
            params![
                format!("remote-{remote_id}"),
                format!("Employee {remote_id}"),
                new_qr_token(),
                now_iso()
            ],
        )?;
        return Ok(conn.last_insert_rowid());
    }
    Err(crate::error::AppError::Cloud(
        "Employee change is missing an id".to_string(),
    ))
}

fn local_shift_id(conn: &Connection, remote_id: &str) -> AppResult<i64> {
    if let Some(local_id) = mapping::get_by_remote(conn, SHIFT, remote_id)? {
        return Ok(local_id);
    }
    conn.execute(
        "INSERT INTO shifts(name, start_time, end_time, days_mask, break_minutes, active)
         VALUES (?1, '00:00', '00:00', 0, 0, 1)",
        params![format!("Shift {remote_id}")],
    )?;
    let local_id = conn.last_insert_rowid();
    mapping::set(conn, SHIFT, local_id, remote_id, "")?;
    Ok(local_id)
}

pub fn apply_employee(conn: &Connection, payload: &Value) -> AppResult<i64> {
    let local_id = local_employee_id(conn, payload)?;
    conn.execute(
        "UPDATE employees SET code = ?2, name = ?3, department = ?4, role = ?5,
             active = ?6, pin_hash = ?7 WHERE id = ?1",
        params![
            local_id,
            text_or(payload, "code", &format!("employee-{local_id}")),
            text_or(payload, "name", &format!("Employee {local_id}")),
            text(payload, "department"),
            text(payload, "role"),
            flag(payload, "active", true) as i64,
            text(payload, "pin_hash").unwrap_or_default(),
        ],
    )?;
    if let Some(remote_id) = text(payload, "id") {
        mapping::set(
            conn,
            EMPLOYEE,
            local_id,
            &remote_id,
            &text(payload, "updated_at").unwrap_or_default(),
        )?;
    }
    Ok(local_id)
}

pub fn apply_shift(conn: &Connection, payload: &Value) -> AppResult<i64> {
    let remote_id = text(payload, "id")
        .ok_or_else(|| crate::error::AppError::Cloud("Shift change is missing an id".to_string()))?;
    let local_id = match mapping::get_by_remote(conn, SHIFT, &remote_id)? {
        Some(id) => id,
        None => {
            conn.execute(
                "INSERT INTO shifts(name, start_time, end_time, days_mask, break_minutes, active)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    text_or(payload, "name", "Shift"),
                    text_or(payload, "start_time", "00:00"),
                    text_or(payload, "end_time", "00:00"),
                    days_to_mask(&days(payload)),
                    integer(payload, "break_minutes").unwrap_or(0),
                    flag(payload, "active", true) as i64,
                ],
            )?;
            let id = conn.last_insert_rowid();
            mapping::set(conn, SHIFT, id, &remote_id, "")?;
            return Ok(id);
        }
    };
    conn.execute(
        "UPDATE shifts SET name = ?2, start_time = ?3, end_time = ?4, days_mask = ?5,
             break_minutes = ?6, active = ?7 WHERE id = ?1",
        params![
            local_id,
            text_or(payload, "name", "Shift"),
            text_or(payload, "start_time", "00:00"),
            text_or(payload, "end_time", "00:00"),
            days_to_mask(&days(payload)),
            integer(payload, "break_minutes").unwrap_or(0),
            flag(payload, "active", true) as i64,
        ],
    )?;
    mapping::set_version(
        conn,
        SHIFT,
        local_id,
        &text(payload, "updated_at").unwrap_or_default(),
    )?;
    Ok(local_id)
}

pub fn apply_assignment(conn: &Connection, payload: &Value) -> AppResult<i64> {
    let remote_id = text(payload, "id").ok_or_else(|| {
        crate::error::AppError::Cloud("Assignment change is missing an id".to_string())
    })?;
    let employee_remote = text(payload, "employee_id").unwrap_or_default();
    let shift_remote = text(payload, "shift_id").unwrap_or_default();
    let employee_id = local_employee_id(
        conn,
        &serde_json::json!({ "id": employee_remote, "code": format!("remote-{employee_remote}") }),
    )?;
    let shift_id = local_shift_id(conn, &shift_remote)?;
    let local_id = match mapping::get_by_remote(conn, ASSIGNMENT, &remote_id)? {
        Some(id) => id,
        None => {
            conn.execute(
                "INSERT INTO shift_assignments(employee_id, shift_id, effective_from, effective_to)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    employee_id,
                    shift_id,
                    text_or(payload, "effective_from", "1970-01-01"),
                    text(payload, "effective_to")
                ],
            )?;
            let id = conn.last_insert_rowid();
            mapping::set(conn, ASSIGNMENT, id, &remote_id, "")?;
            return Ok(id);
        }
    };
    conn.execute(
        "UPDATE shift_assignments SET employee_id = ?2, shift_id = ?3,
             effective_from = ?4, effective_to = ?5 WHERE id = ?1",
        params![
            local_id,
            employee_id,
            shift_id,
            text_or(payload, "effective_from", "1970-01-01"),
            text(payload, "effective_to")
        ],
    )?;
    mapping::set_version(
        conn,
        ASSIGNMENT,
        local_id,
        &text(payload, "updated_at").unwrap_or_default(),
    )?;
    Ok(local_id)
}

pub fn apply_duty_roster(conn: &Connection, payload: &Value) -> AppResult<i64> {
    let remote_id = text(payload, "id").ok_or_else(|| {
        crate::error::AppError::Cloud("Duty roster change is missing an id".to_string())
    })?;
    let employee_remote = text(payload, "employee_id").unwrap_or_default();
    let employee_id = local_employee_id(
        conn,
        &serde_json::json!({ "id": employee_remote, "code": format!("remote-{employee_remote}") }),
    )?;
    let shift_id = match text(payload, "shift_id") {
        Some(remote_shift) => Some(local_shift_id(conn, &remote_shift)?),
        None => None,
    };
    let local_id = match mapping::get_by_remote(conn, DUTY_ROSTER, &remote_id)? {
        Some(id) => id,
        None => {
            conn.execute(
                "INSERT INTO duty_rosters(employee_id, duty_date, shift_id, duty, notes, source, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    employee_id,
                    text_or(payload, "duty_date", "1970-01-01"),
                    shift_id,
                    text_or(payload, "duty", "Duty"),
                    text(payload, "notes"),
                    text_or(payload, "source", "manual"),
                    text(payload, "created_at").unwrap_or_else(now_iso),
                ],
            )?;
            let id = conn.last_insert_rowid();
            mapping::set(conn, DUTY_ROSTER, id, &remote_id, "")?;
            return Ok(id);
        }
    };
    conn.execute(
        "UPDATE duty_rosters SET employee_id = ?2, duty_date = ?3, shift_id = ?4,
             duty = ?5, notes = ?6, source = ?7 WHERE id = ?1",
        params![
            local_id,
            employee_id,
            text_or(payload, "duty_date", "1970-01-01"),
            shift_id,
            text_or(payload, "duty", "Duty"),
            text(payload, "notes"),
            text_or(payload, "source", "manual"),
        ],
    )?;
    mapping::set_version(
        conn,
        DUTY_ROSTER,
        local_id,
        &text(payload, "updated_at").unwrap_or_default(),
    )?;
    Ok(local_id)
}

pub fn apply_event(conn: &Connection, payload: &Value) -> AppResult<i64> {
    let remote_id = text(payload, "id")
        .ok_or_else(|| crate::error::AppError::Cloud("Event change is missing an id".to_string()))?;
    if let Some(local_id) = mapping::get_by_remote(conn, EVENT, &remote_id)? {
        return Ok(local_id);
    }
    let employee_remote = text(payload, "employee_id").unwrap_or_default();
    let employee_id = local_employee_id(
        conn,
        &serde_json::json!({ "id": employee_remote, "code": format!("remote-{employee_remote}") }),
    )?;
    let occurred_at = text(payload, "occurred_at").unwrap_or_else(now_iso);
    let epoch = integer(payload, "occurred_at_epoch")
        .or_else(|| chrono::DateTime::parse_from_rfc3339(&occurred_at).ok().map(|value| value.timestamp()))
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO time_events(employee_id, event_type, occurred_at, occurred_at_epoch, source, note, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            employee_id,
            text_or(payload, "event_type", "clock_in"),
            occurred_at,
            epoch,
            text_or(payload, "source", "scan"),
            text(payload, "note"),
            text(payload, "created_at").unwrap_or_else(now_iso),
        ],
    )?;
    let local_id = conn.last_insert_rowid();
    mapping::set(conn, EVENT, local_id, &remote_id, "")?;
    Ok(local_id)
}

const SYNCED_SETTINGS: [&str; 4] = [
    "company_name",
    "overtime_daily_hours",
    "overtime_weekly_hours",
    "idle_lock_minutes",
];

pub fn apply_setting(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    if !SYNCED_SETTINGS.contains(&key) {
        return Ok(());
    }
    // Settings are keyed by name in the `settings` table already. Using
    // cloud_ids here would collide on (kind, local_id) for every setting.
    conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// Used when a desktop connects to a server without importing its local data.
/// The server becomes the only source of truth from that point on.
pub fn clear_cache(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "DELETE FROM time_events;
         DELETE FROM duty_rosters;
         DELETE FROM shift_assignments;
         DELETE FROM employees;
         DELETE FROM shifts;
         DELETE FROM cloud_ids;
         DELETE FROM outbox;
         DELETE FROM cloud_conflicts;",
    )?;
    for key in [
        "company_name",
        "overtime_daily_hours",
        "overtime_weekly_hours",
        "idle_lock_minutes",
    ] {
        conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
    }
    Ok(())
}

pub fn apply_change(conn: &Connection, entity: &str, operation: &str, payload: &Value) -> AppResult<()> {
    if operation == "delete" {
        let kind = match entity {
            "employee" => EMPLOYEE,
            "shift" => SHIFT,
            "assignment" => ASSIGNMENT,
            "duty_roster" => DUTY_ROSTER,
            "event" => EVENT,
            _ => return Ok(()),
        };
        let remote_id = text(payload, "id").unwrap_or_default();
        if let Some(local_id) = mapping::get_by_remote(conn, kind, &remote_id)? {
            let table = match kind {
                EMPLOYEE => "employees",
                SHIFT => "shifts",
                ASSIGNMENT => "shift_assignments",
                DUTY_ROSTER => "duty_rosters",
                EVENT => "time_events",
                _ => unreachable!(),
            };
            conn.execute(&format!("DELETE FROM {table} WHERE id = ?1"), params![local_id])?;
            mapping::remove(conn, kind, local_id)?;
        }
        return Ok(());
    }

    match entity {
        "employee" => {
            apply_employee(conn, payload)?;
        }
        "shift" => {
            apply_shift(conn, payload)?;
        }
        "assignment" => {
            apply_assignment(conn, payload)?;
        }
        "duty_roster" => {
            apply_duty_roster(conn, payload)?;
        }
        "event" => {
            apply_event(conn, payload)?;
        }
        "setting" => {
            let key = text(payload, "key").unwrap_or_default();
            let value = match payload.get("value") {
                Some(Value::String(raw)) => raw.clone(),
                Some(Value::Number(raw)) => raw.to_string(),
                Some(Value::Bool(raw)) => raw.to_string(),
                _ => String::new(),
            };
            apply_setting(conn, &key, &value)?;
        }
        _ => {}
    }
    Ok(())
}

pub fn apply_bootstrap(conn: &Connection, snapshot: &Value) -> AppResult<i64> {
    let mut applied = 0;
    for (key, entity) in [
        ("employees", "employee"),
        ("shifts", "shift"),
        ("assignments", "assignment"),
        ("duty_rosters", "duty_roster"),
        ("events", "event"),
    ] {
        if let Some(rows) = snapshot.get(key).and_then(Value::as_array) {
            for row in rows {
                apply_change(conn, entity, "upsert", row)?;
                applied += 1;
            }
        }
    }
    if let Some(settings) = snapshot.get("settings").and_then(Value::as_object) {
        for (key, value) in settings {
            let value = match value {
                Value::String(raw) => raw.clone(),
                Value::Number(raw) => raw.to_string(),
                Value::Bool(raw) => raw.to_string(),
                _ => continue,
            };
            apply_setting(conn, key, &value)?;
            applied += 1;
        }
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{lock_conn, settings, Db};
    use std::sync::{Arc, Mutex};

    fn test_db() -> Db {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../db/migrations.sql")).unwrap();
        Db {
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    #[test]
    fn settings_are_not_mapped_as_local_records() {
        let db = test_db();
        let conn = lock_conn(&db).unwrap();
        apply_setting(&conn, "company_name", "Acme").unwrap();
        apply_setting(&conn, "idle_lock_minutes", "30").unwrap();
        assert_eq!(settings::get_value(&conn, "company_name").unwrap(), "Acme");
        assert_eq!(settings::get_value(&conn, "idle_lock_minutes").unwrap(), "30");
        let mapped: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM cloud_ids WHERE kind = 'setting'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(mapped, 0);
    }

    #[test]
    fn bootstrap_reports_how_many_records_were_applied() {
        let db = test_db();
        let conn = lock_conn(&db).unwrap();
        let snapshot = serde_json::json!({
            "employees": [
                { "id": " emp-1 ", "code": "E1", "name": "Jane", "active": true },
                { "id": "emp-2", "code": "E2", "name": "John", "active": true }
            ],
            "settings": { "company_name": "Acme", "idle_lock_minutes": "30" }
        });
        assert_eq!(apply_bootstrap(&conn, &snapshot).unwrap(), 4);
    }
}
