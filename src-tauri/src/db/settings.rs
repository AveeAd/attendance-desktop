use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppResult;
use crate::models::AppSettings;

pub fn get_value(conn: &Connection, key: &str) -> AppResult<String> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| {
            row.get(0)
        })
        .optional()?
        .unwrap_or_default())
}

pub fn set_value(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn parse_or<T: std::str::FromStr>(value: &str, fallback: T) -> T {
    value.parse().unwrap_or(fallback)
}

pub fn get_all(conn: &Connection) -> AppResult<AppSettings> {
    let defaults = AppSettings::default();
    let pin_hash = get_value(conn, "admin_pin_hash")?;
    Ok(AppSettings {
        company_name: {
            let v = get_value(conn, "company_name")?;
            if v.is_empty() { defaults.company_name } else { v }
        },
        overtime_daily_hours: parse_or(&get_value(conn, "overtime_daily_hours")?, defaults.overtime_daily_hours),
        overtime_weekly_hours: parse_or(&get_value(conn, "overtime_weekly_hours")?, defaults.overtime_weekly_hours),
        idle_lock_minutes: parse_or(&get_value(conn, "idle_lock_minutes")?, defaults.idle_lock_minutes),
        pin_enabled: !pin_hash.is_empty(),
    })
}
