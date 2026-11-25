use rusqlite::{params, Connection};

use crate::db::now_iso;
use crate::error::{AppError, AppResult};
use crate::models::{EventType, TimeEvent};

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<TimeEvent> {
    let type_str: String = row.get("event_type")?;
    Ok(TimeEvent {
        id: row.get("id")?,
        employee_id: row.get("employee_id")?,
        employee_name: row.get("employee_name")?,
        event_type: EventType::parse(&type_str).unwrap_or(EventType::ClockIn),
        occurred_at: row.get("occurred_at")?,
        occurred_at_epoch: row.get("occurred_at_epoch")?,
        source: row.get("source")?,
        note: row.get("note")?,
        created_at: row.get("created_at")?,
    })
}

const SELECT: &str = "SELECT e.id, e.employee_id, emp.name AS employee_name, e.event_type,
                             e.occurred_at, e.occurred_at_epoch, e.source, e.note, e.created_at
                      FROM time_events e JOIN employees emp ON emp.id = e.employee_id";

#[allow(clippy::too_many_arguments)]
pub fn insert(
    conn: &Connection,
    employee_id: i64,
    event_type: EventType,
    occurred_at: &str,
    epoch: i64,
    source: &str,
    note: Option<&str>,
) -> AppResult<TimeEvent> {
    conn.execute(
        "INSERT INTO time_events(employee_id, event_type, occurred_at, occurred_at_epoch, source, note, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            employee_id,
            event_type.as_str(),
            occurred_at,
            epoch,
            source,
            note,
            now_iso()
        ],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn get(conn: &Connection, id: i64) -> AppResult<TimeEvent> {
    conn.query_row(&format!("{SELECT} WHERE e.id = ?1"), params![id], row_to_event)
        .map_err(|_| AppError::EventNotFound(id))
}

pub fn list(
    conn: &Connection,
    employee_id: Option<i64>,
    from_epoch: Option<i64>,
    to_epoch: Option<i64>,
    limit: i64,
) -> AppResult<Vec<TimeEvent>> {
    let mut sql = format!("{SELECT} WHERE 1 = 1");
    let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(id) = employee_id {
        values.push(Box::new(id));
        sql.push_str(&format!(" AND e.employee_id = ?{}", values.len()));
    }
    if let Some(from) = from_epoch {
        values.push(Box::new(from));
        sql.push_str(&format!(" AND e.occurred_at_epoch >= ?{}", values.len()));
    }
    if let Some(to) = to_epoch {
        values.push(Box::new(to));
        sql.push_str(&format!(" AND e.occurred_at_epoch < ?{}", values.len()));
    }
    sql.push_str(" ORDER BY e.occurred_at_epoch DESC, e.id DESC");
    values.push(Box::new(limit));
    sql.push_str(&format!(" LIMIT ?{}", values.len()));

    let params_refs: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params_refs.as_slice(), row_to_event)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    let changed = conn.execute("DELETE FROM time_events WHERE id = ?1", params![id])?;
    if changed == 0 {
        return Err(AppError::EventNotFound(id));
    }
    Ok(())
}
