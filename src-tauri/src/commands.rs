use std::path::PathBuf;

use argon2::password_hash::{SaltString, PasswordHash};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use chrono::{Duration, Local, NaiveDate};
use rand_core::OsRng;
use tauri::command;

use crate::db::{lock_conn, parse_timestamp, timestamp_or_now, Db};
use crate::error::{AppError, AppResult};
use crate::models::{
    AppSettings, AssignmentInput, ClockInput, ClockResult, CreateEmployee, DutyRosterEntry,
    DutyRosterInput, Employee, EventType, ScheduleEntry, SettingsInput, Shift, ShiftAssignment,
    ShiftInput, TimeEvent, TimesheetReport, UpdateEmployee,
};
use crate::reporting::{
    build_duty_roster, build_schedule, build_statuses, build_timesheet, compute_periods,
};

#[command]
pub fn list_employees(db: tauri::State<'_, Db>, active_only: Option<bool>) -> AppResult<Vec<Employee>> {
    let conn = lock_conn(&db)?;
    crate::db::employees::list(&conn, active_only.unwrap_or(false))
}

#[command]
pub fn get_employee(db: tauri::State<'_, Db>, id: i64) -> AppResult<Employee> {
    let conn = lock_conn(&db)?;
    crate::db::employees::get(&conn, id)
}

#[command]
pub fn create_employee(db: tauri::State<'_, Db>, input: CreateEmployee) -> AppResult<Employee> {
    let conn = lock_conn(&db)?;
    crate::db::employees::create(&conn, &input)
}

#[command]
pub fn update_employee(
    db: tauri::State<'_, Db>,
    id: i64,
    input: UpdateEmployee,
) -> AppResult<Employee> {
    let conn = lock_conn(&db)?;
    crate::db::employees::update(&conn, id, &input)
}

#[command]
pub fn set_employee_active(db: tauri::State<'_, Db>, id: i64, active: bool) -> AppResult<()> {
    let conn = lock_conn(&db)?;
    crate::db::employees::set_active(&conn, id, active)
}

#[command]
pub fn clock_event(db: tauri::State<'_, Db>, input: ClockInput) -> AppResult<ClockResult> {
    let conn = lock_conn(&db)?;
    let employee = crate::db::employees::get_by_identifier(&conn, &input.identifier)?;
    let (occurred_at, epoch) = timestamp_or_now(input.occurred_at.as_deref())?;
    let source = input
        .source
        .unwrap_or_else(|| "scan".to_string())
        .trim()
        .to_lowercase();
    let event = crate::db::events::insert(
        &conn,
        employee.id,
        input.event_type,
        &occurred_at,
        epoch,
        &source,
        input.note.as_deref(),
    )?;

    let events = crate::db::events::list(&conn, Some(employee.id), None, None, 500)?;
    let status = build_statuses(&[employee.clone()], &events)
        .into_iter()
        .next()
        .unwrap();
    let message = match input.event_type {
        EventType::ClockIn => "Clocked in".to_string(),
        EventType::ClockOut => "Clocked out".to_string(),
        EventType::BreakStart => "Break started".to_string(),
        EventType::BreakEnd => "Break ended".to_string(),
    };

    Ok(ClockResult {
        event,
        employee,
        state: status.state,
        message,
    })
}

#[command]
pub fn list_events(
    db: tauri::State<'_, Db>,
    employee_id: Option<i64>,
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
) -> AppResult<Vec<TimeEvent>> {
    let conn = lock_conn(&db)?;
    let from_epoch = from.as_deref().map(|v| parse_timestamp(v)).transpose()?.map(|d| d.timestamp());
    let to_epoch = to.as_deref().map(|v| parse_timestamp(v)).transpose()?.map(|d| d.timestamp());
    crate::db::events::list(&conn, employee_id, from_epoch, to_epoch, limit.unwrap_or(200))
}

#[command]
pub fn create_manual_event(
    db: tauri::State<'_, Db>,
    employee_id: i64,
    event_type: EventType,
    occurred_at: String,
    note: Option<String>,
) -> AppResult<TimeEvent> {
    let conn = lock_conn(&db)?;
    let _employee = crate::db::employees::get(&conn, employee_id)?;
    let (occurred_at, epoch) = timestamp_or_now(Some(&occurred_at))?;
    crate::db::events::insert(
        &conn,
        employee_id,
        event_type,
        &occurred_at,
        epoch,
        "manual",
        note.as_deref(),
    )
}

