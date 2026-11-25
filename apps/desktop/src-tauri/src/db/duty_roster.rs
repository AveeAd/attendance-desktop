use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};

use crate::db::{now_iso, shifts};
use crate::error::{AppError, AppResult};
use crate::models::{DutyRosterGenerateResult, DutyRosterInput, DutyRosterRecord};

const RECORD_COLUMNS: &str = "r.id, r.employee_id, emp.name AS employee_name, r.duty_date,
     r.shift_id, s.name AS shift_name, r.duty, r.notes, r.source, r.created_at";

const RECORD_JOINS: &str = "FROM duty_rosters r
     JOIN employees emp ON emp.id = r.employee_id
     LEFT JOIN shifts s ON s.id = r.shift_id";

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<DutyRosterRecord> {
    Ok(DutyRosterRecord {
        id: row.get("id")?,
        employee_id: row.get("employee_id")?,
        employee_name: row.get("employee_name")?,
        duty_date: row.get("duty_date")?,
        shift_id: row.get("shift_id")?,
        shift_name: row.get("shift_name")?,
        duty: row.get("duty")?,
        notes: row.get("notes")?,
        source: row.get("source")?,
        created_at: row.get("created_at")?,
    })
}

fn clean_notes(notes: Option<&str>) -> Option<String> {
    notes
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn validate_date(value: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidInput("Invalid duty date".to_string()))
}

fn validate_input(conn: &Connection, input: &DutyRosterInput) -> AppResult<String> {
    validate_date(&input.duty_date)?;
    let duty = input.duty.trim();
    if duty.is_empty() {
        return Err(AppError::InvalidInput("Duty name is required".to_string()));
    }
    crate::db::employees::get(conn, input.employee_id)?;
    if let Some(shift_id) = input.shift_id {
        let shift = shifts::get_shift(conn, shift_id)?;
        if !shift.active {
            return Err(AppError::InvalidInput("Shift is inactive".to_string()));
        }
    }
    Ok(duty.to_string())
}

pub fn list_for_range(conn: &Connection, from: &str, to: &str) -> AppResult<Vec<DutyRosterRecord>> {
    validate_date(from)?;
    validate_date(to)?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {RECORD_COLUMNS} {RECORD_JOINS}
         WHERE r.duty_date >= ?1 AND r.duty_date <= ?2
         ORDER BY r.duty_date, emp.name COLLATE NOCASE, r.id"
    ))?;
    let rows = stmt
        .query_map(params![from, to], row_to_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<DutyRosterRecord> {
    conn.query_row(
        &format!("SELECT {RECORD_COLUMNS} {RECORD_JOINS} WHERE r.id = ?1"),
        params![id],
        row_to_record,
    )
    .map_err(|_| AppError::DutyRosterNotFound(id))
}

pub fn create(conn: &Connection, input: &DutyRosterInput) -> AppResult<DutyRosterRecord> {
    let duty = validate_input(conn, input)?;
    conn.execute(
        "INSERT INTO duty_rosters(employee_id, duty_date, shift_id, duty, notes, source, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'manual', ?6)",
        params![
            input.employee_id,
            input.duty_date,
            input.shift_id,
            duty,
            clean_notes(input.notes.as_deref()),
            now_iso(),
        ],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, input: &DutyRosterInput) -> AppResult<DutyRosterRecord> {
    let duty = validate_input(conn, input)?;
    let changed = conn.execute(
        "UPDATE duty_rosters SET employee_id = ?2, duty_date = ?3, shift_id = ?4,
             duty = ?5, notes = ?6 WHERE id = ?1",
        params![
            id,
            input.employee_id,
            input.duty_date,
            input.shift_id,
            duty,
            clean_notes(input.notes.as_deref()),
        ],
    )?;
    if changed == 0 {
        return Err(AppError::DutyRosterNotFound(id));
    }
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    let changed = conn.execute("DELETE FROM duty_rosters WHERE id = ?1", params![id])?;
    if changed == 0 {
        return Err(AppError::DutyRosterNotFound(id));
    }
    Ok(())
}

fn has_duty(conn: &Connection, employee_id: i64, date: &str, shift_id: i64) -> AppResult<bool> {
    conn.query_row(
        "SELECT 1 FROM duty_rosters
         WHERE employee_id = ?1 AND duty_date = ?2 AND shift_id = ?3 LIMIT 1",
        params![employee_id, date, shift_id],
        |_| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(Into::into)
}

pub fn generate_from_schedule(
    conn: &mut Connection,
    week_start: &str,
) -> AppResult<DutyRosterGenerateResult> {
    let start = validate_date(week_start)?;
    let tx = conn.transaction()?;
    let assignments = shifts::list_assignments(&tx, None)?;
    let shifts = shifts::list_shifts(&tx, true)?;
    let mut created = 0;
    let mut skipped = 0;

    for offset in 0..7 {
        let date = start + Duration::days(offset);
        let date_str = date.format("%Y-%m-%d").to_string();
        let weekday = date.weekday().num_days_from_monday() as i64 + 1;

        for assignment in assignments.iter().filter(|assignment| {
            assignment.effective_from <= date_str
                && assignment
                    .effective_to
                    .as_deref()
                    .is_none_or(|end| date_str.as_str() <= end)
        }) {
            let Some(shift) = shifts
                .iter()
                .find(|shift| shift.id == assignment.shift_id && shift.days.contains(&weekday))
            else {
                continue;
            };

            if has_duty(&tx, assignment.employee_id, &date_str, shift.id)? {
                skipped += 1;
                continue;
            }

            tx.execute(
                "INSERT INTO duty_rosters(employee_id, duty_date, shift_id, duty, notes, source, created_at)
                 VALUES (?1, ?2, ?3, ?4, NULL, 'generated', ?5)",
                params![
                    assignment.employee_id,
                    date_str,
                    shift.id,
                    shift.name,
                    now_iso(),
                ],
            )?;
            created += 1;
        }
    }

    tx.commit()?;
    Ok(DutyRosterGenerateResult { created, skipped })
}
