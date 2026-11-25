mod cloud;
mod commands;
mod db;
mod error;
mod mobile;
mod models;
mod reporting;

use std::time::Duration;

use db::Db;
use mobile::MobileServer;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let db = Db::open(app.handle())?;
            app.manage(db.clone());
            app.manage(MobileServer::default());

            let sync_db = db.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(15));
                loop {
                    match cloud::sync::sync_if_configured(&sync_db) {
                        Ok(Some(_)) => {}
                        Ok(None) => {}
                        Err(error) => {
                            cloud::sync::fail(&sync_db, &error.to_string());
                            eprintln!("Cloud sync failed: {error}");
                        }
                    }
                    std::thread::sleep(Duration::from_secs(60));
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_employees,
            commands::get_employee,
            commands::create_employee,
            commands::update_employee,
            commands::set_employee_active,
            commands::clock_event,
            commands::list_events,
            commands::create_manual_event,
            commands::delete_event,
            commands::get_employee_statuses,
            commands::list_shifts,
            commands::create_shift,
            commands::update_shift,
            commands::delete_shift,
            commands::list_assignments,
            commands::create_assignment,
            commands::delete_assignment,
            commands::get_schedule,
            commands::list_duty_roster,
            commands::create_duty_roster,
            commands::update_duty_roster,
            commands::delete_duty_roster,
            commands::generate_duty_roster,
            commands::export_duty_roster_csv,
            commands::get_timesheet,
            commands::export_csv,
            commands::export_xlsx,
            commands::get_settings,
            commands::update_settings,
            commands::set_admin_pin,
            commands::verify_admin_pin,
            commands::clear_admin_pin,
            commands::backup_database,
            commands::start_mobile_server,
            commands::stop_mobile_server,
            commands::mobile_server_status,
            commands::get_cloud_config,
            commands::configure_cloud,
            commands::disconnect_cloud,
            commands::sync_cloud,
            commands::cloud_conflicts,
            commands::acknowledge_cloud_conflicts,
            commands::cloud_outbox,
            commands::retry_cloud_writes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Attendance");
}
