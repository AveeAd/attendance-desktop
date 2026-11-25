use rusqlite::{params, Connection};

use crate::db::{days_to_mask, mask_to_days};
use crate::error::{AppError, AppResult};
use crate::models::{AssignmentInput, Shift, ShiftAssignment, ShiftInput};

fn row_to_shift(row: &rusqlite::Row<'_>) -> rusqlite::Result<Shift> {
    let mask: i64 = row.get("days_mask")?;
    Ok(Shift {
        id: row.get("id")?,
        name: row.get("name")?,
        start_time: row.get("start_time")?,
        end_time: row.get("end_time")?,
        days: mask_to_days(mask),
        break_minutes: row.get("break_minutes")?,
        active: row.get::<_, i64>("active")? == 1,
    })
}

fn row_to_assignment(row: &rusqlite::Row<'_>) -> rusqlite::Result<ShiftAssignment> {
    Ok(ShiftAssignment {
        id: row.get("id")?,
        employee_id: row.get("employee_id")?,
        employee_name: row.get("employee_name")?,
        shift_id: row.get("shift_id")?,
        shift_name: row.get("shift_name")?,
        effective_from: row.get("effective_from")?,
        effective_to: row.get("effective_to")?,
    })
}

pub fn list_shifts(conn: &Connection, active_only: bool) -> AppResult<Vec<Shift>> {
    let sql = if active_only {
        "SELECT id, name, start_time, end_time, days_mask, break_minutes, active FROM shifts WHERE active = 1 ORDER BY start_time, name"
    } else {
        "SELECT id, name, start_time, end_time, days_mask, break_minutes, active FROM shifts ORDER BY active DESC, start_time, name"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map([], row_to_shift)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get_shift(conn: &Connection, id: i64) -> AppResult<Shift> {
    conn.query_row(
        "SELECT id, name, start_time, end_time, days_mask, break_minutes, active FROM shifts WHERE id = ?1",
        params![id],
        row_to_shift,
    )
    .map_err(|_| AppError::ShiftNotFound(id))
}

pub fn create_shift(conn: &Connection, input: &ShiftInput) -> AppResult<Shift> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::InvalidInput("Shift name is required".to_string()));
    }
    crate::db::parse_time(&input.start_time)?;
    crate::db::parse_time(&input.end_time)?;
    conn.execute(
        "INSERT INTO shifts(name, start_time, end_time, days_mask, break_minutes, active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            name,
            input.start_time,
            input.end_time,
            days_to_mask(&input.days),
            input.break_minutes.max(0),
            input.active as i64
        ],
    )?;
    get_shift(conn, conn.last_insert_rowid())
}

pub fn update_shift(conn: &Connection, id: i64, input: &ShiftInput) -> AppResult<Shift> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::InvalidInput("Shift name is required".to_string()));
    }
    crate::db::parse_time(&input.start_time)?;
    crate::db::parse_time(&input.end_time)?;
    let changed = conn.execute(
        "UPDATE shifts SET name = ?2, start_time = ?3, end_time = ?4, days_mask = ?5, break_minutes = ?6, active = ?7 WHERE id = ?1",
        params![
            id,
            name,
            input.start_time,
            input.end_time,
            days_to_mask(&input.days),
            input.break_minutes.max(0),
            input.active as i64
        ],
    )?;
    if changed == 0 {
        return Err(AppError::ShiftNotFound(id));
    }
    get_shift(conn, id)
}

pub fn delete_shift(conn: &Connection, id: i64) -> AppResult<()> {
    let changed = conn.execute("DELETE FROM shifts WHERE id = ?1", params![id])?;
    if changed == 0 {
        return Err(AppError::ShiftNotFound(id));
    }
    Ok(())
}

pub fn list_assignments(conn: &Connection, employee_id: Option<i64>) -> AppResult<Vec<ShiftAssignment>> {
    let mut sql = String::from(
        "SELECT a.id, a.employee_id, emp.name AS employee_name, a.shift_id, s.name AS shift_name,
                a.effective_from, a.effective_to
         FROM shift_assignments a
         JOIN employees emp ON emp.id = a.employee_id
         JOIN shifts s ON s.id = a.shift_id WHERE 1 = 1",
    );
    if employee_id.is_some() {
        sql.push_str(" AND a.employee_id = ?1");
    }
    sql.push_str(" ORDER BY a.effective_from DESC, emp.name COLLATE NOCASE");
    let mut stmt = conn.prepare(&sql)?;
    let rows = match employee_id {
        Some(id) => stmt.query_map(params![id], row_to_assignment)?
            .collect::<rusqlite::Result<Vec<_>>>()?,
        None => stmt.query_map([], row_to_assignment)?
            .collect::<rusqlite::Result<Vec<_>>>()?,
    };
    Ok(rows)
}

pub fn create_assignment(conn: &Connection, input: &AssignmentInput) -> AppResult<ShiftAssignment> {
    if input.effective_to.as_deref().is_some_and(|d| d < input.effective_from.as_str()) {
        return Err(AppError::InvalidInput(
            "Effective end date must be after the start date".to_string(),
        ));
    }
    conn.execute(
        "INSERT INTO shift_assignments(employee_id, shift_id, effective_from, effective_to)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            input.employee_id,
            input.shift_id,
            input.effective_from,
            input.effective_to
        ],
    )?;
    let id = conn.last_insert_rowid();
    conn.query_row(
        "SELECT a.id, a.employee_id, emp.name AS employee_name, a.shift_id, s.name AS shift_name,
                a.effective_from, a.effective_to
         FROM shift_assignments a
         JOIN employees emp ON emp.id = a.employee_id
         JOIN shifts s ON s.id = a.shift_id WHERE a.id = ?1",
        params![id],
        row_to_assignment,
    )
    .map_err(Into::into)
}

pub fn delete_assignment(conn: &Connection, id: i64) -> AppResult<()> {
    let changed = conn.execute("DELETE FROM shift_assignments WHERE id = ?1", params![id])?;
    if changed == 0 {
        return Err(AppError::InvalidInput("Assignment not found".to_string()));
    }
    Ok(())
}
