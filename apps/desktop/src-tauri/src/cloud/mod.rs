pub mod apply;
pub mod client;
pub mod config;
pub mod import;
pub mod mapping;
pub mod outbox;
pub mod queue;
pub mod sync;

pub use config::CloudConfigView;
pub use outbox::{CloudConflict, OutboxItemView};
pub use sync::SyncResult;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{employees, lock_conn, Db};
    use crate::models::CreateEmployee;
    use std::sync::{Arc, Mutex};

    fn test_db() -> Db {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../db/migrations.sql")).unwrap();
        Db {
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    #[test]
    fn applying_a_server_change_overwrites_the_local_copy() {
        let db = test_db();
        let conn = lock_conn(&db).unwrap();
        let employee = employees::create(
            &conn,
            &CreateEmployee {
                code: "EMP-1".to_string(),
                name: "Local name".to_string(),
                department: None,
                role: None,
                pin: None,
            },
        )
        .unwrap();

        apply::apply_employee(
            &conn,
            &serde_json::json!({
                "id": "server-employee-id",
                "code": "EMP-1",
                "name": "Server name",
                "department": null,
                "role": null,
                "active": true,
                "pin_hash": "",
                "updated_at": "2026-11-25T10:00:00Z",
            }),
        )
        .unwrap();

        let stored = employees::get(&conn, employee.id).unwrap();
        assert_eq!(stored.name, "Server name");
        let reference = mapping::get(&conn, mapping::EMPLOYEE, employee.id)
            .unwrap()
            .unwrap();
        assert_eq!(reference.remote_id, "server-employee-id");
        assert_eq!(reference.version, "2026-11-25T10:00:00Z");
    }
}