#[command]
pub fn delete_event(db: tauri::State<'_, Db>, id: i64) -> AppResult<()> {
    let conn = lock_conn(&db)?;
    crate::db::events::delete(&conn, id)
}

#[command]
pub fn get_employee_statuses(db: tauri::State<'_, Db>) -> AppResult<Vec<crate::models::EmployeeStatus>> {
    let conn = lock_conn(&db)?;
    let employees = crate::db::employees::list(&conn, true)?;
    let today = Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .earliest()
        .unwrap()
        .timestamp();
    let events = crate::db::events::list(&conn, None, Some(today), None, 5000)?;
    Ok(build_statuses(&employees, &events))
}

#[command]
pub fn list_shifts(db: tauri::State<'_, Db>, active_only: Option<bool>) -> AppResult<Vec<Shift>> {
    let conn = lock_conn(&db)?;
    crate::db::shifts::list_shifts(&conn, active_only.unwrap_or(false))
}

#[command]
pub fn create_shift(db: tauri::State<'_, Db>, input: ShiftInput) -> AppResult<Shift> {
    let conn = lock_conn(&db)?;
    crate::db::shifts::create_shift(&conn, &input)
}

#[command]
pub fn update_shift(db: tauri::State<'_, Db>, id: i64, input: ShiftInput) -> AppResult<Shift> {
    let conn = lock_conn(&db)?;
    crate::db::shifts::update_shift(&conn, id, &input)
}

#[command]
pub fn delete_shift(db: tauri::State<'_, Db>, id: i64) -> AppResult<()> {
    let conn = lock_conn(&db)?;
    crate::db::shifts::delete_shift(&conn, id)
}

#[command]
pub fn list_assignments(
    db: tauri::State<'_, Db>,
    employee_id: Option<i64>,
) -> AppResult<Vec<ShiftAssignment>> {
    let conn = lock_conn(&db)?;
    crate::db::shifts::list_assignments(&conn, employee_id)
}

#[command]
pub fn create_assignment(
    db: tauri::State<'_, Db>,
    input: AssignmentInput,
) -> AppResult<ShiftAssignment> {
    let conn = lock_conn(&db)?;
    crate::db::shifts::create_assignment(&conn, &input)
}

#[command]
pub fn delete_assignment(db: tauri::State<'_, Db>, id: i64) -> AppResult<()> {
    let conn = lock_conn(&db)?;
    crate::db::shifts::delete_assignment(&conn, id)
}

#[command]
pub fn get_schedule(db: tauri::State<'_, Db>, week_start: String) -> AppResult<Vec<ScheduleEntry>> {
    let conn = lock_conn(&db)?;
    let assignments = crate::db::shifts::list_assignments(&conn, None)?;
    let shifts = crate::db::shifts::list_shifts(&conn, true)?;
    let from = NaiveDate::parse_from_str(&week_start, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidInput("Invalid week start date".to_string()))?;
    let from_epoch = from
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .earliest()
        .unwrap()
        .timestamp();
    let to_epoch = (from + chrono::Duration::days(7))
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .earliest()
        .unwrap()
        .timestamp();
    let events = crate::db::events::list(&conn, None, Some(from_epoch), Some(to_epoch), 5000)?;
    let periods = compute_periods(&events);
    build_schedule(&assignments, &shifts, &periods, &events, &week_start)
}

#[command]
pub fn list_duty_roster(db: tauri::State<'_, Db>, week_start: String) -> AppResult<Vec<DutyRosterEntry>> {
    let conn = lock_conn(&db)?;
    let start = NaiveDate::parse_from_str(&week_start, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidInput("Invalid week start date".to_string()))?;
    let end = (start + Duration::days(6)).format("%Y-%m-%d").to_string();
    duty_roster_for_range(&conn, &week_start, &end)
}

#[command]
pub fn create_duty_roster(db: tauri::State<'_, Db>, input: DutyRosterInput) -> AppResult<DutyRosterEntry> {
    let conn = lock_conn(&db)?;
    let record = crate::db::duty_roster::create(&conn, &input)?;
    Ok(build_duty_roster(&[record], &shifts_for_roster(&conn)?, &[], &today_statuses(&conn)?)
        .into_iter()
        .next()
        .unwrap())
}

#[command]
pub fn update_duty_roster(
    db: tauri::State<'_, Db>,
    id: i64,
    input: DutyRosterInput,
) -> AppResult<DutyRosterEntry> {
    let conn = lock_conn(&db)?;
    let record = crate::db::duty_roster::update(&conn, id, &input)?;
    Ok(build_duty_roster(&[record], &shifts_for_roster(&conn)?, &[], &today_statuses(&conn)?)
        .into_iter()
        .next()
        .unwrap())
}

