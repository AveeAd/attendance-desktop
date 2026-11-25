use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use rand_core::{OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension};

use crate::db::now_iso;
use crate::error::{AppError, AppResult};
use crate::models::{CreateEmployee, Employee, UpdateEmployee};

fn row_to_employee(row: &rusqlite::Row<'_>) -> rusqlite::Result<Employee> {
    Ok(Employee {
        id: row.get("id")?,
        code: row.get("code")?,
        name: row.get("name")?,
        department: row.get("department")?,
        role: row.get("role")?,
        active: row.get::<_, i64>("active")? == 1,
        pin_hash: row.get("pin_hash")?,
        has_pin: !row.get::<_, String>("pin_hash")?.is_empty(),
        qr_token: row.get("qr_token")?,
        created_at: row.get("created_at")?,
    })
}

const SELECT: &str = "SELECT id, code, name, department, role, active, pin_hash, qr_token, created_at FROM employees";

pub fn list(conn: &Connection, active_only: bool) -> AppResult<Vec<Employee>> {
    let sql = if active_only {
        format!("{SELECT} WHERE active = 1 ORDER BY name COLLATE NOCASE")
    } else {
        format!("{SELECT} ORDER BY active DESC, name COLLATE NOCASE")
    };
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], row_to_employee)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Employee> {
    conn.query_row(&format!("{SELECT} WHERE id = ?1"), params![id], row_to_employee)
        .map_err(|_| AppError::EmployeeNotFound(id.to_string()))
}

pub fn get_by_identifier(conn: &Connection, identifier: &str) -> AppResult<Employee> {
    let value = identifier.trim();
    if value.is_empty() {
        return Err(AppError::InvalidInput("Identifier is required".to_string()));
    }
    conn.query_row(
        &format!("{SELECT} WHERE active = 1 AND (lower(code) = lower(?1) OR qr_token = ?1)"),
        params![value],
        row_to_employee,
    )
    .optional()?
    .ok_or_else(|| AppError::EmployeeNotFound(value.to_string()))
}

fn new_token() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn hash_pin(pin: &str) -> AppResult<String> {
    let value = pin.trim();
    if value.len() < 4 || !value.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::InvalidInput(
            "PIN must be at least 4 digits".to_string(),
        ));
    }
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(value.as_bytes(), &salt)
        .map_err(|e| AppError::InvalidInput(format!("Cannot hash PIN: {e}")))?;
    Ok(hash.to_string())
}

pub fn verify_pin(employee: &Employee, pin: &str) -> bool {
    use argon2::password_hash::PasswordHash;
    use argon2::PasswordVerifier;
    if employee.pin_hash.is_empty() {
        return true;
    }
    let Ok(parsed) = PasswordHash::new(&employee.pin_hash) else {
        return false;
    };
    Argon2::default().verify_password(pin.as_bytes(), &parsed).is_ok()
}

pub fn create(conn: &Connection, input: &CreateEmployee) -> AppResult<Employee> {
    let code = input.code.trim();
    let name = input.name.trim();
    if code.is_empty() || name.is_empty() {
        return Err(AppError::InvalidInput(
            "Employee code and name are required".to_string(),
        ));
    }
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM employees WHERE lower(code) = lower(?1))",
        params![code],
        |row| row.get(0),
    )?;
    if exists {
        return Err(AppError::InvalidInput(format!(
            "Employee code '{code}' is already in use"
        )));
    }
    let token = new_token();
    let now = now_iso();
    let pin_hash = match input.pin.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        Some(pin) => hash_pin(pin)?,
        None => String::new(),
    };
    conn.execute(
        "INSERT INTO employees(code, name, department, role, active, pin_hash, qr_token, created_at)
         VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7)",
        params![
            code,
            name,
            input.department.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            input.role.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            pin_hash,
            token,
            now
        ],
    )?;
    let id = conn.last_insert_rowid();
    get(conn, id)
}

pub fn update(conn: &Connection, id: i64, input: &UpdateEmployee) -> AppResult<Employee> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::InvalidInput("Employee name is required".to_string()));
    }
    let existing = get(conn, id)?;
    let pin_hash = match input.pin.as_deref() {
        Some(pin) if !pin.trim().is_empty() => hash_pin(pin)?,
        Some(_) => String::new(),
        None => existing.pin_hash.clone(),
    };
    conn.execute(
        "UPDATE employees SET name = ?2, department = ?3, role = ?4, active = ?5, pin_hash = ?6 WHERE id = ?1",
        params![
            id,
            name,
            input.department.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            input.role.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            input.active as i64,
            pin_hash,
        ],
    )?;
    get(conn, id)
}

pub fn set_active(conn: &Connection, id: i64, active: bool) -> AppResult<()> {
    let changed = conn.execute(
        "UPDATE employees SET active = ?2 WHERE id = ?1",
        params![id, active as i64],
    )?;
    if changed == 0 {
        return Err(AppError::EmployeeNotFound(id.to_string()));
    }
    Ok(())
}
