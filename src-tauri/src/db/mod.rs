pub mod duty_roster;
pub mod employees;
pub mod events;
pub mod settings;
pub mod shifts;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct Db {
    pub conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(app: &AppHandle) -> AppResult<Self> {
        let dir: PathBuf = app
            .path()
            .app_data_dir()
            .map_err(|e| AppError::InvalidInput(format!("Cannot resolve app data dir: {e}")))?;
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("attendance.db");
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON;",
        )?;
        conn.execute_batch(include_str!("migrations.sql"))?;
        apply_upgrades(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn backup_to(&self, target: &PathBuf) -> AppResult<()> {
        let target_str = target.to_string_lossy().replace('\'', "''");
        let conn = self.conn.lock().map_err(|_| {
            AppError::InvalidInput("Database connection is unavailable".to_string())
        })?;
        conn.execute(&format!("VACUUM INTO '{target_str}'"), [])?;
        Ok(())
    }
}

/// Adds columns introduced after the initial schema without touching existing data.
fn apply_upgrades(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(employees)")?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !columns.iter().any(|name| name == "pin_hash") {
        conn.execute(
            "ALTER TABLE employees ADD COLUMN pin_hash TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    Ok(())
}

pub fn lock_conn(db: &Db) -> AppResult<std::sync::MutexGuard<'_, Connection>> {
    db.conn.lock().map_err(|_| {
        AppError::InvalidInput("Database connection is unavailable".to_string())
    })
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn parse_timestamp(value: &str) -> AppResult<DateTime<chrono::FixedOffset>> {
    DateTime::parse_from_rfc3339(value)
        .map_err(|_| AppError::InvalidTimestamp(value.to_string()))
}

pub fn timestamp_or_now(value: Option<&str>) -> AppResult<(String, i64)> {
    match value {
        Some(raw) => {
            let parsed = parse_timestamp(raw)?;
            Ok((parsed.to_rfc3339(), parsed.timestamp()))
        }
        None => {
            let now = Utc::now();
            Ok((now.to_rfc3339(), now.timestamp()))
        }
    }
}

pub fn days_to_mask(days: &[i64]) -> i64 {
    days.iter().fold(0, |mask, day| {
        if (1..=7).contains(day) {
            mask | (1 << (day - 1))
        } else {
            mask
        }
    })
}

pub fn mask_to_days(mask: i64) -> Vec<i64> {
    (1..=7).filter(|day| mask & (1 << (day - 1)) != 0).collect()
}

pub fn parse_time(value: &str) -> AppResult<(u32, u32)> {
    let mut parts = value.split(':');
    let hour: u32 = parts
        .next()
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| AppError::InvalidInput(format!("Invalid time: {value}")))?;
    let minute: u32 = parts
        .next()
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| AppError::InvalidInput(format!("Invalid time: {value}")))?;
    if hour > 23 || minute > 59 || parts.next().is_some() {
        return Err(AppError::InvalidInput(format!("Invalid time: {value}")));
    }
    Ok((hour, minute))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        AssignmentInput, CreateEmployee, DutyRosterInput, Employee, EventType, Shift, ShiftInput,
    };

    fn test_db() -> Db {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("migrations.sql")).unwrap();
        Db {
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    fn make_employee(conn: &Connection, code: &str, name: &str) -> Employee {
        employees::create(
            conn,
            &CreateEmployee {
                code: code.to_string(),
                name: name.to_string(),
                department: None,
                role: None,
                pin: None,
            },
        )
        .unwrap()
    }

    fn make_shift(conn: &Connection, name: &str, days: &[i64], active: bool) -> Shift {
        shifts::create_shift(
            conn,
            &ShiftInput {
                name: name.to_string(),
                start_time: "10:00".to_string(),
                end_time: "12:00".to_string(),
                days: days.to_vec(),
                break_minutes: 15,
                active,
            },
        )
        .unwrap()
    }

    fn duty_input(
        employee_id: i64,
        duty_date: &str,
        shift_id: Option<i64>,
        duty: &str,
    ) -> DutyRosterInput {
        DutyRosterInput {
            employee_id,
            duty_date: duty_date.to_string(),
            shift_id,
            duty: duty.to_string(),
            notes: None,
        }
    }

    #[test]
    fn employee_crud_and_identifier_lookup() {
        let db = test_db();
        let conn = lock_conn(&db).unwrap();
        let created = employees::create(
            &conn,
            &CreateEmployee {
                code: "EMP-001".to_string(),
                name: "Jane Doe".to_string(),
                department: Some("Ops".to_string()),
                role: None,
                pin: None,
            },
        )
        .unwrap();
        assert!(created.active);
        assert_eq!(created.qr_token.len(), 32);

        let by_code = employees::get_by_identifier(&conn, " emp-001 ").unwrap();
        assert_eq!(by_code.id, created.id);
        let by_token = employees::get_by_identifier(&conn, &created.qr_token).unwrap();
        assert_eq!(by_token.id, created.id);

        assert!(employees::create(
            &conn,
            &CreateEmployee {
                code: "emp-001".to_string(),
                name: "Duplicate".to_string(),
                department: None,
                role: None,
                pin: None,
            }
        )
        .is_err());
    }

    #[test]
    fn employee_pin_is_hashed_and_verified() {
        let db = test_db();
        let conn = lock_conn(&db).unwrap();
        let employee = employees::create(
            &conn,
            &CreateEmployee {
                code: "EMP-PIN".to_string(),
                name: "Pin User".to_string(),
                department: None,
                role: None,
                pin: Some("1234".to_string()),
            },
        )
        .unwrap();

        assert!(employee.has_pin);
        assert!(!employee.pin_hash.is_empty());
        assert!(!employee.pin_hash.contains("1234"));
        assert!(employees::verify_pin(&employee, "1234"));
        assert!(!employees::verify_pin(&employee, "0000"));
        assert!(employees::hash_pin("12").is_err());
    }

    #[test]
    fn events_persist_with_epoch_for_sorting() {
        let db = test_db();
        let conn = lock_conn(&db).unwrap();
        let employee = employees::create(
            &conn,
            &CreateEmployee {
                code: "EMP-002".to_string(),
                name: "John Roe".to_string(),
                department: None,
                role: None,
                pin: None,
            },
        )
        .unwrap();
        events::insert(
            &conn,
            employee.id,
            EventType::ClockIn,
            "2026-11-25T09:00:00+08:00",
            1_764_000_000,
            "scan",
            None,
        )
        .unwrap();
        events::insert(
            &conn,
            employee.id,
            EventType::ClockOut,
            "2026-11-25T17:00:00+08:00",
            1_764_028_800,
            "scan",
            None,
        )
        .unwrap();
        let events = events::list(&conn, Some(employee.id), None, None, 10).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, EventType::ClockOut);
        assert_eq!(events[1].event_type, EventType::ClockIn);
    }

    #[test]
    fn settings_have_defaults() {
        let db = test_db();
        let conn = lock_conn(&db).unwrap();
        let settings = settings::get_all(&conn).unwrap();
        assert_eq!(settings.company_name, "My Organization");
        assert!(!settings.pin_enabled);
        assert_eq!(settings.overtime_daily_hours, 8.0);
    }

    #[test]
    fn duty_roster_crud_filters_by_date() {
        let db = test_db();
        let conn = lock_conn(&db).unwrap();
        let employee = make_employee(&conn, "DUTY-001", "Duty Tester");

        let first = duty_roster::create(
            &conn,
            &duty_input(employee.id, "2026-11-25", None, "Front desk"),
        )
        .unwrap();
        duty_roster::create(&conn, &duty_input(employee.id, "2026-11-27", None, "Patrol")).unwrap();

        let monday = duty_roster::list_for_range(&conn, "2026-11-25", "2026-11-26").unwrap();
        assert_eq!(monday.len(), 1);
        assert_eq!(monday[0].duty, "Front desk");
        assert_eq!(monday[0].employee_name, "Duty Tester");

        duty_roster::update(
            &conn,
            first.id,
            &duty_input(employee.id, "2026-11-27", None, "Reception"),
        )
        .unwrap();
        let week = duty_roster::list_for_range(&conn, "2026-11-23", "2026-11-29").unwrap();
        assert_eq!(week.len(), 2);
        assert!(week.iter().all(|entry| entry.duty_date == "2026-11-27"));

        duty_roster::delete(&conn, first.id).unwrap();
        assert!(duty_roster::get(&conn, first.id).is_err());
        assert!(duty_roster::delete(&conn, first.id).is_err());
        assert_eq!(
            duty_roster::list_for_range(&conn, "2026-11-23", "2026-11-29")
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn duty_roster_generation_dedupes_and_respects_filters() {
        let db = test_db();
        let mut conn = lock_conn(&db).unwrap();
        let active = make_shift(&conn, "Monday shift", &[1], true);
        let inactive = make_shift(&conn, "Retired shift", &[1], false);

        let on_main = make_employee(&conn, "DUTY-002", "On Schedule");
        let starts_late = make_employee(&conn, "DUTY-003", "Starts Later");
        let ended = make_employee(&conn, "DUTY-004", "Already Ended");
        let retired = make_employee(&conn, "DUTY-005", "Retired Shift");

        for (employee_id, shift_id, from, to) in [
            (on_main.id, active.id, "2026-11-01", None),
            (starts_late.id, active.id, "2026-11-24", None),
            (ended.id, active.id, "2026-11-01", Some("2026-11-22")),
            (retired.id, inactive.id, "2026-11-01", None),
        ] {
            shifts::create_assignment(
                &conn,
                &AssignmentInput {
                    employee_id,
                    shift_id,
                    effective_from: from.to_string(),
                    effective_to: to.map(str::to_string),
                },
            )
            .unwrap();
        }

        let first = duty_roster::generate_from_schedule(&mut conn, "2026-11-23").unwrap();
        assert_eq!(first.created, 1);
        assert_eq!(first.skipped, 0);

        let second = duty_roster::generate_from_schedule(&mut conn, "2026-11-23").unwrap();
        assert_eq!(second.created, 0);
        assert_eq!(second.skipped, 1);

        let entries = duty_roster::list_for_range(&conn, "2026-11-23", "2026-11-29").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].employee_id, on_main.id);
        assert_eq!(entries[0].shift_id, Some(active.id));
        assert_eq!(entries[0].duty, "Monday shift");
        assert_eq!(entries[0].source, "generated");
    }
}