#[command]
pub fn delete_duty_roster(db: tauri::State<'_, Db>, id: i64) -> AppResult<()> {
    let conn = lock_conn(&db)?;
    crate::db::duty_roster::delete(&conn, id)
}

#[command]
pub fn generate_duty_roster(
    db: tauri::State<'_, Db>,
    week_start: String,
) -> AppResult<crate::models::DutyRosterGenerateResult> {
    let mut conn = lock_conn(&db)?;
    crate::db::duty_roster::generate_from_schedule(&mut conn, &week_start)
}

fn shifts_for_roster(conn: &rusqlite::Connection) -> AppResult<Vec<Shift>> {
    crate::db::shifts::list_shifts(conn, false)
}

fn today_statuses(conn: &rusqlite::Connection) -> AppResult<Vec<crate::models::EmployeeStatus>> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let (from_epoch, _) = date_range(&today, &today)?;
    let events = crate::db::events::list(conn, None, Some(from_epoch), None, 5000)?;
    let employees = crate::db::employees::list(conn, true)?;
    Ok(build_statuses(&employees, &events))
}

fn duty_roster_for_range(
    conn: &rusqlite::Connection,
    from: &str,
    to: &str,
) -> AppResult<Vec<DutyRosterEntry>> {
    let (from_epoch, to_epoch) = date_range(from, to)?;
    let records = crate::db::duty_roster::list_for_range(conn, from, to)?;
    let shifts = shifts_for_roster(conn)?;
    let events = crate::db::events::list(conn, None, Some(from_epoch), Some(to_epoch), 20000)?;
    let periods = compute_periods(&events);
    Ok(build_duty_roster(&records, &shifts, &periods, &today_statuses(conn)?))
}

fn csv_time(value: Option<&str>) -> String {
    value
        .and_then(|value| parse_timestamp(value).ok())
        .map(|value| value.format("%H:%M").to_string())
        .unwrap_or_default()
}

#[derive(serde::Serialize)]
struct DutyRosterCsvRow {
    #[serde(rename = "Date")]
    duty_date: String,
    #[serde(rename = "Employee")]
    employee_name: String,
    #[serde(rename = "Duty")]
    duty: String,
    #[serde(rename = "Shift")]
    shift_name: String,
    #[serde(rename = "Start")]
    start: String,
    #[serde(rename = "End")]
    end: String,
    #[serde(rename = "Status")]
    attendance_status: String,
    #[serde(rename = "Clock in")]
    clock_in: String,
    #[serde(rename = "Clock out")]
    clock_out: String,
    #[serde(rename = "Worked (min)")]
    worked_minutes: i64,
    #[serde(rename = "Late (min)")]
    late_minutes: i64,
    #[serde(rename = "Early (min)")]
    early_minutes: i64,
    #[serde(rename = "Notes")]
    notes: String,
}

#[command]
pub fn export_duty_roster_csv(
    db: tauri::State<'_, Db>,
    path: String,
    from: String,
    to: String,
) -> AppResult<i64> {
    let conn = lock_conn(&db)?;
    let entries = duty_roster_for_range(&conn, &from, &to)?;
    let mut writer = csv::Writer::from_path(PathBuf::from(&path))?;
    for entry in &entries {
        writer.serialize(DutyRosterCsvRow {
            duty_date: entry.duty_date.clone(),
            employee_name: entry.employee_name.clone(),
            duty: entry.duty.clone(),
            shift_name: entry.shift_name.clone().unwrap_or_default(),
            start: csv_time(entry.planned_start.as_deref()),
            end: csv_time(entry.planned_end.as_deref()),
            attendance_status: entry.attendance_status.clone(),
            clock_in: csv_time(entry.actual_clock_in.as_deref()),
            clock_out: csv_time(entry.actual_clock_out.as_deref()),
            worked_minutes: entry.worked_minutes,
            late_minutes: entry.late_minutes,
            early_minutes: entry.early_minutes,
            notes: entry.notes.clone().unwrap_or_default(),
        })?;
    }
    writer.flush()?;
    Ok(entries.len() as i64)
}

