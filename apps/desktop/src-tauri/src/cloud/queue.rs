use rusqlite::Connection;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::cloud::mapping::{self, ASSIGNMENT, DUTY_ROSTER, EMPLOYEE, EVENT, SETTING, SHIFT};
use crate::cloud::outbox;
use crate::db::{employees, events, shifts};
use crate::error::AppResult;

fn unique_key(prefix: &str) -> String {
    format!("{prefix}:{}", Uuid::new_v4())
}

pub fn cancel_pending(conn: &Connection, kind: &str, local_id: i64) -> AppResult<()> {
    conn.execute(
        "DELETE FROM outbox WHERE status = 'pending' AND kind = ?1 AND local_id = ?2",
        rusqlite::params![kind, local_id],
    )?;
    Ok(())
}

fn queue(
    conn: &Connection,
    key: &str,
    kind: &str,
    operation: &str,
    local_id: i64,
    payload: &Value,
) -> AppResult<()> {
    outbox::enqueue(conn, key, kind, operation, local_id, payload)
}

fn version(conn: &Connection, kind: &str, local_id: i64) -> AppResult<String> {
    Ok(mapping::get(conn, kind, local_id)?
        .map(|reference| reference.version)
        .unwrap_or_default())
}

pub fn employee_upsert(conn: &Connection, id: i64) -> AppResult<()> {
    let employee = employees::get(conn, id)?;
    let remote_id = mapping::ensure_uuid(conn, EMPLOYEE, id)?;
    let payload = json!({
        "id": remote_id,
        "code": employee.code,
        "name": employee.name,
        "department": employee.department,
        "role": employee.role,
        "active": employee.active,
        "pin_hash": employee.pin_hash,
        "updated_at": version(conn, EMPLOYEE, id)?,
    });
    queue(
        conn,
        &unique_key("employee-upsert"),
        EMPLOYEE,
        "upsert",
        id,
        &payload,
    )
}

#[allow(dead_code)]
pub fn employee_delete(conn: &Connection, id: i64) -> AppResult<()> {
    cancel_pending(conn, EMPLOYEE, id)?;
    if let Some(remote_id) = mapping::get(conn, EMPLOYEE, id)?.map(|reference| reference.remote_id) {
        queue(
            conn,
            &unique_key("employee-delete"),
            EMPLOYEE,
            "delete",
            id,
            &json!({ "id": remote_id }),
        )?;
    }
    Ok(())
}

pub fn shift_upsert(conn: &Connection, id: i64) -> AppResult<()> {
    let shift = shifts::get_shift(conn, id)?;
    let remote_id = mapping::ensure_uuid(conn, SHIFT, id)?;
    let payload = json!({
        "id": remote_id,
        "name": shift.name,
        "start_time": shift.start_time,
        "end_time": shift.end_time,
        "days": shift.days,
        "break_minutes": shift.break_minutes,
        "active": shift.active,
        "updated_at": version(conn, SHIFT, id)?,
    });
    queue(conn, &unique_key("shift-upsert"), SHIFT, "upsert", id, &payload)
}

pub fn shift_delete(conn: &Connection, id: i64) -> AppResult<()> {
    cancel_pending(conn, SHIFT, id)?;
    if let Some(remote_id) = mapping::get(conn, SHIFT, id)?.map(|reference| reference.remote_id) {
        queue(
            conn,
            &unique_key("shift-delete"),
            SHIFT,
            "delete",
            id,
            &json!({ "id": remote_id }),
        )?;
    }
    Ok(())
}

pub fn assignment_upsert(conn: &Connection, id: i64) -> AppResult<()> {
    let assignment = shifts::list_assignments(conn, None)?
        .into_iter()
        .find(|assignment| assignment.id == id)
        .ok_or_else(|| crate::error::AppError::InvalidInput("Assignment not found".to_string()))?;
    let remote_id = mapping::ensure_uuid(conn, ASSIGNMENT, id)?;
    let employee_id = mapping::ensure_uuid(conn, EMPLOYEE, assignment.employee_id)?;
    let shift_id = mapping::ensure_uuid(conn, SHIFT, assignment.shift_id)?;
    let payload = json!({
        "id": remote_id,
        "employee_id": employee_id,
        "shift_id": shift_id,
        "effective_from": assignment.effective_from,
        "effective_to": assignment.effective_to,
        "updated_at": version(conn, ASSIGNMENT, id)?,
    });
    queue(
        conn,
        &unique_key("assignment-upsert"),
        ASSIGNMENT,
        "upsert",
        id,
        &payload,
    )
}

pub fn assignment_delete(conn: &Connection, id: i64) -> AppResult<()> {
    cancel_pending(conn, ASSIGNMENT, id)?;
    if let Some(remote_id) = mapping::get(conn, ASSIGNMENT, id)?.map(|reference| reference.remote_id) {
        queue(
            conn,
            &unique_key("assignment-delete"),
            ASSIGNMENT,
            "delete",
            id,
            &json!({ "id": remote_id }),
        )?;
    }
    Ok(())
}