fn date_range(from: &str, to: &str) -> AppResult<(i64, i64)> {
    let start = NaiveDate::parse_from_str(from, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidInput("Invalid start date".to_string()))?;
    let end = NaiveDate::parse_from_str(to, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidInput("Invalid end date".to_string()))?;
    if end < start {
        return Err(AppError::InvalidInput("End date must be after start date".to_string()));
    }
    Ok((
        start
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .earliest()
            .unwrap()
            .timestamp(),
        (end + chrono::Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .earliest()
            .unwrap()
            .timestamp(),
    ))
}

fn report_for(
    conn: &rusqlite::Connection,
    from: &str,
    to: &str,
    employee_id: Option<i64>,
) -> AppResult<TimesheetReport> {
    let (from_epoch, to_epoch) = date_range(from, to)?;
    let employees = crate::db::employees::list(conn, false)?;
    let employees: Vec<_> = employees
        .into_iter()
        .filter(|employee| employee_id.is_none_or(|id| employee.id == id))
        .collect();
    let events = crate::db::events::list(conn, employee_id, Some(from_epoch), Some(to_epoch), 20000)?;
    let periods = compute_periods(&events);
    let settings = crate::db::settings::get_all(conn)?;
    Ok(build_timesheet(&employees, &periods, &settings, from_epoch, to_epoch))
}

#[command]
pub fn get_timesheet(
    db: tauri::State<'_, Db>,
    from: String,
    to: String,
    employee_id: Option<i64>,
) -> AppResult<TimesheetReport> {
    let conn = lock_conn(&db)?;
    report_for(&conn, &from, &to, employee_id)
}

#[command]
pub fn export_csv(
    db: tauri::State<'_, Db>,
    path: String,
    from: String,
    to: String,
    employee_id: Option<i64>,
) -> AppResult<i64> {
    let conn = lock_conn(&db)?;
    let report = report_for(&conn, &from, &to, employee_id)?;
    let mut writer = csv::Writer::from_path(PathBuf::from(&path))?;
    for row in &report.rows {
        writer.serialize(row)?;
    }
    writer.flush()?;
    Ok(report.rows.len() as i64)
}

#[command]
pub fn export_xlsx(
    db: tauri::State<'_, Db>,
    path: String,
    from: String,
    to: String,
    employee_id: Option<i64>,
) -> AppResult<i64> {
    let conn = lock_conn(&db)?;
    let report = report_for(&conn, &from, &to, employee_id)?;
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("Timesheet")?;
    let headers = [
        "Employee ID", "Employee", "Date", "Clock In", "Clock Out", "Break (min)",
        "Worked (min)", "Overtime (min)", "Status",
    ];
    for (col, header) in headers.iter().enumerate() {
        sheet.write_string(0, col as u16, *header)?;
    }
    for (index, row) in report.rows.iter().enumerate() {
        let r = (index + 1) as u32;
        sheet.write_number(r, 0, row.employee_id as f64)?;
        sheet.write_string(r, 1, &row.employee_name)?;
        sheet.write_string(r, 2, &row.date)?;
        sheet.write_string(r, 3, row.clock_in.as_deref().unwrap_or(""))?;
        sheet.write_string(r, 4, row.clock_out.as_deref().unwrap_or(""))?;
        sheet.write_number(r, 5, row.break_minutes as f64)?;
        sheet.write_number(r, 6, row.worked_minutes as f64)?;
        sheet.write_number(r, 7, row.overtime_minutes as f64)?;
        sheet.write_string(r, 8, &row.status)?;
    }
    workbook.save(PathBuf::from(&path))?;
    Ok(report.rows.len() as i64)
}

#[command]
pub fn get_settings(db: tauri::State<'_, Db>) -> AppResult<AppSettings> {
    let conn = lock_conn(&db)?;
    crate::db::settings::get_all(&conn)
}

#[command]
pub fn update_settings(db: tauri::State<'_, Db>, input: SettingsInput) -> AppResult<AppSettings> {
    if input.overtime_daily_hours < 0.0 || input.overtime_weekly_hours < 0.0 || input.idle_lock_minutes < 0 {
        return Err(AppError::InvalidInput("Values cannot be negative".to_string()));
    }
    let conn = lock_conn(&db)?;
    crate::db::settings::set_value(&conn, "company_name", input.company_name.trim())?;
    crate::db::settings::set_value(&conn, "overtime_daily_hours", &input.overtime_daily_hours.to_string())?;
    crate::db::settings::set_value(&conn, "overtime_weekly_hours", &input.overtime_weekly_hours.to_string())?;
    crate::db::settings::set_value(&conn, "idle_lock_minutes", &input.idle_lock_minutes.to_string())?;
    crate::db::settings::get_all(&conn)
}

#[command]
pub fn set_admin_pin(
    db: tauri::State<'_, Db>,
    current: Option<String>,
    new_pin: String,
) -> AppResult<()> {
    if new_pin.len() < 4 || !new_pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::InvalidInput(
            "PIN must be at least 4 digits".to_string(),
        ));
    }
    let conn = lock_conn(&db)?;
    let stored = crate::db::settings::get_value(&conn, "admin_pin_hash")?;
    if !stored.is_empty() {
        match current {
            Some(value) => {
                let parsed = PasswordHash::new(&stored)
                    .map_err(|_| AppError::InvalidInput("Stored PIN hash is invalid".to_string()))?;
                Argon2::default()
                    .verify_password(value.as_bytes(), &parsed)
                    .map_err(|_| AppError::IncorrectPin)?;
            }
            None => return Err(AppError::PinAlreadySet),
        }
    }
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(new_pin.as_bytes(), &salt)
        .map_err(|e| AppError::InvalidInput(format!("Cannot hash PIN: {e}")))?;
    crate::db::settings::set_value(&conn, "admin_pin_hash", &hash.to_string())
}

#[command]
pub fn verify_admin_pin(db: tauri::State<'_, Db>, pin: String) -> AppResult<bool> {
    let conn = lock_conn(&db)?;
    let stored = crate::db::settings::get_value(&conn, "admin_pin_hash")?;
    if stored.is_empty() {
        return Err(AppError::PinNotSet);
    }
    let parsed = PasswordHash::new(&stored)
        .map_err(|_| AppError::InvalidInput("Stored PIN hash is invalid".to_string()))?;
    Ok(Argon2::default().verify_password(pin.as_bytes(), &parsed).is_ok())
}

#[command]
pub fn clear_admin_pin(db: tauri::State<'_, Db>, current: String) -> AppResult<()> {
    let conn = lock_conn(&db)?;
    let stored = crate::db::settings::get_value(&conn, "admin_pin_hash")?;
    if stored.is_empty() {
        return Err(AppError::PinNotSet);
    }
    let parsed = PasswordHash::new(&stored)
        .map_err(|_| AppError::InvalidInput("Stored PIN hash is invalid".to_string()))?;
    Argon2::default()
        .verify_password(current.as_bytes(), &parsed)
        .map_err(|_| AppError::IncorrectPin)?;
    crate::db::settings::set_value(&conn, "admin_pin_hash", "")
}

#[command]
pub fn backup_database(db: tauri::State<'_, Db>, path: String) -> AppResult<()> {
    let target = PathBuf::from(&path);
    if target.exists() {
        std::fs::remove_file(&target)?;
    }
    db.backup_to(&target)
}

#[command]
pub fn start_mobile_server(
    db: tauri::State<'_, Db>,
    server: tauri::State<'_, crate::mobile::MobileServer>,
) -> AppResult<crate::mobile::MobileServerState> {
    server.start(db.inner().clone())
}

#[command]
pub fn stop_mobile_server(
    server: tauri::State<'_, crate::mobile::MobileServer>,
) -> AppResult<crate::mobile::MobileServerState> {
    server.stop()
}

#[command]
pub fn mobile_server_status(
    server: tauri::State<'_, crate::mobile::MobileServer>,
) -> AppResult<crate::mobile::MobileServerState> {
    server.status()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duty_roster_csv_exposes_report_columns() {
        let row = DutyRosterCsvRow {
            duty_date: "2026-11-25".to_string(),
            employee_name: "Jane Doe".to_string(),
            duty: "Front desk".to_string(),
            shift_name: "Morning".to_string(),
            start: csv_time(Some("2026-11-25T10:00:00+08:00")),
            end: csv_time(Some("2026-11-25T12:00:00+08:00")),
            attendance_status: "completed".to_string(),
            clock_in: csv_time(Some("2026-11-25T10:05:00+08:00")),
            clock_out: csv_time(Some("2026-11-25T12:00:00+08:00")),
            worked_minutes: 115,
            late_minutes: 5,
            early_minutes: 0,
            notes: "Covering reception".to_string(),
        };

        let mut writer = csv::Writer::from_writer(Vec::new());
        writer.serialize(row).unwrap();
        writer.flush().unwrap();
        let output = String::from_utf8(writer.into_inner().unwrap()).unwrap();
        let mut lines = output.lines();
        assert_eq!(
            lines.next().unwrap(),
            "Date,Employee,Duty,Shift,Start,End,Status,Clock in,Clock out,Worked (min),Late (min),Early (min),Notes"
        );
        assert!(lines.next().unwrap().starts_with(
            "2026-11-25,Jane Doe,Front desk,Morning,10:00,12:00,completed,10:05,12:00,115,5,0,Covering reception"
        ));
    }
}