pub fn duty_roster_upsert(conn: &Connection, id: i64) -> AppResult<()> {
    let record = crate::db::duty_roster::get(conn, id)?;
    let remote_id = mapping::ensure_uuid(conn, DUTY_ROSTER, id)?;
    let employee_id = mapping::ensure_uuid(conn, EMPLOYEE, record.employee_id)?;
    let shift_id = match record.shift_id {
        Some(shift_id) => Some(mapping::ensure_uuid(conn, SHIFT, shift_id)?),
        None => None,
    };
    let payload = json!({
        "id": remote_id,
        "employee_id": employee_id,
        "duty_date": record.duty_date,
        "shift_id": shift_id,
        "duty": record.duty,
        "notes": record.notes,
        "source": record.source,
        "created_at": record.created_at,
        "updated_at": version(conn, DUTY_ROSTER, id)?,
    });
    queue(
        conn,
        &unique_key("duty-roster-upsert"),
        DUTY_ROSTER,
        "upsert",
        id,
        &payload,
    )
}

pub fn duty_roster_delete(conn: &Connection, id: i64) -> AppResult<()> {
    cancel_pending(conn, DUTY_ROSTER, id)?;
    if let Some(remote_id) = mapping::get(conn, DUTY_ROSTER, id)?.map(|reference| reference.remote_id) {
        queue(
            conn,
            &unique_key("duty-roster-delete"),
            DUTY_ROSTER,
            "delete",
            id,
            &json!({ "id": remote_id }),
        )?;
    }
    Ok(())
}

pub fn event(conn: &Connection, id: i64) -> AppResult<()> {
    let event = events::get(conn, id)?;
    let remote_id = mapping::ensure_uuid(conn, EVENT, id)?;
    let employee_id = mapping::ensure_uuid(conn, EMPLOYEE, event.employee_id)?;
    let payload = json!({
        "id": remote_id,
        "employee_id": employee_id,
        "event_type": event.event_type.as_str(),
        "occurred_at": event.occurred_at,
        "occurred_at_epoch": event.occurred_at_epoch,
        "source": event.source,
        "note": event.note,
    });
    queue(conn, &remote_id, EVENT, "create", id, &payload)
}

pub fn event_delete(conn: &Connection, id: i64) -> AppResult<()> {
    cancel_pending(conn, EVENT, id)?;
    if let Some(remote_id) = mapping::get(conn, EVENT, id)?.map(|reference| reference.remote_id) {
        queue(
            conn,
            &unique_key("event-delete"),
            EVENT,
            "delete",
            id,
            &json!({ "id": remote_id }),
        )?;
    }
    Ok(())
}

pub fn setting(conn: &Connection, key: &str) -> AppResult<()> {
    if !["company_name", "overtime_daily_hours", "overtime_weekly_hours", "idle_lock_minutes"].contains(&key) {
        return Ok(());
    }
    let value = crate::db::settings::get_value(conn, key)?;
    // The server does not expose per-setting version timestamps in changes or
    // bootstrap responses, so settings use last-write-wins instead of a
    // stale-version header.
    queue(
        conn,
        &unique_key(&format!("setting-{key}")),
        SETTING,
        "upsert",
        0,
        &json!({ "key": key, "value": value }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{lock_conn, Db};
    use std::sync::{Arc, Mutex};

    fn test_db() -> Db {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../db/migrations.sql")).unwrap();
        Db {
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    #[test]
    fn offline_clock_event_queues_one_durable_item() {
        let db = test_db();
        let conn = lock_conn(&db).unwrap();
        let employee = employees::create(
            &conn,
            &crate::models::CreateEmployee {
                code: "E1".to_string(),
                name: "Jane".to_string(),
                department: None,
                role: None,
                pin: None,
            },
        )
        .unwrap();
        let recorded = events::insert(
            &conn,
            employee.id,
            crate::models::EventType::ClockIn,
            "2026-11-25T09:00:00Z",
            1764061200,
            "mobile",
            None,
        )
        .unwrap();
        event(&conn, recorded.id).unwrap();
        event(&conn, recorded.id).unwrap();
        let pending = outbox::list_pending(&conn).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].kind, EVENT);
        let payload: Value = serde_json::from_str(&pending[0].payload).unwrap();
        assert_eq!(payload["id"].as_str().unwrap().len(), 36);
        assert_eq!(payload["event_type"], "clock_in");
        assert!(payload["employee_id"].as_str().unwrap().len() > 0);
    }

    #[test]
    fn deleting_a_record_cancels_its_pending_upsert() {
        let db = test_db();
        let conn = lock_conn(&db).unwrap();
        let employee = employees::create(
            &conn,
            &crate::models::CreateEmployee {
                code: "E2".to_string(),
                name: "John".to_string(),
                department: None,
                role: None,
                pin: None,
            },
        )
        .unwrap();
        employee_upsert(&conn, employee.id).unwrap();
        assert_eq!(outbox::list_pending(&conn).unwrap().len(), 1);
        employee_delete(&conn, employee.id).unwrap();
        assert_eq!(outbox::list_pending(&conn).unwrap().len(), 1);
        let pending = outbox::list_pending(&conn).unwrap();
        assert_eq!(pending[0].operation, "delete");
    }
}
